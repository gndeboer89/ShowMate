use showmate_core::*;
use std::time::Duration;

fn secs(s: f64) -> Duration {
    Duration::from_secs_f64(s)
}

fn group(name: &str, mode: GroupMode) -> Cue {
    Cue::new(CueId(0), name, CueKind::Group).group(mode)
}

fn audio(name: &str, dur: f64) -> Cue {
    Cue::new(CueId(0), name, CueKind::Audio).duration_secs(dur)
}

/// Bouwt een show; `spans` zet de span van groepen op de opgegeven indices.
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

fn id(e: &Engine<ManualClock>, i: usize) -> CueId {
    e.show().cues[i].id
}

fn names(e: &Engine<ManualClock>) -> Vec<String> {
    e.show()
        .cues
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c.name,
                if c.span > 0 {
                    format!("[{}]", c.span)
                } else {
                    String::new()
                }
            )
        })
        .collect()
}

fn started(events: &[Event]) -> Vec<CueId> {
    events
        .iter()
        .filter_map(|e| {
            if let Event::CueStarted(i) = e {
                Some(*i)
            } else {
                None
            }
        })
        .collect()
}

// ---- afspelen -------------------------------------------------------------------------

#[test]
fn parallel_group_starts_children_with_their_own_delays() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Parallel),
            audio("a", 10.0),
            audio("b", 10.0).delay_secs(2.0),
            audio("c", 10.0).delay_secs(4.0),
        ],
        &[(0, 3)],
    );
    let ev = e.go();
    assert_eq!(started(&ev), vec![id(&e, 0), id(&e, 1)]);
    clock.advance_secs(2.0);
    assert_eq!(started(&e.tick()), vec![id(&e, 2)]);
    clock.advance_secs(2.0);
    assert_eq!(started(&e.tick()), vec![id(&e, 3)]);
    // groep eindigt als het laatste kind (start 4 s + 10 s) klaar is
    clock.advance_secs(9.9);
    assert!(!e.tick().contains(&Event::CueFinished(id(&e, 0))));
    clock.advance_secs(0.1);
    let ev = e.tick();
    assert!(ev.contains(&Event::CueFinished(id(&e, 3))));
    assert!(ev.contains(&Event::CueFinished(id(&e, 0))));
    assert_eq!(e.states()[0], CueState::Finished);
}

#[test]
fn sequence_group_plays_children_one_after_another() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 3.0),
            audio("b", 2.0).delay_secs(1.0),
            audio("c", 1.0),
        ],
        &[(0, 3)],
    );
    assert_eq!(started(&e.go()), vec![id(&e, 0), id(&e, 1)]);
    clock.advance_secs(2.9);
    assert!(e.tick().is_empty());
    clock.advance_secs(0.1); // a klaar op 3, b start op 3 + 1 delay = 4
    let ev = e.tick();
    assert_eq!(ev, vec![Event::CueFinished(id(&e, 1))]);
    clock.advance_secs(1.0);
    assert_eq!(started(&e.tick()), vec![id(&e, 2)]);
    clock.advance_secs(2.0); // b klaar op 6 → c start op 6
    assert_eq!(started(&e.tick()), vec![id(&e, 3)]);
    clock.advance_secs(1.0); // c klaar op 7 → groep klaar
    let ev = e.tick();
    assert!(ev.contains(&Event::CueFinished(id(&e, 0))));
    assert_eq!(e.states()[0], CueState::Finished);
}

#[test]
fn sequence_does_not_drift_with_a_coarse_tick() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
            audio("c", 1.0),
        ],
        &[(0, 3)],
    );
    e.go();
    clock.advance_secs(10.0); // alles in één tick
    let ev = e.tick();
    assert!(ev.contains(&Event::CueFinished(id(&e, 0))));
    assert_eq!(e.states()[3], CueState::Finished);
    // c liep van 2 tot 3, niet van 10
    assert_eq!(e.show_time(), secs(10.0));
}

