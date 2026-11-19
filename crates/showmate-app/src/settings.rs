//! Programma-instellingen uit `settings.json` naast de showfile. Alles is optioneel.
//!
//! ```json
//! {
//!   "osc":  { "destinations": [{ "name": "Lighting", "host": "192.168.1.50", "port": 8000 }],
//!             "commands":     [{ "name": "Spots op", "address": "/eos/cue/1/fire", "args": "" }] },
//!   "midi": { "output": "loopMIDI Port" },
//!   "audio": { "device": "Speakers (Realtek High Definition Audio)" },
//!   "shortcuts": { "go": "Enter", "pause": "F5" },
//!   "language": "nl",
//!   "video": { "fullscreen": true, "screens": [1, 2], "mpv_path": "C:\\mpv\\mpv.exe", "extra_args": [] }
//! }
//! ```
//! OSC en MIDI zijn ook in het programma aan te passen (⚙ Instellingen). `video.screens[n]` is de
//! monitor-index (0 = eerste monitor) voor Display n+1.

use serde::{Deserialize, Serialize};
use showmate_video::VideoConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub video: VideoSettings,
    #[serde(default)]
    pub osc: OscSettings,
    #[serde(default)]
    pub midi: MidiSettings,
    #[serde(default)]
    pub audio: AudioSettings,
    /// Eigen sneltoetsen: actie → toets (bv. `"go": "Enter"`); alleen afwijkingen van de standaard.
    #[serde(default)]
    pub shortcuts: std::collections::BTreeMap<String, String>,
    /// Laatst geopende show; wordt bij het starten weer geopend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_show: Option<PathBuf>,
    /// Recent geopende shows (nieuwste eerst).
    #[serde(default)]
    pub recent: Vec<PathBuf>,
    /// Bestanden die je toevoegt kopiëren naar de map `media` naast de show.
    #[serde(default = "default_true")]
    pub copy_media: bool,
    /// Taal van de interface: `"nl"` (standaard) of `"en"`.
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_true() -> bool {
    true
}

/// Zet `path` bovenaan de lijst recente shows (zonder dubbelen, maximaal 8).
pub fn push_recent(recent: &mut Vec<PathBuf>, path: &Path) {
    recent.retain(|p| p != path);
    recent.insert(0, path.to_path_buf());
    recent.truncate(8);
}

/// Map voor instellingen en de standaardshow: de map van `SHOWMATE_FILE` (ontwikkeling),
/// anders `%APPDATA%\ShowMate`, anders de huidige map.
pub fn app_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("SHOWMATE_FILE") {
        return Path::new(&p)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
    }
    if let Some(dir) = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME")) {
        return PathBuf::from(dir).join("ShowMate");
    }
    PathBuf::new()
}

fn default_language() -> String {
    "nl".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSettings {
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpv_path: Option<PathBuf>,
    /// Oude instelling (display → monitor); alleen nog gelezen, vervangen door `outputs`.
    #[serde(default, skip_serializing)]
    #[allow(dead_code)] // wordt alleen nog gelezen zodat oude bestanden laden
    pub screens: Vec<usize>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Benoemde video-uitgangen: elk met de monitor(en) waarop het beeld komt.
    #[serde(default = "default_video_outputs")]
    pub outputs: Vec<VideoOutputDef>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            fullscreen: false,
            mpv_path: None,
            screens: Vec::new(),
            extra_args: Vec::new(),
            outputs: default_video_outputs(),
        }
    }
}

/// Een video-uitgang, bv. "Projector" → monitor 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoOutputDef {
    pub name: String,
    /// Monitoren (0 = eerste monitor) waarop deze uitgang het beeld toont; meerdere = spiegelen.
    pub monitors: Vec<usize>,
}

fn default_video_outputs() -> Vec<VideoOutputDef> {
    let def = |name: &str, monitors: &[usize]| VideoOutputDef {
        name: name.into(),
        monitors: monitors.to_vec(),
    };
    vec![
        def("Display 1", &[0]),
        def("Display 2", &[1]),
        def("Display 1+2", &[0, 1]),
    ]
}

/// Een audio-uitgang, bv. "Zaal" → geluidskaart X, kanaal 1+2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioOutputDef {
    pub name: String,
    /// Naam van de geluidskaart; `None` = het standaardapparaat van het systeem.
    #[serde(default)]
    pub device: Option<String>,
    /// Kanalen van de kaart (1-gebaseerd) voor links en rechts.
    pub left: u16,
    pub right: u16,
}

