//! Behaviour of the per-document watch registry.
//!
//! The load-bearing test here is [`repeated_atomic_saves_keep_notifying`]. A
//! watch placed on the file itself passes a naive "does it notify?" test once
//! and then goes permanently silent, because an atomic-rename save unlinks the
//! inode the watch is bound to. Only the *repeated* save distinguishes a
//! correct implementation from that one.

use openspec_core::{DocumentChange, DocumentKey, DocumentWatcher};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::broadcast;

/// Generous — filesystem events on macOS / FSEvents have non-trivial latency.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);
/// Short debounce so tests don't spend their time waiting.
const TEST_DEBOUNCE: Duration = Duration::from_millis(50);
/// Stands in for one frontend — a window label in the desktop shell, a
/// per-page client id in the browser.
const OWNER: &str = "window-1";

struct Fixture {
    _tmp: TempDir,
    root: PathBuf,
}

impl Fixture {
    /// Creates `docs/` with `docs/a.md` and `docs/b.md`, and a root-level
    /// `README.md` — one document, one sibling, one file in another directory.
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        // Canonicalised up front: `notify` reports canonical paths, and the
        // key's root is echoed back verbatim in every notification.
        let root = tmp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/a.md"), "# a\n").unwrap();
        std::fs::write(root.join("docs/b.md"), "# b\n").unwrap();
        std::fs::write(root.join("README.md"), "# readme\n").unwrap();
        Self { _tmp: tmp, root }
    }

    fn key(&self, rel: &str) -> DocumentKey {
        DocumentKey::new(self.root.clone(), rel)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

/// Replace `path`'s contents the way an editor does: write a sibling
/// temporary file, then rename it over the target. This unlinks the target's
/// inode, which is precisely what a file-level watch cannot survive.
fn atomic_write(path: &Path, contents: &str) {
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, contents).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

async fn wait_for_change(rx: &mut broadcast::Receiver<DocumentChange>) -> DocumentChange {
    let result = tokio::time::timeout(EVENT_TIMEOUT, async {
        loop {
            match rx.recv().await {
                Ok(change) => return change,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
            }
        }
    })
    .await;
    result.expect("timed out waiting for a DocumentChange")
}

/// Wait for a change naming `rel`, ignoring any other document's.
async fn wait_for_doc(rx: &mut broadcast::Receiver<DocumentChange>, rel: &str) -> DocumentChange {
    let result = tokio::time::timeout(EVENT_TIMEOUT, async {
        loop {
            let change = match rx.recv().await {
                Ok(change) => change,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
            };
            if change.rel_path == rel {
                return change;
            }
        }
    })
    .await;
    result.unwrap_or_else(|_| panic!("timed out waiting for a DocumentChange naming {rel}"))
}

// -------------------------------------------------------------------------
// Delivery
// -------------------------------------------------------------------------

#[tokio::test]
async fn in_place_modification_notifies() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();

    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.root, fx.root, "the key's root is echoed verbatim");
    assert_eq!(change.rel_path, "docs/a.md");
}

/// The regression a file-level watch would pass once and then fail forever.
/// Three consecutive atomic-rename saves must each notify.
#[tokio::test]
async fn repeated_atomic_saves_keep_notifying() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    for round in 0..3 {
        atomic_write(&fx.path("docs/a.md"), &format!("# a round {round}\n"));
        let change = wait_for_doc(&mut rx, "docs/a.md").await;
        assert_eq!(
            change.rel_path,
            "docs/a.md",
            "save number {} produced no notification — a watch bound to the \
             file's inode goes silent after the first atomic rename",
            round + 1
        );
    }
}

/// Non-vacuous absence check: the sibling is written *first*, so if its events
/// were delivered they would arrive before the target's. Asserting on the
/// first event received proves the filter works without asserting on a
/// timeout.
#[tokio::test]
async fn sibling_file_does_not_notify() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    std::fs::write(fx.path("docs/b.md"), "# b changed\n").unwrap();
    tokio::time::sleep(TEST_DEBOUNCE * 4).await;
    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();

    let change = wait_for_change(&mut rx).await;
    assert_eq!(
        change.rel_path, "docs/a.md",
        "the first notification must be the watched document, not its sibling"
    );
}

