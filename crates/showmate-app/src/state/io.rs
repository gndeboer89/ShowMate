//! OSC, MIDI en het instellingenvenster.

use super::{AppState, kind_key};
use crate::settings::{self, OscCommand, OscDestination};
use crate::{CommandRow, DestRow, MainWindow, SettingsWindow};
use showmate_core::{Cue, CueKind, MidiKind};
use showmate_io::{MidiOut, format_args as format_osc_args, list_midi_ports, parse_args};
use slint::{ModelRc, SharedString, VecModel};

const MIDI_NONE: &str = "(geen)";
/// Achtervoegsel voor een cue-uitgang die niet (meer) bestaat in de instellingen.
pub(super) const UNDEFINED: &str = "  (niet gedefinieerd)";
const OWN_COMMAND: &str = "(eigen)";
const MIDI_KINDS: [(MidiKind, &str); 4] = [
    (MidiKind::NoteOn, "Note On"),
    (MidiKind::NoteOff, "Note Off"),
    (MidiKind::ControlChange, "Control Change"),
    (MidiKind::ProgramChange, "Program Change"),
];

pub(super) fn strings(items: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
}

impl AppState {
    // ---- verbinding ----------------------------------------------------

    pub(super) fn connect_midi(&mut self) {
        self.midi = None;
        self.midi_status.clear();
        if let Some(name) = self.settings.midi.output.clone() {
            match MidiOut::connect(&name) {
                Ok(m) => self.midi = Some(m),
                Err(e) => self.midi_status = e,
            }
        }
    }

    // ---- labels voor de cue-lijst -----------------------------------------

    fn effective_destination(&self, cue: &Cue) -> Option<&OscDestination> {
        let dests = &self.settings.osc.destinations;
        if cue.output.trim().is_empty() {
            dests.first()
        } else {
            dests.iter().find(|d| d.name == cue.output)
        }
    }

    /// (media-kolom, output-kolom) voor OSC/MIDI-cues.
    pub(super) fn io_labels(&self, cue: &Cue) -> Option<(String, String)> {
        match cue.kind {
            CueKind::Osc => {
                let media = if cue.osc_command.is_empty() {
                    format!("{} {}", cue.osc_address, cue.osc_args)
                        .trim()
                        .to_string()
                } else {
                    format!("cmd: {}", cue.osc_command)
                };
                let dest = self
                    .effective_destination(cue)
                    .map_or(cue.output.as_str(), |d| d.name.as_str());
                Some((media, format!("OSC ({dest})")))
            }
            CueKind::Midi => {
                let m = &cue.midi;
                let kind = MIDI_KINDS
                    .iter()
                    .find(|(k, _)| *k == m.kind)
                    .map_or("", |(_, n)| *n);
                let media = format!("{kind} ch{} {} {}", m.channel, m.data1, m.data2);
                let port = self.midi.as_ref().map_or("MIDI", |p| p.name());
                Some((media, port.to_string()))
            }
            CueKind::Group => Some((format!("{} cues", cue.span), String::new())),
            _ => None,
        }
    }

    // ---- afvuren -----------------------------------------------------------

    fn osc_message(&self, cue: &Cue) -> Result<(String, String), String> {
        if cue.osc_command.is_empty() {
            return Ok((cue.osc_address.clone(), cue.osc_args.clone()));
        }
        self.settings
            .osc
            .commands
            .iter()
            .find(|c| c.name == cue.osc_command)
            .map(|c| (c.address.clone(), c.args.clone()))
            .ok_or_else(|| {
                t!(
                    "commando '{}' bestaat niet (meer)",
                    "command '{}' no longer exists",
                    cue.osc_command
                )
            })
    }

    /// Verstuurt het OSC- of MIDI-bericht van `cue`.
    pub(super) fn fire_io(&mut self, cue: &Cue) -> Result<(), String> {
        match cue.kind {
            CueKind::Osc => {
                let (address, args_text) = self.osc_message(cue)?;
                let dest = self.effective_destination(cue).cloned().ok_or_else(|| {
                    if cue.output.trim().is_empty() {
                        t!(
                            "geen OSC-bestemming ingesteld (Instellingen)",
                            "no OSC destination configured (Settings)"
                        )
                    } else {
                        t!(
                            "OSC-bestemming '{}' bestaat niet",
                            "OSC destination '{}' does not exist",
                            cue.output
                        )
                    }
                })?;
                let args =
                    parse_args(&args_text).map_err(|e| t!("argumenten: {e}", "arguments: {e}"))?;
                self.osc.send(&dest.host, dest.port, &address, args)
            }
            CueKind::Midi => match self.midi.as_mut() {
                Some(m) => m.send(&cue.midi),
                None => Err(t!(
                    "geen MIDI-uitgang gekozen (Instellingen)",
                    "no MIDI output selected (Settings)"
                )),
            },
            _ => Ok(()),
        }
    }

