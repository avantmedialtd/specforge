//! The tray's *attention surface*: `WatcherManager::any_change_touches_specs`
//! (the glyph-variant predicate) and `WatcherManager::total_active_logical_count`
//! (the badge count).
//!
//! Both read the aggregated `last_views` snapshot through one shared exclusion
//! point, so a top-level row the user has parked drives neither. Reading the raw
//! cache for the glyph instead — the v0.16.1 behaviour — kept a parked
//! repository flipping the menu-bar icon, because the cache deliberately stays
//! live while a workspace is parked.
//!
//! Because the predicate is view-derived, every fixture here builds the manager
//! with a registry *and* a presentation store: a `WatcherManager::new()` has no
//! registry, so `last_views` never populates and the predicate is always false.

use openspec_core::presentation::{PresentationKey, WorkspacePresentationStore};
use openspec_core::{WatcherManager, WorkspaceRegistry, WorkspaceView};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

const TEST_DEBOUNCE: Duration = Duration::from_millis(50);

/// Writes a workspace's OpenSpec tree into `root`. Each change is
/// `(change_id, capability_spec_names)`; an empty `capability_spec_names` slice
/// means the change has no spec delta.
fn write_changes(root: &Path, changes: &[(&str, &[&str])]) {
    std::fs::create_dir_all(root.join("openspec/changes")).unwrap();
    for (change_id, specs) in changes {
        let change_dir = root.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), format!("# {change_id}\n")).unwrap();
        for cap in *specs {
            let spec_dir = change_dir.join("specs").join(cap);
            std::fs::create_dir_all(&spec_dir).unwrap();
            std::fs::write(spec_dir.join("spec.md"), "## ADDED Requirements\n").unwrap();
        }
    }
}

fn run_git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(out.status.success(), "git {args:?} failed");
}

/// A flat (non-git) workspace — aggregates to `WorkspaceView::Flat`.
fn flat_workspace(root: &Path, changes: &[(&str, &[&str])]) -> PathBuf {
    write_changes(root, changes);
    root.canonicalize().unwrap()
}

/// A git-backed workspace — aggregates to `WorkspaceView::Repo`, the arm whose
/// spec scan walks logical changes and their instances.
fn git_workspace(root: &Path, changes: &[(&str, &[&str])]) -> PathBuf {
    write_changes(root, changes);
    run_git(&["init", "-b", "main"], root);
    run_git(&["config", "user.email", "t@t"], root);
    run_git(&["config", "user.name", "t"], root);
    run_git(&["commit", "--allow-empty", "-m", "init"], root);
    root.canonicalize().unwrap()
}

/// A live manager over `roots`, aggregated once. Returns it alongside the
/// presentation store and the registry so a test can park a row and
/// re-aggregate.
async fn manager_over(
    tmp: &TempDir,
    roots: &[PathBuf],
) -> (
    WatcherManager,
    Arc<Mutex<WorkspacePresentationStore>>,
    Arc<Mutex<WorkspaceRegistry>>,
) {
    let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
    for root in roots {
        reg.register(root.clone()).unwrap();
    }
    let registry = Arc::new(Mutex::new(reg));

    let store = Arc::new(Mutex::new(WorkspacePresentationStore::new(
        tmp.path().join("presentation.json"),
    )));

    let manager = WatcherManager::with_registry(TEST_DEBOUNCE, Some(registry.clone()));
    manager.set_presentation(store.clone());
    let folders = registry.lock().unwrap().folders();
    for folder in folders {
        manager.add_workspace(folder).await.unwrap();
    }
    manager.sync_repos();
    manager.aggregate_and_emit();
    (manager, store, registry)
}

/// The top-level row key a workspace parks under: `repo:` for a git-backed
/// workspace, `flat:` otherwise.
fn key_for(registry: &Arc<Mutex<WorkspaceRegistry>>, root: &Path) -> PresentationKey {
    let reg = registry.lock().unwrap();
    match &reg.entry(root).expect("registered workspace").repo_id {
        Some(repo_id) => PresentationKey::Repo(repo_id.as_path().to_path_buf()),
        None => PresentationKey::Flat(root.to_path_buf()),
    }
}

