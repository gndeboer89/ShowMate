//! De cue-engine: een state machine die cues plant en start op basis van showtijd.
//!
//! Semantiek
//! - **Showtijd** loopt vanaf de eerste GO en staat stil tijdens pauze. Alle
//!   planning gebeurt in showtijd, zodat pauzeren alles gelijk bevriest.
//! - **GO** plant de eerste nog wachtende cue (`start + delay`). Direct volgende
//!   cues met `StartMode::Auto` worden meegepland, elk `delay` na de start van
//!   de vorige. De eerstvolgende `Go`-cue wacht op de volgende GO.
//! - Een cue eindigt na `duration`. Zonder duur eindigen Osc/Midi-cues direct;
//!   Audio/Video lopen tot STOP. `looping` cues eindigen nooit vanzelf.
//! - De engine wordt aangedreven door [`Engine::tick`]; zij doet zelf geen I/O.

use crate::clock::Clock;
use crate::cue::{CueId, CueKind, StartMode};
use crate::show::Show;
use std::time::Duration;

/// Runtime-status van één cue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueState {
    /// Nog niet gepland (wacht op GO).
    Waiting,
    /// Gepland; start op `start_at` (showtijd). In de UI: "Wachtend".
    Scheduled { start_at: Duration },
    /// Loopt sinds `started_at` (showtijd).
    Active { started_at: Duration },
    /// Vanzelf afgelopen.
    Finished,
    /// Handmatig gestopt.
    Stopped,
}

/// Gebeurtenissen voor de uitvoerlaag (audio/video/OSC) en de UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    CueStarted(CueId),
    CueFinished(CueId),
    CueStopped(CueId),
}

/// De eerstvolgende cue die nog moet starten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextCue {
    pub index: usize,
    /// Tijd tot start als de cue al gepland is; `None` = wacht op GO.
    pub starts_in: Option<Duration>,
}

pub struct Engine<C: Clock> {
    clock: C,
    show: Show,
    states: Vec<CueState>,
    show_time: Duration,
    last_clock: Duration,
    started: bool,
    paused: bool,
}

impl<C: Clock> Engine<C> {
    pub fn new(clock: C, show: Show) -> Self {
        let last_clock = clock.now();
        let states = vec![CueState::Waiting; show.cues.len()];
        Self {
            clock,
            show,
            states,
            show_time: Duration::ZERO,
            last_clock,
            started: false,
            paused: false,
        }
    }

    // ---- bediening ---------------------------------------------------

    /// Start de volgende wachtende cue (plus bijbehorende auto-follow keten).
    /// Genegeerd tijdens pauze.
    pub fn go(&mut self) -> Vec<Event> {
        let mut events = self.tick();
        if self.paused {
            return events;
        }
        self.started = true;
        if let Some(first) = self.states.iter().position(|s| *s == CueState::Waiting) {
            let mut start_at = self.show_time + self.show.cues[first].delay;
            self.states[first] = CueState::Scheduled { start_at };
            let mut i = first + 1;
            while i < self.states.len()
                && self.states[i] == CueState::Waiting
                && self.show.cues[i].start == StartMode::Auto
            {
                start_at += self.show.cues[i].delay;
                self.states[i] = CueState::Scheduled { start_at };
                i += 1;
            }
        }
        events.extend(self.tick()); // cues zonder delay starten meteen
        events
    }

    pub fn pause(&mut self) -> Vec<Event> {
        let events = self.tick();
        self.paused = true;
        events
    }

    pub fn resume(&mut self) -> Vec<Event> {
        let events = self.tick(); // synchroniseert de klok zonder showtijd op te tellen
        self.paused = false;
        events
    }

    pub fn toggle_pause(&mut self) -> Vec<Event> {
        if self.paused {
            self.resume()
        } else {
            self.pause()
        }
    }

    /// Stopt alles: actieve cues worden `Stopped`, geplande cues weer `Waiting`.
    /// De showtijd blijft doorlopen; eerder gespeelde cues blijven gespeeld.
    pub fn stop(&mut self) -> Vec<Event> {
        let mut events = self.tick();
        for (i, state) in self.states.iter_mut().enumerate() {
            match *state {
                CueState::Active { .. } => {
                    *state = CueState::Stopped;
                    events.push(Event::CueStopped(self.show.cues[i].id));
                }
                CueState::Scheduled { .. } => *state = CueState::Waiting,
                _ => {}
            }
        }
        self.paused = false;
        events
    }

    /// Zet de hele show terug naar het begin (alle cues wachtend, showtijd 0).
    pub fn reset(&mut self) {
        self.states.fill(CueState::Waiting);
        self.show_time = Duration::ZERO;
        self.last_clock = self.clock.now();
        self.started = false;
        self.paused = false;
    }

    // ---- voortgang ---------------------------------------------------

    /// Brengt de engine bij tot de huidige klokstand en geeft nieuwe events terug.
    /// Aanroepen vanuit de engine-thread, bv. elke 5–20 ms.
    pub fn tick(&mut self) -> Vec<Event> {
        let now = self.clock.now();
        let delta = now.saturating_sub(self.last_clock);
        self.last_clock = now;
        if !self.started || self.paused {
            return Vec::new();
        }
        self.show_time += delta;

        let mut events = Vec::new();
        for i in 0..self.states.len() {
            let id = self.show.cues[i].id;
            if let CueState::Scheduled { start_at } = self.states[i]
                && start_at <= self.show_time
            {
                // Exacte geplande tijd, niet `now`: voorkomt drift in de keten.
                self.states[i] = CueState::Active {
                    started_at: start_at,
                };
                events.push(Event::CueStarted(id));
            }
            if let CueState::Active { started_at } = self.states[i]
                && self
                    .end_time(i, started_at)
                    .is_some_and(|end| end <= self.show_time)
            {
                self.states[i] = CueState::Finished;
                events.push(Event::CueFinished(id));
            }
        }
        events
    }

    fn end_time(&self, index: usize, started_at: Duration) -> Option<Duration> {
        let cue = &self.show.cues[index];
        if cue.looping {
            return None;
        }
        match cue.duration {
            Some(d) => Some(started_at + d),
            None if matches!(cue.kind, CueKind::Osc | CueKind::Midi) => Some(started_at),
            None => None,
        }
    }

    // ---- status ------------------------------------------------------

    pub fn show(&self) -> &Show {
        &self.show
    }

    pub fn states(&self) -> &[CueState] {
        &self.states
    }

    pub fn show_time(&self) -> Duration {
        self.show_time
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// De eerste cue die nog niet gestart is, met aftelling als hij al gepland is.
    pub fn next_cue(&self) -> Option<NextCue> {
        self.states
            .iter()
            .enumerate()
            .find_map(|(index, s)| match *s {
                CueState::Waiting => Some(NextCue {
                    index,
                    starts_in: None,
                }),
                CueState::Scheduled { start_at } => Some(NextCue {
                    index,
                    starts_in: Some(start_at.saturating_sub(self.show_time)),
                }),
                _ => None,
            })
    }

    /// Voortgang van een actieve cue: (verstreken, totaal).
    pub fn progress(&self, index: usize) -> Option<(Duration, Option<Duration>)> {
        match self.states.get(index)? {
            CueState::Active { started_at } => Some((
                self.show_time.saturating_sub(*started_at),
                self.show.cues[index].duration,
            )),
            _ => None,
        }
    }
}
