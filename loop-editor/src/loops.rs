use serde::{Deserialize, Serialize};
use std::path::Path;

/// Een opgeslagen loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLoop {
    pub track_path: String,
    pub label: String,
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
    pub pitch_semitones: f32,
    pub tempo: f32,
}

const LOOPS_FILE: &str = "loops.json";

/// Laad opgeslagen loops van schijf.
pub fn load_loops() -> Vec<SavedLoop> {
    match std::fs::read_to_string(LOOPS_FILE) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Sla loops weg naar schijf.
pub fn save_loops(loops: &[SavedLoop]) {
    if let Ok(json) = serde_json::to_string_pretty(loops) {
        let _ = std::fs::write(LOOPS_FILE, json);
    }
}

/// Voeg een loop toe en sla op. Geeft de nieuwe lijst terug.
pub fn add_loop(loops: &mut Vec<SavedLoop>, saved: SavedLoop) {
    loops.push(saved);
    save_loops(loops);
}

/// Verwijder een loop op index en sla op.
pub fn remove_loop(loops: &mut Vec<SavedLoop>, index: usize) {
    if index < loops.len() {
        loops.remove(index);
        save_loops(loops);
    }
}

/// Genereer een uniek label voor een nieuwe loop.
pub fn generate_label(track_path: &str, loops: &[SavedLoop]) -> String {
    let file_stem = Path::new(track_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Onbekend".to_string());

    // Tel bestaande loops voor deze track
    let count = loops.iter().filter(|l| l.track_path == track_path).count();

    if count == 0 {
        format!("{} - Loop 1", file_stem)
    } else {
        format!("{} - Loop {}", file_stem, count + 1)
    }
}