/// A document in a different directory from the registered one is not
/// delivered, which is what makes the watch non-recursive rather than merely
/// narrow.
#[tokio::test]
async fn file_outside_the_watched_directory_does_not_notify() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    std::fs::write(fx.path("README.md"), "# readme changed\n").unwrap();
    tokio::time::sleep(TEST_DEBOUNCE * 4).await;
    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();

    let change = wait_for_change(&mut rx).await;
    assert_eq!(change.rel_path, "docs/a.md");
}

#[tokio::test]
async fn two_documents_in_one_directory_notify_independently() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("docs/b.md")).unwrap();
    assert_eq!(watcher.registration_count(), 2);
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "two documents in one directory share a single watch"
    );

    std::fs::write(fx.path("docs/b.md"), "# b changed\n").unwrap();
    let change = wait_for_doc(&mut rx, "docs/b.md").await;
    assert_eq!(change.rel_path, "docs/b.md");
}

/// One batch naming a document several times yields exactly ONE notification.
///
/// The batch is handed in directly rather than produced by writing the file
/// three times, deliberately. Whether three quick writes land in one debounce
/// window is a property of the debouncer and of how loaded the machine is —
/// true on a laptop, false on a busy CI runner, and asserting it made this
/// test fail in CI while passing locally. What this module actually owns is
/// the per-batch deduplication, and that is what is asserted here, with no
/// timing in it at all: `deliver_batch_for_tests` is synchronous, so once it
/// returns every notification it will ever send has been sent.
#[tokio::test]
async fn one_batch_yields_one_notification_per_document() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    let a = fx.path("docs/a.md");
    watcher.deliver_batch_for_tests(&[a.clone(), a.clone(), a]);

    let first = rx
        .try_recv()
        .expect("the batch must produce a notification");
    assert_eq!(first.rel_path, "docs/a.md");
    assert!(
        rx.try_recv().is_err(),
        "three events for one document in one batch produced more than one notification"
    );
}

/// Two documents in one batch each get their own notification — the
/// deduplication is per document, not per batch.
#[tokio::test]
async fn one_batch_notifies_each_document_it_names() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("docs/b.md")).unwrap();

    watcher.deliver_batch_for_tests(&[fx.path("docs/a.md"), fx.path("docs/b.md")]);

    let mut seen = vec![
        rx.try_recv().unwrap().rel_path,
        rx.try_recv().unwrap().rel_path,
    ];
    seen.sort();
    assert_eq!(seen, vec!["docs/a.md".to_string(), "docs/b.md".to_string()]);
    assert!(rx.try_recv().is_err(), "no third notification is owed");
}

/// A batch naming only unwatched files notifies nothing — the same filter the
/// live-watch test exercises, asserted here without a clock.
#[tokio::test]
async fn a_batch_naming_nothing_watched_is_silent() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    watcher.deliver_batch_for_tests(&[fx.path("docs/b.md"), fx.path("README.md")]);

    assert!(
        rx.try_recv().is_err(),
        "a batch naming no watched document must be silent"
    );
}

// -------------------------------------------------------------------------
// Reference counting
// -------------------------------------------------------------------------

#[tokio::test]
async fn two_registrations_share_one_watch_and_survive_one_release() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    assert_eq!(
        watcher.registration_count(),
        1,
        "one document, however many surfaces hold it"
    );
    assert_eq!(watcher.watched_dir_count(), 1);

    watcher.release(OWNER, &fx.key("docs/a.md"));
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "the watch must survive while a second surface still holds it"
    );

    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();
    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
}

#[tokio::test]
async fn the_last_release_tears_the_watch_down() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    watcher.release(OWNER, &fx.key("docs/a.md"));
    watcher.release(OWNER, &fx.key("docs/a.md"));

    assert_eq!(watcher.registration_count(), 0);
    assert_eq!(
        watcher.watched_dir_count(),
        0,
        "nothing open means no filesystem watch"
    );
}

#[tokio::test]
async fn releasing_an_unheld_key_is_a_no_op() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    watcher.release(OWNER, &fx.key("docs/b.md"));
    watcher.release(OWNER, &fx.key("docs/b.md"));

    assert_eq!(
        watcher.registration_count(),
        1,
        "releasing a key nobody registered must not disturb another document"
    );
    assert_eq!(watcher.watched_dir_count(), 1);
}

