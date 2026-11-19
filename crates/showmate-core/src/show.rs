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

    // ---- groepen ----------------------------------------------------------------

    /// Header van de groep waar cue `index` kind van is.
    pub fn parent_of(&self, index: usize) -> Option<usize> {
        (0..index.min(self.cues.len())).rev().find(|&h| {
            let c = &self.cues[h];
            c.kind == CueKind::Group && h + c.span >= index
        })
    }

    pub fn is_child(&self, index: usize) -> bool {
        self.parent_of(index).is_some()
    }

    /// Aantal cues van het blok op `index` (een gewone cue = 1; een groep = 1 + kinderen).
    pub fn block_len(&self, index: usize) -> usize {
        match self.cues.get(index) {
            Some(c) if c.kind == CueKind::Group => 1 + c.span,
            _ => 1,
        }
    }

    /// Herstelt ongeldige groepsstructuur (na het laden van een bewerkt/beschadigd bestand):
    /// `span` past binnen de lijst, alleen groepen hebben kinderen, geen groepen in groepen.
    pub fn sanitize(&mut self) {
        let n = self.cues.len();
        let mut i = 0;
        while i < n {
            if self.cues[i].kind != CueKind::Group {
                self.cues[i].span = 0;
                i += 1;
                continue;
            }
            let mut span = self.cues[i].span.min(n - 1 - i);
            if let Some(nested) = (1..=span).find(|k| self.cues[i + k].kind == CueKind::Group) {
                span = nested - 1;
            }
            self.cues[i].span = span;
            for k in 1..=span {
                self.cues[i + k].span = 0;
            }
            i += 1 + span;
        }
    }

    pub fn to_json(&self) -> Result<String, ShowError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> Result<Self, ShowError> {
        let mut show: Show = serde_json::from_str(json)?;
        if show.version > FORMAT_VERSION {
            return Err(ShowError::UnsupportedVersion(show.version));
        }
        show.sanitize();
        Ok(show)
    }

    /// Schrijft atomair: eerst naar een tijdelijk bestand, dan hernoemen. De vorige versie
    /// blijft als `<bestand>.bak` bewaard en een eventuele automatische kopie wordt opgeruimd.
    pub fn save(&self, path: &Path) -> Result<(), ShowError> {
        let tmp = sibling(path, ".tmp");
        std::fs::write(&tmp, self.to_json()?)?;
        if path.exists() {
            let _ = std::fs::copy(path, sibling(path, ".bak"));
        }
        std::fs::rename(&tmp, path)?;
        let _ = std::fs::remove_file(autosave_path(path));
        Ok(())
    }

    /// Schrijft een automatische kopie naast `path` (de echte showfile blijft ongemoeid).
    pub fn autosave(&self, path: &Path) -> Result<(), ShowError> {
        let target = autosave_path(path);
        let tmp = sibling(&target, ".tmp");
        std::fs::write(&tmp, self.to_json()?)?;
        std::fs::rename(&tmp, &target)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ShowError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}

fn sibling(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    os.into()
}

/// Pad van de automatische kopie bij een showfile.
pub fn autosave_path(path: &Path) -> std::path::PathBuf {
    sibling(path, ".autosave")
}

/// Bestaat er een automatische kopie die nieuwer is dan de showfile (of is er geen showfile)?
/// Dan is ShowMate waarschijnlijk gesloten of gecrasht met niet-opgeslagen wijzigingen.
pub fn recoverable_autosave(path: &Path) -> Option<std::path::PathBuf> {
    let auto = autosave_path(path);
    let auto_time = std::fs::metadata(&auto).and_then(|m| m.modified()).ok()?;
    match std::fs::metadata(path).and_then(|m| m.modified()) {
        Ok(show_time) if show_time > auto_time => None,
        _ => Some(auto),
    }
}
