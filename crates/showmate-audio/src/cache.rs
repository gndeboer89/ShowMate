use crate::clip::AudioClip;
use crate::decode::decode_file;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum ClipStatus {
    /// Nog nooit gevraagd.
    Unknown,
    Loading,
    Ready(Arc<AudioClip>),
    Failed(String),
}

/// Het gedecodeerde bestand plus de versies per samplerate (elke geluidskaart kan een andere rate hebben).
struct Loaded {
    original: Arc<AudioClip>,
    by_rate: Mutex<HashMap<u32, Arc<AudioClip>>>,
}

impl Loaded {
    fn at(&self, rate: u32) -> Arc<AudioClip> {
        if self.original.sample_rate() == rate {
            return self.original.clone();
        }
        if let Some(c) = self.by_rate.lock().expect("rate-lock").get(&rate) {
            return c.clone();
        }
        // Buiten het slot resamplen: dat kan even duren.
        let resampled = Arc::new(self.original.resampled(rate));
        self.by_rate
            .lock()
            .expect("rate-lock")
            .entry(rate)
            .or_insert(resampled)
            .clone()
    }
}

enum Entry {
    Loading,
    Ready(Arc<Loaded>),
    Failed(String),
}

/// Laadt en decodeert clips op achtergrondthreads en levert ze op elke gewenste samplerate.
#[derive(Clone, Default)]
pub struct ClipCache {
    inner: Arc<Mutex<HashMap<PathBuf, Entry>>>,
}

impl ClipCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start (indien nodig) het laden van `path` en bereidt de versies voor op `rates` voor
    /// (zodat GO niet hoeft te wachten op resamplen). Meerdere aanroepen zijn goedkoop.
    pub fn request(&self, path: &Path, rates: &[u32]) {
        let rates = rates.to_vec();
        let existing = {
            let mut map = self.inner.lock().expect("cache-lock");
            match map.get(path) {
                Some(Entry::Ready(l)) => Some(Some(l.clone())),
                Some(Entry::Loading | Entry::Failed(_)) => return,
                None => {
                    map.insert(path.to_path_buf(), Entry::Loading);
                    None
                }
            }
        };
        let (inner, path) = (self.inner.clone(), path.to_path_buf());
        std::thread::spawn(move || {
            let loaded = match existing.flatten() {
                Some(l) => l,
                None => match decode_file(&path) {
                    Ok(clip) => {
                        let l = Arc::new(Loaded {
                            original: Arc::new(clip),
                            by_rate: Mutex::default(),
                        });
                        inner
                            .lock()
                            .expect("cache-lock")
                            .insert(path, Entry::Ready(l.clone()));
                        l
                    }
                    Err(e) => {
                        inner
                            .lock()
                            .expect("cache-lock")
                            .insert(path, Entry::Failed(e.to_string()));
                        return;
                    }
                },
            };
            for rate in rates {
                loaded.at(rate);
            }
        });
    }

    fn entry_status(entry: Option<&Entry>, rate: Option<u32>) -> ClipStatus {
        match entry {
            None => ClipStatus::Unknown,
            Some(Entry::Loading) => ClipStatus::Loading,
            Some(Entry::Failed(e)) => ClipStatus::Failed(e.clone()),
            Some(Entry::Ready(l)) => ClipStatus::Ready(match rate {
                Some(r) => l.at(r),
                None => l.original.clone(),
            }),
        }
    }

    /// Het bestand op zijn eigen samplerate (voor golfvorm en duur).
    pub fn status(&self, path: &Path) -> ClipStatus {
        let loaded = {
            let map = self.inner.lock().expect("cache-lock");
            match map.get(path) {
                Some(Entry::Ready(l)) => Some(l.clone()),
                other => return Self::entry_status(other, None),
            }
        };
        ClipStatus::Ready(loaded.expect("Ready").original.clone())
    }

    /// Het bestand op samplerate `rate` (voor afspelen op een apparaat met die rate).
    pub fn status_at(&self, path: &Path, rate: u32) -> ClipStatus {
        let loaded = {
            let map = self.inner.lock().expect("cache-lock");
            match map.get(path) {
                Some(Entry::Ready(l)) => l.clone(),
                other => return Self::entry_status(other, None),
            }
        };
        ClipStatus::Ready(loaded.at(rate))
    }

    /// Vergeet een pad (bv. nadat het bestand is vervangen) zodat het opnieuw geladen wordt.
    pub fn forget(&self, path: &Path) {
        self.inner.lock().expect("cache-lock").remove(path);
    }
}
