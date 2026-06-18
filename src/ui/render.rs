use crate::models::Library;
use crate::player::{PlayerCommand, PlayerEvent, RepeatMode};
use crate::scanner::ScannerMessage;
use crate::search::filter_library;
use crate::ui::shortcuts;
use crate::ui::types::{FilterNode, NavLevel, ViewMode};
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
                    self.now_playing_path = Some(path);
                    self.now_playing_position = 0.0;
                    self.status_error = None; // Wis foutmelding bij nieuwe track
                }
                PlayerEvent::PositionUpdate(pos, dur) => {
                    self.now_playing_position = pos;
                    self.now_playing_duration = dur;
                }
                PlayerEvent::RepeatModeChanged(mode) => {
                    self.repeat_mode = mode;
                }
                PlayerEvent::ShuffleModeChanged(on) => {
                    self.shuffle_on = on;
                }
                PlayerEvent::QueueChanged(queue) => {
                    self.queue = queue;
                }
                PlayerEvent::LoopChanged(a, b) => {
                    self.loop_a = a;
                    self.loop_b = b;
                }
                PlayerEvent::PlaybackError(msg) => {
                    self.status_error = Some(msg);
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
        // --- TRACK DETAILS POPUP ---
        // if self.show_track_details {
        //     // self.show_track_details_popup(ctx);
        // }

        // Optioneel: Maak een inschuifbaar zijpaneel dat alleen verschijnt als er iets geselecteerd is
        if !self.tracks_to_edit.is_empty() {
            egui::SidePanel::right("batch_edit_panel")
                .default_width(350.0) // Breedte van het paneel
                .resizable(true)
                .show(ctx, |ui| {
                    // Hier roepen we de nieuwe functie aan!
                    self.show_batch_edit_panel(ui);
                });
        }

        // --- WACHTRIJ PANEEL ---
        if self.show_queue {
            let tx = self.player_tx.clone();
            egui::SidePanel::right("queue_panel")
                .default_width(300.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Wachtrij");
                    ui.separator();

                    // Huidig nummer
                    if let Some(ref track) = self.now_playing {
                        ui.label(
                            RichText::new(format!("▶ {}", track))
                                .size(14.0)
                                .color(Color32::from_rgb(100, 200, 100))
                                .strong(),
                        );
                        ui.add_space(4.0);
                    }

                    // Overige tracks in queue
                    if self.queue.is_empty() {
                        ui.label("Geen nummers in wachtrij.");
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(ui.available_height() - 40.0)
                            .show(ui, |ui| {
                                for (i, path) in self.queue.iter().enumerate() {
                                    let file_name = std::path::Path::new(path)
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| path.clone());

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:02}.", i + 1))
                                                .size(12.0)
                                                .color(Color32::GRAY),
                                        );
                                        ui.label(RichText::new(&file_name).size(13.0));
                                        if ui.button("❌").clicked() {
                                            let _ = tx.send(PlayerCommand::RemoveFromQueue(i));
                                        }
                                    });
                                }
                            });

                        ui.add_space(8.0);
                        if ui.button("Wis wachtrij").clicked() {
                            let _ = tx.send(PlayerCommand::ClearQueue);
                        }
                    }
                });
        }

        // --- HELP SCHERM ---
        let mut reset_shortcuts = false;
        if self.show_help || self.force_help {
            let s = &self.config.shortcuts;
            egui::Window::new("Sneltoetsen & Help")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    // Configuratie fouten (indien aanwezig)
                    if !self.config_errors.is_empty() {
                        ui.label(
                            RichText::new("⚠ Configuratieproblemen")
                                .size(16.0)
                                .color(Color32::from_rgb(255, 100, 100))
                                .strong(),
                        );
                        for err in &self.config_errors {
                            ui.label(
                                RichText::new(format!("  • {}", err))
                                    .size(13.0)
                                    .color(Color32::from_rgb(255, 150, 150)),
                            );
                        }
                        if ui.button("Herstel foutieve shortcuts").clicked() {
                            reset_shortcuts = true;
                        }
                        if ui.button("Negeer").clicked() {
                            self.force_help = false;
                        }
                        ui.separator();
                    }
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
                    ui.label(format!(
                        "• {} : Spoel 2 seconden terug",
                        shortcuts::get_key_display(s, "Rewind")
                    ));
                    ui.label(format!(
                        "• {} : Spoel 2 seconden vooruit",
                        shortcuts::get_key_display(s, "Forward")
                    ));
                    ui.label(format!(
                        "• {} : Herhaalmodus (Uit / 1 / Alles)",
                        shortcuts::get_key_display(s, "RepeatToggle")
                    ));
                    ui.label(format!(
                        "• {} : Shuffle aan / uit",
                        shortcuts::get_key_display(s, "ShuffleToggle")
                    ));
                    ui.label(format!(
                        "• {} : Volume omhoog",
                        shortcuts::get_key_display(s, "VolumeUp")
                    ));
                    ui.label(format!(
                        "• {} : Volume omlaag",
                        shortcuts::get_key_display(s, "VolumeDown")
                    ));
                    ui.label(format!(
                        "• {} : Zet loop-punt A",
                        shortcuts::get_key_display(s, "LoopA")
                    ));
                    ui.label(format!(
                        "• {} : Zet loop-punt B",
                        shortcuts::get_key_display(s, "LoopB")
                    ));
                    ui.label(format!(
                        "• {} : Wis A-B loop",
                        shortcuts::get_key_display(s, "ClearLoop")
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
                        "• {} : Browse selectie",
                        shortcuts::get_key_display(s, "SelectionBrowse")
                    ));
                    ui.label(format!(
                        "• {} : Forceer een rescan van de bibliotheek",
                        shortcuts::get_key_display(s, "Rescan")
                    ));
                    ui.label(format!(
                        "• {} : Rescan alleen gemarkeerde tracks",
                        shortcuts::get_key_display(s, "RescanMarked")
                    ));
                    ui.label(format!(
                        "• {} : Compacte modus (alleen speler)",
                        shortcuts::get_key_display(s, "CompactToggle")
                    ));
                    ui.label(format!(
                        "• {} : Herstel audio verbinding",
                        shortcuts::get_key_display(s, "ReconnectAudio")
                    ));
                    ui.label(format!(
                        "• {} : Toon / verberg dit helpvenster",
                        shortcuts::get_key_display(s, "Help")
                    ));
                    ui.label(format!(
                        "• {} : Toon / verberg wachtrij",
                        shortcuts::get_key_display(s, "QueueToggle")
                    ));
                    ui.label(format!(
                        "• {} : Navigeer naar huidig nummer",
                        shortcuts::get_key_display(s, "NowPlaying")
                    ));
                    ui.separator();
                    if ui.button("Sluiten").clicked() {
                        self.show_help = false;
                        self.force_help = false;
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new("Bladeren op metadata").strong());
                    ui.label(format!(
                        "• {} : Bladeren op jaartal",
                        shortcuts::get_key_display(s, "YearBrowse")
                    ));
                    ui.label(format!(
                        "• {} : Bladeren op componist",
                        shortcuts::get_key_display(s, "ComposerBrowse")
                    ));
                });
        }

        // Reset shortcuts indien gevraagd (alleen foutieve, behoud custom)
        if reset_shortcuts {
            crate::ui::shortcuts::repair_shortcuts(&mut self.config.shortcuts, &self.config_errors);
            if let Ok(toml_str) = toml::to_string(&self.config) {
                let _ = std::fs::write("config.toml", toml_str);
            }
            self.config_errors.clear();
            self.force_help = false;
        }

        // --- Verwerk scanner events ---
        while let Ok(msg) = self.scanner_rx.try_recv() {
            match msg {
                ScannerMessage::LibraryLoaded(lib) => {
                    self.library = Some(lib);
                    self.recompute();
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
        if self.now_playing.is_some() || self.status_error.is_some() {
            egui::TopBottomPanel::bottom("now_playing_panel").show(ctx, |ui| {
                ui.add_space(6.0);

                // Track info
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("🎵")
                            .color(Color32::from_rgb(100, 200, 100))
                            .size(18.0),
                    );
                    if let Some(track) = &self.now_playing {
                        ui.label(RichText::new(track).size(16.0).strong());
                    }
                });

                ui.add_space(4.0);

                // Foutmelding (rood, tijdelijk)
                if let Some(ref err) = self.status_error {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("⚠ {}", err))
                                .size(13.0)
                                .color(Color32::from_rgb(255, 100, 100)),
                        );
                    });
                    ui.add_space(2.0);
                }

                // Voortgangsbalk + tijd
                if self.now_playing_duration > 0.0 {
                    let pos_mins = (self.now_playing_position / 60.0) as u32;
                    let pos_secs = self.now_playing_position as u32 % 60;
                    let dur_mins = (self.now_playing_duration / 60.0) as u32;
                    let dur_secs = self.now_playing_duration as u32 % 60;
                    let time_text = format!(
                        "{}:{:02}  /  {}:{:02}",
                        pos_mins, pos_secs, dur_mins, dur_secs
                    );

                    ui.horizontal(|ui| {
                        let fraction =
                            (self.now_playing_position / self.now_playing_duration).clamp(0.0, 1.0);
                        let bar = egui::ProgressBar::new(fraction)
                            .show_percentage()
                            .desired_width(ui.available_width() - 180.0);
                        ui.add(bar);
                        ui.label(RichText::new(time_text).size(12.0).color(Color32::GRAY));

                        // Volume indicator
                        let vol_percent = (self.volume * 100.0) as u32;
                        ui.label(
                            RichText::new(format!("🔊 {}%", vol_percent))
                                .size(12.0)
                                .color(Color32::GRAY),
                        );

                        // Herhaalmodus indicator
                        let repeat_text = match self.repeat_mode {
                            RepeatMode::None => "",
                            RepeatMode::One => "🔂 1",
                            RepeatMode::All => "🔁 All",
                        };
                        if !repeat_text.is_empty() {
                            ui.label(
                                RichText::new(repeat_text)
                                    .size(12.0)
                                    .color(Color32::from_rgb(100, 200, 100)),
                            );
                        }

                        // Shuffle indicator
                        if self.shuffle_on {
                            ui.label(
                                RichText::new("🔀")
                                    .size(12.0)
                                    .color(Color32::from_rgb(100, 200, 100)),
                            );
                        }

                        // A-B loop indicator
                        if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
                            let a_mins = (a / 60.0) as u32;
                            let a_secs = a as u32 % 60;
                            let b_mins = (b / 60.0) as u32;
                            let b_secs = b as u32 % 60;
                            ui.label(
                                RichText::new(format!(
                                    "🔁 [{:02}:{:02} → {:02}:{:02}]",
                                    a_mins, a_secs, b_mins, b_secs
                                ))
                                .size(12.0)
                                .color(Color32::from_rgb(255, 200, 100)),
                            );
                        }
                    });
                }

                ui.add_space(6.0);
            });
        }

        // --- WAVEFORM EDITOR WINDOW ---
        if self.show_waveform {
            let waveform_path = self.waveform_state.path.clone();
            let player_position = if self.now_playing_path.as_deref() == waveform_path.as_deref() {
                Some(self.now_playing_position)
            } else {
                None
            };

            egui::Window::new("🌊 Waveform Editor")
                .id(egui::Id::new("waveform_window"))
                .collapsible(false)
                .resizable(true)
                .default_size([800.0, 350.0])
                .show(ctx, |ui| {
                    // Bestandsinfo
                    if let Some(ref path) = self.waveform_state.path {
                        let file_name = std::path::Path::new(path)
                            .file_name()
                            .map(|n| n.to_string_lossy())
                            .unwrap_or_else(|| std::borrow::Cow::from(path));
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("📄 {}", file_name))
                                    .size(14.0)
                                    .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:.1}s  |  {} Hz  |  Zoom: {}x",
                                            self.waveform_state.duration_secs,
                                            self.waveform_state.sample_rate,
                                            (self.waveform_state.zoom / 50.0 * 100.0) as u32
                                        ))
                                        .size(11.0)
                                        .color(egui::Color32::GRAY),
                                    );
                                },
                            );
                        });
                        ui.separator();
                    }

                    // Foutmelding
                    if let Some(ref err) = self.waveform_state.error {
                        ui.label(
                            egui::RichText::new(format!("⚠ {}", err))
                                .size(13.0)
                                .color(egui::Color32::from_rgb(255, 100, 100)),
                        );
                    }

                    // Waveform
                    crate::waveform::render_waveform(ui, &mut self.waveform_state, player_position);

                    ui.separator();

                    // Zoom / scroll knoppen
                    ui.horizontal(|ui| {
                        if ui.button("🔍−").clicked() {
                            self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
                        }
                        if ui.button("🔍+").clicked() {
                            self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
                        }
                        ui.separator();
                        if ui.button("⟲ Reset zoom/scroll").clicked() {
                            self.waveform_state.zoom = 50.0;
                            self.waveform_state.scroll_offset = 0.0;
                        }

                        // Sluit-knop rechts
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Sluit (0)").clicked() {
                                self.show_waveform = false;
                            }
                        });
                    });
                });
        }

        // Compacte modus: verberg bibliotheek, alleen now-playing balk
        if self.compact_mode {
            ctx.request_repaint();
            return;
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

                        // Search binnen de actieve gefilterde set
                        // let base_lib = self
                        //     .filtered_library
                        //     .as_ref()
                        //     .or(self.cached_filtered.as_ref())
                        //     .or(self.library.as_ref());
                        //
                        let base_lib = self.cached_filtered.as_ref().or(self.library.as_ref());

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

        // --- KIES ACTIEVE LIBRARY (disjoint borrow, geen clone!) ---
        let current_lib = self
            .filtered_library
            .as_ref()
            .or(self.cached_filtered.as_ref())
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

        // --- LEEGE BIBLIOTHEEK ---
        if current_lib.artists.is_empty()
            && self.filtered_library.is_none()
            && !self.is_picker_active()
        {
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
            let search_q = self.search_query.clone();
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
                        egui::RichText::new(format!("voor: \"{}\"", search_q))
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

        // --- PICKER VIEWS ---

        if self.is_picker_active() {
            if let Some(node) = self.filter_path.get(self.filter_step) {
                // Genre picker
                if let FilterNode::Genre(_) = node {
                    let genre_sel_count = self.selected_tracks.len();
                    egui::CentralPanel::default().show(ctx, |ui| {
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
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let selected = i == self.selected_genre;
                                            let resp = ui.selectable_label(
                                                selected,
                                                RichText::new(format!("{} ({})", genre, count))
                                                    .size(16.0),
                                            );
                                            if resp.clicked() {
                                                genre_to_select = Some(genre.clone());
                                            }
                                            if selected
                                                && (ctx.input(|i| {
                                                    i.key_pressed(Key::Enter)
                                                        || i.key_pressed(Key::ArrowRight)
                                                }))
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

                // Year picker
                if let FilterNode::Year(_) = node {
                    let y_sel_count = self.selected_tracks.len();
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if y_sel_count > 0 {
                                ui.label(
                                    RichText::new(format!("📋 {} geselecteerd", y_sel_count))
                                        .color(Color32::LIGHT_BLUE)
                                        .strong(),
                                );
                                ui.separator();
                            }
                            ui.label(RichText::new("Jaartallen").color(Color32::GRAY));
                        });
                        ui.separator();
                        ScrollArea::vertical().show(ui, |ui| {
                            let mut year_to_select: Option<u32> = None;
                            for (i, (year_opt, count)) in self.years.iter().enumerate() {
                                let label = match year_opt {
                                    Some(y) => format!("{} ({})", y, count),
                                    None => format!("Onbekend ({})", count),
                                };
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let selected = i == self.selected_year;
                                            let resp = ui.selectable_label(
                                                selected,
                                                RichText::new(label).size(16.0),
                                            );
                                            if resp.clicked() {
                                                year_to_select = Some(year_opt.unwrap_or(0));
                                            }
                                            if selected
                                                && (ctx.input(|i| {
                                                    i.key_pressed(Key::Enter)
                                                        || i.key_pressed(Key::ArrowRight)
                                                }))
                                            {
                                                year_to_select = Some(year_opt.unwrap_or(0));
                                            }
                                            if selected && self.scroll_to_selection {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                            if let Some(year) = year_to_select {
                                self.select_year(year);
                            }
                        });
                    });
                    self.scroll_to_selection = false;
                    ctx.request_repaint();
                    return;
                }

                // Composer picker
                if let FilterNode::Composer(_) = node {
                    let c_sel_count = self.selected_tracks.len();
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if c_sel_count > 0 {
                                ui.label(
                                    RichText::new(format!("📋 {} geselecteerd", c_sel_count))
                                        .color(Color32::LIGHT_BLUE)
                                        .strong(),
                                );
                                ui.separator();
                            }
                            ui.label(RichText::new("Componisten").color(Color32::GRAY));
                        });
                        ui.separator();
                        ScrollArea::vertical().show(ui, |ui| {
                            let mut composer_to_select: Option<String> = None;
                            for (i, (name, count)) in self.composers.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::centered_and_justified(
                                            egui::Direction::TopDown,
                                        ),
                                        |ui| {
                                            let selected = i == self.selected_composer;
                                            let resp = ui.selectable_label(
                                                selected,
                                                RichText::new(format!("{} ({})", name, count))
                                                    .size(16.0),
                                            );
                                            if resp.clicked() {
                                                composer_to_select = Some(name.clone());
                                            }
                                            if selected
                                                && (ctx.input(|i| {
                                                    i.key_pressed(Key::Enter)
                                                        || i.key_pressed(Key::ArrowRight)
                                                }))
                                            {
                                                composer_to_select = Some(name.clone());
                                            }
                                            if selected && self.scroll_to_selection {
                                                resp.scroll_to_me(None);
                                            }
                                        },
                                    );
                                });
                            }
                            if let Some(name) = composer_to_select {
                                self.select_composer(&name);
                            }
                        });
                    });
                    self.scroll_to_selection = false;
                    ctx.request_repaint();
                    return;
                }
            }
        }
        let has_filter = self.filtered_library.is_some();
        let search_query = self.search_query.clone();
        let sel_count = self.selected_tracks.len();
        let breadcrumb = self.breadcrumb();

        let mut sa = self.selected_artist;
        let mut sal = self.selected_album;
        let mut sd = self.selected_disk;
        let mut st = self.selected_track;
        let mut cl = self.current_level.clone();
        let mut sts = self.scroll_to_selection;
        let vm = self.view_mode.clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if sel_count > 0 {
                    ui.label(
                        RichText::new(format!("📋 {} geselecteerd", sel_count))
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
                    ui.label(egui::RichText::new(&breadcrumb).color(egui::Color32::GRAY));
                }
                // Breadcrumb artist/album
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

        self.selected_artist = sa;
        self.selected_album = sal;
        self.selected_disk = sd;
        self.selected_track = st;
        self.current_level = cl;
        self.scroll_to_selection = sts;

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
                    let padding = 16.0_f32; // Ietsje ruimere padding toont mooier bij grotere hoezen

                    // 1. Bepaal het aantal kolommen dynamisch
                    let mut columns = if num_albums <= 4 {
                        // Bij 2, 3 of 4 albums: forceer ze op 1 rij zodat ze de breedte perfect opvullen!
                        num_albums
                    } else {
                        // Basis verhoogd naar 320px. Dit voorkomt een eindeloze horizontale
                        // rij van kleine hoesjes en dwingt egui netjes naar een 2e of 3e rij.
                        let desired_thumb = 320.0_f32;
                        (available_ui_width / desired_thumb).floor() as usize
                    };

                    // Veiligheidscheck
                    if columns == 0 {
                        columns = 1;
                    }
                    columns = std::cmp::min(columns, num_albums);

                    // 2. Anti-"weeskind" logica
                    // Voorkomt dat er 1 eenzaam hoesje op de laatste rij valt.
                    // Bijv: 9 albums verdeeld over 4 kolommen = 4 + 4 + 1.
                    // Door kolommen met 1 te verlagen wordt dit een perfect 3x3 grid!
                    if columns > 2 && num_albums > columns {
                        if num_albums % columns == 1 {
                            columns -= 1;
                        }
                    }

                    // 3. Bereken de uiteindelijke breedte per hoes
                    let thumb_w = ((available_ui_width - padding * (columns as f32 + 1.0))
                        / columns as f32)
                        .max(150.0)
                        .min(800.0); // Maximum flink verhoogd zodat ze de beschikbare real estate echt mogen pakken

                    let thumb_size = egui::vec2(thumb_w, thumb_w);
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
                            let total: usize = artist
                                .albums
                                .iter()
                                .flat_map(|al| al.disks.iter().map(|d| d.tracks.len()))
                                .sum();
                            let sel: usize = artist
                                .albums
                                .iter()
                                .flat_map(|al| al.disks.iter().flat_map(|d| d.tracks.iter()))
                                .filter(|t| selected_tracks.contains(&t.path))
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
                            let total: usize = album.disks.iter().map(|d| d.tracks.len()).sum();
                            let sel: usize = album
                                .disks
                                .iter()
                                .flat_map(|d| d.tracks.iter())
                                .filter(|t| selected_tracks.contains(&t.path))
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
                            let total = disk.tracks.len();
                            let sel = disk
                                .tracks
                                .iter()
                                .filter(|t| selected_tracks.contains(&t.path))
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
                        let row_height = ui.spacing().interact_size.y;
                        let full_width = ui.available_width();

                        for (i, track) in current_lib.artists[*selected_artist].albums
                            [*selected_album]
                            .disks[*selected_disk]
                            .tracks
                            .iter()
                            .enumerate()
                        {
                            let is_selected = i == *selected_track;
                            let is_marked = selected_tracks.contains(&track.path);

                            // Bouw de labeltekst met tracknummer, titel en duur
                            let track_num_str = if track.track_number > 0 {
                                format!("{:02}.  ", track.track_number)
                            } else {
                                String::new()
                            };
                            let mark_str = if is_marked { "☑ " } else { "" };
                            let dur_str = if track.duration_secs > 0 {
                                let mins = track.duration_secs / 60;
                                let secs = track.duration_secs % 60;
                                format!("   {}:{:02}", mins, secs)
                            } else {
                                String::new()
                            };
                            let text =
                                format!("{}{}{}{}", track_num_str, mark_str, track.title, dur_str);

                            let label = egui::SelectableLabel::new(
                                is_selected,
                                RichText::new(&text).size(16.0),
                            );
                            let resp = ui.add_sized(egui::vec2(full_width, row_height), label);

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
