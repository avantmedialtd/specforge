//! Per-document filesystem watching.
//!
//! [`WatcherManager`](crate::watcher::WatcherManager) keeps the *change cache*
//! fresh: it watches each workspace's `openspec/` subtree and then filters the
//! events it delivers down to `openspec/changes/`. That is the right scope for
//! a cache of changes, and the wrong scope for a reader looking at one file —
//! a capability specification under `openspec/specs/` lies inside the watched
//! tree and is filtered out of delivery, and a `README.md` is not watched at
//! all.
//!
//! This module supplies the missing half: a refcounted registry of individual
//! documents, each watched precisely and only while some surface is displaying
//! it. It is deliberately independent of `WatcherManager` — it consults none of
//! that manager's roots, filters, or self-write suppression, and changes none
//! of them. A document beneath a change directory therefore lies within both
//! mechanisms and may notify twice; that is specified to be harmless, because
//! the frontends coalesce refreshes and a re-read of unchanged bytes is
//! unobservable.
//!
//! # Why the parent directory, never the file
//!
//! Editors, `git checkout`, and most atomic writers replace a file by writing
//! a temporary file and renaming it over the target. That unlinks the inode,
//! and an inotify watch follows the *inode*, not the path: a watch established
//! on the file itself delivers one event and then goes permanently silent
//! while still reporting itself as healthy. The failure looks exactly like
//! "it updated once and then stopped". Watching the containing directory
//! non-recursively and filtering by name is immune to it.
//!
//! # Why the nearest existing ancestor
//!
//! A watch cannot be placed on a directory that does not exist, and the
//! directory holding a document can disappear — a change being archived moves
//! its whole directory, and a `git checkout` can swap a subtree wholesale. So
//! when the target directory is missing this module watches the nearest
//! ancestor that does exist, and *promotes* the watch back down as soon as a
//! batch reveals the target has reappeared. That keeps re-arming push-based:
//! no timer, no polling, and no window in which a restored document silently
//! stops updating.

use crate::paths::canonicalize;
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

/// Matches [`crate::watcher`]'s debounce, so a save that produces a burst of
/// filesystem events yields one notification on either path.
const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(200);
const EVENT_CHANNEL_CAPACITY: usize = 128;

/// Identifies one watched document: the browse root a surface is reading
/// against, and the document's path relative to it (forward-slash separated,
/// as it crosses the IPC boundary).
///
/// The root is stored **exactly as the caller supplied it**, never
/// canonicalised, because it is echoed back in [`DocumentChange`] and a
/// frontend matches that value against the root it holds. Canonicalisation
/// happens only where paths are compared against the filesystem's own
/// reports, in [`Inner::reconcile`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentKey {
    pub root: PathBuf,
    pub rel_path: String,
}

impl DocumentKey {
    pub fn new(root: impl Into<PathBuf>, rel_path: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            rel_path: rel_path.into(),
        }
    }
}

/// Emitted when a watched document's file changes. Carries identifiers only —
/// never content. A surface receiving one re-reads the document through the
/// guarded read, so exactly one code path reads a file and exactly one guard
/// applies to it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChange {
    pub root: PathBuf,
    pub rel_path: String,
}

