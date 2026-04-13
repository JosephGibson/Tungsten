use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Debounce window: collapse rapid successive writes (e.g. editor swap-saves)
/// into a single reload event per path per window.
const DEBOUNCE_MS: u64 = 50;

/// Watches an assets directory on a background thread and forwards file-change
/// events to the main thread via `std::sync::mpsc`. No Arc<Mutex>, no async.
pub struct HotReloadWatcher {
    /// Kept alive so the OS watcher doesn't stop when this struct is held.
    _watcher: RecommendedWatcher,
    receiver: Receiver<PathBuf>,
    /// Paths with pending events; value is the time of the last event seen.
    pending: HashMap<PathBuf, Instant>,
}

impl HotReloadWatcher {
    /// Start watching `watch_dir` recursively. Returns `None` if watcher
    /// setup fails (unsupported platform, permission error, etc.); the caller
    /// should log and run without hot reload.
    pub fn new(watch_dir: PathBuf) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<PathBuf>();

        let mut watcher = match notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                // Only react to data writes, not metadata or access events.
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    for path in event.paths {
                        let _ = tx.send(path);
                    }
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                log::error!("Hot reload: failed to create watcher: {e}");
                return None;
            }
        };

        if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::Recursive) {
            log::error!("Hot reload: failed to watch '{}': {e}", watch_dir.display());
            return None;
        }

        log::info!("Hot reload watching '{}'", watch_dir.display());
        Some(Self {
            _watcher: watcher,
            receiver: rx,
            pending: HashMap::new(),
        })
    }

    /// Drain all events whose debounce window has elapsed. Call once per frame,
    /// after tick() and before render(). Returns absolute file paths ready for
    /// processing.
    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let window = Duration::from_millis(DEBOUNCE_MS);

        // Absorb all queued events, resetting each path's deadline.
        loop {
            match self.receiver.try_recv() {
                Ok(path) => {
                    self.pending.insert(path, now);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    log::warn!("Hot reload: watcher channel disconnected");
                    break;
                }
            }
        }

        // Collect paths whose last event was more than DEBOUNCE_MS ago.
        let mut ready = Vec::new();
        self.pending.retain(|path, last_seen| {
            if now.duration_since(*last_seen) >= window {
                ready.push(path.clone());
                false
            } else {
                true
            }
        });
        ready
    }
}

#[cfg(test)]
mod tests {
    use super::DEBOUNCE_MS;

    #[test]
    fn debounce_constant_is_50ms() {
        assert_eq!(DEBOUNCE_MS, 50);
    }
}
