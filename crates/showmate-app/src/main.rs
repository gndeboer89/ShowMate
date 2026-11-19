#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod state;

use slint::{ComponentHandle, Timer, TimerMode};
use state::AppState;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let app = Rc::new(RefCell::new(AppState::open_default()));
    app.borrow_mut().refresh_all(&ui);

    // Later verhuist de engine naar een eigen thread; voor nu tikt hij op de UI-timer.
    let timer = Timer::default();
    {
        let (weak, app) = (ui.as_weak(), app.clone());
        timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
            if let Some(ui) = weak.upgrade() {
                app.borrow_mut().tick(&ui);
            }
        });
    }

    // Elke callback: muteer de staat en ververs daarna de UI.
    macro_rules! wire {
        ($on:ident, |$st:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let (weak, app) = (ui.as_weak(), app.clone());
            ui.$on(move |$($arg: $ty),*| {
                if let Some(ui) = weak.upgrade() {
                    let mut $st = app.borrow_mut();
                    $body;
                    $st.after_edit(&ui);
                }
            });
        }};
    }

    wire!(on_go, |st| st.go());
    wire!(on_pause, |st| st.toggle_pause());
    wire!(on_stop, |st| st.stop());
    wire!(on_select, |st, i: i32| st.select(i));
    wire!(on_add_cue, |st, kind: slint::SharedString| st
        .add_cue(&kind));
    wire!(on_remove_cue, |st| st.remove_selected());
    wire!(on_move_up, |st| st.move_selected(true));
    wire!(on_move_down, |st| st.move_selected(false));
    wire!(on_save, |st| st.save());
    wire!(on_edit_name, |st, v: slint::SharedString| st
        .edit(|c| c.name = v.to_string()));
    wire!(on_edit_media, |st, v: slint::SharedString| st
        .edit_media(&v));
    wire!(on_edit_volume, |st, v: slint::SharedString| st
        .edit_volume(&v));
    wire!(on_edit_fade_in, |st, v: slint::SharedString| st
        .edit_fade(&v, true));
    wire!(on_edit_fade_out, |st, v: slint::SharedString| st
        .edit_fade(&v, false));
    wire!(on_edit_output, |st, v: slint::SharedString| st
        .edit(|c| c.output = v.to_string()));
    wire!(on_edit_notes, |st, v: slint::SharedString| st
        .edit(|c| c.notes = v.to_string()));
    wire!(on_edit_start, |st, auto: bool| st.edit(|c| {
        c.start = if auto {
            showmate_core::StartMode::Auto
        } else {
            showmate_core::StartMode::Go
        };
    }));
    wire!(on_edit_looping, |st, v: bool| st.edit(|c| c.looping = v));
    wire!(on_edit_delay, |st, v: slint::SharedString| st
        .edit_delay(&v));
    wire!(on_edit_duration, |st, v: slint::SharedString| st
        .edit_duration(&v));

    ui.run()
}
