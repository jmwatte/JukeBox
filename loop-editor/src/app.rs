use crate::loops::SavedLoop;
use crate::waveform::{render_waveform, WaveformState};
use crate::waveform_player::{start_waveform_thread, WaveformCommand, WaveformEvent};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, RichText};
use std::path::Path;

pub struct LoopEditorApp {
    // Waveform state
    pub waveform_state: WaveformState,
    pub waveform_cmd_tx: Sender<WaveformCommand>,
    pub waveform_event_rx: Receiver<WaveformEvent>,
    pub waveform_is_playing: bool,
    pub waveform_play_position: f32,
    pub waveform_play_duration: f32,

    // Loop library
    pub saved_loops: Vec<SavedLoop>,
    pub show_loop_library: bool,

    // File path input
    pub file_path: String,
    pub status_message: String,
}

impl LoopEditorApp {
    pub fn new() -> Self {
        let (waveform_cmd_tx, waveform_event_rx) = start_waveform_thread();
        let saved_loops = crate::loops::load_loops();

        Self {
            waveform_state: WaveformState::default(),
            waveform_cmd_tx,
            waveform_event_rx,
            waveform_is_playing: false,
            waveform_play_position: 0.0,
            waveform_play_duration: 0.0,
            saved_loops,
            show_loop_library: false,
            file_path: String::new(),
            status_message: String::new(),
        }
    }

    pub fn load_file(&mut self, path: &str) {
        match crate::waveform::decode_audio(path) {
            Ok((samples, sample_rate, duration_secs)) => {
                self.waveform_state.path = Some(path.to_string());
                self.waveform_state.samples = samples;
                self.waveform_state.sample_rate = sample_rate;
                self.waveform_state.duration_secs = duration_secs;
                self.waveform_state.zoom = 50.0;
                self.waveform_state.scroll_offset = 0.0;
                self.waveform_state.loop_a_secs = None;
                self.waveform_state.loop_b_secs = None;
                self.waveform_state.error = None;
                self.waveform_play_position = 0.0;
                self.waveform_play_duration = duration_secs;
                self.status_message = format!(
                    "Geladen: {} ({:.1}s, {} Hz)",
                    Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default(),
                    duration_secs,
                    sample_rate,
                );
            }
            Err(e) => {
                self.waveform_state.error = Some(e.clone());
                self.status_message = format!("Fout bij laden: {}", e);
            }
        }
    }
}

