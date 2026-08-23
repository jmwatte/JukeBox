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
    m.insert("CopyPath".into(), "Ctrl+C".into());
    m.insert("ToggleFavorite".into(), "F".into());
    m.insert("FavoritesBrowse".into(), "Shift+F".into());
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
    m.insert("AlwaysOnTop".into(), "F12".into());
    m.insert("QueueToggle".into(), "Q".into());
    m.insert("NowPlaying".into(), "F2".into());
    m.insert("VolumeUp".into(), "=".into());
    m.insert("VolumeDown".into(), "-".into());
    m.insert("ReconnectAudio".into(), "F6".into());
    m.insert("Rescan".into(), "F5".into());
    m.insert("RescanMarked".into(), "Shift+R".into());
    m.insert("WaveformOpen".into(), "0".into());
    m.insert("WaveformSaveLoop".into(), "Ctrl+S".into());
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
///
/// Belangrijk: leestekens zoals `;` en `'` worden vóór de generieke letter-arm
/// afgehandeld. egui mapt ze op specifieke `Key`-varianten (`Key::Semicolon`,
/// `Key::Quote`, …); de `Event::Text`-fallback dekt toetsenbordindelingen waar
/// het teken op een andere fysieke toets zit (bv. AZERTY).
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
        // Cijfers
        "0" => ctx.input(|i| i.key_pressed(Key::Num0)),
        // Leestekens (vóór de generieke letter-arm!)
        ";" => punctuation(ctx, &[Key::Semicolon, Key::Comma], &[";", ","]),
        "'" => punctuation(ctx, &[Key::Quote, Key::Period], &["'", "."]),
        "=" => punctuation(ctx, &[Key::Equals, Key::Plus], &["=", "+"]),
        "-" => punctuation(ctx, &[Key::Minus], &["-"]),
        "/" => punctuation(ctx, &[Key::Slash], &["/"]),
        "?" => punctuation(ctx, &[Key::Questionmark], &["?"]),
        "[" => punctuation(ctx, &[Key::OpenBracket], &["["]),
        "]" => punctuation(ctx, &[Key::CloseBracket], &["]"]),
        "\\" => punctuation(ctx, &[Key::Backslash], &["\\"]),
        "," => punctuation(ctx, &[Key::Comma], &[","]),
        "." => punctuation(ctx, &[Key::Period], &["."]),
        // Shift+Letter combinaties (generiek)
        s if s.starts_with("Shift+") && s.len() == 7 => {
            let c = s.chars().nth(6).unwrap();
            if let Some(key) = char_to_key(c) {
                ctx.input(|i| i.key_pressed(key) && i.modifiers.shift)
            } else {
                false
            }
        }
        // Ctrl+Letter combinaties (generiek)
        s if s.starts_with("Ctrl+") && s.len() == 6 => {
            let c = s.chars().nth(5).unwrap();
            if let Some(key) = char_to_key(c) {
                ctx.input(|i| i.key_pressed(key) && i.modifiers.ctrl)
            } else {
                false
            }
        }
        // Lettertoetsen: een enkele letter
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
        _ => false,
    }
}

/// Controleer een leesteken: via het bijbehorende `Key`-event óf via een
/// `Event::Text`-event (voor toetsenbordindelingen waar het teken op een
/// andere fysieke toets zit). `keys`/`texts` zijn fallbacks voor verwante
/// tekens (bv. `;` en `,` op AZERTY).
fn punctuation(ctx: &egui::Context, keys: &[Key], texts: &[&str]) -> bool {
    ctx.input(|i| {
        keys.iter().any(|k| i.key_pressed(*k))
            || i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if texts.iter().any(|x| x == t)))
    })
}

