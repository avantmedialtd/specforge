//! `WatcherManager::any_change_touches_specs` — the predicate that drives
//! the tray-glyph variant. The predicate is true iff any cached change in
//! any registered workspace has a non-empty `ArtifactStatus.specs`.

use openspec_core::{WatcherManager, WorkspaceFolder};
use std::time::Duration;
use tempfile::TempDir;

const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

/// Builds a workspace tempdir with the given changes. Each change is
/// `(change_id, capability_spec_names)`. An empty `capability_spec_names`
/// slice means the change has no spec delta.
async fn workspace_with(changes: &[(&str, &[&str])]) -> (TempDir, WorkspaceFolder) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    tokio::fs::create_dir_all(root.join("openspec/changes"))
        .await
        .unwrap();
    for (change_id, specs) in changes {
        let change_dir = root.join("openspec/changes").join(change_id);
        tokio::fs::create_dir_all(&change_dir).await.unwrap();
        tokio::fs::write(change_dir.join("proposal.md"), format!("# {}\n", change_id))
            .await
            .unwrap();
        for cap in *specs {
            let spec_dir = change_dir.join("specs").join(cap);
            tokio::fs::create_dir_all(&spec_dir).await.unwrap();
            tokio::fs::write(spec_dir.join("spec.md"), "## ADDED Requirements\n")
                .await
                .unwrap();
        }
    }
    let canonical = root.canonicalize().unwrap();
    let workspace = WorkspaceFolder::from_path(canonical);
    (tmp, workspace)
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_cache_is_false() {
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn workspace_with_no_spec_deltas_is_false() {
    let (_tmp, ws) = workspace_with(&[("alpha", &[]), ("beta", &[])]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    manager.add_workspace(ws).await.unwrap();

    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn workspace_with_one_spec_delta_is_true() {
    let (_tmp, ws) = workspace_with(&[("alpha", &[]), ("beta", &["auth"])]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    manager.add_workspace(ws).await.unwrap();

    assert!(manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn second_workspace_carries_the_delta() {
    let (_tmp_a, ws_a) = workspace_with(&[("alpha", &[])]).await;
    let (_tmp_b, ws_b) = workspace_with(&[("beta", &["payments"])]).await;
    let manager = WatcherManager::new(TEST_DEBOUNCE);
    manager.add_workspace(ws_a).await.unwrap();
    manager.add_workspace(ws_b).await.unwrap();

    assert!(manager.any_change_touches_specs());
}
