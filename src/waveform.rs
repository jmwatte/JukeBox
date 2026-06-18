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
pub fn render_waveform(
    ui: &mut egui::Ui,
    state: &mut WaveformState,
    now_playing_position: Option<f32>,
) {
    let width = ui.available_width().max(100.0);
    let height = 200.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());

    let painter = ui.painter();
    let center_y = rect.center().y;

    if state.samples.is_empty() {
        // Geen samples geladen
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Geen waveform data (druk op 0 om een track te openen)",
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::GRAY,
        );
        return;
    }

    let total_samples = state.samples.len();
    let sample_rate = state.sample_rate;

    // Bepaal zichtbaar tijdsbereik
    let visible_secs = width / state.zoom;
    let start_sec = state.scroll_offset;
    let end_sec = (start_sec + visible_secs).min(state.duration_secs);

    let start_sample = (start_sec * sample_rate as f32) as usize;
    let end_sample = (end_sec * sample_rate as f32) as usize;
    let visible_samples = end_sample.saturating_sub(start_sample);

    if visible_samples == 0 {
        return;
    }

    let samples_per_pixel = (visible_samples as f32 / width).ceil() as usize;

    // Teken achtergrond
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

    // Teken tijdschaal
    let time_interval = if state.zoom < 20.0 {
        30.0 // elke 30 seconden
    } else if state.zoom < 50.0 {
        10.0 // elke 10 seconden
    } else if state.zoom < 100.0 {
        5.0 // elke 5 seconden
    } else {
        1.0 // elke seconde
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

    // Teken waveform lijnen
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

    // Teken A-B loop gebied (lichtblauw tussen A en B)
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

            // A marker (groen)
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

            // B marker (rood)
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

    // Teken huidige positie-indicator (als de track speelt)
    if let Some(pos) = now_playing_position {
        if pos >= start_sec && pos <= end_sec {
            let pos_x = rect.left() + (pos - start_sec) * state.zoom;
            painter.line_segment(
                [
                    egui::pos2(pos_x, rect.top()),
                    egui::pos2(pos_x, rect.bottom()),
                ],
                (1.5, egui::Color32::from_rgb(255, 200, 50)),
            );
        }
    }

    // Muis interactie: scrollen (slepen) en zoomen (wiel)
    if response.hovered() {
        // Zoom met Ctrl+Wiel
        ui.ctx().input(|i| {
            let scroll = i.raw_scroll_delta.y;
            if scroll != 0.0 {
                // Zoom rond muispositie
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

                // Houd muispositie vast
                let new_scroll = mouse_sec - (mouse_x - rect.left()) / new_zoom;
                state.scroll_offset = new_scroll.max(0.0);
                state.zoom = new_zoom;
            }
        });
    }

    // Slepen om te scrollen
    if response.dragged_by(egui::PointerButton::Primary) {
        let drag_delta = response.drag_delta();
        state.scroll_offset -= drag_delta.x / state.zoom;
        state.scroll_offset = state.scroll_offset.max(0.0);
    }
}
