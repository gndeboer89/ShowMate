//! UI-staat naast de engine: selectie, bestandspad, rij-model en inspector-vulling.

mod files;
mod io;
mod shortcuts;
mod workflow;

use crate::i18n::tr;
use crate::settings::Settings;
use crate::{CueRow, MainWindow};
use showmate_audio::{
    AudioClip, AudioOutput, ClipCache, ClipStatus, VoiceParams, parse_output_pair,
};
use showmate_core::{
    Cue, CueId, CueKind, CueState, EditError, Engine, Event, GroupMode, StartMode, SystemClock,
    demo::demo_show, format_delay, format_duration, format_timecode, parse_db, parse_duration,
};
use showmate_io::{MidiOut, OscSender};
use showmate_video::{VideoAudio, VideoEvent, VideoOutput, parse_displays};
use slint::{Model, ModelRc, VecModel};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Golfvorm leegmaken (na wisselen van show; cue-id's beginnen per show opnieuw bij 1).
    wave_reset: bool,
    video: VideoOutput,
    video_info: String,
    /// Debug-weergave van de laatst toegepaste videoconfiguratie (om wijzigingen te herkennen).
    video_cfg_key: String,
    video_durations: HashMap<CueId, Duration>,
    preview_ticks: u32,
    preview_slot: usize,
    preview_shown: bool,
    settings: Settings,
    /// Map met `settings.json` (los van de geopende show).
    app_dir: PathBuf,
    copies: Vec<files::PendingCopy>,
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
    undo: Vec<Vec<Cue>>,
    redo: Vec<Vec<Cue>>,
    last_edit: Option<(CueId, Instant)>,
    last_autosave: Instant,
    shortcuts: crate::keys::Shortcuts,
    capturing: Option<crate::keys::Action>,
}

/// Fallback-samplerate voor de cache als er geen audio-apparaat is.
const FALLBACK_RATE: u32 = 48_000;

/// Welke show bij het starten opent: `SHOWMATE_FILE` (ontwikkeling), anders de laatst geopende
/// (als die nog bestaat), anders `show.showmate` in de programmamap.
fn startup_path(settings: &Settings, app_dir: &Path) -> PathBuf {
    if let Some(p) = std::env::var_os("SHOWMATE_FILE") {
        return p.into();
    }
    if let Some(last) = settings.last_show.as_ref().filter(|p| p.is_file()) {
        return last.clone();
    }
    app_dir.join("show.showmate")
}

fn kind_label(kind: CueKind) -> &'static str {
    match kind {
        CueKind::Audio => "Audio",
        CueKind::Video => "Video",
        CueKind::Osc => "OSC",
        CueKind::Midi => "MIDI",
        CueKind::Group => tr("Groep", "Group"),
    }
}

/// Typenaam voor de lijst: groepen tonen hun soort (Groep / Parallel / Loop).
fn type_label(cue: &Cue) -> &'static str {
    match (cue.kind, cue.group_mode) {
        (CueKind::Group, GroupMode::Sequence) => tr("Groep", "Group"),
        (CueKind::Group, GroupMode::Parallel) => "Parallel",
        (CueKind::Group, GroupMode::Loop) => "Loop",
        (kind, _) => kind_label(kind),
    }
}

/// Vaste sleutel per soort voor de UI-logica (los van de weergavetaal).
fn kind_key(kind: CueKind) -> &'static str {
    match kind {
        CueKind::Audio => "audio",
        CueKind::Video => "video",
        CueKind::Osc => "osc",
        CueKind::Midi => "midi",
        CueKind::Group => "group",
    }
}