impl eframe::App for LoopEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Verwerk waveform audio events ──
        while let Ok(event) = self.waveform_event_rx.try_recv() {
            match event {
                WaveformEvent::Playing => {
                    self.waveform_is_playing = true;
                    ctx.request_repaint();
                }
                WaveformEvent::Stopped => {
                    self.waveform_is_playing = false;
                    self.waveform_play_position = 0.0;
                    ctx.request_repaint();
                }
                WaveformEvent::Error(msg) => {
                    self.waveform_is_playing = false;
                    self.status_message = format!("Waveform fout: {}", msg);
                    ctx.request_repaint();
                }
                WaveformEvent::Position(pos, dur) => {
                    self.waveform_play_position = pos;
                    self.waveform_play_duration = dur;
                    ctx.request_repaint();
                }
            }
        }

        // 🔥 CRITICAL: Force continuous repaints while playing so the playhead moves smoothly
        if self.waveform_is_playing {
            ctx.request_repaint();
        }

        // ── Keyboard Shortcuts ──
        let is_text_focused = ctx.memory(|mem| mem.focused().is_some());
        if !is_text_focused && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if self.waveform_is_playing {
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
            } else if let Some(ref path) = self.waveform_state.path {
                // Start playing from current position or loop start
                let (start, end) = match (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    (Some(a), Some(b)) if b > a => (a, b),
                    _ => (
                        self.waveform_play_position,
                        self.waveform_state.duration_secs,
                    ),
                };
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                    path: path.clone(),
                    start_sec: start,
                    end_sec: end,
                    pitch_semitones: self.waveform_state.pitch_semitones,
                    tempo: self.waveform_state.tempo,
                });
            }
        }

        // ── Drag & drop bestanden ──
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            if let Some(path) = dropped
                .first()
                .and_then(|f| f.path.as_ref())
                .and_then(|p| p.to_str())
            {
                self.file_path = path.to_string();
                self.load_file(path);
            }
        }

        // ── Top paneel met bestand openen ──
        egui::TopBottomPanel::top("file_toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("📂 Open bestand").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma"])
                        .pick_file()
                    {
                        let path_str = path.to_string_lossy().to_string();
                        self.file_path = path_str.clone();
                        self.load_file(&path_str);
                    }
                }

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.file_path)
                        .hint_text("Pad naar audiobestand...")
                        .desired_width(500.0),
                );

                // Ook laden als Enter wordt ingedrukt in het tekstveld
                if resp.has_focus() {
                    let enter = ui
                        .ctx()
                        .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    if enter {
                        let path = self.file_path.trim().to_string();
                        if !path.is_empty() {
                            self.load_file(&path);
                        }
                    }
                }

                ui.label(
                    RichText::new("(of sleep een bestand in het venster)")
                        .size(11.0)
                        .color(Color32::GRAY),
                );

                // Status rechts uitlijnen
                if !self.status_message.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    });
                }
            });
            ui.add_space(4.0);
        });

        // ── Hoofdpaneel ──
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.separator();

            // ── Foutmelding ──
            if let Some(ref err) = self.waveform_state.error {
                ui.label(
                    RichText::new(format!("⚠ {}", err))
                        .size(13.0)
                        .color(Color32::from_rgb(255, 100, 100)),
                );
            }

            // ── Waveform ──
            let play_position = if self.waveform_state.path.is_some() {
                Some(self.waveform_play_position)
            } else {
                None
            };

            let (_loop_changed, seek_to) =
                render_waveform(ui, &mut self.waveform_state, play_position);

            // Click of drag-release: update playhead position and optionally restart playback
            if let Some(seek_pos) = seek_to {
                // 🔥 FIX: Always update the UI playhead position, even if not playing!
                self.waveform_play_position = seek_pos;

                // If currently playing, send command to audio thread to restart from new position
                if self.waveform_is_playing {
                    let (start, end) = match (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        (Some(a), Some(b)) if b > a => (seek_pos.clamp(a, b), b),
                        _ => (seek_pos, self.waveform_state.duration_secs),
                    };
                    if let Some(ref path) = self.waveform_state.path {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                            path: path.clone(),
                            start_sec: start,
                            end_sec: end,
                            pitch_semitones: self.waveform_state.pitch_semitones,
                            tempo: self.waveform_state.tempo,
                        });
                    }
                }
            }

            // Toon bestandsinfo rechts
            if self.waveform_state.path.is_some() {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{:.1}s  |  {} Hz  |  Zoom: {}x",
                                self.waveform_state.duration_secs,
                                self.waveform_state.sample_rate,
                                (self.waveform_state.zoom / 50.0 * 100.0) as u32
                            ))
                            .size(11.0)
                            .color(Color32::GRAY),
                        );
                    });
                });
            }

            ui.separator();

            // ── Pitch / Tempo controls ──
            ui.horizontal(|ui| {
                ui.label("Pitch:");
                let old_pitch = self.waveform_state.pitch_semitones;
                let mut pitch = old_pitch;
                ui.add(
                    egui::Slider::new(&mut pitch, -12.0..=12.0)
                        .text("semitones")
                        .step_by(0.5),
                );
                if (pitch - old_pitch).abs() > 0.01 {
                    self.waveform_state.pitch_semitones = pitch;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetPitch(pitch));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.pitch_semitones = 0.0;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetPitch(0.0));
                    }
                }

                ui.separator();

                ui.label("Tempo:");
                let old_tempo = self.waveform_state.tempo;
                let mut tempo = old_tempo;
                ui.add(
                    egui::Slider::new(&mut tempo, 0.25..=2.0)
                        .text("x")
                        .step_by(0.05),
                );
                if (tempo - old_tempo).abs() > 0.005 {
                    self.waveform_state.tempo = tempo;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetTempo(tempo));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.tempo = 1.0;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetTempo(1.0));
                    }
                }

                // Playback status
                if self.waveform_is_playing {
                    let p = self.waveform_play_position;
                    let d = self.waveform_play_duration;
                    ui.label(
                        RichText::new(format!(
                            "▶ {:02}:{:02} / {:02}:{:02}",
                            (p / 60.0) as u32,
                            p as u32 % 60,
                            (d / 60.0) as u32,
                            d as u32 % 60,
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(100, 200, 100)),
                    );
                }
            });

            ui.separator();

            // ── Loop controls + zoom ──
            ui.horizontal(|ui| {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        if self.waveform_is_playing {
                            if ui.button("⏹ Stop").clicked() {
                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                            }
                        } else if ui.button("▶ Play Loop (rubato)").clicked() {
                            if let Some(ref path) = self.waveform_state.path {
                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                                    path: path.clone(),
                                    start_sec: a,
                                    end_sec: b,
                                    pitch_semitones: self.waveform_state.pitch_semitones,
                                    tempo: self.waveform_state.tempo,
                                });
                            }
                        }
                    }
                }

                // Save Loop
                if self.waveform_state.loop_a_secs.is_some()
                    && self.waveform_state.loop_b_secs.is_some()
                {
                    if ui.button("💾 Save Loop").clicked() {
                        if let (Some(a), Some(b)) = (
                            self.waveform_state.loop_a_secs,
                            self.waveform_state.loop_b_secs,
                        ) {
                            if b > a {
                                if let Some(ref path) = self.waveform_state.path {
                                    let label =
                                        crate::loops::generate_label(path, &self.saved_loops);
                                    let saved = SavedLoop {
                                        track_path: path.clone(),
                                        label,
                                        loop_a_secs: a,
                                        loop_b_secs: b,
                                        pitch_semitones: self.waveform_state.pitch_semitones,
                                        tempo: self.waveform_state.tempo,
                                    };
                                    crate::loops::add_loop(&mut self.saved_loops, saved);
                                    self.status_message = format!(
                                        "Loop opgeslagen! ({} totaal)",
                                        self.saved_loops.len()
                                    );
                                }
                            }
                        }
                    }
                }

                ui.separator();

                // Loop bibliotheek toggle
                if ui.button("📚 Loops").clicked() {
                    self.show_loop_library = !self.show_loop_library;
                }

                ui.separator();

                // Zoom
                if ui.button("🔍−").clicked() {
                    self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
                }
                if ui.button("🔍+").clicked() {
                    self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
                }
                if ui.button("⟲ Reset zoom/scroll").clicked() {
                    self.waveform_state.zoom = 50.0;
                    self.waveform_state.scroll_offset = 0.0;
                }
            });
        });

        // ── Loop bibliotheek venster ──
        if self.show_loop_library {
            egui::Window::new("📚 Loop Bibliotheek")
                .id(egui::Id::new("loop_library_window"))
                .resizable(true)
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    if self.saved_loops.is_empty() {
                        ui.label("Geen opgeslagen loops. Maak een A-B loop en klik 'Save Loop'.");
                    } else {
                        let mut delete_idx: Option<usize> = None;
                        let mut load_loop: Option<usize> = None;

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (i, saved) in self.saved_loops.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&saved.label).size(14.0).strong());

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("❌").clicked() {
                                            delete_idx = Some(i);
                                        }
                                        if ui.button("▶ Laden").clicked() {
                                            load_loop = Some(i);
                                        }
                                    });
                                });

                                ui.label(
                                    RichText::new(format!(
                                        "  {:02}:{:02} → {:02}:{:02}  |  Pitch: {:+.1}  Tempo: {:.2}x",
                                        (saved.loop_a_secs / 60.0) as u32,
                                        saved.loop_a_secs as u32 % 60,
                                        (saved.loop_b_secs / 60.0) as u32,
                                        saved.loop_b_secs as u32 % 60,
                                        saved.pitch_semitones,
                                        saved.tempo,
                                    ))
                                    .size(11.0)
                                    .color(Color32::GRAY),
                                );
                                ui.separator();
                            }
                        });

                        if let Some(idx) = delete_idx {
                            crate::loops::remove_loop(&mut self.saved_loops, idx);
                        }

                        if let Some(idx) = load_loop {
                            let saved = self.saved_loops[idx].clone();
                            // Laad de track als deze nog niet geladen is
                            if self.waveform_state.path.as_deref() != Some(&saved.track_path) {
                                self.load_file(&saved.track_path);
                            }
                            self.waveform_state.loop_a_secs = Some(saved.loop_a_secs);
                            self.waveform_state.loop_b_secs = Some(saved.loop_b_secs);
                            self.waveform_state.pitch_semitones = saved.pitch_semitones;
                            self.waveform_state.tempo = saved.tempo;
                            self.status_message = format!("Loop '{}' geladen", saved.label);
                        }
                    }
                });
        }
    }
}
