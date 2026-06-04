use openspec_core::{
    repo_view::WorkspaceView, CacheEvent, WatcherManager, WorkspaceFolder, WorkspaceRegistry,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::broadcast;

/// Generous timeout — filesystem events on macOS / FSEvents have non-trivial
/// latency, especially under load. Tests that expect an event use this.
const EVENT_TIMEOUT: Duration = Duration::from_secs(5);

/// Short debounce so tests don't spend most of their time waiting.
const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

struct Fixture {
    _tmp: TempDir,
    workspace: WorkspaceFolder,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_changes(&[]).await
    }

    async fn with_changes(names: &[&str]) -> Self {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        tokio::fs::create_dir_all(root.join("openspec/changes"))
            .await
            .unwrap();
        for name in names {
            let dir = root.join("openspec/changes").join(name);
            tokio::fs::create_dir(&dir).await.unwrap();
            tokio::fs::write(dir.join("proposal.md"), format!("# {}\n", name))
                .await
                .unwrap();
        }
        let canonical = root.canonicalize().unwrap();
        let workspace = WorkspaceFolder::from_path(canonical);
        Self {
            _tmp: tmp,
            workspace,
        }
    }

    fn changes_dir(&self) -> PathBuf {
        self.workspace.uri.join("openspec/changes")
    }
}

async fn wait_for<F>(rx: &mut broadcast::Receiver<CacheEvent>, pred: F) -> CacheEvent
where
    F: Fn(&CacheEvent) -> bool,
{
    let result = tokio::time::timeout(EVENT_TIMEOUT, async {
        loop {
            match rx.recv().await {
                Ok(event) if pred(&event) => return event,
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
            }
        }
    })
    .await;
    result.expect("timed out waiting for expected CacheEvent")
}