#[tokio::test]
async fn a_fresh_watcher_holds_no_watch() {
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    assert_eq!(watcher.registration_count(), 0);
    assert_eq!(watcher.watched_dir_count(), 0);
}

/// The bound this module promises: watches scale with open documents, not
/// with the size of the tree they live in.
#[tokio::test]
async fn watch_count_tracks_documents_not_tree_size() {
    let fx = Fixture::new();
    for i in 0..50 {
        std::fs::write(fx.path(&format!("docs/filler{i}.md")), "# filler\n").unwrap();
    }
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "one open document in a directory of many files costs one watch"
    );
}

// -------------------------------------------------------------------------
// Directory replacement
// -------------------------------------------------------------------------

/// Removing and recreating the containing directory — a change being archived,
/// or a checkout swapping a subtree — must not leave the document permanently
/// deaf. Driven through `reconcile_now` so the assertion has no timing
/// component: whether the platform delivers an event for the removal of a
/// watched directory is a property of the watch backend, not of this module.
#[tokio::test]
async fn a_replaced_directory_re_arms_the_watch() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    let key = fx.key("docs/a.md");
    watcher.acquire(OWNER, key.clone()).unwrap();
    assert!(
        watcher.is_promoted(&key),
        "an existing directory is watched directly"
    );

    std::fs::remove_dir_all(fx.path("docs")).unwrap();
    watcher.reconcile_now();
    assert!(
        !watcher.is_promoted(&key),
        "with its directory gone the watch falls back to a surviving ancestor"
    );
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "falling back is still exactly one watch"
    );

    std::fs::create_dir(fx.path("docs")).unwrap();
    std::fs::write(fx.path("docs/a.md"), "# a restored\n").unwrap();
    watcher.reconcile_now();
    assert!(
        watcher.is_promoted(&key),
        "the watch returns to the document's own directory once it exists again"
    );

    // A restored subtree can arrive with the file already inside it, so no
    // per-file event is ever delivered — the promotion itself must notify, or
    // the surface would keep showing the vanished state forever.
    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
}

/// Registering a document whose directory does not exist yet must succeed and
/// start notifying once it appears, rather than failing at registration.
#[tokio::test]
async fn a_document_in_a_missing_directory_registers_and_later_notifies() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    let key = fx.key("later/deep/c.md");
    watcher.acquire(OWNER, key.clone()).unwrap();
    assert!(!watcher.is_promoted(&key));

    std::fs::create_dir_all(fx.path("later/deep")).unwrap();
    std::fs::write(fx.path("later/deep/c.md"), "# c\n").unwrap();
    watcher.reconcile_now();

    assert!(watcher.is_promoted(&key));
    let change = wait_for_doc(&mut rx, "later/deep/c.md").await;
    assert_eq!(change.rel_path, "later/deep/c.md");
}

#[tokio::test]
async fn a_path_with_no_parent_is_refused() {
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let result = watcher.acquire(OWNER, DocumentKey::new(PathBuf::from("/"), ""));
    assert!(
        result.is_err(),
        "a relative path naming no file cannot be watched"
    );
    assert_eq!(watcher.watched_dir_count(), 0);
}

// -------------------------------------------------------------------------
// Owner lifecycle
// -------------------------------------------------------------------------

/// Two frontends on one document — a reader window and the main window, or two
/// browser tabs. One going away must not blind the other.
#[tokio::test]
async fn two_owners_share_a_document_and_one_release_keeps_the_watch() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire("window-1", fx.key("docs/a.md")).unwrap();
    watcher.acquire("window-2", fx.key("docs/a.md")).unwrap();
    assert_eq!(watcher.owner_count(), 2);
    assert_eq!(watcher.registration_count(), 1);

    watcher.release("window-1", &fx.key("docs/a.md"));
    assert_eq!(watcher.owner_count(), 1);
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "the second owner still holds the document"
    );

    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();
    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
}

