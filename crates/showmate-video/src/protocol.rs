//! mpv JSON-IPC: één JSON-object per regel.

use serde_json::{Value, json};

/// Bouwt een commandoregel (inclusief newline), bv. `["set_property","pause",true]`.
pub fn command_line(args: &[Value]) -> String {
    let mut line = json!({ "command": args }).to_string();
    line.push('\n');
    line
}

/// De mpv-events waar ShowMate iets mee doet.
#[derive(Debug, Clone, PartialEq)]
pub enum MpvEvent {
    FileLoaded,
    /// Duur van het geladen bestand in seconden.
    Duration(f64),
    /// Bestand is afgelopen. `error`: afgebroken door een fout. Stop/vervangen levert géén event.
    EndOfFile {
        error: bool,
    },
}

/// Leest één regel uit mpv. Onbekende of ongeldige regels geven `None`.
pub fn parse_event(line: &str) -> Option<MpvEvent> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    match v.get("event")?.as_str()? {
        "file-loaded" => Some(MpvEvent::FileLoaded),
        "end-file" => match v.get("reason").and_then(Value::as_str) {
            Some("eof") => Some(MpvEvent::EndOfFile { error: false }),
            Some("error") => Some(MpvEvent::EndOfFile { error: true }),
            _ => None,
        },
        "property-change" if v.get("name").and_then(Value::as_str) == Some("duration") => v
            .get("data")
            .and_then(Value::as_f64)
            .filter(|d| d.is_finite() && *d >= 0.0)
            .map(MpvEvent::Duration),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_command_lines() {
        let line = command_line(&[json!("set_property"), json!("pause"), json!(true)]);
        assert_eq!(line, "{\"command\":[\"set_property\",\"pause\",true]}\n");
        // Windows-paden worden correct ge-escaped.
        let line = command_line(&[
            json!("loadfile"),
            json!(r"C:\media\intro.mp4"),
            json!("replace"),
        ]);
        assert!(line.contains(r"C:\\media\\intro.mp4"));
    }

    #[test]
    fn parses_events() {
        assert_eq!(
            parse_event(r#"{"event":"file-loaded"}"#),
            Some(MpvEvent::FileLoaded)
        );
        assert_eq!(
            parse_event(r#"{"event":"end-file","reason":"eof","playlist_entry_id":1}"#),
            Some(MpvEvent::EndOfFile { error: false })
        );
        assert_eq!(
            parse_event(r#"{"event":"end-file","reason":"error","file_error":"loading failed"}"#),
            Some(MpvEvent::EndOfFile { error: true })
        );
        assert_eq!(parse_event(r#"{"event":"end-file","reason":"stop"}"#), None);
        assert_eq!(
            parse_event(r#"{"event":"property-change","id":1,"name":"duration","data":12.5}"#),
            Some(MpvEvent::Duration(12.5))
        );
        // duration kan null zijn tijdens het laden
        assert_eq!(
            parse_event(r#"{"event":"property-change","id":1,"name":"duration"}"#),
            None
        );
        assert_eq!(parse_event(r#"{"error":"success","request_id":0}"#), None);
        assert_eq!(parse_event("geen json"), None);
    }
}
