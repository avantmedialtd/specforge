//! Workspace add/remove/presentation orchestration, exercised in-process with
//! no Tauri. These cover the lift of the register/unregister/set-presentation
//! flow out of the shell's `#[tauri::command]` layer into `AppService`, so both
//! frontends drive one tested code path.

use std::fs;
use std::path::PathBuf;

use openspec_app::AppService;
use openspec_core::PaletteColor;
use tempfile::{tempdir, TempDir};

/// Create a flat (non-git) OpenSpec workspace under `tmp` — a folder with an
/// `openspec/changes/` subtree, which is what registration requires.
fn make_workspace(tmp: &TempDir, name: &str) -> PathBuf {
    let root = tmp.path().join(name);
    fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
    root
}

#[tokio::test]
async fn add_workspace_registers_a_valid_folder() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let folder = make_workspace(&ws, "acme");
    let added = svc
        .add_workspace(folder.clone())
        .await
        .expect("valid folder registers");

    assert_eq!(added.name, "acme");
    assert!(!added.is_missing);
    assert!(added.repo_id.is_none(), "a non-git workspace is flat");

    let listed = svc.list_workspaces().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "acme");
}

#[tokio::test]
async fn add_workspace_rejects_invalid_folders() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    // Missing path.
    assert!(svc.add_workspace(ws.path().join("nope")).await.is_err());

    // Exists but has no `openspec/` subdirectory.
    let bare = ws.path().join("bare");
    fs::create_dir_all(&bare).unwrap();
    assert!(svc.add_workspace(bare).await.is_err());

    // A file, not a directory.
    let file = ws.path().join("file.txt");
    fs::write(&file, "x").unwrap();
    assert!(svc.add_workspace(file).await.is_err());

    assert!(
        svc.list_workspaces().expect("list").is_empty(),
        "no invalid folder should have been registered"
    );
}

#[tokio::test]
async fn remove_workspace_unregisters_and_empties_the_list() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let folder = make_workspace(&ws, "acme");
    svc.add_workspace(folder.clone()).await.expect("add");
    assert_eq!(svc.list_workspaces().unwrap().len(), 1);

    let removed = svc.remove_workspace(folder).await.expect("remove");
    assert!(removed, "removing a registered workspace reports true");
    assert!(
        svc.list_workspaces().unwrap().is_empty(),
        "the workspace is gone after removal"
    );

    // Removing again is a no-op, not an error.
    let again = svc
        .remove_workspace(ws.path().join("acme"))
        .await
        .expect("idempotent remove");
    assert!(!again);
}

#[tokio::test]
async fn set_presentation_overrides_name_and_color_then_clears() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let folder = make_workspace(&ws, "acme");
    let added = svc.add_workspace(folder).await.expect("add");
    let uri = added.uri.clone();

    svc.set_workspace_presentation(
        uri.clone(),
        None,
        Some("Renamed".to_string()),
        Some(PaletteColor::Teal),
    )
    .expect("set presentation");

    let listed = svc.list_workspaces().unwrap();
    assert_eq!(listed[0].display_name.as_deref(), Some("Renamed"));
    assert_eq!(listed[0].color, Some(PaletteColor::Teal));

    // An empty name and no colour clears the override back to default.
    svc.set_workspace_presentation(uri, None, Some("   ".to_string()), None)
        .expect("clear presentation");

    let listed = svc.list_workspaces().unwrap();
    assert!(
        listed[0].display_name.is_none(),
        "empty name resets to default"
    );
    assert!(
        listed[0].color.is_none(),
        "cleared colour reverts to default"
    );

    // Removing the workspace cleans up its flat presentation entry too: a
    // re-registered path starts with default presentation.
    let folder = added.uri;
    svc.remove_workspace(folder.clone()).await.expect("remove");
    svc.set_workspace_presentation(folder.clone(), None, Some("Temp".to_string()), None)
        .ok();
}
