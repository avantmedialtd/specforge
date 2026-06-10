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

/// A single git trailer — a `Key: value` line from the last paragraph of a
/// commit message, as recognised by git's own trailer parser (`%(trailers)`).
/// Captured verbatim and rendered as neutral metadata; no key is special-cased.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    pub key: String,
    pub value: String,
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
    /// Git trailers from the message's last paragraph, in git's order.
    pub trailers: Vec<Trailer>,
}

/// One commit with its parents, ref decorations, and **full** author identity,
/// for the commit garden. Like [`RawCommit`] but carries the author email
/// (`%ae`) so a node can be attributed to a person; it keeps refs (so the
/// garden's graph can show branch/tag/HEAD labels) and drops trailers. Not an
/// IPC type — the garden derivation consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredCommit {
    pub id: String,
    pub parents: Vec<String>,
    pub author: crate::identity::Author,
    /// Author date, ISO-8601 / strict (`%aI`).
    pub date: String,
    pub subject: String,
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

/// How a git invocation is anchored: in a working directory (`current_dir`) or
/// against a bare git dir (`--git-dir`). Centralising the two shapes is what
/// lets one place route WSL-hosted repositories through `wsl.exe` to the
/// distribution's native git.
#[derive(Clone, Copy)]
enum GitAnchor<'a> {
    Cwd(&'a Path),
    GitDir(&'a Path),
}

/// The packaged app is a GUI-subsystem process with no console, so a
/// console-subsystem child (`git.exe`, `wsl.exe`) would otherwise be given a
/// fresh console whose window flashes on screen for every invocation.
/// `CREATE_NO_WINDOW` gives the child an invisible console instead — console
/// APIs still work, and stdio is piped by the `.output()` call sites anyway.
#[cfg(target_os = "windows")]
fn suppress_console_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// Build a `git` command for `args`, anchored per `anchor`. For a WSL-hosted
/// anchor path (Windows only) the command runs the distribution's native git
/// via `wsl.exe`, with the anchor path translated to its Linux form; otherwise
/// it is a plain `git` invocation. Path arguments inside `args` are assumed to
/// be repo-relative (`openspec/changes`, `<sha>:<path>`, refs) — never absolute
/// Windows paths — so they need no translation; the only path that crosses the
/// boundary is the anchor.
fn git_command(anchor: GitAnchor, args: &[&str]) -> Command {
    #[cfg(target_os = "windows")]
    {
        let anchor_path = match anchor {
            GitAnchor::Cwd(p) | GitAnchor::GitDir(p) => p,
        };
        if let Some(wsl) = crate::wsl::parse_wsl_path(anchor_path) {
            let wsl_anchor = match anchor {
                GitAnchor::Cwd(_) => crate::wsl::WslGitAnchor::Cwd(&wsl.linux_path),
                GitAnchor::GitDir(_) => crate::wsl::WslGitAnchor::GitDir(&wsl.linux_path),
            };
            let mut cmd = Command::new("wsl.exe");
            cmd.args(crate::wsl::wsl_git_command_args(
                &wsl.distro,
                wsl_anchor,
                args,
            ));
            suppress_console_window(&mut cmd);
            return cmd;
        }
    }
    let mut cmd = Command::new("git");
    match anchor {
        GitAnchor::Cwd(p) => {
            cmd.current_dir(p);
        }
        GitAnchor::GitDir(p) => {
            cmd.arg("--git-dir").arg(p);
        }
    }
    cmd.args(args);
    #[cfg(target_os = "windows")]
    suppress_console_window(&mut cmd);
    cmd
}

/// Detect the git common directory that owns `path`. Returns `None` when
/// `path` is not inside a git repository or when `git` is missing on PATH.
pub fn git_common_dir(path: &Path) -> Option<RepoId> {
    let output = git_command(GitAnchor::Cwd(path), &["rev-parse", "--git-common-dir"])
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
    crate::paths::canonicalize(&absolute).ok().map(RepoId)
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
    let output = git_command(GitAnchor::Cwd(worktree_path), &["branch", "--show-current"])
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

/// The local git identity (`user.name` / `user.email`) in effect at `path`,
/// read with git's normal repository-local → global cascade. Returns `None`
/// when `git` is missing or neither value is configured; an [`Author`]
/// (`crate::identity::Author`) with only one component when only one is set.
/// This is the identity the watcher stamps on live achievements for a workspace.
pub fn git_identity(path: &Path) -> Option<crate::identity::Author> {
    let name = git_config_value(path, "user.name");
    let email = git_config_value(path, "user.email");
    if name.is_none() && email.is_none() {
        return None;
    }
    Some(crate::identity::Author { name, email })
}

/// Read a single git config value as seen from `path` (repo-local with global
/// fallback, git's default). `None` on any error or an empty value.
fn git_config_value(path: &Path, key: &str) -> Option<String> {
    let output = git_command(GitAnchor::Cwd(path), &["config", "--get", key])
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
    let output = match git_command(
        GitAnchor::GitDir(&common_dir.0),
        &["worktree", "list", "--porcelain"],
    )
    .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let raw = match String::from_utf8(output.stdout) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    // A WSL repo's git reports Linux worktree paths; translate them to the
    // Windows UNC form so the registry stores the same shape it does for the
    // user-registered workspace. `None` elsewhere keeps native canonicalisation.
    #[cfg(target_os = "windows")]
    let wsl_distro = crate::wsl::parse_wsl_path(&common_dir.0).map(|w| w.distro);
    #[cfg(not(target_os = "windows"))]
    let wsl_distro: Option<String> = None;
    parse_worktree_porcelain(&raw, wsl_distro.as_deref())
}

fn parse_worktree_porcelain(text: &str, wsl_distro: Option<&str>) -> Vec<WorktreeInfo> {
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
        let resolved = match wsl_distro {
            // WSL: git reports Linux paths; translate to the Windows UNC form.
            // The registry canonicalises the result (over 9P) the same way it
            // does the user-registered workspace, keeping one stable RepoId.
            Some(distro) => crate::wsl::wsl_to_unc(distro, &path.to_string_lossy()),
            // For prunable worktrees the path is missing on disk so canonicalize
            // fails — fall back to the literal path so we can still identify it
            // for removal.
            None => crate::paths::canonicalize(&path).unwrap_or(path),
        };
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
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &["symbolic-ref", "--short", ref_name],
    )
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
    let output = git_command(GitAnchor::GitDir(&common_dir.0), &["config", "--get", key])
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
    let output = git_command(GitAnchor::GitDir(&common_dir.0), &["remote"]).output();
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
    // Records are NUL-separated (`-z`) so a multi-line trailer value can never
    // be mistaken for a record boundary; fields within a record use the ASCII
    // unit separator (0x1f). The trailers field is itself packed: trailers are
    // joined by the record separator (0x1e) and each trailer's key/value by the
    // group separator (0x1d). All three are distinct C0 control bytes that
    // cannot occur in a hash, name, date, subject, decoration, or trailer text,
    // so splitting is unambiguous at every level and needs no escaping.
    const FMT: &str = "%H\x1f%P\x1f%an\x1f%aI\x1f%D\x1f%s\x1f%(trailers:only,unfold,key_value_separator=%x1d,separator=%x1e)";
    let limit_arg = limit.to_string();
    let pretty = format!("--pretty=format:{FMT}");
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &[
            "log",
            "--all",
            "--date-order",
            "-z",
            "-n",
            &limit_arg,
            &pretty,
        ],
    )
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
    for record in raw.split('\0') {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split('\x1f');
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
        let trailers = parse_trailers(fields.next().unwrap_or(""));
        commits.push(RawCommit {
            id,
            parents,
            author,
            date,
            subject,
            refs: parse_decorations(decorations, &remotes),
            trailers,
        });
    }
    commits
}

