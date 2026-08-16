# Plan: Dependencies up-to-date brengen

## Doel

Alle crates in dit project naar een actuele versie brengen. De `Cargo.toml` is sinds de
initiële commit (8 juni 2026) nooit geüpdatet; rodio 0.19 en eframe 0.28 zijn ~2 jaar oud.
Er is geen bewuste reden voor de oude versies — puur "nooit geüpdatet".

Aanpak: **incrementeel, aparte commit per crate**. Elke stap is klein genoeg om te testen
en een regressie is direct terug te draaien. Volgorde: kleine veilige klussen eerst, dan
de audio-kern, dan cache, en de grote UI-sprong als afsluitend project.

## Audit (geïnstalleerd → actueel, aug 2026)

| Crate | Geïnstalleerd | Actueel | Achterstand | Risico |
|---|---|---|---|---|
| rodio | 0.19.0 (vendored) | 0.22.2 | ~2 j, 3 majors | Hoog — API anders (`Sink`→`Player`) |
| eframe | 0.28.1 | 0.36.1 | ~2 j, 8 majors | Hoog — grote refactoring |
| egui_extras | 0.28.1 | 0.36.1 | idem | volgt eframe |
| lofty | 0.22.4 | 0.25.1 | ~2 j, 3 majors | Middel (0.23.x is yanked) |
| bincode | 1.3.3 | 3.0.0 | 2 majors | Middel — breekt cache-format |
| rand | 0.8.6 | 0.10.2 | 2 majors | Laag — alleen `shuffle_vec` |
| lru | 0.12.5 | 0.18.2 | 6 minors | Nul — dode dependency |
| toml | 0.8.23 | 1.1.4 | 1 major | Laag — alleen `from_str`/`to_string` |
| symphonia | 0.5.5 | 0.6.1 | 1 major | Laag — bewust niet upgraden (zie onder) |
| image | 0.25.10 | 0.25.10 | 0 | — |
| serde | 1.0.228 | 1.0.229 | patch | — |
| serde_json | 1.0.150 | 1.0.151 | patch | — |
| log | 0.4.32 | 0.4.33 | patch | — |
| env_logger | 0.11.11 | 0.11.11 | 0 | — |
| walkdir | 2.5.0 | 2.5.0 | 0 | — |
| crossbeam-channel | 0.5.15 | 0.5.16 | patch | — |
| rayon | 1.12.0 | 1.12.0 | 0 | — |
| natord | 1.0.9 | 1.0.9 | 0 | — |

Notities:

- `Cargo.toml` specificeert het **minimum**; `rayon = "1.8"` lost bijvoorbeeld op naar
  1.12.0. Alleen waar de major verschilt loop je écht achter.
- rodio 0.22.2 gebruikt zelf nog `symphonia ^0.5.5` en `rand ^0.10`.

---

## Fase 0 — Voorbereiding

- [x] `Cargo.lock` uit `.gitignore` halen en committen.
      Zolang de lock niet gecommit is, bouwt elke machine een andere dependency-tree en
      zijn upgrades niet reproduceerbaar testbaar of terug te draaien.
      Validatie: `cargo build` is na commit "up to date" zonder netwerk.
      Commit: `Commit Cargo.lock for reproducible builds`

## Fase 1 — Snel & veilig (klein, laag risico)

| # | Taak | Bestanden | Actie | Validatie | Commit |
|---|---|---|---|---|---|
| 1 | lru verwijderen | `Cargo.toml` | Crate wordt nergens gebruikt (dode dependency) — weghalen | `cargo build` | `Remove unused lru dependency` |
| 2 | rand 0.8 → 0.10 | `src/player.rs`, `src/ui/navigation.rs`, `Cargo.toml` | `thread_rng()` → `rng()`, `gen_range` → `random_range` (`RngExt`), `shuffle` via `SliceRandom`. Bleek ook gebruikt in `RandomAlbum` (navigation.rs) | `cargo test` | `Migrate rand to 0.10` |
| 3 | toml 0.8 → 1.1 | `src/config.rs`, `src/ui/render.rs`, `Cargo.toml` | Gebruik is alleen `toml::from_str`/`to_string` — nul code-wijzigingen nodig geweest | `cargo test` + config laadt | `Update toml to 1.1` |
| 4 | lofty 0.22 → 0.25 | `src/scanner.rs`, `src/ui/edit.rs`, `src/ui/navigation.rs`, `Cargo.toml` | `tag.get`/`remove_key` nemen `ItemKey` nu by value (was `&ItemKey`). `ItemKey::Unknown` is verwijderd in lofty 0.23 — de custom-key-fallbacks (`----:com.apple.itunes:genre`, `originalyear`, `toryear`) zijn verwijderd; `ORIGINALYEAR` (FLAC/OGG) wordt nu automatisch als `OriginalReleaseDate` gelezen | `cargo test` + tags lezen/editen | `Update lofty to 0.25` |

## Fase 2 — Audio-kern: rodio 0.19 → 0.22

