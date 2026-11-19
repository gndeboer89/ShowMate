//! Sneltoetsen: acties, standaardtoetsen, normaliseren van toetsaanslagen en toewijzen.
//!
//! Een toets wordt als tekst opgeslagen, bv. `Space`, `Esc`, `Ctrl+Z`, `Ctrl+Shift+Z`, `Up`, `F5`.
//! Modifiers staan altijd in de volgorde `Ctrl+Alt+Shift+Meta+`.

use crate::i18n::tr;
use slint::platform::Key;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Go,
    Pause,
    Stop,
    Panic,
    Reset,
    Undo,
    Redo,
    Duplicate,
    Save,
    Delete,
    SelectPrev,
    SelectNext,
}

pub const ALL: [Action; 12] = [
    Action::Go,
    Action::Pause,
    Action::Stop,
    Action::Panic,
    Action::Reset,
    Action::Undo,
    Action::Redo,
    Action::Duplicate,
    Action::Save,
    Action::Delete,
    Action::SelectPrev,
    Action::SelectNext,
];

impl Action {
    /// Vaste sleutel in `settings.json`.
    pub fn id(self) -> &'static str {
        match self {
            Action::Go => "go",
            Action::Pause => "pause",
            Action::Stop => "stop",
            Action::Panic => "panic",
            Action::Reset => "reset",
            Action::Undo => "undo",
            Action::Redo => "redo",
            Action::Duplicate => "duplicate",
            Action::Save => "save",
            Action::Delete => "delete",
            Action::SelectPrev => "select_prev",
            Action::SelectNext => "select_next",
        }
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Action::Go => "Space",
            Action::Pause => "P",
            Action::Stop => "Esc",
            Action::Panic => "Shift+Esc",
            Action::Reset => "Ctrl+R",
            Action::Undo => "Ctrl+Z",
            Action::Redo => "Ctrl+Y",
            Action::Duplicate => "Ctrl+D",
            Action::Save => "Ctrl+S",
            Action::Delete => "Delete",
            Action::SelectPrev => "Up",
            Action::SelectNext => "Down",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Go => tr("GO", "GO"),
            Action::Pause => tr("Pauze / hervat", "Pause / resume"),
            Action::Stop => tr("STOP", "STOP"),
            Action::Panic => tr(
                "PANIC (alles meteen stoppen)",
                "PANIC (stop everything at once)",
            ),
            Action::Reset => tr("Reset (terug naar het begin)", "Reset (back to the start)"),
            Action::Undo => tr("Ongedaan maken", "Undo"),
            Action::Redo => tr("Opnieuw", "Redo"),
            Action::Duplicate => tr("Cue dupliceren", "Duplicate cue"),
            Action::Save => tr("Opslaan", "Save"),
            Action::Delete => tr("Cue verwijderen", "Delete cue"),
            Action::SelectPrev => tr("Vorige cue selecteren", "Select previous cue"),
            Action::SelectNext => tr("Volgende cue selecteren", "Select next cue"),
        }
    }
}

/// Benoemde toetsen (Slint codeert ze als bijzondere tekens).
fn named_keys() -> Vec<(char, &'static str)> {
    let mut v = vec![
        (char::from(Key::Escape), "Esc"),
        (char::from(Key::Delete), "Delete"),
        (char::from(Key::Backspace), "Backspace"),
        (char::from(Key::Return), "Enter"),
        (char::from(Key::Tab), "Tab"),
        (char::from(Key::UpArrow), "Up"),
        (char::from(Key::DownArrow), "Down"),
        (char::from(Key::LeftArrow), "Left"),
        (char::from(Key::RightArrow), "Right"),
        (char::from(Key::Home), "Home"),
        (char::from(Key::End), "End"),
        (char::from(Key::PageUp), "PageUp"),
        (char::from(Key::PageDown), "PageDown"),
        (char::from(Key::Insert), "Insert"),
        (char::from(Key::F1), "F1"),
        (char::from(Key::F2), "F2"),
        (char::from(Key::F3), "F3"),
        (char::from(Key::F4), "F4"),
        (char::from(Key::F5), "F5"),
        (char::from(Key::F6), "F6"),
        (char::from(Key::F7), "F7"),
        (char::from(Key::F8), "F8"),
        (char::from(Key::F9), "F9"),
        (char::from(Key::F10), "F10"),
        (char::from(Key::F11), "F11"),
        (char::from(Key::F12), "F12"),
        (' ', "Space"),
    ];
    v.sort_by_key(|(c, _)| *c);
    v
}

