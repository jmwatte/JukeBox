use crate::config::Config;
use crate::models::{Album, Library};
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::{collect_genres, filter_by_genre, filter_library};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, Image, Key, RichText, ScrollArea};
//use lofty::file::TaggedFile;
//use lofty::file::{AudioFile, TaggedFileExt}; // <--- WriteOptions toegevoegd
//use lofty::probe::Probe;
//use lofty::tag::{Accessor, Tag};
use std::collections::HashSet;
use std::path::Path;

#[derive(PartialEq, Clone)]
pub enum NavLevel {
    Artist,
    Album,
    Disk,
    Track,
}

#[derive(PartialEq, Clone)]
pub enum ViewMode {
    Tracklist,
    AlbumCover,
}

#[derive(PartialEq, Clone)]
pub enum BrowseMode {
    Library,
    Genre,
    Recent,
}

pub struct MusicPlayerApp {
    config: Config,
    player_tx: Sender<PlayerCommand>,
    player_event_rx: Receiver<PlayerEvent>, // Kanaal om 'Now Playing' te ontvangen
    scanner_tx: Sender<ScannerMessage>,
    scanner_rx: Receiver<ScannerMessage>,
    library: Option<Library>,

    // NIEUWE STATUS VARIABELEN
    now_playing: Option<String>,
    show_help: bool,

    _status_message: String,

    current_level: NavLevel,
    view_mode: ViewMode,
    selected_artist: usize,
    selected_album: usize,
    selected_disk: usize,
    selected_track: usize,
    scroll_to_selection: bool,
    search_query: String,
    //search_results: Vec<SearchResult>,
    // selected_search_index: usize,
    is_search_active: bool,
    search_input_id: egui::Id,
    filtered_library: Option<Library>,

    // Genre browsing
    browse_mode: BrowseMode,
    genres: Vec<(String, usize)>,
    selected_genre: usize,
    genre_filtered_library: Option<Library>,
    selected_genre_name: String,
    sort_by_date: bool,
    // NEW: Recent Albums
    recent_albums: Vec<(String, Album)>, // (Artist Name, Album)
    selected_recent: usize,

    // Track Details Popup
    show_track_details: bool,
    editing_track_path: Option<String>,
    edit_title: String,
    edit_artist: String,
    edit_album: String,
    edit_genre: String,
    save_status: Option<String>, // Om "Saved!" of errors te tonen
    raw_tags_display: String,    // NIEUW: Om alle tags als tekst te tonen
    read_error: Option<String>,  // NIEUW: Om de lofty error te tonen
    update_title: bool,
    update_artist: bool,
    update_album: bool,
    update_genre: bool,
    selected_tracks: HashSet<String>,
    tracks_to_edit: Vec<String>,
}

impl MusicPlayerApp {
    pub fn new(
        config: Config,
        player_tx: Sender<PlayerCommand>,
        player_event_rx: Receiver<PlayerEvent>,
        scanner_tx: Sender<ScannerMessage>,
        scanner_rx: Receiver<ScannerMessage>,
    ) -> Self {
        let view_mode = if config.startup_view == "cover" {
            ViewMode::AlbumCover
        } else {
            ViewMode::Tracklist
        };
        Self {
            config,
            player_tx,
            player_event_rx,
            scanner_tx,
            scanner_rx,
            library: None,
            now_playing: None,
            show_help: false,
            _status_message: "Bibliotheek opstarten...".to_string(),
            filtered_library: None,
            // _search_active: false,
            search_query: String::new(),
            current_level: NavLevel::Artist,
            view_mode,
            selected_artist: 0,
            selected_album: 0,
            selected_disk: 0,
            selected_track: 0,
            scroll_to_selection: true,
            // search_query: String::new(),
            // search_results: Vec::new(),
            // selected_search_index: 0,
            is_search_active: false,
            search_input_id: egui::Id::new("global_search_input"),
            browse_mode: BrowseMode::Library,
            genres: Vec::new(),
            selected_genre: 0,
            genre_filtered_library: None,
            selected_genre_name: String::new(),
            recent_albums: Vec::new(),
            selected_recent: 0,
            sort_by_date: false,
            show_track_details: false,
            editing_track_path: None,
            edit_title: String::new(),
            edit_artist: String::new(),
            edit_album: String::new(),
            edit_genre: String::new(),
            save_status: None,
            raw_tags_display: String::new(),
            read_error: None,
            update_title: false,
            update_artist: false,
            update_album: false,
            update_genre: false,
            selected_tracks: HashSet::new(),
            tracks_to_edit: Vec::new(),
        }
    }

    /// Returns the currently active library (search-filtered, genre-filtered, or full)
    // fn active_library(&self) -> Option<&Library> {
    //     self.filtered_library
    //         .as_ref()
    //         .or(self.genre_filtered_library.as_ref())
    //         .or(self.library.as_ref())
    // }
    fn toggle_sort(&mut self) {
        self.sort_by_date = !self.sort_by_date;

        // This closure sorts artists by their newest album, and albums by date
        let sort_fn = |lib: &mut Library| {
            if self.sort_by_date {
                // Sort Artists by their newest album timestamp (Descending)
                lib.artists.sort_by(|a, b| {
                    let a_max = a
                        .albums
                        .iter()
                        .map(|al| al.added_timestamp)
                        .max()
                        .unwrap_or(0);
                    let b_max = b
                        .albums
                        .iter()
                        .map(|al| al.added_timestamp)
                        .max()
                        .unwrap_or(0);
                    b_max.cmp(&a_max)
                });
                // Sort Albums by timestamp (Descending)
                for artist in &mut lib.artists {
                    artist
                        .albums
                        .sort_by(|a, b| b.added_timestamp.cmp(&a.added_timestamp));
                }
            } else {
                // Revert to Alphabetical
                lib.artists.sort_by(|a, b| a.name.cmp(&b.name));
                for artist in &mut lib.artists {
                    artist.albums.sort_by(|a, b| a.title.cmp(&b.title));
                }
            }
        };

        // Apply to all active libraries
        if let Some(lib) = &mut self.library {
            sort_fn(lib);
        }
        if let Some(lib) = &mut self.filtered_library {
            sort_fn(lib);
        }
        if let Some(lib) = &mut self.genre_filtered_library {
            sort_fn(lib);
        }

        // Reset selection to top so we don't go out of bounds
        self.selected_artist = 0;
        self.selected_album = 0;
        self.scroll_to_selection = true;
    }
    fn enter_genre_mode(&mut self) {
        if let Some(lib) = &self.library {
            self.genres = collect_genres(lib);
            self.selected_genre = 0;
            self.browse_mode = BrowseMode::Genre;
            self.genre_filtered_library = None;
            self.selected_genre_name.clear();
            self.current_level = NavLevel::Artist;
            self.scroll_to_selection = true;
        }
    }

