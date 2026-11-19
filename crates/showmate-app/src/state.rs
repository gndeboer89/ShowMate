//! UI-staat naast de engine: selectie, bestandspad, rij-model en inspector-vulling.

mod io;

use crate::settings::Settings;
use crate::{CueRow, MainWindow};
use showmate_audio::{
    AudioClip, AudioOutput, ClipCache, ClipStatus, VoiceParams, parse_output_pair,
};
use showmate_core::{
    Cue, CueId, CueKind, CueState, EditError, Engine, Event, Show, StartMode, SystemClock,
    demo::demo_show, format_delay, format_duration, format_timecode, parse_db, parse_duration,
};
use showmate_io::{MidiOut, OscSender};
use showmate_video::{VideoAudio, VideoEvent, VideoOutput, parse_displays};
use slint::{Model, ModelRc, VecModel};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const FPS: u32 = 25;

pub struct AppState {
    pub engine: Engine<SystemClock>,
    selected: Option<usize>,
    path: PathBuf,
    dirty: bool,
    message: String,
    delay_ok: bool,
    duration_ok: bool,
    rows: Rc<VecModel<CueRow>>,
    fill_inspector: bool,
    volume_ok: bool,
    fade_in_ok: bool,
    fade_out_ok: bool,
    audio: Option<AudioOutput>,
    audio_info: String,
    cache: ClipCache,
    meter: (f32, f32),
    wave_cue: Option<CueId>,
    video: VideoOutput,
    video_info: String,
    video_durations: HashMap<CueId, Duration>,
    preview_ticks: u32,
    preview_slot: usize,
    preview_shown: bool,
    settings: Settings,
    settings_dir: PathBuf,
    osc: OscSender,
    midi: Option<MidiOut>,
    midi_ports: Vec<String>,
    midi_status: String,
    midi_list_error: String,
    midi_ok: bool,
    osc_args_error: String,
    settings_open: bool,
    settings_note: String,
    /// Instellingenlijsten en keuzelijsten opnieuw vullen (met/zonder rijen te vervangen).
    fill_settings: bool,
    fill_names: bool,
}

/// Fallback-samplerate voor de cache als er geen audio-apparaat is.
const FALLBACK_RATE: u32 = 48_000;

/// `SHOWMATE_FILE`, anders `%APPDATA%\ShowMate\show.showmate`, anders `./show.showmate`.
fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SHOWMATE_FILE") {
        return p.into();
    }
    if let Some(dir) = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME")) {
        return PathBuf::from(dir).join("ShowMate").join("show.showmate");
    }
    PathBuf::from("show.showmate")
}

fn kind_label(kind: CueKind) -> &'static str {
    match kind {
        CueKind::Audio => "Audio",
        CueKind::Video => "Video",
        CueKind::Osc => "OSC",
        CueKind::Midi => "MIDI",
    }
}

fn status_label(state: CueState) -> &'static str {
    match state {
        CueState::Waiting | CueState::Scheduled { .. } => "Wachtend",
        CueState::Active { .. } => "Actief",
        CueState::Finished => "Gespeeld",
        CueState::Stopped => "Gestopt",
    }
}

/// Seconden zonder overbodige nullen: `5`, `5.5`; vanaf een minuut hele secs als `mm:ss`.
fn plain_secs(d: Duration) -> String {
    if d.as_secs() >= 60 && d.subsec_millis() == 0 {
        format_duration(d)
    } else {
        format!("{}", (d.as_secs_f64() * 1000.0).round() / 1000.0)
    }
}

