//! Video voor ShowMate: één `mpv`-proces per scherm, aangestuurd via mpv's JSON-IPC.
//!
//! Waarom een proces en geen gelinkte libmpv? Geen link-/DLL-problemen bij het bouwen,
//! een crash van de decoder haalt de show-software niet neer, en mpv doet hardware-decoding,
//! loops en meerdere schermen al. Nadeel: `mpv.exe` moet meegeleverd worden (zie README).

mod audio;
mod controller;
mod displays;
mod ipc;
mod player;
mod protocol;

pub use audio::{VideoAudio, volume_percent};
pub use controller::{VideoConfig, VideoEvent, VideoOutput, locate_mpv};
pub use displays::parse_displays;
pub use protocol::{MpvEvent, command_line, parse_event};
