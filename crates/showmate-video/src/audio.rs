//! Geluid uit videobestanden: standaard aan, per cue te dempen.

/// Geluidsinstellingen van één videocue.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VideoAudio {
    /// Geluid uit het bestand dempen.
    pub mute: bool,
    /// Versterking in dB (0 = ongewijzigd).
    pub gain_db: f32,
}

/// dB → mpv's `volume`-eigenschap. mpv schaalt kubisch (100 = 0 dB, 50 ≈ −18 dB),
/// dus `volume = 100 · gain^(1/3)`. −60 dB en lager is stilte.
pub fn volume_percent(gain_db: f32) -> f64 {
    if gain_db <= -60.0 {
        return 0.0;
    }
    let gain = 10f64.powf(f64::from(gain_db) / 20.0);
    (100.0 * gain.cbrt()).clamp(0.0, 200.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_db_to_mpv_volume() {
        assert!((volume_percent(0.0) - 100.0).abs() < 1e-9);
        assert!((volume_percent(-6.0206) - 79.37).abs() < 0.05);
        assert!((volume_percent(-18.0618) - 50.0).abs() < 0.05);
        assert_eq!(volume_percent(-60.0), 0.0);
        assert!(volume_percent(12.0) > 150.0 && volume_percent(12.0) <= 200.0);
    }
}
