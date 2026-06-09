use crate::models::{Album, Artist, Disk, Library, Track};
use crossbeam_channel::Sender;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rayon::prelude::*; // NIEUW: Rayon imports
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Mutex; // NIEUW: Mutex voor thread-safe schrijven
use walkdir::WalkDir;

pub enum ScannerMessage {
    Progress(String),
    LibraryLoaded(Library),
    ScanComplete,
}

const CACHE_FILE: &str = "library_cache.bin";

pub async fn load_or_scan_library(
    dir: String,
    audio_exts: Vec<String>,
    cover_names: Vec<String>,
    cover_exts: Vec<String>,
    tx: Sender<ScannerMessage>,
) {
    // 1. Probeer de cache in te laden
    if Path::new(CACHE_FILE).exists() {
        let _ = tx.send(ScannerMessage::Progress("Cache laden...".into()));
        if let Ok(file) = File::open(CACHE_FILE) {
            let reader = BufReader::new(file);
            if let Ok(library) = bincode::deserialize_from(reader) {
                let _ = tx.send(ScannerMessage::LibraryLoaded(library));
                let _ = tx.send(ScannerMessage::ScanComplete);
                return;
            }
        }
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
                    let mut genre: String = "Unknown Genre".to_string();

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

                    // Lees Genre tag
                    if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                        if let Some(tag) = tagged_file.primary_tag() {
                            if let Some(g) = tag.genre() {
                                genre = g.to_string();
                            }
                        }
                    }

                    let track = Track {
                        path: path.to_string_lossy().to_string(),
                        title: path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        track_number: 0,
                        duration_secs: 0,
                        genre: Some(genre),
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
        let _ = bincode::serialize_into(writer, &library);
    }

    // 5. Stuur het eindresultaat
    let _ = tx.send(ScannerMessage::LibraryLoaded(library));
    let _ = tx.send(ScannerMessage::ScanComplete);
}
