use crate::player::{PlayerCommand, PlayerEvent, RepeatMode};
use crossbeam_channel::{Receiver, Sender};
use std::time::Instant;

/// Statusboodschappen vervallen automatisch na dit aantal seconden, zodat
/// ze niet eindeloos in de now-playing-balk blijven hangen.
const STATUS_MSG_SECS: f64 = 4.0;

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
    pub always_on_top: bool,
    pub _status_message: String,
    /// Tijdstip waarop de huidige statusboodschap is gezet (voor vervaltimer).
    status_message_set_at: Option<Instant>,
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
            always_on_top: false,
            _status_message: "Bibliotheek opstarten...".to_string(),
            status_message_set_at: None,
        }
    }

    /// Zet een statusboodschap die na [`STATUS_MSG_SECS`] seconden vervalt.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self._status_message = msg.into();
        self.status_message_set_at = Some(Instant::now());
    }

    /// Wis de statusboodschap als de vervaltijd verstreken is.
    pub fn expire_status(&mut self) {
        if let Some(at) = self.status_message_set_at {
            if at.elapsed().as_secs_f64() > STATUS_MSG_SECS {
                self._status_message.clear();
                self.status_message_set_at = None;
            }
        }
    }
}
