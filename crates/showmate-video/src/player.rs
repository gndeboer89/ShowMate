//! Eén mpv-proces voor één scherm.
//!
//! Er zijn twee verbindingen: een schrijfverbinding voor commando's en een leesverbinding
//! voor events. Op Windows serialiseert een blokkerende lees op dezelfde pipe-handle het
//! schrijven, dus delen we ze bewust niet.

use crate::audio::{VideoAudio, volume_percent};
use crate::ipc::{self, Stream};
use crate::protocol::{MpvEvent, command_line, parse_event};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Mpv(MpvEvent),
    /// Het proces of de verbinding is weg.
    Exited(String),
}

#[derive(Debug, Clone)]
pub struct PlayerConfig {
    pub mpv: PathBuf,
    pub display: usize,
    pub fullscreen: bool,
    /// Monitor-index voor `--fs-screen` (0-gebaseerd).
    pub screen: usize,
    pub extra_args: Vec<String>,
}

#[derive(Default)]
struct Writer {
    stream: Option<Stream>,
    /// Commando's die gegeven zijn terwijl mpv nog opstartte.
    pending: Vec<String>,
    broken: bool,
}

pub struct Player {
    child: Child,
    writer: Arc<Mutex<Writer>>,
    endpoint: String,
}

impl Player {
    pub fn spawn(cfg: &PlayerConfig, events: Sender<(usize, PlayerEvent)>) -> Result<Self, String> {
        let endpoint = ipc::endpoint(std::process::id(), cfg.display);
        ipc::cleanup(&endpoint);

        let mut cmd = Command::new(&cfg.mpv);
        cmd.arg("--no-config")
            .arg("--idle=yes")
            .arg("--force-window=no")
            .arg("--keep-open=no")
            .arg("--msg-level=all=warn")
            .arg("--volume-max=200")
            .arg("--osc=no")
            .arg("--osd-level=0")
            .arg("--no-input-default-bindings")
            .arg("--input-vo-keyboard=no")
            .arg("--no-input-cursor")
            .arg("--cursor-autohide=always")
            .arg("--hwdec=auto-safe")
            .arg("--screenshot-format=jpeg")
            .arg("--screenshot-jpeg-quality=60")
            .arg(format!("--title=ShowMate Display {}", cfg.display + 1))
            .arg(format!("--input-ipc-server={endpoint}"));
        if cfg.fullscreen {
            cmd.arg("--fs")
                .arg(format!("--fs-screen={}", cfg.screen))
                .arg("--ontop");
        } else {
            let off = 40 + cfg.display * 60;
            cmd.arg("--autofit=960x540")
                .arg(format!("--geometry=+{off}+{off}"));
        }
        cmd.args(&cfg.extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: geen consolevenster
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("kan {} niet starten: {e}", cfg.mpv.display()))?;

        // mpv's foutmeldingen opvangen (en leeghalen, anders loopt de pipe vol): daarmee kunnen we
        // uitleggen *waarom* er geen verbinding komt (bv. een optie die deze mpv-versie niet kent).
        let diag = Arc::new(Diag::default());
        if let Some(stderr) = child.stderr.take() {
            let diag = diag.clone();
            std::thread::spawn(move || diag.drain(stderr));
        }

        let writer = Arc::new(Mutex::new(Writer::default()));
        let (w, ep, display) = (writer.clone(), endpoint.clone(), cfg.display);
        std::thread::spawn(move || connect_and_pump(&ep, display, &w, &events, &diag));
        Ok(Self {
            child,
            writer,
            endpoint,
        })
    }

    /// Stuurt een commando; tijdens het opstarten wordt het bewaard en later verstuurd.
    pub fn send(&self, args: &[Value]) {
        let line = command_line(args);
        let mut w = self.writer.lock().expect("writer-lock");
        if w.broken {
            return;
        }
        match w.stream.as_mut() {
            Some(s) => {
                if s.write_all(line.as_bytes()).is_err() {
                    w.broken = true;
                    w.stream = None;
                }
            }
            None => w.pending.push(line),
        }
    }

    pub fn is_broken(&self) -> bool {
        self.writer.lock().expect("writer-lock").broken
    }

