use crate::git::{self, RepoId};
use crate::types::{RegisteredWorkspace, WorkspaceFolder};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

/// Identity/dedup key for a stored workspace path. Uses the canonicalised path
/// when it resolves (collapsing case and symlink differences for folders that
/// exist), falling back to the stored path when the folder is missing — a
/// supported state. `PathBuf` equality already normalises trailing separators,
/// repeated separators, and `.` components, so two spellings of a *missing*
/// folder still collapse onto one key.
fn dedup_key(uri: &Path) -> PathBuf {
    crate::paths::canonicalize(uri).unwrap_or_else(|_| uri.to_path_buf())
}

/// Whether a root `git worktree list` names is a working tree at all. Every
/// working tree holds a `.git` directory or file; the entries that do not are
/// a bare repository's own directory and the git store that a
/// `--separate-git-dir` repository or a submodule lists as its main worktree.
fn is_working_tree(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Whether a working tree is too broad to track without OpenSpec of its own:
/// the user's `home` directory (a dotfiles repository) or a filesystem root.
/// Tracking one would watch, `git status` and list the whole of it — what a
/// mistaken pick of `~/Downloads` inside a dotfiles home would otherwise do.
fn too_broad_without_openspec(root: &Path, home: Option<&Path>) -> bool {
    root.parent().is_none()
        || home.is_some_and(|home| crate::paths::canonicalize(home).is_ok_and(|h| h == root))
}

/// The user's home directory, as the platform names it.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error(
        "neither an OpenSpec workspace nor a git repository (no `openspec/` subdirectory, and not inside a git working tree): {0}"
    )]
    NotOpenSpecOrGit(PathBuf),
    #[error(
        "not registering {0}: a git repository without OpenSpec at your home directory or a filesystem root is too broad to track; pick the repository's own folder, or one with an `openspec/` subdirectory"
    )]
    TooBroad(PathBuf),
    #[error("workspace already registered: {0}")]
    AlreadyRegistered(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// How a workspace ended up in the registry. Only [`WorkspaceOrigin::UserRegistered`]
/// entries persist to disk; [`WorkspaceOrigin::Discovered`] entries are re-derived
/// from `git worktree list` at startup and as worktrees change at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceOrigin {
    UserRegistered,
    Discovered { discovered_via: RepoId },
}

/// A registered workspace plus its origin and git-repo association.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub folder: WorkspaceFolder,
    pub origin: WorkspaceOrigin,
    pub repo_id: Option<RepoId>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    workspaces: Vec<WorkspaceFolder>,
}

/// In-memory store of registered workspaces with disk persistence backed by
/// a JSON file at `config_path`. Distinguishes user-registered workspaces
/// (persisted, shown in Settings, manually managed) from discovered worktrees
/// of user-registered git repositories (transient, recomputed at startup).
#[derive(Debug)]
pub struct WorkspaceRegistry {
    config_path: PathBuf,
    /// Insertion-ordered so the registry preserves the config order of
    /// `workspaces.json`: loading keeps the file's order, registering appends,
    /// and saving writes that same order back. `IndexMap` gives `HashMap`-like
    /// lookup with deterministic iteration; removals use `shift_remove` to keep
    /// the order intact.
    entries: IndexMap<PathBuf, RegistryEntry>,
}

