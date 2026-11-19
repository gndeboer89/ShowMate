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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoSettings {
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mpv_path: Option<PathBuf>,
    #[serde(default)]
    pub screens: Vec<usize>,
    #[serde(default)]
    pub extra_args: Vec<String>,
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

/// Audio-uitgang.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Naam van het uitvoerapparaat; `None` = standaardapparaat van het systeem.
    #[serde(default)]
    pub device: Option<String>,
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
pub fn load(dir: &Path) -> (Settings, Option<String>) {
    let path = dir.join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(s) => (s, None),
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
            screens: self.screens.clone(),
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
