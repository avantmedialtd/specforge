use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Tracks paths the application has recently written so that the watcher
/// can ignore the corresponding filesystem events.
///
/// This is infrastructure for future interactive features (e.g. toggling a
/// task checkbox writes to `tasks.md`); v1 is read-only and never records
/// anything, so the tracker is effectively a no-op in production. Worth
/// landing now so the watcher pipeline already has the hook.
#[derive(Debug)]
pub struct SelfWriteTracker {
    inner: Mutex<Vec<(PathBuf, Instant)>>,
    ttl: Duration,
}

impl SelfWriteTracker {
    /// Creates a tracker that forgets a recorded path after `ttl` elapses.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            ttl,
        }
    }

    /// Record that the application just wrote to `path`. Subsequent
    /// watcher events on the same path within `ttl` should be ignored.
    pub fn record(&self, path: impl Into<PathBuf>) {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("self-write mutex poisoned");
        Self::gc(&mut inner, self.ttl, now);
        inner.push((path.into(), now));
    }

    /// Returns true if `path` was recorded as a self-write within the
    /// TTL window.
    pub fn was_self_written(&self, path: &Path) -> bool {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("self-write mutex poisoned");
        Self::gc(&mut inner, self.ttl, now);
        inner.iter().any(|(p, _)| p.as_path() == path)
    }

    /// Number of currently-tracked paths (after GC).
    pub fn len(&self) -> usize {
        let now = Instant::now();
        let mut inner = self.inner.lock().expect("self-write mutex poisoned");
        Self::gc(&mut inner, self.ttl, now);
        inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn gc(entries: &mut Vec<(PathBuf, Instant)>, ttl: Duration, now: Instant) {
        entries.retain(|(_, t)| now.duration_since(*t) < ttl);
    }
}
