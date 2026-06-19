use crossbeam_channel::{Receiver, Sender};
use rodio::{OutputStream, Sink, Source};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Commando's van UI naar waveform audio-thread
// ---------------------------------------------------------------------------

pub enum WaveformCommand {
    Play {
        path: String,
        decode_start_sec: f32, // Start of the region to decode (e.g., Loop A)
        decode_end_sec: f32,   // End of the region to decode (e.g., Loop B)
        play_start_sec: f32,   // Exact position to start playback (e.g., clicked position)
        pitch_semitones: f32,
        tempo: f32,
    },
    Stop,
    SetPitch(f32),
    SetTempo(f32),
}

// ---------------------------------------------------------------------------
// Events van waveform audio-thread naar UI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WaveformEvent {
    Playing,
    Stopped,
    Error(String),
    Position(f32, f32),
}

// ---------------------------------------------------------------------------
// Start de thread. Geeft (cmd_tx, event_rx).
// ---------------------------------------------------------------------------

pub fn start_waveform_thread() -> (Sender<WaveformCommand>, Receiver<WaveformEvent>) {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();

    std::thread::spawn(move || {
        run_waveform_audio(cmd_rx, event_tx);
    });

    (cmd_tx, event_rx)
}

// ---------------------------------------------------------------------------
// Interne audio-loop
// ---------------------------------------------------------------------------

