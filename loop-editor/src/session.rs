use serde::{Deserialize, Serialize};

const SESSION_FILE: &str = "session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub file_path: Option<String>,
    pub play_position: f32,
    pub zoom: f32,
    pub scroll_offset: f32,
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
    pub pitch_semitones: f32,
    pub tempo: f32,
    pub volume: f32,
    pub channel_mode: String,
}

impl SessionState {
    pub fn save(
        file_path: Option<&str>,
        play_position: f32,
        zoom: f32,
        scroll_offset: f32,
        loop_a_secs: Option<f32>,
        loop_b_secs: Option<f32>,
        pitch_semitones: f32,
        tempo: f32,
        volume: f32,
        channel_mode: &str,
    ) {
        let state = SessionState {
            file_path: file_path.map(|s| s.to_string()),
            play_position,
            zoom,
            scroll_offset,
            loop_a_secs,
            loop_b_secs,
            pitch_semitones,
            tempo,
            volume,
            channel_mode: channel_mode.to_string(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(SESSION_FILE, json);
        }
    }

    pub fn load() -> Option<SessionState> {
        match std::fs::read_to_string(SESSION_FILE) {
            Ok(json) => serde_json::from_str(&json).ok(),
            Err(_) => None,
        }
    }
}
