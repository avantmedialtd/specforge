//! Thin wrapper over the system `git` binary.
//!
//! All operations shell out to `git`. If `git` is missing or returns a
//! non-zero exit code the relevant function returns `None` (or an empty
//! list) — callers degrade gracefully to "not a git repository" rather
//! than failing.
//!
//! Operations used:
//! - `rev-parse --git-common-dir` — identify the repo a workspace belongs to
//! - `symbolic-ref --short refs/remotes/origin/HEAD` — default branch from remote
//! - `config --get init.defaultBranch` — default branch fallback
//! - `branch --show-current` — current branch of a worktree
//! - `worktree list --porcelain` — enumerate every worktree of a repo

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Canonical absolute path to a repository's git common directory (the one
/// shared by every worktree). Two worktrees of the same repository have the
/// same `RepoId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(pub PathBuf);

impl RepoId {
    pub fn as_path(&self) -> &Path {
        &self.0
    }
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

/// A single worktree of a repository, as parsed from
/// `git worktree list --porcelain`. `branch` is `None` for detached HEAD or
/// bare worktrees. `is_main` marks the canonical first entry (the directory
/// that contains `.git/`, not a `.git` file). `is_prunable` marks worktrees
/// whose on-disk path is missing but whose metadata still exists under
/// `.git/worktrees/<name>/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_prunable: bool,
}

/// Detect the git common directory that owns `path`. Returns `None` when
/// `path` is not inside a git repository or when `git` is missing on PATH.
pub fn git_common_dir(path: &Path) -> Option<RepoId> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = Path::new(trimmed);
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        path.join(candidate)
    };
    absolute.canonicalize().ok().map(RepoId)
}

/// Resolve the default branch of a repository via the documented cascade:
///
/// 1. `refs/remotes/origin/HEAD` (stripped of the `origin/` prefix)
/// 2. `init.defaultBranch` from the repository's config
/// 3. The branch currently checked out in the main worktree
///
/// Returns `None` when every step fails (e.g. brand-new bare repo with no
/// remote, no init config, and a detached HEAD in the main worktree).
pub fn default_branch(common_dir: &RepoId) -> Option<String> {
    if let Some(value) = symbolic_ref(common_dir, "refs/remotes/origin/HEAD") {
        let stripped = value.strip_prefix("origin/").unwrap_or(&value);
        if !stripped.is_empty() {
            return Some(stripped.to_string());
        }
    }
    if let Some(value) = config_get(common_dir, "init.defaultBranch") {
        if !value.is_empty() {
            return Some(value);
        }
    }
    if let Some(main_worktree) = main_worktree_path(common_dir) {
        if let Some(value) = current_branch(&main_worktree) {
            return Some(value);
        }
    }
    None
}

