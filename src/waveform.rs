use eframe::egui;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

/// State voor de waveform-editor
#[derive(Clone)]
pub struct WaveformState {
    pub path: Option<String>,
    pub samples: Vec<f32>, // PCM samples (mono, gemixt)
    pub sample_rate: u32,
    pub duration_secs: f32,
    pub zoom: f32,          // pixels per second
    pub scroll_offset: f32, // scroll offset in seconds
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
    pub error: Option<String>,
    /// Breedte van het waveform-paneel (px) van de laatste frame, nodig om de
    /// view op een loop te kunnen centreren via toetsenbord.
    pub panel_width: f32,
}

impl Default for WaveformState {
    fn default() -> Self {
        Self {
            path: None,
            samples: Vec::new(),
            sample_rate: 44100,
            duration_secs: 0.0,
            zoom: 50.0, // 50 pixels per seconde (default)
            scroll_offset: 0.0,
            loop_a_secs: None,
            loop_b_secs: None,
            error: None,
            panel_width: 800.0,
        }
    }
}

/// Toetsenbord-bewerkingen voor de A-B loop. Elke functie geeft `true` terug
/// als de loop daadwerkelijk veranderd is (dan moet de player gesynchroniseerd
/// worden). De stappen volgen LoopMachine: markers 0,05s, playhead 0,20s.
impl WaveformState {
    /// Verschuif marker A met `delta` seconden (±0,05s), beperkt tot de track
    /// en zodanig dat A < B blijft.
    pub fn nudge_a(&mut self, delta: f32) -> bool {
        let Some(a) = self.loop_a_secs else {
            return false;
        };
        let step = delta.abs();
        let mut new_a = (a + delta).clamp(0.0, self.duration_secs);
        if let Some(b) = self.loop_b_secs {
            if new_a >= b {
                new_a = (b - step).max(0.0);
            }
        }
        self.loop_a_secs = Some(new_a);
        (new_a - a).abs() > 0.0001
    }

    /// Verschuif marker B met `delta` seconden (±0,05s), beperkt tot de track
    /// en zodanig dat A < B blijft.
    pub fn nudge_b(&mut self, delta: f32) -> bool {
        let Some(b) = self.loop_b_secs else {
            return false;
        };
        let step = delta.abs();
        let mut new_b = (b + delta).clamp(0.0, self.duration_secs);
        if let Some(a) = self.loop_a_secs {
            if new_b <= a {
                new_b = (a + step).min(self.duration_secs);
            }
        }
        self.loop_b_secs = Some(new_b);
        (new_b - b).abs() > 0.0001
    }

    /// Verplaats de hele loop één eigen lengte naar links (lengte blijft gelijk).
    pub fn shift_loop_left(&mut self) -> bool {
        let (Some(a), Some(b)) = (self.loop_a_secs, self.loop_b_secs) else {
            return false;
        };
        if b <= a {
            return false;
        }
        let len = b - a;
        let new_a = (a - len).max(0.0);
        self.loop_a_secs = Some(new_a);
        self.loop_b_secs = Some(new_a + len);
        (new_a - a).abs() > 0.0001
    }

    /// Verplaats de hele loop één eigen lengte naar rechts (lengte blijft gelijk).
    pub fn shift_loop_right(&mut self) -> bool {
        let (Some(a), Some(b)) = (self.loop_a_secs, self.loop_b_secs) else {
            return false;
        };
        if b <= a {
            return false;
        }
        let len = b - a;
        let new_b = (b + len).min(self.duration_secs);
        self.loop_a_secs = Some(new_b - len);
        self.loop_b_secs = Some(new_b);
        (new_b - b).abs() > 0.0001
    }

    /// Verdubbel de looplengte naar rechts (A blijft staan).
    pub fn double_loop(&mut self) -> bool {
        let (Some(a), Some(b)) = (self.loop_a_secs, self.loop_b_secs) else {
            return false;
        };
        if b <= a {
            return false;
        }
        let new_b = (a + (b - a) * 2.0).min(self.duration_secs);
        self.loop_b_secs = Some(new_b);
        (new_b - b).abs() > 0.0001
    }

