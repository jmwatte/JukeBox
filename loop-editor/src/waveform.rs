use eframe::egui;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

/// Soort marker: bepaalt kleur en functie
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Section, // Sectie (bijv. "Intro", "Chorus") — Goud
    Measure, // Maat — Blauw
    Beat,    // Beat — Groen
}

impl MarkerKind {
    /// Kleur van het driehoekje
    pub fn color(&self) -> egui::Color32 {
        match self {
            MarkerKind::Section => egui::Color32::from_rgb(220, 180, 50), // Goud
            MarkerKind::Measure => egui::Color32::from_rgb(80, 160, 255), // Blauw
            MarkerKind::Beat => egui::Color32::from_rgb(80, 220, 120),    // Groen
        }
    }

    /// Kleur van de rand (iets donkerder)
    pub fn stroke_color(&self) -> egui::Color32 {
        match self {
            MarkerKind::Section => egui::Color32::from_rgb(180, 140, 30),
            MarkerKind::Measure => egui::Color32::from_rgb(50, 120, 200),
            MarkerKind::Beat => egui::Color32::from_rgb(50, 170, 80),
        }
    }

    /// Prefix voor automatisch gegenereerde naam
    pub fn prefix(&self) -> &'static str {
        match self {
            MarkerKind::Section => "S",
            MarkerKind::Measure => "M",
            MarkerKind::Beat => "B",
        }
    }
}
/// Een benoemde marker op een positie in de track.
#[derive(Debug, Clone)]
pub struct Marker {
    pub name: String,
    pub position_secs: f32,
    pub kind: MarkerKind,
}

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
    /// Benoemde markers (onafhankelijk van A-B selectie)
    pub markers: Vec<Marker>,
    /// Marker die momenteel wordt bewerkt (naam aanpassen)
    pub editing_marker: Option<usize>,
    /// Tijdelijke opslag voor marker-naam tijdens bewerken
    pub editing_marker_name: String,
    pub select_drag_start: Option<f32>,
    // ✅ NIEUW: Houdt bij of we wachten op de audio-thread na een seek
    pub seek_pending: Option<f32>,
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
            markers: Vec::new(),
            editing_marker: None,
            editing_marker_name: String::new(),
            select_drag_start: None,
            seek_pending: None,
        }
    }
}

