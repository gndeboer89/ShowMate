//! Audio voor ShowMate: decoderen, mixen en uitvoer.
//!
//! - [`AudioClip`] / [`decode_file`]: bestand → stereo f32 op de uitvoer-samplerate.
//! - [`Mixer`]: pure, real-time-veilige mixer (geen cpal; volledig testbaar).
//! - [`ClipCache`]: laadt clips op de achtergrond zodat GO nooit hoeft te wachten.
//! - [`AudioOutput`] (feature `device`): cpal-stream die de mixer aandrijft.

mod cache;
mod clip;
mod decode;
mod error;
mod mixer;
mod output;

#[cfg(feature = "device")]
mod device;
#[cfg(feature = "device")]
mod hub;

pub use cache::{ClipCache, ClipStatus};
pub use clip::AudioClip;
pub use decode::decode_file;
pub use error::AudioError;
pub use mixer::{Command, MAX_VOICES, Mixer, Voice, VoiceParams, db_to_gain};
pub use output::parse_output_pair;

#[cfg(feature = "device")]
pub use device::{AudioOutput, default_output_name, list_output_devices};
#[cfg(feature = "device")]
pub use hub::{AudioHub, DeviceKey};
