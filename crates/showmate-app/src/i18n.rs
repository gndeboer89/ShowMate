//! Eenvoudige vertaling voor teksten die uit Rust komen. De UI zelf gebruikt Slint's `@tr`
//! (zie `lang/en/LC_MESSAGES/showmate-app.po`). Brontaal is Nederlands.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(english: bool) {
    ENGLISH.store(english, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Kiest een vaste tekst op taal.
pub fn tr(nl: &'static str, en: &'static str) -> &'static str {
    if english() { en } else { nl }
}

/// `t!("Nederlands {x}", "English {x}", args…)`: als `format!`, in de gekozen taal.
/// Beide teksten mogen `{}`/`{naam}` bevatten (inline captures werken).
#[macro_export]
macro_rules! t {
    ($nl:literal, $en:literal $(, $($arg:tt)*)?) => {
        if $crate::i18n::english() {
            format!($en $(, $($arg)*)?)
        } else {
            format!($nl $(, $($arg)*)?)
        }
    };
}
