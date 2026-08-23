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
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use walkdir::WalkDir;

pub enum ScannerMessage {
    Progress(String),
    LibraryLoaded(Library),
    ScanComplete,
    LoopsRemapped(Vec<crate::loops::SavedLoop>),
    FavoritesRemapped(Vec<String>),
    MusicDirChanged(String),
}

pub const CACHE_VERSION: u32 = 2;
pub const CACHE_FILE: &str = "library_cache.bin";

#[derive(Serialize, Deserialize)]
struct CacheData {
    version: u32,
    dir_modified: u64, // UNIX timestamp van de muziekmap bij cache-aanmaak
    // Map waaruit de cache is opgebouwd. Nodig om te herkennen dat alleen de
    // schijfletter (of het pad) is gewijzigd, zodat de cache zonder rescan
    // herbruikt kan worden.
    music_dir: String,
    library: Library,
}

/// UNIX-timestamp van de "modified"-tijd van een map (0 als de map niet bestaat).
fn dir_modified(dir: &str) -> u64 {
    std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Vergelijk twee mappaden case-insensitief en zonder staart-backslash.
/// Windows-paden zijn hoofdletterongevoelig, ook voor de schijfletter.
fn dirs_match(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim_end_matches(|c| c == '\\' || c == '/').to_lowercase();
    norm(a) == norm(b)
}

/// Vervang in één pad het oude root-gedeelte door een nieuw root-gedeelte.
/// Paden die niet onder `old_root` vallen blijven ongewijzigd.
pub(crate) fn remap_one_path(path: &str, old_root: &str, new_root: &str) -> String {
    match Path::new(path).strip_prefix(Path::new(old_root)) {
        Ok(rel) => {
            let mut new_path = PathBuf::from(new_root);
            // Component per component toevoegen zodat de scheidingstekens
            // consistent zijn met het platform (geen gemengde \ en /).
            for comp in rel.components() {
                new_path.push(comp.as_os_str());
            }
            new_path.to_string_lossy().into_owned()
        }
        Err(_) => path.to_string(),
    }
}

/// Pas alle opgeslagen paden in de bibliotheek aan van `old_root` naar `new_root`.
/// Wordt gebruikt wanneer alleen de schijfletter is gewijzigd: de inhoud is
/// identiek, dus herscannen is onnodig.
fn remap_library_paths(library: &mut Library, old_root: &str, new_root: &str) {
    for artist in &mut library.artists {
        for album in &mut artist.albums {
            if let Some(ref mut cover) = album.cover_path {
                *cover = remap_one_path(cover, old_root, new_root);
            }
            for disk in &mut album.disks {
                for track in &mut disk.tracks {
                    track.path = remap_one_path(&track.path, old_root, new_root);
                }
            }
        }
    }
}

/// Zoek de daadwerkelijk gebruikte muziekmap.
/// Als de geconfigureerde map niet (meer) bestaat — bijv. omdat de USB-schijf een
/// andere schijfletter heeft gekregen — wordt op alle mounted schijven gezocht naar
/// een map met dezelfde relatieve naam (bijv. "H:\music" → "X:\music").
fn find_effective_music_dir(configured: &str) -> String {
    if Path::new(configured).exists() {
        return configured.to_string();
    }

    // Relatief deel na de schijfletter: "H:\music" → "music"
    let tail: Vec<Component> = Path::new(configured)
        .components()
        .skip_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        .collect();
    if tail.is_empty() {
        return configured.to_string();
    }

    let mut found: Option<String> = None;
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if !Path::new(&drive).exists() {
            continue;
        }
        let mut candidate = PathBuf::from(&drive);
        for comp in &tail {
            candidate.push(comp.as_os_str());
        }
        if candidate.exists() {
            if found.is_some() {
                log::warn!(
                    "Meerdere schijven met '{}' gevonden — eerste gebruikt.",
                    tail.iter()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("\\")
                );
                return found.unwrap();
            }
            found = Some(candidate.to_string_lossy().into_owned());
        }
    }
    found.unwrap_or_else(|| configured.to_string())
}

/// Sla een Library direct naar de cache, zodat een herstart sneller laden kan.
/// `music_dir` wordt gebruikt om de wijzigingstijd vast te leggen voor cache-validatie
/// en om later schijfletterwijzigingen te kunnen herkennen.
pub fn save_cache(library: &Library, music_dir: &str) {
    if let Ok(file) = std::fs::File::create(CACHE_FILE) {
        let mut writer = std::io::BufWriter::new(file);
        let data = CacheData {
            version: CACHE_VERSION,
            dir_modified: dir_modified(music_dir),
            music_dir: music_dir.to_string(),
            library: library.clone(),
        };
        let _ =
            bincode::serde::encode_into_std_write(&data, &mut writer, bincode::config::legacy());
    }
}