fn set_disabled(store: &Arc<Mutex<WorkspacePresentationStore>>, key: PresentationKey, on: bool) {
    store.lock().unwrap().set_disabled(key, on).unwrap();
}

// -------------------------------------------------------------------------
// The predicate itself, over both view shapes.
// -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn empty_registry_is_false() {
    let tmp = TempDir::new().unwrap();
    let (manager, _store, _reg) = manager_over(&tmp, &[]).await;
    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn flat_workspace_with_no_spec_deltas_is_false() {
    let tmp = TempDir::new().unwrap();
    let ws = flat_workspace(&tmp.path().join("flat"), &[("alpha", &[]), ("beta", &[])]);
    let (manager, _store, _reg) = manager_over(&tmp, &[ws]).await;
    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn flat_workspace_with_one_spec_delta_is_true() {
    let tmp = TempDir::new().unwrap();
    let ws = flat_workspace(
        &tmp.path().join("flat"),
        &[("alpha", &[]), ("beta", &["auth"])],
    );
    let (manager, _store, _reg) = manager_over(&tmp, &[ws]).await;
    assert!(manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_workspace_with_no_spec_deltas_is_false() {
    let tmp = TempDir::new().unwrap();
    let ws = git_workspace(&tmp.path().join("repo"), &[("alpha", &[])]);
    let (manager, _store, _reg) = manager_over(&tmp, &[ws]).await;
    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_workspace_with_one_spec_delta_is_true() {
    let tmp = TempDir::new().unwrap();
    let ws = git_workspace(
        &tmp.path().join("repo"),
        &[("alpha", &[]), ("beta", &["payments"])],
    );
    let (manager, _store, _reg) = manager_over(&tmp, &[ws]).await;
    assert!(manager.any_change_touches_specs());
}

/// The predicate moved from the raw cache to `last_views`, which — unlike the
/// cache — also carries *archived* content. It must not have gained a source of
/// truth: only non-archived changes drive the glyph, exactly as before.
///
/// Two shapes of archived content are present here, both spec-touching on disk:
///   * `archive/2026-01-01-gamma/` — a wholly-archived logical change, which
///     buckets into `RepoView::archived` and is never scanned;
///   * `archive/beta/` — an undated archive directory sharing its name with the
///     live `beta`, so its stub instance rides *inside* the `active` logical
///     change alongside the live one. That is the only way an archived instance
///     reaches the scanned collection, and it stays harmless solely because
///     `list_archived_stubs` synthesises `ArtifactStatus::default()`.
#[tokio::test(flavor = "multi_thread")]
async fn archived_spec_deltas_never_drive_the_glyph() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("repo");
    let ws = git_workspace(&root, &[("beta", &[])]);

    let archive = root.join("openspec/changes/archive");
    for dir in ["beta", "2026-01-01-gamma"] {
        let spec_dir = archive.join(dir).join("specs/payments");
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::write(spec_dir.join("spec.md"), "## ADDED Requirements\n").unwrap();
        std::fs::write(archive.join(dir).join("proposal.md"), "# archived\n").unwrap();
    }

    let (manager, _store, _reg) = manager_over(&tmp, std::slice::from_ref(&ws)).await;

    // The fixture is live: both archived shapes really did reach the view, so a
    // `false` below is the exclusion working rather than an empty row.
    let views = manager.workspace_views();
    let WorkspaceView::Repo(repo) = &views[0] else {
        panic!("expected a repo row")
    };
    assert_eq!(
        repo.archived.len(),
        1,
        "the dated archive is its own logical change"
    );
    assert!(
        repo.active
            .iter()
            .flat_map(|lc| &lc.instances)
            .any(|i| i.is_archived_here),
        "the undated archive rides inside the active logical change"
    );

    assert!(
        !manager.any_change_touches_specs(),
        "only non-archived changes drive the glyph; an archived change's spec \
         delta must not flip it, whether it buckets into `archived` or rides \
         along inside an `active` logical change"
    );
}

// -------------------------------------------------------------------------
// A parked row is not asking for attention — the regression this file exists
// for. The glyph and the badge must agree on which rows are excluded.
// -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn parked_repo_with_a_spec_delta_does_not_flip_the_glyph() {
    let tmp = TempDir::new().unwrap();
    let with_specs = git_workspace(&tmp.path().join("a"), &[("beta", &["payments"])]);
    let without = git_workspace(&tmp.path().join("b"), &[("alpha", &[])]);
    let (manager, store, reg) = manager_over(&tmp, &[with_specs.clone(), without]).await;

    // Control: the fixture really does drive the glyph while enabled.
    assert!(
        manager.any_change_touches_specs(),
        "control: an enabled repo's spec delta reaches the glyph"
    );

    set_disabled(&store, key_for(&reg, &with_specs), true);
    manager.aggregate_and_emit();

    assert!(
        !manager.any_change_touches_specs(),
        "a parked repository must not keep flipping the menu-bar glyph — its \
         cache stays live by design, so the predicate must read the view"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_parked_row_does_not_hide_an_enabled_row_with_specs() {
    let tmp = TempDir::new().unwrap();
    let with_specs = git_workspace(&tmp.path().join("a"), &[("beta", &["payments"])]);
    let without = git_workspace(&tmp.path().join("b"), &[("alpha", &[])]);
    let (manager, store, reg) = manager_over(&tmp, &[with_specs, without.clone()]).await;

    // Park the row that carries *no* spec delta: the exclusion is per row, not
    // a global off-switch.
    set_disabled(&store, key_for(&reg, &without), true);
    manager.aggregate_and_emit();

    assert!(
        manager.any_change_touches_specs(),
        "parking one row must not suppress another row's spec activity"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parked_flat_workspace_does_not_flip_the_glyph() {
    let tmp = TempDir::new().unwrap();
    let ws = flat_workspace(&tmp.path().join("flat"), &[("beta", &["auth"])]);
    let (manager, store, reg) = manager_over(&tmp, std::slice::from_ref(&ws)).await;
    assert!(manager.any_change_touches_specs(), "control: enabled");

    set_disabled(&store, key_for(&reg, &ws), true);
    manager.aggregate_and_emit();

    assert!(!manager.any_change_touches_specs());
}

#[tokio::test(flavor = "multi_thread")]
async fn un_parking_restores_the_glyph() {
    let tmp = TempDir::new().unwrap();
    let ws = git_workspace(&tmp.path().join("repo"), &[("beta", &["payments"])]);
    let (manager, store, reg) = manager_over(&tmp, std::slice::from_ref(&ws)).await;
    let key = key_for(&reg, &ws);

    set_disabled(&store, key.clone(), true);
    manager.aggregate_and_emit();
    assert!(!manager.any_change_touches_specs(), "precondition: parked");

    set_disabled(&store, key, false);
    manager.aggregate_and_emit();

    assert!(
        manager.any_change_touches_specs(),
        "re-enabling restores the spec-activity variant in one recompute, \
         without waiting for a filesystem event"
    );
}

// -------------------------------------------------------------------------
// The badge shares the glyph's exclusion point, so it is pinned here too — the
// existing badge assertions elsewhere are all 0 or 1, which a per-row count
// mutated to a constant 1 would still satisfy.
// -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_badge_count_shares_the_glyphs_row_exclusion() {
    let tmp = TempDir::new().unwrap();
    // Two changes per row, so a per-row count collapsed to a constant 1 is
    // visible in the total — every other badge assertion in the suite is 0 or 1.
    let repo = git_workspace(&tmp.path().join("repo"), &[("alpha", &[]), ("beta", &[])]);
    let flat = flat_workspace(&tmp.path().join("flat"), &[("gamma", &[]), ("delta", &[])]);
    let (manager, store, reg) = manager_over(&tmp, &[repo.clone(), flat.clone()]).await;

    assert_eq!(
        manager.total_active_logical_count(),
        4,
        "two logical changes in the repo row plus two in the flat row"
    );

    set_disabled(&store, key_for(&reg, &repo), true);
    manager.aggregate_and_emit();
    assert_eq!(
        manager.total_active_logical_count(),
        2,
        "parking the repo row removes exactly its two changes"
    );

    set_disabled(&store, key_for(&reg, &flat), true);
    manager.aggregate_and_emit();
    assert_eq!(
        manager.total_active_logical_count(),
        0,
        "parking every row empties the badge"
    );
}