/// Read up to `limit` commits across all refs (`git log --all`), newest first
/// by author date, with parents and the full author identity (`%an`/`%ae`) —
/// the garden's source. Records are NUL-separated (`-z`); fields use the ASCII
/// unit separator (0x1f). Returns an empty vec on any error so the garden
/// degrades to a dormant plant.
pub fn commit_log_authored(common_dir: &RepoId, limit: usize) -> Vec<AuthoredCommit> {
    const FMT: &str = "%H\x1f%P\x1f%an\x1f%ae\x1f%aI\x1f%D\x1f%s";
    let limit_arg = limit.to_string();
    let pretty = format!("--pretty=format:{FMT}");
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &[
            "log",
            "--all",
            "--date-order",
            "-z",
            "-n",
            &limit_arg,
            &pretty,
        ],
    )
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
    for record in raw.split('\0') {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.split('\x1f');
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
        let name = fields.next().map(str::to_string).filter(|s| !s.is_empty());
        let email = fields.next().map(str::to_string).filter(|s| !s.is_empty());
        let date = fields.next().unwrap_or("").to_string();
        let decorations = fields.next().unwrap_or("");
        let subject = fields.next().unwrap_or("").to_string();
        commits.push(AuthoredCommit {
            id,
            parents,
            author: crate::identity::Author { name, email },
            date,
            subject,
            refs: parse_decorations(decorations, &remotes),
        });
    }
    commits
}

