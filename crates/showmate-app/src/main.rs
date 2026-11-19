#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

#[macro_use]
mod i18n;
mod dialogs;
mod keys;
mod media;
mod settings;
mod state;

use slint::{ComponentHandle, SharedString, Timer, TimerMode};
use state::AppState;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let sui = SettingsWindow::new()?;
    let app = Rc::new(RefCell::new(AppState::open_default()));
    app.borrow().apply_language();
    app.borrow_mut().refresh_all(&ui, &sui);

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

    // Elke callback: muteer de staat en ververs daarna beide vensters.
    // `wire!(ui, on_x, |st, arg: T| …)` voor het hoofdvenster, `wire!(sui, …)` voor instellingen.
    macro_rules! wire {
        ($win:ident, $on:ident, |$st:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
            $win.$on(move |$($arg: $ty),*| {
                if let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) {
                    let mut $st = app.borrow_mut();
                    $body;
                    $st.after_edit(&m, &s);
                }
            });
        }};
    }

    // ---- hoofdvenster ----
    wire!(ui, on_go, |st| st.go());
    wire!(ui, on_pause, |st| st.toggle_pause());
    wire!(ui, on_stop, |st| st.stop());
    wire!(ui, on_select, |st, i: i32| st.select(i));
    wire!(ui, on_add_cue, |st, kind: SharedString| st.add_cue(&kind));
    wire!(ui, on_remove_cue, |st| st.remove_selected());
    wire!(ui, on_move_up, |st| st.move_selected(true));
    wire!(ui, on_move_down, |st| st.move_selected(false));
    wire!(ui, on_save, |st| st.save());
    wire!(ui, on_edit_name, |st, v: SharedString| st
        .edit(|c| c.name = v.to_string()));
    wire!(ui, on_edit_media, |st, v: SharedString| st.edit_media(&v));
    wire!(ui, on_edit_volume, |st, v: SharedString| st.edit_volume(&v));
    wire!(ui, on_edit_fade_in, |st, v: SharedString| st
        .edit_fade(&v, true));
    wire!(ui, on_edit_fade_out, |st, v: SharedString| st
        .edit_fade(&v, false));
    wire!(ui, on_edit_output, |st, v: SharedString| st
        .edit(|c| c.output = v.to_string()));
    wire!(ui, on_edit_notes, |st, v: SharedString| st
        .edit(|c| c.notes = v.to_string()));
    wire!(ui, on_edit_start, |st, auto: bool| st.edit(|c| {
        c.start = if auto {
            showmate_core::StartMode::Auto
        } else {
            showmate_core::StartMode::Go
        };
    }));
    wire!(ui, on_edit_looping, |st, v: bool| st
        .edit(|c| c.looping = v));
    wire!(ui, on_undo, |st| st.undo());
    wire!(ui, on_redo, |st| st.redo());
    wire!(ui, on_duplicate_cue, |st| st.duplicate_selected());
    wire!(ui, on_panic, |st| st.panic_stop());
    wire!(ui, on_reset_show, |st| st.reset_show());
    wire!(ui, on_edit_group_mode, |st, i: i32| st.edit_group_mode(i));
    wire!(ui, on_file_new, |st| st.new_show());
    wire!(ui, on_file_open, |st| st.open_show_dialog());
    wire!(ui, on_file_save_as, |st| st.save_as());
    wire!(ui, on_open_recent, |st, i: i32| st.open_recent(i));
    wire!(ui, on_browse_media, |st| st.browse_media());
    wire!(ui, on_edit_mute, |st, v: bool| st.edit_mute(v));
    wire!(ui, on_edit_osc_dest, |st, v: SharedString| st
        .edit_osc_dest(&v));
    wire!(ui, on_edit_osc_command, |st, v: SharedString| st
        .edit_osc_command(&v));
    wire!(ui, on_edit_osc_address, |st, v: SharedString| st
        .edit_osc_address(&v));
    wire!(ui, on_edit_osc_args, |st, v: SharedString| st
        .edit_osc_args(&v));
    wire!(ui, on_edit_midi_kind, |st, i: i32| st.edit_midi_kind(i));
    wire!(ui, on_edit_midi_num, |st, f: i32, v: SharedString| st
        .edit_midi_num(f, &v));
    wire!(ui, on_test_cue, |st| st.test_cue());
    wire!(ui, on_edit_delay, |st, v: SharedString| st.edit_delay(&v));
    wire!(ui, on_edit_duration, |st, v: SharedString| st
        .edit_duration(&v));
    wire!(ui, on_about, |st| st.about());

    // Sneltoetsen uit het hoofdvenster.
    {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        ui.on_key_event(move |text, ctrl, shift, alt, meta| {
            let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) else {
                return false;
            };
            let mut st = app.borrow_mut();
            let handled = st.handle_key(&text, ctrl, shift, alt, meta, false);
            st.after_edit(&m, &s);
            handled
        });
    }

    // Menu Instellingen → een pagina van het instellingenvenster openen.
    {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        ui.on_open_settings(move |page| {
            let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) else {
                return;
            };
            {
                let mut st = app.borrow_mut();
                st.open_settings();
                s.set_page(page);
                st.after_edit(&m, &s);
            }
            let _ = s.show();
        });
    }

    // Afsluiten via het menu: niet-opgeslagen werk bewaren en stoppen.
    {
        let (ws, app) = (sui.as_weak(), app.clone());
        ui.on_quit_app(move || {
            app.borrow_mut().on_exit();
            if let Some(s) = ws.upgrade() {
                let _ = s.hide();
            }
            let _ = slint::quit_event_loop();
        });
    }

    // ---- instellingenvenster ----
    wire!(sui, on_language_selected, |st, i: i32| st.set_language(i));
    wire!(sui, on_copy_media_toggled, |st, v: bool| st
        .set_copy_media(v));
    wire!(sui, on_audio_selected, |st, i: i32| st.audio_selected(i));
    wire!(sui, on_audio_refresh, |st| st.audio_refresh());
    wire!(sui, on_audio_test, |st, p: i32| st.audio_test(p));
    wire!(sui, on_video_fullscreen_toggled, |st, v: bool| st
        .video_fullscreen_toggled(v));
    wire!(sui, on_video_screen_selected, |st, d: i32, m: i32| st
        .video_screen_selected(d, m));
    wire!(sui, on_mpv_path_edited, |st, v: SharedString| st
        .mpv_path_edited(&v));
    wire!(sui, on_browse_mpv, |st| st.browse_mpv());
    wire!(
        sui,
        on_dest_edited,
        |st, i: i32, f: i32, v: SharedString| st.dest_edited(i, f, &v)
    );
    wire!(sui, on_dest_add, |st| st.dest_add());
    wire!(sui, on_dest_remove, |st, i: i32| st.dest_remove(i));
    wire!(sui, on_cmd_edited, |st, i: i32, f: i32, v: SharedString| st
        .cmd_edited(i, f, &v));
    wire!(sui, on_cmd_add, |st| st.cmd_add());
    wire!(sui, on_cmd_remove, |st, i: i32| st.cmd_remove(i));
    wire!(sui, on_midi_selected, |st, v: SharedString| st
        .midi_selected(&v));
    wire!(sui, on_midi_refresh, |st| st.midi_refresh());
    wire!(sui, on_key_capture, |st, i: i32| st.key_capture(i));
    wire!(sui, on_key_clear, |st, i: i32| st.key_clear(i));
    wire!(sui, on_key_reset, |st, i: i32| st.key_reset(i));
    wire!(sui, on_keys_reset_all, |st| st.keys_reset_all());
    {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        sui.on_key_event(move |text, ctrl, shift, alt, meta| {
            let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) else {
                return false;
            };
            let mut st = app.borrow_mut();
            let handled = st.handle_key(&text, ctrl, shift, alt, meta, true);
            st.after_edit(&m, &s);
            handled
        });
    }
    // Sluiten (knop of kruisje): instellingen toepassen en bewaren.
    let close_settings = {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        move || {
            let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) else {
                return;
            };
            let mut st = app.borrow_mut();
            st.close_settings();
            st.after_edit(&m, &s);
            let _ = s.hide();
        }
    };
    {
        let close = close_settings.clone();
        sui.on_done(close);
    }
    {
        let close = close_settings;
        sui.window().on_close_requested(move || {
            close();
            slint::CloseRequestResponse::HideWindow
        });
    }

    // Bestanden die op het venster worden gesleept (winit-gebeurtenis; Slint heeft hier geen eigen API voor).
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        ui.window().on_winit_window_event(move |_, event| {
            if let WindowEvent::DroppedFile(path) = event
                && let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade())
            {
                let mut st = app.borrow_mut();
                st.add_dropped_files(vec![path.clone()]);
                st.after_edit(&m, &s);
            }
            EventResult::Propagate
        });
    }

    // Alleen voor tests: simuleert het slepen van bestanden (`SHOWMATE_TEST_DROP=pad1;pad2`).
    if let Ok(list) = std::env::var("SHOWMATE_TEST_DROP") {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        Timer::single_shot(Duration::from_millis(1500), move || {
            if let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) {
                let mut st = app.borrow_mut();
                let paths = list
                    .split(';')
                    .filter(|p| !p.is_empty())
                    .map(Into::into)
                    .collect();
                st.add_dropped_files(paths);
                st.after_edit(&m, &s);
            }
        });
    }

    // Alleen voor tests: opent een instellingenpagina bij het starten (`SHOWMATE_TEST_SETTINGS=<pagina>`).
    if let Ok(page) = std::env::var("SHOWMATE_TEST_SETTINGS") {
        let (wm, ws, app) = (ui.as_weak(), sui.as_weak(), app.clone());
        Timer::single_shot(Duration::from_millis(800), move || {
            if let (Some(m), Some(s)) = (wm.upgrade(), ws.upgrade()) {
                let mut st = app.borrow_mut();
                st.open_settings();
                s.set_page(page.parse().unwrap_or(0));
                st.after_edit(&m, &s);
                let _ = s.show();
            }
        });
    }

    // Bij afsluiten niet-opgeslagen werk bewaren (wordt bij de volgende start hersteld).
    {
        let (ws, app) = (sui.as_weak(), app.clone());
        ui.window().on_close_requested(move || {
            app.borrow_mut().on_exit();
            if let Some(s) = ws.upgrade() {
                let _ = s.hide();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    ui.run()
}
