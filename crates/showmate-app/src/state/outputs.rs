//! Benoemde audio- en video-uitgangen: koppeling aan geluidskaarten/kanalen en monitoren,
//! het kiezen ervan per cue, en de bewerkpagina's in de instellingen.

use super::AppState;
use crate::settings::{AudioOutputDef, AudioRoute, VideoOutputDef};
use crate::{AudioOutRow, SettingsWindow, VideoOutRow};
use showmate_audio::{AudioClip, DeviceKey, VoiceParams};
use showmate_core::CueKind;
use slint::{ModelRc, VecModel};
use std::sync::Arc;
use std::time::Duration;

impl AppState {
    // ---- uitgang van een cue omzetten naar een bestemming ------------------------------

    pub(super) fn audio_route(&self, output: &str) -> Result<AudioRoute, String> {
        self.settings.audio_route(output)
    }

    pub(super) fn video_monitors(&self, output: &str) -> Vec<usize> {
        self.settings.video_monitors(output)
    }

    pub(super) fn first_output_name(&self, kind: CueKind) -> Option<String> {
        match kind {
            CueKind::Audio => self.settings.audio.outputs.first().map(|o| o.name.clone()),
            CueKind::Video => self.settings.video.outputs.first().map(|o| o.name.clone()),
            _ => None,
        }
    }

    /// Samplerates waarop clips klaargezet moeten worden (die van de geopende kaarten).
    pub(super) fn rates(&self) -> Vec<u32> {
        let rates = self.audio.sample_rates();
        if rates.is_empty() {
            vec![super::FALLBACK_RATE]
        } else {
            rates
        }
    }

    /// Opent de geluidskaarten die de show nodig heeft (zodat GO niet hoeft te wachten) en sluit de rest.
    pub(super) fn warm_audio(&mut self) {
        let mut devices: Vec<DeviceKey> = Vec::new();
        let mut problems: Vec<String> = Vec::new();
        let outputs: Vec<String> = self
            .engine
            .show()
            .cues
            .iter()
            .filter(|c| c.kind == CueKind::Audio)
            .map(|c| c.output.clone())
            .collect();
        for out in outputs {
            match self.audio_route(&out) {
                Ok(route) if !devices.contains(&route.device) => devices.push(route.device),
                Ok(_) => {}
                Err(e) => problems.push(e),
            }
        }
        self.audio.retain_only(&devices);
        for d in &devices {
            if let Err(e) = self.audio.ensure(d.as_deref()) {
                let name = d
                    .clone()
                    .unwrap_or_else(|| t!("standaardapparaat", "default device"));
                problems.push(format!("{name}: {e}"));
            }
        }
        self.audio_info = if self.audio.is_empty() {
            t!("geen audio-apparaat actief", "no audio device active")
        } else {
            self.audio.describe()
        };
        if let Some(p) = problems.first() {
            self.message = t!("Audio: {p}", "Audio: {p}");
        }
        self.preload_all(); // clips op de rates van de (nieuwe) kaarten zetten
    }

    /// Start een audio-cue op de juiste kaart en kanalen. Geeft een probleem terug als het mislukt.
    pub(super) fn play_audio_cue(
        &mut self,
        id: showmate_core::CueId,
        cue: &showmate_core::Cue,
    ) -> Option<String> {
        if cue.media.trim().is_empty() {
            return Some(t!("geen bestand opgegeven", "no file specified"));
        }
        let route = match self.audio_route(&cue.output) {
            Ok(r) => r,
            Err(e) => return Some(e),
        };
        let rate = match self.audio.ensure(route.device.as_deref()) {
            Ok(stream) => stream.sample_rate(),
            Err(e) => return Some(e.to_string()),
        };
        let path = self.resolve_media(&cue.media);
        match self.cache.status_at(&path, rate) {
            showmate_audio::ClipStatus::Ready(clip) => {
                self.engine.set_media_duration(id, clip.duration());
                let params = VoiceParams {
                    id: id.0,
                    clip,
                    gain_db: cue.volume_db,
                    fade_in: cue.fade_in,
                    fade_out: cue.fade_out,
                    looping: cue.looping,
                    max_duration: if cue.looping { None } else { cue.duration },
                    out_left: route.left,
                    out_right: route.right,
                };
                self.audio
                    .play(route.device.as_deref(), params)
                    .err()
                    .map(|e| e.to_string())
            }
            showmate_audio::ClipStatus::Loading => {
                Some(t!("{} wordt nog geladen", "{} is still loading", cue.media))
            }
            showmate_audio::ClipStatus::Failed(e) => Some(format!("{}: {e}", cue.media)),
            showmate_audio::ClipStatus::Unknown => {
                self.cache.request(&path, &self.rates());
                Some(t!(
                    "{} was nog niet geladen",
                    "{} had not been loaded yet",
                    cue.media
                ))
            }
        }
    }

    // ---- cue-eigenschap: uitgang kiezen ----------------------------------------------------

