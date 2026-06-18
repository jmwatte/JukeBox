use crate::models::{Album, Artist, Disk, Library, Track};
use crossbeam_channel::Sender;
use lofty::file::AudioFile;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Mutex;
use walkdir::WalkDir;

pub enum ScannerMessage {
    Progress(String),
    LibraryLoaded(Library),
    ScanComplete,
}

pub const CACHE_VERSION: u32 = 1;
pub const CACHE_FILE: &str = "library_cache.bin";

#[derive(Serialize, Deserialize)]
struct CacheData {
    version: u32,
    dir_modified: u64, // UNIX timestamp van de muziekmap bij cache-aanmaak
    library: Library,
}

/// Sla een Library direct naar de cache, zonder een volledige herscan.
/// Dit is handig nadat tags in-memory zijn aangepast.
/// OPMERKING: dir_modified wordt op 0 gezet, dus volgende startup zal opnieuw scannen.
pub fn save_cache(library: &Library) {
    if let Ok(file) = std::fs::File::create(CACHE_FILE) {
        let writer = std::io::BufWriter::new(file);
        let data = CacheData {
            version: CACHE_VERSION,
            dir_modified: 0,
            library: library.clone(),
        };
        let _ = bincode::serialize_into(writer, &data);
    }
}

pub async fn load_or_scan_library(
    dir: String,
    audio_exts: Vec<String>,
    cover_names: Vec<String>,
    cover_exts: Vec<String>,
    tx: Sender<ScannerMessage>,
) {
    // Huidige modificatietijd van de muziekdirectory
    let current_dir_modified = std::fs::metadata(&dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // 1. Probeer de cache in te laden
    if Path::new(CACHE_FILE).exists() {
        let _ = tx.send(ScannerMessage::Progress("Cache laden...".into()));
        if let Ok(file) = File::open(CACHE_FILE) {
            let reader = BufReader::new(file);
            match bincode::deserialize_from::<_, CacheData>(reader) {
                Ok(cache)
                    if cache.version == CACHE_VERSION
                        && cache.dir_modified == current_dir_modified =>
                {
                    let library = cache.library;
                    if !library.artists.is_empty() {
                        let _ = tx.send(ScannerMessage::LibraryLoaded(library));
                        let _ = tx.send(ScannerMessage::ScanComplete);
                        return;
                    }
                }
                Ok(cache) => {
                    if cache.version != CACHE_VERSION {
                        println!(
                            "Cache versie {} != verwachte {} — opnieuw scannen.",
                            cache.version, CACHE_VERSION
                        );
                    } else {
                        println!("Muziekmap gewijzigd sinds cache — opnieuw scannen.");
                    }
                }
                Err(e) => {
                    println!("Cache corrupt ({:?}) — opnieuw scannen.", e);
                }
            }
        }
        // Cache was leeg, corrupt of verouderd — verwijder hem en scan opnieuw
        let _ = std::fs::remove_file(CACHE_FILE);
    }

    let _ = tx.send(ScannerMessage::Progress(
        "Eerste indexering gestart (parallel met Rayon)... ".into(),
    ));

    // NIEUW: Wrap de HashMaps in een Mutex zodat meerdere threads er veilig in kunnen schrijven
    let artists_map = Mutex::new(HashMap::<
        String,
        HashMap<String, HashMap<String, Vec<Track>>>,
    >::new());
    let album_covers = Mutex::new(HashMap::<String, String>::new());

    // NIEUW: par_bridge() maakt de WalkDir iterator parallel
    WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .par_bridge()
        .for_each(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return;
            }

            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let file_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let parent_dir = path.parent().unwrap_or(Path::new(" "));

            // Check albumhoes
            if cover_exts.contains(&ext) {
                let is_cover = cover_names.iter().any(|name| file_name.contains(name));
                if is_cover {
                    let dir_str = parent_dir.to_string_lossy().to_string();
                    album_covers
                        .lock()
                        .unwrap()
                        .insert(dir_str, path.to_string_lossy().to_string());
                    return;
                }
            }

            // Check audiobestand
            if audio_exts.contains(&ext) {
                let base_dir = Path::new(&dir);
                if let Ok(rel_path) = path.strip_prefix(base_dir) {
                    let components: Vec<String> = rel_path
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();

                    let mut artist_name = "Onbekende Artiest".to_string();
                    let mut album_name = "Onbekend Album".to_string();
                    let mut disk_name = "Default".to_string();
                    let mut genre: String = "".to_string();

                    if components.len() == 1 {
                        // Bestand staat direct in H:\MUSIC\
                    } else if components.len() == 2 {
                        artist_name = components[0].clone();
                    } else {
                        artist_name = components[0].clone();
                        let folder_chain = &components[1..components.len() - 1];

                        if let Some(last_folder) = folder_chain.last() {
                            let is_cd = last_folder.to_lowercase().starts_with("cd")
                                || last_folder.to_lowercase().starts_with("disc");

                            if is_cd && folder_chain.len() > 1 {
                                disk_name = last_folder.clone();
                                album_name = folder_chain[..folder_chain.len() - 1].join(" - ");
                            } else {
                                album_name = folder_chain.join(" - ");
                            }
                        }
                    }
                    // Lees ALLE metadata uit tags
                    let mut title: Option<String> = None;
                    let mut track_number: Option<u32> = None;
                    let mut disc_number: Option<u32> = None;
                    let mut track_artist: Option<String> = None;
                    let mut album_artist: Option<String> = None;
                    let mut year: Option<u32> = None;
                    let mut composer: Option<String> = None;
                    let mut duration_secs: u32 = 0;

                    if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                        let mut all_genres = Vec::new();

                        for tag in tagged_file.tags() {
                            // Titel en artiest via Accessor (werkt over alle tag-standaarden)
                            if title.is_none() {
                                if let Some(t) = tag.title() {
                                    title = Some(t.to_string());
                                }
                            }
                            if track_artist.is_none() {
                                if let Some(a) = tag.artist() {
                                    track_artist = Some(a.to_string());
                                }
                            }

                            for item in tag.items() {
                                match item.key() {
                                    // 1. Tracknummer
                                    lofty::tag::ItemKey::TrackNumber => {
                                        if track_number.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                track_number = text.parse::<u32>().ok();
                                            }
                                        }
                                    }

                                    // 2. Schijfnummer
                                    lofty::tag::ItemKey::DiscNumber => {
                                        if disc_number.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                disc_number = text.parse::<u32>().ok();
                                            }
                                        }
                                    }

                                    // 3. Album artiest
                                    lofty::tag::ItemKey::AlbumArtist => {
                                        if album_artist.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                album_artist = Some(text.to_string());
                                            }
                                        }
                                    }

                                    // 4. Genre (alle tags verzamelen)
                                    lofty::tag::ItemKey::Genre => {
                                        if let lofty::tag::ItemValue::Text(text) = item.value() {
                                            all_genres.push(text.clone());
                                        }
                                    }

                                    // 5. Custom iTunes Genre tag
                                    lofty::tag::ItemKey::Unknown(key)
                                        if key.to_lowercase() == "----:com.apple.itunes:genre" =>
                                    {
                                        if let lofty::tag::ItemValue::Text(text) = item.value() {
                                            all_genres.push(text.clone());
                                        }
                                    }

                                    // 6. Jaartallen
                                    lofty::tag::ItemKey::Year
                                    | lofty::tag::ItemKey::RecordingDate
                                    | lofty::tag::ItemKey::OriginalReleaseDate => {
                                        if year.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                let year_str: String =
                                                    text.chars().take(4).collect();
                                                year = year_str.parse::<u32>().ok();
                                            }
                                        }
                                    }

                                    // 7. Jaartal-fallbacks (custom keys)
                                    lofty::tag::ItemKey::Unknown(key)
                                        if key.to_lowercase() == "originalyear"
                                            || key.to_lowercase() == "toryear" =>
                                    {
                                        if year.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                let year_str: String =
                                                    text.chars().take(4).collect();
                                                year = year_str.parse::<u32>().ok();
                                            }
                                        }
                                    }

                                    // 8. Componist
                                    lofty::tag::ItemKey::Composer => {
                                        if composer.is_none() {
                                            if let lofty::tag::ItemValue::Text(text) = item.value()
                                            {
                                                composer = Some(text.to_string());
                                            }
                                        }
                                    }

                                    _ => {}
                                }
                            }
                        }

                        // Duur uit properties
                        duration_secs = tagged_file.properties().duration().as_secs() as u32;

                        // Voeg alle gevonden genres samen met separator
                        if !all_genres.is_empty() {
                            genre = all_genres.join(";");
                        }
                    }

                    let track_title = title.unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });
                    let track = Track {
                        path: path.to_string_lossy().to_string(),
                        title: track_title,
                        artist: track_artist,
                        album_artist,
                        track_number: track_number.unwrap_or(0),
                        disc_number: disc_number.unwrap_or(0),
                        duration_secs,
                        genre: Some(genre),
                        year,
                        composer,
                    };

                    // NIEUW: Lock de mutex kort om de track toe te voegen
                    artists_map
                        .lock()
                        .unwrap()
                        .entry(artist_name)
                        .or_default()
                        .entry(album_name)
                        .or_default()
                        .entry(disk_name)
                        .or_default()
                        .push(track);
                }
            }
        });

    // Unwrap de mutexes terug naar normale HashMaps
    let artists_map = artists_map.into_inner().unwrap();
    let album_covers = album_covers.into_inner().unwrap();

    let _ = tx.send(ScannerMessage::Progress(
        "Bibliotheek structureren... ".into(),
    ));

    // NIEUW: Parallelle conversie van HashMap naar Library struct
    let mut artists_vec: Vec<Artist> = artists_map
        .par_iter()
        .map(|(artist_name, albums_map)| {
            let mut artist = Artist {
                name: artist_name.clone(),
                albums: Vec::new(),
            };

            for (album_name, disks_map) in albums_map {
                let mut album = Album {
                    title: album_name.clone(),
                    cover_path: None,
                    disks: Vec::new(),
                    added_timestamp: 0,
                };
                let mut max_time = 0;
                for (disk_name, tracks) in disks_map {
                    let mut sorted_tracks = tracks.clone();
                    sorted_tracks.sort_by(|a, b| natord::compare(&a.title, &b.title));

                    for track in &sorted_tracks {
                        if let Ok(meta) = std::fs::metadata(&track.path) {
                            // Try created date first, fallback to modified date
                            let time = meta
                                .created()
                                .or_else(|_| meta.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

                            let secs = time
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            if secs > max_time {
                                max_time = secs;
                            }
                        }
                    }

                    album.disks.push(Disk {
                        name: disk_name.clone(),
                        tracks: sorted_tracks,
                    });
                }
                album.added_timestamp = max_time;
                // Cover koppelen
                if let Some(first_disk) = album.disks.first() {
                    if let Some(first_track) = first_disk.tracks.first() {
                        let track_path = Path::new(&first_track.path);
                        if let Some(parent) = track_path.parent() {
                            let parent_str = parent.to_string_lossy().to_string();
                            album.cover_path = album_covers.get(&parent_str).cloned();

                            if album.cover_path.is_none() {
                                if let Some(grandparent) = parent.parent() {
                                    let grand_str = grandparent.to_string_lossy().to_string();
                                    album.cover_path = album_covers.get(&grand_str).cloned();
                                }
                            }
                        }
                    }
                }
                artist.albums.push(album);
            }
            artist.albums.sort_by(|a, b| a.title.cmp(&b.title));
            artist
        })
        .collect();

    // Sorteer artiesten alfabetisch
    artists_vec.sort_by(|a, b| a.name.cmp(&b.name));
    let library = Library {
        artists: artists_vec,
    };

    // 4. Sla de cache op
    let _ = tx.send(ScannerMessage::Progress("Bibliotheek opslaan... ".into()));
    if let Ok(file) = File::create(CACHE_FILE) {
        let writer = BufWriter::new(file);
        let data = CacheData {
            version: CACHE_VERSION,
            dir_modified: current_dir_modified,
            library: library.clone(),
        };
        let _ = bincode::serialize_into(writer, &data);
    }

    // 5. Stuur het eindresultaat
    let _ = tx.send(ScannerMessage::LibraryLoaded(library));
    let _ = tx.send(ScannerMessage::ScanComplete);
}
