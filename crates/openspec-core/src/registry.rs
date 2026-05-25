use crate::git::{self, RepoId};
use crate::types::{RegisteredWorkspace, WorkspaceFolder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("not an OpenSpec workspace (no `openspec/` subdirectory): {0}")]
    NotAnOpenSpecWorkspace(PathBuf),
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
    entries: HashMap<PathBuf, RegistryEntry>,
}

impl WorkspaceRegistry {
    /// Creates an empty registry tied to `config_path` without reading it.
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            entries: HashMap::new(),
        }
    }

    /// Loads the registry from `config_path`. A missing file is treated as
    /// an empty registry; a corrupt file is reported as `InvalidData`. After
    /// loading the user-registered entries, the discovered set is re-derived
    /// by scanning each detected repository's worktrees.
    pub fn load(config_path: PathBuf) -> io::Result<Self> {
        let mut entries: HashMap<PathBuf, RegistryEntry> = HashMap::new();
        if config_path.exists() {
            let raw = fs::read_to_string(&config_path)?;
            let config: ConfigFile = serde_json::from_str(&raw)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for folder in config.workspaces {
                let repo_id = git::git_common_dir(&folder.uri);
                entries.insert(
                    folder.uri.clone(),
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

    /// Validates `path` and adds it to the registry. If `path` is inside a
    /// git repository, every other worktree of the repository is also
    /// discovered and added with `WorkspaceOrigin::Discovered`. Returns the
    /// list of `WorkspaceFolder`s newly added — the user-registered entry
    /// plus any discovered siblings the caller needs to start watching.
    /// Persists to disk on success (discovered entries are never persisted).
    pub fn register(&mut self, path: PathBuf) -> Result<Vec<WorkspaceFolder>, RegistrationError> {
        if !path.exists() {
            return Err(RegistrationError::PathNotFound(path));
        }
        let canonical = path.canonicalize()?;
        if !canonical.is_dir() {
            return Err(RegistrationError::NotADirectory(canonical));
        }
        if !canonical.join("openspec").is_dir() {
            return Err(RegistrationError::NotAnOpenSpecWorkspace(canonical));
        }

        let repo_id = git::git_common_dir(&canonical);

        // If the entry already exists, the only legal action is to promote a
        // previously-discovered entry to user-registered.
        if let Some(existing) = self.entries.get(&canonical) {
            match &existing.origin {
                WorkspaceOrigin::UserRegistered => {
                    return Err(RegistrationError::AlreadyRegistered(canonical));
                }
                WorkspaceOrigin::Discovered { .. } => {
                    if let Some(entry) = self.entries.get_mut(&canonical) {
                        entry.origin = WorkspaceOrigin::UserRegistered;
                    }
                    let added = match repo_id {
                        Some(ref rid) => self.discover_and_collect(rid),
                        None => Vec::new(),
                    };
                    self.save()?;
                    return Ok(added);
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

        let mut added = vec![folder];
        if let Some(rid) = repo_id {
            added.extend(self.discover_and_collect(&rid));
        }

        self.save()?;
        Ok(added)
    }

    /// Removes the entry at `path`. If the removed entry was user-registered
    /// and was the last user-registered entry for its repository, every
    /// discovered entry tagged with that repository is also removed.
    /// Returns the canonicalised paths actually removed (including cascaded).
    /// Persists to disk when a user-registered removal occurred.
    pub fn unregister(&mut self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let entry = match self.entries.remove(&canonical) {
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
                        if self.entries.remove(&p).is_some() {
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
        let truth: HashMap<PathBuf, ()> = git::worktree_list(repo_id)
            .into_iter()
            .filter(|wt| !wt.is_prunable)
            .filter_map(|wt| wt.path.canonicalize().ok().map(|c| (c, ())))
            .collect();

        let mut added = Vec::new();
        for path in truth.keys() {
            if self.entries.contains_key(path) {
                continue;
            }
            if !path.join("openspec").is_dir() {
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
                    && !truth.contains_key(p.as_path())
            })
            .map(|(p, _)| p.clone())
            .collect();
        for p in &removed {
            self.entries.remove(p);
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

    /// Distinct repository IDs across all entries.
    pub fn repos(&self) -> Vec<RepoId> {
        let unique: HashSet<RepoId> = self
            .entries
            .values()
            .filter_map(|e| e.repo_id.clone())
            .collect();
        unique.into_iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = ConfigFile {
            workspaces: self
                .entries
                .values()
                .filter(|e| matches!(e.origin, WorkspaceOrigin::UserRegistered))
                .map(|e| e.folder.clone())
                .collect(),
        };
        let raw = serde_json::to_string_pretty(&config)?;
        fs::write(&self.config_path, raw)
    }

    fn derive_all_discovered(&mut self) {
        let repos: HashSet<RepoId> = self
            .entries
            .values()
            .filter_map(|e| e.repo_id.clone())
            .collect();
        for repo_id in repos {
            self.discover_and_collect(&repo_id);
        }
    }

    fn discover_and_collect(&mut self, repo_id: &RepoId) -> Vec<WorkspaceFolder> {
        let mut added = Vec::new();
        for wt in git::worktree_list(repo_id) {
            if wt.is_prunable {
                continue;
            }
            let canonical = match wt.path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };
            if self.entries.contains_key(&canonical) {
                continue;
            }
            if !canonical.join("openspec").is_dir() {
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
        git(
            &["worktree", "add", "-b", branch, path.to_str().unwrap()],
            root,
        );
        fs::create_dir_all(path.join("openspec/changes")).unwrap();
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
}