    pub fn edit_output(&mut self, name: &str) {
        if name.ends_with(super::io::UNDEFINED) {
            return; // de weergave van een niet-bestaande uitgang is geen keuze
        }
        let name = name.to_string();
        self.edit(|c| c.output = name);
        let audio = self
            .selected
            .is_some_and(|i| self.engine.show().cues[i].kind == CueKind::Audio);
        if audio {
            self.warm_audio();
        }
        self.fill_inspector = true;
    }

    // ---- namen die in cues voorkomen meenemen bij hernoemen -----------------------------------

    fn rename_output_references(&mut self, kind: CueKind, old: &str, new: &str) {
        if old == new || old.is_empty() {
            return;
        }
        for i in 0..self.engine.show().cues.len() {
            let c = &self.engine.show().cues[i];
            if c.kind == kind && c.output == old {
                let new = new.to_string();
                let _ = self.engine.update_cue(i, |c| c.output = new);
                self.dirty = true;
            }
        }
    }

    fn unique_name(existing: impl Iterator<Item = String>, base: &str) -> String {
        let names: Vec<String> = existing.collect();
        let mut n = names.len() + 1;
        while names.iter().any(|x| *x == format!("{base} {n}")) {
            n += 1;
        }
        format!("{base} {n}")
    }

    // ---- instellingen: audio-uitgangen --------------------------------------------------------

    pub fn audio_out_add(&mut self) {
        let name = Self::unique_name(
            self.settings.audio.outputs.iter().map(|o| o.name.clone()),
            &t!("Uitgang", "Output"),
        );
        self.settings.audio.outputs.push(AudioOutputDef {
            name,
            device: None,
            left: 1,
            right: 2,
        });
        self.fill_settings = true;
    }

    pub fn audio_out_remove(&mut self, index: i32) {
        if let Ok(i) = usize::try_from(index)
            && i < self.settings.audio.outputs.len()
        {
            self.settings.audio.outputs.remove(i);
            self.fill_settings = true;
        }
    }

