use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const SHORTCUTS_FILE: &str = "shortcuts.json";
const CURRENT_VERSION: u32 = 1;

// ─────────────────────────────────────────────────────────────────────────────
// Alle mogelijke acties in de app
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    // Playback
    PlayPause,
    Stop,
    SeekForward,
    SeekBackward,

    // Loop
    SetLoopA,
    SetLoopB,
    ClearLoop,
    ToggleLoopBypass,
    NudgeLoopLeft,
    NudgeLoopRight,
    ToggleLoopPoint,
    NudgeALeft,
    NudgeARight,
    NudgeBLeft,
    NudgeBRight,
    NudgePlayheadLeft,
    NudgePlayheadRight,
    CenterLoop,
    SaveLoop,

    // Markers
    AddSectionMarker,
    AddMeasureMarker,
    AddBeatMarker,
    DeleteNearestMarker,

    // View
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ShowShortcuts,

    // File
    OpenFile,
    Undo,
    Redo,
    RestartLoop,
}

impl ShortcutAction {
    /// Menselijke leesbare naam voor de UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PlayPause => "Play / Pause",
            Self::Stop => "Stop playback",
            Self::SeekForward => "Seek forward 2s",
            Self::SeekBackward => "Seek backward 2s",
            Self::SetLoopA => "Set loop A at playhead",
            Self::SetLoopB => "Set loop B at playhead",
            Self::ClearLoop => "Clear A-B loop",
            Self::ToggleLoopBypass => "Toggle loop bypass",
            Self::NudgeLoopLeft => "Nudge loop left",
            Self::NudgeLoopRight => "Nudge loop right",
            Self::ToggleLoopPoint => "Toggle loop point (1e=set, 2e=A-B)",
            Self::NudgeALeft => "Nudge marker A left",
            Self::NudgeARight => "Nudge marker A right",
            Self::NudgeBLeft => "Nudge marker B left",
            Self::NudgeBRight => "Nudge marker B right",
            Self::NudgePlayheadLeft => "Nudge playhead left",
            Self::NudgePlayheadRight => "Nudge playhead right",
            Self::CenterLoop => "Center view on loop",
            Self::SaveLoop => "Save current loop",
            Self::AddSectionMarker => "Add section marker",
            Self::AddMeasureMarker => "Add measure marker",
            Self::AddBeatMarker => "Add beat marker",
            Self::DeleteNearestMarker => "Delete nearest marker",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ResetZoom => "Reset zoom/scroll",
            Self::ShowShortcuts => "Show shortcuts help",
            Self::OpenFile => "Open audio file",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::RestartLoop => "Restart loop (seek to A & play)",
        }
    }

    /// Categorie voor groepering in de UI
    pub fn category(&self) -> &'static str {
        match self {
            Self::PlayPause | Self::Stop | Self::SeekForward | Self::SeekBackward => "Playback",
            Self::SetLoopA
            | Self::SetLoopB
            | Self::ClearLoop
            | Self::ToggleLoopBypass
            | Self::NudgeLoopLeft
            | Self::NudgeLoopRight
            | Self::ToggleLoopPoint
            | Self::NudgeALeft
            | Self::NudgeARight
            | Self::NudgeBLeft
            | Self::NudgeBRight
            | Self::NudgePlayheadLeft
            | Self::NudgePlayheadRight
            | Self::CenterLoop
            | Self::SaveLoop => "Loop",
            Self::AddSectionMarker
            | Self::AddMeasureMarker
            | Self::AddBeatMarker
            | Self::DeleteNearestMarker => "Markers",
            Self::ZoomIn | Self::ZoomOut | Self::ResetZoom | Self::ShowShortcuts => "View",
            Self::OpenFile => "File",
            Self::Undo | Self::Redo | Self::RestartLoop => "Edit",
        }
    }

    /// Alle acties (voor iteratie)
    pub fn all() -> &'static [ShortcutAction] {
        &[
            Self::PlayPause,
            Self::Stop,
            Self::SeekForward,
            Self::SeekBackward,
            Self::SetLoopA,
            Self::SetLoopB,
            Self::ClearLoop,
            Self::ToggleLoopBypass,
            Self::NudgeLoopLeft,
            Self::NudgeLoopRight,
            Self::ToggleLoopPoint,
            Self::NudgeALeft,
            Self::NudgeARight,
            Self::NudgeBLeft,
            Self::NudgeBRight,
            Self::NudgePlayheadLeft,
            Self::NudgePlayheadRight,
            Self::AddSectionMarker,
            Self::AddMeasureMarker,
            Self::AddBeatMarker,
            Self::DeleteNearestMarker,
            Self::ZoomIn,
            Self::ZoomOut,
            Self::ResetZoom,
            Self::ShowShortcuts,
            Self::OpenFile,
            Self::Undo,
            Self::Redo,
            Self::RestartLoop,
            Self::CenterLoop,
            Self::SaveLoop,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Een key binding: toets + modifiers
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: SerializableKey,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyBinding {
    pub fn new(key: SerializableKey) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Menselijke leesbare representatie (bijv. "Ctrl+S", "Space", "Shift+→")
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.display_name());
        parts.join("+")
    }

    /// Check of deze binding overeenkomt met de huidige input state
    pub fn matches(&self, input: &egui::InputState) -> bool {
        let mods = input.modifiers;
        let mods_match = self.ctrl == mods.ctrl && self.shift == mods.shift && self.alt == mods.alt;
        mods_match && input.key_pressed(self.key.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Serializeerbare wrapper rond egui::Key
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializableKey {
    Space,
    Enter,
    Escape,
    Backspace,
    Tab,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    OpenBracket,
    CloseBracket,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl SerializableKey {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Escape => "Esc",
            Self::Backspace => "Backspace",
            Self::Tab => "Tab",
            Self::ArrowLeft => "◀",
            Self::ArrowRight => "▶",
            Self::ArrowUp => "↑",
            Self::ArrowDown => "↓",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::OpenBracket => "[",
            Self::CloseBracket => "]",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
        }
    }
}

// Conversie naar egui::Key
impl From<SerializableKey> for egui::Key {
    fn from(k: SerializableKey) -> Self {
        match k {
            SerializableKey::Space => egui::Key::Space,
            SerializableKey::Enter => egui::Key::Enter,
            SerializableKey::Escape => egui::Key::Escape,
            SerializableKey::Backspace => egui::Key::Backspace,
            SerializableKey::Tab => egui::Key::Tab,
            SerializableKey::ArrowLeft => egui::Key::ArrowLeft,
            SerializableKey::ArrowRight => egui::Key::ArrowRight,
            SerializableKey::ArrowUp => egui::Key::ArrowUp,
            SerializableKey::ArrowDown => egui::Key::ArrowDown,
            SerializableKey::A => egui::Key::A,
            SerializableKey::B => egui::Key::B,
            SerializableKey::C => egui::Key::C,
            SerializableKey::D => egui::Key::D,
            SerializableKey::E => egui::Key::E,
            SerializableKey::F => egui::Key::F,
            SerializableKey::G => egui::Key::G,
            SerializableKey::H => egui::Key::H,
            SerializableKey::I => egui::Key::I,
            SerializableKey::J => egui::Key::J,
            SerializableKey::K => egui::Key::K,
            SerializableKey::L => egui::Key::L,
            SerializableKey::M => egui::Key::M,
            SerializableKey::N => egui::Key::N,
            SerializableKey::O => egui::Key::O,
            SerializableKey::P => egui::Key::P,
            SerializableKey::Q => egui::Key::Q,
            SerializableKey::R => egui::Key::R,
            SerializableKey::S => egui::Key::S,
            SerializableKey::T => egui::Key::T,
            SerializableKey::U => egui::Key::U,
            SerializableKey::V => egui::Key::V,
            SerializableKey::W => egui::Key::W,
            SerializableKey::X => egui::Key::X,
            SerializableKey::Y => egui::Key::Y,
            SerializableKey::Z => egui::Key::Z,
            SerializableKey::Num0 => egui::Key::Num0,
            SerializableKey::Num1 => egui::Key::Num1,
            SerializableKey::Num2 => egui::Key::Num2,
            SerializableKey::Num3 => egui::Key::Num3,
            SerializableKey::Num4 => egui::Key::Num4,
            SerializableKey::Num5 => egui::Key::Num5,
            SerializableKey::Num6 => egui::Key::Num6,
            SerializableKey::Num7 => egui::Key::Num7,
            SerializableKey::Num8 => egui::Key::Num8,
            SerializableKey::Num9 => egui::Key::Num9,
            SerializableKey::OpenBracket => egui::Key::OpenBracket,
            SerializableKey::CloseBracket => egui::Key::CloseBracket,
            SerializableKey::F1 => egui::Key::F1,
            SerializableKey::F2 => egui::Key::F2,
            SerializableKey::F3 => egui::Key::F3,
            SerializableKey::F4 => egui::Key::F4,
            SerializableKey::F5 => egui::Key::F5,
            SerializableKey::F6 => egui::Key::F6,
            SerializableKey::F7 => egui::Key::F7,
            SerializableKey::F8 => egui::Key::F8,
            SerializableKey::F9 => egui::Key::F9,
            SerializableKey::F10 => egui::Key::F10,
            SerializableKey::F11 => egui::Key::F11,
            SerializableKey::F12 => egui::Key::F12,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// De shortcuts config
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default)]
    pub version: u32,
    pub bindings: HashMap<ShortcutAction, KeyBinding>,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Playback
        bindings.insert(
            ShortcutAction::PlayPause,
            KeyBinding::new(SerializableKey::Space),
        );
        bindings.insert(
            ShortcutAction::Stop,
            KeyBinding::new(SerializableKey::Escape),
        );
        bindings.insert(
            ShortcutAction::SeekForward,
            KeyBinding::new(SerializableKey::ArrowUp),
        );
        bindings.insert(
            ShortcutAction::SeekBackward,
            KeyBinding::new(SerializableKey::ArrowDown),
        );

        // Loop
        bindings.insert(
            ShortcutAction::SetLoopA,
            KeyBinding::new(SerializableKey::OpenBracket),
        );
        bindings.insert(
            ShortcutAction::SetLoopB,
            KeyBinding::new(SerializableKey::CloseBracket),
        );
        bindings.insert(
            ShortcutAction::ClearLoop,
            KeyBinding::new(SerializableKey::Backspace).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ToggleLoopBypass,
            KeyBinding::new(SerializableKey::B).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::NudgeLoopLeft,
            KeyBinding::new(SerializableKey::ArrowLeft).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgeLoopRight,
            KeyBinding::new(SerializableKey::ArrowRight).with_shift(),
        );
        bindings.insert(
            ShortcutAction::ToggleLoopPoint,
            KeyBinding::new(SerializableKey::OpenBracket),
        );
        bindings.insert(
            ShortcutAction::NudgeALeft,
            KeyBinding::new(SerializableKey::J),
        );
        bindings.insert(
            ShortcutAction::NudgeARight,
            KeyBinding::new(SerializableKey::J).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgeBLeft,
            KeyBinding::new(SerializableKey::L),
        );
        bindings.insert(
            ShortcutAction::NudgeBRight,
            KeyBinding::new(SerializableKey::L).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgePlayheadLeft,
            KeyBinding::new(SerializableKey::ArrowLeft),
        );
        bindings.insert(
            ShortcutAction::NudgePlayheadRight,
            KeyBinding::new(SerializableKey::ArrowRight),
        );

        // Markers
        bindings.insert(
            ShortcutAction::AddSectionMarker,
            KeyBinding::new(SerializableKey::S),
        );
        bindings.insert(
            ShortcutAction::AddMeasureMarker,
            KeyBinding::new(SerializableKey::M),
        );
        bindings.insert(
            ShortcutAction::AddBeatMarker,
            KeyBinding::new(SerializableKey::B),
        );
        bindings.insert(
            ShortcutAction::DeleteNearestMarker,
            KeyBinding::new(SerializableKey::Backspace),
        );

        // View
        bindings.insert(
            ShortcutAction::ZoomIn,
            KeyBinding::new(SerializableKey::Num0).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ZoomOut,
            KeyBinding::new(SerializableKey::Num0)
                .with_ctrl()
                .with_shift(),
        );
        bindings.insert(
            ShortcutAction::ResetZoom,
            KeyBinding::new(SerializableKey::R).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ShowShortcuts,
            KeyBinding::new(SerializableKey::F1),
        );

        // File
        bindings.insert(
            ShortcutAction::OpenFile,
            KeyBinding::new(SerializableKey::O).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::Undo,
            KeyBinding::new(SerializableKey::Z).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::Redo,
            KeyBinding::new(SerializableKey::Z).with_ctrl().with_shift(),
        );
        bindings.insert(
            ShortcutAction::RestartLoop,
            KeyBinding::new(SerializableKey::Enter),
        );
        bindings.insert(
            ShortcutAction::CenterLoop,
            KeyBinding::new(SerializableKey::C),
        );
        bindings.insert(
            ShortcutAction::SaveLoop,
            KeyBinding::new(SerializableKey::S).with_ctrl(),
        );

        Self {
            version: CURRENT_VERSION,
            bindings,
        }
    }
}

