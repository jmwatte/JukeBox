# JukeBox — Verbeterplan

> Analyse en actieplan voor de Rust muziekspeler "JukeBox"
> Datum: 2026-07-15

---

## Inhoudsopgave

1. [Statusoverzicht](#1-statusoverzicht)
2. [Wat goed is](#2-wat-goed-is)
3. [Wat beter kan](#3-wat-beter-kan)
4. [Actiepunten](#4-actiepunten)
    - [🔴 Hoog — direct aanpakken](#-hoog--direct-aanpakken)
    - [🟡 Medium — gewenst](#-medium--gewenst)
    - [🟢 Laag — nice to have](#-laag--nice-to-have)
5. [Notities](#5-notities)

---

## 1. Statusoverzicht

| Aspect | Oordeel | Toelichting |
|---|---|---|
| Architectuur | 7/10 | Goede thread-scheiding, maar `MusicPlayerApp` is een God Object |
| Correctheid | 7/10 | Enkele `unwrap()` gevallen, shuffle herimplementeert `rand` |
| Testdekking | 0/10 | Geen tests — grootste risico |
| Documentatie | 4/10 | `PLAN.md` aanwezig, maar geen `README.md` of handleiding |
| Onderhoudbaarheid | 6/10 | God Object maakt wijzigingen risicovol, dode code aanwezig |
| Gebruikerservaring | 8/10 | Doordachte shortcuts, tag-editor, waveform, filters |

**Eindoordeel: 7/10** — Solide basis met typische "one-man-project" groeipijnen.

---

## 2. Wat goed is

### 2.1 Architectuur & Concurrency

- **Audio-thread los van GUI-thread** via `crossbeam-channel`. Geen lock-contention op de audio hot-path.
- **Waveform-speler in eigen thread** met `rubato` voor pitch-shifting en time-stretching.
- **Parallelle scanner** via `rayon::par_bridge` met `Mutex<HashMap>` — efficient gebruik van multi-core.

### 2.2 Metadata & Scanning

- **Uitgebreide tag-lezer** via `lofty`: genre, jaar (met fallbacks `originalyear`, `toryear`), componist, discnummer, album-artiest.
- **Cache-systeem** met `bincode`, versienummer en `dir_modified` timestamp.
- **CD/disc-detectie** uit mappenstructuur (mappen beginnend met `cd` of `disc`).

### 2.3 Foutafhandeling

- Audio-thread stuurt `PlaybackError` events naar GUI — geen panics.
- Scanner vangt corrupte cache en herscant automatisch.
- **Reconnect Audio** (F6) herstelt verloren verbinding zonder herstart.

### 2.4 UI/UX

- **Configureerbaar shortcut-systeem** met validatie en auto-repair.
- **Batch tag-editor** met resizable split view, raw tag inspectie, genre-removal.
- **Compact mode**, help-scherm, filter-pipeline (genre → jaar → componist).
- **Waveform-editor** met zoom, scroll, sleepbare A-B markers, playhead drag.
- **A-B loop** zowel in de normale speler als in de waveform-editor.

### 2.5 Codekwaliteit

- Commentaar en berichten consistent in het Nederlands.
- Duidelijke foutmeldingen.
- Goede dependency-keuzes (`rodio`, `lofty`, `symphonia`, `walkdir`).

---

## 3. Wat beter kan

### 3.1 `MusicPlayerApp` — God Object

`MusicPlayerApp` in `app.rs` heeft **~100 velden** en wordt geïmplementeerd over vier bestanden:
`app.rs`, `render.rs`, `navigation.rs`, `edit.rs`. Dit is een klassiek anti-patroon.

**Probleem:** Wijzigingen zijn risicovol omdat één struct door vier bestanden wordt gedeeld.
De borrow-checker wordt steeds lastiger tevreden te stellen.

### 3.2 Geen testen

**Nul tests** — geen unit tests, geen integration tests. Voor kernlogica zoals:
- Queue management
- Shuffle/repeat modes
- Filterfuncties (`search.rs`)
- Tag-editing (`edit.rs`)

...is dit het grootste risico voor regressies.

### 3.3 Zelfgemaakte PRNG in `shuffle_vec`

```rust
// player.rs:372-386
fn shuffle_vec<T>(vec: &mut Vec<T>) {
    // Zelfgemaakte LCG in plaats van rand::Rng
    let seed = std::time::SystemTime::now()...;
    let mut rng = seed;
    // ...
}
```

Je hebt `rand = "0.8"` al in `Cargo.toml` staan. Gebruik die.

### 3.4 Onveilig `unwrap()` gebruik

- `tokio::runtime::Runtime::new().unwrap()` in `main.rs` — panickt bij resource limits.
- `artists_map.lock().unwrap()` in rayon closure — panickt bij poisoned mutex.
- `toml::to_string(&default_config).unwrap()` in `config.rs`.

### 3.5 Overbodige `tokio` async in scanner

```rust
std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { scanner::load_or_scan_library(...).await; });
});
```

De scanner is volledig synchroon (walkdir + rayon). Geen `.await` punten.
Dit voegt complexiteit zonder voordeel.

### 3.6 Cache-reset na tag-edit

```rust
// scanner.rs:39-41
pub fn save_cache(library: &Library) {
    CacheData { version: CACHE_VERSION, dir_modified: 0, library: library.clone() }
}
```

Na elke tag-edit wordt `dir_modified = 0` weggeschreven. **Bij de volgende start wordt de
hele bibliotheek opnieuw gescand.** Dit is inefficiënt voor gebruikers met grote collecties.

### 3.7 `#![allow(dead_code)]` in main.rs

Schakelt een nuttige compiler-waarschuwing **globaal** uit.
Beter om specifiek per item `#[allow(dead_code)]` te zetten of ongebruikte code op te ruimen.

### 3.8 Clone-rijke filterfuncties

Alle filterfuncties in `search.rs` doen **deep clones van de hele Library**.
Voor elke toetsaanslag in de zoekbalk wordt een groot deel van de bibliotheek gekopieerd.

### 3.9 Geen logging framework

Alleen `println!` / `eprintln!` door de hele codebase.
Een `log` + `env_logger` of `tracing` zou veel beter zijn voor debugging.

### 3.10 Grof polling-interval in audio-thread

```rust
std::thread::sleep(Duration::from_millis(100));
```

Maximaal 10 positie-updates per seconde. Voor nauwkeurige weergave (seek, loop) is dit grof.

### 3.11 Dubbele A-B loop systemen

Er zijn **twee** A-B loop implementaties:
1. **Main player** (`player.rs`) — via rodio `seek()`
2. **Waveform player** (`waveform_player.rs`) — via rubato

Synchronisatie via `waveform_pending_loop` is fragiel en foutgevoelig.

---

## 4. Actiepunten

### 🔴 Hoog — direct aanpakken

| # | Actie | Bestand(en) | Moette | Impact |
|---|---|---|---|---|
| H1 | **Splits `MusicPlayerApp` op** in deel-staten: `PlaybackState`, `NavigationState`, `FilterState`, `EditState`, `WaveformState`. Elke module krijgt zijn eigen struct met alleen de relevante velden en methodes. | `ui/app.rs`, `ui/render.rs`, `ui/navigation.rs`, `ui/edit.rs` | 3 dagen | Zeer groot — schaalbare architectuur |
| H2 | **Schrijf unit tests** voor: `search.rs` (filter_library, filter_by_genre, collect_genres), `player.rs` (shuffle, repeat modes), `loops.rs` (add/remove/generate_label). | Nieuwe `tests/` map of inline `#[cfg(test)]` | 1 dag | Groot — voorkomt regressies |
| H3 | **Vervang `unwrap()` door proper error handling.** Minstens: scanner `lock().unwrap()` → `expect()` of error-log, main `Runtime::new().unwrap()` → match met foutmelding. | `main.rs`, `scanner.rs`, `config.rs` | 0.5 dag | Medium — voorkomt panics |

**H1 detailplan — opsplitsen `MusicPlayerApp`:**

```
Huidig:                          Nieuw:
MusicPlayerApp                   MusicPlayerApp (coördinator)
├── ~100 velden                  ├── playback: PlaybackState
├── 4 impl-bestanden             ├── navigation: NavigationState
                                 ├── filters: FilterState
                                 ├── editing: EditState
                                 └── waveform: WaveformState
```

Elke `*State` struct krijgt zijn eigen `impl`-blok met de relevante methodes.
`MusicPlayerApp::update()` delegeert naar de deel-staten.

---

### 🟡 Medium — gewenst

| # | Actie | Bestand(en) | Moette | Impact |
|---|---|---|---|---|
| M1 | **Vervang `shuffle_vec` door `rand::Rng::shuffle`.** | `player.rs` | 15 min | Klein — beter onderhoud |
| M2 | **Haal `tokio` async weg uit scanner.** De scanner is synchroon. Vervang door directe `std::thread::spawn` + synchrone functie. | `scanner.rs`, `main.rs` | 0.5 dag | Medium — vereenvoudigt code |
| M3 | **Los `dir_modified = 0` cache-probleem op.** Lees de echte directory timestamp uit bij `save_cache` zodat de cache geldig blijft na tag-edits. | `scanner.rs` | 0.5 dag | Medium — snellere startups |
| M4 | **Vervang `#![allow(dead_code)]` door specifieke `#[allow(...)]`** of remove dead code. | `main.rs` | 15 min | Klein — code hygiene |
| M5 | **Consolideer A-B loop systemen.** Kies één implementatie (main player) of maak de waveform player de enige. Voorkom dubbele state. | `player.rs`, `waveform_player.rs`, `ui/render.rs` | 1 dag | Groot — minder bugs |

---

### 🟢 Laag — nice to have

| # | Actie | Bestand(en) | Moette | Impact |
|---|---|---|---|---|
| L1 | **Optimaliseer filterfuncties.** Gebruik borrowing i.p.v. cloning. Overweeg `Arc<Vec<Track>>` of een `TrackRef`-struct die alleen paden bijhoudt. | `search.rs` | 1 dag | Groot bij grote libraries |
| L2 | **Voeg `log` + `env_logger` toe.** Vervang `println!` / `eprintln!`. | `Cargo.toml`, overal | 0.5 dag | Medium — debugging |
| L3 | **Verlaag polling-interval (30ms) of gebruik blocking `recv()`** voor position-updates. | `player.rs` | 0.5 dag | Medium — vloeiendere UI |
| L4 | **Heractiveer `#![windows_subsystem = "windows"]`** voor release builds. | `main.rs` | 5 min | Klein — verbergt console |
| L5 | **Schrijf README.md** met installatie en gebruik. | Nieuw bestand | 1 dag | Groot — gebruikersvriendelijk |

---

## 5. Notities

### 5.1 Zoekfunctie

Ja, er is al een zoekfunctie. In `search.rs` staat `filter_library()` die zoekt op:

- Track titel (`track.title`)
- Album naam (`album.title`)
- Artiest naam (`artist.name`)
- Bestandspad (`track.path`)

De zoekfunctie wordt geactiveerd met `/` (of de toegewezen shortcut) en toont een
zoekvenster waarin je typt. Search-resultaten worden live gefilterd.

### 5.2 Loop-editor

Het bestand `loop-editor/` is een apart project dat uit de hoofdcode is verwijderd.
Het bevat een experimentele waveform-editor met C++ rubberband-integratie en is **niet**
onderdeel van de hoofd-JukeBox applicatie.

**Aanbeveling:** Verwijder de `loop-editor/` directory uit de hoofd-repo als deze definitief
gescheiden blijft. De A-B loop-functionaliteit in de hoofd-app (via `[` / `]` / `\` en de
waveform-editor) is voldoende voor eenvoudig loop-gebruik.

### 5.3 Prioriteitsadvies

```
Week 1: H1 (splitsen) + H2 (tests)
Week 2: M2 (tokio eruit) + M4 (dead code)
Week 3: M1 (rand) + M3 (cache) + M5 (loop consolidatie)
Week 4: L1-L5 (optimalisaties + documentatie)
```
