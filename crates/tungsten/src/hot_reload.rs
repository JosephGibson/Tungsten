use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Collapse editor swap-saves into one reload event per path.
const DEBOUNCE_MS: u64 = 50;

/// What the watcher thread sends the main thread, which does the logging.
#[derive(Debug, PartialEq)]
enum WatchMessage {
    /// A file was created or modified.
    Changed(PathBuf),
    /// The watcher reported an error.
    Error(String),
    /// The OS dropped events (an inotify queue overflow, say), so some edits
    /// arrive as nothing.
    Rescan,
}

/// Turns one watcher callback into the messages it sends.
fn classify(result: notify::Result<Event>, mut send: impl FnMut(WatchMessage)) {
    match result {
        Ok(event) if event.need_rescan() => send(WatchMessage::Rescan),
        Ok(event) if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) => {
            for path in event.paths {
                send(WatchMessage::Changed(path));
            }
        }
        Ok(_) => {}
        Err(error) => send(WatchMessage::Error(error.to_string())),
    }
}

/// The main thread's side of the channel: debounced paths, and the reports
/// logged once each.
struct Inbox {
    receiver: Receiver<WatchMessage>,
    /// Pending path -> last event time.
    pending: HashMap<PathBuf, Instant>,
    /// Canonical recursive watch roots.
    recursive_roots: Vec<PathBuf>,
    /// Canonical extra files watched via parent directories.
    allowed_extra_files: HashSet<PathBuf>,
    /// A watch error has been logged; later ones are not.
    logged_error: bool,
    /// A rescan has been logged; later ones are not.
    logged_rescan: bool,
    /// The watcher thread has stopped; nothing more arrives.
    disconnected: bool,
}

impl Inbox {
    fn new(
        receiver: Receiver<WatchMessage>,
        recursive_roots: Vec<PathBuf>,
        allowed_extra_files: HashSet<PathBuf>,
    ) -> Self {
        Self {
            receiver,
            pending: HashMap::new(),
            recursive_roots,
            allowed_extra_files,
            logged_error: false,
            logged_rescan: false,
            disconnected: false,
        }
    }

    /// The paths whose debounce ended by `now`.
    fn drain(&mut self, now: Instant) -> Vec<PathBuf> {
        let window = Duration::from_millis(DEBOUNCE_MS);

        // Accepted events reset the path debounce deadline.
        loop {
            match self.receiver.try_recv() {
                Ok(WatchMessage::Changed(path)) => {
                    if accept_path(&path, &self.recursive_roots, &self.allowed_extra_files) {
                        self.pending.insert(path, now);
                    }
                }
                Ok(WatchMessage::Error(error)) => {
                    if !self.logged_error {
                        self.logged_error = true;
                        log::error!(
                            "Hot reload: watcher error: {error}; later errors are not logged"
                        );
                    }
                }
                Ok(WatchMessage::Rescan) => {
                    if !self.logged_rescan {
                        self.logged_rescan = true;
                        log::warn!(
                            "Hot reload: the watcher dropped events; an edit may not reload until its file is saved again"
                        );
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.disconnected {
                        self.disconnected = true;
                        log::warn!("Hot reload: the watcher stopped; hot reload is off");
                    }
                    break;
                }
            }
        }

        // A stopped watcher sends nothing more and the app drops it, so the
        // paths still inside their debounce go now.
        let flush = self.disconnected;
        let mut ready = Vec::new();
        self.pending.retain(|path, last_seen| {
            if flush || now.duration_since(*last_seen) >= window {
                ready.push(path.clone());
                false
            } else {
                true
            }
        });
        ready
    }
}

/// Background file watcher; `mpsc`, no async, no shared mutable state.
pub struct HotReloadWatcher {
    /// Keep OS watcher alive.
    _watcher: RecommendedWatcher,
    inbox: Inbox,
}

impl HotReloadWatcher {
    /// Start recursive dir watches plus parent watches for explicit files.
    #[must_use]
    pub fn new(watch_dirs: &[PathBuf], extra_files: &[PathBuf]) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<WatchMessage>();

        let mut watcher = match notify::recommended_watcher(move |res: notify::Result<Event>| {
            classify(res, |message| {
                let _ = tx.send(message);
            });
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
            // A recursive root already delivers this file. A second watch on
            // its parent would register that directory again, non-recursive.
            if under_recursive_root(&canonical_or_clone(file), &recursive_roots) {
                continue;
            }
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
            inbox: Inbox::new(rx, recursive_roots, allowed_extra_files),
        })
    }

    /// Drain debounced, accepted paths; call once per frame before render.
    /// A watcher error and a dropped-events notice are logged once each.
    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        self.inbox.drain(Instant::now())
    }

    /// The watcher thread stopped, logged once: the app drops the watcher.
    pub(crate) fn disconnected(&self) -> bool {
        self.inbox.disconnected
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
    under_recursive_root(&canon, recursive_roots)
}

/// True when the canonical path `canon` lies under one of the recursive roots.
fn under_recursive_root(canon: &Path, recursive_roots: &[PathBuf]) -> bool {
    recursive_roots.iter().any(|root| canon.starts_with(root))
}

#[cfg(test)]
#[path = "tests/hot_reload.rs"]
mod tests;
