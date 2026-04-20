use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Debounce window: collapse rapid successive writes (e.g. editor swap-saves)
/// into a single reload event per path per window.
const DEBOUNCE_MS: u64 = 50;

/// Watches asset directories and optional extra files on a background thread
/// and forwards file-change events to the main thread via `std::sync::mpsc`.
/// No Arc<Mutex>, no async.
///
/// Recursive asset directories are watched as `RecursiveMode::Recursive`.
/// Extra files (e.g. workspace-root `input.json`) are handled by watching
/// their parent non-recursively and filtering in `drain_ready` so only the
/// explicit file bubbles up — keeping noise from unrelated files in the
/// same directory out of the reload path.
pub struct HotReloadWatcher {
    /// Kept alive so the OS watcher doesn't stop when this struct is held.
    _watcher: RecommendedWatcher,
    receiver: Receiver<PathBuf>,
    /// Paths with pending events; value is the time of the last event seen.
    pending: HashMap<PathBuf, Instant>,
    /// Canonical roots of recursive watches. A received path is accepted if
    /// it starts with any of these.
    recursive_roots: Vec<PathBuf>,
    /// Canonical paths of extra files watched via their parent directory.
    allowed_extra_files: HashSet<PathBuf>,
}

impl HotReloadWatcher {
    /// Start watching each directory in `watch_dirs` recursively and each
    /// path in `extra_files` non-recursively (via its parent). Returns
    /// `None` if watcher setup fails (unsupported platform, permission
    /// error, etc.); the caller should log and run without hot reload.
    /// Directories or extra-file parents that do not exist are skipped
    /// with a warning rather than failing.
    pub fn new(watch_dirs: &[PathBuf], extra_files: &[PathBuf]) -> Option<Self> {
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

        let mut recursive_roots = Vec::new();
        let mut watched_any = false;
        for dir in watch_dirs {
            if !dir.exists() {
                log::warn!("Hot reload: skipping non-existent dir '{}'", dir.display());
                continue;
            }
            if let Err(e) = watcher.watch(dir, RecursiveMode::Recursive) {
                log::error!("Hot reload: failed to watch '{}': {e}", dir.display());
                return None;
            }
            log::info!("Hot reload watching '{}'", dir.display());
            recursive_roots.push(canonical_or_clone(dir));
            watched_any = true;
        }

        // Extra files: watch each unique parent non-recursively. Register
        // the canonical file path so `drain_ready` can filter unrelated
        // events from the same parent directory.
        let mut allowed_extra_files = HashSet::new();
        let mut watched_parents: HashSet<PathBuf> = HashSet::new();
        for file in extra_files {
            let parent = match file.parent() {
                Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                _ => PathBuf::from("."),
            };
            if !parent.exists() {
                log::warn!(
                    "Hot reload: skipping extra-file parent '{}' (does not exist)",
                    parent.display()
                );
                continue;
            }
            let canon_parent = canonical_or_clone(&parent);
            if watched_parents.insert(canon_parent.clone()) {
                if let Err(e) = watcher.watch(&canon_parent, RecursiveMode::NonRecursive) {
                    log::error!(
                        "Hot reload: failed to watch extra-file parent '{}': {e}",
                        canon_parent.display()
                    );
                    continue;
                }
                log::info!(
                    "Hot reload watching extra-file parent '{}'",
                    canon_parent.display()
                );
            }
            allowed_extra_files.insert(canonical_or_clone(file));
            watched_any = true;
        }

        if !watched_any {
            log::warn!("Hot reload: no directories to watch — hot reload disabled");
            return None;
        }

        Some(Self {
            _watcher: watcher,
            receiver: rx,
            pending: HashMap::new(),
            recursive_roots,
            allowed_extra_files,
        })
    }

    /// Drain all events whose debounce window has elapsed. Call once per
    /// frame, after tick() and before render(). Returns absolute file paths
    /// ready for processing, filtered to events that fall under a recursive
    /// root or match an explicit extra file.
    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let window = Duration::from_millis(DEBOUNCE_MS);

        // Absorb all queued events, resetting each accepted path's deadline.
        // Events that fall outside our configured scope (e.g. unrelated files
        // in a non-recursively watched parent) are dropped here.
        loop {
            match self.receiver.try_recv() {
                Ok(path) => {
                    if self.accept(&path) {
                        self.pending.insert(path, now);
                    }
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

    fn accept(&self, path: &Path) -> bool {
        let canon = canonical_or_clone(path);
        if self.allowed_extra_files.contains(&canon) {
            return true;
        }
        self.recursive_roots
            .iter()
            .any(|root| canon.starts_with(root))
    }
}

fn canonical_or_clone(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::DEBOUNCE_MS;

    #[test]
    fn debounce_constant_is_50ms() {
        assert_eq!(DEBOUNCE_MS, 50);
    }
}
