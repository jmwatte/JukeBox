use crossbeam_channel::{Receiver, Sender};
use rodio::{OutputStream, Sink, Source};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Commando's van UI naar waveform audio-thread
// ---------------------------------------------------------------------------

pub enum WaveformCommand {
    Play {
        samples: Arc<Mutex<Vec<f32>>>,
        sample_rate: u32,
        start_sample: usize,
        segment_start_sec: f32,
        a_sample: usize,
        b_sample: usize,
        pitch_semitones: Arc<Mutex<f32>>,
        tempo: Arc<Mutex<f32>>,
    },
    Stop,
    /// Pauzeer de sink (rodio::Sink::pause). De source `next()` wordt niet aangeroepen,
    /// dus de interne `pos` blijft staan.
    #[allow(dead_code)]
    Pause,
    /// Hervat na pauze.
    #[allow(dead_code)]
    Resume,
    TogglePause,
    /// Update de loop-grenzen zonder de source te herstarten.
    /// a_secs en b_secs zijn absolute posities in de track.
    SetLoopBounds {
        a_secs: f32,
        b_secs: f32,
    },
    /// Verplaats de playhead naar een absolute positie in de track.
    Seek {
        pos_secs: f32,
    },
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
    Paused,
    Resumed,
    Error(String),
    Position(f32, f32),
}

// ---------------------------------------------------------------------------
// Gedeelde loop-grenzen
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct LoopBounds {
    pub a: usize,
    pub b: usize,
}

impl LoopBounds {
    pub fn enabled(&self) -> bool {
        self.b > self.a
    }
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
// Interne audio-loop — geen apply_rubato, geen restart_playback
// ---------------------------------------------------------------------------

fn run_waveform_audio(rx: Receiver<WaveformCommand>, event_tx: Sender<WaveformEvent>) {
    let mut _stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut is_playing = false;
    let mut is_paused = false;

    // Gedeelde state — wordt via Arc<Mutex<>> door de RealTimePitchTempoSource gebruikt
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let pitch_semitones: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let tempo: Arc<Mutex<f32>> = Arc::new(Mutex::new(1.0));
    let loop_bounds: Arc<Mutex<LoopBounds>> = Arc::new(Mutex::new(LoopBounds { a: 0, b: 0 }));
    let source_pos: Arc<Mutex<f64>> = Arc::new(Mutex::new(0.0));
    let segment_start_sec: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let segment_dur: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let current_sample_rate: Arc<Mutex<u32>> = Arc::new(Mutex::new(44100));

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
                    samples: new_samples,
                    sample_rate: sr,
                    start_sample,
                    segment_start_sec: seg_start,
                    a_sample,
                    b_sample,
                    pitch_semitones: ps,
                    tempo: t,
                } => {
                    // Kopieer de samples en parameters naar gedeelde state
                    *samples.lock().unwrap() = new_samples.lock().unwrap().clone();
                    *pitch_semitones.lock().unwrap() = *ps.lock().unwrap();
                    *tempo.lock().unwrap() = *t.lock().unwrap();
                    *current_sample_rate.lock().unwrap() = sr;
                    *source_pos.lock().unwrap() = start_sample as f64;
                    *segment_start_sec.lock().unwrap() = seg_start;

                    let len = samples.lock().unwrap().len();
                    *segment_dur.lock().unwrap() = len as f32 / sr as f32;

                    if b_sample > a_sample && b_sample <= len {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                        };
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds { a: 0, b: 0 };
                    }

                    // Nieuwe real-time source aanmaken
                    let source = RealTimePitchTempoSource::new(
                        samples.clone(),
                        sr,
                        pitch_semitones.clone(),
                        tempo.clone(),
                        loop_bounds.clone(),
                        source_pos.clone(),
                    );

                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                        s.append(source);
                        s.play();
                        is_playing = true;
                        is_paused = false;
                        let _ = event_tx.send(WaveformEvent::Playing);
                    } else {
                        let _ = event_tx.send(WaveformEvent::Error("Geen audio-apparaat".into()));
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

                WaveformCommand::Pause => {
                    if let Some(s) = &sink {
                        if !s.is_paused() {
                            s.pause();
                            is_paused = true;
                            let _ = event_tx.send(WaveformEvent::Paused);
                        }
                    }
                }

                WaveformCommand::Resume => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        }
                    }
                }

                WaveformCommand::TogglePause => {
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        } else {
                            s.pause();
                            is_paused = true;
                            let _ = event_tx.send(WaveformEvent::Paused);
                        }
                    }
                }

                WaveformCommand::SetLoopBounds { a_secs, b_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let a_sample = (a_secs.max(0.0) * sr as f32) as usize;
                    let b_sample = (b_secs.max(0.0) * sr as f32) as usize;

                    // Update loop bounds — de source pikt dit direct op
                    if b_sample > a_sample {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                        };
                        *segment_start_sec.lock().unwrap() = a_secs;
                        *segment_dur.lock().unwrap() = (b_secs - a_secs).max(0.001);
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds { a: 0, b: 0 };
                    }
                }

                WaveformCommand::Seek { pos_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let start_sec = *segment_start_sec.lock().unwrap();
                    // Relatieve sample-offset binnen het segment
                    let rel_secs = (pos_secs - start_sec).max(0.0);
                    let sample = (rel_secs * sr as f32) as f64;
                    *source_pos.lock().unwrap() = sample;
                }

                WaveformCommand::SetPitch(semitones) => {
                    *pitch_semitones.lock().unwrap() = semitones;
                }

                WaveformCommand::SetTempo(new_tempo) => {
                    *tempo.lock().unwrap() = new_tempo;
                }
            }
        }

        // Positie-updates sturen naar UI (~60 fps)
        if is_playing && !is_paused {
            if let Some(s) = &sink {
                if s.empty() {
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                } else {
                    let pos_samples = *source_pos.lock().unwrap();
                    let sr = *current_sample_rate.lock().unwrap();
                    let start_sec = *segment_start_sec.lock().unwrap();
                    let dur = *segment_dur.lock().unwrap();
                    let bounds = *loop_bounds.lock().unwrap();

                    // Absolute positie in de track
                    let pos_secs = pos_samples as f32 / sr as f32;
                    let effective_pos = if bounds.enabled() {
                        let loop_dur = (bounds.b - bounds.a) as f32 / sr as f32;
                        start_sec + (pos_secs % loop_dur)
                    } else {
                        start_sec + pos_secs
                    };
                    let _ = event_tx.send(WaveformEvent::Position(effective_pos, dur));
                }
            }
        }

        std::thread::sleep(Duration::from_millis(16));
    }
}

