//! Native bestands- en berichtdialogen. Windows gebruikt `rfd`; op andere platformen zijn er
//! stubs die antwoorden uit `SHOWMATE_TEST_DIALOGS` lezen (`;`-gescheiden: een pad, of
//! `yes`/`no`/`cancel` voor de vraag over niet-opgeslagen wijzigingen). Alleen voor ontwikkeling.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsaved {
    Save,
    Discard,
    Cancel,
}

/// Bestandsfilter: weergavenaam + extensies (zonder punt).
pub type Filter<'a> = (&'a str, &'a [&'a str]);

#[cfg(windows)]
mod imp {
    use super::*;

    pub fn pick_file(title: &str, filters: &[Filter], dir: Option<&Path>) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new().set_title(title);
        for (name, exts) in filters {
            d = d.add_filter(*name, exts);
        }
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            d = d.set_directory(dir);
        }
        d.pick_file()
    }

    pub fn save_file(
        title: &str,
        default_name: &str,
        dir: Option<&Path>,
        ext: &str,
    ) -> Option<PathBuf> {
        let mut d = rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(default_name)
            .add_filter(ext, &[ext]);
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            d = d.set_directory(dir);
        }
        let mut path = d.save_file()?;
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        Some(path)
    }

    pub fn confirm_unsaved(title: &str, text: &str) -> Unsaved {
        use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
        match MessageDialog::new()
            .set_level(MessageLevel::Warning)
            .set_title(title)
            .set_description(text)
            .set_buttons(MessageButtons::YesNoCancel)
            .show()
        {
            MessageDialogResult::Yes => Unsaved::Save,
            MessageDialogResult::No => Unsaved::Discard,
            _ => Unsaved::Cancel,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    fn queue() -> &'static Mutex<VecDeque<String>> {
        static Q: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
        Q.get_or_init(|| {
            let items = std::env::var("SHOWMATE_TEST_DIALOGS").unwrap_or_default();
            Mutex::new(
                items
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect(),
            )
        })
    }

    fn next() -> Option<String> {
        queue().lock().ok()?.pop_front()
    }

    pub fn pick_file(_: &str, _: &[Filter], _: Option<&Path>) -> Option<PathBuf> {
        next().map(PathBuf::from)
    }

    pub fn save_file(_: &str, _: &str, _: Option<&Path>, ext: &str) -> Option<PathBuf> {
        let mut p = PathBuf::from(next()?);
        if p.extension().is_none() {
            p.set_extension(ext);
        }
        Some(p)
    }

    pub fn confirm_unsaved(_: &str, _: &str) -> Unsaved {
        match next().as_deref() {
            Some("yes") => Unsaved::Save,
            Some("no") => Unsaved::Discard,
            _ => Unsaved::Cancel,
        }
    }
}

pub use imp::{confirm_unsaved, pick_file, save_file};
