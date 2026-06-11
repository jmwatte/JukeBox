use crate::models::Library;
use crate::player::{PlayerCommand, PlayerEvent};
use crate::scanner::ScannerMessage;
use crate::search::filter_library;
use crate::ui::shortcuts;
use crate::ui::types::{BrowseMode, NavLevel, ViewMode};
use eframe::egui::{self, Color32, Image, Key, RichText, ScrollArea};
use std::path::Path;

use super::app::MusicPlayerApp;

impl eframe::App for MusicPlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Verwerk Now Playing events ---
        while let Ok(event) = self.player_event_rx.try_recv() {
            match event {
                PlayerEvent::NowPlaying(path) => {
                    if let Some(file_name) = Path::new(&path).file_name() {
                        self.now_playing = Some(file_name.to_string_lossy().into_owned());
                    }
                }
            }
        }

        // --- ZOEK FUNCTIE (/) ---
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.is_search_active = true;
            self.search_query.clear();
            self.filtered_library = None;
            self.current_level = NavLevel::Artist;
            self.selected_artist = 0;
        }

        self.handle_keyboard_navigation(ctx);

        // --- HELP SCHERM ---
        if self.show_help {
            let s = &self.config.shortcuts;
            egui::Window::new("Sneltoetsen & Help")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Toetsenbord Navigatie").strong());
                    ui.label(format!(
                        "• {} : Navigeer omlaag",
                        shortcuts::get_key_display(s, "NavigateDown")
                    ));
                    ui.label(format!(
                        "• {} : Navigeer omhoog",
                        shortcuts::get_key_display(s, "NavigateUp")
                    ));
                    ui.label(format!(
                        "• {} : Navigeer links (terug)",
                        shortcuts::get_key_display(s, "NavigateLeft")
                    ));
                    ui.label(format!(
                        "• {} : Navigeer rechts (in)",
                        shortcuts::get_key_display(s, "NavigateRight")
                    ));
                    ui.label(format!(
                        "• {} : Wissel weergave (Lijst / Covers)",
                        shortcuts::get_key_display(s, "ToggleView")
                    ));
                    ui.add_space(5.0);
                    ui.label(RichText::new("Muziek Besturing").strong());
                    ui.label(format!(
                        "• {} : Speel selectie af (wist wachtrij)",
                        shortcuts::get_key_display(s, "Select")
                    ));
                    ui.label(format!(
                        "• {} : Pauzeer / Hervat",
                        shortcuts::get_key_display(s, "PlayPause")
                    ));
                    ui.label(format!(
                        "• {} : Voeg selectie toe achteraan de wachtrij",
                        shortcuts::get_key_display(s, "AppendQueue")
                    ));
                    ui.label(format!(
                        "• {} : Skip naar het volgende nummer",
                        shortcuts::get_key_display(s, "Skip")
                    ));
                    ui.add_space(5.0);
                    ui.label(RichText::new("Extra").strong());
                    ui.label(format!(
                        "• {} : Selecteer een willekeurig album",
                        shortcuts::get_key_display(s, "RandomAlbum")
                    ));
                    ui.label(format!(
                        "• {} : Bladeren per genre",
                        shortcuts::get_key_display(s, "GenreBrowse")
                    ));
                    ui.label(format!(
                        "• {} : Toon nieuwste albums (Recent)",
                        shortcuts::get_key_display(s, "RecentAlbums")
                    ));
                    ui.label(format!(
                        "• {} : Sorteer op datum (Descending)",
                        shortcuts::get_key_display(s, "SortToggle")
                    ));
                    ui.label(format!(
                        "• {} : Open de map van de huidige track",
                        shortcuts::get_key_display(s, "OpenFolder")
                    ));
                    ui.label(format!(
                        "• {} : Track Details & Tags bewerken",
                        shortcuts::get_key_display(s, "TrackDetails")
                    ));
                    ui.label(format!(
                        "• {} : Markeer track voor batch edit",
                        shortcuts::get_key_display(s, "MarkTrack")
                    ));
                    ui.label(format!(
                        "• {} : Wis alle markeringen",
                        shortcuts::get_key_display(s, "ClearMarks")
                    ));
                    ui.label(format!(
                        "• {} : Forceer een rescan van de bibliotheek",
                        shortcuts::get_key_display(s, "Rescan")
                    ));
                    ui.label(format!(
                        "• {} : Herstel audio verbinding",
                        shortcuts::get_key_display(s, "ReconnectAudio")
                    ));
                    ui.label(format!(
                        "• {} : Toon / verberg dit helpvenster",
                        shortcuts::get_key_display(s, "Help")
                    ));
                    ui.separator();
                    if ui.button("Sluiten").clicked() {
                        self.show_help = false;
                    }
                });
        }

        // --- Verwerk scanner events ---
        while let Ok(msg) = self.scanner_rx.try_recv() {
            match msg {
                ScannerMessage::LibraryLoaded(lib) => {
                    self.library = Some(lib);
                    if !self.search_query.is_empty() {
                        self.filtered_library = Some(filter_library(
                            self.library.as_ref().unwrap(),
                            &self.search_query,
                        ));
                    }
                }
                ScannerMessage::Progress(text) => {
                    self._status_message = text;
                }
                ScannerMessage::ScanComplete => {
                    self._status_message = "Klaar!".to_string();
                }
            }
        }

        // --- Laadscherm tijdens scan ---
        if self.library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Bibliotheek indexeren...").size(24.0));
                        ui.add_space(10.0);
                        ui.label(RichText::new(&self._status_message).color(Color32::GRAY));
                    });
                });
            });
            ctx.request_repaint();
            return;
        }

        // --- NOW PLAYING BALK ---
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

        // --- ZOEKBALK ---
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

                    if self.is_search_active && !response.has_focus() {
                        ctx.memory_mut(|m| m.request_focus(self.search_input_id));
                    }

                    if response.changed() {
                        self.current_level = NavLevel::Artist;
                        self.selected_artist = 0;
                        self.selected_album = 0;
                        self.selected_disk = 0;
                        self.selected_track = 0;

                        let base_lib = self
                            .genre_filtered_library
                            .as_ref()
                            .or(self.library.as_ref());

                        if let Some(base_lib) = base_lib {
                            if self.search_query.trim().is_empty() {
                                self.filtered_library = None;
                            } else {
                                self.filtered_library =
                                    Some(filter_library(base_lib, &self.search_query));
                            }
                        }
                    }

                    if response.has_focus() {
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.is_search_active = false;
                            self.filtered_library = None;
                            self.search_query.clear();
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.is_search_active = false;
                            ctx.memory_mut(|m| m.surrender_focus(self.search_input_id));
                        }
                    }
                });
        }

        // --- KIES ACTIEVE LIBRARY ---
        // Prioriteit: gefilterd (search) > genre-filtered > selection-browse > volledige library
        let current_lib = self
            .filtered_library
            .as_ref()
            .or(self.genre_filtered_library.as_ref())
            .or(self.selection_library.as_ref())
            .or(self.library.as_ref());
        let Some(current_lib) = current_lib else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Bibliotheek scannen...").size(24.0));
                });
            });
            ctx.request_repaint();
            return;
        };

        // --- LEEGE BIBLIOTHEEK (geen muziek gevonden) ---
        if current_lib.artists.is_empty() && self.filtered_library.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Geen muziek gevonden")
                            .size(28.0)
                            .color(egui::Color32::YELLOW),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Stel uw muziek folder in in de config")
                            .size(16.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new(format!("Huidig pad: {}", self.config.music_directory))
                            .size(14.0)
                            .color(egui::Color32::DARK_GRAY),
                    );
                    ui.add_space(30.0);
                    ui.label(
                        egui::RichText::new("Druk op F5 om opnieuw te scannen")
                            .size(14.0)
                            .color(egui::Color32::GRAY),
                    );
                });
            });
            ctx.request_repaint();
            return;
        }

        // --- LEEGE ZOEKRESULTATEN ---
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

                    let image_bytes = include_bytes!("../../assets/no_results.png");
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

        // --- GENRE PICKER ---
        if self.browse_mode == BrowseMode::Genre && self.genre_filtered_library.is_none() {
            let genre_sel_count = self.selected_tracks.len();

            egui::CentralPanel::default().show(ctx, |ui| {
                // Consequente balk zoals in library view
                ui.horizontal(|ui| {
                    if genre_sel_count > 0 {
                        ui.label(
                            RichText::new(format!("📋 {} geselecteerd", genre_sel_count))
                                .color(Color32::LIGHT_BLUE)
                                .strong(),
                        );
                        ui.separator();
                    }
                    ui.label(RichText::new("Genres").color(Color32::GRAY));
                });
                ui.separator();

                ScrollArea::vertical().show(ui, |ui| {
                    let mut genre_to_select: Option<String> = None;

                    for (i, (genre, count)) in self.genres.iter().enumerate() {
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

                    if let Some(genre) = genre_to_select {
                        self.select_genre(&genre);
                    }
                });
            });

            self.scroll_to_selection = false;
            ctx.request_repaint();
            return;
        }

        // --- RECENT ALBUMS ---
        if self.browse_mode == BrowseMode::Recent {
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
                            if let Some(path) = &album.cover_path {
                                ui.add(
                                    Image::new(format!("file://{}", path))
                                        .max_size(egui::vec2(40.0, 40.0)),
                                );
                            } else {
                                ui.add_space(40.0);
                            }

                            let resp = ui.selectable_label(
                                selected,
                                RichText::new(format!("{} - {}", artist_name, album.title))
                                    .size(16.0),
                            );

                            if resp.clicked() {
                                self.selected_recent = i;
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

        // --- HOOFD PANEL ---
        // Kopieer velden om borrow-conflict met current_lib te voorkomen
        let has_filter = self.filtered_library.is_some();
        let browse_mode = self.browse_mode.clone();
        let selected_genre_name = self.selected_genre_name.clone();
        let search_query = self.search_query.clone();
        let is_selection_mode = self.browse_mode == BrowseMode::Selection;
        let sel_count = self.selected_tracks.len();

        let mut sa = self.selected_artist;
        let mut sal = self.selected_album;
        let mut sd = self.selected_disk;
        let mut st = self.selected_track;
        let mut cl = self.current_level.clone();
        let mut sts = self.scroll_to_selection;
        let vm = self.view_mode.clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Selectieteller
                if sel_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("📋 {} geselecteerd", sel_count))
                            .color(egui::Color32::LIGHT_BLUE)
                            .strong(),
                    );
                    ui.separator();
                }

                if has_filter {
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
                    ui.label(egui::RichText::new(&search_query).strong());
                    ui.label(
                        egui::RichText::new(" (Esc om te wissen) ")
                            .size(12.0)
                            .color(egui::Color32::GRAY),
                    );
                } else {
                    ui.label(egui::RichText::new("Bibliotheek").color(egui::Color32::GRAY));
                    if is_selection_mode {
                        ui.label(" > Selectie".to_string());
                    } else if browse_mode == BrowseMode::Genre && !selected_genre_name.is_empty() {
                        ui.label(format!(" > Genre: {}", selected_genre_name));
                    }
                }

                // Breadcrumb
                if let Some(artist) = current_lib.artists.get(sa) {
                    ui.label(format!(" > {}", artist.name));
                    if matches!(cl, NavLevel::Album | NavLevel::Disk | NavLevel::Track) {
                        if let Some(album) = artist.albums.get(sal) {
                            ui.label(format!(" > {}", album.title));
                            if matches!(cl, NavLevel::Disk | NavLevel::Track) {
                                if let Some(disk) = album.disks.get(sd) {
                                    ui.label(format!(" > {}", disk.name));
                                }
                            }
                        }
                    }
                }
            });

            ui.separator();

            // Render: album cover view of tracklist view
            match vm {
                ViewMode::AlbumCover if cl != NavLevel::Track => {
                    Self::render_cover_view_inline(
                        ui,
                        ctx,
                        current_lib,
                        &mut sa,
                        &mut sal,
                        &mut cl,
                        &mut sts,
                    );
                }
                _ => {
                    Self::render_tracklist_view_inline(
                        ui,
                        ctx,
                        current_lib,
                        &mut sa,
                        &mut sal,
                        &mut sd,
                        &mut st,
                        &mut sts,
                        &mut cl,
                        &mut self.selected_tracks,
                    );
                }
            }
        });

        // Schrijf de lokale kopieën terug naar self
        self.selected_artist = sa;
        self.selected_album = sal;
        self.selected_disk = sd;
        self.selected_track = st;
        self.current_level = cl;
        self.scroll_to_selection = sts;

        // --- TRACK DETAILS POPUP ---
        if self.show_track_details {
            self.show_track_details_popup(ctx);
        }

        self.scroll_to_selection = false;
        ctx.request_repaint();
    }
}

