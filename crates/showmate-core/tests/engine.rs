use showmate_core::demo::demo_show;
use showmate_core::*;
use std::time::Duration;

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

fn engine(cues: Vec<Cue>) -> (Engine<ManualClock>, ManualClock) {
    let mut show = Show::new("test");
    for c in cues {
        show.push(c);
    }
    let clock = ManualClock::new();
    (Engine::new(clock.clone(), show), clock)
}

fn cue(name: &str, kind: CueKind) -> Cue {
    Cue::new(CueId(0), name, kind)
}

fn id(e: &Engine<ManualClock>, i: usize) -> CueId {
    e.show().cues[i].id
}

#[test]
fn nothing_runs_before_go() {
    let (mut e, clock) = engine(vec![cue("a", CueKind::Audio)]);
    clock.advance_secs(10.0);
    assert!(e.tick().is_empty());
    assert_eq!(e.show_time(), Duration::ZERO);
    assert_eq!(e.states()[0], CueState::Waiting);
}

#[test]
fn go_starts_cue_without_delay_immediately() {
    let (mut e, _c) = engine(vec![cue("a", CueKind::Audio).duration_secs(10.0)]);
    let ev = e.go();
    assert_eq!(ev, vec![Event::CueStarted(id(&e, 0))]);
    assert!(matches!(e.states()[0], CueState::Active { .. }));
}

#[test]
fn delay_is_respected() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).delay_secs(5.0).duration_secs(10.0),
    ]);
    assert!(e.go().is_empty());
    assert!(matches!(e.states()[0], CueState::Scheduled { .. }));
    clock.advance_secs(4.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1);
    assert_eq!(e.tick(), vec![Event::CueStarted(id(&e, 0))]);
}

#[test]
fn go_cue_waits_for_next_go() {
    let (mut e, _c) = engine(vec![
        cue("a", CueKind::Audio).duration_secs(10.0),
        cue("b", CueKind::Audio).duration_secs(10.0),
    ]);
    e.go();
    assert_eq!(e.states()[1], CueState::Waiting);
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 1,
            starts_in: None
        })
    );
    let ev = e.go();
    assert_eq!(ev, vec![Event::CueStarted(id(&e, 1))]);
}

#[test]
fn auto_cues_chain_relative_to_previous_start() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).duration_secs(100.0),
        cue("b", CueKind::Audio)
            .auto()
            .delay_secs(5.0)
            .duration_secs(100.0),
        cue("c", CueKind::Audio)
            .auto()
            .delay_secs(10.0)
            .duration_secs(100.0),
        cue("d", CueKind::Audio).duration_secs(100.0), // Go: niet meegepland
    ]);
    e.go();
    assert_eq!(e.states()[3], CueState::Waiting);
    clock.advance_secs(5.0);
    assert_eq!(e.tick(), vec![Event::CueStarted(id(&e, 1))]);
    clock.advance_secs(9.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1);
    assert_eq!(e.tick(), vec![Event::CueStarted(id(&e, 2))]);
}

#[test]
fn chain_does_not_drift_with_coarse_ticks() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).duration_secs(100.0),
        cue("b", CueKind::Audio)
            .auto()
            .delay_secs(1.0)
            .duration_secs(100.0),
        cue("c", CueKind::Audio)
            .auto()
            .delay_secs(1.0)
            .duration_secs(100.0),
    ]);
    e.go();
    clock.advance_secs(2.5); // één grove tick: b én c zijn aan de beurt
    let ev = e.tick();
    assert_eq!(
        ev,
        vec![Event::CueStarted(id(&e, 1)), Event::CueStarted(id(&e, 2))]
    );
    // c is gestart op 2.0 s, niet op 2.5 s
    assert_eq!(
        e.states()[2],
        CueState::Active {
            started_at: secs(2.0)
        }
    );
}

#[test]
fn cue_finishes_after_duration() {
    let (mut e, clock) = engine(vec![cue("a", CueKind::Audio).duration_secs(3.0)]);
    e.go();
    clock.advance_secs(2.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1);
    assert_eq!(e.tick(), vec![Event::CueFinished(id(&e, 0))]);
    assert_eq!(e.states()[0], CueState::Finished);
}

