use serde::{Deserialize, Serialize};
use std::path::Path;

/// Een opgeslagen loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLoop {
    pub track_path: String,
    pub label: String,
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
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

/// Pas de paden in opgeslagen loops aan na een schijfletterwijziging.
/// Geeft de bijgewerkte lijst terug (en slaat deze op) wanneer er iets wijzigde.
pub fn remap_loops(old_root: &str, new_root: &str) -> Option<Vec<SavedLoop>> {
    let mut loops = load_loops();
    let mut changed = false;
    for l in &mut loops {
        let remapped = crate::scanner::remap_one_path(&l.track_path, old_root, new_root);
        if remapped != l.track_path {
            l.track_path = remapped;
            changed = true;
        }
    }
    if changed {
        save_loops(&loops);
        log::info!("Opgeslagen loops bijgewerkt naar nieuwe schijfletter.");
        Some(loops)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_loop(track_path: &str, label: &str) -> SavedLoop {
        SavedLoop {
            track_path: track_path.to_string(),
            label: label.to_string(),
            loop_a_secs: 10.0,
            loop_b_secs: 20.0,
        }
    }

    #[test]
    fn generate_label_first_loop() {
        let label = generate_label("/music/test/song.flac", &[]);
        assert_eq!(label, "song - Loop 1");
    }

    #[test]
    fn generate_label_without_path() {
        let label = generate_label("", &[]);
        // Leeg pad → file_stem() geeft None → "Onbekend"
        assert_eq!(label, "Onbekend - Loop 1");
    }

    #[test]
    fn generate_label_second_loop() {
        let loops = vec![sample_loop("/music/test/song.flac", "song - Loop 1")];
        let label = generate_label("/music/test/song.flac", &loops);
        assert_eq!(label, "song - Loop 2");
    }

    #[test]
    fn generate_label_third_loop() {
        let loops = vec![
            sample_loop("/music/test/song.flac", "song - Loop 1"),
            sample_loop("/music/test/song.flac", "song - Loop 2"),
        ];
        let label = generate_label("/music/test/song.flac", &loops);
        assert_eq!(label, "song - Loop 3");
    }

    #[test]
    fn generate_label_other_track_unaffected() {
        let loops = vec![sample_loop("/other/file.mp3", "other - Loop 1")];
        let label = generate_label("/music/test/song.flac", &loops);
        assert_eq!(label, "song - Loop 1");
    }

    #[test]
    fn saved_loop_struct() {
        let l = sample_loop("/track.flac", "test");
        assert_eq!(l.track_path, "/track.flac");
        assert_eq!(l.label, "test");
        assert_eq!(l.loop_a_secs, 10.0);
        assert_eq!(l.loop_b_secs, 20.0);
    }
}
