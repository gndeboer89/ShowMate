use showmate_core::*;

fn engine() -> (Engine<ManualClock>, ManualClock) {
    let mut show = Show::new("t");
    for n in ["a", "b", "c"] {
        show.push(Cue::new(CueId(0), n, CueKind::Audio).duration_secs(10.0));
    }
    let clock = ManualClock::new();
    (Engine::new(clock.clone(), show), clock)
}

fn names(e: &Engine<ManualClock>) -> Vec<&str> {
    e.show().cues.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn add_cue_after_selection_or_at_end() {
    let (mut e, _) = engine();
    assert_eq!(e.add_cue(CueKind::Video, Some(0)), Ok(1));
    assert_eq!(e.show().cues[1].kind, CueKind::Video);
    assert_eq!(e.add_cue(CueKind::Osc, None), Ok(4));
    assert_eq!(e.states().len(), 5);
    let ids: std::collections::HashSet<_> = e.show().cues.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 5, "ids moeten uniek blijven");
}

#[test]
fn remove_and_move_keep_states_in_sync() {
    let (mut e, _) = engine();
    e.go(); // a actief
    assert_eq!(e.remove_cue(0), Err(EditError::CueActive));
    assert_eq!(e.move_cue(0, false), Ok(1)); // actieve cue mag wel schuiven
    assert_eq!(names(&e), ["b", "a", "c"]);
    assert!(matches!(e.states()[1], CueState::Active { .. }));
    assert_eq!(e.states()[0], CueState::Waiting);
    assert_eq!(e.remove_cue(2).unwrap().name, "c");
    assert_eq!(e.states().len(), 2);
}

#[test]
fn move_at_edges_is_noop() {
    let (mut e, _) = engine();
    assert_eq!(e.move_cue(0, true), Ok(0));
    assert_eq!(e.move_cue(2, false), Ok(2));
    assert_eq!(e.move_cue(9, true), Err(EditError::OutOfRange));
    assert_eq!(names(&e), ["a", "b", "c"]);
}

#[test]
fn structural_edits_blocked_while_cues_scheduled() {
    let (mut e, _) = engine();
    e.update_cue(1, |c| {
        c.start = StartMode::Auto;
        c.delay = std::time::Duration::from_secs(30);
    })
    .unwrap();
    e.go(); // a actief, b gepland over 30 s
    assert!(matches!(e.states()[1], CueState::Scheduled { .. }));
    assert_eq!(e.add_cue(CueKind::Audio, None), Err(EditError::Busy));
    assert_eq!(e.move_cue(2, true), Err(EditError::Busy));
    assert_eq!(e.remove_cue(2), Err(EditError::Busy));
    e.stop();
    assert!(e.add_cue(CueKind::Audio, None).is_ok());
}

#[test]
fn update_cue_cannot_change_id() {
    let (mut e, _) = engine();
    let id = e.show().cues[0].id;
    e.update_cue(0, |c| {
        c.id = CueId(999);
        c.name = "x".into();
    })
    .unwrap();
    assert_eq!(e.show().cues[0].id, id);
    assert_eq!(e.show().cues[0].name, "x");
    assert_eq!(e.update_cue(7, |_| {}), Err(EditError::OutOfRange));
}

#[test]
fn edited_delay_applies_to_next_go() {
    let (mut e, clock) = engine();
    e.update_cue(0, |c| c.delay = std::time::Duration::from_secs(3))
        .unwrap();
    e.go();
    clock.advance_secs(3.0);
    assert_eq!(e.tick(), vec![Event::CueStarted(e.show().cues[0].id)]);
}
