use crate::config::Config;
use crate::models::{Album, Library};
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::{collect_genres, filter_by_genre};
use crate::ui::types::{BrowseMode, NavLevel, ViewMode};
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

    // Genre browsing
    pub browse_mode: BrowseMode,
    pub genres: Vec<(String, usize)>,
    pub selected_genre: usize,
    pub genre_filtered_library: Option<Library>,
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
        if let Some(lib) = &mut self.genre_filtered_library {
            sort_fn(lib);
        }

        self.selected_artist = 0;
        self.selected_album = 0;
        self.scroll_to_selection = true;
    }

    pub fn enter_genre_mode(&mut self) {
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

    pub fn enter_recent_mode(&mut self) {
        if let Some(lib) = &self.library {
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
            self.browse_mode = BrowseMode::Recent;
            self.scroll_to_selection = true;
        }
    }

    pub fn exit_browse_mode(&mut self) {
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

    pub fn select_genre(&mut self, genre: &str) {
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
}