- [x] `src/player.rs` migreren naar de nieuwe API:
  - `Sink::try_new(&handle)` → `handle.play()`-equivalent: `DeviceSinkBuilder::open_default_sink()`
    + `Player::connect_new(handle.mixer())`; `OutputStream`/`OutputStreamHandle` bestaan niet meer
  - `try_seek` heet in 0.22 **nog steeds** `try_seek` (geen `seek`)
  - `get_pos`, `skip_one`, `empty`, `clear`, `append`, `set_volume`, `is_paused`, `play`, `pause`
    bestaan op `Player` onder dezelfde namen — geen verdere wijzigingen nodig
  - `Decoder::new(BufReader::new(f))` → `Decoder::try_from(f)` (zet automatisch byte_len +
    seekable voor seek en duur)
- [x] `vendor/rodio/` + `[patch.crates-io]` verwijderen — de byte_len- en
      total_duration-fixes zitten upstream in 0.21+ ingebouwd
- [x] Validatie: `symphonia_flac_seek_works` slaagt, 41/41 tests groen, release-build ok
- [x] Commit: `Upgrade rodio to 0.22, drop vendor patch`

## Fase 3 — Cache-format: bincode 1.3 → 2.0

- [x] `src/scanner.rs` migreren (3 callsites: `serialize_into` ×2, `deserialize_from` ×1
      in `save_cache`/`load_or_scan_library`)
  - Gekozen: **bincode 2.0.1 + `serde`-feature + `config::legacy()`** i.p.v. bincode 3.0:
    - serde-derives blijven werken (`bincode::serde::encode_into_std_write` /
      `decode_from_std_read`)
    - `legacy()` is byte-compatibel met bincode 1 → bestaande cache blijft leesbaar,
      **geen herscan nodig**
    - bincode 3.0 heeft géén serde-feature en zou een volledige rewrite van alle
      datamodellen (Encode/Decode i.p.v. Serialize/Deserialize) vereisen zonder winst
- [x] Validatie: `cargo test` (41/41), build ok
- [x] Commit: `Migrate bincode cache to 2.0 with serde compat`

## Fase 4 — UI-sprong: eframe/egui_extras 0.28 → 0.36 (grootste)

- [x] `eframe::App::update(ctx, frame)` → `App::ui(ui, frame)` + `let ctx = ui.ctx().clone();`
      (egui 0.36 herontwerp: root-UI i.p.v. alleen Context)
- [x] Panels: `SidePanel`/`TopBottomPanel` → `Panel::left/right/top/bottom`;
      `CentralPanel::show(ctx, …)` → `.show(ui, …)` (alle CentralPanels hadden early-return,
      dus max. één per frame — geen ruimte-conflicten)
- [x] `Window::new(...).show(ctx, …)` → `.show(&ctx, …)` (werkt onveranderd)
- [x] Kleine API-hernoemingen: `default_width` → `default_size`, `id_source` → `id_salt`,
      `child_ui` → `new_child(UiBuilder)`, `wants_keyboard_input` →
      `egui_wants_keyboard_input`, `raw_scroll_delta` → `smooth_scroll_delta`,
      `SelectableLabel::new` → `Button::new(...).selected(...)`, `ctx.run` → `ctx.run_ui`
      (tests)
- [x] Test-infra: `FullOutput.textures_delta.clear()` in test-helper (epaint 0.36 panikt
      anders bij het droppen van de Context)
- [x] Validatie: 41/41 tests groen (incl. AZERTY-shortcuts en FLAC-seek), release-build ok
- [x] Commit: `Upgrade eframe and egui_extras to 0.36`

## Fase 5 — Afronding

- [ ] Laatste volledige check: `cargo test` groen, `cargo build --release` zonder waarschuwingen
      (al uitgevoerd na elke fase — nog een keer herbevestigen bij afsluiting)
- [ ] `Cargo.toml` opschonen: rodio-patch-commentaar verwijderen, features/commentaar actueel
      (deels al gedaan bij rodio; controleer de rest)
- [ ] Smoke-test op de USB-schijf: opstarten met gewijzigde schijfletter → cache-remap
      werkt nog (geen herscan)
- [ ] Commit: `Clean up Cargo.toml after dependency updates`

---

## Bewust niet upgraden

- **symphonia 0.5.5** — rodio 0.22.2 gebruikt zelf nog `^0.5.5`; een losse 0.6-upgrade
  geeft twee versies naast elkaar en nul winst
- **image, serde, serde_json, log, env_logger, walkdir, crossbeam-channel, rayon,
  natord** — allemaal actueel of op patch-niveau

## Definitie van klaar

Alle fasen doorlopen → `Cargo.toml` bevat alleen actuele majors, `vendor/` is weg,
alle 41+ tests slagen, en de release-build draait met seek, spoelen, loops en tags
zoals nu.

---

## Vervolgstap (niet in dit traject): `loop-editor/`

De map `loop-editor/` is een **apart project** (waveform loop-editor app) met een eigen
`Cargo.toml` die nog de oude versies gebruikt: `eframe 0.28`, `rodio 0.19` (met
`vendor/rubberband`), `rfd 0.14`, `soundtouch 0.5.4`, `rustfft 6.2`.

- Het heeft een eigen dependency-tree (eigen `Cargo.lock`) en eigen code
  (`src/waveform.rs`, `src/waveform_player.rs`, …)
- Een upgrade volgt hetzelfde stappenplan als hierboven, maar moet apart worden
  uitgevoerd: eerst rodio 0.22 (`Sink`→`Player`, `Decoder::try_from`), dan eframe 0.36
  (`App::ui`, `Panel`, …)
- Besluit van de gebruiker nodig: is de loop-editor nog in actief gebruik?