impl MusicPlayerApp {
    #[allow(unused_variables)]
    fn render_cover_view_inline(
        ui: &mut egui::Ui,
        _ctx: &egui::Context,
        current_lib: &Library,
        selected_artist: &mut usize,
        selected_album: &mut usize,
        current_level: &mut NavLevel,
        scroll_to_selection: &mut bool,
    ) {
        if *current_level == NavLevel::Artist {
            if current_lib.artists.is_empty() {
                return;
            }
            let albums = &current_lib.artists[*selected_artist].albums;
            let num_albums = albums.len();

            if num_albums == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label("Geen albums");
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    let available_ui_width = ui.available_width();
                    let desired_thumb = 220.0_f32;
                    let mut columns = (available_ui_width / desired_thumb).floor() as usize;
                    if columns == 0 {
                        columns = 1;
                    }
                    columns = std::cmp::min(columns, std::cmp::max(1, num_albums));
                    let padding = 12.0_f32;
                    let thumb_w = ((available_ui_width - padding * (columns as f32 + 1.0))
                        / columns as f32)
                        .max(80.0)
                        .min(600.0);
                    let thumb_size = egui::vec2(thumb_w, thumb_w);

                    if num_albums == 1 {
                        ui.centered_and_justified(|ui| {
                            if let Some(path) = &albums[0].cover_path {
                                let big_w = (available_ui_width * 0.6).max(200.0).min(800.0);
                                let resp = ui.add_sized(
                                    egui::vec2(big_w, big_w),
                                    Image::new(format!("file://{}", path))
                                        .show_loading_spinner(false)
                                        .sense(egui::Sense::click()),
                                );
                                if resp.clicked() {
                                    *current_level = NavLevel::Album;
                                    *selected_album = 0;
                                    *scroll_to_selection = true;
                                }
                            }
                            ui.add_space(6.0);
                            ui.label(RichText::new(&albums[0].title).size(20.0));
                        });
                    } else {
                        ui.columns(columns, |cols| {
                            for (i, album) in albums.iter().enumerate() {
                                let col = &mut cols[i % columns];

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
                                                *current_level = NavLevel::Album;
                                                *selected_album = i;
                                                *scroll_to_selection = true;
                                            }
                                        } else {
                                            col_ui.add_space(thumb_size.y);
                                        }

                                        col_ui.add_space(6.0);
                                        col_ui.label(
                                            RichText::new(&album.title)
                                                .size(14.0)
                                                .color(Color32::WHITE),
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
                        .get(*selected_artist)
                        .and_then(|a| a.albums.get(*selected_album))
                    {
                        if let Some(path) = &album.cover_path {
                            let available = ui.available_width();
                            let size_w = (available * 0.5).max(200.0).min(1200.0);
                            let _ = ui.add_sized(
                                egui::vec2(size_w, size_w),
                                Image::new(format!("file://{}", path)).show_loading_spinner(false),
                            );
                        }
                        ui.add_space(6.0);
                        ui.label(RichText::new(&album.title).size(20.0));
                    }
                });
            });
        }
    }

    fn render_tracklist_view_inline(
        ui: &mut egui::Ui,
        _ctx: &egui::Context,
        current_lib: &Library,
        selected_artist: &mut usize,
        selected_album: &mut usize,
        selected_disk: &mut usize,
        selected_track: &mut usize,
        scroll_to_selection: &mut bool,
        current_level: &mut NavLevel,
        selected_tracks: &mut std::collections::HashSet<String>,
    ) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.with_layout(
                egui::Layout::top_down(egui::Align::Center),
                |ui| match *current_level {
                    NavLevel::Artist => {
                        for (i, artist) in current_lib.artists.iter().enumerate() {
                            // Bepaal selectiestatus voor deze artist
                            let all_tracks: Vec<String> = artist
                                .albums
                                .iter()
                                .flat_map(|al| {
                                    al.disks
                                        .iter()
                                        .flat_map(|d| d.tracks.iter().map(|t| t.path.clone()))
                                })
                                .collect();
                            let total = all_tracks.len();
                            let sel = all_tracks
                                .iter()
                                .filter(|p| selected_tracks.contains(p.as_str()))
                                .count();
                            let prefix = if sel == 0 {
                                ""
                            } else if sel == total {
                                "☑ "
                            } else {
                                "⊡ "
                            };
                            let display = format!("{}{}", prefix, artist.name);
                            let resp = ui.selectable_label(
                                i == *selected_artist,
                                RichText::new(&display).size(18.0),
                            );
                            if resp.clicked() {
                                *selected_artist = i;
                                *scroll_to_selection = true;
                            }
                            if i == *selected_artist && *scroll_to_selection {
                                resp.scroll_to_me(None);
                            }
                        }
                    }
                    NavLevel::Album => {
                        for (i, album) in current_lib.artists[*selected_artist]
                            .albums
                            .iter()
                            .enumerate()
                        {
                            let all_tracks: Vec<String> = album
                                .disks
                                .iter()
                                .flat_map(|d| d.tracks.iter().map(|t| t.path.clone()))
                                .collect();
                            let total = all_tracks.len();
                            let sel = all_tracks
                                .iter()
                                .filter(|p| selected_tracks.contains(p.as_str()))
                                .count();
                            let prefix = if sel == 0 {
                                ""
                            } else if sel == total {
                                "☑ "
                            } else {
                                "⊡ "
                            };
                            let display = format!("{}{}", prefix, album.title);
                            let resp = ui.selectable_label(
                                i == *selected_album,
                                RichText::new(&display).size(18.0),
                            );
                            if resp.clicked() {
                                *selected_album = i;
                                *scroll_to_selection = true;
                            }
                            if i == *selected_album && *scroll_to_selection {
                                resp.scroll_to_me(None);
                            }
                        }
                    }
                    NavLevel::Disk => {
                        for (i, disk) in current_lib.artists[*selected_artist].albums
                            [*selected_album]
                            .disks
                            .iter()
                            .enumerate()
                        {
                            let all_tracks: Vec<String> =
                                disk.tracks.iter().map(|t| t.path.clone()).collect();
                            let total = all_tracks.len();
                            let sel = all_tracks
                                .iter()
                                .filter(|p| selected_tracks.contains(p.as_str()))
                                .count();
                            let prefix = if sel == 0 {
                                ""
                            } else if sel == total {
                                "☑ "
                            } else {
                                "⊡ "
                            };
                            let display = format!("{}{}", prefix, disk.name);
                            let resp = ui.selectable_label(
                                i == *selected_disk,
                                RichText::new(&display).size(16.0),
                            );
                            if resp.clicked() {
                                *selected_disk = i;
                                *scroll_to_selection = true;
                            }
                            if i == *selected_disk && *scroll_to_selection {
                                resp.scroll_to_me(None);
                            }
                        }
                    }
                    NavLevel::Track => {
                        for (i, track) in current_lib.artists[*selected_artist].albums
                            [*selected_album]
                            .disks[*selected_disk]
                            .tracks
                            .iter()
                            .enumerate()
                        {
                            let is_selected = i == *selected_track;
                            let is_marked = selected_tracks.contains(&track.path);

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
                                *selected_track = i;
                                *scroll_to_selection = true;
                            }

                            if is_selected && *scroll_to_selection {
                                resp.scroll_to_me(None);
                            }
                        }
                    }
                },
            );
        });
    }
}
