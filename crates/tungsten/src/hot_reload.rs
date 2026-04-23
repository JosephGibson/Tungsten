use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Collapse editor swap-saves into one reload event per path.
const DEBOUNCE_MS: u64 = 50;

/// Background file watcher; `mpsc`, no async, no shared mutable state.
pub struct HotReloadWatcher {
    /// Keep OS watcher alive.
    _watcher: RecommendedWatcher,
    receiver: Receiver<PathBuf>,
    /// Pending path -> last event time.
    pending: HashMap<PathBuf, Instant>,
    /// Canonical recursive watch roots.
    recursive_roots: Vec<PathBuf>,
    /// Canonical extra files watched via parent directories.
    allowed_extra_files: HashSet<PathBuf>,
}

impl HotReloadWatcher {
    /// Start recursive dir watches plus parent watches for explicit files.
    #[must_use]
    pub fn new(watch_dirs: &[PathBuf], extra_files: &[PathBuf]) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<PathBuf>();

        let mut watcher = match notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
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

        // Extra files: watch unique parents, filter to explicit files later.
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

    /// Drain debounced, accepted paths; call once per frame before render.
    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let window = Duration::from_millis(DEBOUNCE_MS);

        // Accepted events reset the path debounce deadline.
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
        accept_path(path, &self.recursive_roots, &self.allowed_extra_files)
    }
}

fn canonical_or_clone(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Accept explicit extra files or paths under recursive roots.
fn accept_path(
    path: &Path,
    recursive_roots: &[PathBuf],
    allowed_extra_files: &HashSet<PathBuf>,
) -> bool {
    let canon = canonical_or_clone(path);
    if allowed_extra_files.contains(&canon) {
        return true;
    }
    recursive_roots.iter().any(|root| canon.starts_with(root))
}

#[cfg(test)]
#[path = "tests/hot_reload.rs"]
mod tests;