    fn enter_recent_mode(&mut self) {
        if let Some(lib) = &self.library {
            let mut flat_albums = Vec::new();
            for artist in &lib.artists {
                for album in &artist.albums {
                    flat_albums.push((artist.name.clone(), album.clone()));
                }
            }
            // Sort descending by timestamp (newest first)
            flat_albums.sort_by(|a, b| b.1.added_timestamp.cmp(&a.1.added_timestamp));

            // Limit to top 500 to keep the UI blazing fast
            flat_albums.truncate(500);

            self.recent_albums = flat_albums;
            self.selected_recent = 0;
            self.browse_mode = BrowseMode::Recent;
            self.scroll_to_selection = true;
        }
    }

    // Renamed from exit_genre_mode
    fn exit_browse_mode(&mut self) {
        self.browse_mode = BrowseMode::Library;
        self.genre_filtered_library = None;
        self.selected_genre_name.clear();
        self.recent_albums.clear();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.selected_album = 0;
        self.selected_disk = 0;
        self.selected_track = 0;
        self.scroll_to_selection = true;
    }

    fn select_genre(&mut self, genre: &str) {
        if let Some(lib) = &self.library {
            self.selected_genre_name = genre.to_string();
            self.genre_filtered_library = Some(filter_by_genre(lib, genre));
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.selected_album = 0;
            self.selected_disk = 0;
            self.selected_track = 0;
            self.scroll_to_selection = true;
        }
    }

