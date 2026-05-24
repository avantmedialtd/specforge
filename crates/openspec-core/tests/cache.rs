use openspec_core::{ArtifactStatus, ChangeData, WorkspaceCache, WorkspaceFolder};
use std::path::PathBuf;

fn workspace(path: &str) -> WorkspaceFolder {
    WorkspaceFolder::from_path(PathBuf::from(path))
}

fn change(workspace: &WorkspaceFolder, id: &str) -> ChangeData {
    ChangeData {
        change_id: id.to_string(),
        title: None,
        sections: vec![],
        total_tasks: 0,
        completed_tasks: 0,
        artifacts: ArtifactStatus::default(),
        workspace: workspace.clone(),
    }
}

#[test]
fn new_cache_is_empty() {
    let cache = WorkspaceCache::new();
    assert_eq!(cache.workspace_count(), 0);
    assert_eq!(cache.total_active_count(), 0);
    assert!(cache.is_empty());
}

#[test]
fn insert_and_changes_for_round_trip() {
    let mut cache = WorkspaceCache::new();
    let ws = workspace("/tmp/alpha");
    cache.insert(
        ws.uri.clone(),
        vec![change(&ws, "first"), change(&ws, "second")],
    );

    let listed = cache.changes_for(&ws.uri);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].change_id, "first");
    assert_eq!(listed[1].change_id, "second");
}

#[test]
fn changes_for_unknown_workspace_returns_empty_slice() {
    let cache = WorkspaceCache::new();
    let listed = cache.changes_for(&PathBuf::from("/nowhere"));
    assert!(listed.is_empty());
}

#[test]
fn insert_replaces_existing_entry() {
    let mut cache = WorkspaceCache::new();
    let ws = workspace("/tmp/alpha");
    cache.insert(ws.uri.clone(), vec![change(&ws, "old")]);
    cache.insert(ws.uri.clone(), vec![change(&ws, "new-one"), change(&ws, "new-two")]);

    let listed = cache.changes_for(&ws.uri);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].change_id, "new-one");
}

#[test]
fn remove_returns_old_value_and_clears() {
    let mut cache = WorkspaceCache::new();
    let ws = workspace("/tmp/alpha");
    cache.insert(ws.uri.clone(), vec![change(&ws, "a")]);
    let removed = cache.remove(&ws.uri).unwrap();
    assert_eq!(removed.len(), 1);
    assert!(cache.is_empty());
}

#[test]
fn total_active_count_sums_across_workspaces() {
    let mut cache = WorkspaceCache::new();
    let alpha = workspace("/tmp/alpha");
    let beta = workspace("/tmp/beta");
    cache.insert(
        alpha.uri.clone(),
        vec![change(&alpha, "a"), change(&alpha, "b")],
    );
    cache.insert(beta.uri.clone(), vec![change(&beta, "c")]);

    assert_eq!(cache.workspace_count(), 2);
    assert_eq!(cache.total_active_count(), 3);
}

#[test]
fn snapshot_returns_clone_independent_of_subsequent_mutation() {
    let mut cache = WorkspaceCache::new();
    let ws = workspace("/tmp/alpha");
    cache.insert(ws.uri.clone(), vec![change(&ws, "a")]);

    let snap = cache.snapshot();
    cache.insert(ws.uri.clone(), vec![]);

    assert_eq!(snap.get(&ws.uri).unwrap().len(), 1);
    assert_eq!(cache.changes_for(&ws.uri).len(), 0);
}