impl ShortcutsConfig {
    /// Laad config uit bestand, of gebruik defaults als het niet bestaat
    pub fn load() -> Self {
        let path = Path::new(SHORTCUTS_FILE);
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(json) => match serde_json::from_str::<ShortcutsConfig>(&json) {
                    Ok(config) => {
                        // Als de versie niet matcht, reset naar defaults
                        if config.version != CURRENT_VERSION {
                            let defaults = Self::default();
                            let _ = defaults.save();
                            return defaults;
                        }
                        // Merge met defaults: ontbrekende acties krijgen hun default
                        let defaults = Self::default();
                        let mut merged: Self = config;
                        for action in ShortcutAction::all() {
                            if !merged.bindings.contains_key(action) {
                                if let Some(binding) = defaults.bindings.get(action) {
                                    merged.bindings.insert(*action, *binding);
                                }
                            }
                        }
                        return merged;
                    }
                    Err(e) => {
                        eprintln!(
                            "[shortcuts] Kon shortcuts.json niet parsen: {}. Gebruik defaults.",
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[shortcuts] Kon shortcuts.json niet lezen: {}. Gebruik defaults.",
                        e
                    );
                }
            }
        }
        let defaults = Self::default();
        // Sla defaults op zodat het bestand bestaat voor de gebruiker
        let _ = defaults.save();
        defaults
    }

    /// Sla config op naar bestand
    pub fn save(&self) -> Result<(), String> {
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                fs::write(SHORTCUTS_FILE, json).map_err(|e| format!("Kon niet opslaan: {}", e))
            }
            Err(e) => Err(format!("Kon niet serialiseren: {}", e)),
        }
    }

    /// Check of een actie geactiveerd is in de huidige input state
    pub fn is_pressed(&self, action: ShortcutAction, input: &egui::InputState) -> bool {
        if let Some(binding) = self.bindings.get(&action) {
            binding.matches(input)
        } else {
            false
        }
    }

    /// Geef de binding voor een actie (voor display in UI)
    pub fn binding_for(&self, action: ShortcutAction) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }

    /// Update een binding en sla op
    pub fn set_binding(
        &mut self,
        action: ShortcutAction,
        binding: KeyBinding,
    ) -> Result<(), String> {
        self.bindings.insert(action, binding);
        self.save()
    }

    /// Reset een actie naar default
    pub fn reset_action(&mut self, action: ShortcutAction) -> Result<(), String> {
        let defaults = Self::default();
        if let Some(binding) = defaults.bindings.get(&action) {
            self.bindings.insert(action, *binding);
            self.save()
        } else {
            Ok(())
        }
    }

    /// Reset alles naar defaults
    pub fn reset_all(&mut self) -> Result<(), String> {
        *self = Self::default();
        self.save()
    }

    /// Check of een key+modifiers combinatie al in gebruik is (voor conflict-detectie)
    pub fn find_conflict(
        &self,
        binding: &KeyBinding,
        exclude: ShortcutAction,
    ) -> Option<ShortcutAction> {
        for (action, existing) in &self.bindings {
            if *action != exclude && existing == binding {
                return Some(*action);
            }
        }
        None
    }
}