#[test]
fn osc_without_duration_finishes_instantly_but_audio_does_not() {
    let (mut e, clock) = engine(vec![
        cue("osc", CueKind::Osc),
        cue("snd", CueKind::Audio).auto(),
    ]);
    let ev = e.go();
    assert_eq!(
        ev,
        vec![
            Event::CueStarted(id(&e, 0)),
            Event::CueFinished(id(&e, 0)),
            Event::CueStarted(id(&e, 1)),
        ]
    );
    clock.advance_secs(1000.0);
    e.tick();
    assert!(matches!(e.states()[1], CueState::Active { .. }));
}

#[test]
fn looping_cue_never_finishes() {
    let (mut e, clock) = engine(vec![
        cue("loop", CueKind::Video).duration_secs(2.0).looping(),
    ]);
    e.go();
    clock.advance_secs(500.0);
    e.tick();
    assert!(matches!(e.states()[0], CueState::Active { .. }));
}

#[test]
fn pause_freezes_show_time_and_scheduling() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).delay_secs(5.0).duration_secs(10.0),
    ]);
    e.go();
    clock.advance_secs(3.0);
    e.pause();
    assert!(e.is_paused());
    clock.advance_secs(100.0);
    assert!(e.tick().is_empty());
    assert_eq!(e.show_time(), secs(3.0));
    e.resume();
    clock.advance_secs(1.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1);
    assert_eq!(e.tick(), vec![Event::CueStarted(id(&e, 0))]);
}

#[test]
fn go_is_ignored_while_paused() {
    let (mut e, _c) = engine(vec![cue("a", CueKind::Audio)]);
    e.pause();
    assert!(e.go().is_empty());
    assert_eq!(e.states()[0], CueState::Waiting);
}

#[test]
fn stop_stops_active_and_unschedules_pending() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).duration_secs(100.0),
        cue("b", CueKind::Audio)
            .auto()
            .delay_secs(50.0)
            .duration_secs(100.0),
    ]);
    e.go();
    clock.advance_secs(1.0);
    let ev = e.stop();
    assert_eq!(ev, vec![Event::CueStopped(id(&e, 0))]);
    assert_eq!(e.states()[0], CueState::Stopped);
    assert_eq!(e.states()[1], CueState::Waiting);
    assert!(!e.is_paused());
    // volgende GO speelt de ongespeelde cue
    clock.advance_secs(1.0);
    e.go();
    assert!(matches!(e.states()[1], CueState::Scheduled { .. }));
}

#[test]
fn next_cue_reports_countdown() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Osc).delay_secs(5.0),
        cue("b", CueKind::Video).auto().delay_secs(5.0),
    ]);
    e.go();
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 0,
            starts_in: Some(secs(5.0))
        })
    );
    clock.advance_secs(5.0);
    e.tick();
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 1,
            starts_in: Some(secs(5.0))
        })
    );
    clock.advance_secs(5.0);
    e.tick();
    assert_eq!(e.next_cue(), None);
}

#[test]
fn reset_returns_to_start() {
    let (mut e, clock) = engine(vec![cue("a", CueKind::Audio).duration_secs(1.0)]);
    e.go();
    clock.advance_secs(2.0);
    e.tick();
    e.reset();
    assert_eq!(e.states()[0], CueState::Waiting);
    assert_eq!(e.show_time(), Duration::ZERO);
}

#[test]
fn progress_of_active_cue() {
    let (mut e, clock) = engine(vec![cue("a", CueKind::Audio).duration_secs(10.0)]);
    e.go();
    clock.advance_secs(4.0);
    e.tick();
    assert_eq!(e.progress(0), Some((secs(4.0), Some(secs(10.0)))));
}

#[test]
fn demo_show_plays_like_the_mockup() {
    let clock = ManualClock::new();
    let mut e = Engine::new(clock.clone(), demo_show());
    e.go(); // cue 1
    e.go(); // cue 2
    e.go(); // cue 3 (+5 s) en auto-keten 4..7
    // cue 3 gepland op 5 s, 4 op 10 s, 5 op 15 s, 6 op 95 s, 7 op 185 s
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 2,
            starts_in: Some(secs(5.0))
        })
    );
    clock.advance_secs(5.0);
    e.tick();
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 3,
            starts_in: Some(secs(5.0))
        })
    );
    clock.advance_secs(180.0);
    e.tick();
    assert!(matches!(e.states()[6], CueState::Active { .. }));
    clock.advance_secs(2.0);
    e.tick();
    assert_eq!(e.states()[6], CueState::Finished);
    // cue 4 (loop) blijft draaien
    assert!(matches!(e.states()[3], CueState::Active { .. }));
}