fn status_label(state: CueState) -> &'static str {
    match state {
        CueState::Waiting | CueState::Scheduled { .. } => tr("Wachtend", "Waiting"),
        CueState::Active { .. } => tr("Actief", "Active"),
        CueState::Finished => tr("Gespeeld", "Played"),
        CueState::Stopped => tr("Gestopt", "Stopped"),
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
        let app_dir = crate::settings::app_dir();
        let (settings, settings_msg) = crate::settings::load(&app_dir);
        let path = startup_path(&settings, &app_dir);
        let mut message = String::new();
        let mut recovered = false;
        let show = match files::load_with_recovery(&path) {
            Ok((show, rec)) => {
                recovered = rec;
                show
            }
            Err(showmate_core::ShowError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                demo_show()
            }
            Err(e) => {
                message = t!(
                    "Laden mislukt, demoshow geopend: {e}",
                    "Loading failed, opened the demo show: {e}"
                );
                demo_show()
            }
        };
        if recovered {
            message = t!(
                "Niet-opgeslagen wijzigingen hersteld uit de automatische kopie; sla op om ze te bewaren",
                "Unsaved changes recovered from the automatic copy; save to keep them"
            );
        }
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
            Err(e) => (None, t!("geen audio: {e}", "no audio: {e}")),
        };
        let video_cfg = settings.video.to_config();
        let video_cfg_key = format!("{video_cfg:?}");
        let video = VideoOutput::new(video_cfg);
        let video_info = match video.mpv() {
            Ok(p) => format!("mpv: {}", p.display()),
            Err(e) => e.to_string(),
        };
        if let Some(m) = settings_msg {
            message = m;
        }
        let cache = ClipCache::new(
            audio
                .as_ref()
                .map_or(FALLBACK_RATE, AudioOutput::sample_rate),
        );
        let shortcuts = crate::keys::Shortcuts::from_overrides(&settings.shortcuts);
        let mut state = Self {
            engine: Engine::new(SystemClock::new(), show),
            selected: None,
            path,
            dirty: recovered,
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
            wave_reset: false,
            video,
            video_info,
            video_cfg_key,
            video_durations: HashMap::new(),
            preview_ticks: 0,
            preview_slot: 0,
            preview_shown: false,
            settings,
            app_dir,
            copies: Vec::new(),
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
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            last_autosave: Instant::now(),
            shortcuts,
            capturing: None,
        };
        state.preload_all();
        state.connect_midi();
        if state.message.is_empty()
            && let Some(e) = state.warm_video()
        {
            state.message = e;
        }
        state
    }

    // ---- media -----------------------------------------------------

    /// Start vast de (onzichtbare) mpv-spelers voor de schermen die de show gebruikt.
    /// Geeft een foutmelding terug als dat niet lukt terwijl mpv wel gevonden is.
    pub(super) fn warm_video(&mut self) -> Option<String> {
        let mut displays: Vec<usize> = self
            .engine
            .show()
            .cues
            .iter()
            .filter(|c| c.kind == CueKind::Video)
            .flat_map(|c| parse_displays(&c.output))
            .collect();
        displays.sort_unstable();
        displays.dedup();
        match self.video.warm(&displays) {
            Err(e) if self.video.mpv().is_ok() => Some(e),
            _ => None,
        }
    }

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
            Some(t!("geen bestand opgegeven", "no file specified"))
        } else {
            // Streams en testbronnen (`av://…`, `http://…`) gaan ongewijzigd naar mpv.
            let path = if cue.media.contains("://") {
                PathBuf::from(&cue.media)
            } else {
                self.resolve_media(&cue.media)
            };
            if !cue.media.contains("://") && !path.is_file() {
                Some(t!("{} niet gevonden", "{} not found", cue.media))
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
            CueKind::Group => return, // groepen hebben zelf geen uitvoer
            CueKind::Osc | CueKind::Midi => {
                if let Err(e) = self.fire_io(&cue) {
                    self.message = format!("Cue {}: {e}", index + 1);
                }
                return;
            }
        }
        let problem = match (self.audio.as_mut(), cue.media.trim().is_empty()) {
            (None, _) => Some(t!("geen audio-apparaat", "no audio device")),
            (_, true) => Some(t!("geen bestand opgegeven", "no file specified")),
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
                    ClipStatus::Loading => {
                        Some(t!("{} wordt nog geladen", "{} is still loading", cue.media))
                    }
                    ClipStatus::Failed(e) => Some(format!("{}: {e}", cue.media)),
                    ClipStatus::Unknown => {
                        self.cache.request(&path);
                        Some(t!(
                            "{} was nog niet geladen",
                            "{} had not been loaded yet",
                            cue.media
                        ))
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
        // De aangeklikte cue staat klaar: de volgende GO start deze.
        if let Some(i) = self.selected {
            self.engine.set_playhead(i);
        }
        self.delay_ok = true;
        self.duration_ok = true;
        self.volume_ok = true;
        self.fade_in_ok = true;
        self.fade_out_ok = true;
        self.message.clear();
        self.fill_inspector = true;
    }

    pub fn add_cue(&mut self, kind: &str) {
        let (kind, mode) = match kind {
            "video" => (CueKind::Video, None),
            "osc" => (CueKind::Osc, None),
            "midi" => (CueKind::Midi, None),
            "group" => (CueKind::Group, Some(GroupMode::Sequence)),
            "parallel" => (CueKind::Group, Some(GroupMode::Parallel)),
            "loop" => (CueKind::Group, Some(GroupMode::Loop)),
            _ => (CueKind::Audio, None),
        };
        let before = self.snapshot();
        match self.engine.add_cue(kind, self.selected) {
            Ok(i) => {
                let name = match kind {
                    CueKind::Audio => tr("Nieuwe audio-cue", "New audio cue"),
                    CueKind::Video => tr("Nieuwe video-cue", "New video cue"),
                    CueKind::Osc => tr("Nieuwe OSC-cue", "New OSC cue"),
                    CueKind::Midi => tr("Nieuwe MIDI-cue", "New MIDI cue"),
                    CueKind::Group => "",
                };
                if !name.is_empty() {
                    let _ = self.engine.update_cue(i, |c| c.name = name.into());
                }
                if let Some(mode) = mode {
                    let name = match mode {
                        GroupMode::Sequence => "Nieuwe groep",
                        GroupMode::Parallel => "Nieuwe parallel-groep",
                        GroupMode::Loop => "Nieuwe loop-groep",
                    };
                    let _ = self.engine.update_cue(i, |c| {
                        c.group_mode = mode;
                        c.name = name.into();
                    });
                }
                self.push_undo(before, None);
                self.selected = Some(i);
                self.mark_changed();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn remove_selected(&mut self) {
        let Some(i) = self.selected else { return };
        let before = self.snapshot();
        match self.engine.remove_cue(i) {
            Ok(_) => {
                self.push_undo(before, None);
                let n = self.engine.show().cues.len();
                self.selected = if n == 0 { None } else { Some(i.min(n - 1)) };
                self.mark_changed();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn move_selected(&mut self, up: bool) {
        let Some(i) = self.selected else { return };
        let before = self.snapshot();
        match self.engine.move_cue(i, up) {
            Ok(new) => {
                // Een kind dat een groep verlaat houdt zijn index; vergelijk dus de lijst zelf.
                if new != i || self.snapshot() != before {
                    self.push_undo(before, None);
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
        let before = self.snapshot();
        let id = self.engine.show().cues[i].id;
        if self.engine.update_cue(i, f).is_ok() {
            if self.snapshot() != before {
                self.push_undo(before, Some(id));
            }
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
                self.message = t!("Opgeslagen: {}", "Saved: {}", self.path.display());
            }
            Err(e) => self.message = t!("Opslaan mislukt: {e}", "Saving failed: {e}"),
        }
    }

    fn fail(&mut self, e: EditError) {
        self.message = match e {
            EditError::Busy => t!(
                "cues zijn gepland; druk eerst op STOP om de lijst te wijzigen",
                "cues are scheduled; press STOP first to change the list"
            ),
            EditError::CueActive => t!(
                "deze cue is actief; stop hem eerst",
                "this cue is active; stop it first"
            ),
            EditError::OutOfRange => t!("cue bestaat niet", "cue does not exist"),
            EditError::Nested => t!(
                "een groep kan niet in een groep",
                "a group cannot be placed in a group"
            ),
            EditError::Unsupported => t!(
                "een groep kan niet gedupliceerd worden",
                "a group cannot be duplicated"
            ),
        };
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
        self.maybe_autosave();
        self.poll_copies();
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
        let playhead = self.engine.playhead();
        let children: Vec<bool> = (0..show.cues.len()).map(|i| show.is_child(i)).collect();
        let rows: Vec<CueRow> = show
            .cues
            .iter()
            .zip(self.engine.states())
            .enumerate()
            .map(|(i, (cue, state))| CueRow {
                number: i as i32 + 1,
                name: cue.name.as_str().into(),
                kind: type_label(cue).into(),
                indent: children.get(i).copied().unwrap_or(false),
                group: cue.kind == CueKind::Group,
                playhead: playhead == Some(i),
                media: io_labels
                    .get(i)
                    .and_then(|l| l.as_ref())
                    .map_or(cue.media.as_str(), |l| l.0.as_str())
                    .into(),
                start: if children.get(i).copied().unwrap_or(false) {
                    "↳"
                } else {
                    match cue.start {
                        StartMode::Go => "GO",
                        StartMode::Auto => "Auto",
                    }
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
                "ShowMate {} — {}{}",
                env!("CARGO_PKG_VERSION"),
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

        // "Volgende cue" = de cue die de volgende GO start (de playhead).
        match playhead {
            Some(i) => {
                ui.set_next_cue(format!("{} • {}", i + 1, show.cues[i].name).into());
                let eta = match self.engine.states()[i] {
                    CueState::Scheduled { start_at } => t!(
                        "(over {})",
                        "(in {})",
                        format_timecode(start_at.saturating_sub(self.engine.show_time()), FPS)
                            .split('.')
                            .next()
                            .unwrap_or("")
                    ),
                    CueState::Active { .. } => t!(
                        "(loopt; GO start hem niet opnieuw)",
                        "(running; GO will not restart it)"
                    ),
                    CueState::Finished | CueState::Stopped => {
                        t!("(gespeeld; GO start opnieuw)", "(played; GO restarts it)")
                    }
                    CueState::Waiting => t!("(wacht op GO)", "(waiting for GO)"),
                };
                ui.set_next_eta(eta.into());
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
                ui.set_insp_title(format!("{} • {} ({})", i + 1, c.name, type_label(c)).into());
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
        if std::mem::take(&mut self.wave_reset) {
            self.wave_cue = None;
            ui.set_wave_commands("".into());
        }
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
                    ClipStatus::Loading => t!("laden…", "loading…"),
                    ClipStatus::Failed(e) => e,
                    _ => t!("geen audio geladen", "no audio loaded"),
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
