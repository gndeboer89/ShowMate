use crate::serde_millis;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CueId(pub u64);

/// Soort cue. Groep/Parallel/Loop-containers volgen in een latere fase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CueKind {
    Audio,
    Video,
    Osc,
    Midi,
}

/// Hoe een cue gestart wordt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartMode {
    /// Wacht op een handmatige GO.
    Go,
    /// Start automatisch, `delay` na de start van de vorige cue (auto-follow).
    Auto,
}

/// Een cue: pure data, zonder runtime-status (die zit in de engine).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: CueId,
    pub name: String,
    pub kind: CueKind,
    /// Bestandsnaam of doel, bv. `intro.mp4` of `MIDI / OSC`.
    #[serde(default)]
    pub media: String,
    pub start: StartMode,
    #[serde(with = "serde_millis", default)]
    pub delay: Duration,
    /// `None` = onbepaald: loopt tot stop (Audio/Video) of is direct klaar (Osc/Midi).
    #[serde(with = "serde_millis::opt", default)]
    pub duration: Option<Duration>,
    /// Herhaal oneindig; de cue eindigt dan nooit vanzelf.
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub notes: String,
}

impl Cue {
    pub fn new(id: CueId, name: impl Into<String>, kind: CueKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            media: String::new(),
            start: StartMode::Go,
            delay: Duration::ZERO,
            duration: None,
            looping: false,
            output: String::new(),
            notes: String::new(),
        }
    }

    pub fn media(mut self, media: impl Into<String>) -> Self {
        self.media = media.into();
        self
    }

    pub fn auto(mut self) -> Self {
        self.start = StartMode::Auto;
        self
    }

    pub fn delay_secs(mut self, secs: f64) -> Self {
        self.delay = Duration::from_secs_f64(secs);
        self
    }

    pub fn duration_secs(mut self, secs: f64) -> Self {
        self.duration = Some(Duration::from_secs_f64(secs));
        self
    }

    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = output.into();
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = notes.into();
        self
    }
}
