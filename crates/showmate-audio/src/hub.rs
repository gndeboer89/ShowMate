//! Meerdere geluidskaarten tegelijk: één cpal-stream per apparaat, lui geopend.

use crate::device::AudioOutput;
use crate::error::AudioError;
use crate::mixer::VoiceParams;
use std::collections::HashMap;

/// Sleutel van een apparaat: `None` = het standaardapparaat van het systeem.
pub type DeviceKey = Option<String>;

#[derive(Default)]
pub struct AudioHub {
    streams: HashMap<DeviceKey, AudioOutput>,
}

impl AudioHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Geeft de stream van `device`; opent hem als dat nog niet gebeurd is.
    pub fn ensure(&mut self, device: Option<&str>) -> Result<&mut AudioOutput, AudioError> {
        let key: DeviceKey = device.map(str::to_string);
        if !self.streams.contains_key(&key) {
            let stream = AudioOutput::open(device)?;
            self.streams.insert(key.clone(), stream);
        }
        Ok(self.streams.get_mut(&key).expect("zojuist geopend"))
    }

    pub fn get(&self, device: Option<&str>) -> Option<&AudioOutput> {
        self.streams.get(&device.map(str::to_string))
    }

    /// Sluit alle streams behalve de apparaten in `keep` (geeft apparaten vrij voor andere programma's).
    pub fn retain_only(&mut self, keep: &[DeviceKey]) {
        self.streams.retain(|k, _| keep.contains(k));
    }

    /// Alle samplerates van de geopende apparaten (zonder dubbelen).
    pub fn sample_rates(&self) -> Vec<u32> {
        let mut rates: Vec<u32> = self
            .streams
            .values()
            .map(AudioOutput::sample_rate)
            .collect();
        rates.sort_unstable();
        rates.dedup();
        rates
    }

    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    /// Korte beschrijving van de geopende apparaten.
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = self
            .streams
            .values()
            .map(|a| {
                format!(
                    "{} ({} Hz, {} kan.)",
                    a.device_name(),
                    a.sample_rate(),
                    a.channels()
                )
            })
            .collect();
        parts.sort();
        parts.join("; ")
    }

    pub fn play(&mut self, device: Option<&str>, params: VoiceParams) -> Result<(), AudioError> {
        self.ensure(device)?.play(params)
    }

    /// Stopt het geluid met `id` op alle apparaten (met een korte fade).
    pub fn stop(&mut self, id: u64) {
        for a in self.streams.values_mut() {
            let _ = a.stop(id);
        }
    }

    pub fn pause(&mut self) {
        for a in self.streams.values_mut() {
            let _ = a.pause();
        }
    }

    pub fn resume(&mut self) {
        for a in self.streams.values_mut() {
            let _ = a.resume();
        }
    }

    /// Alles meteen afkappen op alle apparaten.
    pub fn panic(&mut self) {
        for a in self.streams.values_mut() {
            let _ = a.panic();
        }
    }

    /// Hoogste piek (L, R) over alle apparaten sinds de vorige aanroep.
    pub fn take_peaks(&self) -> (f32, f32) {
        self.streams
            .values()
            .map(AudioOutput::take_peaks)
            .fold((0.0, 0.0), |(l, r), (a, b)| (l.max(a), r.max(b)))
    }

    /// Haalt beëindigde geluiden leeg (anders loopt de wachtrij vol).
    pub fn drain_ended(&mut self) {
        for a in self.streams.values_mut() {
            a.drain_ended();
        }
    }
}
