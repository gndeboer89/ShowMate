use showmate_core::*;

fn audio(name: &str) -> Cue {
    Cue::new(CueId(0), name, CueKind::Audio).duration_secs(10.0)
}

fn engine(cues: Vec<Cue>, spans: &[(usize, usize)]) -> (Engine<ManualClock>, ManualClock) {
    let mut show = Show::new("t");
    for c in cues {
        show.push(c);
    }
    for (i, span) in spans {
        show.cues[*i].span = *span;
    }
    show.sanitize();
    let clock = ManualClock::new();
    (Engine::new(clock.clone(), show), clock)
}

fn active(e: &Engine<ManualClock>) -> Vec<usize> {
    (0..e.states().len())
        .filter(|&i| matches!(e.states()[i], CueState::Active { .. }))
        .collect()
}

#[test]
fn default_playhead_is_the_first_waiting_cue_and_follows_the_list() {
    let (mut e, _) = engine(vec![audio("a"), audio("b"), audio("c")], &[]);
    assert_eq!(e.playhead(), Some(0));
    e.go();
    assert_eq!(e.playhead(), Some(1));
    e.go();
    assert_eq!(e.playhead(), Some(2));
    e.go();
    assert_eq!(e.playhead(), None, "voorbij het einde");
    assert_eq!(active(&e), vec![0, 1, 2]);
}

#[test]
fn go_starts_the_chosen_cue_not_the_next_one_in_the_list() {
    let (mut e, _) = engine(vec![audio("a"), audio("b"), audio("c"), audio("d")], &[]);
    e.go(); // a speelt, playhead op b
    assert_eq!(e.set_playhead(2), Some(2)); // klik op c
    assert_eq!(e.playhead(), Some(2));
    e.go();
    assert_eq!(active(&e), vec![0, 2], "b is overgeslagen");
    assert_eq!(e.playhead(), Some(3), "daarna d");
    // b is nog nooit gespeeld en kan nog steeds gekozen worden
    e.set_playhead(1);
    e.go();
    assert_eq!(active(&e), vec![0, 1, 2]);
}

#[test]
fn a_played_cue_can_be_started_again() {
    let (mut e, clock) = engine(vec![audio("a"), audio("b")], &[]);
    e.go();
    clock.advance_secs(11.0);
    e.tick();
    assert_eq!(e.states()[0], CueState::Finished);
    e.set_playhead(0);
    let ev = e.go();
    assert_eq!(ev, vec![Event::CueStarted(e.show().cues[0].id)]);
    assert!(matches!(e.states()[0], CueState::Active { .. }));
}

#[test]
fn replaying_a_cue_replays_its_auto_follow_chain() {
    let (mut e, clock) = engine(
        vec![audio("a"), audio("b").auto().delay_secs(1.0), audio("c")],
        &[],
    );
    e.go();
    clock.advance_secs(11.0);
    e.tick();
    assert_eq!(e.states()[1], CueState::Finished);
    e.set_playhead(0);
    e.go();
    clock.advance_secs(1.0);
    e.tick();
    assert!(matches!(e.states()[0], CueState::Active { .. }));
    assert!(
        matches!(e.states()[1], CueState::Active { .. }),
        "auto-volger start opnieuw"
    );
    assert_eq!(e.playhead(), Some(2));
}

#[test]
fn go_on_a_running_cue_does_not_start_it_twice() {
    let (mut e, _) = engine(vec![audio("a"), audio("b")], &[]);
    e.go();
    e.set_playhead(0);
    let ev = e.go();
    assert!(ev.is_empty(), "a speelt al: geen tweede start");
    assert_eq!(e.playhead(), Some(1), "de playhead schuift wel door");
}

#[test]
fn clicking_a_child_selects_its_group() {
    let (mut e, _) = engine(
        vec![
            audio("pre"),
            Cue::new(CueId(0), "g", CueKind::Group),
            audio("k1"),
            audio("k2"),
            audio("post"),
        ],
        &[(1, 2)],
    );
    assert_eq!(e.set_playhead(3), Some(1));
    e.go();
    assert_eq!(e.playhead(), Some(4), "na het hele blok");
    assert!(matches!(e.states()[1], CueState::Active { .. }));
}

#[test]
fn replaying_a_group_restarts_its_children() {
    let (mut e, clock) = engine(
        vec![
            Cue::new(CueId(0), "g", CueKind::Group).group(GroupMode::Sequence),
            audio("k1"),
            audio("k2"),
        ],
        &[(0, 2)],
    );
    e.go();
    clock.advance_secs(25.0);
    e.tick();
    assert_eq!(e.states()[0], CueState::Finished);
    assert_eq!(e.states()[2], CueState::Finished);
    e.set_playhead(0);
    e.go();
    assert!(matches!(e.states()[0], CueState::Active { .. }));
    assert!(
        matches!(e.states()[1], CueState::Active { .. }),
        "eerste kind speelt opnieuw"
    );
    assert_eq!(
        e.states()[2],
        CueState::Waiting,
        "tweede kind wacht tot het eerste klaar is"
    );
}

#[test]
fn stop_puts_the_playhead_back_on_a_cancelled_cue() {
    let (mut e, _) = engine(
        vec![audio("a"), audio("b").auto().delay_secs(30.0), audio("c")],
        &[],
    );
    e.go();
    assert_eq!(e.playhead(), Some(2));
    e.stop();
    assert_eq!(e.playhead(), Some(1), "b was geannuleerd");
}

#[test]
fn reset_and_removal_fall_back_to_the_first_waiting_cue() {
    let (mut e, _) = engine(vec![audio("a"), audio("b"), audio("c")], &[]);
    e.set_playhead(2);
    e.remove_cue(2).unwrap();
    assert_eq!(e.playhead(), Some(0), "playhead-cue is weg");
    e.set_playhead(1);
    e.go();
    e.reset();
    assert_eq!(e.playhead(), Some(0));
}

#[test]
fn playhead_survives_moving_cues() {
    let (mut e, _) = engine(vec![audio("a"), audio("b"), audio("c")], &[]);
    e.set_playhead(2);
    e.move_cue(2, true).unwrap(); // c staat nu op index 1
    assert_eq!(e.playhead(), Some(1));
    e.go();
    assert_eq!(e.show().cues[active(&e)[0]].name, "c");
}
