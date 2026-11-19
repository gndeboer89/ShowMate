//! Sneltoetsen afhandelen en bewerken in het instellingenvenster.

use super::AppState;
use crate::KeyRow;
use crate::keys::{self, Action};

impl AppState {
    /// Verwerkt een toetsaanslag uit de UI. Geeft `true` als de toets is afgehandeld.
    /// Verwerkt een toetsaanslag. `from_settings`: komt uit het instellingenvenster, waar alleen
    /// een nieuwe sneltoets wordt opgenomen; anders uit het hoofdvenster (sneltoetsen uitvoeren).
    /// Geeft `true` als de toets is afgehandeld.
    pub fn handle_key(
        &mut self,
        text: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
        from_settings: bool,
    ) -> bool {
        let Some(combo) = keys::normalize(text, ctrl, shift, alt, meta) else {
            // Losse modifier: tijdens het opnemen blijven we wachten op de echte toets.
            return from_settings && self.capturing.is_some();
        };
        if from_settings {
            let Some(action) = self.capturing.take() else {
                return false;
            };
            match self.shortcuts.assign(action, &combo) {
                Ok(()) => {
                    self.settings.shortcuts = self.shortcuts.overrides().clone();
                    self.settings_note.clear();
                }
                Err(keys::Conflict(other)) => {
                    self.settings_note = t!(
                        "{combo} wordt al gebruikt voor: {}",
                        "{combo} is already used for: {}",
                        other.label()
                    );
                }
            }
            self.fill_names = true;
            return true;
        }
        if self.capturing.is_some() {
            return false; // tijdens het opnemen van een toets geen show-acties
        }
        match self.shortcuts.lookup(&combo) {
            Some(action) => {
                self.perform(action);
                true
            }
            None => false,
        }
    }

    fn perform(&mut self, action: Action) {
        let n = self.engine.show().cues.len();
        match action {
            Action::Go => self.go(),
            Action::Pause => self.toggle_pause(),
            Action::Stop => self.stop(),
            Action::Panic => self.panic_stop(),
            Action::Reset => self.reset_show(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::Duplicate => self.duplicate_selected(),
            Action::Save => self.save(),
            Action::NewShow => self.new_show(),
            Action::OpenShow => self.open_show_dialog(),
            Action::SaveAs => self.save_as(),
            Action::Delete => self.remove_selected(),
            Action::SelectPrev => {
                let i = self.selected.map_or(0, |i| i.saturating_sub(1));
                self.select(i as i32);
            }
            Action::SelectNext => {
                let i = self
                    .selected
                    .map_or(0, |i| (i + 1).min(n.saturating_sub(1)));
                self.select(i as i32);
            }
        }
    }

    fn action_at(row: i32) -> Option<Action> {
        usize::try_from(row)
            .ok()
            .and_then(|i| keys::ALL.get(i).copied())
    }

    /// Begint of annuleert het opnemen van een nieuwe toets voor rij `row`.
    pub fn key_capture(&mut self, row: i32) {
        let action = Self::action_at(row);
        self.capturing = if self.capturing == action {
            None
        } else {
            action
        };
        self.settings_note.clear();
        self.fill_names = true;
    }

    pub fn key_clear(&mut self, row: i32) {
        if let Some(a) = Self::action_at(row) {
            self.shortcuts.clear(a);
            self.store_shortcuts();
        }
    }

    pub fn key_reset(&mut self, row: i32) {
        if let Some(a) = Self::action_at(row) {
            self.shortcuts.reset(a);
            self.store_shortcuts();
        }
    }

    pub fn keys_reset_all(&mut self) {
        self.shortcuts.reset_all();
        self.store_shortcuts();
    }

    fn store_shortcuts(&mut self) {
        self.settings.shortcuts = self.shortcuts.overrides().clone();
        self.capturing = None;
        self.settings_note.clear();
        self.fill_names = true;
    }

    pub(super) fn key_rows(&self) -> Vec<KeyRow> {
        keys::ALL
            .iter()
            .map(|a| KeyRow {
                action: a.label().into(),
                key: self.shortcuts.key_of(*a).into(),
                capturing: self.capturing == Some(*a),
            })
            .collect()
    }
}
