# JukeBox – Ontwikkelplan

Hieronder staat een stappenplan met verbeteringen en nieuwe features, geordend van klein/basaal naar groter/afhankelijk. Elke stap bouwt voort op de vorige.

---

## Fase 1 – Metadatascanner repareren (fundament)

### Stap 1.1 – Track-tags correct uitlezen

**Probleem:** De scanner (`scanner.rs`) leest alleen genre, jaar en componist uit de audiotags. Titel, tracknummer en duur worden overgeslagen — `title` is de bestandsnaam, `track_number` en `duration_secs` zijn altijd 0.

**Oplossing:** In `scanner.rs`, bij het aanmaken van een `Track`, ook de volgende velden uit de tags lezen via Lofty:

| Track-veld | Lofty API / ItemKey |
|---|---|
| `title` | `tag.title()` via `Accessor` (of `ItemKey::Title`) |
| `track_number` | `tag.track()` via `Accessor` (of `ItemKey::TrackNumber`) |
| `artist` | `tag.artist()` via `Accessor` (of `ItemKey::Artist`) |
| `disc_number` | `ItemKey::DiscNumber` (niet in `Track` model — toevoegen) |
| `duration_secs` | via Symphonia decoderen **of** utzettend uit Lofty's `properties().duration()` |

**Acties:**
- `Track`-struct uitbreiden met `disc_number: Option<u32>` (of `u32`)
- In `scanner.rs` de Lofty-tag-loop uitbreiden
- `duration_secs` vullen via `tagged_file.properties().duration()` van Lofty (geeft `std::time::Duration`)
- Valback: als `tag.title()` leeg is, de bestandsnaam gebruiken (huidig gedrag)

### Stap 1.2 – `Artist`-tag ook opslaan in `Track`

**Probleem:** De artiestennaam wordt nu alleen afgeleid uit de mapstructuur, niet uit de tag. Als een track in een verkeerde map staat of als het een compilatie betreft, klopt de artiestennaam niet.

**Oplossing:** `Track`-model uitbreiden met `artist: Option<String>` en deze vullen uit `tag.artist()`. In de UI de track-artiest tonen bij track-level (bv. bij compilaties).

### Stap 1.3 – Lofty-properties duration uitlezen

**Probleem:** `duration_secs` wordt niet gevuld.

**Oplossing:**
```rust
if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
    let duration = tagged_file.properties().duration();
    // duration is std::time::Duration
}
```
Deze waarde opslaan in `track.duration_secs` (als `u32` seconden, `.as_secs()`).

### Stap 1.4 – Cache migratie / busten

**Probleem:** Nadat het `Track`-model is uitgebreid met nieuwe velden, is de oude cache (`library_cache.bin`) incompatibel en crasht de app bij deserialization.

**Oplossing:**
- Een `CACHE_VERSION`-constante toevoegen (bv. `1`)
- De cache opslaan met een header die de versie bevat
- Bij versie-mismatch de cache wissen en opnieuw scannen

---

## Fase 2 – UI opwaarderen

### Stap 2.1 – Tracknummer en duur tonen in tracklist ✅

**Afhankelijk van:** Stap 1.1, 1.3

In `render.rs` (`render_tracklist_view_inline`, `NavLevel::Track`):
- Tracknummer tonen vóór de titel (bv. `"01. Song Title"`)
- Duur tonen rechts uitgelijnd (bv. `"3:42"`)
- Indien beschikbaar: discnummer bij disks tonen

### Stap 2.2 – Huidige positie en voortgangsbalk in now-playing balk

**Afhankelijk van:** Stap 1.3 (duration)

In `player.rs`:
- `PlayerEvent::NowPlaying` uitbreiden met duration (of apart `PositionUpdate`-event)
- In `render.rs` (`update`): periodiek `sink.get_pos()` opvragen → naar UI sturen
- In de now-playing balk: voortgangsbalk (`egui::ProgressBar`) + tijdweergave (`"1:23 / 3:42"`)

