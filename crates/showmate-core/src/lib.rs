//! Kern van ShowMate: cue-model, showklok en cue-engine.
//!
//! Deze crate kent geen GUI, audio of video. De engine is deterministisch
//! testbaar doordat de tijdbron (`Clock`) wordt geïnjecteerd.

pub mod clock;
pub mod cue;
pub mod demo;
pub mod engine;
mod serde_millis;
pub mod show;
pub mod timecode;

pub use clock::{Clock, ManualClock, SystemClock};
pub use cue::{Cue, CueId, CueKind, StartMode};
pub use engine::{CueState, Engine, Event, NextCue};
pub use show::{Show, ShowError};
pub use timecode::format_timecode;
