# ShowMate

Cue-gebaseerde show-control software voor Windows, geschreven in Rust met een Slint-GUI.

## Structuur

| Crate | Inhoud |
|---|---|
| `crates/showmate-core` | Cue-model, showfile (JSON), klok, cue-engine. Geen GUI/audio/video; volledig testbaar. |
| `crates/showmate-audio` | Decoderen (symphonia), pure mixer, achtergrond-cache, cpal-uitvoer. Mixer en decoder zijn testbaar zonder geluidskaart. |
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

0. Opzet ✅ · 1. Engine ✅ · 2. GUI (inspector, bewerken) ✅ · 3. Audio ✅ · 4. Video (libmpv)
5. OSC/MIDI · 6. Groep/Parallel/Loop, installer

## Bewerken

De cue-lijst (invoegen, verwijderen, verplaatsen) is vergrendeld zolang er cues *gepland* staan;
druk eerst op STOP. Een actieve cue kan niet verwijderd worden. Eigenschappen (naam, delay, duur, …)
zijn altijd aanpasbaar en gelden voor latere planning.

## Audio

- Formaten: mp3, wav, flac, ogg/vorbis, aac/m4a. Alles wordt bij het openen van de show op de
  achtergrond gedecodeerd en naar de samplerate van het apparaat omgezet, zodat GO nooit wacht.
- Relatieve mediapaden zijn relatief aan de map van de showfile (of de submap `media`).
- Per cue: volume (dB), fade in, fade out, loop, uitgang (`Audio 1+2`, `Audio 3+4`, …).
- STOP en pauze gebruiken een korte ramp tegen kliks. De mixer begrenst de som op 0 dBFS.
- Zonder bestandsduur eindigt een audio-cue na de lengte van het bestand.
- Probeer zonder GUI: `cargo run -p showmate-audio --example play -- bestand.wav`.

Bekende beperkingen: lineaire resampling (geen sinc), WASAPI shared mode (geen ASIO),
cues starten op de UI-timer (≤ 20 ms jitter) tot de engine een eigen thread krijgt.