**Optioneel:** klikken op de balk seekt naar die positie.

### Stap 2.3 – Volume control ✅

In `player.rs`:
- `PlayerCommand::SetVolume(f32)` toevoegen
- In `navigation.rs` `=` en `-` als shortcuts
- Volume-indicator in now-playing balk 🔊 100%

### Stap 2.4 – Forward seek ✅

**Afhankelijk van:** Stap 2.2 (positie-opvragen werkt al)

Symmetrisch aan de `;`-rewind, maar dan vooruit. Toets: `'` (apostrof).

### Stap 2.5 – "Now Playing" navigatie ✅

Sneltoets (`F2`) die de bibliotheekselectie verplaatst naar het huidig spelende nummer.

In `app.rs`:
- `now_playing_path: Option<String>` bijgehouden (volledig pad, niet alleen bestandsnaam)
- `navigate_to_now_playing()` zoekt het pad in de library en stelt selectie in

---

## Fase 3 – Afspeelmodi en queue

### Stap 3.1 – Repeat-modes (None / One / All)

**Afhankelijk van:** Stap 2.2

In `player.rs`:
- `enum RepeatMode { None, One, All }` bijhouden in audio-thread
- `PlayerCommand::ToggleRepeat` toevoegen
- Bij `One`: herhaal huidige track (herplaats in sink nadat hij afgelopen is)
- Bij `All`: als queue leeg is, herbegin van originele queue
- In de UI: indicator in now-playing balk (geen/🔂/🔁)

### Stap 3.2 – Shuffle-modus

**Afhankelijk van:** Stap 3.1 (same area)

In `player.rs`:
- `PlayerCommand::ToggleShuffle` toevoegen
- Bij shuffle: `internal_queue` door elkaar halen met `rand::shuffle`
- In de UI: indicator (🔀)

### Stap 3.3 – Queue weergeven / beheren

Een `Q`-shortcut die een zijpaneel toont met de huidige queue (tracks in `internal_queue` + huidige track). Van daaruit:
- Tracks verwijderen uit queue
- Queue wissen
- Queue opslaan als `.m3u`

### Stap 3.4 – A-B loop (Mark A / Mark B / Play loop / Delete loop)

**Afhankelijk van:** Stap 2.2 (positie-opvragen werkt al)

Een loop-functie waarbinnen een specifiek gedeelte van een track wordt herhaald:

