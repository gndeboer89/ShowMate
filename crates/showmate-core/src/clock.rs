use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Bron van monotone tijd. De engine gebruikt uitsluitend deze trait,
/// nooit rechtstreeks `Instant::now()`.
pub trait Clock {
    /// Tijd sinds een willekeurig, vast startpunt. Loopt nooit terug.
    fn now(&self) -> Duration;
}

/// Echte klok op basis van `Instant`.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// Handmatig bestuurbare klok voor tests. Klonen delen dezelfde tijd.
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    nanos: Arc<AtomicU64>,
}

impl ManualClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn advance(&self, by: Duration) {
        self.nanos.fetch_add(by.as_nanos() as u64, Ordering::SeqCst);
    }

    pub fn advance_secs(&self, secs: f64) {
        self.advance(Duration::from_secs_f64(secs));
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
    }
}