    /// Stuurt het bericht van de geselecteerde OSC/MIDI-cue meteen, zonder show.
    pub fn test_cue(&mut self) {
        let Some(i) = self.selected else { return };
        let cue = self.engine.show().cues[i].clone();
        self.message = match self.fire_io(&cue) {
            Ok(()) => t!("Test verstuurd: cue {}", "Test sent: cue {}", i + 1),
            Err(e) => format!("Cue {}: {e}", i + 1),
        };
    }

    // ---- cue-eigenschappen ---------------------------------------------------

    pub fn edit_mute(&mut self, mute: bool) {
        self.edit(|c| c.mute_audio = mute);
    }

    pub fn edit_osc_dest(&mut self, name: &str) {
        let name = name.to_string();
        self.edit(|c| c.output = name);
    }

    pub fn edit_osc_command(&mut self, name: &str) {
        let name = if name == OWN_COMMAND { "" } else { name }.to_string();
        self.edit(|c| c.osc_command = name);
        self.fill_inspector = true; // adres/argumenten worden (de)activeerd
    }

    pub fn edit_osc_address(&mut self, text: &str) {
        let text = text.to_string();
        self.edit(|c| c.osc_address = text);
    }

    pub fn edit_osc_args(&mut self, text: &str) {
        match parse_args(text) {
            Ok(_) => {
                self.osc_args_error.clear();
                let text = text.to_string();
                self.edit(|c| c.osc_args = text);
            }
            Err(e) => self.osc_args_error = e,
        }
    }

    pub fn edit_midi_kind(&mut self, index: i32) {
        if let Some((kind, _)) = usize::try_from(index).ok().and_then(|i| MIDI_KINDS.get(i)) {
            let kind = *kind;
            self.edit(|c| c.midi.kind = kind);
        }
    }

    /// `field`: 0 = kanaal, 1 = data 1, 2 = data 2.
    pub fn edit_midi_num(&mut self, field: i32, text: &str) {
        let (lo, hi) = if field == 0 { (1, 16) } else { (0, 127) };
        match text.trim().parse::<u8>() {
            Ok(v) if (lo..=hi).contains(&v) => {
                self.midi_ok = true;
                self.edit(|c| match field {
                    0 => c.midi.channel = v,
                    1 => c.midi.data1 = v,
                    _ => c.midi.data2 = v,
                });
            }
            _ => self.midi_ok = false,
        }
    }

    // ---- instellingenvenster -------------------------------------------------------

    /// Zet taal van Rust-teksten en UI volgens de instellingen.
    pub fn apply_language(&self) {
        let english = self.settings.language == "en";
        crate::i18n::set_english(english);
        if let Err(e) = slint::select_bundled_translation(if english { "en" } else { "nl" }) {
            eprintln!("vertaling: {e}");
        }
    }

    pub fn set_language(&mut self, index: i32) {
        self.settings.language = if index == 1 { "en" } else { "nl" }.into();
        self.apply_language();
        self.fill_settings = true;
        self.fill_inspector = true;
    }

    // ---- videoschermen ------------------------------------------------------------

    pub fn video_fullscreen_toggled(&mut self, on: bool) {
        self.settings.video.fullscreen = on;
    }

    pub fn mpv_path_edited(&mut self, text: &str) {
        let t = text.trim();
        self.settings.video.mpv_path = (!t.is_empty()).then(|| t.into());
    }

    pub fn browse_mpv(&mut self) {
        let filters: [crate::dialogs::Filter; 1] = [("mpv", &["exe"])];
        if let Some(p) =
            crate::dialogs::pick_file(crate::i18n::tr("mpv kiezen", "Choose mpv"), &filters, None)
        {
            self.settings.video.mpv_path = Some(p);
            self.fill_settings = true;
        }
    }