impl AppState {
    pub fn open_default() -> Self {
        let path = default_path();
        let mut message = String::new();
        let show = match Show::load(&path) {
            Ok(show) => show,
            Err(showmate_core::ShowError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                demo_show()
            }
            Err(e) => {
                message = format!("Laden mislukt, demoshow geopend: {e}");
                demo_show()
            }
        };
        let (audio, audio_info) = match AudioOutput::open_default() {
            Ok(a) => {
                let info = format!(
                    "{} ({} Hz, {} kan.)",
                    a.device_name(),
                    a.sample_rate(),
                    a.channels()
                );
                (Some(a), info)
            }
            Err(e) => (None, format!("geen audio: {e}")),
        };
        let settings_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let (settings, settings_msg) = crate::settings::load(&settings_dir);
        let mut video = VideoOutput::new(settings.video.to_config());
        let video_info = match video.mpv() {
            Ok(p) => format!("mpv: {}", p.display()),
            Err(e) => e.to_string(),
        };
        let mut warm: Vec<usize> = show
            .cues
            .iter()
            .filter(|c| c.kind == CueKind::Video)
            .flat_map(|c| parse_displays(&c.output))
            .collect();
        warm.sort_unstable();
        warm.dedup();
        if let Err(e) = video.warm(&warm)
            && video.mpv().is_ok()
        {
            message = e;
        }
        if let Some(m) = settings_msg {
            message = m;
        }
        let cache = ClipCache::new(
            audio
                .as_ref()
                .map_or(FALLBACK_RATE, AudioOutput::sample_rate),
        );
        let mut state = Self {
            engine: Engine::new(SystemClock::new(), show),
            selected: None,
            path,
            dirty: false,
            message,
            delay_ok: true,
            duration_ok: true,
            rows: Rc::new(VecModel::default()),
            fill_inspector: false,
            volume_ok: true,
            fade_in_ok: true,
            fade_out_ok: true,
            audio,
            audio_info,
            cache,
            meter: (0.0, 0.0),
            wave_cue: None,
            video,
            video_info,
            video_durations: HashMap::new(),
            preview_ticks: 0,
            preview_slot: 0,
            preview_shown: false,
            settings,
            settings_dir,
            osc: OscSender::new(),
            midi: None,
            midi_ports: Vec::new(),
            midi_status: String::new(),
            midi_list_error: String::new(),
            midi_ok: true,
            osc_args_error: String::new(),
            settings_open: false,
            settings_note: String::new(),
            fill_settings: false,
            fill_names: false,
        };
        state.preload_all();
        state.connect_midi();
        state
    }

    // ---- media -----------------------------------------------------

    /// Relatieve paden gelden t.o.v. de map van de showfile (ook de submap `media`).
    fn resolve_media(&self, media: &str) -> PathBuf {
        let p = Path::new(media);
        if p.is_absolute() {
            return p.to_path_buf();
        }
        let base = self
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let direct = base.join(p);
        if direct.exists() {
            direct
        } else {
            base.join("media").join(p)
        }
    }

    fn preload_all(&self) {
        for cue in self
            .engine
            .show()
            .cues
            .iter()
            .filter(|c| c.kind == CueKind::Audio)
        {
            if !cue.media.trim().is_empty() {
                self.cache.request(&self.resolve_media(&cue.media));
            }
        }
    }

    fn clip_for_kind(&self, cue: &Cue) -> Option<Arc<AudioClip>> {
        if cue.kind == CueKind::Audio && !cue.media.trim().is_empty() {
            self.clip_for(cue)
        } else {
            None
        }
    }

    fn clip_for(&self, cue: &Cue) -> Option<Arc<AudioClip>> {
        match self.cache.status(&self.resolve_media(&cue.media)) {
            ClipStatus::Ready(c) => Some(c),
            _ => None,
        }
    }

    // ---- transport ---------------------------------------------------

    pub fn go(&mut self) {
        let events = self.engine.go();
        self.handle_events(events);
    }

    pub fn toggle_pause(&mut self) {
        let events = self.engine.toggle_pause();
        self.handle_events(events);
    }

    pub fn stop(&mut self) {
        let events = self.engine.stop();
        self.handle_events(events);
        self.video.hide_all();
    }

