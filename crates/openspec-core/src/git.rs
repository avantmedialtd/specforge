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

/// The kind of a ref decoration attached to a commit, parsed from
/// `git log`'s `%D` placeholder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefKind {
    /// A local branch head (e.g. `master`).
    LocalBranch,
    /// A remote-tracking branch head (e.g. `origin/master`).
    RemoteBranch,
    /// An annotated or lightweight tag (e.g. `v0.1.0`).
    Tag,
    /// The `HEAD` pointer itself (present on the checked-out commit).
    Head,
}

/// A single ref decoration on a commit. `name` is the human-facing label
/// (`master`, `origin/master`, `v0.1.0`, or `HEAD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitRef {
    pub name: String,
    pub kind: RefKind,
}

/// One commit as extracted from `git log` — the raw input to the graph
/// layout. Not itself an IPC type; the layout converts it into a serialised
/// laid-out commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCommit {
    /// Full 40-char commit hash.
    pub id: String,
    /// Parent hashes in git's order (first parent is the mainline).
    pub parents: Vec<String>,
    pub author: String,
    /// Author date, ISO-8601 / strict (`%aI`).
    pub date: String,
    pub subject: String,
    /// Structured ref decorations parsed from `%D`.
    pub refs: Vec<CommitRef>,
}

/// One file touched by a commit, for the commit-detail view. `additions` and
/// `deletions` are `None` for binary files (git reports `-` there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    /// Single-letter status from `--name-status` (`A`, `M`, `D`, …).
    pub status: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
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

/// The set of configured remote names (`git remote`). Used to classify ref
/// decorations as local vs remote branches. Empty on any error.
fn remotes(common_dir: &RepoId) -> std::collections::HashSet<String> {
    let output = Command::new("git")
        .args(["--git-dir", &common_dir.0.to_string_lossy(), "remote"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout)
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => std::collections::HashSet::new(),
    }
}

/// Parse a `%D` decoration string into structured refs. Tokens are
/// comma-space separated, e.g. `HEAD -> master, origin/master, tag: v0.1.0`.
/// Unknown markers (`grafted`, `replaced`) are skipped.
fn parse_decorations(raw: &str, remotes: &std::collections::HashSet<String>) -> Vec<CommitRef> {
    let mut refs = Vec::new();
    for token in raw.split(", ") {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token == "HEAD" {
            refs.push(CommitRef {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
            });
        } else if let Some(branch) = token.strip_prefix("HEAD -> ") {
            refs.push(CommitRef {
                name: "HEAD".to_string(),
                kind: RefKind::Head,
            });
            refs.push(CommitRef {
                name: branch.to_string(),
                kind: classify_branch(branch, remotes),
            });
        } else if let Some(tag) = token.strip_prefix("tag: ") {
            refs.push(CommitRef {
                name: tag.to_string(),
                kind: RefKind::Tag,
            });
        } else if token == "grafted" || token == "replaced" {
            continue;
        } else {
            refs.push(CommitRef {
                name: token.to_string(),
                kind: classify_branch(token, remotes),
            });
        }
    }
    refs
}

/// A branch ref is remote when its first path segment names a configured
/// remote (`origin/master` with an `origin` remote); otherwise local. Local
/// branches may legitimately contain slashes (`feature/foo`), so the remote
/// set — not the mere presence of a slash — is the discriminator.
fn classify_branch(name: &str, remotes: &std::collections::HashSet<String>) -> RefKind {
    if let Some((first, _)) = name.split_once('/') {
        if remotes.contains(first) {
            return RefKind::RemoteBranch;
        }
    }
    RefKind::LocalBranch
}

