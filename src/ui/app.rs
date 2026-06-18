use crate::config::Config;
use crate::models::{Album, Library};
use crate::player::{PlayerCommand, PlayerEvent, RepeatMode};
use crate::scanner::ScannerMessage;
use crate::search::{
    collect_composers, collect_genres, collect_years, filter_by_composer, filter_by_genre,
    filter_by_year,
};
use crate::ui::types::{FilterNode, NavLevel, ViewMode};
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

    /// De filter-pipeline. Elke node is een filtertype met een optionele waarde.
    /// - `None` = picker-mode (gebruiker moet nog kiezen)
    /// - `Some(...)` = actief filter
    pub filter_path: Vec<FilterNode>,
    /// Huidige positie in de pipeline (0..=filter_path.len()).
    /// Als filter_step == filter_path.len(), zijn we door alle pickers heen.
    pub filter_step: usize,
    /// Gecachte bibliotheek na toepassen van filters tot aan filter_step.
    pub cached_filtered: Option<Library>,

    // Status
    pub now_playing: Option<String>,
    pub now_playing_path: Option<String>,
    pub now_playing_position: f32,
    pub now_playing_duration: f32,
    pub volume: f32,
    pub repeat_mode: RepeatMode,
    pub shuffle_on: bool,
    pub show_queue: bool,
    pub queue: Vec<String>,
    pub loop_a: Option<f32>,
    pub loop_b: Option<f32>,
    pub status_error: Option<String>,
    pub compact_mode: bool,
    pub config_errors: Vec<String>,
    pub force_help: bool,
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
    pub edit_year: String,
    pub edit_composer: String,
    pub save_status: Option<String>,
    pub raw_tags_display: String,
    pub read_error: Option<String>,
    pub update_title: bool,
    pub update_artist: bool,
    pub update_album: bool,
    pub update_genre: bool,
    pub update_year: bool,
    pub update_composer: bool,
    pub update_remove_genre: bool,
    pub remove_genre_text: String,
    pub selected_tracks: HashSet<String>,
    pub tracks_to_edit: Vec<String>,

    // Jaar/componist pickers (voor later)
    pub years: Vec<(Option<u32>, usize)>,
    pub selected_year: usize,
    pub composers: Vec<(String, usize)>,
    pub selected_composer: usize,

    /// Split ratio between "Bestanden" and "Ruwe tags" columns in the batch edit panel (0.0–1.0).
    pub edit_panel_split: f32,
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
        let mut app = Self {
            config,
            player_tx,
            player_event_rx,
            scanner_tx,
            scanner_rx,
            library: None,
            filter_path: Vec::new(),
            filter_step: 0,
            cached_filtered: None,
            now_playing: None,
            now_playing_path: None,
            now_playing_position: 0.0,
            now_playing_duration: 0.0,
            volume: 1.0,
            repeat_mode: RepeatMode::None,
            shuffle_on: false,
            show_queue: false,
            queue: Vec::new(),
            loop_a: None,
            loop_b: None,
            status_error: None,
            compact_mode: false,
            config_errors: Vec::new(),
            force_help: false,
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
            edit_year: String::new(),
            edit_composer: String::new(),
            save_status: None,
            raw_tags_display: String::new(),
            read_error: None,
            update_title: false,
            update_artist: false,
            update_album: false,
            update_genre: false,
            update_year: false,
            update_composer: false,
            update_remove_genre: false,
            remove_genre_text: String::new(),
            selected_tracks: HashSet::new(),
            tracks_to_edit: Vec::new(),
            years: Vec::new(),
            selected_year: 0,
            composers: Vec::new(),
            selected_composer: 0,
            edit_panel_split: 0.4,
        };

        // Valideer shortcuts bij opstarten
        let errors = crate::ui::shortcuts::validate_shortcuts(&app.config.shortcuts);
        if !errors.is_empty() {
            app.config_errors = errors;
            app.force_help = true;
        }

        app
    }

    // === FILTER PIPELINE ===

    /// De actieve bibliotheek (na alle filters). Gebruik dit voor navigatie & weergave.
    pub fn active_library(&self) -> Option<&Library> {
        self.filtered_library
            .as_ref()
            .or(self.cached_filtered.as_ref())
            .or(self.library.as_ref())
    }

    /// Pas ALLEEN de filters toe tot de huidige filter_step.
    pub fn recompute(&mut self) {
        let Some(ref base) = self.library else {
            self.cached_filtered = None;
            return;
        };
        let mut result = base.clone();
        for node in self.filter_path.iter().take(self.filter_step) {
            match node {
                FilterNode::Genre(Some(name)) => {
                    result = filter_by_genre(&result, name);
                }
                FilterNode::Year(Some(y)) => {
                    result = filter_by_year(&result, *y);
                }
                FilterNode::Composer(Some(c)) => {
                    result = filter_by_composer(&result, c);
                }
                _ => {} // None = picker, slaat geen filter toe
            }
        }
        self.cached_filtered = Some(result);

        // Veiligheid: selectie-indices resetten als ze out-of-bounds zijn
        if let Some(ref lib) = self.cached_filtered {
            if lib.artists.is_empty() {
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
            } else {
                if self.selected_artist >= lib.artists.len() {
                    self.selected_artist = lib.artists.len().saturating_sub(1);
                    self.selected_album = 0;
                    self.selected_disk = 0;
                    self.selected_track = 0;
                }
                let albums = &lib.artists[self.selected_artist].albums;
                if self.selected_album >= albums.len() {
                    self.selected_album = albums.len().saturating_sub(1);
                    self.selected_disk = 0;
                    self.selected_track = 0;
                }
                if !albums.is_empty() {
                    let disks = &albums[self.selected_album].disks;
                    if self.selected_disk >= disks.len() {
                        self.selected_disk = disks.len().saturating_sub(1);
                        self.selected_track = 0;
                    }
                    if !disks.is_empty() {
                        let tracks = &disks[self.selected_disk].tracks;
                        if self.selected_track >= tracks.len() {
                            self.selected_track = tracks.len().saturating_sub(1);
                        }
                    }
                }
            }
            if self.selected_genre >= self.genres.len() && !self.genres.is_empty() {
                self.selected_genre = self.genres.len().saturating_sub(1);
            }
            if self.selected_year >= self.years.len() && !self.years.is_empty() {
                self.selected_year = self.years.len().saturating_sub(1);
            }
            if self.selected_composer >= self.composers.len() && !self.composers.is_empty() {
                self.selected_composer = self.composers.len().saturating_sub(1);
            }
        }
    }

    /// Vul de huidige picker met data uit de (tot filter_step) gefilterde library.
    pub fn populate_current_picker(&mut self) {
        let Some(node) = self.filter_path.get(self.filter_step) else {
            return;
        };
        let Some(ref lib) = self.cached_filtered else {
            return;
        };

        match node {
            FilterNode::Genre(_) => {
                self.genres = collect_genres(lib);
                self.selected_genre = 0;
            }
            FilterNode::Year(_) => {
                self.years = collect_years(lib);
                self.selected_year = 0;
            }
            FilterNode::Composer(_) => {
                self.composers = collect_composers(lib);
                self.selected_composer = 0;
            }
        }
    }

    /// Ga één stap terug in de filter pipeline en herstel de cursor-positie
    /// naar het item dat eerder geselecteerd was.
    pub fn step_back_filter(&mut self) {
        if self.filter_step > 0 {
            self.filter_step -= 1;

            // 1. Bewaar wat we op deze laag hadden gekozen
            let previous_node = self.filter_path[self.filter_step].clone();

            // 2. Wis de waarde zodat het weer een Picker wordt (None)
            self.filter_path[self.filter_step].clear();

            // 3. Herbereken de library en vul de picker lijsten (dit zet index even op 0)
            self.recompute();
            self.populate_current_picker();

            // 4. Zoek de index van de oude keuze en overschrijf de 0!
            match previous_node {
                FilterNode::Genre(Some(g)) => {
                    if let Some(idx) = self.genres.iter().position(|(name, _)| name == &g) {
                        self.selected_genre = idx;
                    }
                }
                FilterNode::Year(Some(y)) => {
                    // y == 0 = sentinel voor "Onbekend" (None in de lijst)
                    let target: Option<u32> = if y == 0 { None } else { Some(y) };
                    if let Some(idx) = self.years.iter().position(|(val, _)| *val == target) {
                        self.selected_year = idx;
                    }
                }
                FilterNode::Composer(Some(c)) => {
                    if let Some(idx) = self.composers.iter().position(|(name, _)| name == &c) {
                        self.selected_composer = idx;
                    }
                }
                _ => {}
            }

            self.scroll_to_selection = true;
        }
    }

    /// Reset de filters naar leeg (volledige bibliotheek).
    pub fn reset_filters(&mut self) {
        self.filter_path.clear();
        self.filter_step = 0;
        self.recompute();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.selected_album = 0;
        self.selected_disk = 0;
        self.selected_track = 0;
        self.scroll_to_selection = true;
    }

    /// Check of de huidige filter_step op een picker wijst (None-waarde node).
    pub fn is_picker_active(&self) -> bool {
        self.filter_path
            .get(self.filter_step)
            .map(|node| {
                matches!(
                    node,
                    FilterNode::Genre(None) | FilterNode::Year(None) | FilterNode::Composer(None)
                )
            })
            .unwrap_or(false)
    }

    /// Genereer de breadcrumb-string uit de filter pipeline.
    pub fn breadcrumb(&self) -> String {
        let mut parts: Vec<String> = self.filter_path.iter().map(|n| n.display_name()).collect();
        if parts.is_empty() {
            parts.push("Bibliotheek".into());
        }
        parts.join(" > ")
    }

    // === HELPER: Selection library builder ===

    /// Bouw een Library uit alleen de geselecteerde tracks.
    pub fn build_selection_library(lib: &Library, selected: &HashSet<String>) -> Library {
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

    // === FILTER HELPERS ===

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

    /// Voeg een Genre-picker toe op de huidige positie, of verwijder hem als hij er al staat.
    pub fn toggle_genre_picker(&mut self) {
        // Staat er al een Genre node op de huidige filter_step? -> Verwijder hem
        if let Some(FilterNode::Genre(_)) = self.filter_path.get(self.filter_step) {
            self.filter_path.remove(self.filter_step);
            if self.filter_step > self.filter_path.len() {
                self.filter_step = self.filter_path.len();
            }
            self.recompute();
            self.populate_current_picker();
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.scroll_to_selection = true;
            return;
        }

        // Voorkom duplicaten in de pipeline
        if self
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Genre(_)))
        {
            return;
        }

        // Voeg een lege Genre node in op de huidige positie
        self.filter_path
            .insert(self.filter_step, FilterNode::Genre(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een genre in de huidige Genre-picker.
    pub fn select_genre(&mut self, genre: &str) {
        self.selected_genre_name = genre.to_string();
        if let Some(FilterNode::Genre(val)) = self.filter_path.get_mut(self.filter_step) {
            *val = Some(genre.to_string());
            self.filter_step += 1;
            self.recompute();
            if self.filter_step < self.filter_path.len() {
                self.populate_current_picker();
            } else {
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
            }
            self.scroll_to_selection = true;
        }
    }

    /// Voeg een Year-picker toe op de huidige positie, of verwijder hem als hij er al staat.
    pub fn toggle_year_picker(&mut self) {
        if let Some(FilterNode::Year(_)) = self.filter_path.get(self.filter_step) {
            self.filter_path.remove(self.filter_step);
            if self.filter_step > self.filter_path.len() {
                self.filter_step = self.filter_path.len();
            }
            self.recompute();
            self.populate_current_picker();
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.scroll_to_selection = true;
            return;
        }

        if self
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Year(_)))
        {
            return;
        }

        self.filter_path
            .insert(self.filter_step, FilterNode::Year(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een jaar in de huidige Year-picker.
    pub fn select_year(&mut self, year: u32) {
        if let Some(FilterNode::Year(val)) = self.filter_path.get_mut(self.filter_step) {
            *val = Some(year);
            self.filter_step += 1;
            self.recompute();
            if self.filter_step < self.filter_path.len() {
                self.populate_current_picker();
            } else {
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
            }
            self.scroll_to_selection = true;
        }
    }

    /// Voeg een Composer-picker toe op de huidige positie, of verwijder hem als hij er al staat.
    pub fn toggle_composer_picker(&mut self) {
        if let Some(FilterNode::Composer(_)) = self.filter_path.get(self.filter_step) {
            self.filter_path.remove(self.filter_step);
            if self.filter_step > self.filter_path.len() {
                self.filter_step = self.filter_path.len();
            }
            self.recompute();
            self.populate_current_picker();
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.scroll_to_selection = true;
            return;
        }

        if self
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Composer(_)))
        {
            return;
        }

        self.filter_path
            .insert(self.filter_step, FilterNode::Composer(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een componist in de huidige Composer-picker.
    pub fn select_composer(&mut self, composer: &str) {
        if let Some(FilterNode::Composer(val)) = self.filter_path.get_mut(self.filter_step) {
            *val = Some(composer.to_string());
            self.filter_step += 1;
            self.recompute();
            if self.filter_step < self.filter_path.len() {
                self.populate_current_picker();
            } else {
                self.current_level = NavLevel::Artist;
                self.selected_artist = 0;
                self.selected_album = 0;
                self.selected_disk = 0;
                self.selected_track = 0;
            }
            self.scroll_to_selection = true;
        }
    }

    pub fn enter_recent_mode(&mut self) {
        if let Some(lib) = self.active_library().cloned() {
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
        }
    }

    pub fn enter_selection_mode(&mut self) {
        // Selection mode gebruikt de geselecteerde tracks direct
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

    /// Navigeer naar het huidig spelende nummer in de bibliotheek
    pub fn navigate_to_now_playing(&mut self, lib: &Library) {
        let target = match &self.now_playing_path {
            Some(p) => p.clone(),
            None => return,
        };

        for (ai, artist) in lib.artists.iter().enumerate() {
            for (ali, album) in artist.albums.iter().enumerate() {
                for (di, disk) in album.disks.iter().enumerate() {
                    for (ti, track) in disk.tracks.iter().enumerate() {
                        if track.path == target {
                            self.selected_artist = ai;
                            self.selected_album = ali;
                            self.selected_disk = di;
                            self.selected_track = ti;
                            self.current_level = crate::ui::types::NavLevel::Track;
                            self.scroll_to_selection = true;
                            return;
                        }
                    }
                }
            }
        }
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
}
