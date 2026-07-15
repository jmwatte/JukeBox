use crate::player::{PlayerCommand, PlayerEvent, RepeatMode};
use crossbeam_channel::{Receiver, Sender};

pub struct PlaybackState {
    pub player_tx: Sender<PlayerCommand>,
    pub player_event_rx: Receiver<PlayerEvent>,
    pub now_playing: Option<String>,
    pub now_playing_path: Option<String>,
    pub now_playing_position: f32,
    pub now_playing_duration: f32,
    pub volume: f32,
    pub repeat_mode: RepeatMode,
    pub shuffle_on: bool,
    pub show_queue: bool,
    pub queue: Vec<String>,
    pub loop_a: Option<f32>,
    pub loop_b: Option<f32>,
    pub status_error: Option<String>,
    pub compact_mode: bool,
    pub _status_message: String,
}

impl PlaybackState {
    pub fn new(player_tx: Sender<PlayerCommand>, player_event_rx: Receiver<PlayerEvent>) -> Self {
        Self {
            player_tx,
            player_event_rx,
            now_playing: None,
            now_playing_path: None,
            now_playing_position: 0.0,
            now_playing_duration: 0.0,
            volume: 1.0,
            repeat_mode: RepeatMode::None,
            shuffle_on: false,
            show_queue: false,
            queue: Vec::new(),
            loop_a: None,
            loop_b: None,
            status_error: None,
            compact_mode: false,
            _status_message: "Bibliotheek opstarten...".to_string(),
        }
    }
}
