//! Platformverbinding met mpv's IPC-server: Unix-socket of Windows named pipe.

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[cfg(unix)]
pub type Stream = std::os::unix::net::UnixStream;
#[cfg(windows)]
pub type Stream = std::fs::File;

static INSTANCE: AtomicUsize = AtomicUsize::new(0);

/// Unieke naam voor `--input-ipc-server` (per proces, per speler).
pub fn endpoint(pid: u32, display: usize) -> String {
    let display = format!("{}-{display}", INSTANCE.fetch_add(1, Ordering::Relaxed));
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\showmate-{pid}-{display}")
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir()
            .join(format!("showmate-{pid}-{display}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

fn connect_once(endpoint: &str) -> io::Result<Stream> {
    #[cfg(unix)]
    {
        Stream::connect(endpoint)
    }
    #[cfg(windows)]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(endpoint)
    }
}

/// Wacht tot mpv zijn IPC-server heeft aangemaakt en verbindt. Stopt eerder als `give_up()`
/// `true` geeft (bv. omdat het mpv-proces al is afgesloten).
pub fn connect(
    endpoint: &str,
    timeout: Duration,
    give_up: impl Fn() -> bool,
) -> io::Result<Stream> {
    let start = Instant::now();
    loop {
        match connect_once(endpoint) {
            Ok(s) => return Ok(s),
            Err(e) if start.elapsed() >= timeout || give_up() => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

/// Ruimt een achtergebleven socketbestand op (Unix); op Windows verdwijnt de pipe vanzelf.
pub fn cleanup(endpoint: &str) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(endpoint);
    }
    #[cfg(windows)]
    {
        let _ = endpoint;
    }
}
