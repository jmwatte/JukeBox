use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::waveform::Marker;

// ───────────────────────────────────────────────
// Data-model
// ───────────────────────────────────────────────

/// De volledige bibliotheek: tracks met loops, markers en notities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub tracks: Vec<TrackData>,
}

impl Library {
    pub fn empty() -> Self {
        Self { tracks: Vec::new() }
    }

    /// Zoek track-data voor een pad. Maak aan als die nog niet bestaat.
    pub fn track_for_path(&mut self, track_path: &str) -> &mut TrackData {
        let idx = self.tracks.iter().position(|t| t.track_path == track_path);
        if let Some(i) = idx {
            &mut self.tracks[i]
        } else {
            let label = Path::new(track_path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Onbekend".to_string());
            self.tracks.push(TrackData {
                track_path: track_path.to_string(),
                label,
                markers: Vec::new(),
                loops: Vec::new(),
            });
            self.tracks.last_mut().unwrap()
        }
    }

    /// Genereer een uniek label voor een nieuwe loop in een track.
    pub fn generate_label(&self, track_path: &str) -> String {
        let file_stem = Path::new(track_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Onbekend".to_string());

        // Tel bestaande loops voor deze track
        let count = self
            .tracks
            .iter()
            .filter(|t| t.track_path == track_path)
            .flat_map(|t| &t.loops)
            .count();

        if count == 0 {
            format!("{} - Loop 1", file_stem)
        } else {
            format!("{} - Loop {}", file_stem, count + 1)
        }
    }
}

/// Metadata voor één audiobestand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    pub track_path: String,
    pub label: String,
    pub markers: Vec<Marker>,
    pub loops: Vec<SavedLoop>,
}

/// Een opgeslagen loop (hoort bij een TrackData).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLoop {
    pub label: String,
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
    /// Aantal halve tonen pitch-shift.
    #[serde(default)]
    pub pitch_semitones: f32,
    /// Tempo-factor.
    #[serde(default = "default_tempo")]
    pub tempo: f32,
    /// Notities voor deze loop (akkoorden, noten, etc.)
    #[serde(default)]
    pub notes: String,
}

fn default_tempo() -> f32 {
    1.0
}

// ───────────────────────────────────────────────
// Laden / Opslaan
// ───────────────────────────────────────────────

const LIBRARY_FILE: &str = "library.json";
const OLD_LOOPS_FILE: &str = "loops.json";

/// Laad de bibliotheek van schijf. Migreert oude loops.json indien nodig.
pub fn load_library() -> Library {
    // Probeer nieuwe format eerst
    if let Ok(json) = std::fs::read_to_string(LIBRARY_FILE) {
        if let Ok(lib) = serde_json::from_str(&json) {
            return lib;
        }
    }

    // Fallback: migreer oude loops.json
    if let Ok(json) = std::fs::read_to_string(OLD_LOOPS_FILE) {
        if let Ok(old_loops) = serde_json::from_str::<Vec<OldSavedLoop>>(&json) {
            let mut lib = Library::empty();
            for old in old_loops {
                let track = lib.track_for_path(&old.track_path);
                track.loops.push(SavedLoop {
                    label: old.label,
                    loop_a_secs: old.loop_a_secs,
                    loop_b_secs: old.loop_b_secs,
                    pitch_semitones: old.pitch_semitones,
                    tempo: old.tempo,
                    notes: String::new(),
                });
            }
            // Sla nieuwe format meteen op
            save_library(&lib);
            // Verwijder oud bestand (optioneel, maar netjes)
            let _ = std::fs::remove_file(OLD_LOOPS_FILE);
            return lib;
        }
    }

    Library::empty()
}

/// Sla de bibliotheek weg naar schijf.
pub fn save_library(library: &Library) {
    if let Ok(json) = serde_json::to_string_pretty(library) {
        let _ = std::fs::write(LIBRARY_FILE, json);
    }
}

// ───────────────────────────────────────────────
// Oude struct (alleen voor migratie)
// ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldSavedLoop {
    track_path: String,
    label: String,
    loop_a_secs: f32,
    loop_b_secs: f32,
    #[serde(default)]
    pitch_semitones: f32,
    #[serde(default = "default_tempo")]
    tempo: f32,
}
