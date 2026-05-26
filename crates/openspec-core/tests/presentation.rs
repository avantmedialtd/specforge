//! Integration tests that exercise the workspace registry and the
//! presentation store together: register a workspace, set its presentation,
//! quit and relaunch (drop + reload), confirm persistence; then unregister
//! and confirm a manual cascade removes the right keys.

use openspec_core::{
    PaletteColor, PresentationKey, WorkspacePresentationStore, WorkspaceRegistry,
};
use std::fs;
use tempfile::TempDir;

fn make_openspec_workspace(root: &std::path::Path) -> std::path::PathBuf {
    fs::create_dir_all(root.join("openspec/changes")).unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn register_then_set_presentation_survives_restart() {
    let tmp = TempDir::new().unwrap();
    let ws = make_openspec_workspace(&tmp.path().join("ws"));
    let reg_path = tmp.path().join("workspaces.json");
    let pres_path = tmp.path().join("presentation.json");

    // First "session": register the workspace and set presentation.
    {
        let mut reg = WorkspaceRegistry::new(reg_path.clone());
        reg.register(ws.clone()).unwrap();
        let mut store = WorkspacePresentationStore::new(pres_path.clone());
        store
            .set(
                PresentationKey::flat(ws.clone()),
                Some("Avant Workspace".into()),
                Some(PaletteColor::Teal),
            )
            .unwrap();
    }

    // Second "session": load both from disk and verify presentation survived.
    let reg = WorkspaceRegistry::load(reg_path).unwrap();
    let store = WorkspacePresentationStore::load(pres_path).unwrap();
    assert!(reg.entry(&ws).is_some(), "workspace registration should restore");
    let key = PresentationKey::flat(ws);
    let (name, color) = store.lookup(&key);
    assert_eq!(name.as_deref(), Some("Avant Workspace"));
    assert_eq!(color, Some(PaletteColor::Teal));
}

#[test]
fn cascade_unregister_flat_workspace_drops_flat_presentation() {
    let tmp = TempDir::new().unwrap();
    let ws = make_openspec_workspace(&tmp.path().join("ws"));
    let pres_path = tmp.path().join("presentation.json");
    let mut store = WorkspacePresentationStore::new(pres_path.clone());
    let key = PresentationKey::flat(ws.clone());

    store
        .set(
            key.clone(),
            Some("Name".into()),
            Some(PaletteColor::Indigo),
        )
        .unwrap();
    assert!(store.get(&key).is_some());

    // Simulate the shell-level cascade: a flat workspace's Flat key is the
    // one that must drop on unregister.
    store.remove(&key).unwrap();
    assert!(store.get(&key).is_none());

    // Reloading from disk confirms the drop is persisted.
    let reloaded = WorkspacePresentationStore::load(pres_path).unwrap();
    assert!(reloaded.get(&key).is_none());
}

#[test]
fn repo_presentation_survives_until_last_user_registered_workspace_is_gone() {
    let tmp = TempDir::new().unwrap();
    let repo_common = tmp.path().join("r/.git");
    fs::create_dir_all(&repo_common).unwrap();
    let pres_path = tmp.path().join("presentation.json");
    let mut store = WorkspacePresentationStore::new(pres_path.clone());
    let key = PresentationKey::repo(repo_common.clone());

    store
        .set(
            key.clone(),
            Some("MyRepo".into()),
            Some(PaletteColor::Rose),
        )
        .unwrap();

    // Cascade does NOT fire: another user-registered workspace for this repo
    // remains, so the Repo presentation must stay.
    // (Simulated: we do not remove the key.)
    assert!(store.get(&key).is_some(), "Repo presentation must survive partial unregister");

    // Now the last user-registered workspace for this repo is unregistered:
    // the shell cascades by removing the Repo presentation key.
    store.remove(&key).unwrap();
    assert!(store.get(&key).is_none());
    let reloaded = WorkspacePresentationStore::load(pres_path).unwrap();
    assert!(reloaded.get(&key).is_none());
}
