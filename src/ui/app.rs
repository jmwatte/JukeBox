use crate::config::Config;
use crate::models::{Album, Library};
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::{
    collect_composers, collect_genres, collect_years, filter_by_composer, filter_by_genre,
    filter_by_year,
};
use crate::ui::shortcuts;
use crate::ui::types::{Layer, NavLevel, ViewMode};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use std::collections::HashSet;

pub struct MusicPlayerApp {
    pub config: Config,
    pub player_tx: Sender<PlayerCommand>,
    pub player_event_rx: Receiver<PlayerEvent>,
    pub scanner_tx: Sender<ScannerMessage>,
    pub scanner_rx: Receiver<ScannerMessage>,
    pub library: Option<Library>,

    /// De filter-stapel. Leeg = Root = volledige bibliotheek.
    pub filter_stack: Vec<Layer>,
    /// Gecachte bibliotheek na toepassen van alle filters (voor navigatie).
    pub cached_filtered: Option<Library>,

    // Status
    pub now_playing: Option<String>,
    pub show_help: bool,
    pub _status_message: String,

    // Navigatie
    pub current_level: NavLevel,
    pub view_mode: ViewMode,
    pub selected_artist: usize,
    pub selected_album: usize,
    pub selected_disk: usize,
    pub selected_track: usize,
    pub scroll_to_selection: bool,
    pub search_query: String,

    // Search
    pub is_search_active: bool,
    pub search_input_id: egui::Id,
    pub filtered_library: Option<Library>,

    // Picker state (genre, year, etc.)
    pub genres: Vec<(String, usize)>,
    pub selected_genre: usize,
    pub selected_genre_name: String,
    pub sort_by_date: bool,

    // Recent Albums
    pub recent_albums: Vec<(String, Album)>,
    pub selected_recent: usize,

    // Track Details / Batch Edit
    pub show_track_details: bool,
    pub editing_track_path: Option<String>,
    pub edit_title: String,
    pub edit_artist: String,
    pub edit_album: String,
    pub edit_genre: String,
    pub save_status: Option<String>,
    pub raw_tags_display: String,
    pub read_error: Option<String>,
    pub update_title: bool,
    pub update_artist: bool,
    pub update_album: bool,
    pub update_genre: bool,
    pub selected_tracks: HashSet<String>,
    pub tracks_to_edit: Vec<String>,

