//! Real-time-veilige mixer. Geen allocaties of locks in [`Mixer::render`].
//!
//! Alle voices zijn stereo. Elke voice gaat naar een uitgangspaar (0 = kanaal 1+2,
//! 1 = kanaal 3+4, …). Op een mono-apparaat wordt L+R opgeteld.

use crate::clip::AudioClip;
use std::sync::Arc;
use std::time::Duration;

pub const MAX_VOICES: usize = 64;
/// Korte ramp bij pauzeren/hervatten tegen klikken.
const PAUSE_RAMP_SECS: f32 = 0.010;

/// dB → lineaire versterking; −60 dB en lager geldt als stilte.
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -60.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// Parameters waarmee een cue een geluid start.
#[derive(Clone)]
pub struct VoiceParams {
    pub id: u64,
    pub clip: Arc<AudioClip>,
    pub gain_db: f32,
    pub fade_in: Duration,
    pub fade_out: Duration,
    pub looping: bool,
    /// Kap het geluid hier af (bv. de duur van de cue).
    pub max_duration: Option<Duration>,
    pub out_pair: usize,
}

/// Een spelend geluid.
pub struct Voice {
    id: u64,
    clip: Arc<AudioClip>,
    pos: usize,
    end: usize,
    looping: bool,
    gain: f32,
    fade_in: usize,
    fade_out: usize,
    elapsed: usize,
    stop_total: usize,
    stop_done: usize,
    out_pair: usize,
    done: bool,
}

impl Voice {
    /// Bouwt een voice voor een uitvoerrate. De clip moet al op die rate staan.
    pub fn new(p: VoiceParams, sample_rate: u32) -> Self {
        let frames = |d: Duration| (d.as_secs_f64() * f64::from(sample_rate)).round() as usize;
        let clip_frames = p.clip.frames();
        let end = p
            .max_duration
            .map_or(clip_frames, |d| frames(d).min(clip_frames));
        Self {
            id: p.id,
            clip: p.clip,
            pos: 0,
            end,
            looping: p.looping,
            gain: db_to_gain(p.gain_db),
            fade_in: frames(p.fade_in),
            fade_out: frames(p.fade_out),
            elapsed: 0,
            stop_total: 0,
            stop_done: 0,
            out_pair: p.out_pair,
            done: clip_frames == 0 || end == 0,
        }
    }

    fn envelope(&self) -> f32 {
        let mut g = self.gain;
        if self.elapsed < self.fade_in {
            g *= self.elapsed as f32 / self.fade_in as f32;
        }
        if !self.looping && self.fade_out > 0 {
            let remaining = self.end.saturating_sub(self.pos);
            if remaining < self.fade_out {
                g *= remaining as f32 / self.fade_out as f32;
            }
        }
        if self.stop_total > 0 {
            g *= 1.0 - self.stop_done as f32 / self.stop_total as f32;
        }
        g
    }

    /// Schuift één frame op; `true` als de voice klaar is.
    fn advance(&mut self) -> bool {
        self.pos += 1;
        self.elapsed += 1;
        if self.stop_total > 0 {
            self.stop_done += 1;
            if self.stop_done >= self.stop_total {
                return true;
            }
        }
        if self.looping {
            if self.pos >= self.clip.frames() {
                self.pos = 0;
            }
            false
        } else {
            self.pos >= self.end
        }
    }
}

/// Opdrachten van de controle-thread naar de audio-thread.
pub enum Command {
    Play(Voice),
    Stop { id: u64, fade_frames: usize },
    StopAll { fade_frames: usize },
    Pause,
    Resume,
}

pub struct Mixer {
    voices: Vec<Voice>,
    ended: Vec<u64>,
    paused: bool,
    pause_level: f32,
    pause_step: f32,
    peak: (f32, f32),
}

impl Mixer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            voices: Vec::with_capacity(MAX_VOICES),
            ended: Vec::with_capacity(MAX_VOICES * 2),
            paused: false,
            pause_level: 1.0,
            pause_step: 1.0 / (PAUSE_RAMP_SECS * sample_rate as f32).max(1.0),
            peak: (0.0, 0.0),
        }
    }

    pub fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::Play(v) => {
                if self.voices.len() < MAX_VOICES {
                    self.voices.push(v);
                } else {
                    self.ended.push(v.id); // vol: meld als direct beëindigd
                }
            }
            Command::Stop { id, fade_frames } => {
                for v in self.voices.iter_mut().filter(|v| v.id == id) {
                    v.stop_total = fade_frames.max(1);
                    v.stop_done = 0;
                }
            }
            Command::StopAll { fade_frames } => {
                for v in &mut self.voices {
                    v.stop_total = fade_frames.max(1);
                    v.stop_done = 0;
                }
            }
            Command::Pause => self.paused = true,
            Command::Resume => self.paused = false,
        }
    }

    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    /// Pakt een id van een beëindigde voice (geen allocatie).
    pub fn pop_ended(&mut self) -> Option<u64> {
        self.ended.pop()
    }

    /// Hoogste piek (L, R) sinds de vorige aanroep.
    pub fn take_peak(&mut self) -> (f32, f32) {
        std::mem::take(&mut self.peak)
    }

    /// Vult `out` (geïnterleaved, `channels` kanalen) met de mix.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        let channels = channels.max(1);
        let frames = out.len() / channels;

        for f in 0..frames {
            let target = if self.paused { 0.0 } else { 1.0 };
            if self.pause_level < target {
                self.pause_level = (self.pause_level + self.pause_step).min(1.0);
            } else if self.pause_level > target {
                self.pause_level = (self.pause_level - self.pause_step).max(0.0);
            }
            if self.paused && self.pause_level <= 0.0 {
                continue; // stil, voices staan stil
            }
            let frame = &mut out[f * channels..(f + 1) * channels];
            for v in self.voices.iter_mut().filter(|v| !v.done) {
                let g = v.envelope() * self.pause_level;
                let samples = v.clip.samples();
                let (l, r) = (samples[v.pos * 2] * g, samples[v.pos * 2 + 1] * g);
                if channels == 1 {
                    if v.out_pair == 0 {
                        frame[0] += (l + r) * 0.5;
                    }
                } else {
                    let base = v.out_pair * 2;
                    if base + 1 < channels {
                        frame[base] += l;
                        frame[base + 1] += r;
                    }
                }
                v.done = v.advance();
            }
        }

        // Voltooide voices opruimen (volgorde is niet belangrijk).
        let mut i = 0;
        while i < self.voices.len() {
            if self.voices[i].done {
                let v = self.voices.swap_remove(i);
                self.ended.push(v.id);
            } else {
                i += 1;
            }
        }

        // Begrenzen en piek meten.
        for frame in out.chunks_exact_mut(channels) {
            for s in frame.iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
            self.peak.0 = self.peak.0.max(frame[0].abs());
            self.peak.1 = self.peak.1.max(frame[channels.min(2) - 1].abs());
        }
    }
}
