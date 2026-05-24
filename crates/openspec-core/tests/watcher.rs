use openspec_core::{CacheEvent, WatcherManager, WorkspaceFolder};
use std::path::PathBuf;
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
        Self { _tmp: tmp, workspace }
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
    let event = wait_for(&mut rx, |e| {
        matches!(e, CacheEvent::Updated { workspace } if workspace == &workspace_uri)
    })
    .await;
    assert!(matches!(event, CacheEvent::Updated { .. }));

    // The cached title should reflect the edit.
    let changes = manager.changes_for(&fx.workspace.uri);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].title.as_deref(), Some("updated title"));
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