    // Jaar/componist pickers (voor later)
    pub years: Vec<(u32, usize)>,
    pub selected_year: usize,
    pub composers: Vec<(String, usize)>,
    pub selected_composer: usize,
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
            filter_stack: Vec::new(),
            cached_filtered: None,
            now_playing: None,
            show_help: false,
            _status_message: "Bibliotheek opstarten...".to_string(),
            filtered_library: None,
            search_query: String::new(),
            current_level: NavLevel::Artist,
            view_mode,
            selected_artist: 0,
            selected_album: 0,
            selected_disk: 0,
            selected_track: 0,
            scroll_to_selection: true,
            is_search_active: false,
            search_input_id: eframe::egui::Id::new("global_search_input"),
            genres: Vec::new(),
            selected_genre: 0,
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
            years: Vec::new(),
            selected_year: 0,
            composers: Vec::new(),
            selected_composer: 0,
        }
    }

    // === FILTER STACK ===

    /// De actieve bibliotheek (na alle filters). Gebruik dit voor navigatie & weergave.
    pub fn active_library(&self) -> Option<&Library> {
        self.filtered_library
            .as_ref()
            .or(self.cached_filtered.as_ref())
            .or(self.library.as_ref())
    }

    /// Pas alle filters in de stack toe op `library` en cache het resultaat.
    pub fn recompute(&mut self) {
        let Some(ref base) = self.library else {
            self.cached_filtered = None;
            return;
        };
        let mut result = base.clone();
        for layer in &self.filter_stack {
            match layer {
                Layer::Genre(name) => {
                    result = filter_by_genre(&result, name);
                }
                Layer::Selection => {
                    result = build_selection_library(&result, &self.selected_tracks);
                }
                Layer::Root
                | Layer::GenrePicker
                | Layer::YearPicker
                | Layer::ComposerPicker
                | Layer::RecentAlbums => {
                    // Pickers veranderen de set niet
                }
                Layer::Year(y) => {
                    result = filter_by_year(&result, *y);
                }
                Layer::Composer(c) => {
                    result = filter_by_composer(&result, c);
                }
            }
        }
        self.cached_filtered = Some(result);
    }

    /// Push een nieuwe layer (picker of filter) en herbereken.
    pub fn push_layer(&mut self, layer: Layer) {
        self.filter_stack.push(layer);
        self.recompute();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.selected_album = 0;
        self.selected_disk = 0;
        self.selected_track = 0;
        self.scroll_to_selection = true;
    }

    /// Pop de bovenste layer en herbereken. Doet niets als alleen Root overblijft.
    pub fn pop_layer(&mut self) {
        if self.filter_stack.len() > 1
            || (self.filter_stack.len() == 1 && self.filter_stack[0] != Layer::Root)
        {
            self.filter_stack.pop();
        } else {
            self.filter_stack.clear();
        }
        self.recompute();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.selected_album = 0;
        self.selected_disk = 0;
        self.selected_track = 0;
        self.scroll_to_selection = true;
    }

    /// Reset de filter stack naar leeg (volledige bibliotheek).
    pub fn reset_filters(&mut self) {
        self.filter_stack.clear();
        self.recompute();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.selected_album = 0;
        self.selected_disk = 0;
        self.selected_track = 0;
        self.scroll_to_selection = true;
    }

    /// Check of de huidige data een picker toont (genre/year/composer lijst).
    pub fn is_picker_active(&self) -> bool {
        self.filter_stack
            .last()
            .map(|l| l.is_picker())
            .unwrap_or(false)
    }

    /// Genereer de breadcrumb-string uit de filter stack.
    pub fn breadcrumb(&self) -> String {
        let mut parts: Vec<String> = self.filter_stack.iter().map(|l| l.display_name()).collect();
        if parts.is_empty() {
            parts.push("Bibliotheek".into());
        }
        parts.join(" > ")
    }

    // === HELPER: Selection library builder ===

    /// Haal de library van de vorige filterlaag (of de volledige library) op.
    /// Gebruikt voor het vullen van pickers (genres, jaren etc.) die moeten werken
    /// op de set **voordat** de picker werd gepusht.
    pub fn library_before_top_picker(&self) -> Option<Library> {
        // Als de stack eindigt op een picker, negeer die dan voor de data
        let mut result = self.library.clone()?;
        let picker_count = self
            .filter_stack
            .iter()
            .rev()
            .take_while(|l| l.is_picker())
            .count();
        let effective_len = self.filter_stack.len() - picker_count;

        for layer in self.filter_stack.iter().take(effective_len) {
            match layer {
                Layer::Genre(name) => result = filter_by_genre(&result, name),
                Layer::Selection => {
                    result = build_selection_library(&result, &self.selected_tracks)
                }
                Layer::Year(y) => result = filter_by_year(&result, *y),
                Layer::Composer(c) => result = filter_by_composer(&result, c),
                _ => {}
            }
        }
        Some(result)
    }

    // === OUDE HELPERS (herwerkt voor stack) ===

    pub fn toggle_sort(&mut self) {
        self.sort_by_date = !self.sort_by_date;

        let sort_fn = |lib: &mut Library| {
            if self.sort_by_date {
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
                for artist in &mut lib.artists {
                    artist
                        .albums
                        .sort_by(|a, b| b.added_timestamp.cmp(&a.added_timestamp));
                }
            } else {
                lib.artists.sort_by(|a, b| a.name.cmp(&b.name));
                for artist in &mut lib.artists {
                    artist.albums.sort_by(|a, b| a.title.cmp(&b.title));
                }
            }
        };

        if let Some(lib) = &mut self.library {
            sort_fn(lib);
        }
        if let Some(lib) = &mut self.filtered_library {
            sort_fn(lib);
        }
        if let Some(lib) = &mut self.cached_filtered {
            sort_fn(lib);
        }
        self.selected_artist = 0;
        self.selected_album = 0;
        self.scroll_to_selection = true;
    }

    pub fn enter_genre_picker(&mut self) {
        // Bepaal de al geselecteerde genre (als die er is) om te highlighten
        let current_genre = self.filter_stack.iter().rev().find_map(|l| {
            if let Layer::Genre(name) = l {
                Some(name.clone())
            } else {
                None
            }
        });

        if let Some(lib) = self.library_before_top_picker() {
            self.genres = collect_genres(&lib);
            self.selected_genre = current_genre
                .as_ref()
                .and_then(|g| self.genres.iter().position(|(name, _)| name == g))
                .unwrap_or(0);
            self.selected_genre_name = current_genre.unwrap_or_default();
            self.push_layer(Layer::GenrePicker);
        }
    }

    pub fn select_genre(&mut self, genre: &str) {
        // Vervang de GenrePicker door een Genre filter
        let _ = self.filter_stack.pop(); // verwijder GenrePicker
        self.selected_genre_name = genre.to_string();
        self.push_layer(Layer::Genre(genre.to_string()));
    }

    pub fn enter_year_picker(&mut self) {
        let current_year = self.filter_stack.iter().rev().find_map(|l| {
            if let Layer::Year(y) = l {
                Some(*y)
            } else {
                None
            }
        });
        if let Some(lib) = self.library_before_top_picker() {
            self.years = collect_years(&lib);
            self.selected_year = current_year
                .and_then(|y| self.years.iter().position(|(year, _)| *year == y))
                .unwrap_or(0);
            self.push_layer(Layer::YearPicker);
        }
    }

    pub fn select_year(&mut self, year: u32) {
        let _ = self.filter_stack.pop();
        self.push_layer(Layer::Year(year));
    }

    pub fn enter_composer_picker(&mut self) {
        let current_composer = self.filter_stack.iter().rev().find_map(|l| {
            if let Layer::Composer(c) = l {
                Some(c.clone())
            } else {
                None
            }
        });
        if let Some(lib) = self.library_before_top_picker() {
            self.composers = collect_composers(&lib);
            self.selected_composer = current_composer
                .as_ref()
                .and_then(|c| self.composers.iter().position(|(name, _)| name == c))
                .unwrap_or(0);
            self.push_layer(Layer::ComposerPicker);
        }
    }

    pub fn select_composer(&mut self, composer: &str) {
        let _ = self.filter_stack.pop();
        self.push_layer(Layer::Composer(composer.to_string()));
    }

    pub fn enter_recent_mode(&mut self) {
        if let Some(lib) = self.library_before_top_picker() {
            let mut flat_albums = Vec::new();
            for artist in &lib.artists {
                for album in &artist.albums {
                    flat_albums.push((artist.name.clone(), album.clone()));
                }
            }
            flat_albums.sort_by(|a, b| b.1.added_timestamp.cmp(&a.1.added_timestamp));
            flat_albums.truncate(500);
            self.recent_albums = flat_albums;
            self.selected_recent = 0;
            self.push_layer(Layer::RecentAlbums);
        }
    }

    pub fn enter_selection_mode(&mut self) {
        if self.selected_tracks.is_empty() {
            return;
        }
        self.push_layer(Layer::Selection);
    }

    pub fn play_selected_item(&self, lib: &Library, replace: bool) {
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

    pub fn get_current_track_path(&self, lib: &Library) -> Option<String> {
        lib.artists
            .get(self.selected_artist)
            .and_then(|a| a.albums.get(self.selected_album))
            .and_then(|al| al.disks.get(self.selected_disk))
            .and_then(|d| d.tracks.get(self.selected_track))
            .map(|t| t.path.clone())
    }

    // === MARKERING OP ALLE NIVEAUS ===

    pub fn get_tracks_at_level(&self, lib: &Library, level: &NavLevel) -> Vec<String> {
        match level {
            NavLevel::Track => self.get_current_track_path(lib).into_iter().collect(),
            NavLevel::Disk => lib
                .artists
                .get(self.selected_artist)
                .and_then(|a| a.albums.get(self.selected_album))
                .and_then(|al| al.disks.get(self.selected_disk))
                .map(|d| d.tracks.iter().map(|t| t.path.clone()).collect())
                .unwrap_or_default(),
            NavLevel::Album => lib
                .artists
                .get(self.selected_artist)
                .and_then(|a| a.albums.get(self.selected_album))
                .map(|al| {
                    al.disks
                        .iter()
                        .flat_map(|d| d.tracks.iter().map(|t| t.path.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            NavLevel::Artist => lib
                .artists
                .get(self.selected_artist)
                .map(|a| {
                    a.albums
                        .iter()
                        .flat_map(|al| {
                            al.disks
                                .iter()
                                .flat_map(|d| d.tracks.iter().map(|t| t.path.clone()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Tel geselecteerde tracks (voor UI weergave)
    pub fn selected_count(&self) -> usize {
        self.selected_tracks.len()
    }

    /// Generieke picker-navigatie: pijltjes, select, M (markeren)
    pub fn handle_picker_navigation(
        &mut self,
        ctx: &egui::Context,
        cfg: &std::collections::HashMap<String, String>,
        len: usize,
        selected: &mut usize,
        get_selected_name: impl Fn(&Self) -> Option<String>,
        select_action: impl FnOnce(String),
        _extra: impl Fn(usize, &mut egui::Ui),
    ) {
        if shortcuts::check_action(cfg, ctx, "NavigateDown") {
            if *selected + 1 < len {
                *selected += 1;
                self.scroll_to_selection = true;
            }
        }
        if shortcuts::check_action(cfg, ctx, "NavigateUp") {
            if *selected > 0 {
                *selected -= 1;
                self.scroll_to_selection = true;
            }
        }
        if shortcuts::check_action(cfg, ctx, "Select")
            || shortcuts::check_action(cfg, ctx, "NavigateRight")
        {
            if let Some(name) = get_selected_name(self) {
                select_action(name);
            }
        }
        // M op picker: alle tracks van dit item markeren
        if shortcuts::check_action(cfg, ctx, "MarkTrack") {
            if let Some(name) = get_selected_name(self) {
                let base_lib = self.library_before_top_picker();
                if let Some(lib) = base_lib {
                    let filtered = self.filter_stack.iter().rev().find_map(|l| match l {
                        Layer::GenrePicker => Some(crate::search::filter_by_genre(&lib, &name)),
                        Layer::YearPicker => {
                            if let Ok(y) = name.parse::<u32>() {
                                Some(crate::search::filter_by_year(&lib, y))
                            } else {
                                None
                            }
                        }
                        Layer::ComposerPicker => {
                            Some(crate::search::filter_by_composer(&lib, &name))
                        }
                        _ => None,
                    });
                    if let Some(filtered) = filtered {
                        let paths: Vec<String> = filtered
                            .artists
                            .iter()
                            .flat_map(|a| {
                                a.albums.iter().flat_map(|al| {
                                    al.disks
                                        .iter()
                                        .flat_map(|d| d.tracks.iter().map(|t| t.path.clone()))
                                })
                            })
                            .collect();
                        if !paths.is_empty() {
                            let all_selected = paths
                                .iter()
                                .all(|p| self.selected_tracks.contains(p.as_str()));
                            if all_selected {
                                for p in &paths {
                                    self.selected_tracks.remove(p);
                                }
                            } else {
                                for p in &paths {
                                    self.selected_tracks.insert(p.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Bouw een Library uit alleen de geselecteerde tracks.
fn build_selection_library(lib: &Library, selected: &HashSet<String>) -> Library {
    let mut artist_map: std::collections::HashMap<
        String,
        std::collections::HashMap<String, Vec<crate::models::Track>>,
    > = std::collections::HashMap::new();

    for artist in &lib.artists {
        for album in &artist.albums {
            for disk in &album.disks {
                for track in &disk.tracks {
                    if selected.contains(&track.path) {
                        artist_map
                            .entry(artist.name.clone())
                            .or_default()
                            .entry(album.title.clone())
                            .or_default()
                            .push(track.clone());
                    }
                }
            }
        }
    }

    let mut artists = Vec::new();
    for (artist_name, albums_map) in artist_map {
        let mut albums = Vec::new();
        for (album_title, tracks) in albums_map {
            albums.push(crate::models::Album {
                title: album_title,
                cover_path: None,
                disks: vec![crate::models::Disk {
                    name: "Default".into(),
                    tracks,
                }],
                added_timestamp: 0,
            });
        }
        albums.sort_by(|a, b| a.title.cmp(&b.title));
        artists.push(crate::models::Artist {
            name: artist_name,
            albums,
        });
    }
    artists.sort_by(|a, b| a.name.cmp(&b.name));
    Library { artists }
}
