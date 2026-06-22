// ───────────────────────────────────────────────
// TimeStretch — Rubber Band tijd-rekken / pitch-shift
// ───────────────────────────────────────────────

pub struct TimeStretch {
    inner: Backend,
}

impl TimeStretch {
    pub fn new(sample_rate: u32, channels: u32, total_frames: usize) -> Self {
        Self {
            inner: Backend::new(sample_rate, channels, total_frames),
        }
    }

    /// Snelheid: 1.0 = normaal, 0.5 = half zo snel, 2.0 = dubbel zo snel
    pub fn set_speed(&mut self, speed: f32) {
        self.inner.set_speed(speed);
    }

    /// Pitch in halve tonen: 0 = normaal, +12 = octaaf omhoog, -12 = octaaf omlaag
    pub fn set_pitch_semitones(&mut self, semitones: f32) {
        self.inner.set_pitch_semitones(semitones);
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn put_samples(&mut self, input: &[f32], frames: usize) {
        self.inner.put_samples(input, frames);
    }

    pub fn receive_samples(&mut self, output: &mut [f32], frames: usize) -> usize {
        self.inner.receive_samples(output, frames)
    }
}

// ───────────────────────────────────────────────
// Rubber Band backend (via C FFI)
// ───────────────────────────────────────────────
#[cfg(feature = "rubberband")]
mod backend {
    use std::ffi::c_void;
    use std::os::raw::{c_int, c_uint};

    const FEED_FRAMES: usize = 4_096;

    #[allow(non_camel_case_types)]
    type RubberBandState = *mut c_void;

    // Rubber Band options
    const PROCESS_REALTIME: c_int = 0x0000_0001;
    const WINDOW_SHORT: c_int = 0x0010_0000;
    const FORMANT_PRESERVED: c_int = 0x0100_0000;
    const PITCH_HIGH_CONSISTENCY: c_int = 0x0400_0000;
    const CHANNELS_TOGETHER: c_int = 0x1000_0000;
    const ENGINE_FINER: c_int = 0x2000_0000;

    unsafe extern "C" {
        fn rubberband_new(
            sampleRate: c_uint,
            channels: c_uint,
            options: c_int,
            initialTimeRatio: f64,
            initialPitchScale: f64,
        ) -> RubberBandState;
        fn rubberband_delete(state: RubberBandState);
        fn rubberband_reset(state: RubberBandState);
        fn rubberband_set_time_ratio(state: RubberBandState, ratio: f64);
        fn rubberband_set_pitch_scale(state: RubberBandState, scale: f64);
        fn rubberband_set_formant_option(state: RubberBandState, options: c_int);
        fn rubberband_set_pitch_option(state: RubberBandState, options: c_int);
        fn rubberband_set_expected_input_duration(state: RubberBandState, samples: c_uint);
        fn rubberband_set_max_process_size(state: RubberBandState, samples: c_uint);
        fn rubberband_process(
            state: RubberBandState,
            input: *const *const f32,
            samples: c_uint,
            final_: c_int,
        );
        fn rubberband_available(state: RubberBandState) -> c_int;
        fn rubberband_retrieve(
            state: RubberBandState,
            output: *const *mut f32,
            samples: c_uint,
        ) -> c_uint;
    }

    pub struct Backend {
        handle: RubberBandState,
        channels: usize,
        cached_speed: f32,
        cached_pitch: f32,
        input_planar: Vec<Vec<f32>>,
        input_ptrs: Vec<*const f32>,
        output_planar: Vec<Vec<f32>>,
        output_ptrs: Vec<*mut f32>,
    }

    impl Backend {
        pub fn new(sample_rate: u32, channels: u32, total_frames: usize) -> Self {
            let options = PROCESS_REALTIME | ENGINE_FINER | CHANNELS_TOGETHER | WINDOW_SHORT;

            let handle = unsafe {
                rubberband_new(
                    sample_rate,
                    channels,
                    options,
                    1.0, // initialTimeRatio
                    1.0, // initialPitchScale
                )
            };

            unsafe {
                rubberband_set_formant_option(handle, FORMANT_PRESERVED);
                rubberband_set_pitch_option(handle, PITCH_HIGH_CONSISTENCY);
                rubberband_set_expected_input_duration(
                    handle,
                    total_frames.min(u32::MAX as usize) as c_uint,
                );
                rubberband_set_max_process_size(
                    handle,
                    FEED_FRAMES.min(u32::MAX as usize) as c_uint,
                );
            }

            Self {
                handle,
                channels: channels as usize,
                cached_speed: 1.0,
                cached_pitch: 0.0,
                input_planar: vec![Vec::new(); channels as usize],
                input_ptrs: vec![std::ptr::null(); channels as usize],
                output_planar: vec![Vec::new(); channels as usize],
                output_ptrs: vec![std::ptr::null_mut(); channels as usize],
            }
        }

