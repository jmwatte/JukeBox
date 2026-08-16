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

- [ ] `Cargo.lock` uit `.gitignore` halen en committen.
      Zolang de lock niet gecommit is, bouwt elke machine een andere dependency-tree en
      zijn upgrades niet reproduceerbaar testbaar of terug te draaien.
      Validatie: `cargo build` is na commit "up to date" zonder netwerk.
      Commit: `Commit Cargo.lock for reproducible builds`

## Fase 1 — Snel & veilig (klein, laag risico)

| # | Taak | Bestanden | Actie | Validatie | Commit |
|---|---|---|---|---|---|
| 1 | lru verwijderen | `Cargo.toml` | Crate wordt nergens gebruikt (dode dependency) — weghalen | `cargo build` | `Remove unused lru dependency` |
| 2 | rand 0.8 → 0.10 | `src/player.rs`, `Cargo.toml` | Alleen `shuffle_vec`: `thread_rng()` → `rand::rng()`. Doe dit vóór of vlak na rodio (rodio 0.22 brengt rand 0.10 mee) | `cargo test` | `Migrate rand to 0.10` |
| 3 | toml 0.8 → 1.1 | `src/config.rs`, `src/ui/render.rs`, `Cargo.toml` | Gebruik is alleen `toml::from_str`/`to_string` — kans is groot dat er nul code-wijzigingen nodig zijn | `cargo test` + config laadt | `Update toml to 1.1` |
| 4 | lofty 0.22 → 0.25 | `src/scanner.rs`, `src/ui/edit.rs`, `src/ui/navigation.rs`, `Cargo.toml` | Slaat de yanked 0.23.x over. Gebruik: `Probe`, `ItemKey`/`ItemValue`, `TagType`, `TagExt`, `WriteOptions`. Compile-and-fix; let op `TagType`- en fout-API | `cargo test` + tags lezen/editen | `Update lofty to 0.25` |

## Fase 2 — Audio-kern: rodio 0.19 → 0.22

- [ ] `src/player.rs` migreren naar de nieuwe API:
  - `Sink::try_new(&handle)` → `handle.play()` (retourneert `Player`)
  - `try_seek(d)` → `seek(d)` (geeft `Result` terug)
  - `get_pos`, `skip_one`, `empty`, `clear`, `append`, `set_volume`, `is_paused`, `play`
    bestaan op `Player` onder dezelfde namen
  - `Decoder::new` en `total_duration()` blijven, maar samples zijn nu `f32` — let op casts
- [ ] `vendor/rodio/` + `[patch.crates-io]` verwijderen — onze twee patches
      (`byte_len`-seek en `total_duration`-conversie) zitten upstream in 0.21+ ingebouwd
- [ ] Validatie: `cargo test symphonia_flac_seek_works` (poortwachter seek + duur),
      daarna `cargo test` volledig + release-build
- [ ] Commit: `Upgrade rodio to 0.22, drop vendor patch`

## Fase 3 — Cache-format: bincode 1.3 → 3.0

- [ ] `src/scanner.rs` migreren (3 callsites: `serialize_into` ×2, `deserialize_from` ×1
      in `save_cache`/`load_or_scan_library`)
  - bincode 2/3: `encode_to_vec`/`decode_from_slice` met `bincode::config::standard()`,
    eigen `Encode`/`Decode`-traits (of serde-compat-feature)
- [ ] Accepteer: **éénmalige herscan** — het cache-formaat breekt, de eerste start na de
      upgrade scant opnieuw. De enige stap die dit kost.
- [ ] Validatie: `cargo test`, start de app → cache wordt opnieuw opgebouwd
- [ ] Commit: `Migrate bincode cache to 3.0`

## Fase 4 — UI-sprong: eframe/egui_extras 0.28 → 0.36 (grootste, apart project)

- [ ] Branch/plan maken — sprong over 8 majors verdient eigen tijd
- [ ] Compile-and-fix van `src/ui/*` (render, app, navigation, shortcuts, edit):
      `Context`-API, widget-signaturen en kleur/`RichText`-API zijn veranderd
- [ ] Toetsenmapping opnieuw valideren — `src/ui/shortcuts.rs` bevat de AZERTY-fixes
      (`Key::Semicolon`/`Key::Quote`, `Comma`/`Period`-fallbacks); opnieuw testen tegen
      de nieuwe egui-`Key`-enum
- [ ] Handmatige UI-test: navigatie, filters, editor, albumhoezen (`image`/`egui_extras`)
- [ ] Validatie: `cargo test` (incl. `ui::shortcuts::tests`), release-build, grondige handtest
- [ ] Commit: `Upgrade eframe and egui_extras to 0.36` (of meerdere commits)

## Fase 5 — Afronding

- [ ] Laatste volledige check: `cargo test` groen, `cargo build --release` zonder waarschuwingen
- [ ] `Cargo.toml` opschonen: rodio-patch-commentaar verwijderen, features/commentaar actueel
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