impl From<&DocumentKey> for DocumentChange {
    fn from(key: &DocumentKey) -> Self {
        Self {
            root: key.root.clone(),
            rel_path: key.rel_path.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum DocumentWatchError {
    #[error("document path has no parent directory: {0}")]
    NoParent(String),
    #[error(transparent)]
    Notify(#[from] notify::Error),
}

/// One registered document.
struct Registration {
    /// Directory that must be watched to observe this document, in the form
    /// the caller's root produced it — not canonicalised. Used as the key
    /// into [`State::dirs`].
    target_dir: PathBuf,
    file_name: OsString,
    refcount: usize,
}

/// The live watch backing one target directory.
struct DirWatch {
    /// The path actually watched: the target directory when it exists, else
    /// the nearest ancestor of it that does. Canonicalised, so the paths
    /// `notify` reports back can be compared by equality.
    armed_at: PathBuf,
    /// Where the target directory *would* be, expressed through `armed_at`'s
    /// canonical form. Equal to the canonicalised target directory whenever
    /// that directory exists.
    target_canon: PathBuf,
}

impl DirWatch {
    /// True when the watch sits on the document's own directory rather than
    /// on a surviving ancestor of it.
    fn is_promoted(&self) -> bool {
        self.armed_at == self.target_canon
    }
}

struct State {
    /// Lazily created on the first registration and dropped when the last one
    /// is released, so an application that never opens a document holds no
    /// watcher threads and no filesystem watch.
    debouncer: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    /// Every document some owner currently holds, with the total number of
    /// registrations across all owners.
    regs: HashMap<DocumentKey, Registration>,
    /// Per-owner accounting: which documents each owner holds, and how many
    /// times. An "owner" is whatever the host uses to identify one frontend
    /// that can go away as a unit — a window label in the desktop shell, a
    /// per-page client id in the browser.
    ///
    /// This second index exists so a frontend that disappears without
    /// releasing anything — a closed tab, a crashed renderer — cannot strand
    /// a watch. Its connection dropping is enough; see [`Self::release_owner`].
    owners: HashMap<String, HashMap<DocumentKey, usize>>,
    dirs: HashMap<PathBuf, DirWatch>,
}

struct Inner {
    state: Mutex<State>,
    tx: broadcast::Sender<DocumentChange>,
    /// Cloned into each debouncer so batches reach the processing task.
    events_tx: mpsc::UnboundedSender<DebounceEventResult>,
    /// Parked here from construction until the processing task starts and
    /// takes it. A `Mutex` rather than a thread-local: the watcher is
    /// routinely built on one thread and first registered from another.
    events_rx: Mutex<Option<mpsc::UnboundedReceiver<DebounceEventResult>>>,
    /// Set once, when the first registration starts the processing task.
    ///
    /// Not aborted on drop, and deliberately so: `events_tx` and the
    /// debouncer's clone of it are both owned by this `Inner`, so dropping it
    /// drops every sender, `rx.recv()` resolves to `None`, and the task ends
    /// on its own. An explicit abort would be a second mechanism for the same
    /// guarantee, and the kind that quietly stops matching the first.
    task: Mutex<Option<JoinHandle<()>>>,
    debounce: Duration,
}

/// Refcounted registry of watched documents. Cheap to clone; clones share one
/// registry through an `Arc`.
#[derive(Clone)]
pub struct DocumentWatcher {
    inner: Arc<Inner>,
}

impl Default for DocumentWatcher {
    fn default() -> Self {
        Self::new(DEFAULT_DEBOUNCE)
    }
}

impl DocumentWatcher {
    pub fn new(debounce: Duration) -> Self {
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            state: Mutex::new(State {
                debouncer: None,
                regs: HashMap::new(),
                owners: HashMap::new(),
                dirs: HashMap::new(),
            }),
            tx,
            events_tx,
            // Taken by the processing task on first registration, which is
            // also the first moment a Tokio runtime is guaranteed to exist.
            events_rx: Mutex::new(Some(events_rx)),
            task: Mutex::new(None),
            debounce,
        });
        DocumentWatcher { inner }
    }

    /// Subscribe to change notifications for every watched document.
    pub fn subscribe(&self) -> broadcast::Receiver<DocumentChange> {
        self.inner.tx.subscribe()
    }

    /// Register `owner`'s interest in `key`, establishing a watch on the first
    /// registration and joining the existing one on subsequent registrations.
    ///
    /// `owner` identifies the frontend holding this registration, so that
    /// everything it holds can be dropped at once when it goes away (see
    /// [`Self::release_owner`]). Several surfaces inside one owner may hold
    /// the same document; each takes its own registration.
    ///
    /// The caller is responsible for having authorised the root and guarded
    /// the relative path first; this module places no watch that a guarded
    /// read would refuse only because it does not itself re-run the guard.
    pub fn acquire(&self, owner: &str, key: DocumentKey) -> Result<(), DocumentWatchError> {
        let joined = key.root.join(&key.rel_path);
        let target_dir = joined
            .parent()
            .ok_or_else(|| DocumentWatchError::NoParent(key.rel_path.clone()))?
            .to_path_buf();
        let file_name = joined
            .file_name()
            .ok_or_else(|| DocumentWatchError::NoParent(key.rel_path.clone()))?
            .to_os_string();

        self.inner.start_task_if_needed();

        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state
            .regs
            .entry(key.clone())
            .and_modify(|r| r.refcount += 1)
            .or_insert(Registration {
                target_dir,
                file_name,
                refcount: 1,
            });
        *state
            .owners
            .entry(owner.to_string())
            .or_default()
            .entry(key)
            .or_insert(0) += 1;
        Inner::reconcile(&mut state, self.inner.debounce, &self.inner.events_tx)?;
        Ok(())
    }

    /// Release one of `owner`'s registrations of `key`. The watch is torn down
    /// only when the last registration across every owner goes away. Releasing
    /// a key this owner does not hold is a no-op, so a surface unmounting twice
    /// — or unmounting after a failed registration — cannot tear down a watch
    /// some other surface still depends on.
    pub fn release(&self, owner: &str, key: &DocumentKey) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if !Self::take_one(&mut state, owner, key) {
            return;
        }
        // A reconcile failure while releasing leaves a watch armed on a
        // directory nothing is registered for. That wastes a descriptor but
        // corrupts nothing, and there is no caller to report it to.
        let _ = Inner::reconcile(&mut state, self.inner.debounce, &self.inner.events_tx);
    }

    /// Release everything `owner` holds.
    ///
    /// This is the mechanism that makes the watch count honest across hosts. A
    /// frontend does not always get to clean up after itself: a browser tab can
    /// be closed or killed with reader windows open, and no unwatch call is
    /// ever made. Its transport dropping is the signal, and this drops every
    /// registration that went with it in one step.
    pub fn release_owner(&self, owner: &str) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(held) = state.owners.remove(owner) else {
            return;
        };
        for (key, count) in held {
            for _ in 0..count {
                if let Some(reg) = state.regs.get_mut(&key) {
                    reg.refcount -= 1;
                    if reg.refcount == 0 {
                        state.regs.remove(&key);
                        break;
                    }
                }
            }
        }
        let _ = Inner::reconcile(&mut state, self.inner.debounce, &self.inner.events_tx);
    }