        pub fn set_speed(&mut self, speed: f32) {
            if (speed - self.cached_speed).abs() <= 1e-4 {
                return;
            }
            let speed = speed.max(0.01);
            unsafe {
                // Rubber Band time ratio = unstretched / stretched.
                // speed 0.5 (half speed) → time ratio 2.0
                rubberband_set_time_ratio(self.handle, 1.0 / speed as f64);
            }
            self.cached_speed = speed;
        }

        pub fn set_pitch_semitones(&mut self, semitones: f32) {
            if (semitones - self.cached_pitch).abs() <= 1e-4 {
                return;
            }
            let ratio = 2.0_f64.powf(semitones as f64 / 12.0);
            unsafe {
                rubberband_set_pitch_scale(self.handle, ratio);
            }
            self.cached_pitch = semitones;
        }

        pub fn clear(&mut self) {
            unsafe {
                rubberband_reset(self.handle);
            }
        }

        pub fn put_samples(&mut self, input: &[f32], frames: usize) {
            self.ensure_input_capacity(frames);
            // Mono: gewoon kopiëren naar eerste kanaal
            self.input_planar[0][..frames].copy_from_slice(&input[..frames]);
            unsafe {
                rubberband_process(self.handle, self.input_ptrs.as_ptr(), frames as c_uint, 0);
            }
        }

        pub fn receive_samples(&mut self, output: &mut [f32], frames: usize) -> usize {
            let available = unsafe { rubberband_available(self.handle) }.max(0) as usize;
            let take = available.min(frames);
            if take == 0 {
                return 0;
            }
            self.ensure_output_capacity(take);
            let got = unsafe {
                rubberband_retrieve(self.handle, self.output_ptrs.as_ptr(), take as c_uint) as usize
            };
            // Mono: kopieer van eerste kanaal
            output[..got].copy_from_slice(&self.output_planar[0][..got]);
            got
        }

        fn ensure_input_capacity(&mut self, frames: usize) {
            for ch in 0..self.channels {
                self.input_planar[ch].resize(frames, 0.0);
                self.input_ptrs[ch] = self.input_planar[ch].as_ptr();
            }
        }

        fn ensure_output_capacity(&mut self, frames: usize) {
            for ch in 0..self.channels {
                self.output_planar[ch].resize(frames, 0.0);
                self.output_ptrs[ch] = self.output_planar[ch].as_mut_ptr();
            }
        }
    }

    unsafe impl Send for Backend {}

    impl Drop for Backend {
        fn drop(&mut self) {
            unsafe {
                rubberband_delete(self.handle);
            }
        }
    }
}

// ───────────────────────────────────────────────
// SoundTouch fallback (als feature "rubberband" uit staat)
// ───────────────────────────────────────────────
#[cfg(not(feature = "rubberband"))]
mod backend {
    use soundtouch::{Setting, SoundTouch};

    pub struct Backend {
        st: SoundTouch,
        cached_speed: f32,
        cached_pitch: f32,
    }

    impl Backend {
        pub fn new(sample_rate: u32, channels: u32, _total_frames: usize) -> Self {
            let mut st = SoundTouch::new();
            st.set_channels(channels)
                .set_sample_rate(sample_rate)
                .set_setting(Setting::SequenceMs, 80)
                .set_setting(Setting::SeekwindowMs, 30)
                .set_setting(Setting::OverlapMs, 16);

            Self {
                st,
                cached_speed: 1.0,
                cached_pitch: 0.0,
            }
        }

        pub fn set_speed(&mut self, speed: f32) {
            if (speed - self.cached_speed).abs() <= 1e-4 {
                return;
            }
            self.st.set_tempo(speed as f64);
            self.cached_speed = speed;
        }

        pub fn set_pitch_semitones(&mut self, semitones: f32) {
            if (semitones - self.cached_pitch).abs() <= 1e-4 {
                return;
            }
            let ratio = 2.0_f64.powf(semitones as f64 / 12.0);
            self.st.set_pitch(ratio);
            self.cached_pitch = semitones;
        }

        pub fn clear(&mut self) {
            self.st.clear();
        }

        pub fn put_samples(&mut self, input: &[f32], frames: usize) {
            self.st.put_samples(input, frames);
        }

        pub fn receive_samples(&mut self, output: &mut [f32], frames: usize) -> usize {
            self.st.receive_samples(output, frames)
        }
    }
}

pub use backend::Backend;