/// The leak this mechanism exists to prevent: a frontend that never releases
/// anything, because its tab was closed or killed.
#[tokio::test]
async fn releasing_an_owner_drops_everything_it_held() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire("tab-1", fx.key("docs/a.md")).unwrap();
    watcher.acquire("tab-1", fx.key("docs/b.md")).unwrap();
    watcher.acquire("tab-2", fx.key("docs/a.md")).unwrap();
    assert_eq!(watcher.registration_count(), 2);

    watcher.release_owner("tab-1");

    assert_eq!(watcher.owner_count(), 1);
    assert_eq!(
        watcher.registration_count(),
        1,
        "only the document the surviving owner still holds remains watched"
    );
    assert_eq!(watcher.watched_dir_count(), 1);

    watcher.release_owner("tab-2");
    assert_eq!(watcher.registration_count(), 0);
    assert_eq!(
        watcher.watched_dir_count(),
        0,
        "with every owner gone, no filesystem watch is retained"
    );
}

#[tokio::test]
async fn releasing_an_owner_that_holds_nothing_is_a_no_op() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire("tab-1", fx.key("docs/a.md")).unwrap();

    watcher.release_owner("tab-2");

    assert_eq!(watcher.registration_count(), 1);
    assert_eq!(watcher.watched_dir_count(), 1);
}

/// One owner holding the same document from two surfaces — the detail pane and
/// the file browser's preview in a single page — takes two registrations, so
/// one surface unmounting must not blind the other.
#[tokio::test]
async fn one_owner_holding_a_document_twice_needs_two_releases() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    watcher.release(OWNER, &fx.key("docs/a.md"));
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "the owner's second surface still holds the document"
    );

    watcher.release(OWNER, &fx.key("docs/a.md"));
    assert_eq!(watcher.watched_dir_count(), 0);
    assert_eq!(watcher.owner_count(), 0);
}

/// Releasing a document a *different* owner holds must not touch it.
#[tokio::test]
async fn releasing_another_owners_document_is_a_no_op() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire("tab-1", fx.key("docs/a.md")).unwrap();

    watcher.release("tab-2", &fx.key("docs/a.md"));

    assert_eq!(
        watcher.registration_count(),
        1,
        "one owner cannot release another's registration"
    );
    assert_eq!(watcher.watched_dir_count(), 1);
}

// -------------------------------------------------------------------------
// Reconciliation invariants
// -------------------------------------------------------------------------

/// The event-driven promotion path, as opposed to the `reconcile_now` one the
/// replaced-directory test drives. When the directory reappears, the batch that
/// reveals it must both promote the watch AND report the document — a restored
/// subtree can arrive with the file already in it, so the promotion is the only
/// signal there will ever be.
#[tokio::test]
async fn a_restored_directory_notifies_without_an_explicit_reconcile() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    let key = fx.key("docs/a.md");
    watcher.acquire(OWNER, key.clone()).unwrap();

    std::fs::remove_dir_all(fx.path("docs")).unwrap();
    // The removal is itself a batch, delivered by the watch on `docs`, which
    // demotes to the surviving ancestor with no explicit reconcile call.
    let deadline = tokio::time::Instant::now() + EVENT_TIMEOUT;
    while watcher.is_promoted(&key) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(TEST_DEBOUNCE).await;
    }
    assert!(
        !watcher.is_promoted(&key),
        "the batch reporting the directory's removal must demote the watch"
    );

    // Restore the whole subtree at once — file included — so no per-file event
    // is ever delivered for it.
    std::fs::create_dir(fx.path("docs")).unwrap();
    std::fs::write(fx.path("docs/a.md"), "# a restored\n").unwrap();

    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
    assert!(watcher.is_promoted(&key));
}

/// Releasing one document must tear down only ITS directory's watch. With the
/// stale-set filter inverted, the released directory would stay watched and the
/// surviving one would be churned, so the count is what says the right one went.
#[tokio::test]
async fn releasing_one_of_two_directories_leaves_the_other_watched() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("README.md")).unwrap();
    assert_eq!(
        watcher.watched_dir_count(),
        2,
        "two documents in different directories need two watches"
    );

    watcher.release(OWNER, &fx.key("README.md"));

    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "exactly the released document's directory is unwatched"
    );
    assert_eq!(watcher.registration_count(), 1);

    // And the survivor still works — the released one's teardown must not have
    // disturbed it.
    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();
    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
}

