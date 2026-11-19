use crate::audio::VideoAudio;
use crate::player::{Player, PlayerConfig, PlayerEvent};
use crate::protocol::MpvEvent;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug, Clone, Default)]
pub struct VideoConfig {
    /// Pad naar mpv; anders `SHOWMATE_MPV`, `mpv\mpv.exe` naast de app, of `mpv` op het PATH.
    pub mpv_path: Option<PathBuf>,
    /// Volledig scherm op de monitor van het display. Standaard een gewoon venster.
    pub fullscreen: bool,
    /// Monitor-index per display (display 1 = index 0). Ontbrekende waarden: gelijk aan het displaynummer.
    pub screens: Vec<usize>,
    /// Extra mpv-opties, bv. `--vo=x11` of `--hwdec=no`.
    pub extra_args: Vec<String>,
}

/// Gebeurtenissen voor de rest van de app. `cue` is de id die aan `start` is meegegeven.
#[derive(Debug, Clone, PartialEq)]
pub enum VideoEvent {
    /// Het bestand is afgelopen op alle schermen waar de cue speelde.
    Ended { cue: u64 },
    /// Lengte van het bestand in seconden.
    Duration { cue: u64, secs: f64 },
    /// Iets ging mis (geen mpv, bestand niet afspeelbaar, speler gecrasht).
    /// `cue` is gezet als een lopende cue erdoor is afgebroken.
    Error { cue: Option<u64>, message: String },
}

/// Zoekt mpv: ingestelde pad, `SHOWMATE_MPV`, naast de app, daarna het PATH.
pub fn locate_mpv(configured: Option<&Path>) -> Result<PathBuf, String> {
    let exe = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(configured.map(Path::to_path_buf));
    candidates.extend(std::env::var_os("SHOWMATE_MPV").map(PathBuf::from));
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        candidates.push(dir.join("mpv").join(exe));
        candidates.push(dir.join(exe));
    }
    if let Some(found) = candidates.iter().find(|c| c.is_file()) {
        return Ok(found.clone());
    }
    // Tot slot het PATH: werkt als `mpv --version` start.
    let on_path = Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if on_path {
        return Ok(PathBuf::from(exe));
    }
    Err(format!(
        "mpv niet gevonden; zet {exe} in de map 'mpv' naast ShowMate of stel SHOWMATE_MPV in"
    ))
}

pub struct VideoOutput {
    cfg: VideoConfig,
    mpv: Result<PathBuf, String>,
    players: HashMap<usize, Player>,
    /// Welke cue er nu op welk display speelt.
    playing: HashMap<usize, u64>,
    tx: Sender<(usize, PlayerEvent)>,
    rx: Receiver<(usize, PlayerEvent)>,
}

impl VideoOutput {
    pub fn new(cfg: VideoConfig) -> Self {
        let mpv = locate_mpv(cfg.mpv_path.as_deref());
        let (tx, rx) = channel();
        Self {
            cfg,
            mpv,
            players: HashMap::new(),
            playing: HashMap::new(),
            tx,
            rx,
        }
    }

    /// `Ok(pad)` of waarom mpv ontbreekt.
    pub fn mpv(&self) -> Result<&Path, &str> {
        self.mpv.as_deref().map_err(String::as_str)
    }

    fn ensure_player(&mut self, display: usize) -> Result<&Player, String> {
        if self.players.get(&display).is_some_and(Player::is_broken) {
            self.players.remove(&display);
        }
        if !self.players.contains_key(&display) {
            let mpv = self.mpv.clone()?;
            let cfg = PlayerConfig {
                mpv,
                display,
                fullscreen: self.cfg.fullscreen,
                screen: self.cfg.screens.get(display).copied().unwrap_or(display),
                extra_args: self.cfg.extra_args.clone(),
            };
            let player = Player::spawn(&cfg, self.tx.clone())?;
            self.players.insert(display, player);
        }
        Ok(&self.players[&display])
    }

    /// Start vast een (nog onzichtbaar) mpv-proces zodat het eerste GO sneller gaat.
    pub fn warm(&mut self, displays: &[usize]) -> Result<(), String> {
        for d in displays {
            self.ensure_player(*d)?;
        }
        Ok(())
    }

    /// Speelt `path` op alle genoemde schermen; een lopende cue op dat scherm wordt vervangen.
    /// Bij meerdere schermen komt het geluid alleen van het eerste (anders speelt het dubbel).
    pub fn start(
        &mut self,
        cue: u64,
        displays: &[usize],
        path: &Path,
        looping: bool,
        audio: VideoAudio,
    ) -> Result<(), String> {
        for (n, &d) in displays.iter().enumerate() {
            let audio = VideoAudio {
                mute: audio.mute || n > 0,
                ..audio
            };
            self.ensure_player(d)?.load(path, looping, audio);
            self.playing.insert(d, cue);
        }
        Ok(())
    }

    /// Stopt `cue` op de schermen waar hij nog speelt; het scherm blijft zwart.
    pub fn stop_cue(&mut self, cue: u64) {
        let displays: Vec<usize> = self
            .playing
            .iter()
            .filter(|(_, c)| **c == cue)
            .map(|(d, _)| *d)
            .collect();
        for d in displays {
            self.playing.remove(&d);
            if let Some(p) = self.players.get(&d) {
                p.stop();
            }
        }
    }

    /// Stopt alles en sluit de uitvoervensters.
    pub fn hide_all(&mut self) {
        self.playing.clear();
        for p in self.players.values() {
            p.hide();
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        for p in self.players.values() {
            p.set_paused(paused);
        }
    }

    pub fn is_playing(&self, cue: u64) -> bool {
        self.playing.values().any(|c| *c == cue)
    }

    /// Het eerste display waar `cue` speelt (voor de preview).
    pub fn display_of(&self, cue: u64) -> Option<usize> {
        self.playing
            .iter()
            .filter(|(_, c)| **c == cue)
            .map(|(d, _)| *d)
            .min()
    }

    /// Vraagt mpv om een stilstaand beeld van wat er nu op `display` te zien is.
    pub fn screenshot(&self, display: usize, file: &Path) {
        if let Some(p) = self.players.get(&display) {
            p.screenshot(file);
        }
    }

    /// Verwerkt events van de spelers. Elke tick aanroepen.
    pub fn poll(&mut self) -> Vec<VideoEvent> {
        let mut out = Vec::new();
        while let Ok((display, ev)) = self.rx.try_recv() {
            let cue = self.playing.get(&display).copied();
            match ev {
                PlayerEvent::Mpv(MpvEvent::Duration(secs)) => {
                    if let Some(cue) = cue {
                        out.push(VideoEvent::Duration { cue, secs });
                    }
                }
                PlayerEvent::Mpv(MpvEvent::FileLoaded) => {}
                PlayerEvent::Mpv(MpvEvent::EndOfFile { error }) => {
                    let Some(cue) = cue else { continue };
                    self.playing.remove(&display);
                    if error {
                        out.push(VideoEvent::Error {
                            cue: Some(cue),
                            message: "video kon niet worden afgespeeld".into(),
                        });
                    }
                    if !self.is_playing(cue) {
                        out.push(VideoEvent::Ended { cue });
                    }
                }
                PlayerEvent::Exited(why) => {
                    self.players.remove(&display);
                    let cue = self.playing.remove(&display);
                    out.push(VideoEvent::Error {
                        cue,
                        message: format!("videospeler {}: {why}", display + 1),
                    });
                    if let Some(cue) = cue.filter(|c| !self.is_playing(*c)) {
                        out.push(VideoEvent::Ended { cue });
                    }
                }
            }
        }
        out
    }
}