    /// Halveer de looplengte naar rechts (A blijft staan).
    pub fn halve_loop(&mut self) -> bool {
        let (Some(a), Some(b)) = (self.loop_a_secs, self.loop_b_secs) else {
            return false;
        };
        if b <= a {
            return false;
        }
        let new_b = a + (b - a) / 2.0;
        if new_b <= a {
            return false;
        }
        self.loop_b_secs = Some(new_b);
        true
    }

    /// Centreer de viewport op de A-B loop: zoom zodanig dat de loop + marge
    /// past en scroll naar het midden van de loop.
    pub fn center_view_on_loop(&mut self) {
        let (Some(a), Some(b)) = (self.loop_a_secs, self.loop_b_secs) else {
            return;
        };
        if b <= a || self.panel_width <= 0.0 || self.duration_secs <= 0.0 {
            return;
        }
        let loop_width = b - a;
        let target_zoom = (self.panel_width * 0.6 / loop_width).clamp(5.0, 5000.0);
        self.zoom = target_zoom;
        let visible_secs = self.panel_width / self.zoom;
        let mid = (a + b) / 2.0;
        let max_scroll = (self.duration_secs - visible_secs).max(0.0);
        self.scroll_offset = (mid - visible_secs / 2.0).clamp(0.0, max_scroll);
    }

    /// Centreer de viewport op een positie (seconden), zonder zoom te wijzigen.
    pub fn center_view_on_pos(&mut self, pos_secs: f32) {
        if self.panel_width <= 0.0 || self.zoom <= 0.0 {
            return;
        }
        let visible_secs = self.panel_width / self.zoom;
        let max_scroll = (self.duration_secs - visible_secs).max(0.0);
        self.scroll_offset = (pos_secs - visible_secs / 2.0).clamp(0.0, max_scroll);
    }
}

/// Decodeer een audiobestand naar mono PCM samples (f32).
/// Geeft (samples, sample_rate, duration_secs) terug.
pub fn decode_audio(path: &str) -> Result<(Vec<f32>, u32, f32), String> {
    let path_obj = Path::new(path);

    // 1. Open bestand
    let file = File::open(&path_obj).map_err(|e| format!("Kan bestand niet openen: {}", e))?;

    // 2. Maak MediaSourceStream
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // 3. Bepaal extensie voor hint
    let ext = path_obj
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut hint = Hint::new();
    hint.with_extension(&ext);

    // 4. Probeer formaat te detecteren
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| format!("Kan formaat niet detecteren: {}", e))?;

    let mut format = probed.format;

    // 5. Zoek de audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.sample_rate.is_some())
        .ok_or_else(|| "Geen audio track gevonden".to_string())?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;

    // 6. Maak decoder
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Kan decoder niet maken: {}", e))?;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);

    // 7. Decodeer packets naar samples
    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(pkt) => pkt,
            Err(symphonia::core::errors::Error::IoError(ref err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                // Skip decode fouten, ga door met volgende packet
                continue;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                // Mix naar mono en converteer naar f32
                let num_frames = decoded.frames();
                let num_channels = decoded.spec().channels.count();

                // Gebruik SampleBuffer om naar f32 te converteren
                let mut sample_buf =
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                sample_buf.copy_interleaved_ref(decoded);

                let buf = sample_buf.samples();

                // Mix naar mono: gemiddelde van kanalen
                for frame in 0..num_frames {
                    let mut frame_sum = 0.0_f32;
                    for ch in 0..num_channels {
                        let idx = frame * num_channels + ch;
                        if idx < buf.len() {
                            frame_sum += buf[idx];
                        }
                    }
                    samples.push(frame_sum / num_channels as f32);
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                continue;
            }
            Err(_) => break,
        }
    }

    let duration_secs = samples.len() as f32 / sample_rate as f32;

    Ok((samples, sample_rate, duration_secs))
}

