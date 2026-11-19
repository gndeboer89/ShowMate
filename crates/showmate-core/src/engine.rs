//! De cue-engine: een state machine die cues plant en start op basis van showtijd.
//!
//! Semantiek
//! - **Showtijd** loopt vanaf de eerste GO en staat stil tijdens pauze. Alle
//!   planning gebeurt in showtijd, zodat pauzeren alles gelijk bevriest.
//! - **GO** plant de eerste nog wachtende cue (`start + delay`). Direct volgende
//!   cues met `StartMode::Auto` worden meegepland, elk `delay` na de start van
//!   de vorige. De eerstvolgende `Go`-cue wacht op de volgende GO.
//! - Een cue eindigt na `duration`. Zonder duur eindigen Osc/Midi-cues direct;
//!   Audio/Video eindigen na de gemelde mediaduur, of lopen anders tot STOP. `looping` cues eindigen nooit vanzelf.
//! - **Groepen** (`CueKind::Group`) bevatten de `span` cues direct na de header.
//!   `Parallel` start alle kinderen tegelijk (elk `delay` na de groepsstart), `Sequence`
//!   start ze na elkaar (elk `delay` nadat het vorige is afgelopen) en `Loop` herhaalt dat
//!   tot STOP of tot de duur van de groep om is. Een groep eindigt als al zijn kinderen
//!   klaar zijn. De `start`-modus van kinderen telt niet; GO slaat kinderen over.
//! - De engine wordt aangedreven door [`Engine::tick`]; zij doet zelf geen I/O.

use crate::clock::Clock;
use crate::cue::{Cue, CueId, CueKind, GroupMode, StartMode};
use crate::show::Show;
use std::collections::HashMap;
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
    /// Alles is gepauzeerd (uitvoerlaag moet bevriezen).
    Paused,
    Resumed,
}

/// Waarom een bewerking is geweigerd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("cues zijn gepland; druk eerst op STOP om de lijst te wijzigen")]
    Busy,
    #[error("deze cue is actief; stop hem eerst")]
    CueActive,
    #[error("cue bestaat niet")]
    OutOfRange,
    #[error("een groep kan niet in een groep")]
    Nested,
    #[error("een groep kan niet gedupliceerd worden")]
    Unsupported,
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
    /// Door de uitvoerlaag gemelde mediaduur, gebruikt als een cue zelf geen duur heeft.
    media_len: HashMap<CueId, Duration>,
    /// Lopende groepen.
    groups: HashMap<CueId, GroupRun>,
    /// Einde van afgelopen cues (showtijd), nodig om de volgende in een reeks te plannen.
    ended_at: HashMap<CueId, Duration>,
}

