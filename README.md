# 🎵 JukeBoks — Toetsenbord-gestuurde Muziekspeler

Een minimalistische, snelle muziekspeler voor Windows, gebouwd in Rust met egui.
Ontworpen voor toetsenbordbediening, met een hiërarchische bibliotheekweergave
(Artiest → Album → Disk → Track), geavanceerde filteropties en een ingebouwde
waveform-editor met A-B looping.

![Taal: Rust](https://img.shields.io/badge/taal-Rust-orange)
![GUI: egui/eframe](https://img.shields.io/badge/GUI-egui-9cf)
![Licentie: MIT](https://img.shields.io/badge/licentie-MIT-green)

---

## 📦 Installatie

### Vanuit broncode

1. Installeer [Rust](https://rustup.rs/) (stable toolchain)
2. Clone of download dit project
3. Compileer en start:

```sh
cd geminijukebox
cargo run --release
```

> **Let op:** `--release` is aanbevolen. Het release-profiel heeft LTO en
> optimalisaties ingeschakeld voor kleinere en snellere uitvoer.

### Systeemvereisten

- **Besturingssysteem:** Windows (WASAPI audio via rodio)
- **Geheugen:** ~50-200 MB, afhankelijk van bibliotheekgrootte
- **Opslag:** ~30 MB (gecompileerd)

---

## 🚀 Snelstart

1. **Zet je muziek klaar:** Pas `music_directory` aan in `config.toml`
   (standaard: `H:\music`).
2. **Start de app:** `cargo run --release`
3. **Navigeer:** Pijltjestoetsen door Artiest → Album → Track
4. **Speel af:** `Enter` op een track, album of artiest
5. **Help:** `H` voor alle sneltoetsen

---

## ⌨️ Sneltoetsen

Alle toetsen zijn configureerbaar in `config.toml` onder `[shortcuts]`.

### Navigatie

| Toets | Actie |
|---|---|
| `↑ ↓` | Navigeer omhoog/omlaag |
| `←` | Terug naar vorig niveau |
| `→` of `Enter` | Inzoomen op selectie / afspelen |
| `Escape` | Zoekfunctie sluiten / wissen / terug |

### Afspelen

| Toets | Actie |
|---|---|
| `Spatie` | Pauzeren / Hervatten |
| `N` | Skip naar volgend nummer |
| `;` | 2 seconden terugspoelen |
| `'` | 2 seconden vooruitspoelen |
| `X` | Herhaalmodus (Uit → 1 → Alle) |
| `F8` | Shuffle aan/uit |
| `+` / `-` | Volume omhoog/omlaag |

### A-B Loop

| Toets | Actie |
|---|---|
| `[` | Zet loop-punt A (huidige positie) |
| `]` | Zet loop-punt B (huidige positie) |
| `\` | Wis A-B loop |

### Bibliotheek

| Toets | Actie |
|---|---|
| `G` | Bladeren op genre |
| `Y` | Bladeren op jaartal |
| `C` | Bladeren op componist |
| `S` | Sorteer op datum (aflopend) |
| `B` | Nieuwste albums (Recent) |
| `R` | Willekeurig album |
| `/` | Zoeken in bibliotheek |

### Weergave

| Toets | Actie |
|---|---|
| `T` | Wissel lijstweergave / albumhoezen |
| `F2` | Navigeer naar huidig nummer |
| `Q` | Toon/verberg wachtrij |
| `F11` | Compacte modus (alleen speler) |
| `H` | Toon/verberg helpscherm |
| `0` | Open waveform-editor |

### Tag Bewerken

| Toets | Actie |
|---|---|
| `I` | Track details & tags bewerken |
| `M` | Markeer track voor batch-edit |
| `Shift+M` | Wis alle markeringen |
| `Z` | Browse selectie |
| `O` | Open map van huidige track |

### Systeem

| Toets | Actie |
|---|---|
| `F5` | Bibliotheek herscannen |
| `Shift+R` | Her scan alleen gemarkeerde tracks |
| `F6` | Audio-verbinding herstellen |

---

## 📂 Mappenstructuur

De app verwacht een muziekbibliotheek in de volgende structuur:

```
H:\music\
├── ArtiestNaam\
│   ├── AlbumNaam\
│   │   ├── cover.jpg            ← Albumhoes (cover/folder/album/front/art)
│   │   ├── 01 Nummer.flac
│   │   ├── 02 Nummer.mp3
│   │   └── ...
│   └── NogEenAlbum\
│       ├── cd1\                 ← CD/Disc submappen worden herkend
│       │   ├── 01 Track.flac
│       │   └── ...
│       └── cd2\
│           └── ...
└── NogEenArtiest\
    └── ...
```

**Ondersteunde formaten:** `mp3`, `flac`, `opus`, `ogg`, `m4a`, `mp4`, `wav`, `aac`, `alac`

---

## ⚙️ Configuratie

Het bestand `config.toml` wordt automatisch aangemaakt bij de eerste start.

```toml
music_directory = 'H:\music'
window_size = [800, 600]
startup_view = "cover"        # "cover" of "tracklist"

[shortcuts]
PlayPause = "Space"
Skip = "N"
# ... alle sneltoetsen zijn aanpasbaar
```

### Configuratie valideren

Bij het opstarten wordt `config.toml` gevalideerd:
- Onbekende acties worden gemeld.
- Ontbrekende acties worden gemeld.
- Dubbele toetsen worden gedetecteerd.
- Ongeldige toetsen worden gemeld.

Via het helpscherm (`H`) kun je foutieve shortcuts automatisch laten repareren.

---

## 🔍 Zoeken

Druk op `/` om de zoekfunctie te openen. Er wordt live gezocht op:

- **Track titel**
- **Album naam**
- **Artiest naam**
- **Bestandspad**

Zoekresultaten worden getoond in de normale hiërarchische weergave.
Druk op `Escape` om de zoekfunctie te sluiten.

---

## 🎛️ Filters

Naast zoeken kun je de bibliotheek filteren op metadata:

1. **Genre** (`G`) — blader en selecteer een genre
2. **Jaartal** (`Y`) — blader en selecteer een jaar
3. **Componist** (`C`) — blader en selecteer een componist

Filters kunnen worden gecombineerd. De huidige actieve filters worden getoond
als breadcrumbs bovenaan het scherm. Druk op `←` om een filterstap terug te gaan.

---

## 🏷️ Tag Bewerken

Selecteer een of meerdere tracks en druk op `I` om de tag-editor te openen.

### Single track
Bewerk titel, artiest, album, genre(s), jaar en componist.

### Batch bewerken
1. Markeer tracks met `M`
2. Het batch-panel verschijnt rechts
3. Vink de velden aan die je wilt bijwerken
4. Klik "Opslaan in alle geselecteerde"

### Raw tags inspectie
In de tag-editor zie je een gesplitst paneel:
- **Links:** bestanden in de selectie
- **Rechts:** ruwe tag-dump per bestand

---

## 🌊 Waveform Editor

Druk op `0` om de waveform-editor te openen voor de huidige selectie of
het huidig afgespeelde nummer.

### Functionaliteit

- **A-B loop markers:** dubbelklik om A te zetten, Shift+dubbelklik voor B
- **Sleep A-B markers:** versleep de groene (A) en rode (B) lijnen
- **Sleep de hele loop:** klik in het blauwe A-B gebied en sleep
- **Verplaats playhead:** sleep de gele driehoekjes boven/onder
- **Klik om te seeken:** klik ergens op de waveform
- **Rechterklik:** wis de A-B loop
- **Scroll:** zoom in/uit op de waveform
- **Pitch/Tempo:** schuifregelaars voor pitch (semitones) en tempo (0.25× – 2.0×)

### Loops opslaan

1. Zet A-B markers in de waveform
2. Klik "💾 Save Loop" of `Ctrl+S`
3. Bekijk opgeslagen loops via "📚 Loops"

---

## 📜 Licentie

MIT

---

## 🛠️ Technische details

| Component | Technologie |
|---|---|
| GUI | egui / eframe 0.36 |
| Audio output | rodio 0.22 (WASAPI) |
| Audio decodering | symphonia 0.5 |
| Metadata (tags) | lofty 0.25 |
| Parallel scanning | rayon, walkdir |
| Serialisatie | bincode 2.0, serde, toml |
| Concurrency | crossbeam-channel |

### Projectstructuur

```
src/
├── main.rs              # Entree + thread-opstart
├── config.rs            # Config laden/opslaan
├── models.rs            # Data modellen (Track, Album, Artist, Library)
├── scanner.rs           # Bibliotheek scannen + cache
├── search.rs            # Zoeken en filteren
├── player.rs            # Audio-thread (afspelen, queue, repeat, shuffle)
├── loops.rs             # Opgeslagen loops (opslaan/laden)
├── waveform.rs          # Waveform decoderen + renderen
└── ui/
    ├── mod.rs           # Module export
    ├── app.rs           # MusicPlayerApp struct (velden + logica)
    ├── render.rs        # egui rendering (update, panels, windows)
    ├── navigation.rs    # Toetsenbordnavigatie + shortcuts
    ├── shortcuts.rs     # Shortcut systeem (validatie, repair)
    ├── types.rs         # Types (NavLevel, ViewMode, FilterNode)
    ├── edit.rs          # Tag-editor (single + batch)
    ├── filters.rs       # Filter-hulpfuncties
    └── playback.rs      # Playback-state types

config.toml              # Gebruikersconfiguratie
library_cache.bin        # Bibliotheekcache (automatisch gegenereerd)
loops.json               # Opgeslagen loops (automatisch gegenereerd)
```