    /// Decrement one registration of `key` held by `owner`, dropping the
    /// document entirely when no owner holds it any more. Returns whether
    /// anything was actually released.
    fn take_one(state: &mut State, owner: &str, key: &DocumentKey) -> bool {
        let Some(held) = state.owners.get_mut(owner) else {
            return false;
        };
        match held.get_mut(key) {
            Some(count) if *count > 1 => *count -= 1,
            Some(_) => {
                held.remove(key);
                if held.is_empty() {
                    state.owners.remove(owner);
                }
            }
            None => return false,
        }
        if let Some(reg) = state.regs.get_mut(key) {
            reg.refcount -= 1;
            if reg.refcount == 0 {
                state.regs.remove(key);
            }
        }
        true
    }

    /// Number of frontends currently holding at least one registration.
    pub fn owner_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .owners
            .len()
    }

    /// Re-evaluate every watch against the filesystem right now, emitting a
    /// change for each document whose watch moved onto its own directory.
    ///
    /// The same reconciliation runs after every debounced batch, which is how
    /// re-arming stays push-based in normal operation. This entry point exists
    /// for a caller that already knows the tree moved under it — and it is
    /// what lets the directory-replacement behaviour be tested with no timing
    /// component at all, rather than by waiting on an event whose delivery is
    /// a property of the platform's watch backend rather than of this module.
    pub fn reconcile_now(&self) {
        let moved = {
            let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
            Inner::reconcile(&mut state, self.inner.debounce, &self.inner.events_tx)
                .unwrap_or_default()
        };
        for key in moved {
            let _ = self.inner.tx.send(DocumentChange::from(&key));
        }
    }

    /// Number of distinct documents currently registered.
    pub fn registration_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .regs
            .len()
    }

    /// Number of filesystem watches currently established — the bound this
    /// module promises: a function of open documents, never of workspace size.
    pub fn watched_dir_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dirs
            .len()
    }

    /// True when the watch for `key`'s directory sits on that directory itself
    /// rather than on a surviving ancestor. Test-facing: it distinguishes
    /// "watching the document" from "waiting for its directory to reappear".
    pub fn is_promoted(&self, key: &DocumentKey) -> bool {
        let state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state
            .regs
            .get(key)
            .and_then(|reg| state.dirs.get(&reg.target_dir))
            .is_some_and(DirWatch::is_promoted)
    }
}