/// Controleer of een toetswaarde bekend is in `key_pressed`.
fn is_valid_key_value(key: &str) -> bool {
    match key {
        "Space" | "Enter" | "Escape" | "Tab" | "Backspace" | "Delete" | "ArrowUp" | "ArrowDown"
        | "ArrowLeft" | "ArrowRight" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8"
        | "F9" | "F10" | "F11" | "F12" | ";" | "'" | "=" | "-" | "/" | "?" | "[" | "]" | "\\"
        | "0" | "," | "." => true,
        // Ctrl+Letter combinaties
        s if s.starts_with("Ctrl+") && s.len() == 6 => {
            s.chars().nth(5).map_or(false, |c| c.is_ascii_alphabetic())
        }
        // Shift+Letter combinaties
        s if s.starts_with("Shift+") && s.len() == 7 => {
            s.chars().nth(6).map_or(false, |c| c.is_ascii_alphabetic())
        }
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

/// Herstel alleen de foutieve shortcuts naar hun standaardwaarde.
/// Geldige custom shortcuts blijven behouden.
pub fn repair_shortcuts(shortcuts: &mut HashMap<String, String>, errors: &[String]) {
    let defaults = default_shortcuts();

    for error in errors {
        // Error-patronen:
        // "Onbekende actie \"Foo\" (toets: \"X\")" → verwijder
        if let Some(start) = error.find("\"") {
            if let Some(end) = error[start + 1..].find("\"") {
                let action = &error[start + 1..start + 1 + end];

                if error.starts_with("Onbekende actie") {
                    shortcuts.remove(action);
                } else if error.starts_with("Actie") && error.contains("ontbreekt") {
                    if let Some(default_key) = defaults.get(action) {
                        shortcuts.insert(action.to_string(), default_key.clone());
                    }
                } else if error.starts_with("Actie") && error.contains("ongeldige toets") {
                    if let Some(default_key) = defaults.get(action) {
                        shortcuts.insert(action.to_string(), default_key.clone());
                    }
                } else if error.starts_with("Dubbele toets") {
                    // Reset alle acties in deze foutmelding
                    for a in error.split_whitespace() {
                        let a = a.trim_matches('"').to_string();
                        if defaults.contains_key(&a) {
                            if let Some(default_key) = defaults.get(&a) {
                                shortcuts.insert(a, default_key.clone());
                            }
                        }
                    }
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Event;

    /// Draai een egui-frame met gesimuleerde toetsgebeurtenissen en geef de Context terug.
    fn run_events(events: Vec<Event>) -> egui::Context {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut full_output = ctx.run_ui(raw, |_| {});
        // In egui 0.36 panikt epaint als de Context gedropt wordt met onverwerkte
        // textures; in tests verwerken we ze niet, dus clear ze expliciet.
        full_output.textures_delta.clear();
        ctx
    }

    fn key_event(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    fn text_event(text: &str) -> Event {
        Event::Text(text.to_string())
    }

    /// Windows/winit stuurt voor een lettertoets ZOWEL een Key-event als een Text-event.
    fn press(key: Key, text: &str) -> Vec<Event> {
        vec![key_event(key), text_event(text)]
    }

    #[test]
    fn semicolon_is_detected() {
        // ';' wordt door egui-winit gemapt naar Key::Semicolon + Text ";"
        let ctx = run_events(press(Key::Semicolon, ";"));
        assert!(key_pressed(&ctx, ";"), "';' moet Rewind triggeren");
    }

    #[test]
    fn semicolon_key_only_detected() {
        // Ook zonder Text-event moet de Key alleen al werken
        let ctx = run_events(vec![key_event(Key::Semicolon)]);
        assert!(key_pressed(&ctx, ";"));
    }

    #[test]
    fn apostrophe_is_detected() {
        // ''' wordt door egui-winit gemapt naar Key::Quote + Text "'"
        let ctx = run_events(press(Key::Quote, "'"));
        assert!(key_pressed(&ctx, "'"), "''' moet Forward triggeren");
    }

    #[test]
    fn key_only_event_for_quote_detected() {
        // Sommige backends sturen alleen een Key-event: Key::Quote moet volstaan
        let ctx = run_events(vec![key_event(Key::Quote)]);
        assert!(key_pressed(&ctx, "'"), "Key::Quote moet Forward triggeren");
    }

    #[test]
    fn period_is_fallback_for_forward() {
        // De bestaande AZERTY-fallback: '.' wordt ook geaccepteerd voor '''
        let ctx = run_events(press(Key::Period, "."));
        assert!(key_pressed(&ctx, "'"));
    }

    #[test]
    fn text_only_event_still_detected() {
        // Sommige backends sturen alleen een Text-event
        let ctx = run_events(vec![text_event("'")]);
        assert!(key_pressed(&ctx, "'"));
    }

    #[test]
    fn letter_a_does_not_trigger_rewind() {
        // Regressie: vroeger viel ';' in de generieke letter-arm en checkte die
        // Key::A — waardoor de A-toets Rewind triggert. Dat mag niet meer.
        let ctx = run_events(press(Key::A, "a"));
        assert!(!key_pressed(&ctx, ";"));
        assert!(!key_pressed(&ctx, "'"));
    }

    #[test]
    fn check_action_for_rewind() {
        let mut shortcuts = HashMap::new();
        shortcuts.insert("Rewind".to_string(), ";".to_string());
        let ctx = run_events(press(Key::Semicolon, ";"));
        assert!(check_action(&shortcuts, &ctx, "Rewind"));
    }
}
