# JukeBox — Roadmap: Metadata & Search

## 1. Metadata uitlezen (track.title, track_number, duration)

### Huidige situatie
In `scanner.rs` (regel 164-173) wordt de track aangemaakt met alleen de **filename** als titel:

```rust
title: path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
track_number: 0,
duration_secs: 0,
```

`lofty` wordt al gebruikt voor `genre`, maar **title, artist, album, track_number, duration, year en composer** worden genegeerd.

### Plan

In dezelfde `if let Ok(tagged_file) = Probe::open(path)...` block waar nu alleen `genre` wordt uitgelezen, voegen we toe:

```rust
// --- Titel ---
if let Some(t) = tagged_file.primary_tag().and_then(|t| t.title()) {
    track.title = t.to_string();
}

// --- Track nummer ---
if let Some(n) = tagged_file.primary_tag().and_then(|t| t.track()) {
    track.track_number = n;
}

// --- Duurtijd via symphonia ---
// Gebruik symphonia om de duurtijd te bepalen (lofty heeft geen duration)
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

if let Ok(src) = std::fs::File::open(path) {
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(&ext);
    if let Ok(mut probed) = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
        if let Some(track) = probed.format.tracks().first() {
            if let Some(codec_params) = track.codec_params {
                track.duration_secs = codec_params.time_base
                    .and_then(|tb| codec_params.n_frames.map(|n| tb.calc_time(n).seconds))
                    .unwrap_or(0) as u32;
            }
        }
    }
}

// --- Jaartal (year) ---
// Lofty's Accessor trait heeft geen `.year()`, dus we zoeken handmatig in de tags
for tag in tagged_file.tags() {
    for item in tag.items() {
        match item.key() {
            lofty::tag::ItemKey::Year => {
                if let lofty::tag::ItemValue::Text(text) = item.value() {
                    track.year = text.parse::<u32>().ok();
                }
            }
            lofty::tag::ItemKey::OriginalYear => {
                if track.year.is_none() {
                    if let lofty::tag::ItemValue::Text(text) = item.value() {
                        track.year = text.parse::<u32>().ok();
                    }
                }
            }
            _ => {}
        }
    }
}

// --- Componist (composer) ---
for tag in tagged_file.tags() {
    for item in tag.items() {
        match item.key() {
            lofty::tag::ItemKey::Composer => {
                if let lofty::tag::ItemValue::Text(text) = item.value() {
                    track.composer = Some(text.to_string());
                    break;
                }
            }
            _ => {}
        }
    }
    if track.composer.is_some() { break; }
}
```

**Waarom symphonia voor duration?** Lofty leest alleen tags (metadata), niet de audio-stream. Symphonia kan de audio-formaat-info uitlezen zonder de hele track te decoderen.

### Impact op search
Zodra `track.title` de echte titel bevat, matcht de search (`track_lower.contains(&query_lower)` in `search.rs` regel 35) op de **juiste titel** in plaats van op filename. Dat is een enorme verbetering.

### Performance
Als duration via symphonia te traag is (elke track openen tijdens scan), kunnen we duration overslaan en later als optionele post-processing doen, of alleen de symphonia-format header lezen zonder de hele stream te parsen.

---

## 2. Search optimalisatie (inverted index)

### Huidige situatie
`filter_library()` in `search.rs` loopt **elke toetsaanslag** door **alle tracks** in de library. O(n) met een dure `clone()` van de hele `Library` struct.

### Plan

**Fase 1 — Search op de achtergrond (direct te doen)**
- Huidige `filter_library()` blijft zoals het is, maar wordt in een `rayon::spawn` uitgevoerd
- Resultaat wordt via een `crossbeam_channel` teruggestuurd naar de UI thread
- UI loopt niet vast tijdens het filteren

**Fase 2 — Inverted index (meer werk)**
Tijdens het scannen bouwen we een `HashMap<String, Vec<usize>>` (woord → track-indices):

```rust
// In scanner.rs, na het vullen van de library:
pub struct SearchIndex {
    /// Map van lowercase woord naar track paden
    word_to_tracks: HashMap<String, Vec<String>>,
}
```

Voor elke track:
1. Splits `title`, `artist`, `album_title` en `genre` in woorden
2. Stop ze lowercase in de index

