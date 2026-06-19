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
    pub pitch_semitones: f32,
    pub tempo: f32,
    pub error: Option<String>,
    pub dragging_loop_region: bool,
    pub dragging_playhead: bool,
    pub playhead_drag_secs: Option<f32>,
    pub playhead_frames_after_drag: u32,
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
            pitch_semitones: 0.0,
            tempo: 1.0,
            error: None,
            dragging_loop_region: false,
            dragging_playhead: false,
            playhead_drag_secs: None,
            playhead_frames_after_drag: 0,
        }
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

    //let samples_per_pixel = (visible_samples as f32 / width).ceil() as usize;

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
    //  FIX: Draw waveform based on exact pixel-to-time mapping to prevent drift
    let width_px = width as usize;
    for pixel_x in 0..width_px {
        // Calculate exact time range for this specific pixel
        let t_start = start_sec + (pixel_x as f32) / state.zoom;
        let t_end = start_sec + ((pixel_x + 1) as f32) / state.zoom;

        let sample_start = (t_start * sample_rate as f32) as usize;
        let sample_end = (t_end * sample_rate as f32) as usize;

        // Clamp to valid sample range
        let sample_start = sample_start.min(total_samples);
        let sample_end = sample_end.min(total_samples);

        if sample_start >= total_samples || sample_start >= sample_end {
            continue;
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

        let x = rect.left() + pixel_x as f32;
        let p1 = egui::pos2(x, center_y + min_val * height * 0.45);
        let p2 = egui::pos2(x, center_y + max_val * height * 0.45);

        painter.line_segment([p1, p2], (1.0, egui::Color32::from_gray(160)));
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

    // Huidige positie-indicator + interactie (playhead verslepen)
    // Tijdens drag én tot 3 frames na loslaten: toon versleepte positie.
    // Zo krijgt de player-thread de tijd om PositionUpdate te sturen.
    let render_pos = if state.playhead_frames_after_drag > 0 {
        state.playhead_drag_secs.or(now_playing_position)
    } else {
        now_playing_position
    };

    // Aftellen: na 3 frames wissen we de drag-positie
    if state.playhead_frames_after_drag > 0 {
        state.playhead_frames_after_drag -= 1;
        if state.playhead_frames_after_drag == 0 {
            state.playhead_drag_secs = None;
        }
    }

    if let Some(pos) = render_pos {
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

            // --- Driehoekjes boven en onder voor grip ---
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

            // --- Playhead drag detectie ---
            if let Some(_actual_pos) = now_playing_position {
                let strip_half = 10.0;
                if response.drag_started() {
                    if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                        let dx = (mouse_pos.x - pos_x).abs();
                        let dy_top = (mouse_pos.y - rect.top()).abs();
                        let dy_bot = (mouse_pos.y - rect.bottom()).abs();
                        let in_strip = dx <= strip_half;
                        let in_triangles =
                            dx <= tri_size && (dy_top <= tri_height || dy_bot <= tri_height);
                        state.dragging_playhead = in_strip || in_triangles;
                    }
                }
                if response.drag_stopped() {
                    state.dragging_playhead = false;
                    // Blijf nog 3 frames op de versleepte positie
                    // zodat de PositionUpdate van de player kan arriveren
                    if state.playhead_drag_secs.is_some() {
                        state.playhead_frames_after_drag = 3;
                    }
                }
            } else {
                state.dragging_playhead = false;
                state.playhead_drag_secs = None;
                state.playhead_frames_after_drag = 0;
            }
        } else {
            state.dragging_playhead = false;
            state.playhead_drag_secs = None;
            state.playhead_frames_after_drag = 0;
        }
    } else {
        state.dragging_playhead = false;
        state.playhead_drag_secs = None;
        state.playhead_frames_after_drag = 0;
    }

    // Playhead verslepen (render positie updaten, geen seek command tijdens drag)
    if state.dragging_playhead && response.dragged_by(egui::PointerButton::Primary) {
        if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let seek_pos = ((mouse_pos.x - rect.left()) / state.zoom + start_sec)
                .clamp(0.0, state.duration_secs);
            state.playhead_drag_secs = Some(seek_pos);
            state.playhead_frames_after_drag = 3; // reset teller
        }
    }

    // Enkelklik op waveform: seek naar die positie
    if response.clicked() {
        if let Some(sec) = mouse_sec {
            seek_action = Some(sec.clamp(0.0, state.duration_secs));
        }
    }

    // Playhead drag losgelaten: seek naar de versleepte positie
    if response.drag_stopped() && state.dragging_playhead {
        if let Some(sec) = state.playhead_drag_secs {
            seek_action = Some(sec);
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
            let scroll = i.raw_scroll_delta.y;
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

    // --- Loop-regio slepen: verplaats de hele A-B loop ---
    // (alleen als playhead niet wordt versleept)
    if !state.dragging_playhead {
        if let (Some(a), Some(b)) = (state.loop_a_secs, state.loop_b_secs) {
            if b > a {
                if response.drag_started() {
                    if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                        let mouse_sec = (mouse_pos.x - rect.left()) / state.zoom + start_sec;
                        state.dragging_loop_region = mouse_sec >= a && mouse_sec <= b;
                    }
                }
                if response.drag_stopped() {
                    state.dragging_loop_region = false;
                }
            } else {
                state.dragging_loop_region = false;
            }
        } else {
            state.dragging_loop_region = false;
        }
    }

    // Versleep de hele loop (behoud lengte)
    if state.dragging_loop_region && response.dragged_by(egui::PointerButton::Primary) {
        let drag_delta = response.drag_delta();
        let delta_secs = drag_delta.x / state.zoom;
        if let (Some(a), Some(b)) = (state.loop_a_secs, state.loop_b_secs) {
            let len = b - a;
            let new_a = (a + delta_secs).clamp(0.0, state.duration_secs - len);
            state.loop_a_secs = Some(new_a);
            state.loop_b_secs = Some(new_a + len);
            loop_changed = true;
        }
    }

    // Slepen op waveform (scrol) — alleen als we niet op marker, playhead of loop-regio slepen
    if response.dragged_by(egui::PointerButton::Primary)
        && !loop_changed
        && !state.dragging_playhead
        && !state.dragging_loop_region
    {
        let drag_delta = response.drag_delta();
        state.scroll_offset -= drag_delta.x / state.zoom;
        state.scroll_offset = state.scroll_offset.max(0.0);
    }

    (loop_changed, seek_action)
}
