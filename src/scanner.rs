use crate::models::{Album, Artist, Disk, Library, Track};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use walkdir::WalkDir;

pub enum ScannerMessage {
    Progress(String), // Voor visuele feedback tijdens de eerste lange scan
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
    // 1. Probeer de cache in te laden voor een bliksemsnelle start
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

    // 2. Als er geen cache is (of deze corrupt is), start de volledige scan op de achtergrond
    let _ = tx.send(ScannerMessage::Progress(
        "Eerste indexering gestart (dit kan even duren bij 2TB)...".into(),
    ));

    // We gebruiken HashMaps tijdelijk om de hiërarchie op te bouwen
    let mut artists_map: HashMap<String, HashMap<String, HashMap<String, Vec<Track>>>> =
        HashMap::new();
    let mut album_covers: HashMap<String, String> = HashMap::new();

    // Loop door alle bestanden in de map
    for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
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
        let parent_dir = path.parent().unwrap_or(Path::new(""));

        // Check of het een albumhoes is
        if cover_exts.contains(&ext) {
            // Kijk of de bestandsnaam (bijv. "AHardDaysNight_front") een van onze cover_names ("front", "cover") BEVAT
            let is_cover = cover_names.iter().any(|name| file_name.contains(name));

            if is_cover {
                let dir_str = parent_dir.to_string_lossy().to_string();
                album_covers.insert(dir_str, path.to_string_lossy().to_string());
                continue;
            }
        }

        // Check of het een audiobestand is
        // Check of het een audiobestand is
        if audio_exts.contains(&ext) {
            // Bereken het pad relatief ten opzichte van de hoofdmap (bijv. H:\MUSIC)
            let base_dir = Path::new(&dir);
            if let Ok(rel_path) = path.strip_prefix(base_dir) {
                let components: Vec<String> = rel_path
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();

                let mut artist_name = "Onbekende Artiest".to_string();
                let mut album_name = "Onbekend Album".to_string();
                let mut disk_name = "Default".to_string();

                if components.len() == 1 {
                    // Bestand staat direct in H:\MUSIC\
                } else if components.len() == 2 {
                    // H:\MUSIC\Artiest\track.flac
                    artist_name = components[0].clone();
                } else {
                    // H:\MUSIC\Artiest\...\track.flac
                    artist_name = components[0].clone();

                    // Alle mappen tussen de Artiest en het Audiobestand
                    let folder_chain = &components[1..components.len() - 1];

                    if let Some(last_folder) = folder_chain.last() {
                        let is_cd = last_folder.to_lowercase().starts_with("cd")
                            || last_folder.to_lowercase().starts_with("disc");

                        if is_cd && folder_chain.len() > 1 {
                            disk_name = last_folder.clone();
                            // Voeg alle tussenliggende boxset-mappen samen tot 1 album titel (bijv: "Mozart Requiem Box - 01 Sussmayr Edition")
                            album_name = folder_chain[..folder_chain.len() - 1].join(" - ");
                        } else {
                            // Geen CD map, alles is onderdeel van de albumnaam
                            album_name = folder_chain.join(" - ");
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
                };

                artists_map
                    .entry(artist_name)
                    .or_default()
                    .entry(album_name)
                    .or_default()
                    .entry(disk_name)
                    .or_default()
                    .push(track);
            }
        }
    }

    // 3. Converteer de HashMaps naar onze uiteindelijke, efficiënte `Library` struct
    let mut library = Library::default();

    for (artist_name, albums_map) in artists_map {
        let mut artist = Artist {
            name: artist_name,
            albums: Vec::new(),
        };

        for (album_name, disks_map) in albums_map {
            let mut album = Album {
                title: album_name.clone(),
                cover_path: None,
                disks: Vec::new(),
            };

            for (disk_name, tracks) in disks_map {
                let mut sorted_tracks = tracks;
                sorted_tracks.sort_by(|a, b| natord::compare(&a.title, &b.title));
                album.disks.push(Disk {
                    name: disk_name,
                    tracks: sorted_tracks,
                });
            }

            // KOPPEL DE ALBUMHOES: Zoek de cover op basis van de map van de eerste track
            if let Some(first_disk) = album.disks.first() {
                if let Some(first_track) = first_disk.tracks.first() {
                    let track_path = Path::new(&first_track.path);

                    // Check de map van de track zelf
                    if let Some(parent) = track_path.parent() {
                        let parent_str = parent.to_string_lossy().to_string();
                        album.cover_path = album_covers.get(&parent_str).cloned();

                        // Als er geen cover is, en we zitten in een "CD 1" map, check dan de hoofdmap erboven!
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
        // Sorteer albums alfabetisch
        artist.albums.sort_by(|a, b| a.title.cmp(&b.title));
        library.artists.push(artist);
    }
    // Sorteer artiesten alfabetisch
    library.artists.sort_by(|a, b| a.name.cmp(&b.name));

    // 4. Sla de gecompileerde bibliotheek op naar de binaire cache
    let _ = tx.send(ScannerMessage::Progress("Bibliotheek opslaan...".into()));
    if let Ok(file) = File::create(CACHE_FILE) {
        let writer = BufWriter::new(file);
        let _ = bincode::serialize_into(writer, &library);
    }

    // 5. Stuur het eindresultaat naar de UI
    let _ = tx.send(ScannerMessage::LibraryLoaded(library));
    let _ = tx.send(ScannerMessage::ScanComplete);
}