    /// Start de videospelers opnieuw als de videoinstellingen zijn gewijzigd.
    fn apply_video_settings(&mut self) {
        let cfg = self.settings.video.to_config();
        let key = format!("{cfg:?}");
        if key == self.video_cfg_key {
            return;
        }
        self.video_cfg_key = key;
        // De oude VideoOutput sluit bij het vervangen zijn mpv-processen.
        self.video = showmate_video::VideoOutput::new(cfg);
        self.video_info = match self.video.mpv() {
            Ok(p) => format!("mpv: {}", p.display()),
            Err(e) => e.to_string(),
        };
        if let Some(e) = self.warm_video() {
            self.message = e;
        }
    }

    /// Bereidt het instellingenvenster voor (de pagina kiest `main.rs`).
    pub fn open_settings(&mut self) {
        self.settings_note.clear();
        self.midi_refresh();
        self.audio_devices_refresh();
        self.fill_settings = true;
    }

    /// Past de instellingen toe en bewaart ze (bij sluiten van het instellingenvenster).
    pub fn close_settings(&mut self) {
        self.capturing = None;
        self.osc.forget();
        self.connect_midi();
        self.apply_video_settings();
        self.warm_audio();
        self.settings_note.clear();
        self.message = match settings::save(&self.app_dir, &self.settings) {
            Ok(()) if self.midi_status.is_empty() => {
                t!("Instellingen opgeslagen", "Settings saved")
            }
            Ok(()) => t!(
                "Instellingen opgeslagen; {}",
                "Settings saved; {}",
                self.midi_status
            ),
            Err(e) => t!(
                "Instellingen opslaan mislukt: {e}",
                "Saving settings failed: {e}"
            ),
        };
        self.fill_settings = true;
        self.fill_inspector = true;
    }

    /// Past in alle cues een naam aan die naar een hernoemde bestemming/commando verwees.
    fn rename_references(&mut self, old: &str, new: &str, command: bool) {
        if old == new || old.is_empty() {
            return;
        }
        for i in 0..self.engine.show().cues.len() {
            let cue = &self.engine.show().cues[i];
            let hit = if command {
                cue.kind == CueKind::Osc && cue.osc_command == old
            } else {
                cue.kind == CueKind::Osc && cue.output == old
            };
            if hit {
                let new = new.to_string();
                let _ = self.engine.update_cue(i, |c| {
                    if command {
                        c.osc_command = new
                    } else {
                        c.output = new
                    }
                });
                self.dirty = true;
            }
        }
    }

    /// `field`: 0 = naam, 1 = host, 2 = poort.
    pub fn dest_edited(&mut self, index: i32, field: i32, text: &str) {
        let Some(i) = usize::try_from(index)
            .ok()
            .filter(|i| *i < self.settings.osc.destinations.len())
        else {
            return;
        };
        match field {
            0 => {
                let old = std::mem::replace(
                    &mut self.settings.osc.destinations[i].name,
                    text.to_string(),
                );
                self.rename_references(&old, text, false);
            }
            1 => self.settings.osc.destinations[i].host = text.trim().to_string(),
            _ => match text.trim().parse::<u16>() {
                Ok(p) if p > 0 => {
                    self.settings.osc.destinations[i].port = p;
                    self.settings_note.clear();
                }
                _ => {
                    self.settings_note = t!(
                        "Poort moet een getal van 1 t/m 65535 zijn",
                        "Port must be a number from 1 to 65535"
                    )
                }
            },
        }
        self.fill_names = true;
    }

    pub fn dest_add(&mut self) {
        let mut n = self.settings.osc.destinations.len() + 1;
        while self
            .settings
            .osc
            .destinations
            .iter()
            .any(|d| d.name == t!("Bestemming {n}", "Destination {n}"))
        {
            n += 1;
        }
        self.settings.osc.destinations.push(OscDestination {
            name: t!("Bestemming {n}", "Destination {n}"),
            host: "127.0.0.1".into(),
            port: 8000,
        });
        self.fill_settings = true;
    }

    pub fn dest_remove(&mut self, index: i32) {
        if let Ok(i) = usize::try_from(index)
            && i < self.settings.osc.destinations.len()
        {
            self.settings.osc.destinations.remove(i);
            self.fill_settings = true;
        }
    }