/// Tekens van losse modifier-toetsen: die tellen niet als aanslag.
fn is_modifier_key(c: char) -> bool {
    [
        Key::Shift,
        Key::ShiftR,
        Key::Control,
        Key::ControlR,
        Key::Alt,
        Key::AltGr,
        Key::Meta,
        Key::MetaR,
        Key::CapsLock,
    ]
    .into_iter()
    .any(|k| char::from(k) == c)
}

/// Zet een toetsaanslag om naar de tekst-notatie. `None` voor losse modifiers en onbekende toetsen.
pub fn normalize(text: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Option<String> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None; // samengestelde invoer (IME): geen sneltoets
    }
    if is_modifier_key(c) {
        return None;
    }
    let key = if let Some((_, name)) = named_keys().into_iter().find(|(k, _)| *k == c) {
        name.to_string()
    } else if ctrl
        && ('\u{1}'..='\u{1a}').contains(&c)
        && !matches!(c, '\t' | '\n' | '\r' | '\u{8}')
    {
        // Sommige platformen leveren Ctrl+Z als het stuurteken 0x1A: terug naar de letter.
        // (0x10–0x18 zijn bij Slint modifier-toetsen en vallen hierboven al af.)
        char::from(b'A' + (c as u8 - 1)).to_string()
    } else if c.is_control() {
        return None;
    } else {
        c.to_uppercase().collect()
    };
    let mut out = String::new();
    for (on, name) in [
        (ctrl, "Ctrl"),
        (alt, "Alt"),
        (shift, "Shift"),
        (meta, "Meta"),
    ] {
        if on {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(&key);
    Some(out)
}

/// Waarom een toewijzing is geweigerd.
#[derive(Debug, PartialEq, Eq)]
pub struct Conflict(pub Action);

/// De huidige toetsbindingen: standaardwaarden plus overschrijvingen uit de instellingen.
#[derive(Debug, Clone, Default)]
pub struct Shortcuts {
    /// Alleen afwijkingen van de standaard; een lege tekst = bewust ontoegewezen.
    overrides: BTreeMap<String, String>,
}

impl Shortcuts {
    pub fn from_overrides(overrides: &BTreeMap<String, String>) -> Self {
        // Onbekende acties uit een oud/handmatig bestand negeren.
        let overrides = overrides
            .iter()
            .filter(|(k, _)| ALL.iter().any(|a| a.id() == k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self { overrides }
    }

    pub fn overrides(&self) -> &BTreeMap<String, String> {
        &self.overrides
    }

    /// De toets van `action`; leeg als die niet is toegewezen.
    pub fn key_of(&self, action: Action) -> String {
        self.overrides
            .get(action.id())
            .cloned()
            .unwrap_or_else(|| action.default_key().to_string())
    }

    pub fn lookup(&self, combo: &str) -> Option<Action> {
        ALL.into_iter().find(|a| {
            let key = self.key_of(*a);
            !key.is_empty() && key == combo
        })
    }

    fn set(&mut self, action: Action, key: &str) {
        if key == action.default_key() {
            self.overrides.remove(action.id());
        } else {
            self.overrides
                .insert(action.id().to_string(), key.to_string());
        }
    }

    /// Wijst `combo` toe aan `action`; weigert als een andere actie de toets al heeft.
    pub fn assign(&mut self, action: Action, combo: &str) -> Result<(), Conflict> {
        if let Some(other) = self.lookup(combo).filter(|o| *o != action) {
            return Err(Conflict(other));
        }
        self.set(action, combo);
        Ok(())
    }

    /// Maakt de actie toetsloos.
    pub fn clear(&mut self, action: Action) {
        self.overrides
            .insert(action.id().to_string(), String::new());
    }

    /// Zet één actie terug naar zijn standaardtoets. Heeft een andere actie die toets nu
    /// (door een eigen keuze), dan wordt die toetsloos.
    pub fn reset(&mut self, action: Action) {
        let default = action.default_key();
        if let Some(other) = self.lookup(default).filter(|o| *o != action) {
            self.clear(other);
        }
        self.set(action, default);
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &str, ctrl: bool, shift: bool) -> Option<String> {
        normalize(text, ctrl, shift, false, false)
    }

    #[test]
    fn defaults_are_unique() {
        let keys: Vec<&str> = ALL.iter().map(|a| a.default_key()).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "dubbele standaardtoets: {keys:?}");
        let ids: std::collections::HashSet<_> = ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids.len(), ALL.len());
    }

    #[test]
    fn every_default_is_what_normalize_produces() {
        let esc = char::from(Key::Escape).to_string();
        assert_eq!(press(" ", false, false).as_deref(), Some("Space"));
        assert_eq!(press("p", false, false).as_deref(), Some("P"));
        assert_eq!(press(&esc, false, false).as_deref(), Some("Esc"));
        assert_eq!(press(&esc, false, true).as_deref(), Some("Shift+Esc"));
        assert_eq!(press("r", true, false).as_deref(), Some("Ctrl+R"));
        assert_eq!(press("z", true, false).as_deref(), Some("Ctrl+Z"));
        assert_eq!(press("Z", true, true).as_deref(), Some("Ctrl+Shift+Z"));
        assert_eq!(
            press(&char::from(Key::Delete).to_string(), false, false).as_deref(),
            Some("Delete")
        );
        assert_eq!(
            press(&char::from(Key::UpArrow).to_string(), false, false).as_deref(),
            Some("Up")
        );
        assert_eq!(
            press(&char::from(Key::DownArrow).to_string(), false, false).as_deref(),
            Some("Down")
        );
    }

    #[test]
    fn handles_control_characters_and_modifier_only() {
        assert_eq!(
            press("\u{1a}", true, false).as_deref(),
            Some("Ctrl+Z"),
            "Ctrl+Z als stuurteken"
        );
        // 0x10–0x18 zijn bij Slint de modifier-toetsen zelf (Shift, Control, Alt, …).
        assert_eq!(press("\u{13}", true, false), None);
        assert_eq!(
            press(&char::from(Key::Control).to_string(), true, false),
            None
        );
        assert_eq!(
            press(&char::from(Key::Shift).to_string(), false, true),
            None
        );
        assert_eq!(press("", false, false), None);
        assert_eq!(press("ab", false, false), None);
        assert_eq!(press("\u{7}", false, false), None);
    }

    #[test]
    fn modifier_order_is_stable() {
        assert_eq!(
            normalize("a", true, true, true, false).as_deref(),
            Some("Ctrl+Alt+Shift+A")
        );
        assert_eq!(
            normalize("f", false, false, false, true).as_deref(),
            Some("Meta+F")
        );
        assert_eq!(
            normalize(&char::from(Key::F5).to_string(), false, false, false, false).as_deref(),
            Some("F5")
        );
    }

    #[test]
    fn lookup_and_defaults() {
        let s = Shortcuts::default();
        assert_eq!(s.lookup("Space"), Some(Action::Go));
        assert_eq!(s.lookup("Ctrl+Z"), Some(Action::Undo));
        assert_eq!(s.lookup("Q"), None);
        assert_eq!(s.key_of(Action::Panic), "Shift+Esc");
    }

    #[test]
    fn assign_rejects_conflicts_and_stores_only_overrides() {
        let mut s = Shortcuts::default();
        assert_eq!(s.assign(Action::Pause, "Space"), Err(Conflict(Action::Go)));
        assert_eq!(s.key_of(Action::Pause), "P", "bleef ongewijzigd");
        s.assign(Action::Go, "Enter").unwrap();
        assert_eq!(s.lookup("Space"), None);
        assert_eq!(s.lookup("Enter"), Some(Action::Go));
        // nu is Space vrij
        s.assign(Action::Pause, "Space").unwrap();
        assert_eq!(s.overrides().len(), 2);
        // opnieuw dezelfde toets toewijzen aan dezelfde actie is geen conflict
        s.assign(Action::Go, "Enter").unwrap();
        // terug naar standaard haalt de override weg
        s.assign(Action::Pause, "P").unwrap();
        assert_eq!(s.overrides().len(), 1);
    }

    #[test]
    fn clear_and_reset() {
        let mut s = Shortcuts::default();
        s.clear(Action::Delete);
        assert_eq!(s.key_of(Action::Delete), "");
        assert_eq!(s.lookup(""), None, "een lege toets matcht nooit");
        s.reset(Action::Delete);
        assert_eq!(s.key_of(Action::Delete), "Delete");
        assert!(s.overrides().is_empty());
        // reset pakt een toets terug van de actie die hem inmiddels gebruikt
        s.assign(Action::Go, "F5").unwrap();
        s.assign(Action::Pause, "Space").unwrap();
        s.reset(Action::Go);
        assert_eq!(s.key_of(Action::Go), "Space");
        assert_eq!(s.key_of(Action::Pause), "");
        s.reset_all();
        assert!(s.overrides().is_empty());
        assert_eq!(s.lookup("P"), Some(Action::Pause));
    }

    #[test]
    fn unknown_actions_in_settings_are_ignored() {
        let mut o = BTreeMap::new();
        o.insert("bestaat_niet".to_string(), "X".to_string());
        o.insert("go".to_string(), "Enter".to_string());
        let s = Shortcuts::from_overrides(&o);
        assert_eq!(s.overrides().len(), 1);
        assert_eq!(s.lookup("Enter"), Some(Action::Go));
    }
}
