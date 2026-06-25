use crate::chroma::{detect_chroma, Chroma};
use crate::loops::{Library, SavedLoop};
use crate::session::SessionState;
use crate::shortcuts::{KeyBinding, SerializableKey, ShortcutAction, ShortcutsConfig};
use crate::waveform::{render_waveform, ChannelMode, WaveformState};
use crate::waveform_player::{start_waveform_thread, WaveformCommand, WaveformEvent};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, RichText};
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};

pub struct LoopEditorApp {
    // Waveform state
    pub waveform_state: WaveformState,
    pub waveform_cmd_tx: Sender<WaveformCommand>,
    pub waveform_event_rx: Receiver<WaveformEvent>,
    pub waveform_is_playing: bool,
    pub waveform_play_position: f32,
    pub waveform_play_duration: f32,
    pub waveform_has_content: bool,

    // Library (tracks, loops, markers)
    pub library: Library,
    pub show_loop_library: bool,
    pub active_loop_idx: Option<usize>,

    // Loop point pending (voor 1-toets A-B zetten)
    pub pending_loop_point: Option<f32>,

    // Chroma detectie
    pub chroma_result: Option<Chroma>,

    // File path input
    pub file_path: String,
    pub status_message: String,
    pub status_message_timer: u32,

    // Help / shortcuts
    pub show_shortcuts: bool,

    // Looping bypass
    pub loop_bypassed: bool,

    // Shortcuts
    pub shortcuts: ShortcutsConfig,
    pub show_shortcut_editor: bool,
    pub listening_for_action: Option<ShortcutAction>,

    // Undo/Redo
    pub undo_stack: Vec<UndoState>,
    pub redo_stack: Vec<UndoState>,
}

/// Momentopname van de muteerbare editor state (voor undo/redo).
#[derive(Clone)]
pub struct UndoState {
    pub play_position: f32,
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
    pub pitch_semitones: f32,
    pub tempo: f32,
    pub volume: f32,
    pub zoom: f32,
    pub scroll_offset: f32,
    pub markers: Vec<crate::waveform::Marker>,
    pub loop_bypassed: bool,
}

impl LoopEditorApp {
    pub fn new() -> Self {
        let (waveform_cmd_tx, waveform_event_rx) = start_waveform_thread();
        let library = crate::loops::load_library();
        let shortcuts = ShortcutsConfig::load();

        let mut app = Self {
            waveform_state: WaveformState::default(),
            waveform_cmd_tx,
            waveform_event_rx,
            waveform_is_playing: false,
            waveform_play_position: 0.0,
            waveform_play_duration: 0.0,
            waveform_has_content: false,
            library,
            show_loop_library: false,
            active_loop_idx: None,
            pending_loop_point: None,
            chroma_result: None,
            file_path: String::new(),
            status_message: String::new(),
            status_message_timer: 0,
            show_shortcuts: false,
            loop_bypassed: false,
            shortcuts,
            show_shortcut_editor: false,
            listening_for_action: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        // Laad sessie (vorige file, positie, etc.)
        if let Some(session) = SessionState::load() {
            app.waveform_state.zoom = session.zoom;
            app.waveform_state.scroll_offset = session.scroll_offset;
            app.waveform_state.loop_a_secs = session.loop_a_secs;
            app.waveform_state.loop_b_secs = session.loop_b_secs;
            app.waveform_state.pitch_semitones = session.pitch_semitones;
            app.waveform_state.tempo = session.tempo;
            app.waveform_state.volume = session.volume;
            app.waveform_play_position = session.play_position;
            app.waveform_state.channel_mode =
                serde_json::from_str(&format!("\"{}\"", session.channel_mode))
                    .unwrap_or(ChannelMode::Mono);

            if let Some(ref path) = session.file_path {
                if Path::new(path).exists() {
                    app.file_path = path.clone();
                    app.load_file(path);
                }
            }
        }

        app
    }
    fn egui_key_to_serializable(&self, key: egui::Key) -> SerializableKey {
        match key {
            egui::Key::Space => SerializableKey::Space,
            egui::Key::Enter => SerializableKey::Enter,
            egui::Key::Escape => SerializableKey::Escape,
            egui::Key::Backspace => SerializableKey::Backspace,
            egui::Key::Tab => SerializableKey::Tab,
            egui::Key::ArrowLeft => SerializableKey::ArrowLeft,
            egui::Key::ArrowRight => SerializableKey::ArrowRight,
            egui::Key::ArrowUp => SerializableKey::ArrowUp,
            egui::Key::ArrowDown => SerializableKey::ArrowDown,
            egui::Key::A => SerializableKey::A,
            egui::Key::B => SerializableKey::B,
            egui::Key::C => SerializableKey::C,
            egui::Key::D => SerializableKey::D,
            egui::Key::E => SerializableKey::E,
            egui::Key::F => SerializableKey::F,
            egui::Key::G => SerializableKey::G,
            egui::Key::H => SerializableKey::H,
            egui::Key::I => SerializableKey::I,
            egui::Key::J => SerializableKey::J,
            egui::Key::K => SerializableKey::K,
            egui::Key::L => SerializableKey::L,
            egui::Key::M => SerializableKey::M,
            egui::Key::N => SerializableKey::N,
            egui::Key::O => SerializableKey::O,
            egui::Key::P => SerializableKey::P,
            egui::Key::Q => SerializableKey::Q,
            egui::Key::R => SerializableKey::R,
            egui::Key::S => SerializableKey::S,
            egui::Key::T => SerializableKey::T,
            egui::Key::U => SerializableKey::U,
            egui::Key::V => SerializableKey::V,
            egui::Key::W => SerializableKey::W,
            egui::Key::X => SerializableKey::X,
            egui::Key::Y => SerializableKey::Y,
            egui::Key::Z => SerializableKey::Z,
            egui::Key::Num0 => SerializableKey::Num0,
            egui::Key::Num1 => SerializableKey::Num1,
            egui::Key::Num2 => SerializableKey::Num2,
            egui::Key::Num3 => SerializableKey::Num3,
            egui::Key::Num4 => SerializableKey::Num4,
            egui::Key::Num5 => SerializableKey::Num5,
            egui::Key::Num6 => SerializableKey::Num6,
            egui::Key::Num7 => SerializableKey::Num7,
            egui::Key::Num8 => SerializableKey::Num8,
            egui::Key::Num9 => SerializableKey::Num9,
            egui::Key::OpenBracket => SerializableKey::OpenBracket,
            egui::Key::CloseBracket => SerializableKey::CloseBracket,
            egui::Key::F1 => SerializableKey::F1,
            egui::Key::F2 => SerializableKey::F2,
            egui::Key::F3 => SerializableKey::F3,
            egui::Key::F4 => SerializableKey::F4,
            egui::Key::F5 => SerializableKey::F5,
            egui::Key::F6 => SerializableKey::F6,
            egui::Key::F7 => SerializableKey::F7,
            egui::Key::F8 => SerializableKey::F8,
            egui::Key::F9 => SerializableKey::F9,
            egui::Key::F10 => SerializableKey::F10,
            egui::Key::F11 => SerializableKey::F11,
            egui::Key::F12 => SerializableKey::F12,
            _ => SerializableKey::Space, // Fallback voor niet-behandelde keys
        }
    }
    pub fn load_file(&mut self, path: &str) {
        // Stop huidige playback als er een ander bestand wordt geladen
        if self.waveform_state.path.as_deref() != Some(path) {
            if self.waveform_is_playing {
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                self.waveform_is_playing = false;
            }
            self.waveform_has_content = false;
        }

        match crate::waveform::decode_audio(path, self.waveform_state.channel_mode) {
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

                // Herstel markers uit de bibliotheek
                let track = self.library.track_for_path(path);
                self.waveform_state.markers = track.markers.clone();
                self.active_loop_idx = None;
                self.pending_loop_point = None;
                self.chroma_result = None;
                self.save_session();

                self.status_message = format!(
                    "Geladen: {} ({:.1}s, {} Hz)",
                    Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default(),
                    duration_secs,
                    sample_rate,
                );
                self.status_message_timer = 5 * 60;
            }
            Err(e) => {
                self.waveform_state.error = Some(e.clone());
                self.status_message = format!("Fout bij laden: {}", e);
                self.status_message_timer = 10 * 60;
            }
        }
    }