    /// `field`: 0 = naam, 1 = adres, 2 = argumenten.
    pub fn cmd_edited(&mut self, index: i32, field: i32, text: &str) {
        let Some(i) = usize::try_from(index)
            .ok()
            .filter(|i| *i < self.settings.osc.commands.len())
        else {
            return;
        };
        match field {
            0 => {
                let old =
                    std::mem::replace(&mut self.settings.osc.commands[i].name, text.to_string());
                self.rename_references(&old, text, true);
            }
            1 => self.settings.osc.commands[i].address = text.trim().to_string(),
            _ => {
                self.settings.osc.commands[i].args = text.to_string();
                self.settings_note = match parse_args(text) {
                    Ok(_) => String::new(),
                    Err(e) => t!("Argumenten: {e}", "Arguments: {e}"),
                };
            }
        }
        self.fill_names = true;
    }

    pub fn cmd_add(&mut self) {
        let mut n = self.settings.osc.commands.len() + 1;
        while self
            .settings
            .osc
            .commands
            .iter()
            .any(|c| c.name == t!("Commando {n}", "Command {n}"))
        {
            n += 1;
        }
        self.settings.osc.commands.push(OscCommand {
            name: t!("Commando {n}", "Command {n}"),
            address: "/".into(),
            args: String::new(),
        });
        self.fill_settings = true;
    }

    pub fn cmd_remove(&mut self, index: i32) {
        if let Ok(i) = usize::try_from(index)
            && i < self.settings.osc.commands.len()
        {
            self.settings.osc.commands.remove(i);
            self.fill_settings = true;
        }
    }

    pub fn midi_selected(&mut self, name: &str) {
        self.settings.midi.output = (name != MIDI_NONE).then(|| name.to_string());
        self.midi_status.clear();
        self.fill_names = true;
    }

    pub fn midi_refresh(&mut self) {
        match list_midi_ports() {
            Ok(ports) => {
                self.midi_ports = ports;
                self.midi_list_error.clear();
            }
            Err(e) => {
                self.midi_ports.clear();
                self.midi_list_error = e;
            }
        }
        self.fill_names = true;
    }

    // ---- UI vullen -----------------------------------------------------------------

    /// Lijsten en vlaggen die het instellingenvenster en de inspector-keuzelijsten voeden.
    /// Lijsten in het hoofdvenster die van de instellingen afhangen (inspector en menu's).
    pub(super) fn fill_main_lists(&self, ui: &MainWindow) {
        let s = &self.settings;
        ui.set_dest_names(strings(s.osc.destinations.iter().map(|d| d.name.clone())));
        ui.set_cmd_names(strings(
            std::iter::once(OWN_COMMAND.to_string())
                .chain(s.osc.commands.iter().map(|c| c.name.clone())),
        ));
        ui.set_recent(strings(self.recent_labels()));
        ui.set_key_hints(strings(
            crate::keys::ALL.iter().map(|a| self.shortcuts.key_of(*a)),
        ));
    }

    /// Vult het instellingenvenster.
    pub(super) fn fill_settings_ui(&self, ui: &SettingsWindow, rebuild_rows: bool) {
        let s = &self.settings;
        ui.set_copy_media(s.copy_media);
        ui.set_video_fullscreen(s.video.fullscreen);
        ui.set_mpv_path(
            s.video
                .mpv_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
                .into(),
        );
        ui.set_video_status(
            match self.video.mpv() {
                Ok(p) => format!("mpv: {}", p.display()),
                Err(e) => e.to_string(),
            }
            .into(),
        );
        ui.set_keys(ModelRc::new(VecModel::from(self.key_rows())));
        ui.set_language_index(i32::from(s.language == "en"));
        ui.set_note(self.settings_note.as_str().into());

        self.fill_outputs_ui(ui, rebuild_rows);

        let mut ports = vec![MIDI_NONE.to_string()];
        ports.extend(self.midi_ports.iter().cloned());
        // Een eerder gekozen poort die nu ontbreekt blijft zichtbaar.
        if let Some(sel) = s.midi.output.as_ref().filter(|p| !ports.contains(p)) {
            ports.push(sel.clone());
        }
        let current = s.midi.output.as_deref().unwrap_or(MIDI_NONE);
        ui.set_midi_index(ports.iter().position(|p| p == current).unwrap_or(0) as i32);
        ui.set_midi_ports(strings(ports));
        ui.set_midi_status(
            if !self.midi_list_error.is_empty() {
                t!(
                    "MIDI niet beschikbaar: {}",
                    "MIDI not available: {}",
                    self.midi_list_error
                )
            } else if !self.midi_status.is_empty() {
                self.midi_status.clone()
            } else if self.midi.is_some() {
                t!("verbonden", "connected")
            } else {
                String::new()
            }
            .into(),
        );
        if rebuild_rows {
            // Volledig vervangen: Slint maakt dan alle rij-widgets opnieuw (zo blijven er
            // geen verouderde tekstvelden hangen na verwijderen).
            ui.set_dests(ModelRc::new(VecModel::from(
                s.osc
                    .destinations
                    .iter()
                    .map(|d| DestRow {
                        name: d.name.as_str().into(),
                        host: d.host.as_str().into(),
                        port: d.port.to_string().into(),
                    })
                    .collect::<Vec<_>>(),
            )));
            ui.set_cmds(ModelRc::new(VecModel::from(
                s.osc
                    .commands
                    .iter()
                    .map(|c| CommandRow {
                        name: c.name.as_str().into(),
                        address: c.address.as_str().into(),
                        args: c.args.as_str().into(),
                    })
                    .collect::<Vec<_>>(),
            )));
        }
    }

