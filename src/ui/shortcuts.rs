use eframe::egui::{self, Key};
use std::collections::HashMap;

/// Geeft de volledige lijst van shortcut-acties met hun standaardtoetsen.
pub fn default_shortcuts() -> HashMap<String, String> {
    let mut m = HashMap::new();
    // Navigatie
    m.insert("Escape".into(), "Escape".into());
    m.insert("NavigateUp".into(), "ArrowUp".into());
    m.insert("NavigateDown".into(), "ArrowDown".into());
    m.insert("NavigateLeft".into(), "ArrowLeft".into());
    m.insert("NavigateRight".into(), "ArrowRight".into());
    m.insert("Select".into(), "Enter".into());
    // Afspelen
    m.insert("PlayPause".into(), "Space".into());
    m.insert("Skip".into(), "N".into());
    m.insert("AppendQueue".into(), "A".into());
    // Weergave / modi
    m.insert("ToggleView".into(), "T".into());
    m.insert("SortToggle".into(), "S".into());
    m.insert("GenreBrowse".into(), "G".into());
    m.insert("RecentAlbums".into(), "B".into());
    m.insert("RandomAlbum".into(), "R".into());
    m.insert("SearchMode".into(), "/".into());
    m.insert("Help".into(), "H".into());
    // Bewerken
    m.insert("TrackDetails".into(), "I".into());
    m.insert("MarkTrack".into(), "M".into());
    m.insert("ClearMarks".into(), "Shift+M".into());
    m.insert("OpenFolder".into(), "O".into());
    m.insert("SelectionBrowse".into(), "Z".into());
    m.insert("YearBrowse".into(), "Y".into());
    m.insert("ComposerBrowse".into(), "C".into());
    // Systeem
    m.insert("Rewind".into(), ";".into());
    m.insert("Forward".into(), "'".into());
    m.insert("RepeatToggle".into(), "X".into());
    m.insert("ShuffleToggle".into(), "F8".into());
    m.insert("LoopA".into(), "[".into());
    m.insert("LoopB".into(), "]".into());
    m.insert("ClearLoop".into(), "\\".into());
    m.insert("CompactToggle".into(), "F11".into());
    m.insert("QueueToggle".into(), "Q".into());
    m.insert("NowPlaying".into(), "F2".into());
    m.insert("VolumeUp".into(), "=".into());
    m.insert("VolumeDown".into(), "-".into());
    m.insert("ReconnectAudio".into(), "F6".into());
    m.insert("Rescan".into(), "F5".into());
    m.insert("RescanMarked".into(), "Shift+R".into());
    m
}

/// Controleer of de toets voor een bepaalde actie in de huidige frame is ingedrukt.
/// `config` is de `shortcuts` HashMap uit de config, `action` is de actienaam.
pub fn check_action(config: &HashMap<String, String>, ctx: &egui::Context, action: &str) -> bool {
    let key_str = match config.get(action) {
        Some(k) => k.as_str(),
        None => return false,
    };
    key_pressed(ctx, key_str)
}

/// Geef de leesbare toets voor een actie terug (voor in het help-scherm).
pub fn get_key_display(config: &HashMap<String, String>, action: &str) -> String {
    config
        .get(action)
        .cloned()
        .unwrap_or_else(|| "?".to_string())
}