fn run_waveform_audio(rx: Receiver<WaveformCommand>, event_tx: Sender<WaveformEvent>) {
    let mut _stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut _current_path: Option<String> = None;
    let mut _current_start: f32 = 0.0;
    let mut _current_end: f32 = 0.0;
    let mut current_pitch: f32 = 0.0;
    let mut current_tempo: f32 = 1.0;
    let mut current_duration: f32 = 0.0;
    let mut is_playing = false;
    let mut cached_raw: Option<(Vec<f32>, u32, f32)> = None;

    // Audio device openen
    if let Ok((stream, handle)) = OutputStream::try_default() {
        if let Ok(new_sink) = Sink::try_new(&handle) {
            _stream = Some(stream);
            sink = Some(new_sink);
        }
    }

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                WaveformCommand::Play {
                    path,
                    decode_start_sec,
                    decode_end_sec,
                    play_start_sec,
                    pitch_semitones,
                    tempo,
                } => {
                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                    }

                    _current_path = Some(path.clone());
                    _current_start = decode_start_sec;
                    _current_end = decode_end_sec;
                    current_pitch = pitch_semitones;
                    current_tempo = tempo;
                    is_playing = false;

                    // FIX: Decode the ENTIRE loop region (A to B), not just from the playhead to B
                    match decode_segment(&path, decode_start_sec, decode_end_sec) {
                        Ok((samples, sample_rate, segment_duration)) => {
                            cached_raw = Some((samples.clone(), sample_rate, segment_duration));
                            current_duration = segment_duration;

                            // Calculate starting position before processing (om borrow issues te voorkomen)
                            let raw_offset_samples =
                                ((play_start_sec - decode_start_sec) * sample_rate as f32) as usize;
                            let total_raw = samples.len();

                            // Rubato processing (tempo + pitch)
                            let processed =
                                match apply_rubato(&samples, tempo, pitch_semitones, sample_rate) {
                                    Ok(p) => p,
                                    Err(e) => {
                                        let _ = event_tx
                                            .send(WaveformEvent::Error(format!("Rubato: {}", e)));
                                        samples.clone()
                                    }
                                };

                            let ratio = if total_raw > 0 {
                                processed.len() as f32 / total_raw as f32
                            } else {
                                1.0
                            };
                            let initial_pos = (raw_offset_samples as f32 * ratio) as usize;
                            let initial_pos = initial_pos.min(processed.len().saturating_sub(1));

                            let source = WaveformSource {
                                samples: processed,
                                pos: initial_pos, // Start exactly where the user clicked!
                                sample_rate,
                            };

                            if let Some(s) = &sink {
                                s.append(source);
                                s.play();
                                is_playing = true;
                                let _ = event_tx.send(WaveformEvent::Playing);
                            } else {
                                let _ = event_tx
                                    .send(WaveformEvent::Error("Geen audio-apparaat".into()));
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(WaveformEvent::Error(e));
                        }
                    }
                }

                WaveformCommand::Stop => {
                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                    }
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                }

                WaveformCommand::SetPitch(semitones) => {
                    current_pitch = semitones;
                    if is_playing {
                        restart_playback(
                            &mut sink,
                            &cached_raw,
                            current_pitch,
                            current_tempo,
                            &event_tx,
                            &mut current_duration,
                            &mut is_playing,
                        );
                    }
                }

                WaveformCommand::SetTempo(new_tempo) => {
                    current_tempo = new_tempo;
                    if is_playing {
                        restart_playback(
                            &mut sink,
                            &cached_raw,
                            current_pitch,
                            current_tempo,
                            &event_tx,
                            &mut current_duration,
                            &mut is_playing,
                        );
                    }
                }
            }
        }

        // Positie-updates sturen
        if is_playing {
            if let Some(s) = &sink {
                if s.empty() {
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                } else {
                    let raw_pos = s.get_pos().as_secs_f32();

                    // Calculate position relative to the segment (handles infinite looping)
                    let pos_in_segment = if current_duration > 0.0 {
                        raw_pos % current_duration
                    } else {
                        raw_pos
                    };

                    // Convert to absolute position in the file
                    let pos = _current_start + pos_in_segment;
                    let _ = event_tx.send(WaveformEvent::Position(pos, current_duration));
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}

/// Herstart playback met nieuwe instellingen (gebruikt bij SetPitch / SetTempo).
/// Gebruikt `cached_raw` in plaats van opnieuw van schijf te decoderen.
fn restart_playback(
    sink: &mut Option<Sink>,
    cached_raw: &Option<(Vec<f32>, u32, f32)>,
    pitch: f32,
    tempo: f32,
    event_tx: &Sender<WaveformEvent>,
    current_duration: &mut f32,
    is_playing: &mut bool,
) {
    let (samples, sample_rate, segment_duration) = match cached_raw {
        Some((s, r, d)) => (s.clone(), *r, *d),
        None => return,
    };

    if let Some(s) = sink {
        s.stop();
        s.clear();
    }

    *current_duration = segment_duration;
    let processed = match apply_rubato(&samples, tempo, pitch, sample_rate) {
        Ok(p) => p,
        Err(e) => {
            let _ = event_tx.send(WaveformEvent::Error(format!("Rubato: {}", e)));
            samples
        }
    };

    let source = WaveformSource {
        samples: processed,
        pos: 0,
        sample_rate,
    };

    if let Some(s) = sink {
        s.append(source);
        s.play();
        *is_playing = true;
        let _ = event_tx.send(WaveformEvent::Playing);
    }
}

// ---------------------------------------------------------------------------
// rodio Source wrapper rond Vec<f32>
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct WaveformSource {
    samples: Vec<f32>,
    pos: usize,
    sample_rate: u32,
}

impl Iterator for WaveformSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.samples.is_empty() {
            return None;
        }
        let sample = self.samples[self.pos];
        self.pos += 1;
        if self.pos >= self.samples.len() {
            self.pos = 0; // oneindig loopen
        }
        Some(sample)
    }
}

impl Source for WaveformSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.pos))
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None // oneindig (looping)
    }
}

// ---------------------------------------------------------------------------
// Segment decoderen met symphonia (alleen A-B)
// ---------------------------------------------------------------------------

