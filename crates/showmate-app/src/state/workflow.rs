//! Bewerkgemak en veiligheid: ongedaan maken, dupliceren, panic, reset en automatisch opslaan.

use super::AppState;
use showmate_core::{Cue, CueId, GroupMode, autosave_path};
use std::time::{Duration, Instant};

/// Hoeveel ongedaan-stappen we bewaren.
const UNDO_LIMIT: usize = 100;
/// Snelle opeenvolgende wijzigingen aan dezelfde cue tellen als één stap.
const COALESCE: Duration = Duration::from_millis(1500);
const AUTOSAVE_EVERY: Duration = Duration::from_secs(30);

impl AppState {
    // ---- ongedaan maken ---------------------------------------------------

    pub(super) fn snapshot(&self) -> Vec<Cue> {
        self.engine.show().cues.clone()
    }

    /// Bewaart `before` als ongedaan-stap. Met een `key` worden snelle wijzigingen aan
    /// dezelfde cue samengevoegd (typen in een tekstveld = één stap).
    pub(super) fn push_undo(&mut self, before: Vec<Cue>, key: Option<CueId>) {
        let now = Instant::now();
        let merge = matches!(
            (key, self.last_edit),
            (Some(k), Some((last, at))) if k == last && now.duration_since(at) < COALESCE
        );
        self.last_edit = key.map(|k| (k, now));
        self.redo.clear();
        if merge {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    fn apply_snapshot(&mut self, target: Vec<Cue>, from_undo: bool) {
        let current = self.snapshot();
        match self.engine.restore_cues(target.clone()) {
            Ok(()) => {
                if from_undo {
                    self.redo.push(current);
                } else {
                    self.undo.push(current);
                }
                self.last_edit = None;
                let n = self.engine.show().cues.len();
                self.selected = self
                    .selected
                    .filter(|i| *i < n)
                    .or_else(|| n.checked_sub(1).map(|_| 0));
                self.dirty = true;
                self.message = if from_undo {
                    "Ongedaan gemaakt"
                } else {
                    "Opnieuw gedaan"
                }
                .into();
                self.fill_inspector = true;
            }
            Err(e) => {
                // Terugzetten: de stap blijft op zijn stapel staan.
                let stack = if from_undo {
                    &mut self.undo
                } else {
                    &mut self.redo
                };
                stack.push(target);
                self.fail(e);
            }
        }
    }

    pub fn undo(&mut self) {
        match self.undo.pop() {
            Some(s) => self.apply_snapshot(s, true),
            None => self.message = t!("Niets om ongedaan te maken", "Nothing to undo"),
        }
    }

    pub fn redo(&mut self) {
        match self.redo.pop() {
            Some(s) => self.apply_snapshot(s, false),
            None => self.message = t!("Niets om opnieuw te doen", "Nothing to redo"),
        }
    }

    // ---- bewerken ------------------------------------------------------------

    pub fn duplicate_selected(&mut self) {
        let Some(i) = self.selected else { return };
        let before = self.snapshot();
        match self.engine.duplicate_cue(i) {
            Ok(new) => {
                self.push_undo(before, None);
                self.selected = Some(new);
                self.mark_changed();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn edit_group_mode(&mut self, index: i32) {
        let mode = match index {
            1 => GroupMode::Parallel,
            2 => GroupMode::Loop,
            _ => GroupMode::Sequence,
        };
        self.edit(|c| c.group_mode = mode);
    }

    // ---- veiligheid ------------------------------------------------------------

    /// Alles meteen stoppen: geluid hard afkappen, beelden weg, engine stop.
    pub fn panic_stop(&mut self) {
        if let Some(a) = self.audio.as_mut() {
            let _ = a.panic();
        }
        let events = self.engine.stop();
        self.handle_events(events);
        self.video.hide_all();
        self.message = t!("PANIC: alles gestopt", "PANIC: everything stopped");
    }

    /// Alles stoppen en de show terugzetten naar het begin (alle cues weer wachtend).
    pub fn reset_show(&mut self) {
        self.stop();
        self.engine.reset();
        self.message = t!(
            "Show teruggezet naar het begin",
            "Show reset to the beginning"
        );
    }

    // ---- automatisch opslaan -------------------------------------------------

    /// Elke 30 s een kopie naast de showfile als er niet-opgeslagen wijzigingen zijn.
    pub(super) fn maybe_autosave(&mut self) {
        if self.dirty && self.last_autosave.elapsed() >= AUTOSAVE_EVERY {
            self.write_autosave();
        }
    }

    fn write_autosave(&mut self) {
        self.last_autosave = Instant::now();
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = self.engine.show().autosave(&self.path) {
            self.message = t!(
                "Automatisch opslaan mislukt: {e}",
                "Automatic save failed: {e}"
            );
        }
    }

    /// Bij afsluiten: niet-opgeslagen werk bewaren als automatische kopie (wordt bij de
    /// volgende start hersteld).
    pub fn on_exit(&mut self) {
        if self.dirty {
            self.write_autosave();
        } else {
            let _ = std::fs::remove_file(autosave_path(&self.path));
        }
    }

    /// Help → Over ShowMate.
    pub fn about(&self) {
        let text = format!(
            "ShowMate {}\n\n{}\nCopyright (C) 2026 Geert de Boer\nGPL-3.0-or-later\nhttps://github.com/gndeboer89/ShowMate",
            env!("CARGO_PKG_VERSION"),
            crate::i18n::tr(
                "Cue-software voor shows (audio, video, OSC, MIDI).",
                "Cue software for shows (audio, video, OSC, MIDI)."
            ),
        );
        crate::dialogs::info(crate::i18n::tr("Over ShowMate", "About ShowMate"), &text);
    }
}
