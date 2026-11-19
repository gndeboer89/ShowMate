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
    out.start(7, &[0], Path::new(TEST_SRC), false, VideoAudio::default())
        .unwrap();
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
    out.start(1, &[0], Path::new(TEST_SRC), true, VideoAudio::default())
        .unwrap();
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
    out.start(
        5,
        &[0, 1],
        Path::new(TEST_SRC),
        false,
        VideoAudio::default(),
    )
    .unwrap();
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
    out.start(
        3,
        &[0],
        Path::new("/bestaat/niet.mp4"),
        false,
        VideoAudio::default(),
    )
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
    out.start(1, &[0], Path::new(TEST_SRC), true, VideoAudio::default())
        .unwrap();
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
    out.start(2, &[0], Path::new(TEST_SRC), false, VideoAudio::default())
        .unwrap();
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

// ---- geluid uit video ------------------------------------------------------------------

/// Hoogste piek (0..=1) in een door mpv's `--ao=pcm` geschreven wav-bestand.
fn wav_peak(path: &Path) -> f32 {
    let b = std::fs::read(path).unwrap_or_default();
    if b.len() < 12 {
        return 0.0;
    }
    // Chunks doorlopen: "fmt " en "data".
    let (mut fmt, mut bits, mut data) = (0u16, 0u16, &b[0..0]);
    let mut i = 12;
    while i + 8 <= b.len() {
        let size = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let body = &b[(i + 8).min(b.len())..(i + 8 + size).min(b.len())];
        match &b[i..i + 4] {
            b"fmt " if body.len() >= 16 => {
                fmt = u16::from_le_bytes([body[0], body[1]]);
                bits = u16::from_le_bytes([body[14], body[15]]);
                if fmt == 0xFFFE && body.len() >= 26 {
                    fmt = u16::from_le_bytes([body[24], body[25]]); // subformat
                }
            }
            b"data" => {
                data = body;
                break;
            }
            _ => {}
        }
        i += 8 + size + (size & 1);
    }
    match (fmt, bits) {
        (1, 16) => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| f32::from(i16::from_le_bytes(*c)).abs() / 32768.0)
            .fold(0.0, f32::max),
        (3, 32) => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c).abs())
            .fold(0.0, f32::max),
        _ => panic!("onverwacht wav-formaat {fmt}/{bits}"),
    }
}

fn play_with_audio(tag: &str, audio: VideoAudio) -> Option<f32> {
    let dir = std::env::temp_dir().join(format!("showmate-vaudio-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let video = dir.join("tone.mp4");
    let made = std::process::Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=d=1:s=160x90:r=25",
        ])
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-shortest",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&video)
        .status()
        .is_ok_and(|s| s.success());
    if !made {
        eprintln!("ffmpeg niet gevonden: test overgeslagen");
        return None;
    }
    let pcm = dir.join("out.wav");
    let mut out = VideoOutput::new(VideoConfig {
        extra_args: vec![
            "--vo=null".into(),
            "--ao=pcm".into(),
            format!("--ao-pcm-file={}", pcm.display()),
        ],
        ..VideoConfig::default()
    });
    out.mpv().ok()?;
    out.start(1, &[0], &video, false, audio).unwrap();
    wait_for(&mut out, 15, |e| e.contains(&VideoEvent::Ended { cue: 1 }));
    drop(out); // sluit mpv zodat het wav-bestand compleet is
    let peak = wav_peak(&pcm);
    std::fs::remove_dir_all(dir).unwrap();
    Some(peak)
}

#[test]
fn video_audio_is_on_by_default_and_can_be_muted() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(on) = play_with_audio("on", VideoAudio::default()) else {
        return;
    };
    let off = play_with_audio(
        "off",
        VideoAudio {
            mute: true,
            gain_db: 0.0,
        },
    )
    .unwrap();
    let quiet = play_with_audio(
        "quiet",
        VideoAudio {
            mute: false,
            gain_db: -12.0,
        },
    )
    .unwrap();
    assert!(on > 0.05, "geluid hoort standaard aan te staan, piek {on}");
    assert!(off < 0.001, "gedempt hoort stil te zijn, piek {off}");
    let ratio = quiet / on;
    assert!((0.2..0.32).contains(&ratio), "-12 dB ≈ 0.25×, was {ratio}");
}

#[test]
fn startup_failure_explains_why_and_is_fast() {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = VideoOutput::new(VideoConfig {
        // Een optie die mpv niet kent, zoals bij een andere mpv-versie: mpv sluit meteen af.
        extra_args: vec!["--vo=null".into(), "--bestaat-niet-in-mpv=1".into()],
        ..VideoConfig::default()
    });
    if out.mpv().is_err() {
        eprintln!("mpv niet gevonden: test overgeslagen");
        return;
    }
    let start = Instant::now();
    out.start(1, &[0], Path::new(TEST_SRC), false, VideoAudio::default())
        .unwrap();
    let events = wait_for(&mut out, 8, |e| e.contains(&VideoEvent::Ended { cue: 1 }));
    let msg = events
        .iter()
        .find_map(|e| match e {
            VideoEvent::Error { message, .. } => Some(message.clone()),
            _ => None,
        })
        .expect("foutmelding verwacht");
    assert!(
        msg.contains("bestaat-niet-in-mpv"),
        "melding noemt de oorzaak niet: {msg}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "moet snel falen, niet na de 10 s time-out"
    );
}
