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
    /// Container: de `span` cues direct na deze cue zijn de kinderen (zie [`GroupMode`]).
    Group,
}

/// Hoe een groep zijn kinderen afspeelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GroupMode {
    /// Kinderen na elkaar: elk kind start `delay` nadat het vorige is afgelopen.
    #[default]
    Sequence,
    /// Alle kinderen tegelijk, elk `delay` na de start van de groep.
    Parallel,
    /// Als `Sequence`, maar herhaalt tot STOP (of tot de duur van de groep om is).
    Loop,
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

/// Soort MIDI-bericht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MidiKind {
    #[default]
    NoteOn,
    NoteOff,
    ControlChange,
    ProgramChange,
}

/// Een MIDI-bericht. Kanaal 1–16; data 0–127 (bij ProgramChange telt alleen `data1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiMessage {
    pub kind: MidiKind,
    pub channel: u8,
    pub data1: u8,
    pub data2: u8,
}

impl Default for MidiMessage {
    fn default() -> Self {
        Self {
            kind: MidiKind::NoteOn,
            channel: 1,
            data1: 60,
            data2: 100,
        }
    }
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
    /// Video-cues: geluid uit het videobestand dempen (standaard staat het aan).
    #[serde(default)]
    pub mute_audio: bool,
    /// OSC-cues: naam van een commando uit de instellingen; leeg = `osc_address`/`osc_args`.
    #[serde(default)]
    pub osc_command: String,
    #[serde(default)]
    pub osc_address: String,
    /// Argumenten als tekst, bv. `f:0.7 "tekst" true 5`.
    #[serde(default)]
    pub osc_args: String,
    #[serde(default)]
    pub midi: MidiMessage,
    /// Groepen: hoe de kinderen worden afgespeeld.
    #[serde(default)]
    pub group_mode: GroupMode,
    /// Groepen: aantal cues direct na deze cue dat erbij hoort (kinderen).
    #[serde(default)]
    pub span: usize,
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
            mute_audio: false,
            osc_command: String::new(),
            osc_address: String::new(),
            osc_args: String::new(),
            midi: MidiMessage::default(),
            group_mode: GroupMode::Sequence,
            span: 0,
        }
    }

    /// Standaardcue voor een nieuw toegevoegd cue-type.
    pub fn template(id: CueId, kind: CueKind) -> Self {
        let (name, output) = match kind {
            CueKind::Audio => ("Nieuwe audio-cue", "Audio 1+2"),
            CueKind::Video => ("Nieuwe video-cue", "Display 1"),
            CueKind::Osc => ("Nieuwe OSC-cue", ""),
            CueKind::Midi => ("Nieuwe MIDI-cue", ""),
            CueKind::Group => ("Nieuwe groep", ""),
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

    pub fn mute_audio(mut self) -> Self {
        self.mute_audio = true;
        self
    }

    pub fn osc(mut self, address: impl Into<String>, args: impl Into<String>) -> Self {
        self.osc_address = address.into();
        self.osc_args = args.into();
        self
    }

    pub fn midi(mut self, midi: MidiMessage) -> Self {
        self.midi = midi;
        self
    }

    pub fn group(mut self, mode: GroupMode) -> Self {
        self.kind = CueKind::Group;
        self.group_mode = mode;
        self
    }

    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = notes.into();
        self
    }
}
