//! UI-staat naast de engine: selectie, bestandspad, rij-model en inspector-vulling.

use crate::{CueRow, MainWindow};
use showmate_core::{
    Cue, CueKind, CueState, EditError, Engine, Show, StartMode, SystemClock, demo::demo_show,
    format_delay, format_duration, format_timecode, parse_duration,
};
use slint::{Model, ModelRc, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
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
}

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
        Self {
            engine: Engine::new(SystemClock::new(), show),
            selected: None,
            path,
            dirty: false,
            message,
            delay_ok: true,
            duration_ok: true,
            rows: Rc::new(VecModel::default()),
            fill_inspector: false,
        }
    }

    // ---- acties ----------------------------------------------------

    pub fn select(&mut self, index: i32) {
        let n = self.engine.show().cues.len();
        self.selected = usize::try_from(index).ok().filter(|i| *i < n);
        self.delay_ok = true;
        self.duration_ok = true;
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
    }

    /// Elke timer-tick.
    pub fn tick(&mut self, ui: &MainWindow) {
        self.engine.tick();
        self.refresh_live(ui);
    }

    pub fn refresh_all(&mut self, ui: &MainWindow) {
        ui.set_rows(ModelRc::from(self.rows.clone()));
        self.refresh_live(ui);
        self.fill_inspector(ui);
    }

    fn refresh_live(&mut self, ui: &MainWindow) {
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
                media: cue.media.as_str().into(),
                start: match cue.start {
                    StartMode::Go => "GO",
                    StartMode::Auto => "Auto",
                }
                .into(),
                delay: format_delay(cue.delay).into(),
                duration: match (cue.looping, cue.duration) {
                    (true, _) => "Loop".into(),
                    (false, Some(d)) => format_duration(d).into(),
                    (false, None) => "—".into(),
                },
                output: cue.output.as_str().into(),
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
            }
            None => {
                for set in [
                    MainWindow::set_insp_name,
                    MainWindow::set_insp_media,
                    MainWindow::set_insp_output,
                    MainWindow::set_insp_notes,
                    MainWindow::set_insp_delay,
                    MainWindow::set_insp_duration,
                ] {
                    set(ui, "".into());
                }
                ui.set_insp_title("".into());
                ui.set_insp_looping(false);
            }
        }
    }
}