/// A batch that names no watched document must produce no notification at all.
///
/// This is what pins reconciliation to being idempotent: if every batch
/// re-armed the watch it already holds, each re-arm would look like a promotion
/// and report its documents, so an unrelated edit in the same directory would
/// wake every reader watching a file beside it.
#[tokio::test]
async fn an_unrelated_batch_notifies_nothing() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    // A sibling in the SAME directory: the watch sees this batch, and must
    // decide it names nothing it is registered for.
    std::fs::write(fx.path("docs/b.md"), "# b changed\n").unwrap();

    let stray = tokio::time::timeout(TEST_DEBOUNCE * 8, rx.recv()).await;
    assert!(
        stray.is_err(),
        "a batch naming only an unwatched sibling produced {stray:?}"
    );

    // Non-vacuous: the watch is demonstrably still live.
    std::fs::write(fx.path("docs/a.md"), "# a changed\n").unwrap();
    let change = wait_for_doc(&mut rx, "docs/a.md").await;
    assert_eq!(change.rel_path, "docs/a.md");
}

/// Dropping the watcher ends its processing task by dropping every sender —
/// which is why no explicit abort is kept. A subscriber sees the channel close.
#[tokio::test]
async fn dropping_the_watcher_closes_the_stream() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    drop(watcher);

    let closed = tokio::time::timeout(EVENT_TIMEOUT, rx.recv()).await;
    assert!(
        matches!(closed, Ok(Err(broadcast::error::RecvError::Closed))),
        "dropping the watcher must close its stream, got {closed:?}"
    );
}

/// Two documents whose directories have both vanished demote to the SAME
/// surviving ancestor and are one OS watch between them. Releasing one must not
/// unwatch the path the other still depends on — it would be blinded
/// permanently, because it would never learn its directory came back.
///
/// This is what archiving a change does to all of its artifacts at once.
#[tokio::test]
async fn releasing_one_document_sharing_a_fallback_watch_does_not_blind_the_other() {
    let fx = Fixture::new();
    std::fs::create_dir_all(fx.path("nest/one")).unwrap();
    std::fs::create_dir_all(fx.path("nest/two")).unwrap();
    std::fs::write(fx.path("nest/one/x.md"), "# x\n").unwrap();
    std::fs::write(fx.path("nest/two/y.md"), "# y\n").unwrap();

    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let mut rx = watcher.subscribe();
    let x = fx.key("nest/one/x.md");
    let y = fx.key("nest/two/y.md");
    watcher.acquire(OWNER, x.clone()).unwrap();
    watcher.acquire(OWNER, y.clone()).unwrap();
    assert_eq!(watcher.watched_dir_count(), 2);

    // Both directories go at once, so both documents fall back to `nest`.
    std::fs::remove_dir_all(fx.path("nest/one")).unwrap();
    std::fs::remove_dir_all(fx.path("nest/two")).unwrap();
    watcher.reconcile_now();
    assert!(!watcher.is_promoted(&x));
    assert!(!watcher.is_promoted(&y));
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "two demoted documents share one watch on their common ancestor"
    );

    // Close one reader. The shared watch must survive for the other.
    watcher.release(OWNER, &x);
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "the surviving document still needs the shared ancestor watched"
    );

    // And the survivor must still come back when its directory does.
    std::fs::create_dir_all(fx.path("nest/two")).unwrap();
    std::fs::write(fx.path("nest/two/y.md"), "# y restored\n").unwrap();
    let change = wait_for_doc(&mut rx, "nest/two/y.md").await;
    assert_eq!(change.rel_path, "nest/two/y.md");
    assert!(watcher.is_promoted(&y));
}

