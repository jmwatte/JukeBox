use crate::player::PlayerCommand;
use crate::ui::shortcuts;
use crate::ui::types::{BrowseMode, NavLevel, ViewMode};
use eframe::egui;
use std::path::Path;

use super::app::MusicPlayerApp;

impl MusicPlayerApp {
    pub fn handle_keyboard_navigation(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }

        let cfg = &self.config.shortcuts;

        // --- ESCAPE ---
        if shortcuts::check_action(cfg, ctx, "Escape") {
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

        // --- F5: RESCAN ---
        if shortcuts::check_action(cfg, ctx, "Rescan") {
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

        // --- F6: RECONNECT AUDIO ---
        if shortcuts::check_action(cfg, ctx, "ReconnectAudio") {
            let _ = self.player_tx.send(PlayerCommand::ReconnectAudio);
            self.now_playing = Some("Audio verbinding herstellen...".to_string());
        }

        // --- HELP (? of H) ---
        if shortcuts::check_action(cfg, ctx, "Help") {
            self.show_help = !self.show_help;
        }

        // --- G: GENRE BROWSING ---
        if shortcuts::check_action(cfg, ctx, "GenreBrowse") {
            if self.browse_mode == BrowseMode::Library {
                self.enter_genre_mode();
            } else {
                self.exit_browse_mode();
            }
            return;
        }

        // --- S: SORT TOGGLE ---
        if shortcuts::check_action(cfg, ctx, "SortToggle") {
            self.toggle_sort();
            return;
        }

        // --- B: RECENT ALBUMS ---
        if shortcuts::check_action(cfg, ctx, "RecentAlbums") {
            if self.browse_mode == BrowseMode::Library {
                self.enter_recent_mode();
            } else {
                self.exit_browse_mode();
            }
            return;
        }

        // Recent albums navigation
        if self.browse_mode == BrowseMode::Recent {
            if shortcuts::check_action(cfg, ctx, "NavigateDown") {
                if self.selected_recent + 1 < self.recent_albums.len() {
                    self.selected_recent += 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(cfg, ctx, "NavigateUp") {
                if self.selected_recent > 0 {
                    self.selected_recent -= 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(cfg, ctx, "Select") {
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
        if self.browse_mode == BrowseMode::Genre && self.genre_filtered_library.is_none() {
            if shortcuts::check_action(cfg, ctx, "NavigateDown") {
                if self.selected_genre + 1 < self.genres.len() {
                    self.selected_genre += 1;
                    self.scroll_to_selection = true;
                }
            }
            if shortcuts::check_action(cfg, ctx, "NavigateUp") {
                if self.selected_genre > 0 {
                    self.selected_genre -= 1;
                    self.scroll_to_selection = true;
                }
            }
            return;
        }

        // Kies de actieve library
        let lib = self
            .filtered_library
            .as_ref()
            .or(self.genre_filtered_library.as_ref())
            .or(self.library.as_ref());
        let Some(lib) = lib else {
            return;
        };

        // --- R: RANDOM ALBUM ---
        if shortcuts::check_action(cfg, ctx, "RandomAlbum") {
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
        if shortcuts::check_action(cfg, ctx, "ToggleView") {
            self.view_mode = match self.view_mode {
                ViewMode::Tracklist => ViewMode::AlbumCover,
                ViewMode::AlbumCover => ViewMode::Tracklist,
            };
        }

        // --- PLAYBACK CONTROLS ---
        if shortcuts::check_action(cfg, ctx, "PlayPause") {
            let _ = self.player_tx.send(PlayerCommand::PlayPause);
        }
        if shortcuts::check_action(cfg, ctx, "Select") {
            self.play_selected_item(lib, true);
        }
        if shortcuts::check_action(cfg, ctx, "AppendQueue") {
            self.play_selected_item(lib, false);
        }
        if shortcuts::check_action(cfg, ctx, "Skip") {
            let _ = self.player_tx.send(PlayerCommand::Skip);
        }

        // --- O: OPEN FOLDER ---
        if shortcuts::check_action(cfg, ctx, "OpenFolder") {
            if let Some(track_path) = self.get_current_track_path(lib) {
                if let Some(parent) = Path::new(&track_path).parent() {
                    let _ = std::process::Command::new("explorer").arg(parent).spawn();
                }
            }
        }

        // --- I: TRACK DETAILS ---
        if shortcuts::check_action(cfg, ctx, "TrackDetails") {
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

                let active_lib = self
                    .genre_filtered_library
                    .as_ref()
                    .or(self.filtered_library.as_ref())
                    .or(self.library.as_ref());

                if let Some(lib) = active_lib {
                    if self.selected_tracks.is_empty() {
                        if let Some(track_path) = self.get_current_track_path(lib) {
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

        // --- M: MARK TRACK ---
        if shortcuts::check_action(cfg, ctx, "MarkTrack") {
            if self.current_level == NavLevel::Track {
                if let Some(track_path) = self.get_current_track_path(lib) {
                    if self.selected_tracks.contains(&track_path) {
                        self.selected_tracks.remove(&track_path);
                    } else {
                        self.selected_tracks.insert(track_path);
                    }
                }
            }
        }
        // --- SHIFT+M: CLEAR MARKS ---
        if shortcuts::check_action(cfg, ctx, "ClearMarks") {
            self.selected_tracks.clear();
            self.tracks_to_edit.clear();
        }

        // --- ARROW NAVIGATION ---
        if shortcuts::check_action(cfg, ctx, "NavigateDown") {
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
        if shortcuts::check_action(cfg, ctx, "NavigateUp") {
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
        if shortcuts::check_action(cfg, ctx, "NavigateRight") {
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
        if shortcuts::check_action(cfg, ctx, "NavigateLeft") {
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