    /// OSC/MIDI-velden van de inspector.
    pub(super) fn fill_io_inspector(&self, ui: &MainWindow, cue: &Cue) {
        ui.set_insp_kind(kind_key(cue.kind).into());
        ui.set_insp_child(
            self.selected
                .is_some_and(|i| self.engine.show().is_child(i)),
        );
        ui.set_insp_group_mode_index(match cue.group_mode {
            showmate_core::GroupMode::Sequence => 0,
            showmate_core::GroupMode::Parallel => 1,
            showmate_core::GroupMode::Loop => 2,
        });
        ui.set_insp_mute(cue.mute_audio);
        // Keuzelijst met de gedefinieerde uitgangen; een naam die niet (meer) bestaat blijft zichtbaar.
        let mut out_options: Vec<String> = match cue.kind {
            CueKind::Audio => self
                .settings
                .audio
                .outputs
                .iter()
                .map(|o| o.name.clone())
                .collect(),
            CueKind::Video => self
                .settings
                .video
                .outputs
                .iter()
                .map(|o| o.name.clone())
                .collect(),
            _ => Vec::new(),
        };
        let found = out_options.iter().position(|n| *n == cue.output);
        let unknown = found.is_none() && matches!(cue.kind, CueKind::Audio | CueKind::Video);
        if unknown {
            out_options.push(format!("{}{UNDEFINED}", cue.output));
        }
        ui.set_insp_out_unknown(unknown);
        ui.set_insp_out_index(found.unwrap_or(out_options.len().saturating_sub(1)) as i32);
        ui.set_insp_out_options(strings(out_options));
        let dests = &self.settings.osc.destinations;
        let dest_index = if cue.output.trim().is_empty() {
            0
        } else {
            dests
                .iter()
                .position(|d| d.name == cue.output)
                .map_or(-1, |i| i as i32)
        };
        ui.set_insp_dest_index(dest_index);
        let cmd_index = if cue.osc_command.is_empty() {
            0
        } else {
            self.settings
                .osc
                .commands
                .iter()
                .position(|c| c.name == cue.osc_command)
                .map_or(-1, |i| i as i32 + 1)
        };
        ui.set_insp_cmd_index(cmd_index);
        let using_command = !cue.osc_command.is_empty();
        let (address, args) = self.osc_message(cue).unwrap_or_default();
        ui.set_insp_osc_address(address.clone().into());
        ui.set_insp_osc_args(
            if using_command {
                args.clone()
            } else {
                cue.osc_args.clone()
            }
            .into(),
        );
        ui.set_insp_osc_custom(!using_command);
        ui.set_osc_args_error(self.osc_args_error.as_str().into());
        ui.set_insp_midi_kind_index(
            MIDI_KINDS
                .iter()
                .position(|(k, _)| *k == cue.midi.kind)
                .unwrap_or(0) as i32,
        );
        ui.set_insp_midi_channel(cue.midi.channel.to_string().into());
        ui.set_insp_midi_data1(cue.midi.data1.to_string().into());
        ui.set_insp_midi_data2(cue.midi.data2.to_string().into());
        ui.set_midi_ok(self.midi_ok);
        // Voor het label onder de adresvelden: wat er werkelijk verstuurd wordt.
        let preview = if cue.kind == CueKind::Osc {
            parse_args(&args)
                .map(|a| {
                    format!("{address} {}", format_osc_args(&a))
                        .trim()
                        .to_string()
                })
                .unwrap_or(address)
        } else {
            String::new()
        };
        ui.set_insp_osc_preview(preview.into());
    }
}