/// A registration that could not be armed must leave nothing behind: a phantom
/// entry would keep `regs` non-empty for a document nobody is displaying, so
/// the "nothing open means no watcher" fast path would never fire again.
#[tokio::test]
async fn a_failed_registration_leaves_no_phantom() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);

    // A path whose parent cannot be a directory, because it IS a file: the
    // watch attempt fails at the OS boundary rather than at the guard.
    let bad = DocumentKey::new(fx.root.clone(), "README.md/nested.md");
    let result = watcher.acquire(OWNER, bad.clone());

    if result.is_err() {
        assert_eq!(
            watcher.registration_count(),
            0,
            "a refused registration must not stay in the registry"
        );
        assert_eq!(watcher.owner_count(), 0, "nor leave its owner behind");
        assert_eq!(watcher.watched_dir_count(), 0);
    } else {
        // Some platforms happily watch the path; then the invariant is simply
        // that the registration is present and consistent.
        assert_eq!(watcher.registration_count(), 1);
        watcher.release(OWNER, &bad);
        assert_eq!(watcher.registration_count(), 0);
    }
}

// -------------------------------------------------------------------------
// Arming accounting
// -------------------------------------------------------------------------

/// Two targets that demote to one ancestor arm that path ONCE and disarm it
/// once, so the count reflects OS watches rather than interested parties. Both
/// halves matter: arming twice would leak a descriptor, and disarming on the
/// first release would blind the second document.
#[tokio::test]
async fn a_shared_fallback_path_is_armed_once_and_released_once() {
    let fx = Fixture::new();
    std::fs::create_dir_all(fx.path("nest/one")).unwrap();
    std::fs::create_dir_all(fx.path("nest/two")).unwrap();
    std::fs::write(fx.path("nest/one/x.md"), "# x\n").unwrap();
    std::fs::write(fx.path("nest/two/y.md"), "# y\n").unwrap();

    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    let x = fx.key("nest/one/x.md");
    let y = fx.key("nest/two/y.md");
    watcher.acquire(OWNER, x.clone()).unwrap();
    watcher.acquire(OWNER, y.clone()).unwrap();
    assert_eq!(watcher.watched_dir_count(), 2, "two live directories");

    std::fs::remove_dir_all(fx.path("nest/one")).unwrap();
    std::fs::remove_dir_all(fx.path("nest/two")).unwrap();
    watcher.reconcile_now();
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "both demote to one ancestor, armed once between them"
    );

    watcher.release(OWNER, &x);
    assert_eq!(
        watcher.watched_dir_count(),
        1,
        "the survivor still holds the shared watch"
    );

    watcher.release(OWNER, &y);
    assert_eq!(
        watcher.watched_dir_count(),
        0,
        "the last release drops the shared watch"
    );
}

/// Releasing by relative path is the fallback for a root that can no longer be
/// resolved — its canonical form is unreconstructible once the directory is
/// gone, so the exact key cannot be rebuilt.
#[tokio::test]
async fn a_document_can_be_released_by_its_relative_path() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();
    watcher.acquire(OWNER, fx.key("README.md")).unwrap();
    assert_eq!(watcher.registration_count(), 2);

    watcher.release_by_rel_path(OWNER, "docs/a.md");

    assert_eq!(
        watcher.registration_count(),
        1,
        "exactly the named document is released"
    );
    assert_eq!(watcher.watched_dir_count(), 1);
    // And it released the RIGHT one.
    watcher.release_by_rel_path(OWNER, "README.md");
    assert_eq!(watcher.registration_count(), 0);
    assert_eq!(watcher.watched_dir_count(), 0);
}

#[tokio::test]
async fn releasing_an_unknown_relative_path_changes_nothing() {
    let fx = Fixture::new();
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    watcher.acquire(OWNER, fx.key("docs/a.md")).unwrap();

    watcher.release_by_rel_path(OWNER, "docs/nope.md");
    watcher.release_by_rel_path("other-owner", "docs/a.md");

    assert_eq!(
        watcher.registration_count(),
        1,
        "neither an unknown path nor another owner may release this document"
    );
    assert_eq!(watcher.watched_dir_count(), 1);
}

/// The WSL polling cadence is stored on every platform so one setting cannot
/// mean two different things between the tree and an open document.
#[tokio::test]
async fn the_poll_interval_is_settable_and_readable() {
    let watcher = DocumentWatcher::new(TEST_DEBOUNCE);
    assert_eq!(watcher.poll_interval(), Duration::from_secs(10));

    watcher.set_poll_interval(Duration::from_secs(3));

    assert_eq!(watcher.poll_interval(), Duration::from_secs(3));
}
