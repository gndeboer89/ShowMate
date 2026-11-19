//! Integratietests met een echte mpv (headless via `--vo=null`). Zonder mpv worden ze overgeslagen.

use showmate_video::*;
use std::path::Path;
use std::time::{Duration, Instant};

/// Tests starten echte processen en doden er één; daarom één tegelijk.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn output() -> Option<(VideoOutput, std::sync::MutexGuard<'static, ()>)> {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let out = VideoOutput::new(VideoConfig {
        extra_args: vec!["--vo=null".into(), "--ao=null".into()],
        ..VideoConfig::default()
    });
    if out.mpv().is_err() {
        eprintln!("mpv niet gevonden: test overgeslagen");
        return None;
    }
    Some((out, guard))
}

fn wait_for(
    out: &mut VideoOutput,
    secs: u64,
    mut done: impl FnMut(&[VideoEvent]) -> bool,
) -> Vec<VideoEvent> {
    let start = Instant::now();
    let mut all = Vec::new();
    while start.elapsed() < Duration::from_secs(secs) {
        all.extend(out.poll());
        if done(&all) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    all
}

const TEST_SRC: &str = "av://lavfi:testsrc=duration=1:rate=25:size=160x90";

#[test]
fn plays_to_end_and_reports_duration() {
    let Some((mut out, _guard)) = output() else {
        return;
    };
    out.start(7, &[0], Path::new(TEST_SRC), false).unwrap();
    assert!(out.is_playing(7));
    let events = wait_for(&mut out, 15, |e| e.contains(&VideoEvent::Ended { cue: 7 }));
    assert!(
        events.contains(&VideoEvent::Ended { cue: 7 }),
        "geen einde: {events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, VideoEvent::Duration { cue: 7, secs } if (*secs - 1.0).abs() < 0.2)
        ),
        "geen duur: {events:?}"
    );
    assert!(!out.is_playing(7));
}

#[test]
fn stop_cue_does_not_report_end_and_loop_keeps_running() {
    let Some((mut out, _guard)) = output() else {
        return;
    };
    out.start(1, &[0], Path::new(TEST_SRC), true).unwrap();
    let events = wait_for(&mut out, 3, |e| {
        e.iter().any(|e| matches!(e, VideoEvent::Duration { .. }))
    });
    assert!(
        events
            .iter()
            .any(|e| matches!(e, VideoEvent::Duration { cue: 1, .. }))
    );
    // Loop: ruim na de bestandsduur nog steeds niet afgelopen.
    let events = wait_for(&mut out, 2, |e| {
        e.iter().any(|e| matches!(e, VideoEvent::Ended { .. }))
    });
    assert!(!events.iter().any(|e| matches!(e, VideoEvent::Ended { .. })));
    assert!(out.is_playing(1));
    out.stop_cue(1);
    assert!(!out.is_playing(1));
    let events = wait_for(&mut out, 1, |_| false);
    assert!(events.is_empty(), "stop mag geen Ended geven: {events:?}");
}

#[test]
fn multi_display_cue_ends_once_when_all_ended() {
    let Some((mut out, _guard)) = output() else {
        return;
    };
    out.start(5, &[0, 1], Path::new(TEST_SRC), false).unwrap();
    assert_eq!(out.display_of(5), Some(0));
    let events = wait_for(&mut out, 15, |e| e.contains(&VideoEvent::Ended { cue: 5 }));
    let ended = events
        .iter()
        .filter(|e| **e == VideoEvent::Ended { cue: 5 })
        .count();
    assert_eq!(ended, 1, "{events:?}");
}

#[test]
fn missing_file_reports_error_and_ends() {
    let Some((mut out, _guard)) = output() else {
        return;
    };
    out.start(3, &[0], Path::new("/bestaat/niet.mp4"), false)
        .unwrap();
    let events = wait_for(&mut out, 10, |e| e.contains(&VideoEvent::Ended { cue: 3 }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, VideoEvent::Error { cue: Some(3), .. })),
        "{events:?}"
    );
    assert!(events.contains(&VideoEvent::Ended { cue: 3 }));
}

#[test]
fn killed_player_is_reported_and_respawned() {
    let Some((mut out, _guard)) = output() else {
        return;
    };
    out.start(1, &[0], Path::new(TEST_SRC), true).unwrap();
    wait_for(&mut out, 3, |e| {
        e.iter().any(|e| matches!(e, VideoEvent::Duration { .. }))
    });
    // mpv van buitenaf doden
    let _ = std::process::Command::new("pkill")
        .args(["-f", "--", "--input-ipc-server=.*[s]howmate-"])
        .status();
    let events = wait_for(&mut out, 5, |e| e.contains(&VideoEvent::Ended { cue: 1 }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, VideoEvent::Error { cue: Some(1), .. })),
        "{events:?}"
    );
    // Volgende cue werkt weer (nieuw proces).
    out.start(2, &[0], Path::new(TEST_SRC), false).unwrap();
    let events = wait_for(&mut out, 15, |e| e.contains(&VideoEvent::Ended { cue: 2 }));
    assert!(events.contains(&VideoEvent::Ended { cue: 2 }), "{events:?}");
}

#[test]
fn locate_reports_helpful_error() {
    let err = locate_mpv(Some(Path::new("/nergens/mpv")));
    // Ofwel op het PATH gevonden, ofwel een bruikbare melding.
    if let Err(e) = err {
        assert!(e.contains("SHOWMATE_MPV"));
    }
}
