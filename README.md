# ShowMate

Cue-gebaseerde show-control software voor Windows, geschreven in Rust met een Slint-GUI.

## Structuur

| Crate | Inhoud |
|---|---|
| `crates/showmate-core` | Cue-model, showfile (JSON), klok, cue-engine. Geen GUI/audio/video; volledig testbaar. |
| `crates/showmate-app` | Slint-GUI: cue-lijst, inspector, cues toevoegen/verwijderen/verplaatsen, opslaan. |

## Bouwen

```
cargo test                      # engine-tests (draait overal)
cargo run -p showmate-app       # GUI
cargo build --release -p showmate-app
```

Sneltoetsen: `spatie` = GO, `p` = pauze, `Esc` = stop, `Ctrl+S` = opslaan.
Showfile: `%APPDATA%\ShowMate\show.showmate` (override met `SHOWMATE_FILE`); zonder bestand start de demoshow.

## Engine-semantiek

- Showtijd start bij de eerste GO en staat stil tijdens pauze.
- GO plant de eerstvolgende wachtende cue op `start + delay`; direct volgende `Auto`-cues
  worden meegepland, elk `delay` na de start van de vorige cue.
- Een cue eindigt na `duration`. Zonder duur: Osc/Midi eindigen direct, Audio/Video lopen tot STOP.
  `looping` cues eindigen nooit vanzelf.
- STOP zet actieve cues op *Gestopt* en geplande cues terug op *Wachtend*.

## Roadmap

0. Opzet ✅ · 1. Engine ✅ · 2. GUI (inspector, bewerken) ✅ · 3. Audio · 4. Video (libmpv)
5. OSC/MIDI · 6. Groep/Parallel/Loop, installer

## Bewerken

De cue-lijst (invoegen, verwijderen, verplaatsen) is vergrendeld zolang er cues *gepland* staan;
druk eerst op STOP. Een actieve cue kan niet verwijderd worden. Eigenschappen (naam, delay, duur, …)
zijn altijd aanpasbaar en gelden voor latere planning.
