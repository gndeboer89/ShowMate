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
}
