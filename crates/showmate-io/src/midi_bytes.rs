use showmate_core::{MidiKind, MidiMessage};

/// Zet een bericht om naar MIDI-bytes. Kanaal wordt begrensd op 1–16, data op 0–127.
pub fn midi_bytes(m: &MidiMessage) -> Vec<u8> {
    let ch = m.channel.clamp(1, 16) - 1;
    let (d1, d2) = (m.data1.min(127), m.data2.min(127));
    match m.kind {
        MidiKind::NoteOff => vec![0x80 | ch, d1, d2],
        MidiKind::NoteOn => vec![0x90 | ch, d1, d2],
        MidiKind::ControlChange => vec![0xB0 | ch, d1, d2],
        MidiKind::ProgramChange => vec![0xC0 | ch, d1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(kind: MidiKind, channel: u8, data1: u8, data2: u8) -> MidiMessage {
        MidiMessage {
            kind,
            channel,
            data1,
            data2,
        }
    }

    #[test]
    fn encodes_all_kinds() {
        assert_eq!(
            midi_bytes(&msg(MidiKind::NoteOn, 1, 60, 100)),
            vec![0x90, 60, 100]
        );
        assert_eq!(
            midi_bytes(&msg(MidiKind::NoteOff, 16, 60, 0)),
            vec![0x8F, 60, 0]
        );
        assert_eq!(
            midi_bytes(&msg(MidiKind::ControlChange, 3, 7, 127)),
            vec![0xB2, 7, 127]
        );
        assert_eq!(
            midi_bytes(&msg(MidiKind::ProgramChange, 2, 5, 99)),
            vec![0xC1, 5]
        );
    }

    #[test]
    fn clamps_out_of_range_values() {
        assert_eq!(
            midi_bytes(&msg(MidiKind::NoteOn, 0, 200, 255)),
            vec![0x90, 127, 127]
        );
        assert_eq!(
            midi_bytes(&msg(MidiKind::NoteOn, 99, 1, 1)),
            vec![0x9F, 1, 1]
        );
    }
}
