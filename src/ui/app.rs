use crate::config::Config;
use crate::loops::SavedLoop;
use crate::models::Library;
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::{
    collect_composers, collect_genres, collect_years, filter_by_composer, filter_by_genre,
    filter_by_year,
};
use crate::ui::filters::FilterState;
use crate::ui::playback::PlaybackState;
use crate::ui::types::{FilterNode, NavLevel, ViewMode};

use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use std::collections::HashSet;

pub struct MusicPlayerApp {
    pub config: Config,
    pub playback: PlaybackState,
    pub filters: FilterState,
    pub scanner_tx: Sender<ScannerMessage>,
    pub scanner_rx: Receiver<ScannerMessage>,
    pub library: Option<Library>,
    pub config_errors: Vec<String>,
    pub force_help: bool,
    pub show_help: bool,

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

    /// Split ratio between "Bestanden" and "Ruwe tags" columns in the batch edit panel (0.0–1.0).
    pub edit_panel_split: f32,

    // Waveform Editor
    pub show_waveform: bool,
    pub waveform_state: crate::waveform::WaveformState,

    // Loop bibliotheek
    pub saved_loops: Vec<SavedLoop>,
    pub show_loop_library: bool,
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
            playback: PlaybackState::new(player_tx, player_event_rx),
            filters: FilterState::new(),
            scanner_tx,
            scanner_rx,
            library: None,
            config_errors: Vec::new(),
            force_help: false,
            show_help: false,
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
            edit_panel_split: 0.4,
            show_waveform: false,
            waveform_state: crate::waveform::WaveformState::default(),
            saved_loops: crate::loops::load_loops(),
            show_loop_library: false,
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
            .or(self.filters.cached_filtered.as_ref())
            .or(self.library.as_ref())
    }

    /// Pas ALLEEN de filters toe tot de huidige filter_step.
    pub fn recompute(&mut self) {
        let Some(ref base) = self.library else {
            self.filters.cached_filtered = None;
            return;
        };
        let mut result = base.clone();
        for node in self
            .filters
            .filter_path
            .iter()
            .take(self.filters.filter_step)
        {
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
        self.filters.cached_filtered = Some(result);

        // Veiligheid: selectie-indices resetten als ze out-of-bounds zijn
        if let Some(ref lib) = self.filters.cached_filtered {
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
            if self.filters.selected_genre >= self.filters.genres.len()
                && !self.filters.genres.is_empty()
            {
                self.filters.selected_genre = self.filters.genres.len().saturating_sub(1);
            }
            if self.filters.selected_year >= self.filters.years.len()
                && !self.filters.years.is_empty()
            {
                self.filters.selected_year = self.filters.years.len().saturating_sub(1);
            }
            if self.filters.selected_composer >= self.filters.composers.len()
                && !self.filters.composers.is_empty()
            {
                self.filters.selected_composer = self.filters.composers.len().saturating_sub(1);
            }
        }
    }

    /// Vul de huidige picker met data uit de (tot filter_step) gefilterde library.
    pub fn populate_current_picker(&mut self) {
        let Some(node) = self.filters.filter_path.get(self.filters.filter_step) else {
            return;
        };
        let Some(ref lib) = self.filters.cached_filtered else {
            return;
        };

        match node {
            FilterNode::Genre(_) => {
                self.filters.genres = collect_genres(lib);
                self.filters.selected_genre = 0;
            }
            FilterNode::Year(_) => {
                self.filters.years = collect_years(lib);
                self.filters.selected_year = 0;
            }
            FilterNode::Composer(_) => {
                self.filters.composers = collect_composers(lib);
                self.filters.selected_composer = 0;
            }
        }
    }

    /// Ga één stap terug in de filter pipeline en herstel de cursor-positie
    /// naar het item dat eerder geselecteerd was.
    pub fn step_back_filter(&mut self) {
        if self.filters.filter_step > 0 {
            self.filters.filter_step -= 1;

            // 1. Bewaar wat we op deze laag hadden gekozen
            let previous_node = self.filters.filter_path[self.filters.filter_step].clone();

            // 2. Wis de waarde zodat het weer een Picker wordt (None)
            self.filters.filter_path[self.filters.filter_step].clear();

            // 3. Herbereken de library en vul de picker lijsten (dit zet index even op 0)
            self.recompute();
            self.populate_current_picker();

            // 4. Zoek de index van de oude keuze en overschrijf de 0!
            match previous_node {
                FilterNode::Genre(Some(g)) => {
                    if let Some(idx) = self.filters.genres.iter().position(|(name, _)| name == &g) {
                        self.filters.selected_genre = idx;
                    }
                }
                FilterNode::Year(Some(y)) => {
                    // y == 0 = sentinel voor "Onbekend" (None in de lijst)
                    let target: Option<u32> = if y == 0 { None } else { Some(y) };
                    if let Some(idx) = self
                        .filters
                        .years
                        .iter()
                        .position(|(val, _)| *val == target)
                    {
                        self.filters.selected_year = idx;
                    }
                }
                FilterNode::Composer(Some(c)) => {
                    if let Some(idx) = self
                        .filters
                        .composers
                        .iter()
                        .position(|(name, _)| name == &c)
                    {
                        self.filters.selected_composer = idx;
                    }
                }
                _ => {}
            }

            self.scroll_to_selection = true;
        }
    }

    /// Reset de filters naar leeg (volledige bibliotheek).
    pub fn reset_filters(&mut self) {
        self.filters.filter_path.clear();
        self.filters.filter_step = 0;
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
        self.filters
            .filter_path
            .get(self.filters.filter_step)
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
        let mut parts: Vec<String> = self
            .filters
            .filter_path
            .iter()
            .map(|n| n.display_name())
            .collect();
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
        self.filters.sort_by_date = !self.filters.sort_by_date;

        let sort_fn = |lib: &mut Library| {
            if self.filters.sort_by_date {
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
        if let Some(lib) = &mut self.filters.cached_filtered {
            sort_fn(lib);
        }
        self.selected_artist = 0;
        self.selected_album = 0;
        self.scroll_to_selection = true;
    }

    /// Voeg een Genre-picker toe op de huidige positie, of verwijder hem als hij er al staat.
    pub fn toggle_genre_picker(&mut self) {
        // Staat er al een Genre node op de huidige filter_step? -> Verwijder hem
        if let Some(FilterNode::Genre(_)) = self.filters.filter_path.get(self.filters.filter_step) {
            self.filters.filter_path.remove(self.filters.filter_step);
            if self.filters.filter_step > self.filters.filter_path.len() {
                self.filters.filter_step = self.filters.filter_path.len();
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
            .filters
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Genre(_)))
        {
            return;
        }

        // Voeg een lege Genre node in op de huidige positie
        self.filters
            .filter_path
            .insert(self.filters.filter_step, FilterNode::Genre(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een genre in de huidige Genre-picker.
    pub fn select_genre(&mut self, genre: &str) {
        self.filters.selected_genre_name = Some(genre.to_string());
        if let Some(FilterNode::Genre(val)) =
            self.filters.filter_path.get_mut(self.filters.filter_step)
        {
            *val = Some(genre.to_string());
            self.filters.filter_step += 1;
            self.recompute();
            if self.filters.filter_step < self.filters.filter_path.len() {
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
        if let Some(FilterNode::Year(_)) = self.filters.filter_path.get(self.filters.filter_step) {
            self.filters.filter_path.remove(self.filters.filter_step);
            if self.filters.filter_step > self.filters.filter_path.len() {
                self.filters.filter_step = self.filters.filter_path.len();
            }
            self.recompute();
            self.populate_current_picker();
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.scroll_to_selection = true;
            return;
        }

        if self
            .filters
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Year(_)))
        {
            return;
        }

        self.filters
            .filter_path
            .insert(self.filters.filter_step, FilterNode::Year(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een jaar in de huidige Year-picker.
    pub fn select_year(&mut self, year: u32) {
        if let Some(FilterNode::Year(val)) =
            self.filters.filter_path.get_mut(self.filters.filter_step)
        {
            *val = Some(year);
            self.filters.filter_step += 1;
            self.recompute();
            if self.filters.filter_step < self.filters.filter_path.len() {
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
        if let Some(FilterNode::Composer(_)) =
            self.filters.filter_path.get(self.filters.filter_step)
        {
            self.filters.filter_path.remove(self.filters.filter_step);
            if self.filters.filter_step > self.filters.filter_path.len() {
                self.filters.filter_step = self.filters.filter_path.len();
            }
            self.recompute();
            self.populate_current_picker();
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
            self.scroll_to_selection = true;
            return;
        }

        if self
            .filters
            .filter_path
            .iter()
            .any(|n| matches!(n, FilterNode::Composer(_)))
        {
            return;
        }

        self.filters
            .filter_path
            .insert(self.filters.filter_step, FilterNode::Composer(None));
        self.recompute();
        self.populate_current_picker();
        self.current_level = NavLevel::Artist;
        self.selected_artist = 0;
        self.scroll_to_selection = true;
    }

    /// Selecteer een componist in de huidige Composer-picker.
    pub fn select_composer(&mut self, composer: &str) {
        if let Some(FilterNode::Composer(val)) =
            self.filters.filter_path.get_mut(self.filters.filter_step)
        {
            *val = Some(composer.to_string());
            self.filters.filter_step += 1;
            self.recompute();
            if self.filters.filter_step < self.filters.filter_path.len() {
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
                    flat_albums.push((album.added_timestamp, album.clone()));
                }
            }
            flat_albums.sort_by(|a, b| b.1.added_timestamp.cmp(&a.1.added_timestamp));
            flat_albums.truncate(500);
            self.filters.recent_albums = flat_albums;
            self.filters.selected_recent = 0;
        }
    }

    #[allow(dead_code)]
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
                let _ = self
                    .playback
                    .player_tx
                    .send(PlayerCommand::ReplaceQueue(queue));
            } else {
                let _ = self
                    .playback
                    .player_tx
                    .send(PlayerCommand::AppendToQueue(queue));
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
    #[allow(dead_code)]
    pub fn navigate_to_now_playing(&mut self, lib: &Library) {
        let target = match &self.playback.now_playing_path {
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
    #[allow(dead_code)]
    pub fn selected_count(&self) -> usize {
        self.selected_tracks.len()
    }
}
