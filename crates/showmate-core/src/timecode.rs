use std::time::Duration;

/// Formatteert als `UU:MM:SS.FF` waarbij FF het framenummer is bij `fps`.
pub fn format_timecode(t: Duration, fps: u32) -> String {
    let fps = fps.max(1);
    let total_secs = t.as_secs();
    let frames = (u64::from(t.subsec_nanos()) * u64::from(fps) / 1_000_000_000) as u32;
    format!(
        "{:02}:{:02}:{:02}.{:02}",
        total_secs / 3600,
        total_secs / 60 % 60,
        total_secs % 60,
        frames
    )
}

/// Duur als `MM:SS` (of `U:MM:SS` vanaf een uur), afgerond op hele seconden.
pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs_f64().round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Delay zoals in de lijst: `+5.0 s`; vanaf een minuut `+01:20`.
pub fn format_delay(d: Duration) -> String {
    if d.as_secs() >= 60 {
        format!("+{}", format_duration(d))
    } else {
        format!("+{:.1} s", d.as_secs_f64())
    }
}

/// Leest een tijdsduur uit gebruikersinvoer: `5`, `5.5`, `+5 s`, `5,5s`, `1:20`, `1:02:03`.
/// Geeft `None` bij ongeldige invoer (ook bij leeg).
pub fn parse_duration(input: &str) -> Option<Duration> {
    let t = input.trim().trim_start_matches('+').trim();
    let t = t
        .strip_suffix('s')
        .or_else(|| t.strip_suffix('S'))
        .unwrap_or(t)
        .trim();
    if t.is_empty() {
        return None;
    }
    let mut total = 0.0_f64;
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    for (n, part) in parts.iter().enumerate() {
        let value: f64 = part.trim().replace(',', ".").parse().ok()?;
        if !value.is_finite() || value < 0.0 {
            return None;
        }
        // Alleen het laatste deel (seconden) mag een fractie hebben.
        if n + 1 < parts.len() && value.fract() != 0.0 {
            return None;
        }
        total = total * 60.0 + value;
    }
    (total <= 359_999.0).then(|| Duration::from_secs_f64(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_mockup() {
        let t = Duration::from_secs(12 * 60 + 34) + Duration::from_millis(720);
        assert_eq!(format_timecode(t, 25), "00:12:34.18");
    }

    #[test]
    fn zero_and_hours() {
        assert_eq!(format_timecode(Duration::ZERO, 25), "00:00:00.00");
        assert_eq!(
            format_timecode(Duration::from_secs(3661), 25),
            "01:01:01.00"
        );
    }

    #[test]
    fn parses_user_input() {
        let secs = |x: f64| Some(Duration::from_secs_f64(x));
        assert_eq!(parse_duration("5"), secs(5.0));
        assert_eq!(parse_duration("+5.0 s"), secs(5.0));
        assert_eq!(parse_duration(" 5,5s "), secs(5.5));
        assert_eq!(parse_duration("1:20"), secs(80.0));
        assert_eq!(parse_duration("1:02:03"), secs(3723.0));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("—"), None);
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("-3"), None);
        assert_eq!(parse_duration("1.5:30"), None);
        assert_eq!(parse_duration("1:2:3:4"), None);
        assert_eq!(parse_duration("NaN"), None);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_secs(192)), "03:12");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1:01:01");
        assert_eq!(format_delay(Duration::from_secs(5)), "+5.0 s");
        assert_eq!(format_delay(Duration::from_secs(80)), "+01:20");
    }
}
