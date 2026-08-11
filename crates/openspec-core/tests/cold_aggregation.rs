//! A disabled top-level row is aggregated *cold*: no git subprocess is spawned
//! on its behalf, while its cache-derived content stays exact.
//!
//! This lives in its own integration target rather than in `repo_view.rs`'s unit
//! tests because [`invocation_log`] is process-global. Its assertions are still
//! filtered to this test's own repository paths — belt and braces — so a future
//! test added to this binary cannot make them flaky.

use openspec_core::cache::WorkspaceCache;
use openspec_core::git::invocation_log;
use openspec_core::presentation::PresentationKey;
use openspec_core::repo_view::compute_views;
use openspec_core::{WorkspaceRegistry, WorkspaceView};

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn run_git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git repository with an `openspec/` subtree, one active change on disk, and
/// an untracked file so a warm `git status` has something to report.
fn init_repo(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("openspec/changes/foo")).unwrap();
    fs::write(root.join("openspec/changes/foo/proposal.md"), "x").unwrap();
    run_git(&["init", "-b", "main"], root);
    run_git(&["config", "user.email", "t@t"], root);
    run_git(&["config", "user.name", "t"], root);
    run_git(&["commit", "--allow-empty", "-m", "init"], root);
    fs::write(root.join("untracked.txt"), "dirty").unwrap();
    root.canonicalize().unwrap()
}

#[test]
fn no_git_is_invoked_for_a_disabled_repository() {
    invocation_log::enable();
    let tmp = TempDir::new().unwrap();
    let enabled = init_repo(&tmp.path().join("enabled"));
    let parked = init_repo(&tmp.path().join("parked"));

    let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
    reg.register(enabled.clone()).unwrap();
    reg.register(parked.clone()).unwrap();
    let cache = WorkspaceCache::new();

    let parked_repo_id = reg.entry(&parked).unwrap().repo_id.clone().unwrap();
    let parked_key = PresentationKey::Repo(parked_repo_id.as_path().to_path_buf());

    // Registration itself shells out to git; mark after it so only the
    // recompute's invocations are in scope.
    let mark = invocation_log::mark();
    let views = compute_views(&reg, &cache, |_| None, |key| key == &parked_key);
    let invocations = invocation_log::recorded_since(mark);

    let for_parked: Vec<_> = invocations
        .iter()
        .filter(|inv| inv.anchor.starts_with(&parked))
        .collect();
    assert!(
        for_parked.is_empty(),
        "a parked repository must cost no git invocation during aggregation: {for_parked:?}"
    );

    // Control: without it, a predicate that parked *everything* — or an
    // aggregation that had quietly stopped invoking git at all — would pass.
    let for_enabled: Vec<_> = invocations
        .iter()
        .filter(|inv| inv.anchor.starts_with(&enabled))
        .collect();
    assert!(
        !for_enabled.is_empty(),
        "the enabled repository must still be gathered warm"
    );

    // Both rows are still present; only the parked one is flagged.
    assert_eq!(views.len(), 2, "a parked row stays in the snapshot");
    let parked_view = views
        .iter()
        .find(|v| match v {
            WorkspaceView::Repo(r) => r.repo_id == parked_repo_id.as_path(),
            _ => false,
        })
        .expect("parked repo row present");
    assert!(parked_view.is_disabled());

    // And its cache-derived content survived the cold path: the change on disk
    // is reported, so the Dashboard keeps counting it.
    let WorkspaceView::Repo(parked_view) = parked_view else {
        unreachable!("filtered to a repo row above")
    };
    assert!(
        !parked_view.dirty,
        "no git ran, so no dirty rollup is reported"
    );
    assert_eq!(parked_view.default_branch, None);
}