#[test]
fn loop_group_repeats_until_stopped() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Loop),
            audio("a", 1.0),
            audio("b", 1.0),
        ],
        &[(0, 2)],
    );
    e.go();
    let mut starts = 0;
    for _ in 0..50 {
        clock.advance_secs(0.5);
        starts += started(&e.tick()).len();
    }
    // 25 s verstreken, ronde = 2 s → 12 volledige rondes + begin
    assert!(starts >= 24, "{starts}");
    assert!(matches!(e.states()[0], CueState::Active { .. }));
    let ev = e.stop();
    assert!(ev.contains(&Event::CueStopped(id(&e, 0))));
    assert!(e.tick().is_empty());
}

#[test]
fn loop_of_instant_cues_does_not_spin_forever() {
    let (mut e, _c) = engine(
        vec![
            group("g", GroupMode::Loop),
            Cue::new(CueId(0), "osc", CueKind::Osc),
        ],
        &[(0, 1)],
    );
    let ev = e.go(); // moet terugkeren
    assert!(ev.contains(&Event::CueFinished(id(&e, 0))));
}

#[test]
fn group_duration_caps_a_loop_and_stops_children() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Loop).duration_secs(5.0),
            audio("a", 2.0),
            audio("b", 2.0),
        ],
        &[(0, 2)],
    );
    e.go();
    clock.advance_secs(4.5); // a: 0-2, b: 2-4, a: 4-6 (actief)
    e.tick();
    assert!(matches!(e.states()[1], CueState::Active { .. }));
    clock.advance_secs(0.5);
    let ev = e.tick();
    assert!(ev.contains(&Event::CueStopped(id(&e, 1))));
    assert!(ev.contains(&Event::CueFinished(id(&e, 0))));
    assert_eq!(e.states()[1], CueState::Stopped);
}

#[test]
fn empty_group_finishes_immediately() {
    let (mut e, _c) = engine(vec![group("g", GroupMode::Sequence)], &[]);
    let ev = e.go();
    assert_eq!(
        ev,
        vec![Event::CueStarted(id(&e, 0)), Event::CueFinished(id(&e, 0))]
    );
}

#[test]
fn go_skips_children_and_auto_chain_follows_the_block() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Parallel).delay_secs(1.0),
            audio("a", 10.0),
            audio("b", 10.0),
            audio("after", 10.0).auto().delay_secs(2.0),
            audio("manual", 10.0),
        ],
        &[(0, 2)],
    );
    e.go();
    // `after` volgt de groep (start 1 + 2 = 3 s), `manual` wacht op GO
    assert_eq!(e.states()[4], CueState::Waiting);
    assert_eq!(e.next_cue().unwrap().index, 0);
    clock.advance_secs(1.0);
    e.tick();
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 3,
            starts_in: Some(secs(2.0))
        })
    );
    clock.advance_secs(2.0);
    e.tick();
    assert_eq!(
        e.next_cue(),
        Some(NextCue {
            index: 4,
            starts_in: None
        })
    );
    e.go();
    assert!(matches!(e.states()[4], CueState::Active { .. }));
}

#[test]
fn pause_freezes_a_running_sequence() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 2.0),
            audio("b", 2.0),
        ],
        &[(0, 2)],
    );
    e.go();
    clock.advance_secs(1.0);
    e.pause();
    clock.advance_secs(100.0);
    assert!(e.tick().is_empty());
    e.resume();
    clock.advance_secs(1.0);
    let ev = e.tick();
    assert_eq!(
        ev,
        vec![Event::CueFinished(id(&e, 1)), Event::CueStarted(id(&e, 2))]
    );
}

#[test]
fn complete_of_a_child_advances_the_sequence() {
    let (mut e, clock) = engine(
        vec![
            group("g", GroupMode::Sequence),
            Cue::new(CueId(0), "vid", CueKind::Video),
            audio("b", 5.0),
        ],
        &[(0, 2)],
    );
    e.go();
    clock.advance_secs(3.0);
    e.tick();
    let ev = e.complete(id(&e, 1));
    assert_eq!(
        ev,
        vec![Event::CueFinished(id(&e, 1)), Event::CueStarted(id(&e, 2))]
    );
    // b startte op het moment van het einde van vid (3 s), niet op 0
    assert_eq!(
        e.states()[2],
        CueState::Active {
            started_at: secs(3.0)
        }
    );
}

// ---- structuur ----------------------------------------------------------------------

