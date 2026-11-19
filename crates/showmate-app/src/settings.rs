//! Programma-instellingen uit `settings.json` naast de showfile. Alles is optioneel.
//!
//! ```json
//! { "video": { "fullscreen": true, "screens": [1, 2], "mpv_path": "C:\\mpv\\mpv.exe", "extra_args": ["--hwdec=no"] } }
//! ```
//! `screens[n]` is de monitor-index (0 = eerste monitor) voor Display n+1.

use serde::Deserialize;
use showmate_video::VideoConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub video: VideoSettings,
}

#[derive(Debug, Default, Deserialize)]
pub struct VideoSettings {
    #[serde(default)]
    pub fullscreen: bool,
    pub mpv_path: Option<PathBuf>,
    #[serde(default)]
    pub screens: Vec<usize>,
    #[serde(default)]
    pub extra_args: Vec<String>,
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

    #[test]
    fn parses_partial_and_empty_settings() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(!s.video.fullscreen);
        let s: Settings =
            serde_json::from_str(r#"{"video":{"fullscreen":true,"screens":[1,2]}}"#).unwrap();
        assert!(s.video.fullscreen);
        assert_eq!(s.video.screens, vec![1, 2]);
    }

    #[test]
    fn missing_file_is_silent_and_broken_file_reports() {
        let dir = std::env::temp_dir().join(format!("showmate-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (_, msg) = load(&dir);
        assert!(msg.is_none());
        std::fs::write(dir.join("settings.json"), "{kapot").unwrap();
        let (_, msg) = load(&dir);
        assert!(msg.unwrap().contains("settings.json"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