/// Read up to `limit` commits across all refs (`git log --all`), newest
/// first by author date. Returns an empty vec on any error (git missing,
/// not a repo, command failed) — callers degrade to "no graph".
pub fn commit_log(common_dir: &RepoId, limit: usize) -> Vec<RawCommit> {
    // Field separator is the ASCII unit separator (0x1f); records are
    // newline-separated. `%s` (subject) and `%D` (decorations) never contain
    // newlines, so line-splitting is safe.
    const FMT: &str = "%H\x1f%P\x1f%an\x1f%aI\x1f%D\x1f%s";
    let output = Command::new("git")
        .args([
            "--git-dir",
            &common_dir.0.to_string_lossy(),
            "log",
            "--all",
            "--date-order",
            "-n",
            &limit.to_string(),
            &format!("--pretty=format:{FMT}"),
        ])
        .output();
    let raw = match output {
        Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    let remotes = remotes(common_dir);
    let mut commits = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\x1f');
        let id = fields.next().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let parents = fields
            .next()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let author = fields.next().unwrap_or("").to_string();
        let date = fields.next().unwrap_or("").to_string();
        let decorations = fields.next().unwrap_or("");
        let subject = fields.next().unwrap_or("").to_string();
        commits.push(RawCommit {
            id,
            parents,
            author,
            date,
            subject,
            refs: parse_decorations(decorations, &remotes),
        });
    }
    commits
}

/// List the files a commit changed, with per-file added/removed line counts.
/// Renames are not detected (no `-M`), so a rename surfaces as a delete plus
/// an add — simpler and unambiguous for the detail view. Empty vec on error.
pub fn commit_files(common_dir: &RepoId, sha: &str) -> Vec<CommitFile> {
    let common = common_dir.0.to_string_lossy().to_string();
    // Status (A/M/D) from --name-status, line counts from --numstat. Both
    // emit one line per file in the same order, keyed by path.
    let status = diff_tree_lines(&common, sha, "--name-status");
    let numstat = diff_tree_lines(&common, sha, "--numstat");

    let mut counts: std::collections::HashMap<String, (Option<u32>, Option<u32>)> =
        std::collections::HashMap::new();
    for line in &numstat {
        let mut parts = line.splitn(3, '\t');
        let add = parts.next().unwrap_or("");
        let del = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("").to_string();
        if path.is_empty() {
            continue;
        }
        counts.insert(path, (parse_stat(add), parse_stat(del)));
    }

    let mut files = Vec::new();
    for line in &status {
        let mut parts = line.splitn(2, '\t');
        let status = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        if status.is_empty() || path.is_empty() {
            continue;
        }
        let (additions, deletions) = counts.get(&path).copied().unwrap_or((None, None));
        files.push(CommitFile {
            path,
            status,
            additions,
            deletions,
        });
    }
    files
}