impl Inner {
    /// Start the batch-processing task on first use. Deferred out of `new` so
    /// constructing a `DocumentWatcher` outside a Tokio runtime cannot panic.
    fn start_task_if_needed(self: &Arc<Self>) {
        let mut slot = self.task.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return;
        }
        let Some(mut rx) = self
            .events_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        else {
            return;
        };
        let weak: Weak<Inner> = Arc::downgrade(self);
        *slot = Some(tokio::spawn(async move {
            while let Some(result) = rx.recv().await {
                // Upgrading per batch, and dropping the strong reference
                // before awaiting again, is what lets the watcher be dropped
                // while this task is parked — the same ownership shape
                // `WatcherManager`'s processing task uses.
                let Some(inner) = weak.upgrade() else { break };
                inner.handle_batch(result);
            }
        }));
    }

    /// Handle one debounced batch: emit for every registered document the
    /// batch names, then re-evaluate every watch, since the batch may have
    /// removed or restored a watched directory.
    fn handle_batch(&self, result: DebounceEventResult) {
        let mut changed: Vec<DocumentChange> = Vec::new();
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Ok(events) = result {
                let paths: Vec<&Path> = events
                    .iter()
                    .flat_map(|event| event.paths.iter().map(PathBuf::as_path))
                    .collect();
                for (key, reg) in state.regs.iter() {
                    let Some(dir) = state.dirs.get(&reg.target_dir) else {
                        continue;
                    };
                    let absolute = dir.target_canon.join(&reg.file_name);
                    if paths.iter().any(|p| *p == absolute) {
                        changed.push(DocumentChange::from(key));
                    }
                }
            }
            // A directory that appeared or vanished in this batch changes
            // where each watch belongs. Any document whose watch moves is
            // reported too: a restored directory can arrive with the file
            // already inside it (a `git checkout` restoring a whole subtree),
            // in which case no per-file event is ever delivered and the
            // promotion is the only signal that the document is readable
            // again.
            if let Ok(moved) = Self::reconcile(&mut state, self.debounce, &self.events_tx) {
                for key in moved {
                    let change = DocumentChange::from(&key);
                    if !changed.contains(&change) {
                        changed.push(change);
                    }
                }
            }
        }
        for change in changed {
            // No subscribers is the normal state before a frontend attaches.
            let _ = self.tx.send(change);
        }
    }

    /// Bring the set of established watches into agreement with the set of
    /// registrations, and report every document whose watch moved between its
    /// own directory and an ancestor.
    fn reconcile(
        state: &mut State,
        debounce: Duration,
        events_tx: &mpsc::UnboundedSender<DebounceEventResult>,
    ) -> Result<Vec<DocumentKey>, DocumentWatchError> {
        if state.regs.is_empty() {
            // Dropping the debouncer unwatches everything it held and stops
            // its threads, so an application with nothing open holds no
            // filesystem watch at all.
            state.dirs.clear();
            state.debouncer = None;
            return Ok(Vec::new());
        }

        let wanted: HashMap<PathBuf, (PathBuf, PathBuf)> = state
            .regs
            .values()
            .map(|reg| {
                let (armed_at, target_canon) = resolve_watch(&reg.target_dir);
                (reg.target_dir.clone(), (armed_at, target_canon))
            })
            .collect();

        if state.debouncer.is_none() {
            let tx = events_tx.clone();
            state.debouncer = Some(new_debouncer(debounce, None, move |result| {
                let _ = tx.send(result);
            })?);
        }
        let debouncer = state
            .debouncer
            .as_mut()
            .expect("debouncer was just created if absent");

        let stale: Vec<PathBuf> = state
            .dirs
            .keys()
            .filter(|target| !wanted.contains_key(*target))
            .cloned()
            .collect();
        for target in stale {
            if let Some(watch) = state.dirs.remove(&target) {
                let _ = debouncer.watcher().unwatch(&watch.armed_at);
            }
        }

        let mut moved = Vec::new();
        for (target, (armed_at, target_canon)) in wanted {
            match state.dirs.get(&target) {
                Some(existing) if existing.armed_at == armed_at => continue,
                Some(existing) => {
                    let _ = debouncer.watcher().unwatch(&existing.armed_at);
                }
                None => {}
            }
            debouncer
                .watcher()
                .watch(&armed_at, RecursiveMode::NonRecursive)?;
            let promoted = armed_at == target_canon;
            state.dirs.insert(
                target.clone(),
                DirWatch {
                    armed_at,
                    target_canon,
                },
            );
            if promoted {
                moved.extend(
                    state
                        .regs
                        .iter()
                        .filter(|(_, reg)| reg.target_dir == target)
                        .map(|(key, _)| key.clone()),
                );
            }
        }
        Ok(moved)
    }
}

/// Decide where a watch for `target_dir` belongs: on that directory when it
/// exists, otherwise on the nearest ancestor that does. Returns the path to
/// watch and the canonical form the target directory has (or would have)
/// through it, both canonicalised so they can be compared by equality against
/// the paths `notify` reports.
fn resolve_watch(target_dir: &Path) -> (PathBuf, PathBuf) {
    let existing = nearest_existing(target_dir);
    let armed = canonicalize(&existing).unwrap_or_else(|_| existing.clone());
    let tail = target_dir.strip_prefix(&existing).unwrap_or(Path::new(""));
    let target_canon = if tail.as_os_str().is_empty() {
        armed.clone()
    } else {
        armed.join(tail)
    };
    (armed, target_canon)
}

/// The deepest ancestor of `dir` (including `dir`) that is a directory on
/// disk. Falls back to `dir` itself when nothing in the chain exists, so the
/// caller always gets a path to attempt — the attempt then fails loudly rather
/// than this function guessing.
///
/// Driven by `PathBuf::pop`, which strictly shortens the path and reports when
/// there is nothing left to remove, so termination is a property of the loop
/// rather than of a guard against a path that could be its own parent.
fn nearest_existing(dir: &Path) -> PathBuf {
    let mut candidate = dir.to_path_buf();
    while !candidate.is_dir() {
        if !candidate.pop() {
            return dir.to_path_buf();
        }
    }
    candidate
}
