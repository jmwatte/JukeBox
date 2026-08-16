# 🎵 JukeBoks — Handleiding

JukeBoks is een minimalistische, toetsenbord-gestuurde muziekspeler voor Windows.
De bibliotheek wordt hiërarchisch weergegeven: **Artiest → Album → Disk → Track**.

---

## 🚀 Snelstart

1. **Muziek klaarzetten:** pas `music_directory` aan in `config.toml`
   (naast de executable; standaard `H:\music`).
2. **Start de app:** dubbelklik `jukeboks.exe`.
3. **Navigeren:** pijltjestoetsen door Artiest → Album → Track.
4. **Afspelen:** `Enter` op een track, album of artiest.
5. **Help:** `H` toont alle sneltoetsen.

---

## ⌨️ Sneltoetsen

Alle toetsen zijn aanpasbaar in `config.toml` onder `[shortcuts]`.

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

## 🔍 Zoeken

Druk op `/` om te zoeken. Er wordt live gezocht op **titel, album, artiest en
bestandspad**. `Escape` sluit de zoekfunctie.

## 🎛️ Filters

Naast zoeken kun je filteren op metadata:

1. **Genre** (`G`)
2. **Jaartal** (`Y`)
3. **Componist** (`C`)

Filters zijn te combineren. De actieve filters zie je als breadcrumbs bovenaan;
`←` gaat een filterstap terug.

---

## 🌊 Waveform Editor

Druk op `0` om de waveform-editor te openen voor de huidige selectie of het
huidig afgespeelde nummer.

- **A-B loop markers:** dubbelklik om A te zetten, Shift+dubbelklik voor B
- **Sleep A-B markers:** versleep de groene (A) en rode (B) lijnen
- **Sleep de hele loop:** klik in het blauwe A-B gebied en sleep
- **Verplaats playhead:** sleep de gele driehoekjes boven/onder
- **Klik om te seeken:** klik ergens op de waveform
- **Rechterklik:** wis de A-B loop
- **Scroll:** zoom in/uit op de waveform
- **Pitch/Tempo:** schuifregelaars voor pitch (semitones) en tempo

### Loops opslaan

1. Zet A-B markers in de waveform.
2. Klik **💾 Save Loop** of `Ctrl+S`.
3. Bekijk opgeslagen loops via **📚 Loops**.

---

## 🏷️ Tag Bewerken

Selecteer een of meerdere tracks en druk op `I`.

- **Single track:** bewerk titel, artiest, album, genre(s), jaar en componist.
- **Batch:** markeer tracks met `M`, vink in het rechterpaneel de velden aan die
  je wilt bijwerken en klik **Opslaan in alle geselecteerde**.
- **Raw tags:** het gesplitste paneel toont links de bestanden en rechts de
  ruwe tag-dump per bestand.

---

## ⚙️ Configuratie

`config.toml` wordt automatisch aangemaakt bij de eerste start.

```toml
music_directory = 'H:\music'
window_size = [800, 600]
startup_view = "cover"        # "cover" of "tracklist"

[shortcuts]
PlayPause = "Space"
Skip = "N"
# ... alle sneltoetsen zijn aanpasbaar
```

Bij het opstarten wordt de config gevalideerd: onbekende acties, ontbrekende
acties, dubbele en ongeldige toetsen worden gemeld. Via het helpscherm (`H`)
kun je foutieve shortcuts automatisch laten repareren.

---

## ❓ Veelgestelde problemen

**Mijn muziek is "weg" na het wisselen van een USB-schijfletter (H: → L:).**
Geen paniek: JukeBoks detecteert bij het opstarten dat de geconfigureerde map
niet meer bestaat, zoekt dezelfde map op een andere schijf en koppelt de
bibliotheek én je opgeslagen loops automatisch opnieuw — zonder herscan.

**Geen geluid of audio valt weg.**
Druk op `F6` (of gebruik de bijbehorende shortcut) om de audio-verbinding te
herstellen. Dit gebeurt ook automatisch bij het volgende nummer.

**Spoelen werkt niet voor een bestand.**
Sommige formaten/streams ondersteunen geen seek. Bij de meeste FLAC/MP3/Ogg/M4A
bestanden werkt spoelen (2 seconden terug/vooruit met `;` en `'`).

**Waar staat mijn bibliotheekcache?**
Naast de executable in `library_cache.bin` (automatisch gegenereerd; verwijderen
dwingt een volledige herscan af).

---

## 📜 Licentie

MIT
