use std::time::Duration;

/// Aantal pieken per clip voor de golfvorm.
pub const PEAK_BUCKETS: usize = 512;

/// Volledig gedecodeerde audio: altijd stereo, geïnterleaved f32.
#[derive(Debug, Clone)]
pub struct AudioClip {
    samples: Vec<f32>,
    sample_rate: u32,
    peaks: Vec<f32>,
}

impl AudioClip {
    /// `samples` is stereo geïnterleaved (L, R, L, R, …).
    pub fn from_stereo(mut samples: Vec<f32>, sample_rate: u32) -> Self {
        if samples.len() % 2 == 1 {
            samples.pop();
        }
        let peaks = compute_peaks(&samples, PEAK_BUCKETS);
        Self {
            samples,
            sample_rate: sample_rate.max(1),
            peaks,
        }
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / f64::from(self.sample_rate))
    }

    /// Genormaliseerde piekwaarden (0..=1) voor een golfvorm, `PEAK_BUCKETS` stuks.
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    /// Lineaire resampling naar `rate`. Eenvoudig en goedkoop; een sinc-resampler kan later.
    pub fn resampled(&self, rate: u32) -> Self {
        if rate == self.sample_rate || self.frames() == 0 {
            return Self {
                sample_rate: rate.max(1),
                ..self.clone()
            };
        }
        let ratio = f64::from(self.sample_rate) / f64::from(rate);
        let out_frames = (self.frames() as f64 / ratio).ceil() as usize;
        let last = self.frames() - 1;
        let mut out = Vec::with_capacity(out_frames * 2);
        for i in 0..out_frames {
            let src = i as f64 * ratio;
            let i0 = (src.floor() as usize).min(last);
            let i1 = (i0 + 1).min(last);
            let t = (src - i0 as f64) as f32;
            for ch in 0..2 {
                let a = self.samples[i0 * 2 + ch];
                let b = self.samples[i1 * 2 + ch];
                out.push(a + (b - a) * t);
            }
        }
        Self::from_stereo(out, rate)
    }
}

fn compute_peaks(samples: &[f32], buckets: usize) -> Vec<f32> {
    let frames = samples.len() / 2;
    if frames == 0 {
        return vec![0.0; buckets];
    }
    (0..buckets)
        .map(|b| {
            let from = b * frames / buckets;
            let to = ((b + 1) * frames / buckets).max(from + 1).min(frames);
            samples[from * 2..to * 2]
                .iter()
                .fold(0.0_f32, |m, s| m.max(s.abs()))
                .min(1.0)
        })
        .collect()
}
