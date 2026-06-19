# Migratieplan: Waveform Loop Editor als standalone app

## Doel

Maak een zelfstandige applicatie in de map `geminijukebox/loop-editor/` die alle waveform- en
looping-functionaliteit uit de hoofd-JukeBox-app bundelt. De nieuwe app moet:

1. Een audio-bestand kunnen openen (via bestandsdialoog of drag & drop)
2. De waveform van dat bestand tonen met scrollen & zoomen
3. A-B loop markers kunnen plaatsen en verslepen
4. De A-B loop kunnen afspelen met pitch- en tempo-aanpassing (rubato)
5. Loops kunnen opslaan en laden (loops.json)
6. De playhead kunnen verslepen, ook tijdens pauze (de seek-bugfix meenemen)

**Niet** nodig uit de hoofd-app: library scanner, music player, queue, batch editing, filters,
navigatie, album covers, config, search, etc.

---

## Overzicht bestaande code die hergebruikt wordt

| Bronbestand (JukeBox) | Wat                    | Naar (loop-editor)    |
|------------------------|------------------------|-----------------------|
| `src/waveform.rs`      | `WaveformState`, `decode_audio`, `render_waveform` | `src/waveform.rs` |
| `src/waveform_player.rs` | Hele bestand (WaveformCommand/-Event, audio thread, WaveformSource, decode_segment, apply_rubato) | `src/waveform_player.rs` |
| `src/loops.rs`         | Hele bestand (SavedLoop, load/save/add/remove/generate_label) | `src/loops.rs` |
| `Cargo.toml`           | Dependencies: eframe, rodio, symphonia, rubato, crossbeam-channel, serde, serde_json | `Cargo.toml` (alleen de benodigde deps) |

---

## Plan

### Fase 1 — Projectstructuur opzetten

1. Maak map `loop-editor/` aan.
2. Maak `loop-editor/Cargo.toml` met de minimale dependencies:
   - `eframe = "0.28"`
   - `rodio = { version = "0.19", features = ["symphonia-all"] }`
   - `symphonia = "0.5"`
   - `rubato = "0.15"`
   - `crossbeam-channel = "0.5"`
   - `serde = { version = "1.0", features = ["derive"] }`
   - `serde_json = "1.0"`
3. Kopieer `src/waveform.rs`, `src/waveform_player.rs` en `src/loops.rs` naar `loop-editor/src/`.
4. Pas in `waveform_player.rs` de `use` paths aan van `crate::` naar `use crate::`.
5. Maak `loop-editor/src/main.rs` en `loop-editor/src/app.rs`.

### Fase 2 — App-state (`app.rs`)

Maak een `LoopEditorApp` struct met alleen wat nodig is:

- `waveform_state: WaveformState`
- `waveform_cmd_tx / waveform_event_rx` (voor waveform audio thread)
- `waveform_is_playing / waveform_play_position / waveform_play_duration`
- `saved_loops: Vec<SavedLoop>`
- `show_loop_library: bool`
- Optioneel: `status_message: String`
- **Geen** library, player, queue, filters, navigation, batch edit, etc.

Implementeer:

- `new()` — start waveform audio thread, laad opgeslagen loops
- `update()` — verwerk waveform events, teken UI (zie Fase 3)

### Fase 3 — UI (`main.rs` / `app.rs`)

De UI is één egui-app zonder zijpanelen, zonder navigatie:

1. **Bestand openen**
   - Knop "📂 Open bestand" → opent een native file picker via `rfd` (of werk met een
     eenvoudig tekstveld + pad als `rfd` te zwaar is).
   - Drag & drop support: `ui.ctx().input(|i| i.raw.dropped_files())` → decode audio.
   - Bij laden: roep `decode_audio()` aan, vul `waveform_state`.

2. **Waveform view**
   - Centraal paneel met `render_waveform()`.
   - `now_playing_position` wordt de `waveform_play_position` (eigen audio-thread).
   - `seek_to` uit `render_waveform` wordt genegeerd of omgezet naar een herstart van
     de waveform player vanaf die positie (optioneel).

3. **Pitch / Tempo controls**
   - Twee `egui::Slider`s (pitch -12..+12 semitones, tempo 0.25x..2.0x).
   - Bij wijziging: `WaveformCommand::SetPitch` / `SetTempo` sturen.
   - Rechts ervan: reset-knoppen (⟲).

4. **Playback controls**
   - Als A en B gezet zijn: "▶ Play Loop (rubato)" knop → `WaveformCommand::Play`.
   - Tijdens afspelen: "⏹ Stop" knop → `WaveformCommand::Stop`.
   - Status: "▶ mm:ss / mm:ss" tijdens afspelen.

5. **Loop opslaan**
   - "💾 Save Loop" knop wanneer A en B gezet zijn.
   - "📚 Loops" knop toont de loop bibliotheek.

6. **Zoom controls**
   - "🔍−" / "🔍+" / "⟲ Reset zoom/scroll" knoppen.

7. **Loop library window**
   - Toon opgeslagen loops, laad of verwijder ze.
   - Bij laden: stel A, B, pitch, tempo in op `waveform_state`.

### Fase 4 — main.rs entry point

```rust
mod app;
mod loops;
mod waveform;
mod waveform_player;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };
    eframe::run_native(
        "Waveform Loop Editor",
        options,
        Box::new(|_cc| Ok(Box::new(app::LoopEditorApp::new()))),
    )
}
```

### Fase 5 — Playhead seek-bugfix meenemen

De bugfix uit `player.rs` (play vóór seek in de resume-logica) is niet direct van
toepassing — de waveform player heeft geen pause. De playhead in de standalone app
wordt aangestuurd door `waveform_play_position` (de eigen audio-thread). Bij een
drag van de playhead tijdens pauze moeten we een nieuw `WaveformCommand::Play` sturen
vanaf de nieuwe positie, of een `Seek`-commando toevoegen aan `WaveformCommand`.

**Optie A**: Voeg `WaveformCommand::SeekTo(f32)` toe aan de enum en implementeer
seek in `run_waveform_audio`. Dit is de schone oplossing.

**Optie B**: Bij playhead-drag: stuur `WaveformCommand::Play` met de huidige A-B
maar met `start_sec` = de versleepte positie (binnen de loop). Hiermee herstart
de audio vanaf die positie. Dit is eenvoudiger maar reset de audio-buffer.

**Beslissing**: Kies Optie B voor nu — simpel en robuust. De waveform player
decoded toch alleen het A-B segment (~seconden), dus herstarten is goedkoop.

---

## Tijdsinschatting

| Fase | Wat | Geschatte tijd |
|------|-----|----------------|
| 1 | Projectstructuur, Cargo.toml, kopiëren bestanden | 5 min |
| 2 | App-state schrijven | 10 min |
| 3 | UI bouwen | 20 min |
| 4 | main.rs + testen of het compileert | 5 min |
| 5 | Playhead seek + verfijnen | 10 min |
| **Totaal** | | **~50 min** |

---

## Randvoorwaarden

- De nieuwe app moet in `loop-editor/` staan, op hetzelfde niveau als de hoofd-app.
- De hoofd-app (`geminijukebox/`) blijft ongewijzigd.
- Gebruik dezelfde dependency-versies als de hoofd-app voor consistentie.
- De app moet compileren met `cargo build` in de `loop-editor/` map.
- De app moet een losstaand `.exe` produceren.
