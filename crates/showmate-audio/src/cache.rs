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

/// Laadt en decodeert clips op achtergrondthreads, geresampled naar de uitvoerrate.
#[derive(Clone)]
pub struct ClipCache {
    rate: u32,
    inner: Arc<Mutex<HashMap<PathBuf, ClipStatus>>>,
}

impl ClipCache {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            rate: sample_rate,
            inner: Arc::default(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Start (indien nodig) het laden van `path`. Meerdere aanroepen zijn goedkoop.
    pub fn request(&self, path: &Path) {
        {
            let mut map = self.inner.lock().expect("cache-lock");
            if map.contains_key(path) {
                return;
            }
            map.insert(path.to_path_buf(), ClipStatus::Loading);
        }
        let (inner, rate, path) = (self.inner.clone(), self.rate, path.to_path_buf());
        std::thread::spawn(move || {
            let status = match decode_file(&path) {
                Ok(clip) => ClipStatus::Ready(Arc::new(clip.resampled(rate))),
                Err(e) => ClipStatus::Failed(e.to_string()),
            };
            inner.lock().expect("cache-lock").insert(path, status);
        });
    }

    pub fn status(&self, path: &Path) -> ClipStatus {
        self.inner
            .lock()
            .expect("cache-lock")
            .get(path)
            .cloned()
            .unwrap_or(ClipStatus::Unknown)
    }

    /// Vergeet een pad (bv. nadat het bestand is vervangen) zodat het opnieuw geladen wordt.
    pub fn forget(&self, path: &Path) {
        self.inner.lock().expect("cache-lock").remove(path);
    }
}