/// Teken de waveform in een egui UI.
/// Geeft `(loop_changed, seek_to)` terug:
/// - loop_changed: Of de A-B loop markers zijn gewijzigd
/// - seek_to: Optionele positie (seconden) om naartoe te seeken (playhead drag)
pub fn render_waveform(
    ui: &mut egui::Ui,
    state: &mut WaveformState,
    now_playing_position: Option<f32>,
) -> (bool, Option<f32>) {
    let width = ui.available_width().max(100.0);
    // Onthoud de breedte zodat de view via toetsenbord op de loop gecentreerd
    // kan worden (center_view_on_loop/center_view_on_pos).
    state.panel_width = width;
    let height = 200.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());

    let id_base = ui.id();
    let painter = ui.painter();
    let center_y = rect.center().y;

    let mut loop_changed = false;
    let mut seek_action: Option<f32> = None;

    if state.samples.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Geen waveform data (druk op 0 om een track te openen)",
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::GRAY,
        );
        return (false, None);
    }

    let total_samples = state.samples.len();
    let sample_rate = state.sample_rate;

    let visible_secs = width / state.zoom;
    let start_sec = state.scroll_offset;
    let end_sec = (start_sec + visible_secs).min(state.duration_secs);

    let start_sample = (start_sec * sample_rate as f32) as usize;
    let end_sample = (end_sec * sample_rate as f32) as usize;
    let visible_samples = end_sample.saturating_sub(start_sample);

    if visible_samples == 0 {
        return (false, None);
    }

    let samples_per_pixel = (visible_samples as f32 / width).ceil() as usize;

    // Achtergrond
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

    // Tijdschaal
    let time_interval = if state.zoom < 20.0 {
        30.0
    } else if state.zoom < 50.0 {
        10.0
    } else if state.zoom < 100.0 {
        5.0
    } else {
        1.0
    };

    let first_mark = (start_sec / time_interval).ceil() * time_interval;
    let mut t = first_mark;
    while t < end_sec {
        let x = rect.left() + (t - start_sec) * state.zoom;
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [
                    egui::pos2(x, rect.bottom() - 15.0),
                    egui::pos2(x, rect.bottom()),
                ],
                (1.0, egui::Color32::from_gray(80)),
            );
            let mins = (t / 60.0) as u32;
            let secs = (t as u32) % 60;
            painter.text(
                egui::pos2(x, rect.bottom() - 2.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{}:{:02}", mins, secs),
                egui::TextStyle::Small.resolve(ui.style()),
                egui::Color32::from_gray(120),
            );
        }
        t += time_interval;
    }

    // Waveform lijnen
    let mut x = rect.left();
    let mut pixel_idx = 0usize;
    while x <= rect.right() && pixel_idx * samples_per_pixel < visible_samples {
        let sample_start = start_sample + pixel_idx * samples_per_pixel;
        let sample_end = (sample_start + samples_per_pixel).min(total_samples);

        if sample_start >= total_samples {
            break;
        }

        let mut min_val = 0.0_f32;
        let mut max_val = 0.0_f32;
        for s in sample_start..sample_end {
            let val = state.samples[s];
            if val < min_val {
                min_val = val;
            }
            if val > max_val {
                max_val = val;
            }
        }

        let p1 = egui::pos2(x, center_y + min_val * height * 0.45);
        let p2 = egui::pos2(x, center_y + max_val * height * 0.45);

        painter.line_segment([p1, p2], (1.0, egui::Color32::from_gray(160)));

        x += 1.0;
        pixel_idx += 1;
    }

    // ---- Interactieve A-B markers ----
    // Huidige muispositie in seconden (voor click-to-place)
    let mouse_sec = ui.ctx().input(|i| {
        i.pointer
            .hover_pos()
            .map(|p| (p.x - rect.left()) / state.zoom + start_sec)
    });

    // Teken A-B highlight gebied en markers (vóór interactie, zodat interactie eroverheen kan)
    let marker_half_width = 6.0; // hit area half-width

    if let (Some(a), Some(b)) = (state.loop_a_secs, state.loop_b_secs) {
        if b > a && b > start_sec && a < end_sec {
            let a_x = rect.left() + (a - start_sec) * state.zoom;
            let b_x = rect.left() + (b - start_sec) * state.zoom;
            let a_x_clamped = a_x.max(rect.left());
            let b_x_clamped = b_x.min(rect.right());

            if b_x_clamped > a_x_clamped {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(a_x_clamped, rect.top()),
                        egui::pos2(b_x_clamped, rect.bottom()),
                    ),
                    0.0,
                    egui::Color32::from_rgba_premultiplied(100, 150, 255, 40),
                );
            }

            // A marker tekenen
            if a_x >= rect.left() && a_x <= rect.right() {
                painter.line_segment(
                    [egui::pos2(a_x, rect.top()), egui::pos2(a_x, rect.bottom())],
                    (2.0, egui::Color32::from_rgb(80, 255, 80)),
                );
                painter.text(
                    egui::pos2(a_x, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    "A",
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::from_rgb(80, 255, 80),
                );
            }

            // B marker tekenen
            if b_x >= rect.left() && b_x <= rect.right() {
                painter.line_segment(
                    [egui::pos2(b_x, rect.top()), egui::pos2(b_x, rect.bottom())],
                    (2.0, egui::Color32::from_rgb(255, 80, 80)),
                );
                painter.text(
                    egui::pos2(b_x, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    "B",
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::from_rgb(255, 80, 80),
                );
            }
        }
    }

    // Sleepbare A marker interactie
    if let Some(a) = state.loop_a_secs {
        let a_x = rect.left() + (a - start_sec) * state.zoom;
        // Alleen interactief als zichtbaar
        if a_x >= rect.left() - marker_half_width && a_x <= rect.right() + marker_half_width {
            let marker_rect = egui::Rect::from_center_size(
                egui::pos2(a_x.clamp(rect.left(), rect.right()), rect.center().y),
                egui::vec2(marker_half_width * 2.0, rect.height()),
            );
            let marker_id = id_base.with("drag_a");
            let marker_response = ui.interact(marker_rect, marker_id, egui::Sense::drag());

            if marker_response.dragged() {
                if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                    let new_a = ((pos.x - rect.left()) / state.zoom + start_sec)
                        .clamp(0.0, state.duration_secs);
                    state.loop_a_secs = Some(new_a);
                    loop_changed = true;
                }
            }
        }
    }

    // Sleepbare B marker interactie
    if let Some(b) = state.loop_b_secs {
        let b_x = rect.left() + (b - start_sec) * state.zoom;
        if b_x >= rect.left() - marker_half_width && b_x <= rect.right() + marker_half_width {
            let marker_rect = egui::Rect::from_center_size(
                egui::pos2(b_x.clamp(rect.left(), rect.right()), rect.center().y),
                egui::vec2(marker_half_width * 2.0, rect.height()),
            );
            let marker_id = id_base.with("drag_b");
            let marker_response = ui.interact(marker_rect, marker_id, egui::Sense::drag());

            if marker_response.dragged() {
                if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                    let new_b = ((pos.x - rect.left()) / state.zoom + start_sec)
                        .clamp(0.0, state.duration_secs);
                    state.loop_b_secs = Some(new_b);
                    loop_changed = true;
                }
            }
        }
    }

    // Als A en B beide gezet zijn, zorg dat A < B
    if let (Some(a), Some(b)) = (state.loop_a_secs, state.loop_b_secs) {
        if b < a {
            // Verwissel ze
            state.loop_a_secs = Some(b);
            state.loop_b_secs = Some(a);
            loop_changed = true;
        }
    }

    // Huidige positie-indicator (playhead)
    if let Some(pos) = now_playing_position {
        if pos >= start_sec && pos <= end_sec {
            let pos_x = rect.left() + (pos - start_sec) * state.zoom;

            // --- Playhead lijn tekenen ---
            painter.line_segment(
                [
                    egui::pos2(pos_x, rect.top()),
                    egui::pos2(pos_x, rect.bottom()),
                ],
                (2.0, egui::Color32::from_rgb(255, 200, 50)),
            );

            // --- Driehoekjes boven en onder ---
            let tri_size = 7.0;
            let tri_height = 10.0;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(pos_x, rect.top()),
                    egui::pos2(pos_x - tri_size, rect.top() + tri_height),
                    egui::pos2(pos_x + tri_size, rect.top() + tri_height),
                ],
                egui::Color32::from_rgb(255, 200, 50),
                egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 150, 20)),
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(pos_x, rect.bottom()),
                    egui::pos2(pos_x - tri_size, rect.bottom() - tri_height),
                    egui::pos2(pos_x + tri_size, rect.bottom() - tri_height),
                ],
                egui::Color32::from_rgb(255, 200, 50),
                egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 150, 20)),
            ));
        }
    }

    // Enkelklik op waveform: seek naar die positie
    if response.clicked() {
        if let Some(sec) = mouse_sec {
            seek_action = Some(sec.clamp(0.0, state.duration_secs));
        }
    }

    // Rechterklik op waveform: wis loop
    if response.secondary_clicked() {
        state.loop_a_secs = None;
        state.loop_b_secs = None;
        loop_changed = true;
    }

    // Dubbelklik: zet A op muispositie (als A nog niet gezet is)
    // Shift+dubbelklik: zet B
    if response.double_clicked() {
        if let Some(sec) = mouse_sec {
            let sec = sec.clamp(0.0, state.duration_secs);
            if ui.ctx().input(|i| i.modifiers.shift) {
                state.loop_b_secs = Some(sec);
                // Als A niet gezet is, default naar begin
                if state.loop_a_secs.is_none() {
                    state.loop_a_secs = Some(0.0);
                }
            } else {
                state.loop_a_secs = Some(sec);
            }
            loop_changed = true;
        }
    }

    // Zoom met muiswiel
    if response.hovered() {
        ui.ctx().input(|i| {
            let scroll = i.smooth_scroll_delta.y;
            if scroll != 0.0 {
                let mouse_x = i
                    .pointer
                    .hover_pos()
                    .map(|p| p.x)
                    .unwrap_or(rect.center().x);
                let mouse_sec = if state.zoom > 0.0 {
                    (mouse_x - rect.left()) / state.zoom + start_sec
                } else {
                    0.0
                };

                let zoom_factor = if scroll > 0.0 { 1.15 } else { 1.0 / 1.15 };
                let new_zoom = (state.zoom * zoom_factor).clamp(5.0, 5000.0);

                let new_scroll = mouse_sec - (mouse_x - rect.left()) / new_zoom;
                state.scroll_offset = new_scroll.max(0.0);
                state.zoom = new_zoom;
            }
        });
    }

    // Slepen op de waveform: met Ctrl verplaats je de hele loop (beide markers),
    // zonder Ctrl scrol je door de track.
    if response.dragged_by(egui::PointerButton::Primary) {
        let ctrl_held = ui.ctx().input(|i| i.modifiers.ctrl);
        if ctrl_held {
            if let (Some(a), Some(b)) = (state.loop_a_secs, state.loop_b_secs) {
                if b > a {
                    let len = b - a;
                    let delta_secs = response.drag_delta().x / state.zoom;
                    let max_a = (state.duration_secs - len).max(0.0);
                    let new_a = (a + delta_secs).clamp(0.0, max_a);
                    state.loop_a_secs = Some(new_a);
                    state.loop_b_secs = Some(new_a + len);
                    loop_changed = true;
                }
            }
        } else if !loop_changed {
            let drag_delta = response.drag_delta();
            state.scroll_offset -= drag_delta.x / state.zoom;
            state.scroll_offset = state.scroll_offset.max(0.0);
        }
    }

    (loop_changed, seek_action)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WaveformState met een fictieve loop van 10.0–20.0s in een track van 60s.
    fn state_with_loop() -> WaveformState {
        WaveformState {
            path: Some("test.flac".to_string()),
            samples: vec![0.0; 1000],
            sample_rate: 44100,
            duration_secs: 60.0,
            zoom: 50.0,
            scroll_offset: 0.0,
            loop_a_secs: Some(10.0),
            loop_b_secs: Some(20.0),
            error: None,
            panel_width: 800.0,
        }
    }

    #[test]
    fn nudge_a_moves_marker_within_bounds() {
        let mut s = state_with_loop();
        assert!(s.nudge_a(0.05));
        assert_eq!(s.loop_a_secs, Some(10.05));
        assert!(s.nudge_a(-0.05));
        assert_eq!(s.loop_a_secs, Some(10.0));
    }

    #[test]
    fn nudge_a_cannot_cross_b() {
        let mut s = state_with_loop();
        // 250 stappen van +0,05s → 10.0 + 12.5s, maar B (20.0s) blokkeert op 19.95
        for _ in 0..250 {
            s.nudge_a(0.05);
        }
        let a = s.loop_a_secs.unwrap();
        assert!(a < s.loop_b_secs.unwrap());
        assert!((a - 19.95).abs() < 0.001, "A={a}");
    }

    #[test]
    fn nudge_b_cannot_cross_a() {
        let mut s = state_with_loop();
        // 250 stappen van -0,05s → 20.0 - 12.5s, maar A (10.0s) blokkeert op 10.05
        for _ in 0..250 {
            s.nudge_b(-0.05);
        }
        let b = s.loop_b_secs.unwrap();
        assert!(b > s.loop_a_secs.unwrap());
        assert!((b - 10.05).abs() < 0.001, "B={b}");
    }

    #[test]
    fn nudge_a_clamps_at_zero() {
        let mut s = state_with_loop();
        s.loop_a_secs = Some(0.03);
        assert!(s.nudge_a(-0.05));
        assert_eq!(s.loop_a_secs, Some(0.0));
    }

    #[test]
    fn nudge_without_loop_returns_false() {
        let mut s = WaveformState::default();
        s.duration_secs = 60.0;
        assert!(!s.nudge_a(0.05));
        assert!(!s.nudge_b(0.05));
    }

    #[test]
    fn shift_loop_left_moves_by_its_length() {
        let mut s = state_with_loop();
        assert!(s.shift_loop_left());
        assert_eq!(s.loop_a_secs, Some(0.0)); // 10 - 10
        assert_eq!(s.loop_b_secs, Some(10.0));
        // Nog een keer: mag niet onder 0, dus geen verandering meer
        assert!(!s.shift_loop_left());
        assert_eq!(s.loop_a_secs, Some(0.0));
    }

    #[test]
    fn shift_loop_right_moves_by_its_length() {
        let mut s = state_with_loop();
        assert!(s.shift_loop_right());
        assert_eq!(s.loop_a_secs, Some(20.0));
        assert_eq!(s.loop_b_secs, Some(30.0));
        // Aan het einde: gedeeltelijk verschoven als de track ophoudt
        s.loop_a_secs = Some(50.0);
        s.loop_b_secs = Some(55.0);
        assert!(s.shift_loop_right());
        assert_eq!(s.loop_a_secs, Some(55.0));
        assert_eq!(s.loop_b_secs, Some(60.0));
    }

    #[test]
    fn double_loop_keeps_a() {
        let mut s = state_with_loop();
        assert!(s.double_loop());
        assert_eq!(s.loop_a_secs, Some(10.0));
        assert_eq!(s.loop_b_secs, Some(30.0));
        // Beperkt door de trackduur
        let mut s2 = state_with_loop();
        s2.duration_secs = 25.0;
        assert!(s2.double_loop());
        assert_eq!(s2.loop_b_secs, Some(25.0));
    }

    #[test]
    fn halve_loop_keeps_a() {
        let mut s = state_with_loop();
        assert!(s.halve_loop());
        assert_eq!(s.loop_a_secs, Some(10.0));
        assert_eq!(s.loop_b_secs, Some(15.0));
    }

    #[test]
    fn center_view_on_loop_zooms_and_scrolls() {
        let mut s = state_with_loop();
        s.panel_width = 600.0;
        s.center_view_on_loop();
        // Zoom: 600 * 0.6 / 10 = 36 px/s
        assert!((s.zoom - 36.0).abs() < 0.01);
        // Midden van de loop (15s) gecentreerd: visible = 600/36 ≈ 16,67s
        let visible = 600.0 / s.zoom;
        let expected_scroll = (15.0 - visible / 2.0).clamp(0.0, 60.0 - visible);
        assert!((s.scroll_offset - expected_scroll).abs() < 0.01);
    }

    #[test]
    fn center_view_on_loop_requires_valid_loop() {
        let mut s = WaveformState::default();
        s.duration_secs = 60.0;
        s.panel_width = 600.0;
        // Geen loop → niets doen (geen panic, zoom ongewijzigd)
        s.center_view_on_loop();
        assert_eq!(s.zoom, 50.0);
        assert_eq!(s.scroll_offset, 0.0);
    }

    #[test]
    fn center_view_on_pos_scrolls() {
        let mut s = WaveformState::default();
        s.duration_secs = 60.0;
        s.panel_width = 600.0;
        s.zoom = 50.0;
        s.center_view_on_pos(30.0);
        // visible = 12s, scroll = 30 - 6 = 24
        assert!((s.scroll_offset - 24.0).abs() < 0.01);
    }
}
