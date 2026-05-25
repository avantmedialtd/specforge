use openspec_core::{RegistrationError, WorkspaceRegistry};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn config_path(tmp: &TempDir) -> PathBuf {
    tmp.path().join("workspaces.json")
}

/// Creates a directory inside `tmp` that looks like a valid OpenSpec
/// workspace (contains an `openspec/` subdirectory). Returns the
/// workspace root and the canonical form of it.
fn make_workspace(tmp: &TempDir, name: &str) -> (PathBuf, PathBuf) {
    let root = tmp.path().join(name);
    fs::create_dir_all(root.join("openspec")).unwrap();
    let canonical = root.canonicalize().unwrap();
    (root, canonical)
}

#[test]
fn register_accepts_a_folder_containing_openspec() {
    let tmp = TempDir::new().unwrap();
    let (root, canonical) = make_workspace(&tmp, "alpha");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    let added = registry.register(root).expect("registration should succeed");

    assert_eq!(added.len(), 1, "non-git workspace adds only itself");
    assert_eq!(added[0].uri, canonical);
    assert_eq!(added[0].name, "alpha");
    assert_eq!(registry.len(), 1);
}

#[test]
fn register_rejects_folder_without_openspec_subdir() {
    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("not-a-workspace");
    fs::create_dir_all(&plain).unwrap();

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    let err = registry.register(plain).expect_err("should be rejected");

    assert!(matches!(err, RegistrationError::NotAnOpenSpecWorkspace(_)));
    assert!(registry.is_empty());
}

#[test]
fn register_rejects_missing_path() {
    let tmp = TempDir::new().unwrap();
    let absent = tmp.path().join("does-not-exist");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    let err = registry.register(absent).expect_err("should be rejected");

    assert!(matches!(err, RegistrationError::PathNotFound(_)));
}

#[test]
fn register_rejects_a_file() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("plain-file");
    fs::write(&file, b"not a workspace").unwrap();

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    let err = registry.register(file).expect_err("should be rejected");

    assert!(matches!(err, RegistrationError::NotADirectory(_)));
}

#[test]
fn register_rejects_duplicate() {
    let tmp = TempDir::new().unwrap();
    let (root, _) = make_workspace(&tmp, "beta");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    registry.register(root.clone()).unwrap();
    let err = registry
        .register(root)
        .expect_err("second registration should fail");

    assert!(matches!(err, RegistrationError::AlreadyRegistered(_)));
    assert_eq!(registry.len(), 1);
}

#[test]
fn unregister_removes_existing_workspace() {
    let tmp = TempDir::new().unwrap();
    let (root, canonical) = make_workspace(&tmp, "gamma");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    registry.register(root).unwrap();
    let removed = registry.unregister(&canonical).unwrap();

    assert_eq!(removed, vec![canonical]);
    assert!(registry.is_empty());
}

#[test]
fn unregister_returns_empty_for_unknown_workspace() {
    let tmp = TempDir::new().unwrap();
    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    let removed = registry
        .unregister(&tmp.path().join("never-registered"))
        .unwrap();
    assert!(removed.is_empty());
}

#[test]
fn list_marks_workspace_missing_when_folder_was_deleted() {
    let tmp = TempDir::new().unwrap();
    let (root, canonical) = make_workspace(&tmp, "delta");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    registry.register(root).unwrap();
    fs::remove_dir_all(&canonical).unwrap();

    let listed = registry.list();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].is_missing);
    assert_eq!(listed[0].uri, canonical);
}

#[test]
fn list_sorts_by_name() {
    let tmp = TempDir::new().unwrap();
    let (root_b, _) = make_workspace(&tmp, "beta");
    let (root_a, _) = make_workspace(&tmp, "alpha");
    let (root_c, _) = make_workspace(&tmp, "charlie");

    let mut registry = WorkspaceRegistry::new(config_path(&tmp));
    registry.register(root_b).unwrap();
    registry.register(root_c).unwrap();
    registry.register(root_a).unwrap();

    let names: Vec<_> = registry.list().into_iter().map(|w| w.name).collect();
    assert_eq!(names, vec!["alpha", "beta", "charlie"]);
}

#[test]
fn load_returns_empty_registry_when_config_absent() {
    let tmp = TempDir::new().unwrap();
    let registry = WorkspaceRegistry::load(config_path(&tmp)).unwrap();
    assert!(registry.is_empty());
}

#[test]
fn register_and_reload_round_trip() {
    let tmp = TempDir::new().unwrap();
    let (root1, canonical1) = make_workspace(&tmp, "alpha");
    let (root2, canonical2) = make_workspace(&tmp, "beta");
    let cfg = config_path(&tmp);

    {
        let mut registry = WorkspaceRegistry::new(cfg.clone());
        registry.register(root1).unwrap();
        registry.register(root2).unwrap();
    }

    let reloaded = WorkspaceRegistry::load(cfg).unwrap();
    assert_eq!(reloaded.len(), 2);
    let listed = reloaded.list();
    assert_eq!(listed[0].uri, canonical1);
    assert_eq!(listed[1].uri, canonical2);
}

#[test]
fn unregister_persists_to_disk() {
    let tmp = TempDir::new().unwrap();
    let (root, canonical) = make_workspace(&tmp, "alpha");
    let cfg = config_path(&tmp);

    let mut registry = WorkspaceRegistry::new(cfg.clone());
    registry.register(root).unwrap();
    registry.unregister(&canonical).unwrap();

    let reloaded = WorkspaceRegistry::load(cfg).unwrap();
    assert!(reloaded.is_empty());
}