impl WorkspaceRegistry {
    /// Creates an empty registry tied to `config_path` without reading it.
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            entries: IndexMap::new(),
        }
    }

    /// Loads the registry from `config_path`. A missing file is treated as
    /// an empty registry; a corrupt file is reported as `InvalidData`. After
    /// loading the user-registered entries, the discovered set is re-derived
    /// by scanning each detected repository's worktrees.
    /// Loading NEVER writes to disk: a missing or empty file yields an empty
    /// registry and the file is left untouched; a corrupt file fails closed
    /// (`InvalidData`) and is never overwritten. The file's array order is
    /// preserved verbatim — no re-sorting — and duplicate spellings of the same
    /// path are collapsed first-wins (keeping the earliest occurrence's slot)
    /// without ever dropping a workspace to zero.
    pub fn load(config_path: PathBuf) -> io::Result<Self> {
        let mut entries: IndexMap<PathBuf, RegistryEntry> = IndexMap::new();
        if config_path.exists() {
            let raw = fs::read_to_string(&config_path)?;
            let config: ConfigFile = serde_json::from_str(&raw)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for folder in config.workspaces {
                let key = dedup_key(&folder.uri);
                // First-wins dedup: keep the earliest occurrence, drop later
                // duplicate spellings of the same path.
                if entries.contains_key(&key) {
                    continue;
                }
                let repo_id = git::git_common_dir(&key);
                // Adopt the canonical (or fallback) form as the entry's uri so
                // it matches the lookup key used by `register`/`unregister`.
                let folder = WorkspaceFolder {
                    uri: key.clone(),
                    name: folder.name,
                };
                entries.insert(
                    key,
                    RegistryEntry {
                        folder,
                        origin: WorkspaceOrigin::UserRegistered,
                        repo_id,
                    },
                );
            }
        }
        let mut s = Self {
            config_path,
            entries,
        };
        s.derive_all_discovered();
        Ok(s)
    }

    /// Validates `path` and adds it to the registry. A folder holding an
    /// `openspec/` subdirectory is registered as selected; a folder without
    /// one is accepted only inside a git working tree, and that working
    /// tree's root is registered in its place. If the registered folder is
    /// inside a git repository, every other worktree of the repository is also
    /// discovered and added with `WorkspaceOrigin::Discovered`. Returns the
    /// list of `WorkspaceFolder`s newly added — the user-registered entry
    /// plus any discovered siblings the caller needs to start watching.
    /// Persists to disk on success (discovered entries are never persisted).
    pub fn register(&mut self, path: PathBuf) -> Result<Vec<WorkspaceFolder>, RegistrationError> {
        self.register_resolved(path).map(|(_, added)| added)
    }

    /// [`register`](Self::register), also naming the folder that ended up
    /// user-registered: the selected path or the worktree root it resolved to,
    /// whether newly added or promoted from `Discovered`. A promoted folder is
    /// already tracked, so it is *not* in the returned `added` list — which is
    /// why a caller cannot take `added.first()` as the registered folder.
    pub fn register_resolved(
        &mut self,
        path: PathBuf,
    ) -> Result<(WorkspaceFolder, Vec<WorkspaceFolder>), RegistrationError> {
        if !path.exists() {
            return Err(RegistrationError::PathNotFound(path));
        }
        let canonical = crate::paths::canonicalize(&path)?;
        if !canonical.is_dir() {
            return Err(RegistrationError::NotADirectory(canonical));
        }

        let repo_id = git::git_common_dir(&canonical);
        // A subfolder is never registered without OpenSpec of its own: the
        // file browser lists each tracked folder relative to itself, so a
        // subfolder beside its repository's worktree roots would put one file
        // under two different paths. Git names the root itself, which is what
        // keeps a submodule or a `--separate-git-dir` work tree correct.
        let canonical = if canonical.join("openspec").is_dir() {
            canonical
        } else {
            let root = repo_id
                .as_ref()
                .and_then(|_| git::worktree_toplevel(&canonical))
                .ok_or_else(|| RegistrationError::NotOpenSpecOrGit(canonical.clone()))?;
            if too_broad_without_openspec(&root, home_dir().as_deref()) {
                return Err(RegistrationError::TooBroad(root));
            }
            root
        };

        // If the entry already exists, the only legal action is to promote a
        // previously-discovered entry to user-registered.
        if let Some(existing) = self.entries.get(&canonical) {
            match &existing.origin {
                WorkspaceOrigin::UserRegistered => {
                    return Err(RegistrationError::AlreadyRegistered(canonical));
                }
                WorkspaceOrigin::Discovered { .. } => {
                    let promoted = existing.folder.clone();
                    if let Some(entry) = self.entries.get_mut(&canonical) {
                        entry.origin = WorkspaceOrigin::UserRegistered;
                    }
                    let added = match repo_id {
                        Some(ref rid) => self.discover_and_collect(rid),
                        None => Vec::new(),
                    };
                    self.save()?;
                    return Ok((promoted, added));
                }
            }
        }

        let folder = WorkspaceFolder::from_path(canonical.clone());
        self.entries.insert(
            canonical,
            RegistryEntry {
                folder: folder.clone(),
                origin: WorkspaceOrigin::UserRegistered,
                repo_id: repo_id.clone(),
            },
        );

        let mut added = vec![folder.clone()];
        if let Some(rid) = repo_id {
            added.extend(self.discover_and_collect(&rid));
        }

        self.save()?;
        Ok((folder, added))
    }

    /// Removes the entry at `path`. If the removed entry was user-registered
    /// and was the last user-registered entry for its repository, every
    /// discovered entry tagged with that repository is also removed.
    /// Returns the canonicalised paths actually removed (including cascaded).
    /// Persists to disk when a user-registered removal occurred.
    pub fn unregister(&mut self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let canonical = crate::paths::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        // `shift_remove` (not `swap_remove`) so the remaining entries keep their
        // config order.
        let entry = match self.entries.shift_remove(&canonical) {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        let mut removed = vec![canonical];

        if matches!(entry.origin, WorkspaceOrigin::UserRegistered) {
            if let Some(repo_id) = entry.repo_id.as_ref() {
                let still_has_user = self.entries.values().any(|e| {
                    matches!(e.origin, WorkspaceOrigin::UserRegistered)
                        && e.repo_id.as_ref() == Some(repo_id)
                });
                if !still_has_user {
                    let cascade: Vec<PathBuf> = self
                        .entries
                        .iter()
                        .filter(|(_, e)| match &e.origin {
                            WorkspaceOrigin::Discovered { discovered_via } => {
                                discovered_via == repo_id
                            }
                            WorkspaceOrigin::UserRegistered => false,
                        })
                        .map(|(p, _)| p.clone())
                        .collect();
                    for p in cascade {
                        if self.entries.shift_remove(&p).is_some() {
                            removed.push(p);
                        }
                    }
                }
            }
            self.save()?;
        }

        Ok(removed)
    }

    /// Reconcile the discovered set for `repo_id` against `git worktree list`.
    /// Returns `(added, removed)`: newly-discovered worktrees the caller
    /// should start watching, and paths whose entries the caller should stop
    /// watching. Does not persist (discovered entries are never persisted).
    /// User-registered entries are never touched by reconciliation.
    pub fn reconcile_repo(&mut self, repo_id: &RepoId) -> (Vec<WorkspaceFolder>, Vec<PathBuf>) {
        let truth: HashSet<PathBuf> = git::worktree_list(repo_id)
            .into_iter()
            .filter(|wt| !wt.is_prunable)
            .filter_map(|wt| crate::paths::canonicalize(&wt.path).ok())
            .filter(|root| is_working_tree(root))
            .collect();
        let every_worktree = self.registered_at_a_root(repo_id, &truth);

        // Append newly-discovered worktrees in a deterministic (sorted) order so
        // the discovered set does not depend on hash-set iteration.
        let mut truth_sorted: Vec<&PathBuf> = truth.iter().collect();
        truth_sorted.sort();
        let mut added = Vec::new();
        for path in truth_sorted {
            if self.entries.contains_key(path) {
                continue;
            }
            if !every_worktree && !path.join("openspec").is_dir() {
                continue;
            }
            let folder = WorkspaceFolder::from_path(path.clone());
            self.entries.insert(
                path.clone(),
                RegistryEntry {
                    folder: folder.clone(),
                    origin: WorkspaceOrigin::Discovered {
                        discovered_via: repo_id.clone(),
                    },
                    repo_id: Some(repo_id.clone()),
                },
            );
            added.push(folder);
        }

        let removed: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(p, e)| {
                e.repo_id.as_ref() == Some(repo_id)
                    && matches!(e.origin, WorkspaceOrigin::Discovered { .. })
                    && !truth.contains(p.as_path())
            })
            .map(|(p, _)| p.clone())
            .collect();
        for p in &removed {
            self.entries.shift_remove(p);
        }

        (added, removed)
    }

    /// Returns user-registered workspaces only, alphabetised by name, with
    /// `is_missing` recomputed against the current filesystem state.
    /// Discovered worktrees do not appear here — Settings shows the manageable
    /// set, not the auto-derived one.
    pub fn list(&self) -> Vec<RegisteredWorkspace> {
        let mut items: Vec<RegisteredWorkspace> = self
            .entries
            .values()
            .filter(|e| matches!(e.origin, WorkspaceOrigin::UserRegistered))
            .map(|e| RegisteredWorkspace::from_folder(&e.folder))
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then(a.uri.cmp(&b.uri)));
        items
    }

    /// Number of entries in the registry (user-registered + discovered).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// All `WorkspaceFolder`s currently tracked (user-registered + discovered).
    /// Used by the watcher manager on startup and after registration to
    /// install the corresponding filesystem watchers.
    pub fn folders(&self) -> Vec<WorkspaceFolder> {
        self.entries.values().map(|e| e.folder.clone()).collect()
    }

    /// All entries with their origin and repo association. Used by the
    /// aggregator to build per-repo views.
    pub fn entries(&self) -> Vec<RegistryEntry> {
        self.entries.values().cloned().collect()
    }

    /// Look up an entry by canonical path.
    pub fn entry(&self, path: &Path) -> Option<&RegistryEntry> {
        self.entries.get(path)
    }

    /// Distinct repository IDs across all entries, in first-seen (config) order
    /// so callers that build per-repo state do so deterministically.
    pub fn repos(&self) -> Vec<RepoId> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for entry in self.entries.values() {
            if let Some(rid) = entry.repo_id.clone() {
                if seen.insert(rid.clone()) {
                    out.push(rid);
                }
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn save(&self) -> io::Result<()> {
        let parent = self
            .config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        fs::create_dir_all(&parent)?;

        let config = ConfigFile {
            // `IndexMap` iterates in insertion order, so user-registered entries
            // serialise in config order — never re-scrambled.
            workspaces: self
                .entries
                .values()
                .filter(|e| matches!(e.origin, WorkspaceOrigin::UserRegistered))
                .map(|e| e.folder.clone())
                .collect(),
        };
        let raw = serde_json::to_string_pretty(&config)?;

        // Atomic write: stage into a uniquely-named temp file in the SAME
        // directory (so the rename is atomic on one filesystem, and two
        // concurrent instances never collide on a fixed `.tmp`), fsync it, then
        // rename over the target. A crash or a concurrent writer can therefore
        // never observe — or leave behind — a truncated registry.
        use std::io::Write as _;
        let mut tmp = tempfile::NamedTempFile::new_in(&parent)?;
        tmp.write_all(raw.as_bytes())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.config_path).map_err(|e| e.error)?;
        // Best-effort directory fsync so the rename itself is durable on a crash.
        if let Ok(dir) = fs::File::open(&parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn derive_all_discovered(&mut self) {
        // First-seen (config) order via `repos()`, so discovered entries are
        // appended reproducibly after the user-registered block.
        for repo_id in self.repos() {
            self.discover_and_collect(&repo_id);
        }
    }

    /// Whether discovery tracks every worktree of `repo_id`, or only those
    /// whose root holds an `openspec/` subdirectory. Every worktree is tracked
    /// once the repository has a user-registered entry at one of its worktree
    /// `roots`; a repository registered only below its roots keeps the
    /// `openspec/` filter, so a monorepo package is never joined by the
    /// repository root that contains it.
    fn registered_at_a_root(&self, repo_id: &RepoId, roots: &HashSet<PathBuf>) -> bool {
        self.entries.iter().any(|(path, e)| {
            matches!(e.origin, WorkspaceOrigin::UserRegistered)
                && e.repo_id.as_ref() == Some(repo_id)
                && roots.contains(path)
        })
    }

    fn discover_and_collect(&mut self, repo_id: &RepoId) -> Vec<WorkspaceFolder> {
        let mut added = Vec::new();
        // Deterministic order so the appended discovered set is reproducible
        // rather than dependent on `git worktree list` ordering.
        let mut worktrees: Vec<PathBuf> = git::worktree_list(repo_id)
            .into_iter()
            .filter(|wt| !wt.is_prunable)
            .filter_map(|wt| crate::paths::canonicalize(&wt.path).ok())
            .filter(|root| is_working_tree(root))
            .collect();
        worktrees.sort();
        let roots: HashSet<PathBuf> = worktrees.iter().cloned().collect();
        let every_worktree = self.registered_at_a_root(repo_id, &roots);
        for canonical in worktrees {
            if self.entries.contains_key(&canonical) {
                continue;
            }
            if !every_worktree && !canonical.join("openspec").is_dir() {
                continue;
            }
            let folder = WorkspaceFolder::from_path(canonical.clone());
            self.entries.insert(
                canonical,
                RegistryEntry {
                    folder: folder.clone(),
                    origin: WorkspaceOrigin::Discovered {
                        discovered_via: repo_id.clone(),
                    },
                    repo_id: Some(repo_id.clone()),
                },
            );
            added.push(folder);
        }
        added
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
        assert!(output.status.success(), "git {:?} failed", args);
    }

    fn init_openspec_repo(root: &Path) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        fs::create_dir_all(root.join("openspec/changes")).unwrap();
        git(&["init", "-b", "main"], root);
        git(&["config", "user.email", "t@t"], root);
        git(&["config", "user.name", "t"], root);
        git(&["commit", "--allow-empty", "-m", "init"], root);
        root.canonicalize().unwrap()
    }

    fn add_worktree(root: &Path, branch: &str, path: &Path) {
        add_plain_worktree(root, branch, path);
        fs::create_dir_all(path.join("openspec/changes")).unwrap();
    }

    /// A git repository with one commit and no `openspec/` anywhere.
    fn init_plain_repo(root: &Path) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        git(&["init", "-b", "main"], root);
        git(&["config", "user.email", "t@t"], root);
        git(&["config", "user.name", "t"], root);
        git(&["commit", "--allow-empty", "-m", "init"], root);
        root.canonicalize().unwrap()
    }

    fn add_plain_worktree(root: &Path, branch: &str, path: &Path) {
        git(
            &["worktree", "add", "-b", branch, path.to_str().unwrap()],
            root,
        );
    }

    #[test]
    fn a_git_repository_without_openspec_is_registered_with_every_worktree() {
        let tmp = TempDir::new().unwrap();
        let root = init_plain_repo(&tmp.path().join("api"));
        let feature = tmp.path().join("api-feature");
        add_plain_worktree(&root, "feature", &feature);

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        let added = reg.register(root.clone()).unwrap();
        assert_eq!(added[0].uri, root);
        let entry = reg.entry(&root).unwrap();
        assert!(matches!(entry.origin, WorkspaceOrigin::UserRegistered));
        assert!(entry.repo_id.is_some());
        let feature = feature.canonicalize().unwrap();
        assert!(matches!(
            reg.entry(&feature).unwrap().origin,
            WorkspaceOrigin::Discovered { .. }
        ));

        // A worktree added at runtime is reconciled in, OpenSpec or not.
        let late = tmp.path().join("api-late");
        add_plain_worktree(&root, "late", &late);
        let repo_id = reg.repos().into_iter().next().unwrap();
        let (added, removed) = reg.reconcile_repo(&repo_id);
        let late = late.canonicalize().unwrap();
        assert_eq!(
            added.iter().map(|f| &f.uri).collect::<Vec<_>>(),
            vec![&late]
        );
        assert!(removed.is_empty());
    }

    #[test]
    fn a_subfolder_without_openspec_registers_its_worktree_root() {
        let tmp = TempDir::new().unwrap();
        let root = init_plain_repo(&tmp.path().join("api"));
        fs::create_dir_all(root.join("docs")).unwrap();
        let config = tmp.path().join("workspaces.json");

        let mut reg = WorkspaceRegistry::new(config.clone());
        let added = reg.register(root.join("docs")).unwrap();
        assert_eq!(added[0].uri, root);
        assert!(reg.entry(&root.join("docs")).is_none());

        let reloaded = WorkspaceRegistry::load(config).unwrap();
        let listed: Vec<_> = reloaded.list().into_iter().map(|w| w.uri).collect();
        assert_eq!(listed, vec![root]);
    }

    #[test]
    fn a_subfolder_registers_the_deepest_worktree_that_contains_it() {
        let tmp = TempDir::new().unwrap();
        let root = init_plain_repo(&tmp.path().join("repo"));
        fs::create_dir_all(root.join("docs")).unwrap();
        let nested = root.join(".claude/worktrees/x");
        add_plain_worktree(&root, "x", &nested);
        fs::create_dir_all(nested.join("docs")).unwrap();
        let nested = nested.canonicalize().unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        let added = reg.register(nested.join("docs")).unwrap();
        assert_eq!(
            added[0].uri, nested,
            "the linked worktree, not the main root"
        );
        assert!(matches!(
            reg.entry(&root).unwrap().origin,
            WorkspaceOrigin::Discovered { .. }
        ));

        // The main worktree's own subfolder resolves to the main root, which
        // was discovered above and is promoted rather than duplicated.
        let promoted = reg.register(root.join("docs")).unwrap();
        assert!(promoted.is_empty());
        assert!(matches!(
            reg.entry(&root).unwrap().origin,
            WorkspaceOrigin::UserRegistered
        ));
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn a_subfolder_holding_openspec_is_registered_as_selected() {
        let tmp = TempDir::new().unwrap();
        let root = init_plain_repo(&tmp.path().join("mono"));
        let specs = root.join("packages/specs");
        fs::create_dir_all(specs.join("openspec/changes")).unwrap();
        let specs = specs.canonicalize().unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        let added = reg.register(specs.clone()).unwrap();
        assert_eq!(added[0].uri, specs);
        assert!(reg.entry(&root).is_none());
    }

    #[test]
    fn a_repository_registered_below_its_root_keeps_the_openspec_filter() {
        let tmp = TempDir::new().unwrap();
        let root = init_plain_repo(&tmp.path().join("mono"));
        let specs = root.join("packages/specs");
        fs::create_dir_all(specs.join("openspec/changes")).unwrap();
        // A sibling whose root holds OpenSpec is discovered, as before...
        let with = tmp.path().join("wt-with");
        add_worktree(&root, "with", &with);
        // ...one without is not.
        let without = tmp.path().join("wt-without");
        add_plain_worktree(&root, "without", &without);

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        reg.register(specs.clone()).unwrap();
        assert!(reg.entry(&specs.canonicalize().unwrap()).is_some());
        assert!(
            reg.entry(&root).is_none(),
            "its own worktree root is not tracked beside the subfolder"
        );
        let with = with.canonicalize().unwrap();
        assert!(matches!(
            reg.entry(&with).unwrap().origin,
            WorkspaceOrigin::Discovered { .. }
        ));
        assert!(reg.entry(&without.canonicalize().unwrap()).is_none());
        assert_eq!(reg.len(), 2);

        // Reconciliation keeps the filter: the discovered `wt-with` sits at a
        // worktree root, but only a user-registered entry there lifts it.
        let late = tmp.path().join("wt-late");
        add_plain_worktree(&root, "late", &late);
        let repo_id = reg.repos().into_iter().next().unwrap();
        let (added, _) = reg.reconcile_repo(&repo_id);
        assert!(added.is_empty(), "added={added:?}");
    }

    #[test]
    fn a_worktree_without_openspec_of_a_root_registered_repo_is_discovered() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let old = tmp.path().join("pre-openspec");
        add_plain_worktree(&root, "pre-openspec", &old);

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        let added = reg.register(root.clone()).unwrap();
        let old = old.canonicalize().unwrap();
        assert!(added.iter().any(|f| f.uri == old), "added={added:?}");
        assert!(matches!(
            reg.entry(&old).unwrap().origin,
            WorkspaceOrigin::Discovered { .. }
        ));
    }

    #[test]
    fn a_bare_repository_directory_is_never_tracked() {
        let tmp = TempDir::new().unwrap();
        let origin = init_plain_repo(&tmp.path().join("origin"));
        let bare = tmp.path().join("b.git");
        git(
            &[
                "clone",
                "--bare",
                origin.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
            tmp.path(),
        );
        let wt = tmp.path().join("wt");
        git(&["worktree", "add", wt.to_str().unwrap(), "main"], &bare);
        let bare = bare.canonicalize().unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        // Inside the bare directory there is no working tree to register.
        let err = reg
            .register(bare.join("refs"))
            .expect_err("no working tree");
        assert!(matches!(err, RegistrationError::NotOpenSpecOrGit(_)));

        reg.register(wt.clone()).unwrap();
        assert!(
            reg.entry(&bare).is_none(),
            "the bare directory is not discovered"
        );
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn a_separate_git_dir_work_tree_registers_and_its_store_is_never_tracked() {
        // `git worktree list` names the git STORE as this repository's main
        // worktree, so neither the gate nor discovery may take its word for
        // what a working tree is.
        let tmp = TempDir::new().unwrap();
        let store = tmp.path().join("store.git");
        let work = tmp.path().join("work");
        git(
            &[
                "init",
                "-b",
                "main",
                "--separate-git-dir",
                store.to_str().unwrap(),
                work.to_str().unwrap(),
            ],
            tmp.path(),
        );
        git(&["config", "user.email", "t@t"], &work);
        git(&["config", "user.name", "t"], &work);
        git(&["commit", "--allow-empty", "-m", "init"], &work);
        fs::create_dir_all(work.join("docs")).unwrap();
        let linked = tmp.path().join("linked");
        add_plain_worktree(&work, "feature", &linked);
        let (work, store, linked) = (
            work.canonicalize().unwrap(),
            store.canonicalize().unwrap(),
            linked.canonicalize().unwrap(),
        );

        let mut reg = WorkspaceRegistry::new(tmp.path().join("one.json"));
        let added = reg.register(work.join("docs")).unwrap();
        assert_eq!(added[0].uri, work, "the work tree's root, as git names it");

        // Registered at a listed root, the linked worktree lifts the
        // `openspec/` filter — and the store still never becomes a worktree.
        let mut reg = WorkspaceRegistry::new(tmp.path().join("two.json"));
        reg.register(linked.clone()).unwrap();
        assert!(reg.entry(&store).is_none(), "the git store is not tracked");
        let repo_id = reg.repos().into_iter().next().unwrap();
        let (added, _) = reg.reconcile_repo(&repo_id);
        assert!(added.is_empty(), "reconciliation agrees: added={added:?}");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn a_home_directory_or_filesystem_root_is_too_broad_without_openspec() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let project = home.join("project");
        fs::create_dir_all(&project).unwrap();

        assert!(too_broad_without_openspec(&home, Some(&home)));
        assert!(too_broad_without_openspec(Path::new("/"), None));
        assert!(!too_broad_without_openspec(&project, Some(&home)));
        assert!(!too_broad_without_openspec(&project, None));
    }

    #[test]
    fn the_home_directory_is_the_one_the_environment_names() {
        let expected = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        assert!(expected.is_some(), "the test environment names a home");
        assert_eq!(home_dir(), expected);
    }

    #[test]
    fn registering_a_non_git_workspace_keeps_it_flat() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(ws.join("openspec/changes")).unwrap();
        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        let added = reg.register(ws.clone()).unwrap();
        assert_eq!(added.len(), 1, "non-git workspace should add only itself");
        let canonical = ws.canonicalize().unwrap();
        let entry = reg.entry(&canonical).unwrap();
        assert!(matches!(entry.origin, WorkspaceOrigin::UserRegistered));
        assert!(entry.repo_id.is_none());
    }

    #[test]
    fn registering_a_git_repo_auto_discovers_sibling_worktrees() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);

        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        let added = reg.register(root.clone()).unwrap();
        assert_eq!(
            added.len(),
            2,
            "should add user-registered + discovered sibling"
        );
        assert_eq!(reg.len(), 2);
        let wt2_canonical = wt2.canonicalize().unwrap();
        let discovered = reg.entry(&wt2_canonical).unwrap();
        assert!(matches!(
            discovered.origin,
            WorkspaceOrigin::Discovered { .. }
        ));
    }

    #[test]
    fn unregister_cascades_discovered_when_last_user_entry_for_repo_removed() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);

        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        reg.register(root.clone()).unwrap();
        assert_eq!(reg.len(), 2);

        let removed = reg.unregister(&root).unwrap();
        assert_eq!(removed.len(), 2, "user + cascaded discovered");
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn unregister_keeps_discovered_when_another_user_entry_for_repo_remains() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);

        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        reg.register(root.clone()).unwrap();
        // Promote the discovered wt2 to user-registered.
        let added = reg.register(wt2.clone()).unwrap();
        assert!(added.is_empty(), "promotion adds no new entries");
        assert_eq!(reg.len(), 2);

        // Unregister the main entry; wt2 is still user-registered for the same
        // repo so cascade should not fire.
        let removed = reg.unregister(&root).unwrap();
        assert_eq!(removed, vec![root.clone()]);
        assert_eq!(reg.len(), 1);
        let wt2_canonical = wt2.canonicalize().unwrap();
        let entry = reg.entry(&wt2_canonical).unwrap();
        assert!(matches!(entry.origin, WorkspaceOrigin::UserRegistered));
    }

    #[test]
    fn load_redrives_discovered_from_user_registered_repos() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);

        let config_path = tmp.path().join("workspaces.json");
        {
            let mut reg = WorkspaceRegistry::new(config_path.clone());
            reg.register(root.clone()).unwrap();
            assert_eq!(reg.len(), 2);
        }

        let reg = WorkspaceRegistry::load(config_path).unwrap();
        assert_eq!(reg.len(), 2, "load + re-derive should restore both");
        let wt2_canonical = wt2.canonicalize().unwrap();
        let entry = reg.entry(&wt2_canonical).unwrap();
        assert!(matches!(entry.origin, WorkspaceOrigin::Discovered { .. }));
    }

    #[test]
    fn reconcile_repo_adds_new_worktree_and_removes_stale() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt_old = tmp.path().join("wt_old");
        add_worktree(&root, "old", &wt_old);

        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        reg.register(root.clone()).unwrap();
        assert_eq!(reg.len(), 2);

        let wt_new = tmp.path().join("wt_new");
        add_worktree(&root, "new", &wt_new);

        let wt_old_basename = wt_old.file_name().map(|s| s.to_owned());
        fs::remove_dir_all(&wt_old).unwrap();

        let repo_id = reg.repos().into_iter().next().unwrap();
        let (added, removed) = reg.reconcile_repo(&repo_id);
        let wt_new_canonical = wt_new.canonicalize().unwrap();
        assert!(added.iter().any(|f| f.uri == wt_new_canonical));
        assert!(
            removed
                .iter()
                .any(|p| p.file_name() == wt_old_basename.as_deref()),
            "removed list should include the deleted worktree: removed={:?}",
            removed,
        );
    }

    #[test]
    fn list_returns_only_user_registered_entries() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);

        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config);
        reg.register(root.clone()).unwrap();

        let listed = reg.list();
        assert_eq!(
            listed.len(),
            1,
            "Settings list should hide discovered worktrees"
        );
    }

    // A flat (non-git) workspace directory with an `openspec/` subtree.
    fn flat_ws(parent: &Path, name: &str) -> PathBuf {
        let p = parent.join(name);
        fs::create_dir_all(p.join("openspec/changes")).unwrap();
        p
    }

    #[test]
    fn config_order_is_preserved_across_save_and_reload() {
        let tmp = TempDir::new().unwrap();
        // Registered in a deliberately non-alphabetical order.
        let c = flat_ws(tmp.path(), "ccc");
        let a = flat_ws(tmp.path(), "aaa");
        let b = flat_ws(tmp.path(), "bbb");
        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config.clone());
        reg.register(c.clone()).unwrap();
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();

        let expected = vec![
            c.canonicalize().unwrap(),
            a.canonicalize().unwrap(),
            b.canonicalize().unwrap(),
        ];
        let order: Vec<PathBuf> = reg.folders().into_iter().map(|f| f.uri).collect();
        assert_eq!(order, expected, "folders() preserves registration order");

        let reloaded = WorkspaceRegistry::load(config).unwrap();
        let order2: Vec<PathBuf> = reloaded.folders().into_iter().map(|f| f.uri).collect();
        assert_eq!(order2, expected, "reload preserves the same order");
    }

    #[test]
    fn saving_does_not_reorder_existing_registrations() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config.clone());
        reg.register(flat_ws(tmp.path(), "ccc")).unwrap();
        reg.register(flat_ws(tmp.path(), "aaa")).unwrap();
        reg.register(flat_ws(tmp.path(), "bbb")).unwrap();
        let first = fs::read_to_string(&config).unwrap();
        reg.save().unwrap();
        reg.save().unwrap();
        let again = fs::read_to_string(&config).unwrap();
        assert_eq!(first, again, "repeated saves must not reshuffle the file");
    }

    #[test]
    fn unregister_preserves_order_of_remaining() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        let a = flat_ws(tmp.path(), "aaa");
        let b = flat_ws(tmp.path(), "bbb");
        let c = flat_ws(tmp.path(), "ccc");
        let mut reg = WorkspaceRegistry::new(config.clone());
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();
        reg.register(c.clone()).unwrap();
        reg.unregister(&b.canonicalize().unwrap()).unwrap();

        let expected = vec![a.canonicalize().unwrap(), c.canonicalize().unwrap()];
        let order: Vec<PathBuf> = reg.folders().into_iter().map(|f| f.uri).collect();
        assert_eq!(order, expected);
        let reloaded = WorkspaceRegistry::load(config).unwrap();
        let order2: Vec<PathBuf> = reloaded.folders().into_iter().map(|f| f.uri).collect();
        assert_eq!(order2, expected);
    }

    #[test]
    fn load_missing_file_creates_nothing() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        let reg = WorkspaceRegistry::load(config.clone()).unwrap();
        assert!(reg.is_empty());
        assert!(!config.exists(), "load must not create the config file");
    }

    #[test]
    fn load_empty_file_is_untouched() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        for body in [r#"{"workspaces":[]}"#, "{}"] {
            fs::write(&config, body).unwrap();
            let before = fs::read_to_string(&config).unwrap();
            let reg = WorkspaceRegistry::load(config.clone()).unwrap();
            assert!(reg.is_empty());
            assert_eq!(fs::read_to_string(&config).unwrap(), before);
        }
    }

    #[test]
    fn load_corrupt_file_fails_and_is_preserved() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        fs::write(&config, "{ this is not json").unwrap();
        let before = fs::read_to_string(&config).unwrap();
        let err = WorkspaceRegistry::load(config.clone()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(&config).unwrap(),
            before,
            "a corrupt file must never be overwritten"
        );
    }

    #[test]
    fn load_dedupes_duplicate_resolvable_paths_first_wins() {
        let tmp = TempDir::new().unwrap();
        let a = flat_ws(tmp.path(), "aaa");
        let ca = a.canonicalize().unwrap();
        let config = tmp.path().join("workspaces.json");
        let with_slash = format!("{}/", ca.display());
        let json = serde_json::json!({
            "workspaces": [
                {"uri": ca, "name": "aaa"},
                {"uri": with_slash, "name": "aaa-dup"},
            ]
        });
        fs::write(&config, serde_json::to_string(&json).unwrap()).unwrap();
        let reg = WorkspaceRegistry::load(config).unwrap();
        assert_eq!(reg.len(), 1, "duplicate spellings collapse to one entry");
    }

    #[test]
    fn load_dedupes_missing_folder_spellings_but_keeps_distinct() {
        let tmp = TempDir::new().unwrap();
        let config = tmp.path().join("workspaces.json");
        // Two spellings of the SAME missing folder collapse via the fallback key.
        let gone = tmp.path().join("gone");
        let gone_slash = format!("{}/", gone.display());
        let json = serde_json::json!({
            "workspaces": [
                {"uri": gone, "name": "gone"},
                {"uri": gone_slash, "name": "gone-dup"},
            ]
        });
        fs::write(&config, serde_json::to_string(&json).unwrap()).unwrap();
        assert_eq!(WorkspaceRegistry::load(config.clone()).unwrap().len(), 1);

        // Two genuinely distinct missing folders stay separate.
        let json2 = serde_json::json!({
            "workspaces": [
                {"uri": tmp.path().join("g1"), "name": "g1"},
                {"uri": tmp.path().join("g2"), "name": "g2"},
            ]
        });
        fs::write(&config, serde_json::to_string(&json2).unwrap()).unwrap();
        assert_eq!(WorkspaceRegistry::load(config).unwrap().len(), 2);
    }

    #[test]
    fn discovered_worktrees_never_enter_the_saved_file() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);
        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config.clone());
        reg.register(root.clone()).unwrap();
        assert_eq!(reg.len(), 2, "root + discovered sibling in memory");

        let raw = fs::read_to_string(&config).unwrap();
        let cfg: ConfigFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            cfg.workspaces.len(),
            1,
            "only the user-registered root is persisted"
        );
        assert_eq!(cfg.workspaces[0].uri, root);
    }

    #[test]
    fn promotion_appends_after_user_registered_entries() {
        let tmp = TempDir::new().unwrap();
        let root = init_openspec_repo(&tmp.path().join("repo"));
        let wt2 = tmp.path().join("wt2");
        add_worktree(&root, "feature", &wt2);
        let config = tmp.path().join("workspaces.json");
        let mut reg = WorkspaceRegistry::new(config.clone());
        reg.register(root.clone()).unwrap();
        reg.register(wt2.clone()).unwrap(); // promote the discovered sibling

        let raw = fs::read_to_string(&config).unwrap();
        let cfg: ConfigFile = serde_json::from_str(&raw).unwrap();
        let order: Vec<PathBuf> = cfg.workspaces.iter().map(|w| w.uri.clone()).collect();
        assert_eq!(
            order,
            vec![root.clone(), wt2.canonicalize().unwrap()],
            "promoted worktree is appended after the existing user-registered entry"
        );
    }
}