fn diff_tree_lines(common: &str, sha: &str, mode: &str) -> Vec<String> {
    let output = Command::new("git")
        .args([
            "--git-dir",
            common,
            "diff-tree",
            "--no-commit-id",
            "-r",
            mode,
            sha,
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// `git`'s numstat reports `-` for binary files; map that to `None`.
fn parse_stat(value: &str) -> Option<u32> {
    if value == "-" {
        None
    } else {
        value.parse().ok()
    }
}

/// The raw unified diff for one file of a commit. `git show --format=` drops
/// the commit header, leaving only the patch; it handles root commits (no
/// parent) where `diff-tree` would emit nothing. Empty string on error.
pub fn commit_diff(common_dir: &RepoId, sha: &str, path: &str) -> String {
    let output = Command::new("git")
        .args([
            "--git-dir",
            &common_dir.0.to_string_lossy(),
            "show",
            "--format=",
            sha,
            "--",
            path,
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap_or_default(),
        _ => String::new(),
    }
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

    /// Write `content` to `name` under `root` and commit it with `message`.
    fn commit_file(root: &Path, name: &str, content: &str, message: &str) {
        fs::write(root.join(name), content).unwrap();
        git(&["add", name], root);
        git(&["commit", "-m", message], root);
    }

    #[test]
    fn commit_log_returns_history_newest_first() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "a.txt", "one\n", "second");
        commit_file(&root, "b.txt", "two\n", "third");
        let common = git_common_dir(&root).unwrap();

        let log = commit_log(&common, 50);
        assert_eq!(log.len(), 3, "{log:?}");
        assert_eq!(log[0].subject, "third");
        assert_eq!(log[2].subject, "initial");
        // Each non-root commit has exactly one parent, and that parent is the
        // next commit down.
        assert_eq!(log[0].parents, vec![log[1].id.clone()]);
        assert_eq!(log[1].parents, vec![log[2].id.clone()]);
        assert!(log[2].parents.is_empty(), "root commit has no parent");
    }

    #[test]
    fn commit_log_parses_branch_tag_and_head_decorations() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "a.txt", "one\n", "second");
        // Force a lightweight, unsigned tag so a global `tag.gpgSign` config
        // doesn't turn this into an annotated tag that demands a message.
        git(&["-c", "tag.gpgSign=false", "tag", "v1.0"], &root);
        let common = git_common_dir(&root).unwrap();

        let log = commit_log(&common, 50);
        let head = &log[0];
        assert!(
            head.refs
                .iter()
                .any(|r| r.kind == RefKind::Head && r.name == "HEAD"),
            "HEAD decoration: {:?}",
            head.refs
        );
        assert!(
            head.refs
                .iter()
                .any(|r| r.kind == RefKind::LocalBranch && r.name == "main"),
            "local branch main: {:?}",
            head.refs
        );
        assert!(
            head.refs
                .iter()
                .any(|r| r.kind == RefKind::Tag && r.name == "v1.0"),
            "tag v1.0: {:?}",
            head.refs
        );
    }

    #[test]
    fn commit_log_captures_merge_parents() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "base.txt", "base\n", "base");
        git(&["checkout", "-b", "feature"], &root);
        commit_file(&root, "feat.txt", "feat\n", "feature work");
        git(&["checkout", "main"], &root);
        commit_file(&root, "main.txt", "main\n", "main work");
        // Force a merge commit even if fast-forward were possible.
        git(&["merge", "--no-ff", "feature", "-m", "merge feature"], &root);
        let common = git_common_dir(&root).unwrap();

        let log = commit_log(&common, 50);
        let merge = log
            .iter()
            .find(|c| c.subject == "merge feature")
            .expect("merge commit present");
        assert_eq!(merge.parents.len(), 2, "merge has two parents: {merge:?}");
    }

    #[test]
    fn commit_files_lists_changes_with_counts() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "a.txt", "l1\nl2\nl3\n", "add a");
        let common = git_common_dir(&root).unwrap();
        let head = commit_log(&common, 1)[0].id.clone();

        let files = commit_files(&common, &head);
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(files[0].path, "a.txt");
        assert_eq!(files[0].status, "A");
        assert_eq!(files[0].additions, Some(3));
        assert_eq!(files[0].deletions, Some(0));
    }

    #[test]
    fn commit_files_shows_rename_as_delete_plus_add() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "old.txt", "content\n", "add old");
        git(&["mv", "old.txt", "new.txt"], &root);
        git(&["commit", "-m", "rename old to new"], &root);
        let common = git_common_dir(&root).unwrap();
        let head = commit_log(&common, 1)[0].id.clone();

        let files = commit_files(&common, &head);
        let statuses: std::collections::HashMap<&str, &str> = files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(statuses.get("old.txt"), Some(&"D"), "{files:?}");
        assert_eq!(statuses.get("new.txt"), Some(&"A"), "{files:?}");
    }

    #[test]
    fn commit_diff_returns_unified_diff_for_path() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "a.txt", "hello world\n", "add a");
        let common = git_common_dir(&root).unwrap();
        let head = commit_log(&common, 1)[0].id.clone();

        let diff = commit_diff(&common, &head, "a.txt");
        assert!(diff.contains("+hello world"), "diff body: {diff}");
        assert!(diff.contains("a.txt"), "diff names the file: {diff}");
    }

    #[test]
    fn commit_log_empty_outside_repo() {
        let tmp = TempDir::new().unwrap();
        let bogus = RepoId(tmp.path().join("nope/.git"));
        assert!(commit_log(&bogus, 50).is_empty());
    }
}
