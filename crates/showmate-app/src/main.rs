#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod dialogs;
mod keys;
mod media;
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
    wire!(on_file_new, |st| st.new_show());
    wire!(on_file_open, |st| st.open_show_dialog());
    wire!(on_file_save_as, |st| st.save_as());
    wire!(on_open_recent, |st, i: i32| st.open_recent(i));
    wire!(on_browse_media, |st| st.browse_media());
    wire!(on_copy_media_toggled, |st, v: bool| st.set_copy_media(v));
    wire!(on_video_fullscreen_toggled, |st, v: bool| st
        .video_fullscreen_toggled(v));
    wire!(on_video_screen_selected, |st, d: i32, m: i32| st
        .video_screen_selected(d, m));
    wire!(on_mpv_path_edited, |st, v: slint::SharedString| st
        .mpv_path_edited(&v));
    wire!(on_browse_mpv, |st| st.browse_mpv());
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

    // Bestanden die op het venster worden gesleept (winit-gebeurtenis; Slint heeft hier geen eigen API voor).
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
        let (weak, app) = (ui.as_weak(), app.clone());
        ui.window().on_winit_window_event(move |_, event| {
            if let WindowEvent::DroppedFile(path) = event
                && let Some(ui) = weak.upgrade()
            {
                let mut st = app.borrow_mut();
                st.add_dropped_files(vec![path.clone()]);
                st.after_edit(&ui);
            }
            EventResult::Propagate
        });
    }

    // Alleen voor tests: simuleert het slepen van bestanden (`SHOWMATE_TEST_DROP=pad1;pad2`).
    if let Ok(list) = std::env::var("SHOWMATE_TEST_DROP") {
        let (weak, app) = (ui.as_weak(), app.clone());
        Timer::single_shot(Duration::from_millis(1500), move || {
            if let Some(ui) = weak.upgrade() {
                let mut st = app.borrow_mut();
                let paths = list
                    .split(';')
                    .filter(|p| !p.is_empty())
                    .map(Into::into)
                    .collect();
                st.add_dropped_files(paths);
                st.after_edit(&ui);
            }
        });
    }

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
