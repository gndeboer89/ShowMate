//! Uitvoer naar externe apparaten: OSC (UDP) en MIDI.

mod midi_bytes;
mod osc;

#[cfg(feature = "midi")]
mod midi_out;

pub use midi_bytes::midi_bytes;
pub use osc::{OscSender, encode_message, format_args, parse_args};

#[cfg(feature = "midi")]
pub use midi_out::{MidiOut, list_midi_ports};

pub use rosc::OscType;
