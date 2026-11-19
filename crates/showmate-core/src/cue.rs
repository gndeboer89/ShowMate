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
    /// Volume van audio-cues in dB (0 = ongewijzigd).
    #[serde(default)]
    pub volume_db: f32,
    #[serde(with = "serde_millis", default)]
    pub fade_in: Duration,
    #[serde(with = "serde_millis", default)]
    pub fade_out: Duration,
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
            volume_db: 0.0,
            fade_in: Duration::ZERO,
            fade_out: Duration::ZERO,
        }
    }

    /// Standaardcue voor een nieuw toegevoegd cue-type.
    pub fn template(id: CueId, kind: CueKind) -> Self {
        let (name, output) = match kind {
            CueKind::Audio => ("Nieuwe audio-cue", "Audio 1+2"),
            CueKind::Video => ("Nieuwe video-cue", "Display 1"),
            CueKind::Osc => ("Nieuwe OSC-cue", "OSC"),
            CueKind::Midi => ("Nieuwe MIDI-cue", "MIDI"),
        };
        Self::new(id, name, kind).output(output)
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

    pub fn volume_db(mut self, db: f32) -> Self {
        self.volume_db = db;
        self
    }

    pub fn fade_in_secs(mut self, secs: f64) -> Self {
        self.fade_in = Duration::from_secs_f64(secs);
        self
    }

    pub fn fade_out_secs(mut self, secs: f64) -> Self {
        self.fade_out = Duration::from_secs_f64(secs);
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = notes.into();
        self
    }
}