fn default_audio_outputs() -> Vec<AudioOutputDef> {
    let def = |name: &str, l: u16| AudioOutputDef {
        name: name.into(),
        device: None,
        left: l,
        right: l + 1,
    };
    vec![def("Audio 1+2", 1), def("Audio 3+4", 3)]
}

/// Een OSC-ontvanger (bv. de lichttafel). Cues verwijzen ernaar met de naam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OscDestination {
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Een herbruikbaar OSC-commando: één plek om het adres aan te passen voor alle cues die het gebruiken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OscCommand {
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OscSettings {
    pub destinations: Vec<OscDestination>,
    pub commands: Vec<OscCommand>,
}

impl Default for OscSettings {
    fn default() -> Self {
        Self {
            destinations: vec![OscDestination {
                name: "Lighting".into(),
                host: "127.0.0.1".into(),
                port: 8000,
            }],
            commands: Vec::new(),
        }
    }
}

/// Audio-uitgangen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Oude instelling (één apparaat voor alles); wordt bij het laden op de uitgangen toegepast.
    #[serde(default, skip_serializing)]
    pub device: Option<String>,
    /// Benoemde uitgangen: elk gekoppeld aan een geluidskaart en twee kanalen.
    #[serde(default = "default_audio_outputs")]
    pub outputs: Vec<AudioOutputDef>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: None,
            outputs: default_audio_outputs(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiSettings {
    /// Naam van de MIDI-uitgang; `None` = geen.
    #[serde(default)]
    pub output: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            video: VideoSettings::default(),
            osc: OscSettings::default(),
            midi: MidiSettings::default(),
            audio: AudioSettings::default(),
            shortcuts: Default::default(),
            last_show: None,
            recent: Vec::new(),
            copy_media: true,
            language: default_language(),
        }
    }
}

/// Leest `settings.json` uit `dir`. Ontbreekt het bestand, dan gelden de standaardwaarden;
/// bij een leesfout krijg je die óók, plus een melding.
/// Waar het geluid van een audio-uitgang heen gaat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioRoute {
    /// Geluidskaart; `None` = standaardapparaat.
    pub device: Option<String>,
    /// Kanalen van de kaart, 0-gebaseerd.
    pub left: usize,
    pub right: usize,
}

impl Settings {
    /// Zoekt de audio-uitgang met deze naam. Is er geen uitgang met die naam, dan telt een
    /// oude naam als `Audio 3+4` nog steeds (standaardapparaat, kanalen 3 en 4).
    pub fn audio_route(&self, output: &str) -> Result<AudioRoute, String> {
        if let Some(o) = self.audio.outputs.iter().find(|o| o.name == output) {
            return Ok(AudioRoute {
                device: o.device.clone(),
                left: usize::from(o.left.max(1)) - 1,
                right: usize::from(o.right.max(1)) - 1,
            });
        }
        let name = output.trim().to_lowercase();
        if name.is_empty() || name.starts_with("audio") {
            let pair = showmate_audio::parse_output_pair(output);
            return Ok(AudioRoute {
                device: None,
                left: pair * 2,
                right: pair * 2 + 1,
            });
        }
        Err(t!(
            "audio-uitgang '{output}' bestaat niet",
            "audio output '{output}' does not exist"
        ))
    }

    /// De monitoren van een video-uitgang (zonder uitgang met die naam: oude namen als `Display 1+2`).
    pub fn video_monitors(&self, output: &str) -> Vec<usize> {
        match self.video.outputs.iter().find(|o| o.name == output) {
            Some(o) if !o.monitors.is_empty() => o.monitors.clone(),
            Some(_) => vec![0],
            None => showmate_video::parse_displays(output),
        }
    }

    /// Zet instellingen uit oudere versies om naar het huidige model.
    pub fn normalize(&mut self) {
        // Eén apparaat voor alles → alle uitgangen zonder eigen apparaat gebruiken dat apparaat.
        if let Some(device) = self.audio.device.take() {
            for o in self.audio.outputs.iter_mut().filter(|o| o.device.is_none()) {
                o.device = Some(device.clone());
            }
        }
        // Kanalen beginnen bij 1.
        for o in &mut self.audio.outputs {
            o.left = o.left.max(1);
            o.right = o.right.max(1);
        }
    }
}