**Functionaliteit:**
- **Mark A** (`[`): zet loop-startpunt op huidige positie
- **Mark B** (`]`): zet loop-eindpunt op huidige positie
- **Play loop** (opnieuw `[` of `]` als A en B al zijn gezet): activeer loop
- **Delete loop** (`\` of `Shift+[`): wis A- en B-punten, schakel loop uit

**In `player.rs`:**
- `loop_start: Option<Duration>` en `loop_end: Option<Duration>` bijhouden in audio-thread
- `PlayerCommand::SetLoopStart`, `SetLoopEnd`, `ClearLoop` toevoegen
- In de hoofdloop: als `loop_start` en `loop_end` ingesteld zijn, check of `sink.get_pos() >= loop_end` → seek naar `loop_start`

**In `render.rs` / `navigation.rs`:**
- Sneltoetsen toevoegen voor markeren en wissen
- Visuele indicator in de now-playing balk (bv. `🔁 1:23 – 3:45`)
- Kort visueel signaal bij markeren (bv. statusbericht in now-playing balk)

---

## Fase 4 – Afwerking

### Stap 4.1 – Cache-invalidatie bij bestandswijzigingen

- Bij opstarten: `modified_at` van de muziekdirectory opslaan in cache
- Als de directory nieuwer is dan de cache: opnieuw scannen
- **Optioneel:** `notify`-crate gebruiken voor realtime file watching

### Stap 4.2 – Error handling in audio-thread

- `PlayerEvent::PlaybackError(String)` toevoegen
- Bij falen van `File::open` / `Decoder::new`: error naar UI sturen
- In de UI: statusbericht tonen (tijdelijk, of in de now-playing balk)

### Stap 4.3 – Pseudo-random vervangen

`rand`-crate toevoegen (of `fastrand` voor minimale overhead) en `RandomAlbum` via een echte RNG laten werken.

### Stap 4.4 – Slaaptimer

- `PlayerCommand::SleepTimer(Duration)` toevoegen
- Audio-thread telt af en pauzeert bij 0
- In de UI: timer tonen, `T`-shortcut (of `Ctrl+T`) om in te stellen

### Stap 4.5 – Compacte modus

Een `F11`-achtige toggle die de UI minimaliseert tot alleen de now-playing balk met basiscontrols. Handig voor gebruik als kleine speler naast andere vensters.

### Stap 4.6 – Config validatie (shortcuts) & opstart-hulp ✅

**Probleem:** De `shortcuts` in `config.toml` worden direct geladen in een `HashMap<String, String>`. Als een gebruiker handmatig fouten maakt in de config (dubbele entries, ongeldige toetswaarden, verkeerde actienamen), zijn de shortcuts onvoorspelbaar of werken ze niet.

**Oplossing:**

1. **Validatie bij opstarten** — controleer de geladen shortcuts:
   - Zijn alle actienamen geldig? (vergelijk met `default_shortcuts()` keys)
   - Zijn de toetswaarden bekend bij `key_pressed()`? (`Space`, `Enter`, `F1`–`F12`, letters, `;`, `'`, `=`, `-`, `[`, `]`, `\`, `/`, `?`)
   - Wordt een toets aan meerdere acties toegewezen? (dubbele entry)
   - Verzamel een lijst van fouten/warnings

2. **Helpscherm bij fouten** — als er config-fouten zijn gedetecteerd:
   - Toon automatisch het helpscherm bij opstarten (geforceerd, niet pas na H-toets)
   - Toon de foutmeldingen in een aparte rode sectie bovenaan het helpscherm
   - Bij elke fout: toon de actie, de ongeldige waarde, en wat de fout is
   - Toon een `[Herstel standaard shortcuts]`-knop die `config.toml` herschrijft met de standaardwaarden
   - Toon een `[Negeer en herinner later]`-knop

3. **Reset-functionaliteit** — de knop roept `default_shortcuts()` aan en schrijft deze weg naar `config.toml`, zonder andere config-velden aan te tasten.

**Acties:**
- Validatiefunctie in `config.rs` of `shortcuts.rs`: `fn validate_shortcuts(&HashMap<String, String>) -> Vec<String>`
- Uitbreiding `MusicPlayerApp` met `config_errors: Vec<String>` en `force_help: bool`
- In `render.rs`: als `force_help`, helpvenster tonen met errors en resetknop

---

## Fase W – Waveform Editor (transcribe-achtige functies)

**Doel:** Een waveform-editor waarmee de gebruiker een gemarkeerde track kan openen (shortcut `0`),
de waveform kan zien, visueel A-B loops kan instellen, pitch-shiften en tempo kan vertragen.

### Huidige codebase context (voor AI bij nieuwe chat)

Dit project is een egui/eframe desktop app in Rust. Belangrijke bestanden:

| Bestand | Functie |
|---|---|
| `src/main.rs` | Entrypoint, start audio-thread + UI-thread |
| `src/player.rs` | Audio-thread met rodio Sink, `PlayerCommand`/`PlayerEvent` kanalen |
| `src/models.rs` | Data modellen: `Track`, `Album`, `Artist`, `Library` |
| `src/scanner.rs` | Bibliotheek scanner + cache |
| `src/ui/` | Egui UI: `app.rs` (state), `render.rs` (tekenen), `navigation.rs` (shortcuts), `edit.rs` (batch tag editor), `shortcuts.rs` (toetsen) |

**Key architecture:**
- Audio draait in een aparte `std::thread` met `crossbeam_channel` voor communicatie
- `PlayerCommand` wordt van UI naar audio-thread gestuurd
- `PlayerEvent` wordt van audio-thread naar UI gestuurd
- UI thread draait egui via `eframe::run_native()`
- `MusicPlayerApp` in `app.rs` is de centrale state struct

**Bestaande A-B loop (in player.rs):**
```rust
// PlayerCommand::SetLoopA / SetLoopB / ClearLoop
// loop_a: Option<Duration>, loop_b: Option<Duration>
// Check: if s.get_pos() >= loop_b { s.try_seek(loop_a); }
// Event: PlayerEvent::LoopChanged(Option<f32>, Option<f32>)
```

---

### Stap W1 – PCM-decoding + waveform render

**Nieuwe file:** `src/waveform.rs`

**Doel:** Open een audiobestand, decodeer naar PCM samples, teken de waveform in een egui window.

**Wat moet er gebeuren:**

1. **Directe symphonia decoding (niet via rodio):**
   - Gebruik `symphonia::default::get_codecs()` en `symphonia::core::io::MediaSourceStream`
   - Decodeer het hele bestand naar `Vec<f32>` (mono, gemiddelde van kanalen)
   - Of decodeer in chunks voor grote bestanden (streaming)

2. **Downsamplen voor weergave:**
   - Bepaal hoeveel samples per pixel (afhankelijk van zoom-niveau en vensterbreedte)
   - Voor elke pixel: bepaal de min en max sample in dat blok
   - Teken verticale lijnen van min naar max per pixel

3. **Egui waveform render:**
   - Open een egui `Window` via shortcut `0`
   - `ui.painter().add(egui::Shape::line(...))` voor de waveform
   - Tijdschaal onderaan (elke seconde / 5 seconden een markering)
   - Muiswiel voor zoomen, slepen voor scrollen

**Key API's:**
```rust
// Symphonia direct decoding (voorbeeld, aanpassen aan symphonia 0.5)
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

let src = std::fs::File::open(path)?;
let mss = MediaSourceStream::new(Box::new(src), Default::default());
let mut hint = Hint::new();
hint.with_extension("mp3");
let probed = symphonia::default::get_probe().format(&hint, mss, &Default::default(), &Default::default())?;

// Decodeer naar Vec<f32> (mono)
let track_id = probed.format.default_track().codec_params.track_id;
let mut decoder = symphonia::default::get_codecs().make(&probed.format.default_track().codec_params, &Default::default())?;
let mut samples: Vec<f32> = Vec::new();
loop {
    let packet = probed.format.next_packet()?;
    let decoded = decoder.decode(&packet)?;
    // converteer naar f32, mix naar mono
}

// Egui waveform
let painter = ui.painter();
let rect = ui.allocate_space(egui::vec2(ui.available_width(), 200.0));
for x in 0..rect.width() as usize {
    let min = get_min_sample(x, zoom, &samples);
    let max = get_max_sample(x, zoom, &samples);
    let p1 = egui::pos2(rect.left() + x as f32, rect.center().y + min * rect.height() / 2.0);
    let p2 = egui::pos2(rect.left() + x as f32, rect.center().y + max * rect.height() / 2.0);
    painter.line_segment([p1, p2], (1.0, egui::Color32::from_gray(180)));
}
```

---

### Stap W2 – Visuele A-B loop op waveform

**Doel:** Sleepbare A- en B-markers op de waveform, met oplichtend loop-gebied.

**Wat moet er gebeuren:**

1. **Twee verticale lijnen** op de waveform:
   - A (groen) en B (rood)
   - Sleepbaar via `ui.interact()` met `Sense::drag()`
   - Positie wordt vertaald naar sample-index / tijd

2. **Geselecteerd gebied oplichten:**
   - Tussen A en B: lichtblauwe of gele achtergrond
   - `painter.rect_filled()` met transparante kleur

3. **Huidige positie-indicator:**
   - Dunne rode lijn die meebeweegt tijdens afspelen
   - Update via `now_playing_position` uit `MusicPlayerApp`

4. **Koppeling met bestaande A-B loop in player.rs:**
   - Bij slepen van A of B: stuur `PlayerCommand::SetLoopA` / `SetLoopB`
   - Bij wijzigen via bestaande shortcuts `[` / `]` / `\`: waveform update via `LoopChanged` event
   - Sync bidirectional: wat de waveform doet, moet de player doen en vice versa

**UI state in app.rs / nieuwe waveform struct:**
```rust
pub struct WaveformState {
    pub path: Option<String>,
    pub samples: Vec<f32>,         // PCM samples (mono, gemixt)
    pub sample_rate: u32,
    pub duration_secs: f32,
    pub zoom: f32,                  // pixels per second
    pub scroll_offset: f32,         // scroll offset in seconds
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
}
```

---

### Stap W3 – Loops afspelen via aparte audio-thread

**Doel:** Het A-B segment afspelen via een aparte audio-thread (of via de bestaande player).

**Optie A (simpler): via bestaande player**
- Stel de loop A en B in via `PlayerCommand::SetLoopA` / `SetLoopB`
- De bestaande loop-logic in player.rs speelt het segment af
- Nadeel: geen fine control over pitch/tempo

**Optie B (krachtiger): aparte waveform audio-thread**
- Nieuwe thread die symphonia gebruikt om alleen het A-B segment te decoderen
- rodio Sink voor output
- `WaveformCommand::PlayLoop(start_secs, end_secs)`
- `WaveformCommand::Stop`, `WaveformCommand::SetPitch(f32)`, `WaveformCommand::SetTempo(f32)`
- Telt als voorbereiding op W4

**Aanbevolen:** Start met optie A (simpel), breid later uit naar optie B voor W4.

---

### Stap W4 – Pitch shifting & time stretching (rubato)

**Nieuwe dependency:** `rubato = "0.15"`

**Doel:** Vertragen/versnellen zonder toonhoogte-verandering, en pitch-shiften.

**Wat moet er gebeuren:**

1. **Rubato configureren:**
```rust
use rubato::{PitchShifter, Resampler, SincFixedIn, InterpolationType, WindowFunction};

let pitch_shift = 0.0; // semitones
let tempo = 1.0;        // 0.5 = half tempo, 2.0 = dubbel tempo

let mut resampler = SincFixedIn::<f32>::new(
    tempo,          // scale factor
    1.0,            // sample rate ratio
    SincFixedIn::generate_coefficients(256, 10, InterpolationType::Linear, WindowFunction::BlackmanHarris2);
    samples.len(),  // chunk size
    1,              // channels
)?;

let processed = resampler.process(&[&samples], None)?;
```

2. **Pitch shift zonder tempo-verandering:**
   - Gebruik `rubato::PitchShifter`
   - Pitch shift in semitones: `f32::powf(2.0, semitones / 12.0)`

3. **UI controls:**
   - Schuifregelaar `-12 .. 0 .. +12` semitones
   - Schuifregelaar `25% .. 100% .. 200%` tempo
   - Reset knoppen

4. **Audio pipeline:**
   - Decodeer A-B segment samples
   - Verwerk door rubato
   - Stuur naar rodio Sink
   - Bij wijzigen pitch/tempo: stop huidige sink, verwerk opnieuw, speel af

---

### Stap W5 – Loops opslaan / beheren

**Doel:** Opgeslagen loops bewaren, terugvinden, en exporteren.

**Bestandsformaat:** JSON bestand `loops.json` in de app directory
```rust
#[derive(Serialize, Deserialize)]
pub struct SavedLoop {
    pub track_path: String,
    pub label: String,
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
    pub pitch_semitones: f32,
    pub tempo: f32,
}
```

**Functionaliteit:**
- Loop opslaan met een naam (standaard: tracknaam + " - Loop 1")
- Loop bibliotheek: window met alle opgeslagen loops
- Click op een loop: open waveform met die instellingen
- Delete loop
- Export A-B segment als `.wav` met hound crate (of symphonia)

---

### Sneltoetsen (voor shortcuts.rs)

| Actie | Toets |
|---|---|
| `WaveformOpen` | `0` |
| `WaveformPlayLoop` | `Space` (als waveform focus heeft) |
| `WaveformSetA` | `[` (als waveform open is) |
| `WaveformSetB` | `]` (als waveform open is) |
| `WaveformClearLoop` | `\` (als waveform open is) |
| `WaveformSaveLoop` | `Ctrl+S` |

### Nieuwe files

| Bestand | Inhoud |
|---|---|
| `src/waveform.rs` | WaveformState, decode, render, interactie |
| `src/loops.rs` | SavedLoop struct, save/load JSON, loop bibliotheek |

### Nieuwe dependencies in Cargo.toml

```toml
rubato = "0.15"     # pitch shift + time stretch
# Optioneel voor W5:
# hound = "3.5"     # WAV export
```

---

### Uitvoeringsvolgorde

```
W1: PCM decoderen + waveform tekenen op scherm
  └─ test met een kort mp3/flac bestand

W2: A-B markers sleepbaar op waveform
  └─ koppelen met bestaande player.rs loop logic

W3: Loop afspelen via player
  └─ W1 + W2 vormen een werkende visuele loop-editor

W4: rubato pitch/tempo
  └─ aparte audio thread voor realtime verwerking

W5: Opslaan / beheren / exporteren
  └─ loops.json + loop bibliotheek venster
```

| Feature | Korte beschrijving |
|---|---|
| 🏷️ Multi-value tag toevoegen | In batch editor: genre toevoegen i.p.v. overschrijven |
| 📋 .m3u export/import | Queue / selectie exporteren naar `.m3u`-bestand |
| 🎯 Compilatie-detectie | Albums waar tracks verschillende artiesten hebben |
| 🎚️ Crossfade | Oude sink uitfaden, nieuwe inladen |
| 📱 Mini-modus | Alleen systray + notificatie (maar eframe/egui ondersteunt geen systray out-of-the-box) |
| 🎨 Thema-selectie | Licht/donker thema (egui ondersteunt dit) |

---

## Volgorde (kort)

```
Fase 1: Metadatascanner
  ├─ 1.1 Tags uitlezen (titel, tracknr, discnr, duration)
  ├─ 1.2 Artist-tag in Track
  ├─ 1.3 Duration via Lofty properties
  └─ 1.4 Cache migratie

Fase 2: UI
  ├─ 2.1 Trackinfo in lijst
  ├─ 2.2 Voortgangsbalk
  ├─ 2.3 Volume
  ├─ 2.4 Forward seek
  └─ 2.5 Now Playing navigatie

Fase 3: Afspeelmodi
  ├─ 3.1 Repeat
  ├─ 3.2 Shuffle
  ├─ 3.3 Queue beheer
  └─ 3.4 A-B loop (Mark A / Mark B)

Fase 4: Afwerking
  ├─ 4.1 Cache-invalidatie
  ├─ 4.2 Error handling
  ├─ 4.3 Echte random
  ├─ 4.4 Slaaptimer
  ├─ 4.5 Compacte modus
  └─ 4.6 Config validatie & opstart-hulp

Fase W: Waveform Editor
  ├─ W1 PCM-decoding + waveform render
  ├─ W2 Visuele A-B loop
  ├─ W3 Loops afspelen (aparte audio)
  ├─ W4 Pitch / Tempo (rubato)
  └─ W5 Loops opslaan / beheren
```