// ---------------------------------------------------------------------------
// RealTimePitchTempoSource — rodio::Source die real-time pitch/tempo toepast
// ---------------------------------------------------------------------------

/// Aantal samples per interne chunk.
/// Mutex-locking gebeurt één keer per chunk, niet per sample.
const CHUNK_SIZE: usize = 512;

/// Rodio Source die ruwe PCM-samples afspeelt met real-time pitch shifting
/// en tempo-aanpassing via lineaire interpolatie.
///
/// Pitch en tempo worden via `Arc<Mutex<>>` gedeeld met de UI-thread,
/// zodat wijzigingen direct worden doorgevoerd zonder de source te herstarten.
struct RealTimePitchTempoSource {
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    pitch_semitones: Arc<Mutex<f32>>,
    tempo: Arc<Mutex<f32>>,
    loop_bounds: Arc<Mutex<LoopBounds>>,
    /// Huidige (fractionele) sample-positie in de ruwe buffer.
    /// Wordt gedeeld met de audio-thread voor positie-rapportage.
    source_pos: Arc<Mutex<f64>>,
    /// Interne buffer met verwerkte samples (gevuld in chunks).
    buf: Vec<f32>,
    buf_idx: usize,
}

impl RealTimePitchTempoSource {
    fn new(
        samples: Arc<Mutex<Vec<f32>>>,
        sample_rate: u32,
        pitch_semitones: Arc<Mutex<f32>>,
        tempo: Arc<Mutex<f32>>,
        loop_bounds: Arc<Mutex<LoopBounds>>,
        source_pos: Arc<Mutex<f64>>,
    ) -> Self {
        Self {
            samples,
            sample_rate,
            pitch_semitones,
            tempo,
            loop_bounds,
            source_pos,
            buf: Vec::new(),
            buf_idx: 0,
        }
    }

    /// Vult de interne buffer met `CHUNK_SIZE` verwerkte samples.
    /// Leest de huidige pitch/tempo uit de gedeelde `Arc<Mutex<>>`,
    /// past lineaire interpolatie toe en handelt looping af.
    fn refill_buffer(&mut self) {
        let guard = self.samples.lock().unwrap();
        let raw = &*guard;

        if raw.is_empty() {
            self.buf.clear();
            return;
        }

        let total_len = raw.len();
        let pitch = *self.pitch_semitones.lock().unwrap();
        let t = *self.tempo.lock().unwrap();
        let pitch_factor = f32::powf(2.0, pitch / 12.0);
        let step = (pitch_factor * t) as f64;

        let bounds = *self.loop_bounds.lock().unwrap();
        let looping = bounds.enabled();

        self.buf.clear();
        let mut pos = *self.source_pos.lock().unwrap();

        for _ in 0..CHUNK_SIZE {
            // Looping: spring naar A zodra we bij of voorbij B zijn
            if looping && pos as usize >= bounds.b {
                pos = bounds.a as f64;
            }

            // Als de positie buiten de buffer valt en er is een loop, reset naar A
            if pos as usize >= total_len {
                if looping {
                    pos = bounds.a as f64;
                } else {
                    break; // Geen loop-mogelijkheid → einde
                }
            }

            // Lineaire interpolatie tussen twee omliggende samples
            let floor = pos.floor() as usize;
            let frac = pos - floor as f64;
            let next_idx = (floor + 1).min(total_len - 1);

            let s0 = raw[floor] as f64;
            let s1 = raw[next_idx] as f64;
            let sample = s0 + (s1 - s0) * frac;

            self.buf.push(sample as f32);
            pos += step;
        }

        *self.source_pos.lock().unwrap() = pos;
        self.buf_idx = 0;
    }
}

impl Iterator for RealTimePitchTempoSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.buf_idx >= self.buf.len() {
            self.refill_buffer();
        }
        if self.buf_idx < self.buf.len() {
            let sample = self.buf[self.buf_idx];
            self.buf_idx += 1;
            Some(sample)
        } else {
            None
        }
    }
}

impl Source for RealTimePitchTempoSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(std::usize::MAX)
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None // Loopt oneindig (looping)
    }
}