/// Parse the packed trailers field produced by
/// `%(trailers:…,key_value_separator=%x1d,separator=%x1e)`: trailers are split
/// on the record separator (0x1e) and each trailer's key/value on the group
/// separator (0x1d). Order is preserved and repeated keys are kept. A malformed
/// entry lacking the key/value separator is skipped; key and value are trimmed.
fn parse_trailers(raw: &str) -> Vec<Trailer> {
    raw.split('\x1e')
        .filter(|t| !t.is_empty())
        .filter_map(|t| {
            t.split_once('\x1d').map(|(key, value)| Trailer {
                key: key.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

/// List the files a commit changed, with per-file added/removed line counts.
/// Renames are not detected (no `-M`), so a rename surfaces as a delete plus
/// an add — simpler and unambiguous for the detail view. Empty vec on error.
pub fn commit_files(common_dir: &RepoId, sha: &str) -> Vec<CommitFile> {
    // Status (A/M/D) from --name-status, line counts from --numstat. Both
    // emit one line per file in the same order, keyed by path.
    let status = diff_tree_lines(common_dir, sha, "--name-status");
    let numstat = diff_tree_lines(common_dir, sha, "--numstat");

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

fn diff_tree_lines(common_dir: &RepoId, sha: &str, mode: &str) -> Vec<String> {
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &["diff-tree", "--no-commit-id", "-r", mode, sha],
    )
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
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &["show", "--format=", sha, "--", path],
    )
    .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap_or_default(),
        _ => String::new(),
    }
}

/// One change directory's lifecycle dates, recovered from git history: the
/// author date of the earliest commit that ADDED a file under
/// `openspec/changes/<id>/` (its creation) and under
/// `openspec/changes/archive/<id>/` (its archival). Dates are Unix epoch
/// seconds (`%at`); either is `None` when the corresponding add-event is not
/// recoverable from history (created but never committed, or moved into the
/// archive in a way history doesn't record as an add under the archive path).
/// `created_by` / `archived_by` are the authors of those earliest commits
/// (`%an`/`%ae`), carried through so backfilled achievements are attributed to
/// whoever performed the work; `None` when the date is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeLifecycle {
    pub change_name: String,
    pub created_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub created_by: Option<crate::identity::Author>,
    pub archived_by: Option<crate::identity::Author>,
}

/// Recover every change's creation/archival dates in a SINGLE pass over
/// `openspec/changes/`. One `git log --diff-filter=A --name-status` (with
/// rename detection off, so an archive move surfaces as an Add under the
/// archive path rather than a rename) yields every add-event; the added paths
/// are folded into per-change earliest timestamps. O(repos), not O(changes).
/// Empty vec on any error, matching the other functions here.
pub fn change_lifecycle(common_dir: &RepoId) -> Vec<ChangeLifecycle> {
    // Record separator prefixes each commit header line so the `%at` is
    // unambiguous against the following `A\t<path>` name-status rows. The
    // header also carries the commit author (`%an`/`%ae`), unit-separated, so
    // each add-event can be attributed.
    const RS: char = '\u{1e}';
    const US: char = '\u{1f}';
    let pretty = format!("--pretty=format:{RS}%at{US}%an{US}%ae");
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &[
            "log",
            "--all",
            "--reverse",
            "--no-renames",
            "--diff-filter=A",
            "--name-status",
            &pretty,
            "--",
            "openspec/changes",
        ],
    )
    .output();
    let raw = match output {
        Ok(o) if o.status.success() => match String::from_utf8(o.stdout) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    use std::collections::{BTreeSet, HashMap};
    let mut created: HashMap<String, (i64, crate::identity::Author)> = HashMap::new();
    let mut archived: HashMap<String, (i64, crate::identity::Author)> = HashMap::new();
    let mut current: Option<(i64, crate::identity::Author)> = None;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix(RS) {
            // Header: `<at>US<name>US<email>`.
            let mut fields = rest.split(US);
            current = match fields.next().and_then(|s| s.trim().parse::<i64>().ok()) {
                Some(at) => {
                    let name = fields.next().map(str::to_string).filter(|s| !s.is_empty());
                    let email = fields.next().map(str::to_string).filter(|s| !s.is_empty());
                    Some((at, crate::identity::Author { name, email }))
                }
                None => None,
            };
            continue;
        }
        let Some((at, author)) = current.as_ref() else {
            continue;
        };
        // name-status row: "A\t<path>".
        let Some((_status, path)) = line.split_once('\t') else {
            continue;
        };
        // `--reverse` walks oldest→newest, so the first time a path is seen is
        // its earliest add; `or_insert` keeps that earliest timestamp + author.
        if let Some(name) = archive_change_name(path) {
            archived
                .entry(name)
                .or_insert_with(|| (*at, author.clone()));
        } else if let Some(name) = active_change_name(path) {
            created.entry(name).or_insert_with(|| (*at, author.clone()));
        }
    }

    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(created.keys().cloned());
    names.extend(archived.keys().cloned());
    names
        .into_iter()
        .map(|name| ChangeLifecycle {
            created_at: created.get(&name).map(|(at, _)| *at),
            archived_at: archived.get(&name).map(|(at, _)| *at),
            created_by: created.get(&name).map(|(_, a)| a.clone()),
            archived_by: archived.get(&name).map(|(_, a)| a.clone()),
            change_name: name,
        })
        .collect()
}

/// `openspec/changes/archive/<id>/…` → `Some("<id>")`.
fn archive_change_name(path: &str) -> Option<String> {
    let rest = path.strip_prefix("openspec/changes/archive/")?;
    let name = rest.split('/').next().unwrap_or("");
    (!name.is_empty()).then(|| name.to_string())
}

/// `openspec/changes/<id>/…`, excluding the archive subtree → `Some("<id>")`.
fn active_change_name(path: &str) -> Option<String> {
    let rest = path.strip_prefix("openspec/changes/")?;
    if rest.starts_with("archive/") {
        return None;
    }
    let name = rest.split('/').next().unwrap_or("");
    (!name.is_empty()).then(|| name.to_string())
}

/// Author dates (ISO-8601, `%aI`) of commits across all refs more recent than
/// `since` (a git approxidate string such as `"14 days ago"`). Bounded by
/// `--since` so it never scans the full history. Empty vec on any error.
pub fn commit_activity(common_dir: &RepoId, since: &str) -> Vec<String> {
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &["log", "--all", "--since", since, "--pretty=format:%aI"],
    )
    .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// One `(author-date, author)` pair per commit across all refs more recent than
/// `since`. The ISO-8601 date (`%aI`) drives the scoped heatmap/streak commit
/// days; the [`Author`](crate::identity::Author) (`%an`/`%ae`) drives the
/// per-author leaderboard. Bounded by `--since`. Empty vec on any error. Fields
/// are unit-separated so author names containing spaces parse unambiguously.
pub fn commit_activity_with_authors(
    common_dir: &RepoId,
    since: &str,
) -> Vec<(String, crate::identity::Author)> {
    const US: char = '\u{1f}';
    let output = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &[
            "log",
            "--all",
            "--since",
            since,
            "--pretty=format:%aI\u{1f}%an\u{1f}%ae",
        ],
    )
    .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut parts = line.split(US);
                let date = parts.next().unwrap_or("").trim().to_string();
                if date.is_empty() {
                    return None;
                }
                let name = parts.next().map(str::to_string).filter(|s| !s.is_empty());
                let email = parts.next().map(str::to_string).filter(|s| !s.is_empty());
                Some((date, crate::identity::Author { name, email }))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Backfill source for task completions. Walks the bounded window of commits
/// (across all refs, oldest first) that touched files under
/// `openspec/changes/`, and reports each positive increase in a change's
/// completed-checkbox count as a `(timestamp, change_id, delta, author)` tuple
/// in chronological order. The author (`%an`/`%ae`) attributes the completion
/// to whoever committed it. Archive-path `tasks.md` files are ignored (the
/// change's completions were already counted on its active path). Bounded by
/// `since`; decreases (unchecks / deletions) yield nothing. Empty vec on any
/// git error.
pub fn task_completion_history(
    common_dir: &RepoId,
    since: &str,
) -> Vec<(i64, String, u32, Option<crate::identity::Author>)> {
    const US: char = '\u{1f}';
    // Commits within the window that touched openspec/changes, oldest first.
    // Fields are unit-separated so the author name (which may contain spaces)
    // parses unambiguously.
    let log = git_command(
        GitAnchor::GitDir(&common_dir.0),
        &[
            "log",
            "--all",
            "--reverse",
            "--since",
            since,
            "--format=%H\u{1f}%at\u{1f}%an\u{1f}%ae",
            "--",
            "openspec/changes",
        ],
    )
    .output();
    let log_raw = match log {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap_or_default(),
        _ => return Vec::new(),
    };

    // Last-seen completed count per active tasks.md path, so we emit only the
    // positive deltas as the file evolves across commits.
    let mut last_completed: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut out: Vec<(i64, String, u32, Option<crate::identity::Author>)> = Vec::new();

    for line in log_raw.lines() {
        let mut parts = line.split(US);
        let sha = match parts.next() {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        let ts: i64 = match parts.next().and_then(|t| t.trim().parse().ok()) {
            Some(t) => t,
            None => continue,
        };
        let author = {
            let name = parts.next().map(str::to_string).filter(|s| !s.is_empty());
            let email = parts.next().map(str::to_string).filter(|s| !s.is_empty());
            if name.is_none() && email.is_none() {
                None
            } else {
                Some(crate::identity::Author { name, email })
            }
        };

        // Paths this commit changed (no diff body).
        let names = git_command(
            GitAnchor::GitDir(&common_dir.0),
            &["show", "--name-only", "--pretty=format:", sha],
        )
        .output();
        let names_raw = match names {
            Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap_or_default(),
            _ => continue,
        };

        for path in names_raw.lines().map(str::trim).filter(|p| !p.is_empty()) {
            let Some(change_id) = active_change_id_of_tasks_path(path) else {
                continue;
            };
            // Read the blob at this commit and count completed checkboxes.
            let blob_ref = format!("{sha}:{path}");
            let show = git_command(GitAnchor::GitDir(&common_dir.0), &["show", &blob_ref]).output();
            let completed = match show {
                Ok(o) if o.status.success() => {
                    crate::parser::count_completed_in_text(&String::from_utf8_lossy(&o.stdout))
                }
                // File deleted at this commit: treat as zero, but never emit a
                // negative — just reset the baseline.
                _ => 0,
            };
            let prev = last_completed.get(path).copied().unwrap_or(0);
            if completed > prev {
                out.push((ts, change_id, (completed - prev) as u32, author.clone()));
            }
            last_completed.insert(path.to_string(), completed);
        }
    }
    out
}

/// If `path` is an active (non-archive) `openspec/changes/<id>/tasks.md`,
/// return `<id>`. Archive paths and anything else return `None`.
fn active_change_id_of_tasks_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("openspec/changes/")?;
    if !rest.ends_with("/tasks.md") {
        return None;
    }
    let id = rest.split('/').next()?;
    if id.is_empty() || id == "archive" {
        return None;
    }
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn worktree_porcelain_native_paths_pass_through() {
        // Without a WSL distro, paths are taken as-is (canonicalize falls back
        // to the literal path when it doesn't exist on disk).
        let text = "worktree /tmp/does-not-exist-xyz\nbranch refs/heads/main\n";
        let infos = parse_worktree_porcelain(text, None);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].path, PathBuf::from("/tmp/does-not-exist-xyz"));
        assert_eq!(infos[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn worktree_porcelain_wsl_paths_translate_to_unc() {
        // With a distro, git's Linux worktree paths become Windows UNC paths.
        let text = "worktree /home/dev/proj\nbranch refs/heads/main\n\n\
                    worktree /home/dev/proj-feature\nbranch refs/heads/feature\n";
        let infos = parse_worktree_porcelain(text, Some("Ubuntu"));
        assert_eq!(infos.len(), 2);
        assert_eq!(
            infos[0].path,
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\dev\proj")
        );
        assert!(infos[0].is_main);
        assert_eq!(
            infos[1].path,
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\dev\proj-feature")
        );
        assert_eq!(infos[1].branch.as_deref(), Some("feature"));
    }

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
        git(
            &["merge", "--no-ff", "feature", "-m", "merge feature"],
            &root,
        );
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

    fn trailer(key: &str, value: &str) -> Trailer {
        Trailer {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn commit_log_captures_trailers_in_order() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(
            &root,
            "a.txt",
            "x\n",
            "Add a feature\n\nOpenSpec-Id: add-feature\nCo-Authored-By: Bot <bot@example.com>",
        );
        let common = git_common_dir(&root).unwrap();

        let head = &commit_log(&common, 1)[0];
        assert_eq!(
            head.trailers,
            vec![
                trailer("OpenSpec-Id", "add-feature"),
                trailer("Co-Authored-By", "Bot <bot@example.com>"),
            ],
            "trailers captured in git's order: {:?}",
            head.trailers
        );
    }

    #[test]
    fn commit_log_does_not_treat_body_prose_as_trailers() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        // A multi-paragraph body whose prose contains a colon line and bullets;
        // only the final paragraph is a real trailer block.
        commit_file(
            &root,
            "a.txt",
            "x\n",
            "Bump deps\n\nUpgraded several actions:\n- foo v1 -> v2\n- bar v3 -> v4\n\nOpenSpec-Id: bump-deps",
        );
        let common = git_common_dir(&root).unwrap();

        let head = &commit_log(&common, 1)[0];
        assert_eq!(
            head.trailers,
            vec![trailer("OpenSpec-Id", "bump-deps")],
            "only the last-paragraph trailer is captured, not body prose: {:?}",
            head.trailers
        );
    }

    #[test]
    fn commit_log_keeps_repeated_trailer_keys() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(
            &root,
            "a.txt",
            "x\n",
            "Pair work\n\nCo-Authored-By: A <a@example.com>\nCo-Authored-By: B <b@example.com>",
        );
        let common = git_common_dir(&root).unwrap();

        let head = &commit_log(&common, 1)[0];
        assert_eq!(
            head.trailers,
            vec![
                trailer("Co-Authored-By", "A <a@example.com>"),
                trailer("Co-Authored-By", "B <b@example.com>"),
            ],
            "both occurrences kept, not collapsed: {:?}",
            head.trailers
        );
    }

    #[test]
    fn commit_log_has_no_trailers_when_message_has_none() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        commit_file(&root, "a.txt", "x\n", "Just a subject");
        let common = git_common_dir(&root).unwrap();

        let head = &commit_log(&common, 1)[0];
        assert!(
            head.trailers.is_empty(),
            "no trailers expected: {:?}",
            head.trailers
        );
    }

    /// Commit the staged tree with a fixed author + committer date so
    /// time-window assertions are deterministic.
    fn commit_with_date(root: &Path, message: &str, date: &str) {
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .current_dir(root)
            .output()
            .expect("git commit");
        assert!(
            output.status.success(),
            "commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn change_lifecycle_recovers_create_and_archive_dates() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        // Create change "foo".
        fs::create_dir_all(root.join("openspec/changes/foo")).unwrap();
        fs::write(root.join("openspec/changes/foo/proposal.md"), "x\n").unwrap();
        git(&["add", "."], &root);
        commit_with_date(&root, "create foo", "2026-01-01T12:00:00");
        // Archive it: move the directory under archive/.
        fs::create_dir_all(root.join("openspec/changes/archive")).unwrap();
        git(
            &["mv", "openspec/changes/foo", "openspec/changes/archive/foo"],
            &root,
        );
        commit_with_date(&root, "archive foo", "2026-01-04T12:00:00");

        let common = git_common_dir(&root).unwrap();
        let lifecycles = change_lifecycle(&common);
        let foo = lifecycles
            .iter()
            .find(|l| l.change_name == "foo")
            .expect("foo lifecycle present");
        assert!(foo.created_at.is_some(), "created date: {foo:?}");
        assert!(foo.archived_at.is_some(), "archived date: {foo:?}");
        assert!(
            foo.archived_at.unwrap() > foo.created_at.unwrap(),
            "archive must be later than creation: {foo:?}"
        );
    }

    #[test]
    fn change_lifecycle_create_without_archive_has_no_archive_date() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        fs::create_dir_all(root.join("openspec/changes/bar")).unwrap();
        fs::write(root.join("openspec/changes/bar/proposal.md"), "x\n").unwrap();
        git(&["add", "."], &root);
        commit_with_date(&root, "create bar", "2026-02-01T12:00:00");

        let common = git_common_dir(&root).unwrap();
        let lifecycles = change_lifecycle(&common);
        let bar = lifecycles
            .iter()
            .find(|l| l.change_name == "bar")
            .expect("bar lifecycle present");
        assert!(bar.created_at.is_some());
        assert_eq!(bar.archived_at, None);
    }

    #[test]
    fn change_lifecycle_empty_outside_repo() {
        let tmp = TempDir::new().unwrap();
        let bogus = RepoId(tmp.path().join("nope/.git"));
        assert!(change_lifecycle(&bogus).is_empty());
    }

    #[test]
    fn commit_activity_respects_since_window() {
        // `git log --since` prunes traversal once it hits a commit older than
        // the cutoff (it assumes parents are older), so the OLD commit must be
        // the ancestor and the RECENT commit its descendant — otherwise the
        // recent ancestor would be pruned behind the old child.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        fs::create_dir_all(&root).unwrap();
        git(&["init", "-b", "main"], &root);
        git(&["config", "user.email", "test@example.com"], &root);
        git(&["config", "user.name", "Test"], &root);
        // Ancestor commit dated well outside any small window.
        let old = Command::new("git")
            .args(["commit", "--allow-empty", "-m", "old"])
            .env("GIT_AUTHOR_DATE", "2024-01-01T00:00:00")
            .env("GIT_COMMITTER_DATE", "2024-01-01T00:00:00")
            .current_dir(&root)
            .output()
            .expect("git commit");
        assert!(old.status.success());
        // Recent descendant (default date ~now).
        fs::write(root.join("recent.txt"), "r\n").unwrap();
        git(&["add", "."], &root);
        git(&["commit", "-m", "recent"], &root);

        let common = git_common_dir(&root).unwrap();
        let recent = commit_activity(&common, "30 days ago");
        // The 2024 ancestor is outside the 30-day window.
        assert!(
            recent.iter().all(|d| !d.starts_with("2024")),
            "2024 commit must be excluded by --since: {recent:?}"
        );
        // The recent descendant is inside the window.
        assert!(!recent.is_empty(), "recent window should include 'recent'");
    }

    #[test]
    fn commit_activity_empty_outside_repo() {
        let tmp = TempDir::new().unwrap();
        let bogus = RepoId(tmp.path().join("nope/.git"));
        assert!(commit_activity(&bogus, "30 days ago").is_empty());
    }

    #[test]
    fn task_completion_history_reports_positive_deltas_only() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let tasks_dir = root.join("openspec/changes/foo");
        fs::create_dir_all(&tasks_dir).unwrap();

        // Commit 1: 1 of 3 tasks done.
        fs::write(
            tasks_dir.join("tasks.md"),
            "## A\n- [x] one\n- [ ] two\n- [ ] three\n",
        )
        .unwrap();
        git(&["add", "."], &root);
        git(&["commit", "-m", "foo tasks: 1 done"], &root);

        // Commit 2: 3 of 3 done (+2).
        fs::write(
            tasks_dir.join("tasks.md"),
            "## A\n- [x] one\n- [x] two\n- [x] three\n",
        )
        .unwrap();
        git(&["add", "."], &root);
        git(&["commit", "-m", "foo tasks: all done"], &root);

        // Commit 3: an unchecked regression (-1) must emit nothing.
        fs::write(
            tasks_dir.join("tasks.md"),
            "## A\n- [x] one\n- [x] two\n- [ ] three\n",
        )
        .unwrap();
        git(&["add", "."], &root);
        git(&["commit", "-m", "foo tasks: regressed"], &root);

        let common = git_common_dir(&root).unwrap();
        let history = task_completion_history(&common, "30 days ago");
        let deltas: Vec<u32> = history
            .iter()
            .filter(|(_, id, _, _)| id == "foo")
            .map(|(_, _, d, _)| *d)
            .collect();
        // +1 (first appearance), then +2; the -1 regression is dropped.
        assert_eq!(deltas, vec![1, 2], "history: {history:?}");
    }

    #[test]
    fn task_completion_history_empty_outside_repo() {
        let tmp = TempDir::new().unwrap();
        let bogus = RepoId(tmp.path().join("nope/.git"));
        assert!(task_completion_history(&bogus, "30 days ago").is_empty());
    }

    #[test]
    fn git_identity_reads_repo_local_name_and_email() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        let id = git_identity(&root).expect("identity from configured repo");
        assert_eq!(id.name.as_deref(), Some("Test"));
        assert_eq!(id.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn change_lifecycle_captures_the_commit_author() {
        let tmp = TempDir::new().unwrap();
        let root = init_repo(tmp.path());
        fs::create_dir_all(root.join("openspec/changes/foo")).unwrap();
        fs::write(root.join("openspec/changes/foo/proposal.md"), "x\n").unwrap();
        git(&["add", "."], &root);
        commit_with_date(&root, "create foo", "2026-01-01T12:00:00");

        let common = git_common_dir(&root).unwrap();
        let lifecycles = change_lifecycle(&common);
        let foo = lifecycles
            .iter()
            .find(|l| l.change_name == "foo")
            .expect("foo lifecycle");
        let created_by = foo.created_by.as_ref().expect("created_by author");
        assert_eq!(created_by.email.as_deref(), Some("test@example.com"));
        assert_eq!(created_by.name.as_deref(), Some("Test"));
    }
}