#[test]
fn show_roundtrips_through_json() {
    let show = demo_show();
    let json = show.to_json().unwrap();
    assert_eq!(Show::from_json(&json).unwrap(), show);
}

#[test]
fn show_saves_and_loads_file() {
    let dir = std::env::temp_dir().join(format!("showmate-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.showmate");
    let show = demo_show();
    show.save(&path).unwrap();
    assert_eq!(Show::load(&path).unwrap(), show);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rejects_newer_format_and_garbage() {
    assert!(Show::from_json("geen json").is_err());
    let json = demo_show()
        .to_json()
        .unwrap()
        .replace("\"version\": 1", "\"version\": 99");
    assert!(matches!(
        Show::from_json(&json),
        Err(ShowError::UnsupportedVersion(99))
    ));
}

#[test]
fn media_duration_ends_cue_without_own_duration() {
    let (mut e, clock) = engine(vec![cue("snd", CueKind::Audio)]);
    e.go();
    e.set_media_duration(id(&e, 0), secs(4.0));
    clock.advance_secs(3.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1);
    assert_eq!(e.tick(), vec![Event::CueFinished(id(&e, 0))]);
}

#[test]
fn own_duration_and_loop_win_over_media_duration() {
    let (mut e, clock) = engine(vec![
        cue("a", CueKind::Audio).duration_secs(2.0),
        cue("b", CueKind::Audio).auto().looping(),
    ]);
    e.go();
    e.set_media_duration(id(&e, 0), secs(60.0));
    e.set_media_duration(id(&e, 1), secs(1.0));
    clock.advance_secs(2.0);
    e.tick();
    assert_eq!(e.states()[0], CueState::Finished);
    assert!(matches!(e.states()[1], CueState::Active { .. }));
}

#[test]
fn pause_and_resume_emit_events_once() {
    let (mut e, _c) = engine(vec![cue("a", CueKind::Audio)]);
    assert_eq!(e.pause(), vec![Event::Paused]);
    assert!(e.pause().is_empty());
    assert_eq!(e.resume(), vec![Event::Resumed]);
    assert!(e.resume().is_empty());
}

#[test]
fn stop_while_paused_resumes() {
    let (mut e, _c) = engine(vec![cue("a", CueKind::Audio)]);
    e.go();
    e.pause();
    let ev = e.stop();
    assert_eq!(ev, vec![Event::CueStopped(id(&e, 0)), Event::Resumed]);
}

#[test]
fn new_audio_fields_default_in_old_showfiles() {
    let json =
        r#"{"name":"oud","cues":[{"id":1,"name":"x","kind":"audio","start":"go"}],"next_id":2}"#;
    let show = Show::from_json(json).unwrap();
    assert_eq!(show.cues[0].volume_db, 0.0);
    assert_eq!(show.cues[0].fade_in, Duration::ZERO);
}

#[test]
fn complete_finishes_only_active_cues() {
    let (mut e, _c) = engine(vec![cue("a", CueKind::Video), cue("b", CueKind::Video)]);
    let (a, b) = (id(&e, 0), id(&e, 1));
    assert!(e.complete(a).is_empty(), "wachtende cue");
    e.go();
    assert_eq!(e.complete(a), vec![Event::CueFinished(a)]);
    assert_eq!(e.states()[0], CueState::Finished);
    assert!(e.complete(a).is_empty(), "al klaar");
    assert!(e.complete(b).is_empty(), "nog niet gestart");
    assert!(e.complete(CueId(999)).is_empty());
}

#[test]
fn new_io_fields_default_in_old_showfiles() {
    let json =
        r#"{"name":"oud","cues":[{"id":1,"name":"x","kind":"osc","start":"go"}],"next_id":2}"#;
    let c = &Show::from_json(json).unwrap().cues[0];
    assert!(!c.mute_audio);
    assert_eq!(c.midi, MidiMessage::default());
    assert!(c.osc_address.is_empty() && c.osc_command.is_empty());
}