#[test]
fn sanitize_fixes_bad_spans() {
    let mut show = Show::new("t");
    for n in ["x", "g1", "a", "g2", "b"] {
        show.push(Cue::new(CueId(0), n, CueKind::Audio));
    }
    show.cues[1].kind = CueKind::Group;
    show.cues[1].span = 99;
    show.cues[3].kind = CueKind::Group;
    show.cues[3].span = 5;
    show.cues[0].span = 3; // geen groep
    show.sanitize();
    assert_eq!(show.cues[0].span, 0);
    assert_eq!(show.cues[1].span, 1, "afgekapt voor de geneste groep");
    assert_eq!(show.cues[3].span, 1);
    assert_eq!(show.parent_of(2), Some(1));
    assert_eq!(show.parent_of(4), Some(3));
    assert_eq!(show.parent_of(0), None);
    assert_eq!(show.block_len(1), 2);
}

#[test]
fn groups_survive_json_and_old_files_load() {
    let (e, _) = engine(
        vec![group("g", GroupMode::Loop), audio("a", 1.0)],
        &[(0, 1)],
    );
    let json = e.show().to_json().unwrap();
    let back = Show::from_json(&json).unwrap();
    assert_eq!(back.cues[0].span, 1);
    assert_eq!(back.cues[0].group_mode, GroupMode::Loop);
}

#[test]
fn add_cue_goes_into_the_selected_group() {
    let (mut e, _) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("z", 1.0),
        ],
        &[(0, 1)],
    );
    // na het laatste kind → kind erbij
    assert_eq!(e.add_cue(CueKind::Video, Some(1)), Ok(2));
    assert_eq!(e.show().cues[0].span, 2);
    // na de header → eerste kind
    assert_eq!(e.add_cue(CueKind::Osc, Some(0)), Ok(1));
    assert_eq!(e.show().cues[0].span, 3);
    // na een gewone cue erbuiten → top-level
    let last = e.show().cues.len() - 1;
    assert_eq!(e.add_cue(CueKind::Audio, Some(last)), Ok(last + 1));
    assert_eq!(e.show().cues[0].span, 3);
    assert_eq!(e.show().parent_of(last + 1), None);
}

#[test]
fn add_group_never_nests() {
    let (mut e, _) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
        ],
        &[(0, 2)],
    );
    let i = e.add_cue(CueKind::Group, Some(1)).unwrap();
    assert_eq!(i, 3, "na het blok");
    assert_eq!(e.show().parent_of(3), None);
    assert_eq!(e.show().cues[0].span, 2);
    // lege groep als doel: kinderen komen erin
    assert_eq!(e.add_cue(CueKind::Audio, Some(3)), Ok(4));
    assert_eq!(e.show().cues[3].span, 1);
    // rechtstreeks via insert_cue binnen een groep wordt geweigerd
    assert_eq!(
        e.insert_cue(1, Cue::template(CueId(0), CueKind::Group)),
        Err(EditError::Nested)
    );
}

#[test]
fn removing_a_group_releases_its_children_and_removing_a_child_shrinks_it() {
    let (mut e, _) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
            audio("z", 1.0),
        ],
        &[(0, 2)],
    );
    e.remove_cue(2).unwrap();
    assert_eq!(names(&e), ["g[1]", "a", "z"]);
    e.remove_cue(0).unwrap();
    assert_eq!(names(&e), ["a", "z"]);
    assert_eq!(e.show().parent_of(0), None);
}

#[test]
fn moving_children_within_and_out_of_a_group() {
    let (mut e, _) = engine(
        vec![
            audio("pre", 1.0),
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
            audio("post", 1.0),
        ],
        &[(1, 2)],
    );
    assert_eq!(e.move_cue(3, true), Ok(2)); // binnen de groep
    assert_eq!(names(&e), ["pre", "g[2]", "b", "a", "post"]);
    assert_eq!(e.move_cue(2, true), Ok(1)); // b verlaat de groep naar boven
    assert_eq!(names(&e), ["pre", "b", "g[1]", "a", "post"]);
    assert_eq!(e.show().parent_of(1), None);
    assert_eq!(e.move_cue(3, false), Ok(3)); // a verlaat de groep naar beneden
    assert_eq!(names(&e), ["pre", "b", "g", "a", "post"]);
    assert_eq!(e.show().parent_of(3), None);
}