#[derive(Debug, Clone, Copy)]
struct GroupRun {
    /// Start van de huidige ronde (voor Loop: bewaking tegen eindeloze nul-lengte rondes).
    pass_start: Duration,
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
            media_len: HashMap::new(),
            groups: HashMap::new(),
            ended_at: HashMap::new(),
        }
    }

    /// Hernoemt de show (bv. na 'Opslaan als').
    pub fn set_show_name(&mut self, name: impl Into<String>) {
        self.show.name = name.into();
    }

    /// Meldt de lengte van het media-bestand van een cue (bv. na het laden van audio).
    /// Wordt gebruikt zolang de cue geen eigen duur heeft en niet loopt.
    pub fn set_media_duration(&mut self, id: CueId, len: Duration) {
        self.media_len.insert(id, len);
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
        let n = self.states.len();
        let playhead =
            (0..n).find(|&i| self.states[i] == CueState::Waiting && !self.show.is_child(i));
        if let Some(first) = playhead {
            let mut start_at = self.show_time + self.show.cues[first].delay;
            self.states[first] = CueState::Scheduled { start_at };
            let mut i = first + self.show.block_len(first);
            while i < n
                && self.states[i] == CueState::Waiting
                && self.show.cues[i].start == StartMode::Auto
            {
                start_at += self.show.cues[i].delay;
                self.states[i] = CueState::Scheduled { start_at };
                i += self.show.block_len(i);
            }
        }
        events.extend(self.tick()); // cues zonder delay starten meteen
        events
    }

    pub fn pause(&mut self) -> Vec<Event> {
        let mut events = self.tick();
        if !self.paused {
            self.paused = true;
            events.push(Event::Paused);
        }
        events
    }

    pub fn resume(&mut self) -> Vec<Event> {
        let mut events = self.tick(); // synchroniseert de klok zonder showtijd op te tellen
        if self.paused {
            self.paused = false;
            events.push(Event::Resumed);
        }
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
        self.groups.clear();
        if self.paused {
            self.paused = false;
            events.push(Event::Resumed);
        }
        events
    }

    /// Meldt dat de uitvoerlaag klaar is met een cue (bv. einde van een videobestand).
    /// Alleen actieve cues worden afgerond; verder gebeurt er niets.
    pub fn complete(&mut self, id: CueId) -> Vec<Event> {
        let Some(i) = self.show.cues.iter().position(|c| c.id == id) else {
            return Vec::new();
        };
        if matches!(self.states[i], CueState::Active { .. }) {
            self.states[i] = CueState::Finished;
            self.ended_at.insert(id, self.show_time);
            let mut events = vec![Event::CueFinished(id)];
            events.extend(self.tick()); // volgende cue in een groep kan nu starten
            events
        } else {
            Vec::new()
        }
    }

    /// Zet de hele show terug naar het begin (alle cues wachtend, showtijd 0).
    pub fn reset(&mut self) {
        self.states.fill(CueState::Waiting);
        self.show_time = Duration::ZERO;
        self.last_clock = self.clock.now();
        self.started = false;
        self.paused = false;
        self.groups.clear();
        self.ended_at.clear();
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
        // Herhalen tot er niets meer verandert: een kind dat net klaar is kan de volgende
        // starten, die op zijn beurt (bij 0 delay) meteen weer klaar kan zijn.
        for _ in 0..10_000 {
            let mut changed = self.pass_cues(&mut events);
            changed |= self.pass_groups(&mut events);
            if !changed {
                break;
            }
        }
        events
    }

    /// Scheduled → Active en Active → Finished op basis van de showtijd.
    fn pass_cues(&mut self, events: &mut Vec<Event>) -> bool {
        let mut changed = false;
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
                changed = true;
            }
            if let CueState::Active { started_at } = self.states[i]
                && let Some(end) = self.end_time(i, started_at)
                && end <= self.show_time
            {
                if self.show.cues[i].kind == CueKind::Group {
                    self.cap_group(i, end, events);
                } else {
                    self.states[i] = CueState::Finished;
                    self.ended_at.insert(id, end);
                    events.push(Event::CueFinished(id));
                }
                changed = true;
            }
        }
        changed
    }

    /// De duur van een groep is om: stop de kinderen en rond de groep af.
    fn cap_group(&mut self, header: usize, end: Duration, events: &mut Vec<Event>) {
        let span = self.show.cues[header].span;
        for k in header + 1..=header + span {
            match self.states[k] {
                CueState::Active { .. } => {
                    self.states[k] = CueState::Stopped;
                    events.push(Event::CueStopped(self.show.cues[k].id));
                }
                CueState::Scheduled { .. } => self.states[k] = CueState::Waiting,
                _ => {}
            }
        }
        self.finish_group(header, end, events);
    }

    fn finish_group(&mut self, header: usize, end: Duration, events: &mut Vec<Event>) {
        let id = self.show.cues[header].id;
        self.states[header] = CueState::Finished;
        self.ended_at.insert(id, end);
        self.groups.remove(&id);
        events.push(Event::CueFinished(id));
    }

    /// Plant en beëindigt de kinderen van actieve groepen.
    fn pass_groups(&mut self, events: &mut Vec<Event>) -> bool {
        let mut changed = false;
        for h in 0..self.states.len() {
            let CueState::Active { started_at } = self.states[h] else {
                continue;
            };
            let cue = &self.show.cues[h];
            if cue.kind != CueKind::Group {
                continue;
            }
            let (id, mode, span) = (cue.id, cue.group_mode, cue.span);
            if span == 0 {
                self.finish_group(h, started_at, events);
                changed = true;
                continue;
            }
            let children = h + 1..=h + span;
            let delay = |engine: &Self, k: usize| engine.show.cues[k].delay;

            // Eerste keer: kinderen plannen. (De entry-API past hier niet: het blok hieronder
            // gebruikt `self` opnieuw.)
            #[allow(clippy::map_entry)]
            if !self.groups.contains_key(&id) {
                self.groups.insert(
                    id,
                    GroupRun {
                        pass_start: started_at,
                    },
                );
                match mode {
                    GroupMode::Parallel => {
                        for k in children.clone() {
                            if self.states[k] == CueState::Waiting {
                                let start_at = started_at + delay(self, k);
                                self.states[k] = CueState::Scheduled { start_at };
                            }
                        }
                    }
                    GroupMode::Sequence | GroupMode::Loop => {
                        if self.states[h + 1] == CueState::Waiting {
                            let start_at = started_at + delay(self, h + 1);
                            self.states[h + 1] = CueState::Scheduled { start_at };
                        }
                    }
                }
                changed = true;
                continue;
            }

            let done = |s: CueState| matches!(s, CueState::Finished | CueState::Stopped);
            let last_end = children
                .clone()
                .filter_map(|k| self.ended_at.get(&self.show.cues[k].id).copied())
                .max()
                .unwrap_or(started_at);

            match mode {
                GroupMode::Parallel => {
                    if children.clone().all(|k| done(self.states[k])) {
                        self.finish_group(h, last_end, events);
                        changed = true;
                    }
                }
                GroupMode::Sequence | GroupMode::Loop => {
                    match children.clone().find(|&k| !done(self.states[k])) {
                        Some(k) => {
                            if self.states[k] == CueState::Waiting {
                                let prev_end = if k == h + 1 {
                                    self.groups[&id].pass_start
                                } else {
                                    self.ended_at
                                        .get(&self.show.cues[k - 1].id)
                                        .copied()
                                        .unwrap_or(started_at)
                                };
                                let start_at = prev_end + delay(self, k);
                                self.states[k] = CueState::Scheduled { start_at };
                                changed = true;
                            }
                        }
                        None if mode == GroupMode::Loop => {
                            // Nieuwe ronde, tenzij de vorige geen tijd kostte (anders eindeloze lus).
                            let pass_start = self.groups[&id].pass_start;
                            if last_end <= pass_start {
                                self.finish_group(h, last_end, events);
                            } else {
                                for k in children.clone() {
                                    self.states[k] = CueState::Waiting;
                                }
                                self.groups.insert(
                                    id,
                                    GroupRun {
                                        pass_start: last_end,
                                    },
                                );
                            }
                            changed = true;
                        }
                        None => {
                            self.finish_group(h, last_end, events);
                            changed = true;
                        }
                    }
                }
            }
        }
        changed
    }

    fn end_time(&self, index: usize, started_at: Duration) -> Option<Duration> {
        let cue = &self.show.cues[index];
        if cue.kind == CueKind::Group {
            return cue.duration.map(|d| started_at + d);
        }
        if cue.looping {
            return None;
        }
        match cue.duration {
            Some(d) => Some(started_at + d),
            None if matches!(cue.kind, CueKind::Osc | CueKind::Midi) => Some(started_at),
            None => self.media_len.get(&cue.id).map(|len| started_at + *len),
        }
    }

    // ---- bewerken ----------------------------------------------------
    //
    // Structurele wijzigingen (invoegen, verwijderen, verplaatsen) zijn alleen
    // toegestaan zonder geplande cues, zodat de planning nooit verschuift.
    // Runtime-status schuift mee met de cue.

    fn ensure_idle(&self) -> Result<(), EditError> {
        if self
            .states
            .iter()
            .any(|s| matches!(s, CueState::Scheduled { .. }))
        {
            Err(EditError::Busy)
        } else {
            Ok(())
        }
    }

    /// Voegt een cue in op `index` (0..=len). Valt de positie binnen een groep, dan wordt
    /// het een kind van die groep (een groep zelf kan niet in een groep).
    pub fn insert_cue(&mut self, index: usize, cue: Cue) -> Result<CueId, EditError> {
        let parent = (1..=index).rev().find_map(|h| {
            let h = h - 1;
            let c = self.show.cues.get(h)?;
            (c.kind == CueKind::Group && h < index && index <= h + c.span).then_some(h)
        });
        self.insert_at(index, cue, parent)
    }

    fn insert_at(
        &mut self,
        index: usize,
        mut cue: Cue,
        parent: Option<usize>,
    ) -> Result<CueId, EditError> {
        self.ensure_idle()?;
        if index > self.show.cues.len() {
            return Err(EditError::OutOfRange);
        }
        if parent.is_some() && cue.kind == CueKind::Group {
            return Err(EditError::Nested);
        }
        cue.span = 0;
        let id = self.show.insert(index, cue);
        self.states.insert(index, CueState::Waiting);
        if let Some(h) = parent {
            self.show.cues[h].span += 1;
        }
        Ok(id)
    }

    /// Voegt een nieuwe standaardcue toe en geeft de index terug. Onder een geselecteerde
    /// cue (`after`): in een groep (of na de laatste cue van die groep) wordt het een kind;
    /// een nieuwe groep komt altijd op het hoogste niveau, na het blok van de selectie.
    pub fn add_cue(&mut self, kind: CueKind, after: Option<usize>) -> Result<usize, EditError> {
        let n = self.show.cues.len();
        let (index, parent) = match after.filter(|_| n > 0).map(|a| a.min(n - 1)) {
            None => (n, None),
            Some(a) => {
                let header = self
                    .show
                    .parent_of(a)
                    .or((self.show.cues[a].kind == CueKind::Group).then_some(a));
                match header {
                    Some(h) if kind == CueKind::Group => (h + 1 + self.show.cues[h].span, None),
                    Some(h) => (a + 1, Some(h)),
                    None => (a + 1, None),
                }
            }
        };
        self.insert_at(index, Cue::template(CueId(0), kind), parent)?;
        Ok(index)
    }

    /// Dupliceert een cue (niet-groep) direct eronder, in dezelfde groep. Geeft de nieuwe index terug.
    pub fn duplicate_cue(&mut self, index: usize) -> Result<usize, EditError> {
        let src = self.show.cues.get(index).ok_or(EditError::OutOfRange)?;
        if src.kind == CueKind::Group {
            return Err(EditError::Unsupported);
        }
        let mut copy = src.clone();
        copy.name = format!("{} (kopie)", copy.name);
        let parent = self.show.parent_of(index);
        self.insert_at(index + 1, copy, parent)?;
        Ok(index + 1)
    }

    /// Verwijdert een cue. Een groep wordt opgeheven (de kinderen blijven staan);
    /// een kind verdwijnt uit zijn groep.
    pub fn remove_cue(&mut self, index: usize) -> Result<Cue, EditError> {
        self.ensure_idle()?;
        match self.states.get(index) {
            None => return Err(EditError::OutOfRange),
            Some(CueState::Active { .. }) => return Err(EditError::CueActive),
            Some(_) => {}
        }
        if let Some(h) = self.show.parent_of(index) {
            self.show.cues[h].span -= 1;
        }
        self.states.remove(index);
        Ok(self.show.cues.remove(index))
    }

    /// Verplaatst een cue één plek omhoog of omlaag en geeft de nieuwe index terug.
    /// - Gewone cue of groep (met al zijn kinderen): wisselt met de buureenheid op hoogste niveau.
    /// - Kind: wisselt met zijn buur; aan de rand verlaat het de groep.
    pub fn move_cue(&mut self, index: usize, up: bool) -> Result<usize, EditError> {
        self.ensure_idle()?;
        let n = self.show.cues.len();
        if index >= n {
            return Err(EditError::OutOfRange);
        }
        if let Some(h) = self.show.parent_of(index) {
            let last = h + self.show.cues[h].span;
            return Ok(if up && index == h + 1 {
                // Boven de groep uit.
                self.show.cues[h..=index].rotate_right(1);
                self.states[h..=index].rotate_right(1);
                self.show.cues[h + 1].span -= 1;
                h
            } else if !up && index == last {
                // Onder de groep uit: staat er al, alleen de groep wordt korter.
                self.show.cues[h].span -= 1;
                index
            } else {
                let t = if up { index - 1 } else { index + 1 };
                self.show.cues.swap(index, t);
                self.states.swap(index, t);
                t
            });
        }
        let len = self.show.block_len(index);
        if up {
            if index == 0 {
                return Ok(index);
            }
            let prev = self.show.parent_of(index - 1).unwrap_or(index - 1);
            self.show.cues[prev..index + len].rotate_left(index - prev);
            self.states[prev..index + len].rotate_left(index - prev);
            Ok(prev)
        } else {
            let end = index + len;
            if end >= n {
                return Ok(index);
            }
            let next_len = self.show.block_len(end);
            self.show.cues[index..end + next_len].rotate_left(len);
            self.states[index..end + next_len].rotate_left(len);
            Ok(index + next_len)
        }
    }

    /// Past de eigenschappen van een cue aan. Mag altijd; id, soort en groepsgrootte blijven behouden.
    /// Een al geplande starttijd verandert niet; de nieuwe waarden gelden voor
    /// latere planning en voor het einde van een lopende cue.
    pub fn update_cue(&mut self, index: usize, f: impl FnOnce(&mut Cue)) -> Result<(), EditError> {
        let cue = self.show.cues.get_mut(index).ok_or(EditError::OutOfRange)?;
        let (id, kind, span) = (cue.id, cue.kind, cue.span);
        f(cue);
        (cue.id, cue.kind, cue.span) = (id, kind, span);
        Ok(())
    }

    /// Vervangt de cuelijst door een eerdere stand (ongedaan maken / opnieuw). De status
    /// van cues die blijven bestaan blijft behouden. Niet toegestaan bij geplande cues, of
    /// als een actieve cue zou verdwijnen.
    pub fn restore_cues(&mut self, mut cues: Vec<Cue>) -> Result<(), EditError> {
        self.ensure_idle()?;
        let old: HashMap<CueId, CueState> = self
            .show
            .cues
            .iter()
            .map(|c| c.id)
            .zip(self.states.iter().copied())
            .collect();
        let keeps = |id: &CueId| cues.iter().any(|c| c.id == *id);
        if old
            .iter()
            .any(|(id, s)| matches!(s, CueState::Active { .. }) && !keeps(id))
        {
            return Err(EditError::CueActive);
        }
        self.states = cues
            .iter()
            .map(|c| old.get(&c.id).copied().unwrap_or(CueState::Waiting))
            .collect();
        std::mem::swap(&mut self.show.cues, &mut cues);
        self.show.sanitize();
        Ok(())
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
            .filter(|(index, _)| !self.show.is_child(*index))
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