// -------------------------------------------------------------------------
// add_workspace / remove_workspace basics
// -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn add_workspace_populates_cache() {
    let fx = Fixture::with_changes(&["alpha", "beta"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    let changes = manager.changes_for(&fx.workspace.uri);
    let ids: Vec<_> = changes.iter().map(|c| c.change_id.as_str()).collect();
    assert_eq!(ids, vec!["alpha", "beta"]);
    assert_eq!(manager.total_active_count(), 2);
    assert!(manager.is_watching(&fx.workspace.uri));
    assert_eq!(manager.watched_count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_workspace_clears_cache_and_watcher() {
    let fx = Fixture::with_changes(&["alpha"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    let removed = manager.remove_workspace(&fx.workspace.uri);

    assert!(removed);
    assert_eq!(manager.total_active_count(), 0);
    assert!(!manager.is_watching(&fx.workspace.uri));
    assert!(manager.changes_for(&fx.workspace.uri).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn add_workspace_is_idempotent() {
    let fx = Fixture::with_changes(&["alpha"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);

    manager.add_workspace(fx.workspace.clone()).await.unwrap();
    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    assert_eq!(manager.watched_count(), 1);
    assert_eq!(manager.total_active_count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn add_workspace_with_missing_directory_errors() {
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let bogus = WorkspaceFolder::from_path(PathBuf::from("/this/does/not/exist"));
    let err = manager.add_workspace(bogus).await.expect_err("should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("not a directory") || msg.contains("does not"),
        "unexpected error message: {msg}"
    );
}

// -------------------------------------------------------------------------
// reactive events
// -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn new_change_directory_triggers_change_added_event() {
    let fx = Fixture::new().await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    // Create a new change directory after registration.
    let new_change = fx.changes_dir().join("new-change");
    tokio::fs::create_dir(&new_change).await.unwrap();
    tokio::fs::write(new_change.join("proposal.md"), "# new\n")
        .await
        .unwrap();

    let workspace_uri = fx.workspace.uri.clone();
    let event = wait_for(&mut rx, |e| {
        matches!(
            e,
            CacheEvent::ChangeAdded { workspace, change_id }
                if workspace == &workspace_uri && change_id == "new-change"
        )
    })
    .await;

    assert!(matches!(event, CacheEvent::ChangeAdded { .. }));
    let cached_ids: Vec<_> = manager
        .changes_for(&fx.workspace.uri)
        .into_iter()
        .map(|c| c.change_id)
        .collect();
    assert!(cached_ids.contains(&"new-change".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_change_to_archive_triggers_change_archived_event() {
    let fx = Fixture::with_changes(&["going-away"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    // Move the change directory into the archive subdir.
    let archive_dir = fx.changes_dir().join("archive");
    tokio::fs::create_dir_all(&archive_dir).await.unwrap();
    tokio::fs::rename(
        fx.changes_dir().join("going-away"),
        archive_dir.join("going-away"),
    )
    .await
    .unwrap();

    let workspace_uri = fx.workspace.uri.clone();
    wait_for(&mut rx, |e| {
        matches!(
            e,
            CacheEvent::ChangeArchived { workspace, change_id }
                if workspace == &workspace_uri && change_id == "going-away"
        )
    })
    .await;

    // The archived change must be gone from the cache.
    assert!(manager.changes_for(&fx.workspace.uri).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_change_to_dated_archive_triggers_change_archived_event() {
    let fx = Fixture::with_changes(&["going-away"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    // Move the change into the archive under the date-prefixed name the
    // archive tooling actually produces: `archive/<YYYY-MM-DD>-<id>/`.
    let archive_dir = fx.changes_dir().join("archive");
    tokio::fs::create_dir_all(&archive_dir).await.unwrap();
    tokio::fs::rename(
        fx.changes_dir().join("going-away"),
        archive_dir.join("2026-06-04-going-away"),
    )
    .await
    .unwrap();

    // ChangeArchived must fire for the bare logical id, not the dated name.
    let workspace_uri = fx.workspace.uri.clone();
    wait_for(&mut rx, |e| {
        matches!(
            e,
            CacheEvent::ChangeArchived { workspace, change_id }
                if workspace == &workspace_uri && change_id == "going-away"
        )
    })
    .await;

    assert!(manager.changes_for(&fx.workspace.uri).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_change_without_archiving_is_not_reported_as_archived() {
    let fx = Fixture::with_changes(&["going-away"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    // Delete the active change outright — no archive directory at all.
    tokio::fs::remove_dir_all(fx.changes_dir().join("going-away"))
        .await
        .unwrap();

    // The removal must surface as a plain Updated, never a ChangeArchived.
    // Within a batch ChangeArchived (if any) is emitted before Updated, so
    // draining up to the Updated without seeing one proves the negative.
    let workspace_uri = fx.workspace.uri.clone();
    loop {
        match tokio::time::timeout(EVENT_TIMEOUT, rx.recv()).await {
            Ok(Ok(CacheEvent::ChangeArchived { change_id, .. })) if change_id == "going-away" => {
                panic!("deletion was incorrectly reported as an archive")
            }
            Ok(Ok(CacheEvent::Updated { workspace })) if workspace == workspace_uri => break,
            Ok(Ok(_)) => continue,
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => panic!("channel closed"),
            Err(_) => panic!("timed out waiting for Updated after deletion"),
        }
    }

    assert!(manager.changes_for(&fx.workspace.uri).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_existing_file_triggers_updated_event_only() {
    let fx = Fixture::with_changes(&["existing"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();

    // Modify an existing proposal.md.
    let proposal = fx.changes_dir().join("existing/proposal.md");
    tokio::fs::write(&proposal, "# updated title\n")
        .await
        .unwrap();

    let workspace_uri = fx.workspace.uri.clone();
    let event = wait_for(
        &mut rx,
        |e| matches!(e, CacheEvent::Updated { workspace } if workspace == &workspace_uri),
    )
    .await;
    assert!(matches!(event, CacheEvent::Updated { .. }));

    // The cached title should reflect the edit.
    let changes = manager.changes_for(&fx.workspace.uri);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].title.as_deref(), Some("updated title"));
}

// -------------------------------------------------------------------------
// last_views ordering invariant
// -------------------------------------------------------------------------
//
// Both tests below assert the contract that any subscriber observing a
// raw cache event (`Updated`, `ChangeAdded`, `ChangeArchived`) is allowed
// to read `workspace_views()` immediately and see the post-event snapshot —
// i.e. `last_views` is refreshed before the broadcast send. Pre-refactor,
// `handle_events` updated the cache and broadcast `Updated` immediately,
// leaving `last_views` to be refreshed by a separate broadcast subscriber
// (`spawn_aggregator`) that raced the event forwarder. These two tests
// fail under that pre-refactor ordering and pass after the refresh is
// moved inline.

/// Helper: build a registered-flat-workspace fixture so `workspace_views()`
/// returns non-empty data (the aggregator skips workspaces unknown to the
/// registry). Returns the manager, the broadcast receiver, and the workspace
/// folder.
async fn registered_fixture(
    fx: &Fixture,
) -> (
    WatcherManager,
    broadcast::Receiver<CacheEvent>,
    Arc<Mutex<WorkspaceRegistry>>,
) {
    let cfg = fx._tmp.path().join("workspaces.json");
    let registry = Arc::new(Mutex::new(WorkspaceRegistry::new(cfg)));
    registry
        .lock()
        .unwrap()
        .register(fx.workspace.uri.clone())
        .unwrap();

    let manager = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    let rx = manager.subscribe();
    manager.add_workspace(fx.workspace.clone()).await.unwrap();
    // Seed `last_views` so we can compare pre- vs post-edit state below.
    manager.aggregate_and_emit();
    (manager, rx, registry)
}

#[tokio::test(flavor = "multi_thread")]
async fn editing_tasks_md_immediately_reflects_in_workspace_views() {
    let fx = Fixture::with_changes(&["existing"]).await;

    // Seed tasks.md with two tasks, one already complete.
    let tasks_path = fx.changes_dir().join("existing/tasks.md");
    tokio::fs::write(&tasks_path, "## Group\n\n- [x] 1.1 one\n- [ ] 1.2 two\n")
        .await
        .unwrap();

    let (manager, mut rx, _registry) = registered_fixture(&fx).await;

    // Sanity: initial aggregate snapshot reflects 1/2.
    {
        let views = manager.workspace_views();
        let WorkspaceView::Flat { changes, .. } = &views[0] else {
            panic!("expected Flat view");
        };
        assert_eq!(changes[0].completed_tasks, 1);
        assert_eq!(changes[0].total_tasks, 2);
    }

    // Flip the second task to complete.
    tokio::fs::write(&tasks_path, "## Group\n\n- [x] 1.1 one\n- [x] 1.2 two\n")
        .await
        .unwrap();

    let workspace_uri = fx.workspace.uri.clone();
    wait_for(
        &mut rx,
        |e| matches!(e, CacheEvent::Updated { workspace } if workspace == &workspace_uri),
    )
    .await;

    // At the moment the test sees `Updated`, `workspace_views()` MUST
    // already reflect the new completion count — without this guarantee
    // the frontend's `cache-updated` handler would read the previous
    // snapshot and the UI would lag by one event.
    let views = manager.workspace_views();
    let WorkspaceView::Flat { changes, .. } = &views[0] else {
        panic!("expected Flat view");
    };
    assert_eq!(
        changes[0].completed_tasks, 2,
        "workspace_views() did not reflect the new completed count when Updated was observed"
    );
    assert_eq!(changes[0].total_tasks, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_tasks_md_in_existing_change_immediately_flips_artifacts_tasks() {
    let fx = Fixture::with_changes(&["sparse"]).await;
    // `Fixture::with_changes` writes proposal.md by default — remove it so
    // the only file in the change directory is whatever this test creates.
    // The watcher's `add_workspace` will see a change directory with no
    // artifact files and cache it with `artifacts.tasks == false`.
    let change_dir = fx.changes_dir().join("sparse");
    tokio::fs::remove_file(change_dir.join("proposal.md"))
        .await
        .unwrap();

    let (manager, mut rx, _registry) = registered_fixture(&fx).await;

    // Initial state: the change is tracked, tasks.md is absent.
    {
        let views = manager.workspace_views();
        let WorkspaceView::Flat { changes, .. } = &views[0] else {
            panic!("expected Flat view");
        };
        assert!(
            !changes[0].artifacts.tasks,
            "tasks.md should be absent in the initial state"
        );
    }

    // Now create tasks.md inside the already-tracked change directory.
    tokio::fs::write(change_dir.join("tasks.md"), "## Group\n\n- [ ] 1.1 todo\n")
        .await
        .unwrap();

    let workspace_uri = fx.workspace.uri.clone();
    wait_for(
        &mut rx,
        |e| matches!(e, CacheEvent::Updated { workspace } if workspace == &workspace_uri),
    )
    .await;

    // The moment the test observes Updated, `workspace_views()` must show
    // `artifacts.tasks == true`. Pre-refactor, this flipped only on the
    // *next* unrelated edit because `last_views` was refreshed by a
    // broadcast subscriber that raced this read.
    let views = manager.workspace_views();
    let WorkspaceView::Flat { changes, .. } = &views[0] else {
        panic!("expected Flat view");
    };
    assert!(
        changes[0].artifacts.tasks,
        "artifacts.tasks should be true on the first Updated after tasks.md is created"
    );
    assert_eq!(changes[0].total_tasks, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_workspace_folder_does_not_crash() {
    let fx = Fixture::with_changes(&["doomed"]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    let mut rx = manager.subscribe();

    manager.add_workspace(fx.workspace.clone()).await.unwrap();
    assert_eq!(manager.total_active_count(), 1);

    // Nuke the workspace's openspec/ subtree (simulating the user
    // deleting it from outside the app).
    tokio::fs::remove_dir_all(fx.workspace.uri.join("openspec"))
        .await
        .unwrap();

    // We don't assert a specific event here — the platform's filesystem
    // events for bulk recursive deletes vary. We just verify the manager
    // is still alive and responsive.
    let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(manager.is_watching(&fx.workspace.uri));
    assert_eq!(manager.watched_count(), 1);

    // Removing the workspace explicitly must still work after the folder
    // is gone.
    assert!(manager.remove_workspace(&fx.workspace.uri));
}
