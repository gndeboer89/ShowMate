use crate::cue::{Cue, CueId, CueKind};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ShowError {
    #[error("kan showfile niet lezen/schrijven: {0}")]
    Io(#[from] std::io::Error),
    #[error("ongeldige showfile: {0}")]
    Format(#[from] serde_json::Error),
    #[error("showfile-versie {0} wordt niet ondersteund")]
    UnsupportedVersion(u32),
}

const FORMAT_VERSION: u32 = 1;

/// Een complete show: geordende lijst cues.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Show {
    #[serde(default = "default_version")]
    version: u32,
    pub name: String,
    pub cues: Vec<Cue>,
    next_id: u64,
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

impl Show {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: FORMAT_VERSION,
            name: name.into(),
            cues: Vec::new(),
            next_id: 1,
        }
    }

    /// Reserveert een nieuwe unieke cue-id.
    pub fn next_id(&mut self) -> CueId {
        let id = CueId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Voegt een lege cue toe en geeft een verwijzing terug om hem in te vullen.
    pub fn add_cue(&mut self, name: impl Into<String>, kind: CueKind) -> &mut Cue {
        let id = self.next_id();
        self.cues.push(Cue::new(id, name, kind));
        self.cues.last_mut().expect("zojuist toegevoegd")
    }

    /// Voegt een kant-en-klare cue toe (id wordt opnieuw uitgedeeld).
    pub fn push(&mut self, mut cue: Cue) -> CueId {
        cue.id = self.next_id();
        let id = cue.id;
        self.cues.push(cue);
        id
    }

    /// Voegt een cue in op `index` (clamp op de lengte) met een nieuwe id.
    pub fn insert(&mut self, index: usize, mut cue: Cue) -> CueId {
        cue.id = self.next_id();
        let id = cue.id;
        let index = index.min(self.cues.len());
        self.cues.insert(index, cue);
        id
    }

    pub fn to_json(&self) -> Result<String, ShowError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self, ShowError> {
        let show: Show = serde_json::from_str(json)?;
        if show.version > FORMAT_VERSION {
            return Err(ShowError::UnsupportedVersion(show.version));
        }
        Ok(show)
    }

    /// Schrijft atomair: eerst naar een tijdelijk bestand, dan hernoemen.
    pub fn save(&self, path: &Path) -> Result<(), ShowError> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_json()?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ShowError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}