#[test]
fn moving_a_group_moves_its_whole_block() {
    let (mut e, _) = engine(
        vec![
            audio("pre", 1.0),
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
            audio("post", 1.0),
        ],
        &[(1, 2)],
    );
    assert_eq!(e.move_cue(1, true), Ok(0));
    assert_eq!(names(&e), ["g[2]", "a", "b", "pre", "post"]);
    assert_eq!(e.show().parent_of(2), Some(0));
    assert_eq!(e.move_cue(0, false), Ok(1));
    assert_eq!(e.move_cue(1, false), Ok(2));
    assert_eq!(names(&e), ["pre", "post", "g[2]", "a", "b"]);
}

#[test]
fn a_normal_cue_jumps_over_a_whole_block() {
    let (mut e, _) = engine(
        vec![
            audio("x", 1.0),
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("b", 1.0),
        ],
        &[(1, 2)],
    );
    assert_eq!(e.move_cue(0, false), Ok(3));
    assert_eq!(names(&e), ["g[2]", "a", "b", "x"]);
    assert_eq!(e.move_cue(3, true), Ok(0));
    assert_eq!(names(&e), ["x", "g[2]", "a", "b"]);
}

#[test]
fn moving_keeps_runtime_state_with_the_cue() {
    let (mut e, _) = engine(
        vec![
            audio("x", 1.0),
            audio("y", 1.0),
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
        ],
        &[(2, 1)],
    );
    e.go(); // x actief
    e.move_cue(0, false).unwrap(); // x en y wisselen
    assert!(matches!(e.states()[1], CueState::Active { .. }));
    assert_eq!(e.show().cues[1].name, "x");
}

#[test]
fn update_cue_cannot_change_span_or_kind() {
    let (mut e, _) = engine(
        vec![group("g", GroupMode::Sequence), audio("a", 1.0)],
        &[(0, 1)],
    );
    e.update_cue(0, |c| {
        c.span = 7;
        c.kind = CueKind::Audio;
        c.group_mode = GroupMode::Parallel;
    })
    .unwrap();
    assert_eq!(e.show().cues[0].span, 1);
    assert_eq!(e.show().cues[0].kind, CueKind::Group);
    assert_eq!(e.show().cues[0].group_mode, GroupMode::Parallel);
}

// ---- ongedaan maken ------------------------------------------------------------------

#[test]
fn restore_cues_keeps_state_of_surviving_cues() {
    let (mut e, _) = engine(vec![audio("a", 5.0), audio("b", 5.0)], &[]);
    let before = e.show().cues.clone();
    e.go(); // a actief
    e.add_cue(CueKind::Audio, Some(1)).unwrap();
    assert_eq!(e.states().len(), 3);
    e.restore_cues(before).unwrap();
    assert_eq!(e.states().len(), 2);
    assert!(matches!(e.states()[0], CueState::Active { .. }));
}

#[test]
fn restore_refuses_to_drop_an_active_cue() {
    let (mut e, _) = engine(vec![audio("a", 5.0), audio("b", 5.0)], &[]);
    e.go();
    let mut without_a = e.show().cues.clone();
    without_a.remove(0);
    assert_eq!(e.restore_cues(without_a), Err(EditError::CueActive));
    assert_eq!(e.show().cues.len(), 2);
}

#[test]
fn duplicate_stays_in_the_same_group_and_groups_refuse() {
    let (mut e, _) = engine(
        vec![
            group("g", GroupMode::Sequence),
            audio("a", 1.0),
            audio("z", 1.0),
        ],
        &[(0, 1)],
    );
    assert_eq!(e.duplicate_cue(1), Ok(2));
    assert_eq!(names(&e), ["g[2]", "a", "a (kopie)", "z"]);
    assert_ne!(id(&e, 1), id(&e, 2));
    assert_eq!(e.duplicate_cue(3), Ok(4));
    assert_eq!(
        e.show().cues[0].span,
        2,
        "kopie van een gewone cue buiten de groep blijft erbuiten"
    );
    assert_eq!(e.duplicate_cue(0), Err(EditError::Unsupported));
}