/// Decodeer een audiobestand naar mono PCM samples (f32).
/// Geeft (samples, sample_rate, duration_secs) terug.
///
/// ## Geheugenbescherming
/// Bestanden groter dan 100 MB worden beperkt tot de eerste 5 minuten audio.
pub fn decode_audio(path: &str) -> Result<(Vec<f32>, u32, f32), String> {
    let path_obj = Path::new(path);

    // 0. Controleer bestandsgrootte vóór decoderen
    let file_len = std::fs::metadata(&path_obj).map(|m| m.len()).unwrap_or(0);
    let _large_file_warning = if file_len > 100_000_000 {
        Some(format!(
            "⚠ Bestand is {:.1} MB > 100 MB. Alleen eerste 5 min. gedecodeerd.",
            file_len as f64 / 1_000_000.0
        ))
    } else {
        None
    };

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

    // 7. Bepaal maximum aantal samples (5 minuten limiet voor grote bestanden)
    let max_samples = if file_len > 100_000_000 {
        let five_min_samples = 5 * 60 * sample_rate as usize;
        // Alleen limiteren als het bestand echt groot is
        let estimated_total = (file_len / 2) as usize; // ruwe schatting: ~2 bytes per sample
        if estimated_total > five_min_samples {
            eprintln!(
                "[waveform] {} > 100MB, decodeert max {} samples (5 min)",
                path_obj.display(),
                five_min_samples
            );
            five_min_samples
        } else {
            usize::MAX
        }
    } else {
        usize::MAX
    };

    // 8. Decodeer packets naar samples
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
                    if samples.len() >= max_samples {
                        break;
                    }
                    let mut frame_sum = 0.0_f32;
                    for ch in 0..num_channels {
                        let idx = frame * num_channels + ch;
                        if idx < buf.len() {
                            frame_sum += buf[idx];
                        }
                    }
                    samples.push(frame_sum / num_channels as f32);
                }
                if samples.len() >= max_samples {
                    break;
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
    // ── Marker zone (30px boven de waveform) ──
    let marker_zone_height = 30.0;
    let (marker_zone_rect, mz_response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(100.0), marker_zone_height),
        egui::Sense::click(),
    );
    let marker_painter = ui.painter();

    // Achtergrond marker zone
    marker_painter.rect_filled(marker_zone_rect, 0.0, egui::Color32::from_rgb(25, 25, 40));
    // Dunne lijn onder marker zone
    marker_painter.line_segment(
        [
            egui::pos2(marker_zone_rect.left(), marker_zone_rect.bottom()),
            egui::pos2(marker_zone_rect.right(), marker_zone_rect.bottom()),
        ],
        (1.0, egui::Color32::from_gray(60)),
    );

    let start_sec = state.scroll_offset;
    let visible_secs = marker_zone_rect.width() / state.zoom;
    let marker_start_sec = start_sec;
    let _marker_end_sec = (marker_start_sec + visible_secs).min(state.duration_secs);

    // Teken markers
    let mut seek_action: Option<f32> = None;
    let mut marker_drag_target: Option<(usize, f32)> = None;
    let mut marker_to_delete: Option<usize> = None;
    let mut double_click_marker_pos: Option<f32> = None;

    for (i, marker) in state.markers.iter().enumerate() {
        let px = marker_zone_rect.left() + (marker.position_secs - marker_start_sec) * state.zoom;
        if px < marker_zone_rect.left() - 20.0 || px > marker_zone_rect.right() + 20.0 {
            continue;
        }
        let px_clamped = px.clamp(
            marker_zone_rect.left() + 2.0,
            marker_zone_rect.right() - 2.0,
        );

        // Driehoekje — kleur afhankelijk van marker-type
        let tri_size = 6.0;
        let cx = px_clamped;
        let bot = marker_zone_rect.bottom();
        marker_painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(cx, bot),
                egui::pos2(cx - tri_size, bot - tri_size - 4.0),
                egui::pos2(cx + tri_size, bot - tri_size - 4.0),
            ],
            marker.kind.color(),
            egui::Stroke::new(1.0, marker.kind.stroke_color()),
        ));

        // Naam — toon prefix als de naam nog de default is
        let display_name = if marker.name.starts_with(marker.kind.prefix()) {
            format!(
                "{}: {}",
                marker.kind.prefix(),
                &marker.name[marker.kind.prefix().len() + 1..]
            )
        } else {
            marker.name.clone()
        };
        marker_painter.text(
            egui::pos2(cx, marker_zone_rect.top() + 2.0),
            egui::Align2::CENTER_TOP,
            &display_name,
            egui::TextStyle::Small.resolve(ui.style()),
            marker.kind.color(),
        );

        // Interactie per marker
        let hit_rect = egui::Rect::from_min_max(
            egui::pos2(px_clamped - 10.0, marker_zone_rect.top()),
            egui::pos2(px_clamped + 10.0, marker_zone_rect.bottom()),
        );
        let marker_id = ui.id().with("marker").with(i);
        let marker_resp = ui.interact(hit_rect, marker_id, egui::Sense::click_and_drag());

        if marker_resp.clicked() {
            // Click op marker → seek naar die positie
            seek_action = Some(marker.position_secs.clamp(0.0, state.duration_secs));
            state.playhead_frames_after_drag = 15; // ✅ FIX
        }
        if marker_resp.dragged() {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                let new_sec = ((pos.x - marker_zone_rect.left()) / state.zoom + marker_start_sec)
                    .clamp(0.0, state.duration_secs);
                marker_drag_target = Some((i, new_sec));
            }
        }
        if marker_resp.secondary_clicked() {
            marker_to_delete = Some(i);
        }
        if marker_resp.double_clicked() {
            // Bewerk marker naam
            state.editing_marker = Some(i);
            state.editing_marker_name = marker.name.clone();
        }
    }

    // Marker drag verwerken
    if let Some((idx, new_pos)) = marker_drag_target {
        if let Some(m) = state.markers.get_mut(idx) {
            m.position_secs = new_pos;
        }
    }

    // Dubbelklik in lege marker zone → nieuwe marker
    // Gebruik de response van `allocate_exact_size` zodat de interactie correct is afgestemd
    // op de exacte rect waarin de marker zone is getekend.
    if mz_response.double_clicked() {
        if let Some(pos) = mz_response.interact_pointer_pos() {
            let sec = ((pos.x - marker_zone_rect.left()) / state.zoom + marker_start_sec)
                .clamp(0.0, state.duration_secs);
            double_click_marker_pos = Some(sec);
        }
    }

    if let Some(sec) = double_click_marker_pos {
        // Standaard: Section marker bij dubbelklik
        // Shift+dubbelklik: Measure, Ctrl+dubbelklik: Beat
        let kind = if mz_response.hovered() {
            let shift = ui.ctx().input(|i| i.modifiers.shift);
            let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
            if shift {
                MarkerKind::Measure
            } else if ctrl {
                MarkerKind::Beat
            } else {
                MarkerKind::Section
            }
        } else {
            MarkerKind::Section
        };

        let count = state.markers.iter().filter(|m| m.kind == kind).count() + 1;
        state.markers.push(Marker {
            name: format!("{}{}", kind.prefix(), count),
            position_secs: sec,
            kind,
        });
    }

    // Verwijder marker bij rechterklik
    if let Some(idx) = marker_to_delete {
        state.markers.remove(idx);
    }

    // ── Marker naam bewerken (in-place text edit) ──
    if let Some(idx) = state.editing_marker {
        if let Some(m) = state.markers.get_mut(idx) {
            let px = marker_zone_rect.left() + (m.position_secs - marker_start_sec) * state.zoom;
            let edit_rect = egui::Rect::from_min_max(
                egui::pos2(
                    (px - 60.0).max(marker_zone_rect.left()),
                    marker_zone_rect.top(),
                ),
                egui::pos2(
                    (px + 60.0).min(marker_zone_rect.right()),
                    marker_zone_rect.top() + 22.0,
                ),
            );
            let resp = ui.allocate_ui_at_rect(edit_rect, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.editing_marker_name).desired_width(120.0),
                )
            });
            if resp.inner.lost_focus() || ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
                if !state.editing_marker_name.is_empty() {
                    m.name = state.editing_marker_name.clone();
                }
                state.editing_marker = None;
                state.editing_marker_name.clear();
            }
        }
    }

    // ── Waveform ──
    let width = ui.available_width().max(100.0);
    let height = 200.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());

    let id_base = ui.id();
    let painter = ui.painter();
    let center_y = rect.center().y;

    let mut loop_changed = false;

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
    // ✅ FIX: Gebruik de muis-positie ZODRA de drag start, niet pas erna
    let render_pos = if state.dragging_playhead || state.playhead_frames_after_drag > 0 {
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
            // --- Cursor feedback: verander muis in 'resize' als we near de playhead zijn ---
            if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                if (mouse_pos.x - pos_x).abs() < 15.0 && rect.contains(mouse_pos) {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }
            }

            // --- Playhead drag detectie (met een VEEL bredere, onzichtbare hitbox) ---
            if let Some(_actual_pos) = now_playing_position {
                let strip_half = 20.0; // ✅ FIX: Maak de hitbox 40px breed (was 10px)!

                if response.drag_started() {
                    if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                        let dx = (mouse_pos.x - pos_x).abs();
                        let dy_top = (mouse_pos.y - rect.top()).abs();
                        let dy_bot = (mouse_pos.y - rect.bottom()).abs();

                        let in_strip = dx <= strip_half;
                        let in_triangles =
                            dx <= tri_size && (dy_top <= tri_height || dy_bot <= tri_height);

                        // Alleen starten met slepen als we BINNEN de brede hitbox zijn
                        state.dragging_playhead = in_strip || in_triangles;

                        // Als we op de playhead klikken, consumeer de klik zodat de waveform niet seekt
                        if state.dragging_playhead {
                            //   response.consume();
                        }
                    }
                }

                if response.drag_stopped() {
                    // state.dragging_playhead = false;
                    // Blijf nog 3 frames op de versleepte positie
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

    // ── Ctrl+klik+versleep op waveform: selectie (A-B) maken ──
    // Alleen als Ctrl ingedrukt is en we niet op de playhead of loop-regio slepen
    if !state.dragging_playhead && !state.dragging_loop_region {
        let ctrl_held = ui.ctx().input(|i| i.modifiers.ctrl);

        if ctrl_held && response.drag_started() {
            if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                let sec = (mouse_pos.x - rect.left()) / state.zoom + start_sec;
                state.select_drag_start = Some(sec.clamp(0.0, state.duration_secs));
            }
        }

        if ctrl_held && response.dragged() && state.select_drag_start.is_some() {
            if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                let current_sec = ((mouse_pos.x - rect.left()) / state.zoom + start_sec)
                    .clamp(0.0, state.duration_secs);
                if let Some(start) = state.select_drag_start {
                    let (a, b) = if current_sec > start {
                        (start, current_sec)
                    } else {
                        (current_sec, start)
                    };
                    // Toon groene highlight tijdens drag
                    let a_x = rect.left() + (a - start_sec) * state.zoom;
                    let b_x = rect.left() + (b - start_sec) * state.zoom;
                    if b_x > a_x {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(a_x.max(rect.left()), rect.top()),
                                egui::pos2(b_x.min(rect.right()), rect.bottom()),
                            ),
                            0.0,
                            egui::Color32::from_rgba_premultiplied(100, 200, 100, 60),
                        );
                    }
                }
            }
        }

        if ctrl_held && response.drag_stopped() {
            if let Some(start) = state.select_drag_start.take() {
                if let Some(mouse_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                    let end = ((mouse_pos.x - rect.left()) / state.zoom + start_sec)
                        .clamp(0.0, state.duration_secs);
                    let distance = (end - start).abs();
                    if distance > 0.3 {
                        let (a, b) = if end > start {
                            (start, end)
                        } else {
                            (end, start)
                        };
                        state.loop_a_secs = Some(a);
                        state.loop_b_secs = Some(b);
                        loop_changed = true;
                        seek_action = Some(a); // spring naar begin selectie
                    } else {
                        // Minimale Ctrl+drag → telt als klik → seek
                        seek_action = Some(end.clamp(0.0, state.duration_secs));
                    }
                }
            }
        }

        // Ctrl+klik (zonder drag) op lege ruimte → seek alleen
        if ctrl_held && response.clicked() && !response.dragged() {
            if let Some(sec) = mouse_sec {
                seek_action = Some(sec.clamp(0.0, state.duration_secs));
                state.playhead_frames_after_drag = 15; // ✅ FIX: Negeer oude Position events voor ~250ms
            }
        }
    }

    // Enkelklik op waveform (geen drag): seek naar die positie
    if response.clicked() && !response.dragged() {
        if let Some(sec) = mouse_sec {
            seek_action = Some(sec.clamp(0.0, state.duration_secs));
            state.playhead_frames_after_drag = 15; // ✅ FIX
        }
    }

    // Playhead drag losgelaten: seek naar de versleepte positie
    if response.drag_stopped() && state.dragging_playhead {
        if let Some(sec) = state.playhead_drag_secs {
            seek_action = Some(sec);
            state.playhead_frames_after_drag = 15; // ✅ FIX: Verhoog van 3 naar 15
        }
        state.dragging_playhead = false; // ✅ FIX: Pas hier op false zetten, zodat de seek_action wel vuurt
    }

    // Rechterklik op waveform: wis selectie of marker
    if response.secondary_clicked() {
        state.loop_a_secs = None;
        state.loop_b_secs = None;
        loop_changed = true;
    }

    // Dubbelklik op waveform: zet A (of B met Shift)
    if response.double_clicked() {
        if let Some(sec) = mouse_sec {
            let sec = sec.clamp(0.0, state.duration_secs);
            if ui.ctx().input(|i| i.modifiers.shift) {
                state.loop_b_secs = Some(sec);
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
