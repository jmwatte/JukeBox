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

## Fase 5 – Optionele extra's (lagere prioriteit)

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
```