/// Controleer of een bepaalde toets-representatie is ingedrukt.
fn key_pressed(ctx: &egui::Context, key_str: &str) -> bool {
    match key_str {
        // Speciale toetsen
        "Space" => ctx.input(|i| i.key_pressed(Key::Space)),
        "Enter" => ctx.input(|i| i.key_pressed(Key::Enter)),
        "Escape" => ctx.input(|i| i.key_pressed(Key::Escape)),
        "Tab" => ctx.input(|i| i.key_pressed(Key::Tab)),
        "Backspace" => ctx.input(|i| i.key_pressed(Key::Backspace)),
        "Delete" => ctx.input(|i| i.key_pressed(Key::Delete)),
        // Pijltjes
        "ArrowUp" => ctx.input(|i| i.key_pressed(Key::ArrowUp)),
        "ArrowDown" => ctx.input(|i| i.key_pressed(Key::ArrowDown)),
        "ArrowLeft" => ctx.input(|i| i.key_pressed(Key::ArrowLeft)),
        "ArrowRight" => ctx.input(|i| i.key_pressed(Key::ArrowRight)),
        // Functietoetsen
        "F1" => ctx.input(|i| i.key_pressed(Key::F1)),
        "F2" => ctx.input(|i| i.key_pressed(Key::F2)),
        "F3" => ctx.input(|i| i.key_pressed(Key::F3)),
        "F4" => ctx.input(|i| i.key_pressed(Key::F4)),
        "F5" => ctx.input(|i| i.key_pressed(Key::F5)),
        "F6" => ctx.input(|i| i.key_pressed(Key::F6)),
        "F7" => ctx.input(|i| i.key_pressed(Key::F7)),
        "F8" => ctx.input(|i| i.key_pressed(Key::F8)),
        "F9" => ctx.input(|i| i.key_pressed(Key::F9)),
        "F10" => ctx.input(|i| i.key_pressed(Key::F10)),
        "F11" => ctx.input(|i| i.key_pressed(Key::F11)),
        "F12" => ctx.input(|i| i.key_pressed(Key::F12)),
        // Speciale combinatie: Shift+M
        "Shift+M" => ctx.input(|i| i.key_pressed(Key::M) && i.modifiers.shift),
        // Speciale combinatie: Shift+R
        "Shift+R" => ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.shift),
        // Lettertoets: een enkele letter (hoofdletter = key, kleine letter = text event)
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            if c.is_ascii_uppercase() {
                let key = char_to_key(c).unwrap_or(Key::A);
                ctx.input(|i| i.key_pressed(key))
            } else {
                let lower = c.to_ascii_lowercase();
                let lower_key = char_to_key(lower).unwrap_or(Key::A);
                // Check both the key AND text event for robustness
                ctx.input(|i| {
                    i.key_pressed(lower_key)
                        || i.events
                            .iter()
                            .any(|e| matches!(e, egui::Event::Text(t) if t == s))
                })
            }
        }
        // Tekens zoals "/" of "?" — via Event::Text
        s if s == ";" => ctx.input(|i| {
            i.key_pressed(Key::Semicolon)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == ";"))
        }),
        s if s == "'" => ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "'"))
        }),
        s if s == "=" => ctx.input(|i| {
            i.key_pressed(Key::Plus)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "=" || t == "+"))
        }),
        s if s == "-" => ctx.input(|i| {
            i.key_pressed(Key::Minus)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "-"))
        }),
        s if s == "/" => ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "/"))
        }),
        s if s == "?" => ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "?"))
        }),
        s if s == "[" => ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "["))
        }),
        s if s == "]" => ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "]"))
        }),
        s if s == "\\" => ctx.input(|i| {
            i.key_pressed(Key::Backslash)
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "\\"))
        }),
        _ => false,
    }
}

/// Controleer of een toetswaarde bekend is in `key_pressed`.
fn is_valid_key_value(key: &str) -> bool {
    match key {
        "Space" | "Enter" | "Escape" | "Tab" | "Backspace" | "Delete" | "ArrowUp" | "ArrowDown"
        | "ArrowLeft" | "ArrowRight" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8"
        | "F9" | "F10" | "F11" | "F12" | "Shift+M" | ";" | "'" | "=" | "-" | "/" | "?" | "["
        | "]" | "\\" => true,
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            c.is_ascii_alphabetic()
        }
        _ => false,
    }
}

/// Valideer een shortcuts HashMap en geef een lijst met foutmeldingen terug.
pub fn validate_shortcuts(shortcuts: &HashMap<String, String>) -> Vec<String> {
    let mut errors = Vec::new();
    let defaults = default_shortcuts();

    for (action, key) in shortcuts {
        if !defaults.contains_key(action) {
            errors.push(format!(
                "Onbekende actie \"{}\" (toets: \"{}\")",
                action, key
            ));
        }
    }

    for (action, default_key) in &defaults {
        if !shortcuts.contains_key(action) {
            errors.push(format!(
                "Actie \"{}\" ontbreekt (standaard: \"{}\")",
                action, default_key
            ));
        }
    }

    for (action, key) in shortcuts {
        if !is_valid_key_value(key) {
            errors.push(format!(
                "Actie \"{}\" heeft ongeldige toets \"{}\"",
                action, key
            ));
        }
    }

    let mut seen: HashMap<&String, Vec<&String>> = HashMap::new();
    for (action, key) in shortcuts {
        seen.entry(key).or_default().push(action);
    }
    for (key, actions) in &seen {
        if actions.len() > 1 {
            errors.push(format!(
                "Dubbele toets \"{}\" voor acties: {}",
                key,
                actions
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    errors
}

fn char_to_key(c: char) -> Option<Key> {
    match c {
        'A' | 'a' => Some(Key::A),
        'B' | 'b' => Some(Key::B),
        'C' | 'c' => Some(Key::C),
        'D' | 'd' => Some(Key::D),
        'E' | 'e' => Some(Key::E),
        'F' | 'f' => Some(Key::F),
        'G' | 'g' => Some(Key::G),
        'H' | 'h' => Some(Key::H),
        'I' | 'i' => Some(Key::I),
        'J' | 'j' => Some(Key::J),
        'K' | 'k' => Some(Key::K),
        'L' | 'l' => Some(Key::L),
        'M' | 'm' => Some(Key::M),
        'N' | 'n' => Some(Key::N),
        'O' | 'o' => Some(Key::O),
        'P' | 'p' => Some(Key::P),
        'Q' | 'q' => Some(Key::Q),
        'R' | 'r' => Some(Key::R),
        'S' | 's' => Some(Key::S),
        'T' | 't' => Some(Key::T),
        'U' | 'u' => Some(Key::U),
        'V' | 'v' => Some(Key::V),
        'W' | 'w' => Some(Key::W),
        'X' | 'x' => Some(Key::X),
        'Y' | 'y' => Some(Key::Y),
        'Z' | 'z' => Some(Key::Z),
        _ => None,
    }
}