fn decode_segment(
    path: &str,
    start_sec: f32,
    end_sec: f32,
) -> Result<(Vec<f32>, u32, f32), String> {
    use std::fs::File;
    use std::path::Path;
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::probe::Hint;

    let path_obj = Path::new(path);
    let file = File::open(&path_obj).map_err(|e| format!("Kan bestand niet openen: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let ext = path_obj
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut hint = Hint::new();
    hint.with_extension(&ext);

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| format!("Kan formaat niet detecteren: {}", e))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.sample_rate.is_some())
        .ok_or_else(|| "Geen audio track".to_string())?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Kan decoder niet maken: {}", e))?;

    let start_sample = (start_sec * sample_rate as f32) as usize;
    let end_sample = (end_sec * sample_rate as f32) as usize;

    let mut all_samples: Vec<f32> = Vec::new();
    let mut total_decoded: usize = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(pkt) => pkt,
            Err(symphonia::core::errors::Error::IoError(ref err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let num_frames = decoded.frames();
                let num_channels = decoded.spec().channels.count();

                let mut sample_buf =
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                sample_buf.copy_interleaved_ref(decoded);
                let buf = sample_buf.samples();

                for frame in 0..num_frames {
                    let abs_idx = total_decoded + frame;
                    if abs_idx >= end_sample {
                        break;
                    }
                    if abs_idx >= start_sample {
                        let mut frame_sum = 0.0_f32;
                        for ch in 0..num_channels {
                            let idx = frame * num_channels + ch;
                            if idx < buf.len() {
                                frame_sum += buf[idx];
                            }
                        }
                        all_samples.push(frame_sum / num_channels as f32);
                    }
                }
                total_decoded += num_frames;
                if total_decoded >= end_sample {
                    break;
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if all_samples.is_empty() {
        return Err("Geen samples gedecodeerd".to_string());
    }

    let duration = all_samples.len() as f32 / sample_rate as f32;
    Ok((all_samples, sample_rate, duration))
}

// ---------------------------------------------------------------------------
// Rubato: tempo + pitch processing
// ---------------------------------------------------------------------------

fn apply_rubato(
    samples: &[f32],
    tempo: f32,
    pitch_semitones: f32,
    _sample_rate: u32,
) -> Result<Vec<f32>, String> {
    // Bepaal de resample ratio (output_samples / input_samples)
    // ratio > 1: meer output -> langzamer -> lagere toon / tragere tempo
    // ratio < 1: minder output -> sneller -> hogere toon / snellere tempo
    // tempo > 1.0 = sneller, pitch_semitones > 0 = hogere toon -> beide hebben ratio < 1 nodig
    let pitch_factor = f32::powf(2.0, pitch_semitones / 12.0);
    let resample_ratio = 1.0 / (pitch_factor * tempo);

    // Geen processing nodig als ratio ≈ 1.0
    if (resample_ratio - 1.0).abs() < 0.001 {
        return Ok(samples.to_vec());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        oversampling_factor: 128,
        interpolation: SincInterpolationType::Linear,
        window: WindowFunction::BlackmanHarris2,
    };

    let chunk_size = 1024.min(samples.len()).max(64);

    let mut resampler = SincFixedIn::<f32>::new(
        resample_ratio as f64,
        10.0, // max_relative_ratio
        params,
        chunk_size,
        1, // mono
    )
    .map_err(|e| format!("Resampler constructie: {:?}", e))?;

    let mut output: Vec<f32> = Vec::new();
    let mut input_pos = 0;

    while input_pos < samples.len() {
        let remaining = samples.len() - input_pos;

        if remaining >= chunk_size {
            // Volledige chunk
            let end = input_pos + chunk_size;
            let input_chunk = vec![samples[input_pos..end].to_vec()];
            let result = resampler
                .process(&input_chunk, None)
                .map_err(|e| format!("Rubato process: {:?}", e))?;
            if let Some(ch) = result.first() {
                output.extend_from_slice(ch);
            }
            input_pos += chunk_size;
        } else {
            // Laatste partial chunk: zero-pad naar chunk_size
            let mut padded = samples[input_pos..].to_vec();
            padded.resize(chunk_size, 0.0_f32);
            let input_chunk = vec![padded];
            let result = resampler
                .process(&input_chunk, None)
                .map_err(|e| format!("Rubato partial: {:?}", e))?;
            if let Some(ch) = result.first() {
                output.extend_from_slice(ch);
            }
            break;
        }
    }

    // Flush internal resampler buffers
    if resampler.input_frames_next() > 0 {
        let pad = vec![vec![0.0_f32; resampler.input_frames_next()]];
        let result = resampler.process(&pad, None).ok();
        if let Some(out) = result {
            if let Some(ch) = out.first() {
                output.extend_from_slice(ch);
            }
        }
    }

    // Laatste flush: nog een keer met een lege input om resterende delay uit te krijgen
    if resampler.input_frames_next() > 0 {
        let pad2 = vec![vec![0.0_f32; resampler.input_frames_next()]];
        let result = resampler.process(&pad2, None).ok();
        if let Some(out) = result {
            if let Some(ch) = out.first() {
                output.extend_from_slice(ch);
            }
        }
    }

    Ok(output)
}
