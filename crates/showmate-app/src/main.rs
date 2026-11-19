#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

use showmate_core::demo::demo_show;
use showmate_core::{CueKind, CueState, Engine, StartMode, SystemClock, format_timecode};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

slint::include_modules!();

const FPS: u32 = 25;

fn kind_label(kind: CueKind) -> &'static str {
    match kind {
        CueKind::Audio => "Audio",
        CueKind::Video => "Video",
        CueKind::Osc => "OSC",
        CueKind::Midi => "MIDI",
    }
}

fn status_label(state: CueState) -> &'static str {
    match state {
        CueState::Waiting | CueState::Scheduled { .. } => "Wachtend",
        CueState::Active { .. } => "Actief",
        CueState::Finished => "Gespeeld",
        CueState::Stopped => "Gestopt",
    }
}

fn refresh(ui: &MainWindow, engine: &Engine<SystemClock>) {
    let show = engine.show();
    let rows: Vec<CueRow> = show
        .cues
        .iter()
        .zip(engine.states())
        .enumerate()
        .map(|(i, (cue, state))| CueRow {
            number: i as i32 + 1,
            name: cue.name.as_str().into(),
            kind: kind_label(cue.kind).into(),
            start: match cue.start {
                StartMode::Go => "GO",
                StartMode::Auto => "Auto",
            }
            .into(),
            delay: format!("+{:.1} s", cue.delay.as_secs_f64()).into(),
            status: status_label(*state).into(),
            active: matches!(state, CueState::Active { .. }),
            done: matches!(state, CueState::Finished),
        })
        .collect();
    ui.set_rows(ModelRc::new(VecModel::from(rows)));
    ui.set_timecode(format_timecode(engine.show_time(), FPS).into());
    ui.set_paused(engine.is_paused());

    match engine.next_cue() {
        Some(next) => {
            ui.set_next_cue(format!("{} • {}", next.index + 1, show.cues[next.index].name).into());
            ui.set_next_eta(
                match next.starts_in {
                    Some(d) => format!(
                        "(over {})",
                        format_timecode(d, FPS).rsplit_once('.').map_or("", |p| p.0)
                    ),
                    None => "(wacht op GO)".to_string(),
                }
                .into(),
            );
        }
        None => {
            ui.set_next_cue(SharedString::from("—"));
            ui.set_next_eta(SharedString::new());
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let engine = Rc::new(RefCell::new(Engine::new(SystemClock::new(), demo_show())));
    refresh(&ui, &engine.borrow());

    // Later verhuist de engine naar een eigen thread; voor de spike tikt hij op de UI-timer.
    let timer = Timer::default();
    {
        let (weak, engine) = (ui.as_weak(), engine.clone());
        timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
            let mut engine = engine.borrow_mut();
            engine.tick();
            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &engine);
            }
        });
    }

    ui.on_go({
        let engine = engine.clone();
        move || {
            engine.borrow_mut().go();
        }
    });
    ui.on_pause({
        let engine = engine.clone();
        move || {
            engine.borrow_mut().toggle_pause();
        }
    });
    ui.on_stop({
        let engine = engine.clone();
        move || {
            engine.borrow_mut().stop();
        }
    });

    ui.run()
}
