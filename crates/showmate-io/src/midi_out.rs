use crate::midi_bytes::midi_bytes;
use midir::{MidiOutput, MidiOutputConnection};
use showmate_core::MidiMessage;

const CLIENT_NAME: &str = "ShowMate";

/// Namen van de beschikbare MIDI-uitgangen.
pub fn list_midi_ports() -> Result<Vec<String>, String> {
    let out = MidiOutput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
    Ok(out
        .ports()
        .iter()
        .filter_map(|p| out.port_name(p).ok())
        .collect())
}

/// Een open verbinding met één MIDI-uitgang.
pub struct MidiOut {
    conn: MidiOutputConnection,
    name: String,
}

impl MidiOut {
    /// Verbindt met de uitgang met exact deze naam.
    pub fn connect(port_name: &str) -> Result<Self, String> {
        let out = MidiOutput::new(CLIENT_NAME).map_err(|e| e.to_string())?;
        let port = out
            .ports()
            .into_iter()
            .find(|p| out.port_name(p).is_ok_and(|n| n == port_name))
            .ok_or_else(|| format!("MIDI-uitgang '{port_name}' niet gevonden"))?;
        let conn = out
            .connect(&port, "showmate-out")
            .map_err(|e| e.to_string())?;
        Ok(Self {
            conn,
            name: port_name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn send(&mut self, msg: &MidiMessage) -> Result<(), String> {
        self.conn.send(&midi_bytes(msg)).map_err(|e| e.to_string())
    }
}
