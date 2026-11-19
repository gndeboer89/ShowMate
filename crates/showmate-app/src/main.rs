#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod keys;
mod settings;
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
    app.borrow().apply_language();
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
    wire!(on_language_selected, |st, i: i32| st.set_language(i));
    wire!(on_key_capture, |st, i: i32| st.key_capture(i));
    wire!(on_key_clear, |st, i: i32| st.key_clear(i));
    wire!(on_key_reset, |st, i: i32| st.key_reset(i));
    wire!(on_keys_reset_all, |st| st.keys_reset_all());
    {
        let (weak, app) = (ui.as_weak(), app.clone());
        ui.on_key_event(move |text, ctrl, shift, alt, meta| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let mut st = app.borrow_mut();
            let handled = st.handle_key(&text, ctrl, shift, alt, meta);
            st.after_edit(&ui);
            handled
        });
    }
    wire!(on_undo, |st| st.undo());
    wire!(on_redo, |st| st.redo());
    wire!(on_duplicate_cue, |st| st.duplicate_selected());
    wire!(on_panic, |st| st.panic_stop());
    wire!(on_reset_show, |st| st.reset_show());
    wire!(on_edit_group_mode, |st, i: i32| st.edit_group_mode(i));
    wire!(on_edit_mute, |st, v: bool| st.edit_mute(v));
    wire!(on_edit_osc_dest, |st, v: slint::SharedString| st
        .edit_osc_dest(&v));
    wire!(on_edit_osc_command, |st, v: slint::SharedString| st
        .edit_osc_command(&v));
    wire!(on_edit_osc_address, |st, v: slint::SharedString| st
        .edit_osc_address(&v));
    wire!(on_edit_osc_args, |st, v: slint::SharedString| st
        .edit_osc_args(&v));
    wire!(on_edit_midi_kind, |st, i: i32| st.edit_midi_kind(i));
    wire!(on_edit_midi_num, |st, f: i32, v: slint::SharedString| st
        .edit_midi_num(f, &v));
    wire!(on_test_cue, |st| st.test_cue());
    wire!(on_open_settings, |st| st.open_settings());
    wire!(on_close_settings, |st| st.close_settings());
    wire!(
        on_dest_edited,
        |st, i: i32, f: i32, v: slint::SharedString| st.dest_edited(i, f, &v)
    );
    wire!(on_dest_add, |st| st.dest_add());
    wire!(on_dest_remove, |st, i: i32| st.dest_remove(i));
    wire!(
        on_cmd_edited,
        |st, i: i32, f: i32, v: slint::SharedString| st.cmd_edited(i, f, &v)
    );
    wire!(on_cmd_add, |st| st.cmd_add());
    wire!(on_cmd_remove, |st, i: i32| st.cmd_remove(i));
    wire!(on_midi_selected, |st, v: slint::SharedString| st
        .midi_selected(&v));
    wire!(on_midi_refresh, |st| st.midi_refresh());
    wire!(on_edit_delay, |st, v: slint::SharedString| st
        .edit_delay(&v));
    wire!(on_edit_duration, |st, v: slint::SharedString| st
        .edit_duration(&v));

    // Bij afsluiten niet-opgeslagen werk bewaren (wordt bij de volgende start hersteld).
    {
        let app = app.clone();
        ui.window().on_close_requested(move || {
            app.borrow_mut().on_exit();
            slint::CloseRequestResponse::HideWindow
        });
    }

    ui.run()
}
