# ShowMate

Cue-gebaseerde show-control software voor Windows, geschreven in Rust met een Slint-GUI.

## Structuur

| Crate | Inhoud |
|---|---|
| `crates/showmate-core` | Cue-model, showfile (JSON), klok, cue-engine. Geen GUI/audio/video; volledig testbaar. |
| `crates/showmate-audio` | Decoderen (symphonia), pure mixer, achtergrond-cache, cpal-uitvoer. Mixer en decoder zijn testbaar zonder geluidskaart. |
| `crates/showmate-video` | Eén `mpv`-proces per scherm, aangestuurd via JSON-IPC (Unix-socket / Windows named pipe). |
| `crates/showmate-io` | OSC (UDP, `rosc`) en MIDI-uitvoer (`midir`). Argumenten-parser, MIDI-bytes en UDP-verzending zijn getest. |
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

0. Opzet ✅ · 1. Engine ✅ · 2. GUI (inspector, bewerken) ✅ · 3. Audio ✅ · 4. Video (mpv) ✅
5. OSC/MIDI ✅ · 6. Groepen, panic/reset, ongedaan maken, auto-save, NL/EN, installer ✅

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

## Video

Video draait via **mpv** als apart proces (geen gelinkte libmpv: geen DLL-/linkproblemen, en een
decoder-crash legt de show niet plat). Zet `mpv.exe` (van mpv.io / shinchiro-builds) in de map
`mpv\` naast `showmate.exe`, of stel `SHOWMATE_MPV` in, of zorg dat `mpv` op het PATH staat.

- Output `Display 1`, `Display 2` of `Display 1+2` (beide schermen tegelijk).
- Loop, duur (anders het einde van het bestand) en pauze/hervat werken zoals bij audio.
- Een nieuwe videocue op een scherm vervangt de vorige op dat scherm.
- Bij het openen van de show wordt per gebruikt scherm alvast een (onzichtbare) mpv gestart;
  het venster verschijnt bij de eerste videocue en blijft zwart staan na afloop.
  STOP stopt de beelden en sluit de uitvoervensters.
- Preview rechtsonder: ~2,5×/s een screenshot van het actieve scherm.
- Standaard draait de uitvoer in een gewoon venster. Voor een projector: `settings.json` naast de showfile:

```json
{ "video": { "fullscreen": true, "screens": [1, 2], "extra_args": ["--hwdec=auto"] } }
```

`screens[n]` is de monitor-index (0 = eerste monitor) voor Display n+1. `SHOWMATE_FULLSCREEN=1` en
`SHOWMATE_MPV_ARGS` overschrijven dit.

Beperkingen: opstartlatentie van een videocue is ~50–200 ms (geen pre-roll); mpv-processen worden
bij een crash van ShowMate niet automatisch opgeruimd.

**Geluid uit video** staat standaard aan, via het standaard audioapparaat van Windows. Per cue kun je
het dempen (vinkje *Geluid van video dempen*) en het volume instellen (dB). Bij `Display 1+2` komt het
geluid alleen van het eerste scherm.

## OSC en MIDI

Alles is aan te passen via **⚙ Instellingen** (opgeslagen in `settings.json` naast de showfile):

- **OSC-bestemmingen**: naam, IP-adres/hostnaam en poort (bv. `Lighting` → `192.168.1.50:8000`).
  Een OSC-cue kiest een bestemming op naam; hernoem je er een, dan volgen de cues vanzelf.
- **OSC-commando's**: herbruikbare adressen + argumenten. Pas het adres één keer aan in de
  instellingen en alle cues met dat commando volgen. Een cue kan ook een eigen adres hebben.
- **MIDI-uitgang**: kies een poort (bv. loopMIDI, een interface) en ververs de lijst.

Argumenten schrijf je als tekst: `f:0.7 5 true "tekst"`. Zonder prefix wordt het type geraden
(`5` = int, `0.7` = float); met `i:` `f:` `s:` `b:` forceer je het. Veel lichttafels eisen floats: schrijf `f:0`.

Overgangen (bv. spots 0 → 70% in 5 s) doet de tafel zelf: ShowMate stuurt één bericht bij de start van
de cue. De knop **▶ Test** bij een OSC/MIDI-cue verstuurt het bericht meteen, zonder de show te starten.
MIDI-cues ondersteunen Note On/Off, Control Change en Program Change (kanaal 1–16).

## Groepen

Een groep is een cue met de cues eronder als kinderen (ingesprongen in de lijst). Er zijn drie soorten:

| Soort | Gedrag |
|---|---|
| **Groep** | kinderen na elkaar; elk kind start `delay` nadat het vorige is afgelopen |
| **Parallel** | alle kinderen tegelijk, elk `delay` na de start van de groep |
| **Loop** | als Groep, maar herhaalt tot STOP of tot de *duur* van de groep om is |

Eén GO start de hele groep; de groep eindigt als alle kinderen klaar zijn. De Start-instelling van
kinderen telt niet. Een `Auto`-cue direct na een groep volgt de start van de groep.

- *Cue toevoegen* terwijl een groep of kind is geselecteerd: de nieuwe cue komt in de groep. Een nieuwe
  groep komt altijd op het hoogste niveau (geen groepen in groepen).
- ↑/↓ op een kind schuift het binnen de groep en, aan de rand, uit de groep. Op een groep verplaatst
  het het hele blok.
- Een groep verwijderen *heft hem op*; de kinderen blijven staan.

## Sneltoetsen en veiligheid

Alle sneltoetsen zijn aan te passen in **⚙ Instellingen → Sneltoetsen**: klik *Wijzig* en druk de nieuwe
combinatie (bv. `Ctrl+Shift+G`, `F5`, `Enter`). Een toets die al in gebruik is wordt geweigerd, met de naam van
de actie die hem heeft. *Wis* maakt een actie toetsloos, *Standaard* zet hem terug. Opgeslagen in
`settings.json` (`"shortcuts": {"go": "Enter"}`; alleen afwijkingen). Standaardtoetsen:

| Toets | Actie |
|---|---|
| `Spatie` | GO |
| `P` | pauze / hervat |
| `Esc` | STOP (korte fade) |
| `Shift+Esc` | **PANIC**: geluid meteen afkappen, beelden weg |
| `Ctrl+R` | Reset: alles stoppen en terug naar het begin |
| `Ctrl+Z` / `Ctrl+Y` | ongedaan maken / opnieuw (ook `Ctrl+Shift+Z`) |
| `Ctrl+D` | cue dupliceren |
| `Ctrl+S` | opslaan |
| `Delete` | cue verwijderen |
| `↑` `↓` | vorige / volgende cue selecteren |

- **Ongedaan maken** werkt voor alle wijzigingen aan de lijst en aan cue-eigenschappen (typen in een veld
  telt als één stap), tot 100 stappen. Niet mogelijk zolang cues gepland staan; druk eerst op STOP.
- **Auto-save**: elke 30 s een kopie (`<show>.autosave`) als er niet-opgeslagen wijzigingen zijn, en bij
  afsluiten. Na een crash of afsluiten zonder opslaan wordt die kopie bij de volgende start hersteld.
  Bij opslaan blijft de vorige versie bewaard als `<show>.bak`.

## Taal

Nederlands (standaard) en Engels: kies in ⚙ Instellingen. De interface gebruikt Slint's `@tr`;
vertalingen staan in `crates/showmate-app/lang/<taal>/LC_MESSAGES/showmate-app.po` (`msgctxt "MainWindow"`).
Foutmeldingen uit de audio-, video- en OSC-lagen zijn nog Nederlands.

## Installer

```
cargo build --release -p showmate-app
powershell -File installer\fetch-mpv.ps1        # optioneel: mpv meeleveren
iscc /DAppVersion=0.1.0 installer\showmate.iss  # → dist\ShowMate-Setup-0.1.0.exe
```

De CI bouwt dit ook (job `installer`). Het mpv-downloadscript en de Inno Setup-stap zijn nog niet
op een echte Windows-runner uitgeprobeerd.
