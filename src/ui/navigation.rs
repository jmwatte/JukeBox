use crate::player::PlayerCommand;
use crate::ui::shortcuts;
use crate::ui::types::{Layer, NavLevel, ViewMode};
use eframe::egui;
use std::path::Path;

use super::app::MusicPlayerApp;

impl MusicPlayerApp {
    pub fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let cfg = self.config.shortcuts.clone();

        // --- ESCAPE ---
        if shortcuts::check_action(&cfg, ctx, "Escape") {
            self.selected_tracks.clear();

            // 1. Eerst picker wegpoppen (bv. GenrePicker, RecentAlbums)
            if self.is_picker_active() {
                self.pop_layer();
                return;
            }

            // 2. Dan search wissen
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

            // 3. Dan filterlagen poppen
            if !self.filter_stack.is_empty() {
                self.pop_layer();
                return;
            }
        }

        // --- F5: RESCAN ---
        if shortcuts::check_action(&cfg, ctx, "Rescan") {
            let _ = std::fs::remove_file("library_cache.bin");
            self.library = None;
            self.filtered_library = None;
            self.cached_filtered = None;
            self.filter_stack.clear();
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

        // --- F6: RECONNECT AUDIO ---
        if shortcuts::check_action(&cfg, ctx, "ReconnectAudio") {
            let _ = self.player_tx.send(PlayerCommand::ReconnectAudio);
            self.now_playing = Some("Audio verbinding herstellen...".to_string());
        }

        // --- HELP ---
        if shortcuts::check_action(&cfg, ctx, "Help") {
            self.show_help = !self.show_help;
        }

        // --- G: GENRE PICKER ---
        if shortcuts::check_action(&cfg, ctx, "GenreBrowse") {
            if self.is_picker_active() && self.filter_stack.last() == Some(&Layer::GenrePicker) {
                // Al in genre picker, ga terug
                self.pop_layer();
            } else {
                self.enter_genre_picker();
            }
            return;
        }

        // --- S: SORT TOGGLE ---
        if shortcuts::check_action(&cfg, ctx, "SortToggle") {
            self.toggle_sort();
            return;
        }

        // --- B: RECENT ALBUMS ---
        if shortcuts::check_action(&cfg, ctx, "RecentAlbums") {
            if self.filter_stack.last() == Some(&Layer::RecentAlbums) {
                self.pop_layer();
            } else {
                self.enter_recent_mode();
            }
            return;
        }

        // --- Z: SELECTION BROWSE ---
        if shortcuts::check_action(&cfg, ctx, "SelectionBrowse") {
            if self.filter_stack.last() == Some(&Layer::Selection) {
                self.pop_layer();
            } else if !self.selected_tracks.is_empty() {
                self.enter_selection_mode();
            }
            return;
        }

        // Recent albums navigation
        if self.filter_stack.last() == Some(&Layer::RecentAlbums) {
            if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                if self.selected_recent + 1 < self.recent_albums.len() {
                    self.selected_recent += 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                if self.selected_recent > 0 {
                    self.selected_recent -= 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(&cfg, ctx, "Select") {
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
            return;
        }

        // Genre picker navigation
        if self.filter_stack.last() == Some(&Layer::GenrePicker) {
            if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
                if self.selected_genre + 1 < self.genres.len() {
                    self.selected_genre += 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
                if self.selected_genre > 0 {
                    self.selected_genre -= 1;
                    self.scroll_to_selection = true;
                }
            }
            // M op genre: alle tracks van dit genre markeren
            if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
                if let Some((genre_name, _)) = self.genres.get(self.selected_genre) {
                    let base_lib = self.library_before_top_picker();
                    if let Some(lib) = base_lib {
                        let genre_lib = crate::search::filter_by_genre(&lib, genre_name);
                        let paths: Vec<String> = genre_lib
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
            return;
        }

        // Kies de actieve library voor navigatie
        let Some(lib) = self.active_library().cloned() else {
            return;
        };

        // --- R: RANDOM ALBUM ---
        if shortcuts::check_action(&cfg, ctx, "RandomAlbum") {
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

        // --- T: TOGGLE VIEW ---
        if shortcuts::check_action(&cfg, ctx, "ToggleView") {
            self.view_mode = match self.view_mode {
                ViewMode::Tracklist => ViewMode::AlbumCover,
                ViewMode::AlbumCover => ViewMode::Tracklist,
            };
        }

        // --- PLAYBACK CONTROLS ---
        if shortcuts::check_action(&cfg, ctx, "PlayPause") {
            let _ = self.player_tx.send(PlayerCommand::PlayPause);
        }
        if shortcuts::check_action(&cfg, ctx, "Select") {
            self.play_selected_item(&lib, true);
        }
        if shortcuts::check_action(&cfg, ctx, "AppendQueue") {
            self.play_selected_item(&lib, false);
        }
        if shortcuts::check_action(&cfg, ctx, "Skip") {
            let _ = self.player_tx.send(PlayerCommand::Skip);
        }

        // --- O: OPEN FOLDER ---
        if shortcuts::check_action(&cfg, ctx, "OpenFolder") {
            if let Some(track_path) = self.get_current_track_path(&lib) {
                if let Some(parent) = Path::new(&track_path).parent() {
                    let _ = std::process::Command::new("explorer").arg(parent).spawn();
                }
            }
        }

        // --- I: TRACK DETAILS ---
        if shortcuts::check_action(&cfg, ctx, "TrackDetails") {
            let has_selection = !self.selected_tracks.is_empty();
            if has_selection || self.current_level == NavLevel::Track {
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

                if self.selected_tracks.is_empty() {
                    if let Some(track_path) = self.get_current_track_path(&lib) {
                        self.tracks_to_edit = vec![track_path];
                    }
                } else {
                    self.tracks_to_edit = self.selected_tracks.iter().cloned().collect();
                    self.tracks_to_edit.sort();
                }

                if let Some(first_path) = self.tracks_to_edit.first() {
                    self.editing_track_path = Some(first_path.clone());

                    match Probe::open(first_path).and_then(|p| p.read()) {
                        Ok(tagged_file) => {
                            let mut raw_text = String::new();
                            for tag in tagged_file.tags() {
                                raw_text
                                    .push_str(&format!("--- Tag Type: {:?} ---\n", tag.tag_type()));
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
                self.show_track_details = true;
            }
        }

        // --- M: MARK / UNMARK OP HUIDIG NIVEAU ---
        if shortcuts::check_action(&cfg, ctx, "MarkTrack") {
            let tracks = {
                let act_lib = self.active_library().cloned();
                act_lib
                    .map(|l| self.get_tracks_at_level(&l, &self.current_level))
                    .unwrap_or_default()
            };
            if !tracks.is_empty() {
                let all_selected = tracks
                    .iter()
                    .all(|p| self.selected_tracks.contains(p.as_str()));
                if all_selected {
                    for p in &tracks {
                        self.selected_tracks.remove(p);
                    }
                } else {
                    for p in &tracks {
                        self.selected_tracks.insert(p.clone());
                    }
                }
            }
        }
        // --- SHIFT+M: CLEAR MARKS ---
        if shortcuts::check_action(&cfg, ctx, "ClearMarks") {
            self.selected_tracks.clear();
            self.tracks_to_edit.clear();
        }

        // --- ARROW NAVIGATION ---
        if shortcuts::check_action(&cfg, ctx, "NavigateDown") {
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
        if shortcuts::check_action(&cfg, ctx, "NavigateUp") {
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
        if shortcuts::check_action(&cfg, ctx, "NavigateRight") {
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
        if shortcuts::check_action(&cfg, ctx, "NavigateLeft") {
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
                // Op Artist-niveau: ga terug naar de picker van het bovenste filter
                NavLevel::Artist => {
                    // Zoek de bovenste niet-picker layer en push de bijbehorende picker
                    if let Some(top_filter) =
                        self.filter_stack.iter().rev().find(|l| !l.is_picker())
                    {
                        match top_filter {
                            Layer::Genre(_) => self.enter_genre_picker(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
