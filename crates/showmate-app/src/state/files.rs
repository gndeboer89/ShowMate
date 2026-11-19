//! Shows aanmaken, openen en opslaan; mediabestanden toevoegen (Bladeren, slepen, kopiëren).

use super::AppState;
use crate::dialogs::{self, Unsaved};
use crate::i18n::tr;
use crate::media;
use crate::settings;
use showmate_core::{
    CueKind, Engine, Show, ShowError, SystemClock, autosave_path, recoverable_autosave,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

/// Een kopieeractie op de achtergrond (grote videobestanden mogen de UI niet blokkeren).
pub(super) struct PendingCopy {
    name: String,
    rx: Receiver<Result<PathBuf, String>>,
}

/// Laadt een show; bestaat er een nieuwere automatische kopie, dan wordt die geladen.
/// Het tweede deel is `true` als het die kopie was.
pub(super) fn load_with_recovery(path: &Path) -> Result<(Show, bool), ShowError> {
    if let Some(auto) = recoverable_autosave(path)
        && let Ok(show) = Show::load(&auto)
    {
        return Ok((show, true));
    }
    Show::load(path).map(|s| (s, false))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map_or_else(|| "show".into(), |s| s.to_string_lossy().into_owned())
}

impl AppState {
    /// Map van de show: relatieve mediapaden en de map `media` horen hierbij.
    pub(super) fn base_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    // ---- shows ---------------------------------------------------------------

    /// Vraagt wat er met niet-opgeslagen wijzigingen moet gebeuren. `true` = doorgaan.
    fn confirm_discard(&mut self) -> bool {
        if !self.dirty {
            return true;
        }
        let name = self.engine.show().name.clone();
        let text = t!(
            "'{name}' heeft niet-opgeslagen wijzigingen. Eerst opslaan?",
            "'{name}' has unsaved changes. Save first?"
        );
        match dialogs::confirm_unsaved(tr("ShowMate", "ShowMate"), &text) {
            Unsaved::Save => {
                self.save();
                !self.dirty
            }
            Unsaved::Discard => {
                let _ = std::fs::remove_file(autosave_path(&self.path));
                true
            }
            Unsaved::Cancel => false,
        }
    }

    fn start_dir(&self) -> Option<PathBuf> {
        self.path.parent().map(Path::to_path_buf)
    }

    pub fn new_show(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        let title = tr(
            "Nieuwe show: kies waar je hem opslaat",
            "New show: choose where to save it",
        );
        let dir = self.start_dir();
        let Some(path) =
            dialogs::save_file(title, "Nieuwe show.showmate", dir.as_deref(), "showmate")
        else {
            return;
        };
        let show = Show::new(stem(&path));
        match show.save(&path) {
            Ok(()) => self.replace_show(show, path, false),
            Err(e) => self.message = t!("Opslaan mislukt: {e}", "Saving failed: {e}"),
        }
    }

    pub fn open_show_dialog(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        let filters: [dialogs::Filter; 1] = [("ShowMate", &["showmate"])];
        let dir = self.start_dir();
        if let Some(path) =
            dialogs::pick_file(tr("Show openen", "Open show"), &filters, dir.as_deref())
        {
            self.load_path(path);
        }
    }

    /// Opent `path` (na de vraag over niet-opgeslagen wijzigingen).
    pub fn open_path(&mut self, path: PathBuf) {
        if self.confirm_discard() {
            self.load_path(path);
        }
    }

    pub fn open_recent(&mut self, index: i32) {
        if let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.recent.get(i))
            .cloned()
        {
            self.open_path(path);
        }
    }

    fn load_path(&mut self, path: PathBuf) {
        match load_with_recovery(&path) {
            Ok((show, recovered)) => self.replace_show(show, path, recovered),
            Err(e) => {
                self.message = t!("Openen mislukt: {e}", "Opening failed: {e}");
                // Een verdwenen bestand ook uit de recente lijst halen.
                self.settings.recent.retain(|p| *p != path);
                self.fill_settings = true;
            }
        }
    }

    /// Wisselt naar een andere show en zet alles wat bij de vorige hoorde terug.
    pub(super) fn replace_show(&mut self, show: Show, path: PathBuf, recovered: bool) {
        // Niets mag doorspelen uit de vorige show.
        self.audio.panic();
        self.video.hide_all();

        self.engine = Engine::new(SystemClock::new(), show);
        self.path = path.clone();
        self.dirty = recovered;
        self.selected = None;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
        self.video_durations.clear();
        self.wave_reset = true;
        self.osc_args_error.clear();
        self.delay_ok = true;
        self.duration_ok = true;
        self.volume_ok = true;
        self.fade_in_ok = true;
        self.fade_out_ok = true;
        self.copies.clear();
        self.message = if recovered {
            t!(
                "Niet-opgeslagen wijzigingen hersteld uit de automatische kopie; sla op om ze te bewaren",
                "Unsaved changes recovered from the automatic copy; save to keep them"
            )
        } else {
            t!("Geopend: {}", "Opened: {}", path.display())
        };

        self.settings.last_show = Some(path.clone());
        settings::push_recent(&mut self.settings.recent, &path);
        if let Err(e) = settings::save(&self.app_dir, &self.settings) {
            self.message = t!(
                "Instellingen opslaan mislukt: {e}",
                "Saving settings failed: {e}"
            );
        }

        self.warm_audio();
        if let Some(e) = self.warm_video() {
            self.message = e;
        }
        self.fill_inspector = true;
        self.fill_settings = true;
    }

    pub fn save_as(&mut self) {
        let title = tr("Opslaan als", "Save as");
        let default_name = self.path.file_name().map_or_else(
            || "show.showmate".into(),
            |n| n.to_string_lossy().into_owned(),
        );
        let dir = self.start_dir();
        let Some(new_path) = dialogs::save_file(title, &default_name, dir.as_deref(), "showmate")
        else {
            return;
        };
        let (old_base, old_path) = (self.base_dir(), self.path.clone());
        self.engine.set_show_name(stem(&new_path));
        if let Err(e) = self.engine.show().save(&new_path) {
            self.message = t!("Opslaan mislukt: {e}", "Saving failed: {e}");
            return;
        }
        // Mediabestanden die relatief naast de oude show staan, gaan mee naar de nieuwe map.
        let media: Vec<String> = self
            .engine
            .show()
            .cues
            .iter()
            .map(|c| c.media.clone())
            .collect();
        let carry = media::files_to_carry(&old_base, &self.base_dir_of(&new_path), &media);
        if old_path != new_path {
            let _ = std::fs::remove_file(autosave_path(&old_path));
        }
        self.path = new_path.clone();
        self.dirty = false;
        self.settings.last_show = Some(new_path.clone());
        settings::push_recent(&mut self.settings.recent, &new_path);
        let _ = settings::save(&self.app_dir, &self.settings);
        self.fill_settings = true;
        self.message = t!("Opgeslagen als: {}", "Saved as: {}", new_path.display());
        self.start_copies(carry);
    }

    fn base_dir_of(&self, path: &Path) -> PathBuf {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    // ---- mediabestanden ---------------------------------------------------------

    /// Zet een gekozen/gesleept bestand in de show: kopieert het (op de achtergrond) naar de
    /// media-map als dat aan staat en geeft de waarde voor `Cue::media` terug.
    fn attach_media(&mut self, src: &Path) -> String {
        let placement = media::place(&self.base_dir(), src, self.settings.copy_media);
        match placement.copy {
            Some(job) => self.start_copies(vec![job]),
            None => {
                if media::kind_of(src) == Some(CueKind::Audio) {
                    self.cache
                        .request(&self.resolve_media(&placement.value), &self.rates());
                }
            }
        }
        placement.value
    }

    /// Kiest een bestand voor de geselecteerde audio- of videocue.
    pub fn browse_media(&mut self) {
        let Some(i) = self.selected else { return };
        let kind = self.engine.show().cues[i].kind;
        let (label, exts): (&str, &[&str]) = match kind {
            CueKind::Audio => (
                tr("Audiobestanden", "Audio files"),
                media::audio_extensions(),
            ),
            CueKind::Video => (
                tr("Videobestanden", "Video files"),
                media::video_extensions(),
            ),
            _ => return,
        };
        let filters: [dialogs::Filter; 1] = [(label, exts)];
        let media_dir = self.base_dir().join("media");
        let dir = if media_dir.is_dir() {
            Some(media_dir)
        } else {
            self.start_dir()
        };
        let Some(src) = dialogs::pick_file(
            tr("Bestand kiezen", "Choose file"),
            &filters,
            dir.as_deref(),
        ) else {
            return;
        };
        let value = self.attach_media(&src);
        self.edit(|c| c.media = value);
        self.video_durations.remove(&self.engine.show().cues[i].id);
        self.fill_inspector = true;
    }

    /// Bestanden die op het venster zijn gesleept worden nieuwe cues (een `.showmate`-bestand wordt geopend).
    pub fn add_dropped_files(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            let is_show = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("showmate"));
            if is_show {
                self.open_path(path);
                return;
            }
            let Some(kind) = media::kind_of(&path) else {
                self.message = t!(
                    "Onbekend bestandstype: {}",
                    "Unknown file type: {}",
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                );
                continue;
            };
            let before = self.snapshot();
            match self.engine.add_cue(kind, self.selected) {
                Ok(i) => {
                    let value = self.attach_media(&path);
                    let name = stem(&path);
                    let _ = self.engine.update_cue(i, |c| {
                        c.name = name;
                        c.media = value;
                    });
                    self.push_undo(before, None);
                    self.selected = Some(i);
                    self.mark_changed();
                }
                Err(e) => {
                    self.fail(e);
                    break;
                }
            }
        }
    }

    pub fn set_copy_media(&mut self, on: bool) {
        self.settings.copy_media = on;
    }

    // ---- kopiëren op de achtergrond -----------------------------------------------

    fn start_copies(&mut self, jobs: Vec<(PathBuf, PathBuf)>) {
        for (from, to) in jobs {
            let name = to
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let (tx, rx) = channel();
            let dest = to.clone();
            std::thread::spawn(move || {
                let result = media::copy_atomic(&from, &dest)
                    .map(|()| dest)
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            });
            self.message = t!(
                "Kopiëren naar de media-map: {name}…",
                "Copying to the media folder: {name}…"
            );
            self.copies.push(PendingCopy { name, rx });
        }
    }

    /// Elke tick: verwerkt afgeronde kopieën en start het laden van audio.
    pub(super) fn poll_copies(&mut self) {
        if self.copies.is_empty() {
            return;
        }
        let mut done: Vec<(String, Result<PathBuf, String>)> = Vec::new();
        self.copies.retain(|c| match c.rx.try_recv() {
            Ok(r) => {
                done.push((c.name.clone(), r));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done.push((c.name.clone(), Err("kopieerthread gestopt".into())));
                false
            }
        });
        for (name, result) in done {
            match result {
                Ok(dest) => {
                    if media::kind_of(&dest) == Some(CueKind::Audio) {
                        self.cache.request(&dest, &self.rates());
                    }
                    if self.copies.is_empty() {
                        self.message = t!("Gekopieerd: {name}", "Copied: {name}");
                    }
                }
                Err(e) => {
                    self.message = t!(
                        "Kopiëren van {name} mislukt: {e}",
                        "Copying {name} failed: {e}"
                    )
                }
            }
        }
    }

    /// Tekst voor de recente-shows-lijst in het menu.
    pub(super) fn recent_labels(&self) -> Vec<String> {
        self.settings
            .recent
            .iter()
            .map(|p| {
                let dir = p
                    .parent()
                    .and_then(Path::file_name)
                    .map_or_else(String::new, |d| format!("  —  {}", d.to_string_lossy()));
                format!("{}{dir}", stem(p))
            })
            .collect()
    }
}