    /// Synchroniseer markers van waveform_state naar library en sla op.
    fn sync_markers_to_library(&mut self) {
        if let Some(ref path) = self.waveform_state.path.clone() {
            let track = self.library.track_for_path(path);
            track.markers = self.waveform_state.markers.clone();
            crate::loops::save_library(&self.library);
        }
    }

    /// Stuur huidige A-B loop naar de audio-thread.
    fn sync_loop_bounds(&mut self) {
        let a = self.waveform_state.loop_a_secs.unwrap_or(0.0);
        let b = self.waveform_state.loop_b_secs.unwrap_or(0.0);
        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
            a_secs: a,
            b_secs: b,
        });
    }

    /// Sla huidige editor state op voor undo.
    fn push_undo(&mut self) {
        const MAX_UNDO: usize = 50;
        self.undo_stack.push(UndoState {
            play_position: self.waveform_play_position,
            loop_a_secs: self.waveform_state.loop_a_secs,
            loop_b_secs: self.waveform_state.loop_b_secs,
            pitch_semitones: self.waveform_state.pitch_semitones,
            tempo: self.waveform_state.tempo,
            volume: self.waveform_state.volume,
            zoom: self.waveform_state.zoom,
            scroll_offset: self.waveform_state.scroll_offset,
            markers: self.waveform_state.markers.clone(),
            loop_bypassed: self.loop_bypassed,
        });
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Sla huidige state op in session.json (voor herstart).
    fn save_session(&self) {
        let mode_str = format!("{:?}", self.waveform_state.channel_mode);
        SessionState::save(
            self.waveform_state.path.as_deref(),
            self.waveform_play_position,
            self.waveform_state.zoom,
            self.waveform_state.scroll_offset,
            self.waveform_state.loop_a_secs,
            self.waveform_state.loop_b_secs,
            self.waveform_state.pitch_semitones,
            self.waveform_state.tempo,
            self.waveform_state.volume,
            &mode_str,
        );
    }

    /// Herstel een UndoState.
    fn restore_undo(&mut self, state: UndoState) {
        self.waveform_play_position = state.play_position;
        self.waveform_state.loop_a_secs = state.loop_a_secs;
        self.waveform_state.loop_b_secs = state.loop_b_secs;
        self.waveform_state.pitch_semitones = state.pitch_semitones;
        self.waveform_state.tempo = state.tempo;
        self.waveform_state.volume = state.volume;
        self.waveform_state.zoom = state.zoom;
        self.waveform_state.scroll_offset = state.scroll_offset;
        self.waveform_state.markers = state.markers;
        self.loop_bypassed = state.loop_bypassed;
        self.sync_loop_bounds();
        self.status_message = "Undo/Redo".to_string();
        self.status_message_timer = 2 * 60;
    }

    /// Centreer de viewport op de huidige A-B loop.
    fn center_view_on_loop(&mut self, viewport_width_px: f32) {
        if let (Some(a), Some(b)) = (
            self.waveform_state.loop_a_secs,
            self.waveform_state.loop_b_secs,
        ) {
            if b > a {
                let loop_width = b - a;
                let target_zoom = (viewport_width_px * 0.6) / loop_width;
                self.waveform_state.zoom = target_zoom.max(5.0).min(5000.0);

                let visible_secs = viewport_width_px / self.waveform_state.zoom;
                let mid = (a + b) / 2.0;
                self.waveform_state.scroll_offset = (mid - visible_secs / 2.0)
                    .max(0.0)
                    .min((self.waveform_state.duration_secs - visible_secs).max(0.0));
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
                    self.waveform_has_content = true;
                    ctx.request_repaint();
                }
                WaveformEvent::Stopped => {
                    self.waveform_is_playing = false;
                    self.waveform_has_content = false;
                    ctx.request_repaint();
                }
                WaveformEvent::Paused => {
                    self.waveform_is_playing = false;
                    ctx.request_repaint();
                }
                WaveformEvent::Resumed => {
                    self.waveform_is_playing = true;
                    ctx.request_repaint();
                }
                WaveformEvent::Error(msg) => {
                    self.waveform_is_playing = false;
                    self.status_message = format!("Waveform fout: {}", msg);
                    ctx.request_repaint();
                }
                WaveformEvent::Position(pos, dur) => {
                    self.waveform_play_duration = dur;

                    // ✅ Check of de audio-thread de seek heeft voltooid
                    if let Some(target) = self.waveform_state.seek_pending {
                        // Als de audio-thread binnen 50ms (0.05s) van de target positie is,
                        // beschouwen we de seek als geslaagd.
                        if (pos - target).abs() < 0.05 {
                            self.waveform_state.seek_pending = None;
                        }
                    }

                    // ✅ Accepteer de positie ALLEEN als:
                    // 1. Er geen seek pending is (de audio is gearriveerd)
                    // 2. We niet aan het slepen zijn
                    if self.waveform_state.seek_pending.is_none()
                        && !self.waveform_state.dragging_playhead
                    {
                        self.waveform_play_position = pos;
                    }

                    ctx.request_repaint();
                }
            }
        }

        // Verval statusmelding na 5 seconden
        if self.status_message_timer > 0 {
            self.status_message_timer -= 1;
            if self.status_message_timer == 0 {
                self.status_message.clear();
            }
        }

        // 🔥 CRITICAL: Force continuous repaints while playing so the playhead moves smoothly
        if self.waveform_is_playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        // ── Keyboard Shortcuts ──
        let is_text_focused = ctx.memory(|mem| mem.focused().is_some());
        if let Some(action) = self.listening_for_action {
            if let Some(key_event) = ctx.input(|i| i.keys_down.iter().next().copied()) {
                let mods = ctx.input(|i| i.modifiers);
                let binding = KeyBinding {
                    key: self.egui_key_to_serializable(key_event),
                    ctrl: mods.ctrl,
                    shift: mods.shift,
                    alt: mods.alt,
                };
                // Check op conflicts
                if let Some(conflict) = self.shortcuts.find_conflict(&binding, action) {
                    self.status_message = format!(
                        "⚠ Conflict: '{}' is al gebruikt voor '{}'",
                        binding.display(),
                        conflict.display_name()
                    );
                    self.status_message_timer = 5 * 60;
                } else {
                    if let Err(e) = self.shortcuts.set_binding(action, binding) {
                        self.status_message = format!("Fout bij opslaan: {}", e);
                    } else {
                        self.status_message = format!(
                            "✓ '{}' nu gekoppeld aan '{}'",
                            binding.display(),
                            action.display_name()
                        );
                        self.status_message_timer = 3 * 60;
                    }
                }
                self.listening_for_action = None;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.listening_for_action = None;
            }
        } else if !is_text_focused {
            if self
                .shortcuts
                .is_pressed(ShortcutAction::PlayPause, &ctx.input(|i| i.clone()))
            {
                if self.waveform_has_content {
                    // Audio is geladen (speelt of gepauzeerd) → toggle
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::TogglePause);
                } else if let Some(ref _path) = self.waveform_state.path {
                    // Nog niks geladen in audio-thread → start nieuwe playback
                    let (decode_start, play_start, decode_end) = match (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        (Some(a), Some(b)) if b > a => {
                            // If looping, decode the whole loop (A to B), but start playing at the current playhead
                            let start = self.waveform_play_position.clamp(a, b);
                            (a, start, b)
                        }
                        _ => {
                            // Geen loop: decode alleen vanaf playhead, stuur a == b zodat de
                            // audio-thread weet dat er géén looping is.
                            let start = self.waveform_play_position;
                            (start, start, start) // decode_end == play_start → a_sample == b_sample
                        }
                    };

                    let sr = self.waveform_state.sample_rate as f32;
                    let start_sample = (play_start * sr) as usize;
                    let a_sample = (decode_start * sr) as usize;
                    let b_sample = (decode_end * sr) as usize;

                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                        samples: Arc::new(Mutex::new(self.waveform_state.samples.clone())), // ✅ Volledige track!
                        sample_rate: self.waveform_state.sample_rate,
                        start_sample,
                        segment_start_sec: 0.0, // ✅ De buffer begint nu bij 0.0s van de track
                        a_sample,
                        b_sample,
                        pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                            self.waveform_state.pitch_semitones,
                        ))),
                        tempo: Arc::new(AtomicU32::new(f32::to_bits(self.waveform_state.tempo))),
                    });

                    self.waveform_is_playing = true;
                }
            }

            // ── Marker shortcuts (1-9), Backspace (verwijder dichtstbijzijnde), [ ] (A-B) ──
            if self.waveform_state.path.is_some() {
                // ── Marker shortcuts: S (Section), M (Measure), B (Beat) ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddSectionMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Section)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("S{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Section,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Section marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddMeasureMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Measure)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("M{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Measure,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Measure marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddBeatMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Beat)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("B{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Beat,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Beat marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self.shortcuts.is_pressed(
                    ShortcutAction::DeleteNearestMarker,
                    &ctx.input(|i| i.clone()),
                ) {
                    let pos = self.waveform_play_position;
                    let mut best_idx: Option<usize> = None;
                    let mut best_dist = 2.0_f32;
                    for (i, m) in self.waveform_state.markers.iter().enumerate() {
                        let dist = (m.position_secs - pos).abs();
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = Some(i);
                        }
                    }
                    if let Some(idx) = best_idx {
                        let removed = self.waveform_state.markers.remove(idx);
                        self.push_undo();
                        self.sync_markers_to_library();
                        self.status_message = format!("Marker '{}' verwijderd", removed.name);
                        self.status_message_timer = 3 * 60;
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SetLoopA, &ctx.input(|i| i.clone()))
                {
                    self.waveform_state.loop_a_secs = Some(self.waveform_play_position);
                    self.push_undo();
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                        a_secs: self.waveform_play_position,
                        b_secs: self
                            .waveform_state
                            .loop_b_secs
                            .unwrap_or(self.waveform_state.duration_secs),
                    });
                    self.status_message =
                        format!("Loop A gezet op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SetLoopB, &ctx.input(|i| i.clone()))
                {
                    self.waveform_state.loop_b_secs = Some(self.waveform_play_position);
                    self.push_undo();
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                        a_secs: self.waveform_state.loop_a_secs.unwrap_or(0.0),
                        b_secs: self.waveform_play_position,
                    });
                    self.status_message =
                        format!("Loop B gezet op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeLoopLeft, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        let len = b - a;
                        let new_a = (a - len).max(0.0);
                        let new_b = new_a + len;
                        self.waveform_state.loop_a_secs = Some(new_a);
                        self.waveform_state.loop_b_secs = Some(new_b);

                        self.waveform_play_position = new_a;
                        self.waveform_state.seek_pending = Some(new_a);
                        self.waveform_state.playhead_frames_after_drag = 15;

                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget ← naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
                    } else {
                        self.status_message = "Geen A-B loop ingesteld om te nudgen".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeLoopRight, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        let len = b - a;
                        let dur = self.waveform_state.duration_secs;
                        let new_b = (b + len).min(dur);
                        let new_a = new_b - len;
                        self.waveform_state.loop_a_secs = Some(new_a);
                        self.waveform_state.loop_b_secs = Some(new_b);

                        self.waveform_play_position = new_a;
                        self.waveform_state.seek_pending = Some(new_a);
                        self.waveform_state.playhead_frames_after_drag = 15;

                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget → naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
                    } else {
                        self.status_message = "Geen A-B loop ingesteld om te nudgen".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }
            }

            // ── Nudge marker A links/rechts (J / Shift+J) ──
            if self.waveform_state.path.is_some() {
                let step = 0.05;
                let mut changed = false;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeALeft, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs.as_mut() {
                        *a = (*a - step).max(0.0);
                        if let Some(b) = self.waveform_state.loop_b_secs {
                            if *a >= b {
                                *a = (b - step).max(0.0);
                            }
                        }
                        changed = true;
                    }
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeARight, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs.as_mut() {
                        *a = (*a + step).min(self.waveform_state.duration_secs);
                        if let Some(b) = self.waveform_state.loop_b_secs {
                            if *a >= b {
                                *a = (b - step).max(0.0);
                            }
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.sync_loop_bounds();
                }
            }

            // ── Nudge marker B links/rechts (L / Shift+L) ──
            if self.waveform_state.path.is_some() {
                let step = 0.05;
                let mut changed = false;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeBLeft, &ctx.input(|i| i.clone()))
                {
                    if let Some(b) = self.waveform_state.loop_b_secs.as_mut() {
                        *b = (*b - step).max(0.0);
                        if let Some(a) = self.waveform_state.loop_a_secs {
                            if *b <= a {
                                *b = (a + step).min(self.waveform_state.duration_secs);
                            }
                        }
                        changed = true;
                    }
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeBRight, &ctx.input(|i| i.clone()))
                {
                    if let Some(b) = self.waveform_state.loop_b_secs.as_mut() {
                        *b = (*b + step).min(self.waveform_state.duration_secs);
                        if let Some(a) = self.waveform_state.loop_a_secs {
                            if *b <= a {
                                *b = (a + step).min(self.waveform_state.duration_secs);
                            }
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.sync_loop_bounds();
                }
            }

            // ── ←/→ Playhead nudgen (0.20s) ──
            if self.waveform_state.path.is_some() {
                let step = 0.20;
                let mut new_pos: Option<f32> = None;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgePlayheadLeft, &ctx.input(|i| i.clone()))
                {
                    new_pos = Some((self.waveform_play_position - step).max(0.0));
                }
                if self.shortcuts.is_pressed(
                    ShortcutAction::NudgePlayheadRight,
                    &ctx.input(|i| i.clone()),
                ) {
                    new_pos = Some(
                        (self.waveform_play_position + step).min(self.waveform_state.duration_secs),
                    );
                }
                if let Some(pos) = new_pos {
                    self.waveform_play_position = pos;
                    self.waveform_state.seek_pending = Some(pos);
                    let _ = self
                        .waveform_cmd_tx
                        .send(WaveformCommand::Seek { pos_secs: pos });
                    self.waveform_state.playhead_frames_after_drag = 15;
                }
            }

            // ── ↑/↓ Rewind/Forward 2 seconden ──
            if self.waveform_state.path.is_some() {
                let mut seek_delta: Option<f32> = None;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SeekBackward, &ctx.input(|i| i.clone()))
                {
                    seek_delta = Some(-2.0);
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SeekForward, &ctx.input(|i| i.clone()))
                {
                    seek_delta = Some(2.0);
                }

                if let Some(delta) = seek_delta {
                    let new_pos = (self.waveform_play_position + delta)
                        .clamp(0.0, self.waveform_state.duration_secs);
                    self.waveform_play_position = new_pos;
                    self.waveform_state.seek_pending = Some(new_pos); // ✅ NIEUW: Markeer als pending

                    // if self.waveform_has_content {
                    let _ = self
                        .waveform_cmd_tx
                        .send(WaveformCommand::Seek { pos_secs: new_pos });
                    // ✅ FIX: Negeer oude Position events voor ~250ms
                    self.waveform_state.playhead_frames_after_drag = 15;
                    //   }
                }
            }

            // Stop
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Stop, &ctx.input(|i| i.clone()))
            {
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                self.waveform_is_playing = false;
                self.waveform_has_content = false;
            }

            // ClearLoop
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ClearLoop, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.loop_a_secs = None;
                self.waveform_state.loop_b_secs = None;
                self.pending_loop_point = None;
                self.push_undo();
                let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                    a_secs: 0.0,
                    b_secs: 0.0,
                });
                self.status_message = "Loop gewist".to_string();
                self.status_message_timer = 2 * 60;
            }

            // ToggleLoopBypass
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleLoopBypass, &ctx.input(|i| i.clone()))
            {
                self.loop_bypassed = !self.loop_bypassed;
                let _ = self
                    .waveform_cmd_tx
                    .send(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
                self.status_message = if self.loop_bypassed {
                    "Loop bypassed — speelt door naar einde".to_string()
                } else {
                    "Loop hervat".to_string()
                };
                self.status_message_timer = 2 * 60;
            }

            // ToggleLoopPoint — 1 toets A-B
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleLoopPoint, &ctx.input(|i| i.clone()))
            {
                let pos = self.waveform_play_position;
                if let Some(pending) = self.pending_loop_point {
                    let (a, b) = if pos > pending {
                        (pending, pos)
                    } else {
                        (pos, pending)
                    };
                    self.waveform_state.loop_a_secs = Some(a);
                    self.waveform_state.loop_b_secs = Some(b);
                    self.pending_loop_point = None;
                    self.push_undo();
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                        a_secs: a,
                        b_secs: b,
                    });
                    self.status_message = format!("Loop A-B gezet: {:.1}s → {:.1}s", a, b);
                    self.status_message_timer = 3 * 60;
                } else {
                    self.pending_loop_point = Some(pos);
                    self.status_message = format!("Loop punt 1 op {:.1}s — druk nogmaals", pos);
                    self.status_message_timer = 3 * 60;
                }
            }

            // ZoomIn / ZoomOut / ResetZoom
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ZoomIn, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ZoomOut, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ResetZoom, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = 50.0;
                self.waveform_state.scroll_offset = 0.0;
            }

            // OpenFile
            if self
                .shortcuts
                .is_pressed(ShortcutAction::OpenFile, &ctx.input(|i| i.clone()))
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma"])
                    .pick_file()
                {
                    let path_str = path.to_string_lossy().to_string();
                    self.file_path = path_str.clone();
                    self.load_file(&path_str);
                }
            }

            // Undo
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Undo, &ctx.input(|i| i.clone()))
            {
                if let Some(state) = self.undo_stack.pop() {
                    self.redo_stack.push(UndoState {
                        play_position: self.waveform_play_position,
                        loop_a_secs: self.waveform_state.loop_a_secs,
                        loop_b_secs: self.waveform_state.loop_b_secs,
                        pitch_semitones: self.waveform_state.pitch_semitones,
                        tempo: self.waveform_state.tempo,
                        volume: self.waveform_state.volume,
                        zoom: self.waveform_state.zoom,
                        scroll_offset: self.waveform_state.scroll_offset,
                        markers: self.waveform_state.markers.clone(),
                        loop_bypassed: self.loop_bypassed,
                    });
                    self.restore_undo(state);
                }
            }

            // Redo
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Redo, &ctx.input(|i| i.clone()))
            {
                if let Some(state) = self.redo_stack.pop() {
                    self.undo_stack.push(UndoState {
                        play_position: self.waveform_play_position,
                        loop_a_secs: self.waveform_state.loop_a_secs,
                        loop_b_secs: self.waveform_state.loop_b_secs,
                        pitch_semitones: self.waveform_state.pitch_semitones,
                        tempo: self.waveform_state.tempo,
                        volume: self.waveform_state.volume,
                        zoom: self.waveform_state.zoom,
                        scroll_offset: self.waveform_state.scroll_offset,
                        markers: self.waveform_state.markers.clone(),
                        loop_bypassed: self.loop_bypassed,
                    });
                    self.restore_undo(state);
                }
            }

            // View
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ShowShortcuts, &ctx.input(|i| i.clone()))
            {
                self.show_shortcuts = !self.show_shortcuts;
            }

            // RestartLoop — seek naar A en start playback
            if self
                .shortcuts
                .is_pressed(ShortcutAction::RestartLoop, &ctx.input(|i| i.clone()))
            {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        self.waveform_play_position = a;
                        self.waveform_state.seek_pending = Some(a);
                        self.waveform_state.playhead_frames_after_drag = 15;
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                            samples: Arc::new(Mutex::new(self.waveform_state.samples.clone())),
                            sample_rate: self.waveform_state.sample_rate,
                            start_sample: (a * self.waveform_state.sample_rate as f32) as usize,
                            segment_start_sec: 0.0,
                            a_sample: (a * self.waveform_state.sample_rate as f32) as usize,
                            b_sample: (b * self.waveform_state.sample_rate as f32) as usize,
                            pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                                self.waveform_state.pitch_semitones,
                            ))),
                            tempo: Arc::new(AtomicU32::new(f32::to_bits(
                                self.waveform_state.tempo,
                            ))),
                        });
                        self.waveform_is_playing = true;
                        self.waveform_has_content = true;
                        self.status_message = format!("Loop herstart vanaf {:.1}s", a);
                        self.status_message_timer = 3 * 60;
                    }
                }
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

                // Kanaal modus dropdown
                let old_mode = self.waveform_state.channel_mode;
                egui::ComboBox::from_id_source("channel_mode")
                    .selected_text(old_mode.display())
                    .show_ui(ui, |ui| {
                        for &mode in &[
                            ChannelMode::Mono,
                            ChannelMode::Left,
                            ChannelMode::Right,
                            ChannelMode::Mid,
                            ChannelMode::Side,
                        ] {
                            if ui
                                .selectable_label(
                                    self.waveform_state.channel_mode == mode,
                                    mode.display(),
                                )
                                .clicked()
                            {
                                self.waveform_state.channel_mode = mode;
                            }
                        }
                    });
                if self.waveform_state.channel_mode != old_mode {
                    if let Some(ref path) = self.waveform_state.path.clone() {
                        self.load_file(path);
                    }
                    self.save_session();
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

        // ── Shortcuts help overlay (dynamisch uit shortcuts data) ──
        if self.show_shortcuts {
            egui::Window::new("⌨ Toetsenbord Shortcuts")
                .id(egui::Id::new("shortcuts_window"))
                .resizable(true)
                .default_size([400.0, 500.0])
                .default_pos([200.0, 150.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            // ── Dynamische shortcuts uit ShortcutAction::all() ──
                            use crate::shortcuts::ShortcutAction;
                            let categories =
                                ["Playback", "Loop", "Markers", "View", "File", "Edit"];
                            for category in categories {
                                ui.label(
                                    RichText::new(category)
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(180, 180, 220)),
                                );
                                for action in ShortcutAction::all()
                                    .iter()
                                    .filter(|a| a.category() == category)
                                {
                                    let key_text = self
                                        .shortcuts
                                        .binding_for(*action)
                                        .map(|b| b.display())
                                        .unwrap_or_else(|| "—".to_string());
                                    shortcut_row(ui, &key_text, action.display_name());
                                }
                                ui.separator();
                            }

                            // ── Extra muis-acties (geen shortcuts) ──
                            ui.label(
                                RichText::new("Mouse / Interactie")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(180, 180, 220)),
                            );
                            shortcut_row(ui, "Ctrl+Sleep", "A-B selectie maken");
                            shortcut_row(ui, "Dubbelklik", "Zet A-marker");
                            shortcut_row(ui, "Shift+Dubbelklik", "Zet B-marker");
                            shortcut_row(ui, "Rechterklik", "Wis A-B selectie");
                            shortcut_row(ui, "Scroll", "Zoom in/uit");
                            shortcut_row(ui, "Sleep (geen Ctrl)", "Horizontaal scrollen");
                            ui.separator();

                            if ui.button("⚙ Edit Shortcuts").clicked() {
                                self.show_shortcut_editor = !self.show_shortcut_editor;
                            }
                            ui.label(
                                RichText::new("Druk op F1 om te sluiten")
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });
                    });
                });
        }

        // ── Hoofdpaneel ──
        egui::CentralPanel::default().show(ctx, |ui| {
            let panel_width = ui.available_width().max(100.0);
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

            let (loop_changed, seek_to) =
                render_waveform(ui, &mut self.waveform_state, play_position);

            // 🔥 Loop-grenzen tijdens playback: stuur SetLoopBounds
            //    → audio-thread past ze direct toe zonder de source te herstarten
            // Stuur loop-verandering altijd naar audio-thread, ook als de
            // audio stilstaat. Anders blijft de audio-thread een oude loop
            // onthouden, die bij een volgende Play onzichtbaar wordt hervat.
            if loop_changed {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: a,
                            b_secs: b,
                        });
                        // Als de loop was bypassed, heractiveer haar bij A/B-wijziging
                        if self.loop_bypassed {
                            self.loop_bypassed = false;
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::SetLoopEnabled(true));
                            self.status_message = "Loop geüpdatet en geactiveerd".to_string();
                            self.status_message_timer = 3 * 60;
                        }
                    }
                } else {
                    // Rechterklik: loop gewist → stuur 0/0 naar audio-thread (enabled: false)
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                        a_secs: 0.0,
                        b_secs: 0.0,
                    });
                    self.pending_loop_point = None;
                    self.status_message = "Loop gewist".to_string();
                    self.status_message_timer = 2 * 60;
                }
            }

            // Click of drag-release: update playhead position, seek audio-thread if playing
            if let Some(seek_pos) = seek_to {
                self.waveform_play_position = seek_pos;
                self.waveform_state.seek_pending = Some(seek_pos); // ✅ NIEUW: Markeer als pending
                                                                   //    if self.waveform_has_content {
                let _ = self
                    .waveform_cmd_tx
                    .send(WaveformCommand::Seek { pos_secs: seek_pos });
                //  }
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

                ui.separator();

                ui.label("Vol:");
                let old_vol = self.waveform_state.volume;
                let mut vol = old_vol;
                ui.add(
                    egui::Slider::new(&mut vol, 0.0..=2.0)
                        .text("x")
                        .step_by(0.05),
                );
                if (vol - old_vol).abs() > 0.01 {
                    self.waveform_state.volume = vol;
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetVolume(vol));
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.volume = 1.0;
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetVolume(1.0));
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
                                self.waveform_is_playing = false;
                                ctx.request_repaint();
                            }

                            // Bypass loop toggle
                            let (btn_label, btn_color) = if self.loop_bypassed {
                                ("🔁 Loop: OFF", Color32::from_rgb(200, 100, 100))
                            } else {
                                ("🔁 Loop: ON", Color32::from_rgb(80, 200, 80))
                            };
                            if ui
                                .button(RichText::new(btn_label).color(btn_color))
                                .clicked()
                            {
                                self.loop_bypassed = !self.loop_bypassed;
                                let _ = self
                                    .waveform_cmd_tx
                                    .send(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
                                self.status_message = if self.loop_bypassed {
                                    "Loop bypassed — speelt door naar einde".to_string()
                                } else {
                                    "Loop hervat".to_string()
                                };
                                self.status_message_timer = 3 * 60;
                            }
                        } else if ui.button("▶ Play Loop").clicked() {
                            if let Some(ref _path) = self.waveform_state.path {
                                let sr = self.waveform_state.sample_rate as f32;
                                let a_sample = (a * sr) as usize;
                                let b_sample = (b * sr) as usize;

                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                                    samples: Arc::new(Mutex::new(
                                        self.waveform_state.samples.clone(),
                                    )), // ✅ Volledige track!
                                    sample_rate: self.waveform_state.sample_rate,
                                    start_sample: a_sample, // Start met spelen op A
                                    segment_start_sec: 0.0, // ✅ De buffer begint nu bij 0.0s van de track
                                    a_sample,
                                    b_sample,
                                    pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                                        self.waveform_state.pitch_semitones,
                                    ))),
                                    tempo: Arc::new(AtomicU32::new(f32::to_bits(
                                        self.waveform_state.tempo,
                                    ))),
                                });
                                self.waveform_is_playing = true;
                                ctx.request_repaint();
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
                                    let label = self.library.generate_label(path);
                                    let saved = SavedLoop {
                                        label,
                                        loop_a_secs: a,
                                        loop_b_secs: b,
                                        pitch_semitones: self.waveform_state.pitch_semitones,
                                        tempo: self.waveform_state.tempo,
                                        notes: String::new(),
                                    };
                                    let track = self.library.track_for_path(path);
                                    track.loops.push(saved);
                                    let total = track.loops.len();
                                    crate::loops::save_library(&self.library);
                                    self.status_message =
                                        format!("Loop opgeslagen! ({} totaal)", total);
                                    self.status_message_timer = 3 * 60;
                                }
                            }
                        }
                    }
                }

                ui.separator();

                // Loop bibliotheek toggle
                if ui.button("📚 Alle Tracks").clicked() {
                    self.show_loop_library = !self.show_loop_library;
                }

                ui.separator();

                // Center loop in viewport
                if ui
                    .button("🎯 Center Loop")
                    .on_hover_text("Centreer de A-B loop in het venster")
                    .clicked()
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        if b > a {
                            self.center_view_on_loop(panel_width);
                        }
                    }
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

            // ── Track Paneel (onder de knoppen) ──
            if let Some(ref path) = self.waveform_state.path.clone() {
                let track_path = path.clone();
                ui.separator();

                // ── Opgeslagen Loops (altijd zichtbaar) ──
                ui.strong("Opgeslagen Loops");
                let track = self.library.track_for_path(&track_path);
                if track.loops.is_empty() {
                    ui.label(
                        "Nog geen loops opgeslagen. Maak een A-B selectie en klik 'Save Loop'.",
                    );
                } else {
                    let mut delete_idx: Option<usize> = None;
                    let mut load_idx: Option<usize> = None;

                    for (i, saved) in track.loops.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&saved.label).size(13.0).strong());
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("❌").clicked() {
                                        delete_idx = Some(i);
                                    }
                                    if ui.small_button("▶").clicked() {
                                        load_idx = Some(i);
                                    }
                                },
                            );
                        });
                    }

                    if let Some(idx) = delete_idx {
                        let track = self.library.track_for_path(&track_path);
                        if idx < track.loops.len() {
                            track.loops.remove(idx);
                            crate::loops::save_library(&self.library);
                        }
                    }

                    if let Some(idx) = load_idx {
                        let saved = {
                            let track = self.library.track_for_path(&track_path);
                            track.loops.get(idx).cloned()
                        };
                        if let Some(saved) = saved {
                            self.waveform_state.loop_a_secs = Some(saved.loop_a_secs);
                            self.waveform_state.loop_b_secs = Some(saved.loop_b_secs);
                            self.waveform_state.pitch_semitones = saved.pitch_semitones;
                            self.waveform_state.tempo = saved.tempo;
                            self.waveform_play_position = saved.loop_a_secs;
                            self.waveform_state.seek_pending = Some(saved.loop_a_secs);
                            self.waveform_state.playhead_frames_after_drag = 15;

                            let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                                a_secs: saved.loop_a_secs,
                                b_secs: saved.loop_b_secs,
                            });
                            if self.waveform_has_content {
                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Seek {
                                    pos_secs: saved.loop_a_secs,
                                });
                            }
                            self.status_message = format!("Loop '{}' geladen", saved.label);
                            self.status_message_timer = 3 * 60;
                            self.active_loop_idx = Some(idx);
                            self.center_view_on_loop(panel_width);
                        }
                    }
                }

                // ── Chroma detectie knop ──
                if ui
                    .button(if self.chroma_result.is_some() {
                        "🔄 Opnieuw detecteren"
                    } else {
                        "🔍 Detecteer noten"
                    })
                    .on_hover_text("Analyseer de A-B selectie op toonhoogtes")
                    .clicked()
                {
                    let samples = &self.waveform_state.samples;
                    let sr = self.waveform_state.sample_rate;
                    let a = self.waveform_state.loop_a_secs;
                    let b = self.waveform_state.loop_b_secs;
                    if !samples.is_empty() && sr > 0 {
                        self.chroma_result = Some(detect_chroma(samples, sr, a, b));
                        if let Some(chroma) = self.chroma_result {
                            let (note, conf) = chroma.peak();
                            let name = Chroma::note_name(note);
                            self.status_message = format!(
                                "🔍 Meest waarschijnlijke noot: {} ({:.0}% zeker)",
                                name,
                                conf * 100.0
                            );
                            self.status_message_timer = 5 * 60;
                        }
                    }
                }

                // ── Chroma visualisatie ──
                if let Some(chroma) = self.chroma_result {
                    ui.separator();
                    ui.label(RichText::new("Toonhoogtes (chroma)").size(12.0).strong());
                    let bar_max_width = ui.available_width().min(300.0);
                    for i in 0..12 {
                        let val = chroma.0[i];
                        if val < 0.01 {
                            continue;
                        }
                        let bar_width = bar_max_width * val;
                        let name = Chroma::note_name(i);
                        let name_nl = Chroma::note_name_nl(i);
                        let (r, g, b) = match i % 12 {
                            0 | 2 | 4 | 5 | 7 | 9 | 11 => (220, 180, 50), // witte toetsen
                            _ => (100, 100, 100),                         // zwarte toetsen
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:>3} ({})", name, name_nl))
                                    .size(13.0)
                                    .color(Color32::from_rgb(r, g, b)),
                            );
                            let _ = egui::Frame::none().fill(Color32::from_rgb(r, g, b)).show(
                                ui,
                                |ui| {
                                    ui.set_min_size(egui::vec2(bar_width.max(2.0), 12.0));
                                },
                            );
                        });
                    }
                    if let Some((note, conf)) = chroma.compact(0.2).first().copied() {
                        ui.label(
                            RichText::new(format!(
                                "→ Meest waarschijnlijk: {} ({:.0}%)",
                                Chroma::note_name(note),
                                conf * 100.0
                            ))
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }
                }

                // ── Notities voor de actieve loop ──
                if let Some(idx) = self.active_loop_idx {
                    let track = self.library.track_for_path(&track_path);
                    if idx < track.loops.len() {
                        let label = track.loops[idx].label.clone();
                        let notes = track.loops[idx].notes.clone();
                        let mut current_notes = notes.clone();

                        ui.separator();
                        ui.label(
                            RichText::new(format!("📝 Notities: {}", label))
                                .size(12.0)
                                .strong(),
                        );
                        let resp = ui.add_sized(
                            egui::vec2(ui.available_width(), 100.0),
                            egui::TextEdit::multiline(&mut current_notes)
                                .hint_text("Akkoorden, noten, transcripties..."),
                        );
                        if resp.lost_focus() || resp.changed() {
                            if let Some(track) = self
                                .library
                                .tracks
                                .iter_mut()
                                .find(|t| t.track_path == track_path)
                            {
                                if idx < track.loops.len() {
                                    track.loops[idx].notes = current_notes;
                                    crate::loops::save_library(&self.library);
                                }
                            }
                        }
                    }
                }
            } // end if let Some(path)
        }); // end CentralPanel.show()

        // ── Alle Tracks bibliotheek (popup) ──
        if self.show_loop_library {
            egui::Window::new("📚 Alle Tracks")
                .id(egui::Id::new("loop_library_window"))
                .resizable(true)
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    if self.library.tracks.is_empty() {
                        ui.label("Nog geen tracks. Laad een audiobestand en maak loops.");
                    } else {
                        let mut delete_loop_op: Option<(usize, usize)> = None;
                        let mut load_loop_op: Option<(usize, usize)> = None;

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (ti, track) in self.library.tracks.iter().enumerate() {
                                let has_notes = track.loops.iter().any(|l| !l.notes.is_empty());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("🎵 {}", track.label))
                                            .size(14.0)
                                            .strong(),
                                    );
                                    if has_notes {
                                        ui.label(
                                            RichText::new("📝").size(14.0).color(Color32::GRAY),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("▶").clicked() {
                                                load_loop_op = Some((ti, 0)); // load track, eerste loop
                                            }
                                        },
                                    );
                                });

                                // Sub-lijst loops
                                if !track.loops.is_empty() {
                                    for (li, saved) in track.loops.iter().enumerate() {
                                        ui.indent("loops", |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&saved.label).size(12.0));
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{:02}:{:02} → {:02}:{:02}",
                                                        (saved.loop_a_secs / 60.0) as u32,
                                                        saved.loop_a_secs as u32 % 60,
                                                        (saved.loop_b_secs / 60.0) as u32,
                                                        saved.loop_b_secs as u32 % 60,
                                                    ))
                                                    .size(11.0)
                                                    .color(Color32::GRAY),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui.small_button("❌").clicked() {
                                                            delete_loop_op = Some((ti, li));
                                                        }
                                                        if ui.small_button("▶").clicked() {
                                                            load_loop_op = Some((ti, li));
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                    }
                                } else {
                                    ui.indent("loops", |ui| {
                                        ui.label(
                                            RichText::new("  (geen loops)")
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                    });
                                }
                                ui.separator();
                            }
                        });

                        // ── Verwerk operaties buiten de iterator ──
                        if let Some((ti, li)) = delete_loop_op {
                            if ti < self.library.tracks.len() {
                                self.library.tracks[ti].loops.remove(li);
                                crate::loops::save_library(&self.library);
                            }
                        }

                        if let Some((ti, li)) = load_loop_op {
                            // Clone eerst alle data die we nodig hebben
                            let (track_path, saved) = {
                                let track = &self.library.tracks[ti];
                                let path = track.track_path.clone();
                                let saved = track.loops.get(li).cloned();
                                (path, saved)
                            };

                            if let Some(saved) = saved {
                                let track_changed =
                                    self.waveform_state.path.as_deref() != Some(&track_path);

                                if track_changed {
                                    if self.waveform_is_playing {
                                        let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                                        self.waveform_is_playing = false;
                                    }
                                    self.load_file(&track_path);
                                    self.waveform_has_content = false;
                                }

                                self.waveform_state.loop_a_secs = Some(saved.loop_a_secs);
                                self.waveform_state.loop_b_secs = Some(saved.loop_b_secs);
                                self.waveform_state.pitch_semitones = saved.pitch_semitones;
                                self.waveform_state.tempo = saved.tempo;
                                self.waveform_play_position = saved.loop_a_secs;
                                self.waveform_state.seek_pending = Some(saved.loop_a_secs);
                                self.waveform_state.playhead_frames_after_drag = 15;

                                self.center_view_on_loop(800.0);

                                if self.waveform_is_playing {
                                    let _ = self
                                        .waveform_cmd_tx
                                        .send(WaveformCommand::SetPitch(saved.pitch_semitones));
                                    let _ = self
                                        .waveform_cmd_tx
                                        .send(WaveformCommand::SetTempo(saved.tempo));
                                    let _ =
                                        self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                                            a_secs: saved.loop_a_secs,
                                            b_secs: saved.loop_b_secs,
                                        });
                                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Seek {
                                        pos_secs: saved.loop_a_secs,
                                    });
                                }

                                // Zet actieve loop voor notities (alleen als zelfde track)
                                if !track_changed {
                                    self.active_loop_idx = Some(li);
                                }

                                self.center_view_on_loop(800.0);

                                self.status_message = format!("Loop '{}' geladen", saved.label);
                                self.status_message_timer = 3 * 60;
                            }
                        }
                    }
                });
        }

        // ── Shortcut Editor Window ──
        if self.show_shortcut_editor {
            egui::Window::new("⌨ Shortcut Editor")
                .id(egui::Id::new("shortcut_editor_window"))
                .resizable(true)
                .default_size([550.0, 600.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Klik op een actie en druk op een nieuwe toets om te wijzigen.");
                        if ui.button("🔄 Reset alles naar defaults").clicked() {
                            if let Err(e) = self.shortcuts.reset_all() {
                                self.status_message = format!("Fout: {}", e);
                            } else {
                                self.status_message =
                                    "Alle shortcuts gereset naar defaults".to_string();
                            }
                            self.status_message_timer = 3 * 60;
                        }
                    });
                    ui.separator();

                    // Groepeer per categorie
                    let categories = ["Playback", "Loop", "Markers", "View", "File"];
                    for category in categories {
                        ui.heading(category);
                        for action in ShortcutAction::all()
                            .iter()
                            .filter(|a| a.category() == category)
                        {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(action.display_name()).size(13.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("⟲").clicked() {
                                            let _ = self.shortcuts.reset_action(*action);
                                        }

                                        let binding = self
                                            .shortcuts
                                            .binding_for(*action)
                                            .map(|b| b.display())
                                            .unwrap_or_else(|| "—".to_string());

                                        let is_listening =
                                            self.listening_for_action == Some(*action);
                                        let btn_text = if is_listening {
                                            RichText::new("... druk toets ...")
                                                .color(Color32::YELLOW)
                                        } else {
                                            RichText::new(binding)
                                                .color(Color32::from_rgb(200, 200, 60))
                                        };

                                        if ui.button(btn_text).clicked() {
                                            self.listening_for_action = Some(*action);
                                        }
                                    },
                                );
                            });
                        }
                        ui.separator();
                    }
                });
        }
    }
}

/// Helper om een shortcut-key/uitleg regel te tekenen.
fn shortcut_row(ui: &mut egui::Ui, key: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(200, 200, 60)),
        );
        ui.label(
            RichText::new(description)
                .size(13.0)
                .color(Color32::LIGHT_GRAY),
        );
    });
}