    /// Laadt en start een bestand.
    pub fn load(&self, path: &Path, looping: bool, audio: VideoAudio) {
        self.send(&[json!("set_property"), json!("mute"), json!(audio.mute)]);
        self.send(&[
            json!("set_property"),
            json!("volume"),
            json!(volume_percent(audio.gain_db)),
        ]);
        self.send(&[
            json!("set_property"),
            json!("loop-file"),
            json!(if looping { "inf" } else { "no" }),
        ]);
        self.send(&[json!("set_property"), json!("pause"), json!(false)]);
        self.send(&[json!("set_property"), json!("force-window"), json!("yes")]);
        self.send(&[
            json!("loadfile"),
            json!(path.to_string_lossy()),
            json!("replace"),
        ]);
    }

    /// Stopt het afspelen; het venster blijft zwart staan.
    pub fn stop(&self) {
        self.send(&[json!("stop")]);
    }

    /// Stopt en sluit het venster.
    pub fn hide(&self) {
        self.send(&[json!("set_property"), json!("force-window"), json!("no")]);
        self.send(&[json!("stop")]);
    }

    pub fn set_paused(&self, paused: bool) {
        self.send(&[json!("set_property"), json!("pause"), json!(paused)]);
    }

    pub fn screenshot(&self, file: &Path) {
        self.send(&[
            json!("screenshot-to-file"),
            json!(file.to_string_lossy()),
            json!("video"),
        ]);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.send(&[json!("quit")]);
        let _ = self.child.kill();
        let _ = self.child.wait();
        ipc::cleanup(&self.endpoint);
    }
}

/// Wat mpv op stderr meldt, en of het proces al is gestopt.
#[derive(Default)]
struct Diag {
    text: Mutex<String>,
    exited: AtomicBool,
}

impl Diag {
    fn drain(&self, stderr: impl std::io::Read) {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut t = self.text.lock().expect("diag-lock");
            if t.len() < 1500 {
                t.push_str(line.trim());
                t.push('\n');
            }
        }
        self.exited.store(true, Ordering::SeqCst);
    }

    fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    /// Toelichting voor in een foutmelding.
    fn hint(&self) -> String {
        let t = self.text.lock().expect("diag-lock");
        let lines: Vec<&str> = t.lines().filter(|l| !l.is_empty()).take(3).collect();
        match (lines.is_empty(), self.has_exited()) {
            (false, _) => format!(" — mpv meldt: {}", lines.join(" | ")),
            (true, true) => " — mpv is direct gestopt (zonder melding)".into(),
            (true, false) => String::new(),
        }
    }
}

fn connect_and_pump(
    endpoint: &str,
    display: usize,
    writer: &Mutex<Writer>,
    events: &Sender<(usize, PlayerEvent)>,
    diag: &Diag,
) {
    let exited = |why: String| {
        let _ = events.send((display, PlayerEvent::Exited(why)));
    };
    let give_up = || diag.has_exited();
    let (write_stream, mut read_stream) = match (
        ipc::connect(endpoint, CONNECT_TIMEOUT, give_up),
        ipc::connect(endpoint, CONNECT_TIMEOUT, give_up),
    ) {
        (Ok(w), Ok(r)) => (w, r),
        (Err(e), _) | (_, Err(e)) => {
            writer.lock().expect("writer-lock").broken = true;
            // Even wachten zodat de laatste stderr-regels binnen zijn.
            std::thread::sleep(Duration::from_millis(150));
            return exited(format!("geen verbinding met mpv: {e}{}", diag.hint()));
        }
    };

    // Abonneren vóór het blokkerende lezen begint (zie module-uitleg).
    let subscribe = command_line(&[json!("observe_property"), json!(1), json!("duration")]);
    if read_stream.write_all(subscribe.as_bytes()).is_err() {
        writer.lock().expect("writer-lock").broken = true;
        return exited("verbinding met mpv verbroken".into());
    }

    {
        let mut w = writer.lock().expect("writer-lock");
        let mut stream = write_stream;
        for line in std::mem::take(&mut w.pending) {
            if stream.write_all(line.as_bytes()).is_err() {
                w.broken = true;
                drop(w);
                return exited("verbinding met mpv verbroken".into());
            }
        }
        w.stream = Some(stream);
    }

    for line in BufReader::new(read_stream).lines() {
        let Ok(line) = line else { break };
        if let Some(ev) = parse_event(&line) {
            let _ = events.send((display, PlayerEvent::Mpv(ev)));
        }
    }
    writer.lock().expect("writer-lock").broken = true;
    exited("mpv is gestopt".into());
}