/// Branch currently checked out in `worktree_path`, or `None` if HEAD is
/// detached or if `git` reports nothing usable.
pub fn current_branch(worktree_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(worktree_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Enumerate every worktree of the repository identified by `common_dir`.
/// Returns an empty vec on error (git missing, command failed) — callers
/// treat that the same as "not a git repository."
pub fn worktree_list(common_dir: &RepoId) -> Vec<WorktreeInfo> {
    let output = match Command::new("git")
        .args([
            "--git-dir",
            &common_dir.0.to_string_lossy(),
            "worktree",
            "list",
            "--porcelain",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    parse_worktree_porcelain(&raw)
}

fn parse_worktree_porcelain(text: &str) -> Vec<WorktreeInfo> {
    let mut out = Vec::new();
    let blocks: Vec<&str> = text
        .split("\n\n")
        .filter(|b| !b.trim().is_empty())
        .collect();
    for (idx, block) in blocks.iter().enumerate() {
        let mut path: Option<PathBuf> = None;
        let mut branch: Option<String> = None;
        let mut prunable = false;
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(rest));
            } else if let Some(rest) = line.strip_prefix("branch ") {
                let stripped = rest.strip_prefix("refs/heads/").unwrap_or(rest);
                if !stripped.is_empty() {
                    branch = Some(stripped.to_string());
                }
            } else if line.starts_with("prunable") {
                prunable = true;
            }
        }
        let Some(path) = path else { continue };
        // For prunable worktrees the path is missing on disk so canonicalize
        // fails — fall back to the literal path so we can still identify it
        // for removal.
        let resolved = path.canonicalize().unwrap_or(path);
        out.push(WorktreeInfo {
            path: resolved,
            branch,
            is_main: idx == 0,
            is_prunable: prunable,
        });
    }
    out
}

fn symbolic_ref(common_dir: &RepoId, ref_name: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "--git-dir",
            &common_dir.0.to_string_lossy(),
            "symbolic-ref",
            "--short",
            ref_name,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn config_get(common_dir: &RepoId, key: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "--git-dir",
            &common_dir.0.to_string_lossy(),
            "config",
            "--get",
            key,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// The directory that contains the common `.git` (= main worktree root for
/// non-bare repos). For bare repositories this returns the parent of the
/// bare git directory which is rarely useful, but `default_branch`'s step 3
/// is only attempted when steps 1 and 2 both failed so a misfire is benign.
fn main_worktree_path(common_dir: &RepoId) -> Option<PathBuf> {
    common_dir.0.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(args: &[&str], cwd: &Path) {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git invocation");
        assert!(
            output.status.success(),
            "git {:?} in {} failed: {}",
            args,
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Initialise a git repo at `root` with a deterministic config and one
    /// empty commit on a `main` branch. Returns the canonicalised path so
    /// tests can compare against `git_common_dir`.
    fn init_repo(root: &Path) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        git(&["init", "-b", "main"], root);
        git(&["config", "user.email", "test@example.com"], root);
        git(&["config", "user.name", "Test"], root);
        git(&["commit", "--allow-empty", "-m", "initial"], root);
        root.canonicalize().unwrap()
    }

    #[test]
    fn git_common_dir_resolves_for_a_repo() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let common = git_common_dir(&root).expect("common dir");
        assert_eq!(common.0, root.join(".git").canonicalize().unwrap());
    }

    #[test]
    fn git_common_dir_returns_none_outside_repo() {
        let tmp = TempDir::new().unwrap();
        assert!(git_common_dir(tmp.path()).is_none());
    }

    #[test]
    fn default_branch_falls_back_to_main_worktree_branch_without_remote() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let common = git_common_dir(&root).unwrap();
        let branch = default_branch(&common);
        assert_eq!(branch.as_deref(), Some("main"));
    }

    #[test]
    fn default_branch_uses_init_default_when_no_remote() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        git(&["config", "init.defaultBranch", "trunk"], &root);
        let common = git_common_dir(&root).unwrap();
        let branch = default_branch(&common);
        assert_eq!(branch.as_deref(), Some("trunk"));
    }

    #[test]
    fn worktree_list_single_worktree() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let common = git_common_dir(&root).unwrap();
        let entries = worktree_list(&common);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_main);
        assert!(!entries[0].is_prunable);
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[0].path, root);
    }

    #[test]
    fn worktree_list_multiple_worktrees() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let wt2 = tmp.path().join("wt2");
        git(
            &["worktree", "add", "-b", "feature", wt2.to_str().unwrap()],
            &root,
        );
        let common = git_common_dir(&root).unwrap();
        let entries = worktree_list(&common);
        assert_eq!(entries.len(), 2, "{:?}", entries);
        let main = entries.iter().find(|e| e.is_main).unwrap();
        let secondary = entries.iter().find(|e| !e.is_main).unwrap();
        assert_eq!(main.branch.as_deref(), Some("main"));
        assert_eq!(secondary.branch.as_deref(), Some("feature"));
        assert!(!main.is_prunable);
        assert!(!secondary.is_prunable);
    }

    #[test]
    fn worktree_list_marks_prunable_when_path_is_deleted() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let wt2 = tmp.path().join("ephemeral");
        git(
            &["worktree", "add", "-b", "ephemeral", wt2.to_str().unwrap()],
            &root,
        );
        // Simulate `rm -rf` on the worktree path without running prune.
        fs::remove_dir_all(&wt2).unwrap();

        let common = git_common_dir(&root).unwrap();
        let entries = worktree_list(&common);
        let ephemeral = entries
            .iter()
            .find(|e| e.branch.as_deref() == Some("ephemeral"))
            .expect("ephemeral worktree entry");
        assert!(ephemeral.is_prunable);
    }

    #[test]
    fn worktree_list_empty_when_path_not_in_repo() {
        let tmp = TempDir::new().unwrap();
        // tmp is not a git repo; common_dir for it would be None — but we can
        // still feed an invalid RepoId and expect an empty vec.
        let bogus = RepoId(tmp.path().join("nope/.git"));
        let entries = worktree_list(&bogus);
        assert!(entries.is_empty());
    }

    #[test]
    fn git_common_dir_returns_same_id_from_secondary_worktree() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let wt2 = tmp.path().join("wt2");
        git(
            &["worktree", "add", "-b", "feature", wt2.to_str().unwrap()],
            &root,
        );
        let a = git_common_dir(&root).unwrap();
        let b = git_common_dir(&wt2).unwrap();
        assert_eq!(a, b, "common dir must match across worktrees");
    }
}