    pub fn audio_out_name(&mut self, index: i32, text: &str) {
        let Some(o) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.audio.outputs.get_mut(i))
        else {
            return;
        };
        let old = std::mem::replace(&mut o.name, text.to_string());
        self.rename_output_references(CueKind::Audio, &old, text);
        self.fill_names = true;
    }

    /// Kiest de geluidskaart: index 0 = standaardapparaat, daarna de lijst uit `device_labels()`.
    pub fn audio_out_device(&mut self, index: i32, device_index: i32) {
        let device = if device_index <= 0 {
            None
        } else {
            usize::try_from(device_index)
                .ok()
                .and_then(|d| self.device_labels().get(d).cloned())
        };
        let Some(o) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.audio.outputs.get_mut(i))
        else {
            return;
        };
        o.device = device;
        self.warm_audio();
        self.fill_names = true;
    }

    /// `side`: 0 = links, 1 = rechts. `channel_index` is 0-gebaseerd.
    pub fn audio_out_channel(&mut self, index: i32, side: i32, channel_index: i32) {
        let Some(o) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.audio.outputs.get_mut(i))
        else {
            return;
        };
        let ch = u16::try_from(channel_index.max(0) + 1).unwrap_or(1);
        if side == 0 {
            o.left = ch;
        } else {
            o.right = ch;
        }
        self.fill_names = true;
    }

    /// Speelt een testtoon op deze uitgang (de gekoppelde kaart wordt zo nodig geopend).
    pub fn audio_out_test(&mut self, index: i32) {
        let Some(def) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.audio.outputs.get(i))
            .cloned()
        else {
            return;
        };
        let stream = match self.audio.ensure(def.device.as_deref()) {
            Ok(s) => s,
            Err(e) => {
                self.message = format!("{}: {e}", def.name);
                self.fill_names = true;
                return;
            }
        };
        let channels = usize::from(stream.channels());
        let (left, right) = (
            usize::from(def.left.max(1)) - 1,
            usize::from(def.right.max(1)) - 1,
        );
        if left >= channels && right >= channels {
            self.message = t!(
                "{}: deze kaart heeft maar {channels} kanalen",
                "{}: this device only has {channels} channels",
                def.name
            );
            return;
        }
        let clip = Arc::new(AudioClip::test_tone(stream.sample_rate(), 0.8, 440.0));
        let result = self.audio.play(
            def.device.as_deref(),
            VoiceParams {
                id: u64::MAX - u64::try_from(index).unwrap_or(0),
                clip,
                gain_db: 0.0,
                fade_in: Duration::ZERO,
                fade_out: Duration::ZERO,
                looping: false,
                max_duration: None,
                out_left: left,
                out_right: right,
            },
        );
        self.audio_info = self.audio.describe();
        if let Err(e) = result {
            self.message = e.to_string();
        }
        self.fill_names = true;
    }

    pub fn audio_devices_refresh(&mut self) {
        self.audio_devices = showmate_audio::list_output_devices();
        self.fill_settings = true;
    }

    // ---- instellingen: video-uitgangen --------------------------------------------------------

    pub fn video_out_add(&mut self) {
        let name = Self::unique_name(
            self.settings.video.outputs.iter().map(|o| o.name.clone()),
            &t!("Uitgang", "Output"),
        );
        self.settings.video.outputs.push(VideoOutputDef {
            name,
            monitors: vec![0],
        });
        self.fill_settings = true;
    }

    pub fn video_out_remove(&mut self, index: i32) {
        if let Ok(i) = usize::try_from(index)
            && i < self.settings.video.outputs.len()
        {
            self.settings.video.outputs.remove(i);
            self.fill_settings = true;
        }
    }

    pub fn video_out_name(&mut self, index: i32, text: &str) {
        let Some(o) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.video.outputs.get_mut(i))
        else {
            return;
        };
        let old = std::mem::replace(&mut o.name, text.to_string());
        self.rename_output_references(CueKind::Video, &old, text);
        self.fill_names = true;
    }

    pub fn video_out_monitor(&mut self, index: i32, monitor: i32, on: bool) {
        let Some(o) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.video.outputs.get_mut(i))
        else {
            return;
        };
        let Ok(m) = usize::try_from(monitor) else {
            return;
        };
        o.monitors.retain(|x| *x != m);
        if on {
            o.monitors.push(m);
            o.monitors.sort_unstable();
        }
    }

    // ---- UI-rijen ---------------------------------------------------------------------------

    pub(super) fn audio_out_rows(&self) -> Vec<AudioOutRow> {
        let devices = &self.audio_devices;
        self.settings
            .audio
            .outputs
            .iter()
            .map(|o| {
                // 0 = standaardapparaat; onbekende (ontbrekende) apparaten staan achteraan.
                let device_index = match &o.device {
                    None => 0,
                    Some(d) => devices
                        .iter()
                        .position(|x| x == d)
                        .map_or(devices.len() + 1, |p| p + 1),
                };
                AudioOutRow {
                    name: o.name.as_str().into(),
                    device_index: device_index as i32,
                    left: i32::from(o.left.max(1)) - 1,
                    right: i32::from(o.right.max(1)) - 1,
                }
            })
            .collect()
    }

    pub(super) fn video_out_rows(&self) -> Vec<VideoOutRow> {
        self.settings
            .video
            .outputs
            .iter()
            .map(|o| VideoOutRow {
                name: o.name.as_str().into(),
                m1: o.monitors.contains(&0),
                m2: o.monitors.contains(&1),
                m3: o.monitors.contains(&2),
                m4: o.monitors.contains(&3),
            })
            .collect()
    }

    /// Namen voor de keuzelijst van de geluidskaarten (index 0 = standaardapparaat), inclusief
    /// ontbrekende maar nog gekoppelde apparaten.
    pub(super) fn device_labels(&self) -> Vec<String> {
        let mut labels = vec![t!(
            "(standaardapparaat van het systeem)",
            "(system default device)"
        )];
        labels.extend(self.audio_devices.iter().cloned());
        for d in self
            .settings
            .audio
            .outputs
            .iter()
            .filter_map(|o| o.device.as_ref())
        {
            if !self.audio_devices.contains(d) && !labels.contains(d) {
                labels.push(d.clone());
            }
        }
        labels
    }

    pub(super) fn fill_outputs_ui(&self, ui: &SettingsWindow, rebuild_rows: bool) {
        let channel_note =
            {
                let mut notes: Vec<String> = Vec::new();
                for o in &self.settings.audio.outputs {
                    if let Some(d) = o.device.as_ref().filter(|d| {
                        !self.audio_devices.is_empty() && !self.audio_devices.contains(d)
                    }) {
                        notes.push(t!(
                            "{}: kaart '{}' niet gevonden",
                            "{}: device '{}' not found",
                            o.name,
                            d
                        ));
                    }
                    let needed = usize::from(o.left.max(o.right));
                    let channels = self
                        .audio
                        .get(o.device.as_deref())
                        .map(|a| usize::from(a.channels()));
                    if let Some(channels) = channels.filter(|c| needed > *c) {
                        notes.push(t!(
                            "{}: kaart heeft maar {} kanalen",
                            "{}: device only has {} channels",
                            o.name,
                            channels
                        ));
                    }
                }
                notes.join("  ·  ")
            };
        ui.set_audio_info(
            if channel_note.is_empty() {
                self.audio_info.clone()
            } else {
                format!("{}  —  {channel_note}", self.audio_info)
            }
            .into(),
        );
        ui.set_audio_devices(super::io::strings(self.device_labels()));
        if rebuild_rows {
            ui.set_audio_outs(ModelRc::new(VecModel::from(self.audio_out_rows())));
            ui.set_video_outs(ModelRc::new(VecModel::from(self.video_out_rows())));
        }
    }
}