Search wordt dan:

```rust
pub fn search_index(index: &SearchIndex, query: &str) -> Vec<String> {
    let words: Vec<&str> = query.to_lowercase().split_whitespace().collect();
    // Alle tracks die ALLE woorden bevatten (intersectie)
    let mut result: Option<HashSet<&str>> = None;
    for word in &words {
        if let Some(tracks) = index.word_to_tracks.get(*word) {
            let set: HashSet<&str> = tracks.iter().map(|s| s.as_str()).collect();
            result = Some(match result {
                Some(prev) => prev.intersection(&set).cloned().collect(),
                None => set,
            });
        } else {
            return vec![]; // een woord komt nergens voor
        }
    }
    result.unwrap_or_default().into_iter().map(|s| s.to_string()).collect()
}
```

Daarna `filtered_library` opbouwen door alleen de gematche track-paden te includen.

### Voordelen
- O(1) per woord lookup ipv O(n) per toetsaanslag
- Meerdere woorden = intersectie van sets (bv. "jazz piano" geeft alleen tracks met beide woorden)
- Geen `clone()` van de hele library bij elke toets

---

## 3. Afhankelijkheden & volgorde

| Stap | Wat | Prioriteit | Afhankelijk van |
|---|---|---|---|
| 1 | Metadata (title) uitlezen in scanner | 🔴 Hoog | — |
| 2 | Search werkt dan al beter (titel ipv filename) | ✅ Meteen | Stap 1 |
| 3 | Async search (geen UI freezes) | 🟡 Medium | — |
| 4 | Inverted index bouwen tijdens scan | 🟡 Medium | Stap 1 (voor goede woorden) |
| 5 | SearchIndex serializen/deserializen met bincode | 🟢 Laag | Stap 4 |

---

## 4. Open vragen

1. **Symphonia duration — gewenst of te traag?** Alternatief: duration skippen of alleen voor bepaalde formaten.
2. **Track artiest uit metadata ipv foldernaam?** Sommige bestanden hebben de artiest in tags maar zitten in een "Various Artists" map. Willen we dat de tag voorrang krijgt?
3. **Search op album + artiest ook in de index?** Ja, die woorden moeten ook in de inverted index.

## 5. Nieuwe browse-modus: Year & Composer

Zodra `track.year` en `track.composer` beschikbaar zijn, kunnen we twee nieuwe browse-modi maken:

### Year Browse (bv. `Y`)
- Lijst van alle jaren met track-count (zoals genres)
- `Enter` op een jaar toont alleen tracks uit dat jaar
- Sorteren oplopend/aflopend met `S`
- Handig voor klassiek (werken uit bepaalde periodes) en voor overzicht

### Composer Browse
- Lijst van alle componisten met track-count
- `Enter` op een componist toont alleen diens werken
- Essentieel voor klassieke muziek waar de artiest vaak de uitvoerder is (bv. orkest) maar de componist de echte auteur

### Model-aanpassing
In `models.rs` krijgt `Track` twee nieuwe optionele velden:

```rust
pub struct Track {
    pub path: String,
    pub title: String,
    pub track_number: u32,
    pub duration_secs: u32,
    pub genre: Option<String>,
    pub year: Option<u32>,      // NIEUW
    pub composer: Option<String>, // NIEUW
}
```

### Search-index uitbreiding
De inverted index indexeert ook jaar (als string) en componist, zodat "Beethoven" of "1812" meteen de juiste tracks vindt.

---

## 6. Filter Stack — De grote architectuur-verbetering

### Huidige situatie (probleem)

Momenteel hebben we een `BrowseMode` enum die **mutueel exclusief** is:

```rust
pub enum BrowseMode {
    Library,
    Genre,
    Recent,
    Selection,
}
```

Dit betekent:
- Je kunt **niet** Genre binnen Year doen (geen combinaties)
- Elke mode vereist aparte `if self.browse_mode == ...` checks in de code
- Nieuwe modi toevoegen betekent overal `match` cases bijschrijven
- Breadcrumb-logica is verspreid en fragiel

### Oplossing: Filter stack

We vervangen `BrowseMode` door een **gestapelde lijst van filters**:

