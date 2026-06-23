use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Mutex, OnceLock};

static FFT_PLANNER: OnceLock<Mutex<FftPlanner<f32>>> = OnceLock::new();

/// Chroma resultaat: 12 waarden (C, C#, D, ..., B) van 0.0 tot 1.0
#[derive(Debug, Clone, Copy)]
pub struct Chroma(pub [f32; 12]);

impl Chroma {
    pub fn note_name(index: usize) -> &'static str {
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ][index % 12]
    }

    pub fn note_name_nl(index: usize) -> &'static str {
        [
            "Do", "Do#", "Re", "Re#", "Mi", "Fa", "Fa#", "Sol", "Sol#", "La", "La#", "Si",
        ][index % 12]
    }

    /// Hoogste noot (index 0-11)
    pub fn peak(&self) -> (usize, f32) {
        let mut best = (0, self.0[0]);
        for (i, &v) in self.0.iter().enumerate() {
            if v > best.1 {
                best = (i, v);
            }
        }
        best
    }

    /// Genereer een compacte weergave: alleen noten > drempel
    pub fn compact(&self, threshold: f32) -> Vec<(usize, f32)> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > threshold)
            .map(|(i, &v)| (i, v))
            .collect()
    }
}

/// Bereken chroma voor een slice audio.
/// `samples` = mono f32 samples, `sample_rate` = sample rate in Hz.
/// `start_sec` / `end_sec` = tijdbereik in seconden.
pub fn detect_chroma(
    samples: &[f32],
    sample_rate: u32,
    start_sec: Option<f32>,
    end_sec: Option<f32>,
) -> Chroma {
    let fft_size = 4096;
    let hop_size = 2048;

    let start_sample = start_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(fft_size));
    let end_sample = end_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(samples.len())
        .min(samples.len());

    if end_sample <= start_sample + fft_size {
        return Chroma([0.0; 12]);
    }

    let planner = FFT_PLANNER.get_or_init(|| Mutex::new(FftPlanner::new()));
    let fft = planner.lock().unwrap().plan_fft_forward(fft_size);

    let mut chroma_sum = [0.0_f64; 12];
    let mut _frame_count = 0_u32;

    // Hann window
    let window: Vec<f32> = (0..fft_size)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
        })
        .collect();

    let mut frame = start_sample;
    while frame + fft_size <= end_sample {
        // Venster toepassen + FFT
        let mut buffer: Vec<Complex<f32>> = (0..fft_size)
            .map(|i| Complex::new(samples[frame + i] * window[i], 0.0))
            .collect();

        fft.process(&mut buffer);

        // Energie per FFT bin (alleen positieve frequenties)
        for bin in 1..(fft_size / 2) {
            let energy = buffer[bin].norm_sqr();
            if energy < 1e-12 {
                continue;
            }

            // Frequentie van deze bin
            let freq = bin as f32 * sample_rate as f32 / fft_size as f32;

            // Chroma: log2(freq / 440) * 12 + 9  (C = 0, A = 9)
            let chroma_raw = (freq / 440.0).log2() * 12.0 + 9.0;
            let note_f = chroma_raw.rem_euclid(12.0);
            let note_idx = note_f as usize % 12;

            chroma_sum[note_idx] += energy as f64;
        }

        _frame_count += 1;
        frame += hop_size;
    }

    // Normaliseer
    let max = chroma_sum.iter().cloned().fold(0.0_f64, f64::max);
    let mut result = [0.0_f32; 12];
    if max > 1e-12 {
        for i in 0..12 {
            result[i] = (chroma_sum[i] / max) as f32;
        }
    }

    Chroma(result)
}