pub fn load(dir: &Path) -> (Settings, Option<String>) {
    let path = dir.join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(mut s) => {
                Settings::normalize(&mut s);
                (s, None)
            }
            Err(e) => (
                Settings::default(),
                Some(format!("settings.json genegeerd: {e}")),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
        Err(e) => (
            Settings::default(),
            Some(format!("settings.json onleesbaar: {e}")),
        ),
    }
}

/// Schrijft `settings.json` atomair (tijdelijk bestand, dan hernoemen).
pub fn save(dir: &Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    if !dir.as_os_str().is_empty() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let path = dir.join("settings.json");
    let tmp = dir.join("settings.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

impl VideoSettings {
    /// Vertaalt naar de videolaag; omgevingsvariabelen winnen van het bestand.
    pub fn to_config(&self) -> VideoConfig {
        let mut extra = self.extra_args.clone();
        if let Ok(args) = std::env::var("SHOWMATE_MPV_ARGS") {
            extra.extend(args.split_whitespace().map(str::to_string));
        }
        VideoConfig {
            mpv_path: self.mpv_path.clone(),
            fullscreen: self.fullscreen
                || std::env::var("SHOWMATE_FULLSCREEN").is_ok_and(|v| v == "1"),
            // De monitor staat in de uitgang zelf; hier geldt een gelijke afbeelding (index = monitor).
            screens: Vec::new(),
            extra_args: extra,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("showmate-settings-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_partial_and_empty_settings() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.video.fullscreen);
        assert_eq!(s.osc.destinations.len(), 1, "standaardbestemming Lighting");
        assert_eq!(s.midi.output, None);
        let s: Settings =
            serde_json::from_str(r#"{"video":{"fullscreen":true,"screens":[1,2]}}"#).unwrap();
        assert!(s.video.fullscreen);
        assert_eq!(s.video.screens, vec![1, 2]);
    }

    #[test]
    fn routes_resolve_by_name_with_legacy_fallback() {
        let mut s = Settings::default();
        s.audio.outputs.push(AudioOutputDef {
            name: "Zaal".into(),
            device: Some("USB".into()),
            left: 5,
            right: 8,
        });
        let r = s.audio_route("Zaal").unwrap();
        assert_eq!((r.device.as_deref(), r.left, r.right), (Some("USB"), 4, 7));
        let r = s.audio_route("Audio 3+4").unwrap();
        assert_eq!((r.device, r.left, r.right), (None, 2, 3));
        // oude naam zonder bijbehorende uitgang blijft werken
        s.audio.outputs.clear();
        let r = s.audio_route("Audio 5+6").unwrap();
        assert_eq!((r.left, r.right), (4, 5));
        assert!(s.audio_route("Podium").unwrap_err().contains("Podium"));
        assert_eq!(s.audio_route("").unwrap().left, 0);
    }

    #[test]
    fn video_monitors_resolve_by_name_with_legacy_fallback() {
        let mut s = Settings::default();
        s.video.outputs.push(VideoOutputDef {
            name: "Projector".into(),
            monitors: vec![2],
        });
        s.video.outputs.push(VideoOutputDef {
            name: "Leeg".into(),
            monitors: vec![],
        });
        assert_eq!(s.video_monitors("Projector"), vec![2]);
        assert_eq!(s.video_monitors("Display 1+2"), vec![0, 1]);
        assert_eq!(
            s.video_monitors("Leeg"),
            vec![0],
            "zonder monitor: de eerste"
        );
        s.video.outputs.clear();
        assert_eq!(
            s.video_monitors("Display 2"),
            vec![1],
            "oude naam zonder uitgang"
        );
        assert_eq!(s.video_monitors("onbekend"), vec![0]);
    }

    #[test]
    fn default_outputs_match_the_old_names() {
        let s = Settings::default();
        let names: Vec<&str> = s.audio.outputs.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Audio 1+2", "Audio 3+4"]);
        assert_eq!((s.audio.outputs[1].left, s.audio.outputs[1].right), (3, 4));
        let v: Vec<(&str, &[usize])> = s
            .video
            .outputs
            .iter()
            .map(|o| (o.name.as_str(), o.monitors.as_slice()))
            .collect();
        assert_eq!(
            v,
            [
                ("Display 1", &[0][..]),
                ("Display 2", &[1][..]),
                ("Display 1+2", &[0, 1][..])
            ]
        );
    }

    #[test]
    fn old_single_device_setting_moves_to_the_outputs() {
        let dir = tmp("legacy-audio");
        std::fs::write(
            dir.join("settings.json"),
            r#"{"audio":{"device":"Mijn kaart"}}"#,
        )
        .unwrap();
        let (s, msg) = load(&dir);
        assert!(msg.is_none());
        assert!(s.audio.device.is_none());
        assert!(
            s.audio
                .outputs
                .iter()
                .all(|o| o.device.as_deref() == Some("Mijn kaart"))
        );
        // opgeslagen bestand bevat het oude veld niet meer
        save(&dir, &s).unwrap();
        let text = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!text.contains("\"device\": \"Mijn kaart\"\n  },"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn custom_outputs_round_trip_and_empty_list_stays_empty() {
        let dir = tmp("outputs");
        let mut s = Settings::default();
        s.audio.outputs = vec![AudioOutputDef {
            name: "Zaal".into(),
            device: Some("USB-kaart".into()),
            left: 5,
            right: 6,
        }];
        s.video.outputs = vec![VideoOutputDef {
            name: "Projector".into(),
            monitors: vec![1],
        }];
        save(&dir, &s).unwrap();
        let (back, _) = load(&dir);
        assert_eq!(back.audio.outputs, s.audio.outputs);
        assert_eq!(back.video.outputs, s.video.outputs);
        let t: Settings =
            serde_json::from_str(r#"{"audio":{"outputs":[]},"video":{"outputs":[]}}"#).unwrap();
        assert!(t.audio.outputs.is_empty() && t.video.outputs.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recent_list_is_unique_newest_first_and_capped() {
        let mut r = Vec::new();
        for i in 0..12 {
            push_recent(&mut r, Path::new(&format!("s{i}.showmate")));
        }
        assert_eq!(r.len(), 8);
        assert_eq!(r[0], Path::new("s11.showmate"));
        push_recent(&mut r, Path::new("s5.showmate"));
        assert_eq!(r[0], Path::new("s5.showmate"));
        assert_eq!(
            r.iter().filter(|p| *p == Path::new("s5.showmate")).count(),
            1
        );
        assert_eq!(r.len(), 8);
    }

    #[test]
    fn new_fields_default_for_old_settings_files() {
        let s: Settings = serde_json::from_str(r#"{"language":"en"}"#).unwrap();
        assert!(s.copy_media, "kopiëren staat standaard aan");
        assert!(s.recent.is_empty() && s.last_show.is_none());
    }

    #[test]
    fn shortcuts_round_trip() {
        let dir = tmp("shortcuts");
        let mut s = Settings::default();
        s.shortcuts.insert("go".into(), "Enter".into());
        s.shortcuts.insert("delete".into(), String::new());
        save(&dir, &s).unwrap();
        let (back, msg) = load(&dir);
        assert!(msg.is_none());
        assert_eq!(back.shortcuts, s.shortcuts);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn explicit_empty_destination_list_stays_empty() {
        let s: Settings = serde_json::from_str(r#"{"osc":{"destinations":[]}}"#).unwrap();
        assert!(s.osc.destinations.is_empty());
    }

    #[test]
    fn missing_file_is_silent_and_broken_file_reports() {
        let dir = tmp("load");
        let (_, msg) = load(&dir);
        assert!(msg.is_none());
        std::fs::write(dir.join("settings.json"), "{kapot").unwrap();
        let (_, msg) = load(&dir);
        assert!(msg.unwrap().contains("settings.json"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tmp("roundtrip");
        let mut s = Settings::default();
        s.osc.destinations[0].host = "192.168.1.50".into();
        s.osc.commands.push(OscCommand {
            name: "Spots op".into(),
            address: "/eos/cue/1/fire".into(),
            args: "f:1".into(),
        });
        s.midi.output = Some("Port A".into());
        save(&dir, &s).unwrap();
        let (back, msg) = load(&dir);
        assert!(msg.is_none());
        assert_eq!(back.osc.destinations, s.osc.destinations);
        assert_eq!(back.osc.commands, s.osc.commands);
        assert_eq!(back.midi.output.as_deref(), Some("Port A"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
