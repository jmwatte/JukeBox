use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub music_directory: String,
    pub cover_names: Vec<String>,
    pub cover_extensions: Vec<String>,
    pub audio_extensions: Vec<String>,
    pub window_size: [u32; 2],
    pub shortcuts: HashMap<String, String>,
    pub startup_view: String, // "cover" of "tracklist"
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Aangepast naar jouw exacte map:
            music_directory: "H:\\music".to_string(),
            // Gevoelige cover namen (alles wat hierin voorkomt wordt gematcht)
            cover_names: vec![
                "cover".into(),
                "folder".into(),
                "album".into(),
                "front".into(),
                "art".into(),
            ],
            cover_extensions: vec!["jpg".into(), "jpeg".into(), "png".into()],
            // Uitgebreide lijst met formaten dankzij symphonia!
            audio_extensions: vec![
                "mp3".into(),
                "flac".into(),
                "opus".into(),
                "ogg".into(),
                "m4a".into(),
                "mp4".into(),
                "wav".into(),
                "aac".into(),
                "alac".into(),
            ],
            window_size: [800, 600],
            shortcuts: crate::ui::shortcuts::default_shortcuts(),
            startup_view: "cover".into(),
        }
    }
}

impl Config {
    pub fn load_or_create() -> Self {
        let config_path = Path::new("config.toml");
        if config_path.exists() {
            match fs::read_to_string(config_path) {
                Ok(config_str) => match toml::from_str(&config_str) {
                    Ok(config) => return config,
                    Err(e) => eprintln!(
                        "Fout bij parsen van config.toml: {}. Gebruik standaardwaarden.",
                        e
                    ),
                },
                Err(e) => eprintln!(
                    "Kon config.toml niet lezen: {}. Gebruik standaardwaarden.",
                    e
                ),
            }
            Self::default()
        } else {
            let default_config = Self::default();
            let toml_str = toml::to_string(&default_config).unwrap();
            if let Err(e) = fs::write(config_path, toml_str) {
                eprintln!("Kon standaard config.toml niet schrijven: {}", e);
            }
            default_config
        }
    }
}