pub fn load_or_scan_library(
    dir: String,
    audio_exts: Vec<String>,
    cover_names: Vec<String>,
    cover_exts: Vec<String>,
    tx: Sender<ScannerMessage>,
) {
    // Als de geconfigureerde map niet bestaat (bijv. gewijzigde schijfletter),
    // zoek dezelfde map op een andere schijf en werk de config bij.
    let effective_dir = find_effective_music_dir(&dir);
    if effective_dir != dir {
        log::info!(
            "Geconfigureerde map '{}' niet gevonden; '{}' gebruikt.",
            dir,
            effective_dir
        );
        if crate::config::update_music_directory(&effective_dir) {
            log::info!("config.toml bijgewerkt naar '{}'.", effective_dir);
        }
        // Ook de in-memory config bijwerken, zodat later opgeslagen caches
        // (bijv. na tag-wijzigingen) de juiste map registreren.
        let _ = tx.send(ScannerMessage::MusicDirChanged(effective_dir.clone()));
    }

    // Huidige modificatietijd van de muziekdirectory
    let current_dir_modified = dir_modified(&effective_dir);

    // 1. Probeer de cache in te laden
    if Path::new(CACHE_FILE).exists() {
        let _ = tx.send(ScannerMessage::Progress("Cache laden...".into()));
        if let Ok(file) = File::open(CACHE_FILE) {
            let mut reader = BufReader::new(file);
            match bincode::serde::decode_from_std_read::<CacheData, _, _>(
                &mut reader,
                bincode::config::legacy(),
            ) {
                // Snelle weg: zelfde map, zelfde inhoud → cache direct gebruiken
                Ok(cache)
                    if cache.version == CACHE_VERSION
                        && dirs_match(&cache.music_dir, &effective_dir)
                        && cache.dir_modified == current_dir_modified =>
                {
                    let library = cache.library;
                    if !library.artists.is_empty() {
                        let _ = tx.send(ScannerMessage::LibraryLoaded(library));
                        let _ = tx.send(ScannerMessage::ScanComplete);
                        return;
                    }
                }
                // Schijfletter (of mappad) gewijzigd, maar inhoud identiek:
                // cache hergebruiken en alleen de paden herschrijven — geen rescan nodig.
                Ok(cache)
                    if cache.version == CACHE_VERSION
                        && !cache.music_dir.is_empty()
                        && !dirs_match(&cache.music_dir, &effective_dir)
                        && current_dir_modified != 0
                        && cache.dir_modified == current_dir_modified =>
                {
                    let _ = tx.send(ScannerMessage::Progress(
                        "Schijfletter gewijzigd — bibliotheek opnieuw koppelen...".into(),
                    ));
                    let mut library = cache.library;
                    remap_library_paths(&mut library, &cache.music_dir, &effective_dir);
                    save_cache(&library, &effective_dir);
                    let _ = tx.send(ScannerMessage::LibraryLoaded(library));
                    let _ = tx.send(ScannerMessage::ScanComplete);
                    if let Some(loops) = crate::loops::remap_loops(&cache.music_dir, &effective_dir)
                    {
                        let _ = tx.send(ScannerMessage::LoopsRemapped(loops));
                    }
                    if let Some(favs) =
                        crate::favorites::remap_favorites(&cache.music_dir, &effective_dir)
                    {
                        let _ = tx.send(ScannerMessage::FavoritesRemapped(favs));
                    }
                    return;
                }
                Ok(cache) => {
                    if cache.version != CACHE_VERSION {
                        log::info!(
                            "Cache versie {} != verwachte {} — opnieuw scannen.",
                            cache.version,
                            CACHE_VERSION
                        );
                    } else {
                        log::info!("Muziekmap gewijzigd sinds cache — opnieuw scannen.");
                    }
                }
                Err(e) => {
                    log::warn!("Cache corrupt of verouderd ({:?}) — opnieuw scannen.", e);
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
    WalkDir::new(&effective_dir)
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
                let base_dir = Path::new(&effective_dir);
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

                                    // 5. Jaartallen
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

                                    // 6. Componist
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
                    sorted_tracks.sort_by(|a, b| {
                        a.track_number
                            .cmp(&b.track_number)
                            .then_with(|| natord::compare(&a.title, &b.title))
                    });

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
                album
                    .disks
                    .sort_by(|a, b| natord::compare(&a.name, &b.name));
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
        let mut writer = BufWriter::new(file);
        let data = CacheData {
            version: CACHE_VERSION,
            dir_modified: current_dir_modified,
            music_dir: effective_dir.clone(),
            library: library.clone(),
        };
        let _ =
            bincode::serde::encode_into_std_write(&data, &mut writer, bincode::config::legacy());
    }

    // 5. Stuur het eindresultaat
    let _ = tx.send(ScannerMessage::LibraryLoaded(library));
    let _ = tx.send(ScannerMessage::ScanComplete);
}

/// Hernummer de tags van een lijst met tracks en werk de library bij.
/// Dit is handig nadat de gebruiker tags op schijf heeft aangepast (bv. met een externe tag editor)
/// en alleen de geselecteerde tracks opnieuw wil inlezen zonder volledige rescan.
pub fn rescan_tracks(paths: &[String], library: &mut Library) {
    for path in paths {
        // Tags uitlezen (zelfde logica als in load_or_scan_library)
        let mut title: Option<String> = None;
        let mut track_number: Option<u32> = None;
        let mut disc_number: Option<u32> = None;
        let mut track_artist: Option<String> = None;
        let mut album_artist: Option<String> = None;
        let mut year: Option<u32> = None;
        let mut composer: Option<String> = None;
        let mut genre: String = String::new();
        let mut duration_secs: u32 = 0;

        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            let mut all_genres = Vec::new();

            for tag in tagged_file.tags() {
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
                        lofty::tag::ItemKey::TrackNumber => {
                            if track_number.is_none() {
                                if let lofty::tag::ItemValue::Text(text) = item.value() {
                                    track_number = text.parse::<u32>().ok();
                                }
                            }
                        }
                        lofty::tag::ItemKey::DiscNumber => {
                            if disc_number.is_none() {
                                if let lofty::tag::ItemValue::Text(text) = item.value() {
                                    disc_number = text.parse::<u32>().ok();
                                }
                            }
                        }
                        lofty::tag::ItemKey::AlbumArtist => {
                            if album_artist.is_none() {
                                if let lofty::tag::ItemValue::Text(text) = item.value() {
                                    album_artist = Some(text.to_string());
                                }
                            }
                        }
                        lofty::tag::ItemKey::Genre => {
                            if let lofty::tag::ItemValue::Text(text) = item.value() {
                                all_genres.push(text.clone());
                            }
                        }
                        lofty::tag::ItemKey::Year
                        | lofty::tag::ItemKey::RecordingDate
                        | lofty::tag::ItemKey::OriginalReleaseDate => {
                            if year.is_none() {
                                if let lofty::tag::ItemValue::Text(text) = item.value() {
                                    let year_str: String = text.chars().take(4).collect();
                                    year = year_str.parse::<u32>().ok();
                                }
                            }
                        }
                        lofty::tag::ItemKey::Composer => {
                            if composer.is_none() {
                                if let lofty::tag::ItemValue::Text(text) = item.value() {
                                    composer = Some(text.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            duration_secs = tagged_file.properties().duration().as_secs() as u32;

            if !all_genres.is_empty() {
                genre = all_genres.join(";");
            }
        }

        // Zoek de track in de library en werk bij
        for artist in &mut library.artists {
            for album in &mut artist.albums {
                for disk in &mut album.disks {
                    for track in &mut disk.tracks {
                        if track.path == *path {
                            if let Some(ref t) = title {
                                track.title = t.clone();
                            }
                            if let Some(ref a) = track_artist {
                                track.artist = Some(a.clone());
                            }
                            if let Some(ref a) = album_artist {
                                track.album_artist = Some(a.clone());
                            }
                            if let Some(n) = track_number {
                                track.track_number = n;
                            }
                            if let Some(n) = disc_number {
                                track.disc_number = n;
                            }
                            track.duration_secs = duration_secs;
                            if !genre.is_empty() {
                                track.genre = Some(genre.clone());
                            }
                            if let Some(y) = year {
                                track.year = Some(y);
                            }
                            if let Some(ref c) = composer {
                                track.composer = Some(c.clone());
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_drive_letter() {
        assert_eq!(
            Path::new(&remap_one_path(
                "H:/music/Artist/Album/song.flac",
                "H:/music",
                "L:/music"
            )),
            Path::new("L:/music/Artist/Album/song.flac")
        );
    }

    #[test]
    fn remap_nested_root() {
        assert_eq!(
            Path::new(&remap_one_path(
                "H:/muziek/collectie/Artist/song.flac",
                "H:/muziek/collectie",
                "L:/muziek/collectie"
            )),
            Path::new("L:/muziek/collectie/Artist/song.flac")
        );
    }

    #[test]
    fn remap_leaves_foreign_paths_alone() {
        assert_eq!(
            remap_one_path("C:/other/song.flac", "H:/music", "L:/music"),
            "C:/other/song.flac"
        );
    }

    #[test]
    fn remap_no_partial_prefix_match() {
        // "H:/music2" moet niet herschreven worden naar "L:/music2/..."
        assert_eq!(
            remap_one_path("H:/music2/song.flac", "H:/music", "L:/music"),
            "H:/music2/song.flac"
        );
    }

    #[test]
    fn dirs_match_ignores_case_and_trailing_slash() {
        assert!(dirs_match("H:/music", "h:/music/"));
        assert!(dirs_match("L:/music/", "L:/music"));
        assert!(!dirs_match("H:/music", "L:/music"));
    }

    #[cfg(windows)]
    #[test]
    fn remap_windows_backslash_paths() {
        assert_eq!(
            remap_one_path(
                "H:\\music\\Artist\\Album\\song.flac",
                "H:\\music",
                "L:\\music"
            ),
            "L:\\music\\Artist\\Album\\song.flac"
        );
        // Schijflettercase wordt genegeerd
        assert_eq!(
            remap_one_path("h:\\music\\Artist\\song.flac", "H:\\music", "L:\\music"),
            "L:\\music\\Artist\\song.flac"
        );
    }
}
