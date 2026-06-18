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

### Stap 2.1 – Tracknummer en duur tonen in tracklist

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

### Stap 2.3 – Volume control

In `player.rs`:
- `PlayerCommand::SetVolume(f32)` toevoegen
- In `navigation.rs` `+` en `-` (of `[` / `]`) als shortcuts
- Volume-indicator in now-playing balk (kleine horizontale balk of percentage)

### Stap 2.4 – Forward seek

**Afhankelijk van:** Stap 2.2 (positie-opvragen werkt al)

Symmetrisch aan de `;`-rewind, maar dan vooruit. Suggestie: `'` (apostrof).

### Stap 2.5 – "Now Playing" navigatie

Sneltoets (bv. `Ctrl+N` of `F2`) die de bibliotheekselectie verplaatst naar het huidig spelende nummer.

In `navigation.rs`:
- Zoek het huidige `now_playing`-pad op in de library
- Stel `selected_artist`, `selected_album`, `selected_disk`, `selected_track` in
- Zet `current_level` op `NavLevel::Track`
- Scroll naar de selectie

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
  └─ 3.3 Queue beheer

Fase 4: Afwerking
  ├─ 4.1 Cache-invalidatie
  ├─ 4.2 Error handling
  ├─ 4.3 Echte random
  └─ 4.4 Slaaptimer
```
