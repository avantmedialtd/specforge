//! Workspace add/remove/presentation orchestration, exercised in-process with
//! no Tauri. These cover the lift of the register/unregister/set-presentation
//! flow out of the shell's `#[tauri::command]` layer into `AppService`, so both
//! frontends drive one tested code path.

use std::fs;
use std::path::PathBuf;

use openspec_app::AppService;
use openspec_core::{PaletteColor, WorkspaceView};
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

/// Unregistering clears the *whole* presentation entry, disabled flag included
/// — the `workspace-registry` "Presentation entry cleaned up when underlying
/// workspace is unregistered" scenario ends "…and enabled", and a disabled-only
/// entry is the one shape that is never pruned on save.
///
/// The removal is driven with the spelling `std::fs::canonicalize` produces,
/// because that is where the defect lived: `remove_workspace` used to key its
/// pre-unregister snapshot that way, and on Windows std returns the verbatim
/// `\\?\…` form that never matches a dunce-canonical registry key — so the
/// lookup missed, the whole presentation cascade was skipped, and the orphaned
/// `disabled: true` entry silently re-parked the folder on re-registration.
/// Off Windows the two spellings coincide, so on macOS/Linux this stands as a
/// contract test rather than a fail-before regression test.
#[tokio::test]
async fn unregistering_a_parked_workspace_clears_its_disabled_flag() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let added = svc
        .add_workspace(make_workspace(&ws, "parked"))
        .await
        .expect("add");
    let uri = added.uri.clone();
    svc.set_workspace_disabled(uri.clone(), None, true)
        .await
        .expect("park");
    assert!(svc.list_workspaces().unwrap()[0].disabled);
    assert!(
        svc.workspace_views().is_empty(),
        "precondition: a parked row leaves the tree"
    );

    let spelled = std::fs::canonicalize(&uri).expect("canonicalise");
    assert!(svc.remove_workspace(spelled).await.expect("remove"));
    assert!(
        svc.presentation.lock().unwrap().is_empty(),
        "the presentation entry must not outlive the registration"
    );

    // The user-visible consequence: re-registering comes back enabled and
    // visible, not silently re-parked with no cue as to why.
    svc.add_workspace(uri).await.expect("re-register");
    assert!(!svc.list_workspaces().unwrap()[0].disabled);
    assert_eq!(svc.workspace_views().len(), 1);
}

/// The single accessor every frontend serves its tree from: it drops parked
/// rows *and* joins presentation overrides into the survivors. The desktop
/// shell's `get_workspace_views` delegates here instead of re-implementing the
/// pair in the Tauri crate (which neither `cargo test` nor `cargo mutants`
/// reaches), so this is the only coverage either half has for it.
#[tokio::test]
async fn workspace_views_join_presentation_and_exclude_disabled_rows() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let added = svc
        .add_workspace(make_workspace(&ws, "acme"))
        .await
        .expect("add");
    let uri = added.uri.clone();
    svc.set_workspace_presentation(
        uri.clone(),
        None,
        Some("Renamed".to_string()),
        Some(PaletteColor::Teal),
    )
    .expect("set presentation");

    match svc.workspace_views().as_slice() {
        [WorkspaceView::Flat {
            display_name,
            color,
            ..
        }] => {
            assert_eq!(display_name.as_deref(), Some("Renamed"));
            assert_eq!(*color, Some(PaletteColor::Teal));
        }
        other => panic!("expected one joined flat row, got {other:?}"),
    }

    svc.set_workspace_disabled(uri, None, true)
        .await
        .expect("park");
    assert!(
        svc.workspace_views().is_empty(),
        "a parked row leaves the tree"
    );
}

/// Startup must survive a settings file naming a reading width this build does
/// not know — one written by a newer version, or edited by hand.
///
/// Every frontend starts the same way (`AppService::bootstrap`), so this covers
/// the desktop shell, the browser skin and the terminal frontend at once. The
/// terminal frontend in particular does not implement the reading width at all
/// and must simply ignore the field (`document-width`: *The Terminal Frontend
/// Does Not Apply the Reading Width*).
///
/// The assertion is on the NEIGHBOURS as much as on the width: settings are
/// parsed in one piece and fall back to the complete defaults when that parse
/// fails, so a strict enum here would not report an unknown width — it would
/// silently reset every other preference in the file.
#[tokio::test]
async fn bootstrap_survives_an_unknown_reading_width() {
    let cfg = tempdir().unwrap();
    fs::write(
        cfg.path().join("settings.json"),
        r#"{
            "notificationsEnabled": false,
            "documentWidth": "ultrawide",
            "favoriteChangeIds": ["repo:/r/main/lc:add-dark-mode"],
            "identity": { "displayName": "Ada" },
            "web": { "enabled": true, "port": 4399 }
        }"#,
    )
    .unwrap();

    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    assert_eq!(
        svc.settings.document_width(),
        openspec_app::DocumentWidth::Default,
        "an unrecognised rung loads as the default"
    );

    let settings = svc.settings.snapshot();
    assert!(!settings.notifications_enabled, "notifications kept");
    assert_eq!(
        settings.favorite_change_ids,
        vec!["repo:/r/main/lc:add-dark-mode".to_string()],
        "favorites kept"
    );
    assert_eq!(
        settings.identity.display_name.as_deref(),
        Some("Ada"),
        "identity kept"
    );
    assert!(settings.web.enabled, "web config kept");
    assert_eq!(settings.web.port, 4399, "web port kept");
}