```rust
#[derive(Clone)]
pub enum Layer {
    /// Toon een picker-lijst (genres, jaren, componisten) boven op de huidige set
    GenrePicker,
    YearPicker,
    ComposerPicker,
    RecentAlbums,
    /// Een actief filter dat de set inperkt
    Genre(String),
    Year(u32),
    Composer(String),
    Selection,
}

pub struct MusicPlayerApp {
    lay stack: Vec<Layer>,          // filter-stapel
    library: Option<Library>,       // ongefilterde bibliotheek
    cached_filtered: Option<Library>, // na alle filters, voor navigatie
}
```

**Hoe het werkt:**

| Actie | Wat er gebeurt |
|---|---|
| `G` | Push `Layer::GenrePicker` — toont genre-lijst |
| Kies genre "Jazz" | Pop de picker, push `Layer::Genre("Jazz")`, herbereken |
| `Y` | Push `Layer::YearPicker` — toont jaartallen |
| Kies 1990 | Pop picker, push `Layer::Year(1990)`, herbereken |
| Navigeer (pijltjes) | Werkt altijd op `cached_filtered` |
| `Esc` op artist-level (geen album) | Pop laatste filter-laag, herbereken |

**Herberekenen** = filters na elkaar toepassen:

```rust
fn recompute(&mut self) {
    let mut result = self.library.clone();
    for layer in &self.stack {
        match layer {
            Layer::Genre(name) => result = Some(filter_by_genre(&result?, name)),
            Layer::Year(y)     => result = Some(filter_by_year(&result?, *y)),
            Layer::Composer(c) => result = Some(filter_by_composer(&result?, c)),
            Layer::Selection   => result = Some(build_from_selection(&result?, &self.selected_tracks)),
            _ => {} // pickers hebben geen effect op de set
        }
    }
    self.cached_filtered = result;
}
```

### Concreet voorbeeld: van jaar naar genre naar artiest

```
Start:           [Bibliotheek > ]
Druk Y:          Bibliotheek > Jaartallen]          ← picker: kies 1990
Kies 1990:       [Bibliotheek > 1990 > ]             ← alle artiesten in 1990
Druk G:          [Bibliotheek > 1990 > Genres]       ← picker: kies Classical
Kies Classical:  [Bibliotheek > 1990 > Classical > ] ← alleen Classical uit 1990
Navigeer →:      [1990 > Classical > Artiest > Album > Track]
Esc:             [Bibliotheek > 1990 > ]             ← Classical weg
Esc:             [Bibliotheek > ]                    ← 1990 weg
```

Alle combinaties werken: Year+Genre, Composer+Year, Genre+Selection, etc.

### Wat verandert er in de code?

| Bestand | Wat |
|---|---|
| `types.rs` | `BrowseMode` + `NavLevel` → `Layer` enum + filter-stack |
| `app.rs` | `browse_mode`, `genre_filtered_library`, `selection_library` → `stack: Vec<Layer>` + `cached_filtered` |
| `render.rs` | Centrale `match browse_mode` → check of top-of-stack een picker is, anders toon library |
| `navigation.rs` | Alle `if browse_mode ==` → `if stack.last() == Some(Layer::GenrePicker)` |
| `search.rs` | `filter_by_year()`, `filter_by_composer()` toevoegen (nieuwe functies) |

### Waarom dit nu doen?

1. **Metadata (year, composer)** komt eraan → we hebben de pickers nodig
2. **Het legt de basis** voor álle toekomstige filters zonder steeds nieuwe enum-varianten door de code te jagen
3. **De breadcrumb wordt triviaal** — loop door de stack en toon `Naam > Naam >`
4. **Minder code** — geen aparte `genre_filtered_library` en `selection_library` velden meer

### Route naar implementatie

1. **Fase 1 — Filter stack bouwen** (zonder metadata): vervang `BrowseMode` door `Vec<Layer>` met alleen `Layer::Genre`, `Layer::Selection`, `Layer::RecentAlbums`. Alles blijft werken zoals nu, maar dan gestapeld.
2. **Fase 2 — Metadata uitlezen** (year, composer, title) in scanner
3. **Fase 3 — YearPicker + ComposerPicker** toevoegen aan de stack
4. **Fase 4 — Inverted index** voor search