    /// Vertaalt engine-events naar audio-acties.
    fn handle_events(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::CueStarted(id) => self.start_cue(id),
                Event::CueStopped(id) => {
                    if let Some(a) = self.audio.as_mut() {
                        let _ = a.stop(id.0);
                    }
                    self.video.stop_cue(id.0);
                }
                Event::Paused => {
                    if let Some(a) = self.audio.as_mut() {
                        let _ = a.pause();
                    }
                    self.video.set_paused(true);
                }
                Event::Resumed => {
                    if let Some(a) = self.audio.as_mut() {
                        let _ = a.resume();
                    }
                    self.video.set_paused(false);
                }
                // Een cue met een eigen (kortere) duur kapt het beeld af.
                Event::CueFinished(id) => self.video.stop_cue(id.0),
            }
        }
    }

    fn start_video(&mut self, id: CueId, index: usize, cue: &Cue) {
        let problem = if cue.media.trim().is_empty() {
            Some("geen bestand opgegeven".to_string())
        } else {
            // Streams en testbronnen (`av://…`, `http://…`) gaan ongewijzigd naar mpv.
            let path = if cue.media.contains("://") {
                PathBuf::from(&cue.media)
            } else {
                self.resolve_media(&cue.media)
            };
            if !cue.media.contains("://") && !path.is_file() {
                Some(format!("{} niet gevonden", cue.media))
            } else {
                let audio = VideoAudio {
                    mute: cue.mute_audio,
                    gain_db: cue.volume_db,
                };
                self.video
                    .start(
                        id.0,
                        &parse_displays(&cue.output),
                        &path,
                        cue.looping,
                        audio,
                    )
                    .err()
            }
        };
        if let Some(problem) = problem {
            self.message = format!("Cue {}: {problem}", index + 1);
            let events = self.engine.complete(id); // niet eindeloos "actief" laten staan
            self.handle_events(events);
        }
    }

    fn start_cue(&mut self, id: CueId) {
        let Some(index) = self.engine.show().cues.iter().position(|c| c.id == id) else {
            return;
        };
        let cue = self.engine.show().cues[index].clone();
        match cue.kind {
            CueKind::Audio => {}
            CueKind::Video => return self.start_video(id, index, &cue),
            CueKind::Osc | CueKind::Midi => {
                if let Err(e) = self.fire_io(&cue) {
                    self.message = format!("Cue {}: {e}", index + 1);
                }
                return;
            }
        }
        let problem = match (self.audio.as_mut(), cue.media.trim().is_empty()) {
            (None, _) => Some("geen audio-apparaat".to_string()),
            (_, true) => Some("geen bestand opgegeven".to_string()),
            (Some(_), false) => {
                let path = self.resolve_media(&cue.media);
                match self.cache.status(&path) {
                    ClipStatus::Ready(clip) => {
                        self.engine.set_media_duration(id, clip.duration());
                        let params = VoiceParams {
                            id: id.0,
                            clip,
                            gain_db: cue.volume_db,
                            fade_in: cue.fade_in,
                            fade_out: cue.fade_out,
                            looping: cue.looping,
                            max_duration: if cue.looping { None } else { cue.duration },
                            out_pair: parse_output_pair(&cue.output),
                        };
                        self.audio
                            .as_mut()
                            .and_then(|a| a.play(params).err())
                            .map(|e| e.to_string())
                    }
                    ClipStatus::Loading => Some(format!("{} wordt nog geladen", cue.media)),
                    ClipStatus::Failed(e) => Some(format!("{}: {e}", cue.media)),
                    ClipStatus::Unknown => {
                        self.cache.request(&path);
                        Some(format!("{} was nog niet geladen", cue.media))
                    }
                }
            }
        };
        if let Some(problem) = problem {
            // Voorkom een cue die eindeloos "actief" blijft zonder geluid.
            self.engine.set_media_duration(id, Duration::ZERO);
            self.message = format!("Cue {}: {problem}", index + 1);
        }
    }

    // ---- acties ----------------------------------------------------

    pub fn select(&mut self, index: i32) {
        let n = self.engine.show().cues.len();
        self.selected = usize::try_from(index).ok().filter(|i| *i < n);
        self.delay_ok = true;
        self.duration_ok = true;
        self.volume_ok = true;
        self.fade_in_ok = true;
        self.fade_out_ok = true;
        self.message.clear();
        self.fill_inspector = true;
    }

    pub fn add_cue(&mut self, kind: &str) {
        let kind = match kind {
            "video" => CueKind::Video,
            "osc" => CueKind::Osc,
            "midi" => CueKind::Midi,
            _ => CueKind::Audio,
        };
        match self.engine.add_cue(kind, self.selected) {
            Ok(i) => {
                self.selected = Some(i);
                self.mark_changed();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn remove_selected(&mut self) {
        let Some(i) = self.selected else { return };
        match self.engine.remove_cue(i) {
            Ok(_) => {
                let n = self.engine.show().cues.len();
                self.selected = if n == 0 { None } else { Some(i.min(n - 1)) };
                self.mark_changed();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn move_selected(&mut self, up: bool) {
        let Some(i) = self.selected else { return };
        match self.engine.move_cue(i, up) {
            Ok(new) => {
                if new != i {
                    self.selected = Some(new);
                    self.dirty = true;
                    self.message.clear();
                }
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn edit(&mut self, f: impl FnOnce(&mut Cue)) {
        let Some(i) = self.selected else { return };
        if self.engine.update_cue(i, f).is_ok() {
            self.dirty = true;
        }
    }

    pub fn edit_delay(&mut self, text: &str) {
        let parsed = if text.trim().is_empty() {
            Some(Duration::ZERO)
        } else {
            parse_duration(text)
        };
        self.delay_ok = parsed.is_some();
        if let Some(d) = parsed {
            self.edit(|c| c.delay = d);
        }
    }

    pub fn edit_duration(&mut self, text: &str) {
        let t = text.trim();
        let parsed = if t.is_empty() || t == "—" {
            Some(None)
        } else {
            parse_duration(t).map(Some)
        };
        self.duration_ok = parsed.is_some();
        if let Some(d) = parsed {
            self.edit(|c| c.duration = d);
        }
    }

    pub fn edit_media(&mut self, text: &str) {
        let text = text.to_string();
        if let Some(id) = self
            .selected
            .and_then(|i| self.engine.show().cues.get(i))
            .map(|c| c.id)
        {
            self.video_durations.remove(&id);
        }
        self.edit(|c| c.media = text.clone());
        if self.selected.is_some() && !text.trim().is_empty() {
            self.cache.request(&self.resolve_media(&text));
        }
    }

    pub fn edit_volume(&mut self, text: &str) {
        let parsed = parse_db(text);
        self.volume_ok = parsed.is_some();
        if let Some(db) = parsed {
            self.edit(|c| c.volume_db = db);
        }
    }

    pub fn edit_fade(&mut self, text: &str, fade_in: bool) {
        let parsed = if text.trim().is_empty() {
            Some(Duration::ZERO)
        } else {
            parse_duration(text)
        };
        if fade_in {
            self.fade_in_ok = parsed.is_some();
        } else {
            self.fade_out_ok = parsed.is_some();
        }
        if let Some(d) = parsed {
            self.edit(|c| {
                if fade_in {
                    c.fade_in = d
                } else {
                    c.fade_out = d
                }
            });
        }
    }

    pub fn save(&mut self) {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(dir);
        }
        match self.engine.show().save(&self.path) {
            Ok(()) => {
                self.dirty = false;
                self.message = format!("Opgeslagen: {}", self.path.display());
            }
            Err(e) => self.message = format!("Opslaan mislukt: {e}"),
        }
    }

    fn fail(&mut self, e: EditError) {
        self.message = e.to_string();
    }

    fn mark_changed(&mut self) {
        self.dirty = true;
        self.message.clear();
        self.fill_inspector = true;
    }

    // ---- UI-synchronisatie -------------------------------------------

    /// Na elke gebruikersactie.
    pub fn after_edit(&mut self, ui: &MainWindow) {
        self.refresh_live(ui);
        if std::mem::take(&mut self.fill_inspector) {
            self.fill_inspector(ui);
        }
        if std::mem::take(&mut self.fill_settings) {
            self.fill_settings_ui(ui, true);
            self.fill_names = false;
        } else if std::mem::take(&mut self.fill_names) {
            self.fill_settings_ui(ui, false);
        }
    }

    /// Elke timer-tick.
    pub fn tick(&mut self, ui: &MainWindow) {
        let events = self.engine.tick();
        self.handle_events(events);
        if let Some(a) = self.audio.as_mut() {
            a.drain_ended();
            let (l, r) = a.take_peaks();
            // Snelle attack, rustige release.
            self.meter = (l.max(self.meter.0 * 0.88), r.max(self.meter.1 * 0.88));
        }
        self.poll_video();
        self.refresh_live(ui);
        self.refresh_preview(ui);
    }

    fn poll_video(&mut self) {
        for event in self.video.poll() {
            match event {
                VideoEvent::Duration { cue, secs } => {
                    let id = CueId(cue);
                    let len = Duration::from_secs_f64(secs);
                    self.video_durations.insert(id, len);
                    // Vangnet: eindigt de cue anders niet, dan na de bestandsduur plus marge.
                    self.engine
                        .set_media_duration(id, len + Duration::from_secs(1));
                }
                VideoEvent::Ended { cue } => {
                    let id = CueId(cue);
                    let own_end = self
                        .engine
                        .show()
                        .cues
                        .iter()
                        .find(|c| c.id == id)
                        .is_some_and(|c| c.duration.is_some() || c.looping);
                    if !own_end {
                        let events = self.engine.complete(id);
                        self.handle_events(events);
                    }
                }
                VideoEvent::Error { message, .. } => self.message = message,
            }
        }
    }

    pub fn refresh_all(&mut self, ui: &MainWindow) {
        ui.set_rows(ModelRc::from(self.rows.clone()));
        self.refresh_live(ui);
        self.fill_inspector(ui);
        self.fill_settings_ui(ui, true);
    }

    fn refresh_live(&mut self, ui: &MainWindow) {
        self.refresh_wave(ui);
        let io_labels: Vec<Option<(String, String)>> = self
            .engine
            .show()
            .cues
            .iter()
            .map(|c| self.io_labels(c))
            .collect();
        let show = self.engine.show();
        let rows: Vec<CueRow> = show
            .cues
            .iter()
            .zip(self.engine.states())
            .enumerate()
            .map(|(i, (cue, state))| CueRow {
                number: i as i32 + 1,
                name: cue.name.as_str().into(),
                kind: kind_label(cue.kind).into(),
                media: io_labels
                    .get(i)
                    .and_then(|l| l.as_ref())
                    .map_or(cue.media.as_str(), |l| l.0.as_str())
                    .into(),
                start: match cue.start {
                    StartMode::Go => "GO",
                    StartMode::Auto => "Auto",
                }
                .into(),
                delay: format_delay(cue.delay).into(),
                duration: match (cue.looping, cue.duration) {
                    (true, _) => "Loop".into(),
                    (false, Some(d)) => format_duration(d).into(),
                    (false, None) => {
                        match (self.clip_for_kind(cue), self.video_durations.get(&cue.id)) {
                            (Some(clip), _) => format_duration(clip.duration()).into(),
                            (None, Some(len)) => format_duration(*len).into(),
                            (None, None) => "—".into(),
                        }
                    }
                },
                output: io_labels
                    .get(i)
                    .and_then(|l| l.as_ref())
                    .map_or(cue.output.as_str(), |l| l.1.as_str())
                    .into(),
                status: status_label(*state).into(),
                active: matches!(state, CueState::Active { .. }),
                done: matches!(state, CueState::Finished),
            })
            .collect();

        // In-place bijwerken: alleen gewijzigde rijen aanraken.
        if self.rows.row_count() != rows.len() {
            self.rows.set_vec(rows);
        } else {
            for (i, row) in rows.into_iter().enumerate() {
                if self.rows.row_data(i).as_ref() != Some(&row) {
                    self.rows.set_row_data(i, row);
                }
            }
        }

        ui.set_selected(self.selected.map_or(-1, |i| i as i32));
        ui.set_has_selection(self.selected.is_some());
        ui.set_timecode(format_timecode(self.engine.show_time(), FPS).into());
        ui.set_paused(self.engine.is_paused());
        ui.set_volume_ok(self.volume_ok);
        ui.set_fade_in_ok(self.fade_in_ok);
        ui.set_fade_out_ok(self.fade_out_ok);
        ui.set_audio_info(self.audio_info.as_str().into());
        ui.set_video_info(
            if self.video.mpv().is_err() {
                self.video_info.as_str()
            } else {
                ""
            }
            .into(),
        );
        ui.set_meter_l(meter_fraction(self.meter.0));
        ui.set_meter_r(meter_fraction(self.meter.1));
        ui.set_delay_ok(self.delay_ok);
        ui.set_duration_ok(self.duration_ok);
        ui.set_message(self.message.as_str().into());
        ui.set_window_title(
            format!(
                "ShowMate — {}{}",
                show.name,
                if self.dirty { " *" } else { "" }
            )
            .into(),
        );

        // Lijst bewerken kan niet zolang cues gepland staan.
        ui.set_locked(
            self.engine
                .states()
                .iter()
                .any(|s| matches!(s, CueState::Scheduled { .. })),
        );

        match self.engine.next_cue() {
            Some(next) => {
                ui.set_next_cue(
                    format!("{} • {}", next.index + 1, show.cues[next.index].name).into(),
                );
                ui.set_next_eta(
                    match next.starts_in {
                        Some(d) => format!(
                            "(over {})",
                            format_timecode(d, FPS).split('.').next().unwrap_or("")
                        ),
                        None => "(wacht op GO)".to_string(),
                    }
                    .into(),
                );
            }
            None => {
                ui.set_next_cue("—".into());
                ui.set_next_eta("".into());
            }
        }
    }

    fn fill_inspector(&self, ui: &MainWindow) {
        let cue = self
            .selected
            .and_then(|i| self.engine.show().cues.get(i).map(|c| (i, c)));
        match cue {
            Some((i, c)) => {
                ui.set_insp_title(
                    format!("{} • {} ({})", i + 1, c.name, kind_label(c.kind)).into(),
                );
                ui.set_insp_name(c.name.as_str().into());
                ui.set_insp_media(c.media.as_str().into());
                ui.set_insp_output(c.output.as_str().into());
                ui.set_insp_notes(c.notes.as_str().into());
                ui.set_insp_start_index(match c.start {
                    StartMode::Go => 0,
                    StartMode::Auto => 1,
                });
                ui.set_insp_delay(plain_secs(c.delay).into());
                ui.set_insp_duration(c.duration.map(plain_secs).unwrap_or_default().into());
                ui.set_insp_looping(c.looping);
                self.fill_io_inspector(ui, c);
                ui.set_insp_volume(
                    if c.volume_db == 0.0 {
                        String::new()
                    } else {
                        format!("{}", c.volume_db)
                    }
                    .into(),
                );
                ui.set_insp_fade_in(
                    if c.fade_in.is_zero() {
                        String::new()
                    } else {
                        plain_secs(c.fade_in)
                    }
                    .into(),
                );
                ui.set_insp_fade_out(
                    if c.fade_out.is_zero() {
                        String::new()
                    } else {
                        plain_secs(c.fade_out)
                    }
                    .into(),
                );
            }
            None => {
                for set in [
                    MainWindow::set_insp_name,
                    MainWindow::set_insp_media,
                    MainWindow::set_insp_output,
                    MainWindow::set_insp_notes,
                    MainWindow::set_insp_delay,
                    MainWindow::set_insp_duration,
                    MainWindow::set_insp_volume,
                    MainWindow::set_insp_fade_in,
                    MainWindow::set_insp_fade_out,
                ] {
                    set(ui, "".into());
                }
                ui.set_insp_title("".into());
                ui.set_insp_looping(false);
            }
        }
    }

    /// Golfvorm en voortgang van de actieve (anders de geselecteerde) audio-cue.
    fn refresh_wave(&mut self, ui: &MainWindow) {
        let cues = &self.engine.show().cues;
        let active = (0..cues.len()).find(|&i| {
            cues[i].kind == CueKind::Audio
                && matches!(self.engine.states()[i], CueState::Active { .. })
        });
        let shown = active.or(self.selected.filter(|&i| cues[i].kind == CueKind::Audio));
        let Some(i) = shown else {
            if self.wave_cue.take().is_some() {
                ui.set_wave_commands("".into());
            }
            ui.set_wave_title("".into());
            ui.set_wave_time("".into());
            ui.set_wave_progress(0.0);
            return;
        };
        let cue = cues[i].clone();
        let Some(clip) = self.clip_for(&cue) else {
            if self.wave_cue.take().is_some() {
                ui.set_wave_commands("".into());
            }
            ui.set_wave_title(format!("{} • {}", i + 1, cue.name).into());
            ui.set_wave_time(
                match self.cache.status(&self.resolve_media(&cue.media)) {
                    ClipStatus::Loading => "laden…".to_string(),
                    ClipStatus::Failed(e) => e,
                    _ => "geen audio geladen".to_string(),
                }
                .into(),
            );
            ui.set_wave_progress(0.0);
            return;
        };
        if self.wave_cue != Some(cue.id) {
            self.wave_cue = Some(cue.id);
            ui.set_wave_commands(wave_path(clip.peaks()).into());
        }
        let total = clip.duration();
        let elapsed = self.engine.progress(i).map_or(Duration::ZERO, |(e, _)| {
            if cue.looping && !total.is_zero() {
                Duration::from_secs_f64(e.as_secs_f64() % total.as_secs_f64())
            } else {
                e.min(total)
            }
        });
        ui.set_wave_title(format!("{} • {}", i + 1, cue.name).into());
        ui.set_wave_time(format!("{} / {}", format_clock(elapsed), format_clock(total)).into());
        ui.set_wave_progress(if total.is_zero() {
            0.0
        } else {
            (elapsed.as_secs_f64() / total.as_secs_f64()) as f32
        });
    }
}

/// Lineaire piek → 0..1 op een schaal van −60..0 dB.
fn meter_fraction(peak: f32) -> f32 {
    if peak <= 0.001 {
        0.0
    } else {
        ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
    }
}

/// SVG-pad (viewbox 512×100) met één verticale streep per piek, gespiegeld rond het midden.
fn wave_path(peaks: &[f32]) -> String {
    use std::fmt::Write;
    let mut d = String::with_capacity(peaks.len() * 18);
    for (x, p) in peaks.iter().enumerate() {
        let h = (p * 48.0).max(0.5);
        let _ = write!(d, "M {x} {:.1} L {x} {:.1} ", 50.0 - h, 50.0 + h);
    }
    d
}

fn format_clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

impl AppState {
    fn preview_file(slot: usize) -> PathBuf {
        std::env::temp_dir().join(format!(
            "showmate-preview-{}-{slot}.jpg",
            std::process::id()
        ))
    }

    /// Toont ~2,5×/s een stilstaand beeld van de actieve videocue (via mpv-screenshots).
    fn refresh_preview(&mut self, ui: &MainWindow) {
        let cues = &self.engine.show().cues;
        let active = (0..cues.len()).find(|&i| {
            cues[i].kind == CueKind::Video
                && matches!(self.engine.states()[i], CueState::Active { .. })
        });
        let Some(i) = active else {
            if std::mem::take(&mut self.preview_shown) {
                ui.set_video_has_preview(false);
            }
            ui.set_video_title("".into());
            ui.set_video_time("".into());
            self.preview_ticks = 0;
            return;
        };
        let cue = &cues[i];
        ui.set_video_title(format!("{} • {}", i + 1, cue.name).into());
        let elapsed = self.engine.progress(i).map_or(Duration::ZERO, |(e, _)| e);
        let total = self.video_durations.get(&cue.id).copied().or(cue.duration);
        ui.set_video_time(
            match total {
                Some(t) if !cue.looping => {
                    format!("{} / {}", format_clock(elapsed), format_clock(t))
                }
                _ => format_clock(elapsed),
            }
            .into(),
        );

        if self.preview_ticks.is_multiple_of(20)
            && let Some(display) = self.video.display_of(cue.id.0)
        {
            // Vraag een nieuw beeld aan in de ene slot en toon de vorige uit de andere.
            self.video
                .screenshot(display, &Self::preview_file(self.preview_slot));
            self.preview_slot = 1 - self.preview_slot;
            let shown = Self::preview_file(self.preview_slot);
            if shown.exists()
                && let Ok(img) = slint::Image::load_from_path(&shown)
            {
                ui.set_video_preview(img);
                ui.set_video_has_preview(true);
                self.preview_shown = true;
            }
        }
        self.preview_ticks = self.preview_ticks.wrapping_add(1);
    }
}
