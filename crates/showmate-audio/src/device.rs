//! cpal-uitvoer (WASAPI op Windows) die een [`Mixer`] in de audio-callback draait.

use crate::error::AudioError;
use crate::mixer::{Command, Mixer, Voice, VoiceParams};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Stop-ramp van `stop()` zonder opgegeven fade (tegen kliks).
const DEFAULT_STOP_FADE: Duration = Duration::from_millis(60);

/// Pieken die de audio-thread deelt met de UI. Niet-negatieve f32-bits ordenen als u32.
#[derive(Default)]
struct Peaks {
    l: AtomicU32,
    r: AtomicU32,
}

pub struct AudioOutput {
    _stream: Stream,
    commands: Producer<Command>,
    ended: Consumer<u64>,
    peaks: Arc<Peaks>,
    sample_rate: u32,
    channels: u16,
    device_name: String,
}

impl AudioOutput {
    /// Opent het standaard uitvoerapparaat.
    pub fn open_default() -> Result<Self, AudioError> {
        let dev_err = |e: &dyn std::fmt::Display| AudioError::Device(e.to_string());
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::Device("geen standaard uitvoerapparaat".into()))?;
        let device_name = device.name().unwrap_or_else(|_| "onbekend apparaat".into());
        let supported = device.default_output_config().map_err(|e| dev_err(&e))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;

        let (commands, rx) = RingBuffer::<Command>::new(256);
        let (ended_tx, ended) = RingBuffer::<u64>::new(256);
        let peaks = Arc::new(Peaks::default());
        let mixer = Mixer::new(sample_rate);

        let ctx = Ctx {
            rx,
            ended_tx,
            peaks: peaks.clone(),
            mixer,
            channels: channels as usize,
        };
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, ctx),
            SampleFormat::I16 => build::<i16>(&device, &config, ctx),
            SampleFormat::U16 => build::<u16>(&device, &config, ctx),
            other => {
                return Err(AudioError::Device(format!(
                    "sampleformaat {other:?} niet ondersteund"
                )));
            }
        }?;
        stream.play().map_err(|e| dev_err(&e))?;

        Ok(Self {
            _stream: stream,
            commands,
            ended,
            peaks,
            sample_rate,
            channels,
            device_name,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    fn send(&mut self, cmd: Command) -> Result<(), AudioError> {
        self.commands
            .push(cmd)
            .map_err(|_| AudioError::Device("audio-wachtrij vol".into()))
    }

    /// Start een geluid. De clip moet al op de rate van dit apparaat staan.
    pub fn play(&mut self, params: VoiceParams) -> Result<(), AudioError> {
        if params.clip.sample_rate() != self.sample_rate {
            return Err(AudioError::Device(
                "clip heeft andere samplerate dan het apparaat".into(),
            ));
        }
        let voice = Voice::new(params, self.sample_rate);
        self.send(Command::Play(voice))
    }

    fn frames(&self, d: Duration) -> usize {
        (d.as_secs_f64() * f64::from(self.sample_rate)).round() as usize
    }

    /// Stopt één geluid met een korte fade.
    pub fn stop(&mut self, id: u64) -> Result<(), AudioError> {
        let fade_frames = self.frames(DEFAULT_STOP_FADE);
        self.send(Command::Stop { id, fade_frames })
    }

    pub fn stop_all(&mut self) -> Result<(), AudioError> {
        let fade_frames = self.frames(DEFAULT_STOP_FADE);
        self.send(Command::StopAll { fade_frames })
    }

    pub fn pause(&mut self) -> Result<(), AudioError> {
        self.send(Command::Pause)
    }

    pub fn resume(&mut self) -> Result<(), AudioError> {
        self.send(Command::Resume)
    }

    /// Hoogste piek (L, R) sinds de vorige aanroep, lineair 0..=1.
    pub fn take_peaks(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peaks.l.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.peaks.r.swap(0, Ordering::Relaxed)),
        )
    }

    /// Id's van geluiden die klaar zijn sinds de vorige aanroep.
    pub fn drain_ended(&mut self) -> Vec<u64> {
        let mut ids = Vec::new();
        while let Ok(id) = self.ended.pop() {
            ids.push(id);
        }
        ids
    }
}

struct Ctx {
    rx: Consumer<Command>,
    ended_tx: Producer<u64>,
    peaks: Arc<Peaks>,
    mixer: Mixer,
    channels: usize,
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut ctx: Ctx,
) -> Result<Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
{
    // Ruim genoeg voor gangbare bufferlengtes; groeit hooguit eenmalig.
    let mut scratch: Vec<f32> = Vec::with_capacity(16384);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                while let Ok(cmd) = ctx.rx.pop() {
                    ctx.mixer.apply(cmd);
                }
                scratch.resize(data.len(), 0.0);
                ctx.mixer.render(&mut scratch, ctx.channels);
                for (out, s) in data.iter_mut().zip(scratch.iter()) {
                    *out = T::from_sample(*s);
                }
                let (l, r) = ctx.mixer.take_peak();
                ctx.peaks.l.fetch_max(l.to_bits(), Ordering::Relaxed);
                ctx.peaks.r.fetch_max(r.to_bits(), Ordering::Relaxed);
                while let Some(id) = ctx.mixer.pop_ended() {
                    let _ = ctx.ended_tx.push(id);
                }
            },
            |err| eprintln!("audio-fout: {err}"),
            None,
        )
        .map_err(|e| AudioError::Device(e.to_string()))
}