    fn play_selected_item(&self, lib: &Library, replace: bool) {
        let mut queue = Vec::new();
        match self.current_level {
            NavLevel::Track => {
                let track = &lib.artists[self.selected_artist].albums[self.selected_album].disks
                    [self.selected_disk]
                    .tracks[self.selected_track];
                queue.push(track.path.clone());
            }
            NavLevel::Disk | NavLevel::Album | NavLevel::Artist => {
                let artist = &lib.artists[self.selected_artist];
                match self.current_level {
                    NavLevel::Artist => {
                        for album in &artist.albums {
                            for disk in &album.disks {
                                for track in &disk.tracks {
                                    queue.push(track.path.clone());
                                }
                            }
                        }
                    }
                    NavLevel::Album => {
                        for disk in &artist.albums[self.selected_album].disks {
                            for track in &disk.tracks {
                                queue.push(track.path.clone());
                            }
                        }
                    }
                    NavLevel::Disk => {
                        for track in
                            &artist.albums[self.selected_album].disks[self.selected_disk].tracks
                        {
                            queue.push(track.path.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        if !queue.is_empty() {
            if replace {
                let _ = self.player_tx.send(PlayerCommand::ReplaceQueue(queue));
            } else {
                let _ = self.player_tx.send(PlayerCommand::AppendToQueue(queue));
            }
        }
    }
    // Helper om veilig het pad van de huidige track op te halen
    fn get_current_track_path(&self, lib: &Library) -> Option<String> {
        lib.artists
            .get(self.selected_artist)
            .and_then(|a| a.albums.get(self.selected_album))
            .and_then(|al| al.disks.get(self.selected_disk))
            .and_then(|d| d.tracks.get(self.selected_track))
            .map(|t| t.path.clone())
    }

    // De zware functie: Tags wegschrijven met lofty
    fn save_track_tags(&mut self) {
        use lofty::config::WriteOptions;
        use lofty::file::{AudioFile, TaggedFileExt};
        use lofty::probe::Probe;
        use lofty::tag::{Accessor, Tag, TagType};
        use std::path::Path;

        if self.tracks_to_edit.is_empty() {
            return;
        }

        let mut success_count = 0;
        let mut error_count = 0;

        // FIX: We loopen nu direct over de paden (Strings), geen indices meer!
        for path in &self.tracks_to_edit {
            let result = (|| -> Result<(), String> {
                let mut tagged_file = Probe::open(path)
                    .map_err(|e| format!("Open: {:?}", e))?
                    .read()
                    .map_err(|e| format!("Read: {:?}", e))?;

                // Bepaal het correcte tag-type op basis van de bestandsextensie
                let ext = Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let target_tag_type = match ext.as_str() {
                    "mp3" => TagType::Id3v2,
                    "flac" | "ogg" | "opus" => TagType::VorbisComments,
                    "m4a" | "mp4" => TagType::Mp4Ilst,
                    "wav" => TagType::RiffInfo,
                    _ => TagType::Id3v2,
                };

                // Zoek de bestaande tag van dit type
                let mut tag = Tag::new(target_tag_type);
                for existing_tag in tagged_file.tags() {
                    if existing_tag.tag_type() == target_tag_type {
                        tag = existing_tag.clone();
                        break;
                    }
                }

                // Update alleen de aangevinkte velden
                if self.update_title {
                    tag.set_title(self.edit_title.clone());
                }
                if self.update_artist {
                    tag.set_artist(self.edit_artist.clone());
                }
                if self.update_album {
                    tag.set_album(self.edit_album.clone());
                }
                if self.update_genre {
                    tag.set_genre(self.edit_genre.clone());
                }

                tagged_file.insert_tag(tag);
                tagged_file
                    .save_to_path(path, WriteOptions::default())
                    .map_err(|e| format!("Save: {:?}", e))?;

                Ok(())
            })();

            match result {
                Ok(_) => {
                    success_count += 1;

                    // Update de in-memory library door te zoeken op het pad
                    if let Some(lib) = &mut self.library {
                        for artist in &mut lib.artists {
                            for album in &mut artist.albums {
                                for disk in &mut album.disks {
                                    for track in &mut disk.tracks {
                                        if track.path == *path {
                                            if self.update_title {
                                                track.title = self.edit_title.clone();
                                            }
                                            if self.update_genre {
                                                track.genre = Some(self.edit_genre.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    println!("LOFTY SAVE ERROR voor {:?}: {}", path, e);
                }
            }
        }

        // Ververs raw_tags_display alleen bij single track edit
        if success_count > 0 && self.tracks_to_edit.len() == 1 {
            if let Some(path) = &self.editing_track_path {
                self.raw_tags_display.clear();
                if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
                    let mut raw_text = String::new();
                    for tag in tagged_file.tags() {
                        raw_text.push_str(&format!("--- Tag Type: {:?} ---\n", tag.tag_type()));
                        for item in tag.items() {
                            raw_text.push_str(&format!("{:?}: {:?}\n", item.key(), item.value()));
                        }
                    }
                    self.raw_tags_display = if raw_text.is_empty() {
                        "Geen tags gevonden.".to_string()
                    } else {
                        raw_text
                    };
                }
            }
        }

        if error_count == 0 {
            self.save_status = Some(format!(
                "Succesvol opgeslagen in {} bestand(en)!",
                success_count
            ));
        } else {
            self.save_status = Some(format!(
                "{} opgeslagen, {} faalden.",
                success_count, error_count
            ));
        }

        self.selected_tracks.clear();
    }

    fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.selected_tracks.clear();
            if self.is_search_active || self.filtered_library.is_some() {
                self.is_search_active = false;
                self.filtered_library = None;
                self.search_query.clear();
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
                return;
            }
            if self.browse_mode != BrowseMode::Library {
                self.exit_browse_mode();
                return;
            }
        }
        // --- F6 TOETS: HERSTEL AUDIO VERBINDING ---
        if ctx.input(|i| i.key_pressed(Key::F6)) {
            let _ = self.player_tx.send(PlayerCommand::ReconnectAudio);
            self.now_playing = Some("Audio verbinding herstellen...".to_string());
        }

        // --- HELP SCHERM (?) of (H) ---
        if ctx.input(|i| {
            i.key_pressed(Key::H)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "?"))
        }) {
            self.show_help = !self.show_help;
        }

        // --- F5 RESCAN ---
        if ctx.input(|i| i.key_pressed(Key::F5)) {
            let _ = std::fs::remove_file("library_cache.bin");
            self.library = None;
            self.filtered_library = None;
            self.genre_filtered_library = None;
            self.browse_mode = BrowseMode::Library;
            let tx = self.scanner_tx.clone();
            let config = self.config.clone();
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(async {
                        crate::scanner::load_or_scan_library(
                            config.music_directory,
                            config.audio_extensions,
                            config.cover_names,
                            config.cover_extensions,
                            tx,
                        )
                        .await;
                    });
                }
            });
            return;
        }

        // --- G-TOETS GENRE BROWSING ---
        if ctx.input(|i| i.key_pressed(Key::G)) {
            if self.browse_mode == BrowseMode::Library {
                self.enter_genre_mode();
            } else {
                self.exit_browse_mode();
            }
            return;
        }

        // --- S-TOETS SORTING ---
        if ctx.input(|i| i.key_pressed(Key::S)) {
            self.toggle_sort();
            return;
        }

        // --- B-TOETS RECENT ALBUMS ---
        if ctx.input(|i| i.key_pressed(Key::B)) {
            if self.browse_mode == BrowseMode::Library {
                self.enter_recent_mode();
            } else {
                self.exit_browse_mode();
            }
            return;
        }

        // NEW: Recent albums navigation
        if self.browse_mode == BrowseMode::Recent {
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                if self.selected_recent + 1 < self.recent_albums.len() {
                    self.selected_recent += 1;
                    self.scroll_to_selection = true;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                if self.selected_recent > 0 {
                    self.selected_recent -= 1;
                    self.scroll_to_selection = true;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Enter)) {
                if let Some((_, album)) = self.recent_albums.get(self.selected_recent) {
                    let mut queue = Vec::new();
                    for disk in &album.disks {
                        for track in &disk.tracks {
                            queue.push(track.path.clone());
                        }
                    }
                    let _ = self.player_tx.send(PlayerCommand::ReplaceQueue(queue));
                }
            }
            return; // Stop processing normal library navigation
        }

        // Genre picker navigation
        if self.browse_mode == BrowseMode::Genre && self.genre_filtered_library.is_none() {
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                if self.selected_genre + 1 < self.genres.len() {
                    self.selected_genre += 1;
                    self.scroll_to_selection = true;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                if self.selected_genre > 0 {
                    self.selected_genre -= 1;
                    self.scroll_to_selection = true;
                }
            }
            return;
        }
        // Disjoint borrowing: only borrows the specific fields, allowing us to mutate self later
        let lib = self
            .filtered_library
            .as_ref()
            .or(self.genre_filtered_library.as_ref())
            .or(self.library.as_ref());

        //let lib = self.active_library();
        let Some(lib) = lib else {
            return;
        };

        // --- R-TOETS RANDOM ALBUM ---
        if ctx.input(|i| i.key_pressed(Key::R)) {
            if !lib.artists.is_empty() {
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as usize;
                let random_artist = time % lib.artists.len();
                if !lib.artists[random_artist].albums.is_empty() {
                    let random_album = (time / 100) % lib.artists[random_artist].albums.len();
                    self.selected_artist = random_artist;
                    self.selected_album = random_album;
                    self.current_level = NavLevel::Album;
                    self.scroll_to_selection = true;
                }
            }
        }

        if ctx.input(|i| i.key_pressed(Key::T)) {
            self.view_mode = match self.view_mode {
                ViewMode::Tracklist => ViewMode::AlbumCover,
                ViewMode::AlbumCover => ViewMode::Tracklist,
            };
        }

        // --- AFSPEEL BESTURING ---
        if ctx.input(|i| i.key_pressed(Key::Space)) {
            let _ = self.player_tx.send(PlayerCommand::PlayPause);
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            self.play_selected_item(lib, true);
        }
        if ctx.input(|i| i.key_pressed(Key::A)) {
            self.play_selected_item(lib, false);
        }
        if ctx.input(|i| i.key_pressed(Key::N)) {
            let _ = self.player_tx.send(PlayerCommand::Skip);
        }

        // --- O-TOETS: OPEN FOLDER ---
        if ctx.input(|i| i.key_pressed(Key::O)) {
            if let Some(track_path) = self.get_current_track_path(lib) {
                if let Some(parent) = Path::new(&track_path).parent() {
                    // Opent de verkenner in de map van het bestand
                    let _ = std::process::Command::new("explorer").arg(parent).spawn();
                }
            }
        }

        // --- I-TOETS: TRACK DETAILS & EDITING (SINGLE OR BATCH) ---
        // --- I-TOETS: TRACK DETAILS & EDITING (SINGLE OR BATCH) ---
        if ctx.input(|i| i.key_pressed(Key::I)) {
            if self.current_level == NavLevel::Track {
                self.save_status = None;
                self.raw_tags_display.clear();
                self.read_error = None;
                self.edit_title.clear();
                self.edit_artist.clear();
                self.edit_album.clear();
                self.edit_genre.clear();
                self.update_title = false;
                self.update_artist = false;
                self.update_album = false;
                self.update_genre = false;

                use lofty::file::TaggedFileExt;
                use lofty::probe::Probe;
                use lofty::tag::Accessor;

                // FIX: Gebruik de actieve bibliotheek (Genre, Search, of Library)
                let active_lib = self
                    .genre_filtered_library
                    .as_ref()
                    .or(self.filtered_library.as_ref())
                    .or(self.library.as_ref());

                if let Some(lib) = active_lib {
                    // Bepaal welke paden we gaan editten
                    if self.selected_tracks.is_empty() {
                        // Geen selectie, gebruik de huidige track
                        if let Some(track_path) = self.get_current_track_path(lib) {
                            self.tracks_to_edit = vec![track_path];
                        }
                    } else {
                        // Kopieer de geselecteerde paden direct
                        self.tracks_to_edit = self.selected_tracks.iter().cloned().collect();
                        self.tracks_to_edit.sort();
                    }

                    // Laad de tags van het EERSTE pad om de velden in te vullen
                    if let Some(first_path) = self.tracks_to_edit.first() {
                        self.editing_track_path = Some(first_path.clone());

                        match Probe::open(first_path).and_then(|p| p.read()) {
                            Ok(tagged_file) => {
                                let mut raw_text = String::new();
                                for tag in tagged_file.tags() {
                                    raw_text.push_str(&format!(
                                        "--- Tag Type: {:?} ---\n",
                                        tag.tag_type()
                                    ));
                                    for item in tag.items() {
                                        raw_text.push_str(&format!(
                                            "{:?}: {:?}\n",
                                            item.key(),
                                            item.value()
                                        ));
                                    }
                                }
                                self.raw_tags_display = if raw_text.is_empty() {
                                    "Geen tags gevonden.".to_string()
                                } else {
                                    raw_text
                                };

                                if let Some(t) = tagged_file
                                    .primary_tag()
                                    .or_else(|| tagged_file.first_tag())
                                {
                                    self.edit_title =
                                        t.title().map(|s| s.to_string()).unwrap_or_default();
                                    self.edit_artist =
                                        t.artist().map(|s| s.to_string()).unwrap_or_default();
                                    self.edit_album =
                                        t.album().map(|s| s.to_string()).unwrap_or_default();
                                    self.edit_genre =
                                        t.genre().map(|s| s.to_string()).unwrap_or_default();
                                }
                            }
                            Err(e) => {
                                self.read_error = Some(format!("{:?}", e));
                                self.raw_tags_display =
                                    "Fout bij het parsen van de audio-container.".to_string();
                            }
                        }
                    }
                }
                self.show_track_details = true;
            }
        }

        // --- M-TOETS: MARK TRACK FOR BATCH EDITING ---
        if ctx.input(|i| i.key_pressed(Key::M)) {
            if self.current_level == NavLevel::Track {
                // Haal het unieke pad op van de huidige track
                if let Some(track_path) = self.get_current_track_path(lib) {
                    if self.selected_tracks.contains(&track_path) {
                        self.selected_tracks.remove(&track_path);
                    } else {
                        self.selected_tracks.insert(track_path);
                    }
                }
            }
        }
        // --- SHIFT + M: WIS ALLE MARKERINGEN ---
        if ctx.input(|i| i.key_pressed(Key::M) && i.modifiers.shift) {
            self.selected_tracks.clear();
            self.tracks_to_edit.clear();
        }

        // --- NAVIGATIE PIJLTJES ---
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            match self.current_level {
                NavLevel::Artist => {
                    if self.selected_artist + 1 < lib.artists.len() {
                        self.selected_artist += 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    let albums = &lib.artists[self.selected_artist].albums;
                    if self.selected_album + 1 < albums.len() {
                        self.selected_album += 1;
                        self.scroll_to_selection = true;
                    } else if self.selected_artist + 1 < lib.artists.len() {
                        self.selected_artist += 1;
                        self.selected_album = 0;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if self.selected_disk + 1 < disks.len() {
                        self.selected_disk += 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Track => {
                    let tracks = &lib.artists[self.selected_artist].albums[self.selected_album]
                        .disks[self.selected_disk]
                        .tracks;
                    if self.selected_track + 1 < tracks.len() {
                        self.selected_track += 1;
                        self.scroll_to_selection = true;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            match self.current_level {
                NavLevel::Artist => {
                    if self.selected_artist > 0 {
                        self.selected_artist -= 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    if self.selected_album > 0 {
                        self.selected_album -= 1;
                        self.scroll_to_selection = true;
                    } else if self.selected_artist > 0 {
                        self.selected_artist -= 1;
                        self.selected_album = lib.artists[self.selected_artist]
                            .albums
                            .len()
                            .saturating_sub(1);
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    if self.selected_disk > 0 {
                        self.selected_disk -= 1;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Track => {
                    if self.selected_track > 0 {
                        self.selected_track -= 1;
                        self.scroll_to_selection = true;
                    }
                }
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            match self.current_level {
                NavLevel::Artist => {
                    if !lib.artists[self.selected_artist].albums.is_empty() {
                        self.current_level = NavLevel::Album;
                        self.selected_album = 0;
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Album => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if !disks.is_empty() {
                        if disks.len() == 1 && disks[0].name == "Default" {
                            self.current_level = NavLevel::Track;
                            self.selected_disk = 0;
                            self.selected_track = 0;
                        } else {
                            self.current_level = NavLevel::Disk;
                            self.selected_disk = 0;
                        }
                        self.scroll_to_selection = true;
                    }
                }
                NavLevel::Disk => {
                    if !lib.artists[self.selected_artist].albums[self.selected_album].disks
                        [self.selected_disk]
                        .tracks
                        .is_empty()
                    {
                        self.current_level = NavLevel::Track;
                        self.selected_track = 0;
                        self.scroll_to_selection = true;
                    }
                }
                _ => {}
            }
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            match self.current_level {
                NavLevel::Album => {
                    self.current_level = NavLevel::Artist;
                    self.scroll_to_selection = true;
                }
                NavLevel::Disk => {
                    self.current_level = NavLevel::Album;
                    self.scroll_to_selection = true;
                }
                NavLevel::Track => {
                    let disks =
                        &lib.artists[self.selected_artist].albums[self.selected_album].disks;
                    if disks.len() == 1 && disks[0].name == "Default" {
                        self.current_level = NavLevel::Album;
                    } else {
                        self.current_level = NavLevel::Disk;
                    }
                    self.scroll_to_selection = true;
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for MusicPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update Library Status
        while let Ok(msg) = self.scanner_rx.try_recv() {
            if let ScannerMessage::LibraryLoaded(lib) = msg {
                self.library = Some(lib);
                // Als we aan het zoeken waren, herbereken de filter op de nieuwe library
                if !self.search_query.is_empty() {
                    self.filtered_library = Some(filter_library(
                        self.library.as_ref().unwrap(),
                        &self.search_query,
                    ));
                }
            }
        }

        // Update Now Playing Status
        while let Ok(event) = self.player_event_rx.try_recv() {
            match event {
                PlayerEvent::NowPlaying(path) => {
                    if let Some(file_name) = Path::new(&path).file_name() {
                        self.now_playing = Some(file_name.to_string_lossy().into_owned());
                    }
                } // PlayerEvent::Stopped => self.now_playing = None,
            }
        }
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.is_search_active = true;
            self.search_query.clear();
            self.filtered_library = None; // Reset bij nieuw zoeken
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
        }
        self.handle_keyboard_navigation(ctx);

        // --- HELP SCHERM TEKENEN ---
        if self.show_help {
            egui::Window::new("Sneltoetsen & Help")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Toetsenbord Navigatie").strong());
                    ui.label("• Pijltjestoetsen : Navigeer door de bibliotheek");
                    ui.label("• T : Wissel weergave (Lijst / Covers)");
                    ui.add_space(5.0);
                    ui.label(RichText::new("Muziek Besturing").strong());
                    ui.label("• Enter : Speel selectie af (wist wachtrij)");
                    ui.label("• Spatie : Pauzeer / Hervat");
                    ui.label("• A : Voeg selectie toe achteraan de wachtrij");
                    ui.label("• N : Skip naar het volgende nummer");
                    ui.add_space(5.0);
                    ui.label(RichText::new("Extra").strong());
                    ui.label("• R : Selecteer een willekeurig album");
                    ui.label("• G : Bladeren per genre");
                    ui.label("• B : Toon nieuwste albums (Recent) ");
                    ui.label("• S : Sorteer op datum (Descending)");
                    ui.label("• O : Open de map van de huidige track ");
                    ui.label("• I : Track Details & Tags bewerken ");
                    ui.label("• F5 : Forceer een rescan van de bibliotheek");
                    ui.label("• F6 : Herstel audio verbinding");
                    ui.label("• ? of H : Toon / verberg dit helpvenster");
                    ui.separator();
                    if ui.button("Sluiten").clicked() {
                        self.show_help = false;
                    }
                });
        }

        // Check of de initiële scan klaar is
        // We vangen hier ook de Progress messages op om de gebruiker te laten zien dat we bezig zijn
        while let Ok(msg) = self.scanner_rx.try_recv() {
            match msg {
                ScannerMessage::LibraryLoaded(lib) => {
                    self.library = Some(lib);
                    if !self.search_query.is_empty() {
                        self.filtered_library = Some(crate::search::filter_library(
                            self.library.as_ref().unwrap(),
                            &self.search_query,
                        ));
                    }
                }
                ScannerMessage::Progress(text) => {
                    self._status_message = text; // We gebruiken je bestaande _status_message variabele
                }
                ScannerMessage::ScanComplete => {
                    self._status_message = "Klaar!".to_string();
                }
            }
        }

        if self.library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Bibliotheek indexeren...").size(24.0));
                        ui.add_space(10.0);
                        // Toon de live status (bijv. "Eerste indexering gestart...")
                        ui.label(RichText::new(&self._status_message).color(Color32::GRAY));
                    });
                });
            });
            ctx.request_repaint(); // Blijf verversen totdat de library geladen is
            return;
        }

        // --- NOW PLAYING BALK ONDERAAN ---
        if let Some(track) = &self.now_playing {
            egui::TopBottomPanel::bottom("now_playing_panel").show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🎵 Nu aan het spelen:")
                            .color(Color32::from_rgb(100, 200, 100))
                            .strong(),
                    );
                    ui.label(track);
                });
                ui.add_space(8.0);
            });
        }

        // --- ZOEKBALK RENDEREN ---
        if self.is_search_active {
            egui::Window::new("Zoeken in Bibliotheek")
                .collapsible(false)
                .resizable(false)
                .default_width(600.0)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 50.0])
                .show(ctx, |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Typ om te zoeken... (Esc om te sluiten)")
                            .id(self.search_input_id)
                            .desired_width(f32::INFINITY),
                    );

                    // --- DE CURSOR FIX ---
                    // Forceer focus direct nadat het veld is getekend als de zoekbalk actief is
                    if self.is_search_active && !response.has_focus() {
                        ctx.memory_mut(|m| m.request_focus(self.search_input_id));
                    }

                    if response.changed() {
                        self.current_level = NavLevel::Artist;
                        self.selected_artist = 0;
                        self.selected_album = 0;
                        self.selected_disk = 0;
                        self.selected_track = 0;

                        // Bepaal de basis voor de zoekopdracht:
                        // Gebruik de genre-gefilterde library als die er is, anders de volledige library.
                        let base_lib = self
                            .genre_filtered_library
                            .as_ref()
                            .or(self.library.as_ref());

                        if let Some(base_lib) = base_lib {
                            if self.search_query.trim().is_empty() {
                                self.filtered_library = None;
                            } else {
                                // Zoek nu binnen de juiste subset!
                                self.filtered_library =
                                    Some(filter_library(base_lib, &self.search_query));
                            }
                        }
                    }

                    if response.has_focus() {
                        // ESC: Annuleer, wis filter, sluit venster
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.is_search_active = false;
                            self.filtered_library = None; // Reset de filter!
                            self.search_query.clear();
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }

                        // ENTER: Sluit zoekvenster, maar BEHOUD de filter!
                        // De focus wordt vrijgegeven, waardoor je nu met de pijltjestoetsen
                        // door de gefilterde resultaten in het hoofdscherm kunt navigeren.
                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.is_search_active = false;
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }
                    }
                });
        }

        // --- HOOFDSCHERM ---
        // Disjoint borrowing for the UI rendering
        let current_lib = self
            .filtered_library
            .as_ref()
            .or(self.genre_filtered_library.as_ref())
            .or(self.library.as_ref());
        // let current_lib = self.active_library();
        let Some(current_lib) = current_lib else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Bibliotheek scannen...").size(24.0));
                });
            });
            ctx.request_repaint();
            return;
        };
        // --- CHECK: Lege zoekresultaten? Toon de afbeelding ---
        if current_lib.artists.is_empty() && self.filtered_library.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Geen resultaten gevonden")
                            .size(28.0)
                            .color(egui::Color32::YELLOW),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("voor: \"{}\"", self.search_query))
                            .size(16.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(30.0);

                    // Laad de afbeelding - kies EEN van de twee opties hieronder:

                    // OPTIE A: Embedded (aanbevolen - geen externe bestanden nodig)
                    let image_bytes = include_bytes!("../assets/no_results.png");
                    ui.add(
                        egui::Image::from_bytes("bytes://no_results.png", image_bytes.as_ref())
                            .max_width(600.0)
                            .max_height(600.0),
                    );

                    ui.add_space(30.0);
                    ui.label(
                        egui::RichText::new("Druk op Esc om terug te gaan")
                            .size(14.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
            ctx.request_repaint();
            return;
        }

        // --- GENRE PICKER UI ---
        if self.browse_mode == BrowseMode::Genre && self.genre_filtered_library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Center the heading and instructions at the top
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.heading("Genres");
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Druk op Esc of G om terug te gaan")
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(12.0);
                });

                ScrollArea::vertical().show(ui, |ui| {
                    // 1. Bepaal eerst welk genre er geselecteerd moet worden (zonder self te muteren)
                    let mut genre_to_select: Option<String> = None;

                    for (i, (genre, count)) in self.genres.iter().enumerate() {
                        // FIX: Wrap each item in a centered layout, exactly like the tracks!
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                                |ui| {
                                    let selected = i == self.selected_genre;
                                    let resp = ui.selectable_label(
                                        selected,
                                        RichText::new(format!("{} ({})", genre, count)).size(16.0),
                                    );

                                    if resp.clicked() {
                                        genre_to_select = Some(genre.clone());
                                    }

                                    if selected
                                        && ctx.input(|i| {
                                            i.key_pressed(Key::Enter)
                                                || i.key_pressed(Key::ArrowRight)
                                        })
                                    {
                                        genre_to_select = Some(genre.clone());
                                    }

                                    if selected && self.scroll_to_selection {
                                        resp.scroll_to_me(None);
                                    }
                                },
                            );
                        });
                    }

                    // 2. Voer nu pas de mutatie uit, nadat de lening van self.genres is vrijgegeven
                    if let Some(genre) = genre_to_select {
                        self.select_genre(&genre);
                    }
                });
            });

            self.scroll_to_selection = false;
            ctx.request_repaint();
            return;
        }

        // --- RECENT ALBUMS UI ---
        if self.browse_mode == BrowseMode::Recent {
            // Clone to avoid borrow checker issues when mutating self.selected_recent
            let recent = self.recent_albums.clone();

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.heading("Nieuwste Albums");
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Druk op Esc of B om terug te gaan")
                            .size(12.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(12.0);
                });

                ScrollArea::vertical().show(ui, |ui| {
                    for (i, (artist_name, album)) in recent.iter().enumerate() {
                        let selected = i == self.selected_recent;

                        ui.horizontal(|ui| {
                            // Draw a small cover
                            if let Some(path) = &album.cover_path {
                                ui.add(
                                    Image::new(format!("file://{}", path))
                                        .max_size(egui::vec2(40.0, 40.0)),
                                );
                            } else {
                                ui.add_space(40.0); // Keep alignment if no cover
                            }

                            let resp = ui.selectable_label(
                                selected,
                                RichText::new(format!("{} - {}", artist_name, album.title))
                                    .size(16.0),
                            );

                            if resp.clicked() {
                                self.selected_recent = i;
                                // Optional: Play on click
                                let mut queue = Vec::new();
                                for disk in &album.disks {
                                    for track in &disk.tracks {
                                        queue.push(track.path.clone());
                                    }
                                }
                                let _ = self.player_tx.send(PlayerCommand::ReplaceQueue(queue));
                            }

                            if selected && self.scroll_to_selection {
                                resp.scroll_to_me(None);
                            }
                        });
                    }
                });
            });
            self.scroll_to_selection = false;
            ctx.request_repaint();
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Visuele indicator dat we aan het zoeken zijn + HIT COUNTER
                if self.filtered_library.is_some() {
                    // Tel het totale aantal tracks in de gefilterde bibliotheek
                    let hit_count: usize = current_lib
                        .artists
                        .iter()
                        .map(|a| {
                            a.albums
                                .iter()
                                .map(|al| al.disks.iter().map(|d| d.tracks.len()).sum::<usize>())
                                .sum::<usize>()
                        })
                        .sum::<usize>();

                    ui.label(
                        egui::RichText::new(format!("🔍 {} resultaten voor: ", hit_count))
                            .color(egui::Color32::YELLOW),
                    );
                    ui.label(egui::RichText::new(&self.search_query).strong());
                    ui.label(
                        egui::RichText::new(" (Esc om te wissen) ")
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                } else {
                    ui.label(egui::RichText::new("Bibliotheek").color(egui::Color32::GRAY));
                    // Show genre context if browsing by genre
                    if self.browse_mode == BrowseMode::Genre && !self.selected_genre_name.is_empty()
                    {
                        ui.label(format!(" > Genre: {}", self.selected_genre_name));
                    }
                }

                // Breadcrumb navigatie (aangepast om met current_lib te werken)
                if let Some(artist) = current_lib.artists.get(self.selected_artist) {
                    ui.label(format!(" > {}", artist.name));
                    if matches!(
                        self.current_level,
                        NavLevel::Album | NavLevel::Disk | NavLevel::Track
                    ) {
                        if let Some(album) = artist.albums.get(self.selected_album) {
                            ui.label(format!(" > {}", album.title));
                            if matches!(self.current_level, NavLevel::Disk | NavLevel::Track) {
                                if let Some(disk) = album.disks.get(self.selected_disk) {
                                    ui.label(format!(" > {}", disk.name));
                                }
                            }
                        }
                    }
                }
            });

            ui.separator();

            match self.view_mode {
                ViewMode::AlbumCover if self.current_level != NavLevel::Track => {
                    if self.current_level == NavLevel::Artist {
                        let albums = &current_lib.artists[self.selected_artist].albums;
                        let num_albums = albums.len();

                        if num_albums == 0 {
                            ui.centered_and_justified(|ui| {
                                ui.label("Geen albums");
                            });
                        } else {
                            ScrollArea::vertical().show(ui, |ui| {
                                let available = ui.available_width();
                                let desired_thumb = 220.0_f32;
                                let mut columns = (available / desired_thumb).floor() as usize;
                                if columns == 0 {
                                    columns = 1;
                                }
                                columns = std::cmp::min(columns, std::cmp::max(1, num_albums));
                                let padding = 12.0_f32;
                                let thumb_w = ((available - padding * (columns as f32 + 1.0))
                                    / columns as f32)
                                    .max(80.0)
                                    .min(600.0);
                                let thumb_size = egui::vec2(thumb_w, thumb_w);

                                if num_albums == 1 {
                                    ui.centered_and_justified(|ui| {
                                        if let Some(path) = &albums[0].cover_path {
                                            let big_w = (available * 0.6).max(200.0).min(800.0);
                                            let resp = ui.add_sized(
                                                egui::vec2(big_w, big_w),
                                                Image::new(format!("file://{}", path))
                                                    .show_loading_spinner(false)
                                                    .sense(egui::Sense::click()),
                                            );
                                            if resp.clicked() {
                                                self.current_level = NavLevel::Album;
                                                self.selected_album = 0;
                                                self.scroll_to_selection = true;
                                            }
                                        }
                                        ui.add_space(6.0);
                                        ui.label(RichText::new(&albums[0].title).size(20.0));
                                    });
                                } else {
                                    ui.columns(columns, |cols| {
                                        for (i, album) in albums.iter().enumerate() {
                                            let col = &mut cols[i % columns];

                                            // FIX: Forceer een centrering layout binnen de kolom.
                                            // Dit zorgt ervoor dat zowel de cover als de tekst netjes onder elkaar in het midden van de kolom staan.
                                            col.with_layout(
                                                egui::Layout::top_down(egui::Align::Center),
                                                |col_ui| {
                                                    if let Some(path) = &album.cover_path {
                                                        let resp = col_ui.add_sized(
                                                            thumb_size,
                                                            Image::new(format!("file://{}", path))
                                                                .show_loading_spinner(false)
                                                                .sense(egui::Sense::click()),
                                                        );
                                                        if resp.clicked() {
                                                            self.current_level = NavLevel::Album;
                                                            self.selected_album = i;
                                                            self.scroll_to_selection = true;
                                                        }
                                                    } else {
                                                        // Ruimte reserveren als er geen cover is, zodat de tekst niet naar boven schuift
                                                        col_ui.add_space(thumb_size.y);
                                                    }

                                                    col_ui.add_space(6.0);
                                                    // De tekst wordt nu netjes gecentreerd onder de cover getoond
                                                    col_ui.label(
                                                        RichText::new(&album.title)
                                                            .size(14.0)
                                                            .color(Color32::WHITE), // Ook direct de tekst wit gemaakt voor beter contrast
                                                    );
                                                },
                                            );
                                        }
                                    });
                                }
                            });
                        }
                    } else {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.centered_and_justified(|ui| {
                                if let Some(album) = current_lib
                                    .artists
                                    .get(self.selected_artist)
                                    .and_then(|a| a.albums.get(self.selected_album))
                                {
                                    if let Some(path) = &album.cover_path {
                                        let available = ui.available_width();
                                        let size_w = (available * 0.5).max(200.0).min(1200.0);
                                        let _ = ui.add_sized(
                                            egui::vec2(size_w, size_w),
                                            Image::new(format!("file://{}", path))
                                                .show_loading_spinner(false),
                                        );
                                    }
                                    ui.add_space(6.0);
                                    ui.label(RichText::new(&album.title).size(20.0));
                                }
                            });
                        });
                    }
                }
                _ => {
                    ScrollArea::vertical().show(ui, |ui| {
                        // FIX: Pas de centrering layout ÉÉN KEER toe op de hele lijst!
                        // Dit is véél sneller dan het per item te doen.
                        ui.with_layout(
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| match self.current_level {
                                NavLevel::Artist => {
                                    for (i, artist) in current_lib.artists.iter().enumerate() {
                                        let resp = ui.selectable_label(
                                            i == self.selected_artist,
                                            RichText::new(&artist.name).size(18.0),
                                        );
                                        if resp.clicked() {
                                            self.selected_artist = i;
                                            self.scroll_to_selection = true;
                                        }
                                        if i == self.selected_artist && self.scroll_to_selection {
                                            resp.scroll_to_me(None);
                                        }
                                    }
                                }
                                NavLevel::Album => {
                                    for (i, album) in current_lib.artists[self.selected_artist]
                                        .albums
                                        .iter()
                                        .enumerate()
                                    {
                                        let resp = ui.selectable_label(
                                            i == self.selected_album,
                                            RichText::new(&album.title).size(18.0),
                                        );
                                        if resp.clicked() {
                                            self.selected_album = i;
                                            self.scroll_to_selection = true;
                                        }
                                        if i == self.selected_album && self.scroll_to_selection {
                                            resp.scroll_to_me(None);
                                        }
                                    }
                                }
                                NavLevel::Disk => {
                                    for (i, disk) in current_lib.artists[self.selected_artist]
                                        .albums[self.selected_album]
                                        .disks
                                        .iter()
                                        .enumerate()
                                    {
                                        let resp = ui.selectable_label(
                                            i == self.selected_disk,
                                            RichText::new(format!("CD: {}", disk.name)).size(16.0),
                                        );
                                        if resp.clicked() {
                                            self.selected_disk = i;
                                            self.scroll_to_selection = true;
                                        }
                                        if i == self.selected_disk && self.scroll_to_selection {
                                            resp.scroll_to_me(None);
                                        }
                                    }
                                }
                                NavLevel::Track => {
                                    for (i, track) in current_lib.artists[self.selected_artist]
                                        .albums[self.selected_album]
                                        .disks[self.selected_disk]
                                        .tracks
                                        .iter()
                                        .enumerate()
                                    {
                                        let is_selected = i == self.selected_track;
                                        let is_marked = self.selected_tracks.contains(&track.path);

                                        let display_title = if is_marked {
                                            format!("☑ {}", track.title)
                                        } else {
                                            track.title.clone()
                                        };

                                        let resp = ui.selectable_label(
                                            is_selected,
                                            RichText::new(&display_title).size(16.0),
                                        );

                                        if resp.clicked() {
                                            self.selected_track = i;
                                            self.scroll_to_selection = true;
                                        }

                                        if is_selected && self.scroll_to_selection {
                                            resp.scroll_to_me(None);
                                        }
                                    }
                                }
                            },
                        );
                    });
                }
            }
        });

        // ---  : TRACK DETAILS POPUP ---
        if self.show_track_details {
            let mut _is_open = self.show_track_details;
            // --- TRACK DETAILS POPUP ---
            if self.show_track_details {
                let mut is_open = self.show_track_details;
                let popup_title = if self.tracks_to_edit.len() > 1 {
                    format!(
                        "Batch Edit: {} tracks geselecteerd",
                        self.tracks_to_edit.len()
                    )
                } else {
                    "Track Details & Tags".to_string()
                };

                egui::Window::new(popup_title)
                    .open(&mut is_open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(500.0)
                    .show(ctx, |ui| {
                        if let Some(path) = &self.editing_track_path {
                            ui.label(RichText::new("File:").strong());
                            ui.label(RichText::new(path).size(12.0).color(Color32::GRAY));
                            ui.add_space(10.0);

                            // NIEUW: Lijstje met geselecteerde bestanden (alleen als er meer dan 1 is)
                            if self.tracks_to_edit.len() > 1 {
                                ui.label(
                                    RichText::new("Geselecteerde bestanden:")
                                        .strong()
                                        .size(14.0),
                                );
                                ScrollArea::vertical()
                                    .max_height(120.0) // Beperk de hoogte zodat het venster niet explodeert
                                    .show(ui, |ui| {
                                        // We clonen de lijst even om borrow-checker problemen te voorkomen tijdens het verwijderen
                                        let current_paths: Vec<String> =
                                            self.tracks_to_edit.clone();

                                        for (i, path) in current_paths.iter().enumerate() {
                                            ui.horizontal(|ui| {
                                                // Toon alleen de bestandsnaam voor een schone look
                                                let filename = Path::new(path)
                                                    .file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy();

                                                ui.label(
                                                    RichText::new(filename.to_string())
                                                        .size(12.0)
                                                        .color(Color32::GRAY),
                                                );
                                                ui.add_space(10.0);

                                                // Het "untag" knopje
                                                if ui.small_button("❌").clicked() {
                                                    self.selected_tracks.remove(path);
                                                    self.tracks_to_edit.remove(i);
                                                }
                                            });
                                        }
                                    });
                                ui.separator();
                                ui.add_space(5.0);
                            }

                            // NIEUW: Toon de error in het rood als het bestand corrupt is
                            if let Some(err) = &self.read_error {
                                ui.label(
                                    RichText::new(format!("⚠️ Leesfout: {}", err))
                                        .color(Color32::RED)
                                        .strong(),
                                );
                                ui.add_space(5.0);
                            }

                            // De bestaande editable velden
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.update_title, "");
                                ui.label("Title:");
                                ui.add_sized(
                                    [400.0, 20.0],
                                    egui::TextEdit::singleline(&mut self.edit_title)
                                        .interactive(self.update_title),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.update_artist, "");
                                ui.label("Artist:");
                                ui.add_sized(
                                    [400.0, 20.0],
                                    egui::TextEdit::singleline(&mut self.edit_artist)
                                        .interactive(self.update_artist),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.update_album, "");
                                ui.label("Album:");
                                ui.add_sized(
                                    [400.0, 20.0],
                                    egui::TextEdit::singleline(&mut self.edit_album)
                                        .interactive(self.update_album),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.update_genre, "");
                                ui.label("Genre:");
                                ui.add_sized(
                                    [400.0, 20.0],
                                    egui::TextEdit::singleline(&mut self.edit_genre)
                                        .interactive(self.update_genre),
                                );
                            });

                            ui.add_space(15.0);
                            ui.horizontal(|ui| {
                                if ui.button("💾 Save to File").clicked() {
                                    self.save_track_tags();
                                }
                                if ui.button("Cancel").clicked() {
                                    self.show_track_details = false;
                                }
                                if let Some(status) = &self.save_status {
                                    let color = if status.contains("Error") {
                                        Color32::RED
                                    } else {
                                        Color32::GREEN
                                    };
                                    ui.label(RichText::new(status).color(color));
                                }
                            });

                            // NIEUW: Scheidingslijn en het Raw Tags overzicht
                            ui.add_space(15.0);
                            ui.separator();
                            ui.add_space(5.0);

                            ui.label(RichText::new("Alle Ruwe Tags (Read-Only):").strong());
                            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.raw_tags_display)
                                        .font(egui::TextStyle::Monospace) // Mooier lettertype voor data
                                        .desired_width(f32::INFINITY)
                                        .interactive(false), // Maakt het veld alleen-lezen
                                );
                            });
                        }
                    });
                self.show_track_details = is_open;
            }
        }

        self.scroll_to_selection = false;
        ctx.request_repaint();
    }
}
