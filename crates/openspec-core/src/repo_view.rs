//! Aggregator that turns per-workspace [`ChangeData`] lists into a
//! repo-grouped, logical-change-keyed view for the frontend.
//!
//! The frontend consumes [`WorkspaceView`]s; the tree pane renders one node
//! per top-level entry, with logical changes grouped by `(repo_id, change_name)`
//! and one [`ChangeInstance`] row per worktree that contains the change.
//!
//! Non-git workspaces (no `RepoId`) fall through to [`WorkspaceView::Flat`]
//! and skip the aggregation entirely — there is nothing to aggregate across.

use crate::cache::WorkspaceCache;
use crate::git::{self, RepoId, SpecCommitState, WorktreeStatus};
use crate::parser::list_archived_stubs;
use crate::registry::{WorkspaceOrigin, WorkspaceRegistry};
use crate::types::{ChangeData, PaletteColor, WorkspaceFolder};
use crate::watcher::CacheEvent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Top-level entry the frontend renders. Either a git-backed repository
/// with logical changes aggregated across its worktrees, or a standalone
/// non-git workspace rendered flat as before.
///
/// `display_name` and `color` on both variants are populated by the IPC
/// layer from the presentation store after aggregation; the pure aggregator
/// always leaves them `None`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkspaceView {
    Repo(RepoView),
    Flat {
        workspace: WorkspaceFolder,
        changes: Vec<ChangeData>,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        color: Option<PaletteColor>,
    },
}

/// A git repository's aggregated view: every distinct logical change across
/// every tracked worktree, split into active and archived sections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoView {
    /// Canonical path to the repository's git common dir. Serves as the
    /// stable identity across worktrees.
    pub repo_id: PathBuf,
    /// The repository's main worktree (the one containing the `.git/` dir,
    /// not a `.git` file).
    pub main_worktree: PathBuf,
    /// Default name — the basename of the main worktree path. Used as the
    /// fallback when `display_name` is `None`.
    pub name: String,
    /// Default branch resolved via the documented cascade; `None` if no
    /// branch could be determined.
    pub default_branch: Option<String>,
    /// Logical changes with at least one non-archived instance, sorted by
    /// name.
    pub active: Vec<LogicalChange>,
    /// Logical changes where every instance is archived, sorted by name.
    ///
    /// Populated from cheap stubs (directory listing, no parse) and kept only
    /// so [`diff_views`] can distinguish an archived change from a deleted one
    /// and emit `LogicalChangeArchived`. It is **not** serialized to the
    /// frontend — archived changes are browsed via the Archive view, which
    /// loads them lazily and per-workspace — so the instances here carry stub
    /// `ChangeData` with no parsed content.
    #[serde(default, skip_serializing)]
    pub archived: Vec<LogicalChange>,
    /// Configured display-name override from the presentation store, if any.
    /// Populated post-aggregation by the IPC layer.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Configured tint colour from the presentation store, if any. Populated
    /// post-aggregation by the IPC layer.
    #[serde(default)]
    pub color: Option<PaletteColor>,
    /// True when any worktree of the repository has an uncommitted change
    /// (staged, unstaged, or untracked) — the whole-repo dirty rollup.
    #[serde(default)]
    pub dirty: bool,
    /// Worktree paths that are individually dirty; powers the rollup tooltip.
    #[serde(default)]
    pub dirty_worktrees: Vec<PathBuf>,
    /// True when any change instance in the repository has a spec commit state
    /// other than `Committed`.
    #[serde(default)]
    pub has_uncommitted_specs: bool,
}

/// A change identified by `(repo_id, change_name)`, with one entry per
/// worktree that contains it. Instances are ordered by most-recently-modified
/// first; the first instance is the "primary" the active indicator pins to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicalChange {
    pub name: String,
    pub instances: Vec<ChangeInstance>,
}

/// A single instance of a logical change in one specific worktree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeInstance {
    pub worktree_path: PathBuf,
    pub branch: Option<String>,
    pub is_main_worktree: bool,
    pub is_default_branch: bool,
    pub is_archived_here: bool,
    pub change: ChangeData,
    /// Unix-epoch seconds of the most recent modification time across the
    /// change directory's files. Used to order instances and to pick the
    /// primary.
    pub modified_at: u64,
    pub divergence: Option<DivergenceLabel>,
    /// Commit state of this instance's `openspec/changes/<id>/` directory in
    /// its worktree. Archived instances are reported as `Committed` (their chip
    /// is not surfaced in the active tree).
    #[serde(default = "committed_state")]
    pub spec_commit_state: SpecCommitState,
}

/// Serde default for [`ChangeInstance::spec_commit_state`] — a deserialized
/// instance with no recorded state is treated as committed.
fn committed_state() -> SpecCommitState {
    SpecCommitState::Committed
}

/// Reason this instance is flagged as out of sync with the default-branch
/// instance of the same logical change. `None` is the common in-flight case
/// (change exists only on this branch).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DivergenceLabel {
    /// Content differs from the default-branch instance.
    Diverged,
    /// Archived on the default branch but still active here.
    StaleVsArchived,
}

/// Input shape for the aggregator. One snapshot per tracked worktree of a
/// repository, plus the repository's identity and default branch.
#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub repo_id: RepoId,
    pub main_worktree: PathBuf,
    pub default_branch: Option<String>,
    pub worktrees: Vec<WorktreeSnapshot>,
}

/// Per-worktree snapshot fed to the aggregator. The caller is responsible
/// for parsing the worktree's `openspec/changes/` and `openspec/changes/archive/`
/// directories before constructing this.
#[derive(Debug, Clone)]
pub struct WorktreeSnapshot {
    pub workspace: WorkspaceFolder,
    pub branch: Option<String>,
    pub active_changes: Vec<ChangeData>,
    pub archived_changes: Vec<ChangeData>,
    /// Git working-tree status for this worktree, gathered by the orchestrator
    /// ([`compute_views`]) so the pure aggregator stays I/O-free.
    pub status: WorktreeStatus,
}

/// One top-level row to aggregate, already in final config order. Keeping repos
/// and flats in a single ordered list (rather than two separate vectors) is what
/// lets the output interleave them by config position instead of emitting all
/// repos before all flats.
#[derive(Debug, Clone)]
pub enum ViewInput {
    Repo(RepoSnapshot),
    Flat(WorkspaceFolder, Vec<ChangeData>),
}

/// Aggregate pre-gathered snapshots into the views the frontend consumes,
/// preserving the order of `inputs`. Pure function — no I/O, no git invocations,
/// no global state.
pub fn aggregate(inputs: Vec<ViewInput>) -> Vec<WorkspaceView> {
    inputs
        .into_iter()
        .map(|input| match input {
            ViewInput::Repo(repo) => WorkspaceView::Repo(build_repo_view(repo)),
            ViewInput::Flat(workspace, changes) => WorkspaceView::Flat {
                workspace,
                changes,
                display_name: None,
                color: None,
            },
        })
        .collect()
}

/// Orchestrator: gather the inputs from the registry, the cache, and the
/// caller-supplied default-branch resolver, then run [`aggregate`]. The
/// `default_branch_for` closure is the indirection that lets the runtime
/// use the [`crate::repo_monitor::RepoMonitor`]'s cached value while tests
/// inject arbitrary values.
pub fn compute_views(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
) -> Vec<WorkspaceView> {
    // Build the top-level rows in config first-appearance order, interleaving
    // repo groups and flat workspaces. A repo claims its slot at the position of
    // its earliest user-registered worktree; later worktrees of the same repo
    // fold into that slot. Iteration over `registry.entries()` is in config
    // order (the registry is insertion-ordered), so the result is deterministic.
    enum Slot {
        Repo(RepoId),
        Flat(WorkspaceFolder, Vec<ChangeData>),
    }
    let mut slots: Vec<Slot> = Vec::new();
    let mut repo_seen: HashSet<RepoId> = HashSet::new();
    let mut entries_by_repo: HashMap<RepoId, Vec<crate::registry::RegistryEntry>> = HashMap::new();

    for entry in registry.entries() {
        match &entry.repo_id {
            Some(repo_id) => {
                if repo_seen.insert(repo_id.clone()) {
                    slots.push(Slot::Repo(repo_id.clone()));
                }
                entries_by_repo
                    .entry(repo_id.clone())
                    .or_default()
                    .push(entry);
            }
            None => {
                // Only user-registered non-git entries surface as Flat — a
                // discovered entry without a repo_id shouldn't exist by
                // construction, but if one does we ignore it.
                if matches!(entry.origin, WorkspaceOrigin::UserRegistered) {
                    let changes = cache.changes_for(&entry.folder.uri).to_vec();
                    slots.push(Slot::Flat(entry.folder.clone(), changes));
                }
            }
        }
    }

    let inputs: Vec<ViewInput> = slots
        .into_iter()
        .map(|slot| match slot {
            Slot::Flat(workspace, changes) => ViewInput::Flat(workspace, changes),
            Slot::Repo(repo_id) => {
                let entries_in_repo = entries_by_repo.remove(&repo_id).unwrap_or_default();
                // Determine the main worktree from `git worktree list`. If the
                // call fails (e.g. git binary went missing), fall back to the
                // entry that most plausibly is the main one — heuristic: path
                // matching the parent of the common dir.
                let main_worktree = git::worktree_list(&repo_id)
                    .into_iter()
                    .find(|wt| wt.is_main)
                    .map(|wt| wt.path)
                    .or_else(|| repo_id.as_path().parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| repo_id.as_path().to_path_buf());

                let default_branch = default_branch_for(&repo_id);

                let mut worktrees = Vec::with_capacity(entries_in_repo.len());
                for entry in entries_in_repo {
                    let active_changes = cache.changes_for(&entry.folder.uri).to_vec();
                    // Cheap stubs (directory listing only) — enough for the
                    // logical diff to tell archived from deleted, without parsing
                    // the archive. The archive's content is loaded lazily by the
                    // Archive browser.
                    let archived_changes = list_archived_stubs(&entry.folder).unwrap_or_default();
                    let branch = git::current_branch(&entry.folder.uri);
                    // One `git status` per worktree, beside the `current_branch`
                    // call, classifying the worktree's active change directories.
                    // Archived changes are not classified here (they live under
                    // `archive/` and carry no active-tree chip).
                    let change_ids: Vec<String> =
                        active_changes.iter().map(|c| c.change_id.clone()).collect();
                    let status = git::worktree_status(&entry.folder.uri, &change_ids);
                    worktrees.push(WorktreeSnapshot {
                        workspace: entry.folder.clone(),
                        branch,
                        active_changes,
                        archived_changes,
                        status,
                    });
                }

                ViewInput::Repo(RepoSnapshot {
                    repo_id,
                    main_worktree,
                    default_branch,
                    worktrees,
                })
            }
        })
        .collect();

    aggregate(inputs)
}

/// Diff the previous aggregated state against a new aggregated state and
/// return the [`CacheEvent`]s the frontend / consumers should hear about.
/// Pure function — no I/O. Used by the live system to emit logical-level
/// events after each re-aggregation, and by tests to verify the diff rules
/// directly.
///
/// Events emitted:
/// - [`CacheEvent::LogicalChangeAdded`] for a `(repo_id, change_name)` tuple
///   that did not exist anywhere in `old`.
/// - [`CacheEvent::LogicalChangeArchived`] for a tuple that had at least one
///   non-archived instance in `old` and has none in `new`.
/// - [`CacheEvent::InstanceAdded`] for an instance whose `worktree_path`
///   first appears for this tuple in `new`.
/// - [`CacheEvent::InstanceRemoved`] for an instance whose `worktree_path`
///   was present for this tuple in `old` and is absent in `new`.
///
/// Per-instance archive transitions (an instance moving from active to
/// archived in its worktree) are intentionally *not* surfaced as additional
/// events — only the logical-level archive event when *every* instance has
/// flipped.
pub fn diff_views(old: &[WorkspaceView], new: &[WorkspaceView]) -> Vec<CacheEvent> {
    let mut events = Vec::new();
    let old_map = index_logical_changes(old);
    let new_map = index_logical_changes(new);

    let all_keys: HashSet<&(PathBuf, String)> = old_map.keys().chain(new_map.keys()).collect();

    for key in all_keys {
        let old_state = old_map.get(key);
        let new_state = new_map.get(key);

        match (old_state, new_state) {
            (None, Some(new_lc)) => {
                events.push(CacheEvent::LogicalChangeAdded {
                    repo_id: key.0.clone(),
                    change_name: key.1.clone(),
                });
                for inst_path in &new_lc.instance_paths {
                    events.push(CacheEvent::InstanceAdded {
                        repo_id: key.0.clone(),
                        change_name: key.1.clone(),
                        worktree_path: inst_path.clone(),
                    });
                }
            }
            (Some(old_lc), None) => {
                // Logical change disappeared entirely — every instance was
                // removed (workspace unregistered or worktree pruned). Emit
                // InstanceRemoved for each. No LogicalChangeArchived because
                // the change isn't archived, just gone.
                for inst_path in &old_lc.instance_paths {
                    events.push(CacheEvent::InstanceRemoved {
                        repo_id: key.0.clone(),
                        change_name: key.1.clone(),
                        worktree_path: inst_path.clone(),
                    });
                }
            }
            (Some(old_lc), Some(new_lc)) => {
                // Diff instance paths.
                let added: Vec<&PathBuf> = new_lc
                    .instance_paths
                    .difference(&old_lc.instance_paths)
                    .collect();
                let removed: Vec<&PathBuf> = old_lc
                    .instance_paths
                    .difference(&new_lc.instance_paths)
                    .collect();
                for inst_path in added {
                    events.push(CacheEvent::InstanceAdded {
                        repo_id: key.0.clone(),
                        change_name: key.1.clone(),
                        worktree_path: inst_path.clone(),
                    });
                }
                for inst_path in removed {
                    events.push(CacheEvent::InstanceRemoved {
                        repo_id: key.0.clone(),
                        change_name: key.1.clone(),
                        worktree_path: inst_path.clone(),
                    });
                }
                // Active-to-archived transition (last active instance just
                // got archived).
                if old_lc.had_active_instance && !new_lc.had_active_instance {
                    events.push(CacheEvent::LogicalChangeArchived {
                        repo_id: key.0.clone(),
                        change_name: key.1.clone(),
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }

    events
}

struct LogicalState {
    instance_paths: HashSet<PathBuf>,
    had_active_instance: bool,
}

fn index_logical_changes(views: &[WorkspaceView]) -> HashMap<(PathBuf, String), LogicalState> {
    let mut out: HashMap<(PathBuf, String), LogicalState> = HashMap::new();
    for view in views {
        if let WorkspaceView::Repo(repo) = view {
            let active_iter = repo.active.iter().map(|lc| (lc, true));
            let archived_iter = repo.archived.iter().map(|lc| (lc, false));
            for (lc, in_active_section) in active_iter.chain(archived_iter) {
                let key = (repo.repo_id.clone(), lc.name.clone());
                let entry = out.entry(key).or_insert_with(|| LogicalState {
                    instance_paths: HashSet::new(),
                    had_active_instance: false,
                });
                if in_active_section {
                    entry.had_active_instance = true;
                }
                for inst in &lc.instances {
                    entry.instance_paths.insert(inst.worktree_path.clone());
                }
            }
        }
    }
    out
}

fn build_repo_view(snap: RepoSnapshot) -> RepoView {
    // Stage 1: collect per-(change_name, worktree_path) instances. Each
    // instance is either active or archived in its worktree — both flavours
    // contribute to the same logical change.
    //
    // BTreeMap keeps changes sorted by name in the output.
    let mut by_name: BTreeMap<String, Vec<ChangeInstance>> = BTreeMap::new();

    for wt in &snap.worktrees {
        let wt_path = wt.workspace.uri.clone();
        let is_main = wt_path == snap.main_worktree;
        let is_default = match (&wt.branch, &snap.default_branch) {
            (Some(b), Some(d)) => b == d,
            _ => false,
        };
        for change in &wt.active_changes {
            let modified_at = newest_mtime(
                &wt_path
                    .join("openspec")
                    .join("changes")
                    .join(&change.change_id),
            );
            by_name
                .entry(change.change_id.clone())
                .or_default()
                .push(ChangeInstance {
                    worktree_path: wt_path.clone(),
                    branch: wt.branch.clone(),
                    is_main_worktree: is_main,
                    is_default_branch: is_default,
                    is_archived_here: false,
                    change: change.clone(),
                    modified_at,
                    divergence: None,
                    spec_commit_state: wt.status.spec_state(&change.change_id),
                });
        }
        for change in &wt.archived_changes {
            let modified_at = newest_mtime(
                &wt_path
                    .join("openspec")
                    .join("changes")
                    .join("archive")
                    .join(&change.change_id),
            );
            by_name
                .entry(change.change_id.clone())
                .or_default()
                .push(ChangeInstance {
                    worktree_path: wt_path.clone(),
                    branch: wt.branch.clone(),
                    is_main_worktree: is_main,
                    is_default_branch: is_default,
                    is_archived_here: true,
                    change: change.clone(),
                    modified_at,
                    divergence: None,
                    // Archived instances carry no active-tree chip.
                    spec_commit_state: SpecCommitState::Committed,
                });
        }
    }

    // Stage 2: per logical change, compute divergence labels, sort instances
    // by mtime descending, and bucket into active/archived sections.
    let mut active = Vec::new();
    let mut archived = Vec::new();
    for (name, mut instances) in by_name {
        annotate_divergence(&mut instances);
        instances.sort_by_key(|i| std::cmp::Reverse(i.modified_at));
        let all_archived = instances.iter().all(|i| i.is_archived_here);
        let lc = LogicalChange { name, instances };
        if all_archived {
            archived.push(lc);
        } else {
            active.push(lc);
        }
    }

    let name = snap
        .main_worktree
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| snap.main_worktree.display().to_string());

    // Stage 3: roll the per-worktree status up to the repository. A worktree is
    // counted dirty from its whole-tree bit; specs are uncommitted when any
    // worktree classified at least one change as non-`Committed`.
    let dirty_worktrees: Vec<PathBuf> = snap
        .worktrees
        .iter()
        .filter(|wt| wt.status.dirty)
        .map(|wt| wt.workspace.uri.clone())
        .collect();
    let dirty = !dirty_worktrees.is_empty();
    let has_uncommitted_specs = snap
        .worktrees
        .iter()
        .any(|wt| !wt.status.spec_states.is_empty());

    RepoView {
        repo_id: snap.repo_id.into_path_buf(),
        main_worktree: snap.main_worktree,
        name,
        default_branch: snap.default_branch,
        active,
        archived,
        display_name: None,
        color: None,
        dirty,
        dirty_worktrees,
        has_uncommitted_specs,
    }
}

/// Set the `divergence` field on each non-default instance by comparing
/// against the (at most one) default-branch instance of the same logical
/// change.
fn annotate_divergence(instances: &mut [ChangeInstance]) {
    let default = instances.iter().find(|i| i.is_default_branch).cloned();
    let Some(default) = default else {
        // No default-branch reference exists; no labels possible.
        return;
    };
    for inst in instances.iter_mut() {
        if inst.is_default_branch {
            continue;
        }
        inst.divergence = compute_divergence(inst, &default);
    }
}

fn compute_divergence(
    instance: &ChangeInstance,
    default: &ChangeInstance,
) -> Option<DivergenceLabel> {
    match (instance.is_archived_here, default.is_archived_here) {
        (false, true) => Some(DivergenceLabel::StaleVsArchived),
        (true, _) => None, // Stale or archived-on-both — no label on archived rows.
        (false, false) => {
            let inst_dir = instance
                .worktree_path
                .join("openspec")
                .join("changes")
                .join(&instance.change.change_id);
            let default_dir = default
                .worktree_path
                .join("openspec")
                .join("changes")
                .join(&default.change.change_id);
            if dirs_differ(&inst_dir, &default_dir) {
                Some(DivergenceLabel::Diverged)
            } else {
                None
            }
        }
    }
}

/// Returns true if the directories differ at any file: differing file lists,
/// differing file sizes, or differing byte contents. Symlinks and special
/// files are treated as differing if they're not byte-identical.
fn dirs_differ(a: &std::path::Path, b: &std::path::Path) -> bool {
    let mut a_files = collect_relative_files(a);
    let mut b_files = collect_relative_files(b);
    if a_files.len() != b_files.len() {
        return true;
    }
    a_files.sort();
    b_files.sort();
    if a_files != b_files {
        return true;
    }
    for rel in a_files {
        let pa = a.join(&rel);
        let pb = b.join(&rel);
        let Ok(content_a) = std::fs::read(&pa) else {
            return true;
        };
        let Ok(content_b) = std::fs::read(&pb) else {
            return true;
        };
        if content_a != content_b {
            return true;
        }
    }
    false
}

fn collect_relative_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            walk(base, &path, out);
        } else if meta.is_file() {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

fn newest_mtime(dir: &std::path::Path) -> u64 {
    let mut newest: SystemTime = std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return to_unix(newest);
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if let Ok(mtime) = meta.modified() {
            if mtime > newest {
                newest = mtime;
            }
        }
        if meta.is_dir() {
            let sub = newest_mtime(&entry.path());
            let sub_st = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(sub);
            if sub_st > newest {
                newest = sub_st;
            }
        }
    }
    to_unix(newest)
}

fn to_unix(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ArtifactStatus, ChangeData, WorkspaceFolder};
    use std::fs;
    use tempfile::TempDir;

    fn make_change(
        id: &str,
        proposal: &str,
        ws: &WorkspaceFolder,
        completed: usize,
        total: usize,
    ) -> ChangeData {
        ChangeData {
            change_id: id.to_string(),
            title: Some(format!("Title of {id}")),
            sections: vec![],
            total_tasks: total,
            completed_tasks: completed,
            artifacts: ArtifactStatus {
                proposal: !proposal.is_empty(),
                specs: vec![],
                design: false,
                tasks: false,
            },
            workspace: ws.clone(),
        }
    }

    /// Builds a workspace folder rooted at `path`, materialising the change
    /// directories named in `actives` (active) and `archives` (archived)
    /// with a `proposal.md` so newest_mtime / divergence have something to
    /// look at.
    fn build_workspace(
        path: &std::path::Path,
        actives: &[(&str, &str)],
        archives: &[(&str, &str)],
    ) -> (WorkspaceFolder, Vec<ChangeData>, Vec<ChangeData>) {
        fs::create_dir_all(path.join("openspec/changes/archive")).unwrap();
        for (id, body) in actives {
            let d = path.join("openspec/changes").join(id);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("proposal.md"), body).unwrap();
        }
        for (id, body) in archives {
            let d = path.join("openspec/changes/archive").join(id);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("proposal.md"), body).unwrap();
        }
        let canonical = path.canonicalize().unwrap();
        let ws = WorkspaceFolder::from_path(canonical);
        let active = actives
            .iter()
            .map(|(id, p)| make_change(id, p, &ws, 0, 0))
            .collect();
        let archived = archives
            .iter()
            .map(|(id, p)| make_change(id, p, &ws, 0, 0))
            .collect();
        (ws, active, archived)
    }

    #[test]
    fn aggregate_preserves_interleaved_input_order() {
        // The load-bearing property: aggregate emits one row per input element,
        // in the given order, so repos and flats can interleave by config
        // position instead of "all repos then all flats".
        let tmp = TempDir::new().unwrap();
        let (flat0, changes0, _) = build_workspace(&tmp.path().join("flat0"), &[("c0", "x")], &[]);
        let (repo_ws, active, archived) =
            build_workspace(&tmp.path().join("repo"), &[("c1", "x")], &[]);
        let (flat2, changes2, _) = build_workspace(&tmp.path().join("flat2"), &[("c2", "x")], &[]);
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: repo_ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: repo_ws.clone(),
                branch: Some("main".into()),
                active_changes: active,
                archived_changes: archived,
                status: WorktreeStatus::clean(),
            }],
        };
        let views = aggregate(vec![
            ViewInput::Flat(flat0.clone(), changes0),
            ViewInput::Repo(snap),
            ViewInput::Flat(flat2.clone(), changes2),
        ]);
        assert_eq!(views.len(), 3);
        match &views[0] {
            WorkspaceView::Flat { workspace, .. } => assert_eq!(workspace.uri, flat0.uri),
            _ => panic!("expected flat at index 0"),
        }
        match &views[1] {
            WorkspaceView::Repo(r) => assert_eq!(r.main_worktree, repo_ws.uri),
            _ => panic!("expected repo at index 1"),
        }
        match &views[2] {
            WorkspaceView::Flat { workspace, .. } => assert_eq!(workspace.uri, flat2.uri),
            _ => panic!("expected flat at index 2"),
        }
    }

    #[test]
    fn compute_views_preserves_registration_order_deterministically() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let mk = |n: &str| {
            let p = tmp.path().join(n);
            fs::create_dir_all(p.join("openspec/changes")).unwrap();
            p
        };
        // Register three flat workspaces in a deliberately non-alphabetical order.
        let c = mk("ccc");
        let a = mk("aaa");
        let b = mk("bbb");
        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        reg.register(c.clone()).unwrap();
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();

        let cache = WorkspaceCache::new();
        let order_of = |reg: &WorkspaceRegistry| -> Vec<PathBuf> {
            compute_views(reg, &cache, |_| None)
                .into_iter()
                .map(|v| match v {
                    WorkspaceView::Flat { workspace, .. } => workspace.uri,
                    WorkspaceView::Repo(r) => r.repo_id,
                })
                .collect()
        };

        let expected = vec![
            c.canonicalize().unwrap(),
            a.canonicalize().unwrap(),
            b.canonicalize().unwrap(),
        ];
        // Top-level order follows registration (config) order, not alphabetical,
        // and is identical on every recomputation (no HashMap-seeding wobble).
        for _ in 0..8 {
            assert_eq!(order_of(&reg), expected);
        }
    }

    #[test]
    fn single_worktree_with_single_change_produces_one_logical_change() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) =
            build_workspace(&tmp.path().join("main"), &[("foo", "x")], &[]);
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: ws,
                branch: Some("main".into()),
                active_changes: active,
                archived_changes: archived,
                status: WorktreeStatus::clean(),
            }],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert_eq!(repo.active.len(), 1);
        assert_eq!(repo.active[0].name, "foo");
        assert_eq!(repo.active[0].instances.len(), 1);
        assert!(repo.active[0].instances[0].is_main_worktree);
        assert!(repo.active[0].instances[0].is_default_branch);
        assert!(repo.archived.is_empty());
    }

    #[test]
    fn two_worktrees_with_same_change_identical_content_no_divergence() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[("foo", "same")], &[]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "same")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        let foo = &repo.active[0];
        assert_eq!(foo.instances.len(), 2);
        let secondary = foo.instances.iter().find(|i| !i.is_default_branch).unwrap();
        assert_eq!(secondary.divergence, None);
    }

    #[test]
    fn two_worktrees_with_diverged_content_get_diverged_label() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[("foo", "v1")], &[]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "v2-different")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        let secondary = repo.active[0]
            .instances
            .iter()
            .find(|i| !i.is_default_branch)
            .unwrap();
        assert_eq!(secondary.divergence, Some(DivergenceLabel::Diverged));
    }

    #[test]
    fn change_archived_on_default_and_active_on_branch_gets_stale_label() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[], &[("foo", "merged")]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "stale-active")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        // The logical change is still active (one instance is active) so it
        // belongs in `active`.
        assert_eq!(repo.active.len(), 1);
        let secondary = repo.active[0]
            .instances
            .iter()
            .find(|i| !i.is_default_branch && !i.is_archived_here)
            .unwrap();
        assert_eq!(secondary.divergence, Some(DivergenceLabel::StaleVsArchived));
    }

    #[test]
    fn change_only_on_branch_has_no_divergence_label() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[], &[]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "branch-only")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        let only_inst = &repo.active[0].instances[0];
        assert_eq!(only_inst.divergence, None);
    }

    #[test]
    fn no_default_branch_means_no_labels_anywhere() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[("foo", "v1")], &[]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "v2")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: None,
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        for inst in &repo.active[0].instances {
            assert_eq!(inst.divergence, None);
        }
    }

    #[test]
    fn logical_change_with_all_archived_instances_goes_to_archived_section() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[], &[("foo", "merged")]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[], &[("foo", "merged")]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert!(repo.active.is_empty());
        assert_eq!(repo.archived.len(), 1);
        assert_eq!(repo.archived[0].name, "foo");
    }

    #[test]
    fn instances_are_sorted_by_modified_at_desc() {
        let tmp = TempDir::new().unwrap();
        let (ws_old, active_old, archived_old) =
            build_workspace(&tmp.path().join("old"), &[("foo", "x")], &[]);
        // Sleep so the second workspace's mtime is strictly newer.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let (ws_new, active_new, archived_new) =
            build_workspace(&tmp.path().join("new"), &[("foo", "x")], &[]);

        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_old.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_old,
                    branch: Some("main".into()),
                    active_changes: active_old,
                    archived_changes: archived_old,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_new.clone(),
                    branch: Some("feature".into()),
                    active_changes: active_new,
                    archived_changes: archived_new,
                    status: WorktreeStatus::clean(),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        let foo = &repo.active[0];
        assert_eq!(foo.instances[0].worktree_path, ws_new.uri);
    }

    #[test]
    fn diff_emits_logical_change_added_for_brand_new_change() {
        let repo_id = PathBuf::from("/r/.git");
        let new = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![dummy_instance("/r", "foo", false)],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let events = diff_views(&[], &new);
        assert!(events.contains(&CacheEvent::LogicalChangeAdded {
            repo_id: repo_id.clone(),
            change_name: "foo".into(),
        }));
        assert!(events.contains(&CacheEvent::InstanceAdded {
            repo_id,
            change_name: "foo".into(),
            worktree_path: PathBuf::from("/r"),
        }));
    }

    #[test]
    fn diff_does_not_emit_logical_change_added_for_existing_change_gaining_an_instance() {
        let repo_id = PathBuf::from("/r/.git");
        let old = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![dummy_instance("/r", "foo", false)],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![
                    dummy_instance("/r", "foo", false),
                    dummy_instance("/r/wt2", "foo", false),
                ],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let events = diff_views(&old, &new);
        assert!(!events
            .iter()
            .any(|e| matches!(e, CacheEvent::LogicalChangeAdded { .. })));
        assert!(events.contains(&CacheEvent::InstanceAdded {
            repo_id,
            change_name: "foo".into(),
            worktree_path: PathBuf::from("/r/wt2"),
        }));
    }

    #[test]
    fn diff_emits_logical_change_archived_only_when_last_active_instance_flips() {
        let repo_id = PathBuf::from("/r/.git");
        let old = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![dummy_instance("/r", "foo", false)],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![],
            archived: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![dummy_instance("/r", "foo", true)],
            }],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let events = diff_views(&old, &new);
        assert!(events.contains(&CacheEvent::LogicalChangeArchived {
            repo_id,
            change_name: "foo".into(),
        }));
    }

    #[test]
    fn diff_does_not_emit_archive_when_one_instance_archives_but_another_stays_active() {
        let repo_id = PathBuf::from("/r/.git");
        let old = vec![WorkspaceView::Repo(RepoView {
            repo_id: repo_id.clone(),
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![
                    dummy_instance("/r", "foo", false),
                    dummy_instance("/r/wt2", "foo", false),
                ],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            repo_id,
            main_worktree: PathBuf::from("/r"),
            name: "r".into(),
            default_branch: Some("main".into()),
            // /r still active, /r/wt2 archived → logical change is still active overall
            active: vec![LogicalChange {
                name: "foo".into(),
                instances: vec![
                    dummy_instance("/r", "foo", false),
                    dummy_instance("/r/wt2", "foo", true),
                ],
            }],
            archived: vec![],
            display_name: None,
            color: None,
            dirty: false,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        })];
        let events = diff_views(&old, &new);
        assert!(!events
            .iter()
            .any(|e| matches!(e, CacheEvent::LogicalChangeArchived { .. })));
    }

    fn status(dirty: bool, specs: &[(&str, SpecCommitState)]) -> WorktreeStatus {
        WorktreeStatus {
            dirty,
            spec_states: specs.iter().map(|(id, s)| (id.to_string(), *s)).collect(),
        }
    }

    #[test]
    fn instance_carries_spec_commit_state_and_sets_rollups() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) =
            build_workspace(&tmp.path().join("main"), &[("foo", "x")], &[]);
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: ws,
                branch: Some("main".into()),
                active_changes: active,
                archived_changes: archived,
                status: status(true, &[("foo", SpecCommitState::Untracked)]),
            }],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert_eq!(
            repo.active[0].instances[0].spec_commit_state,
            SpecCommitState::Untracked
        );
        assert!(repo.dirty);
        assert!(repo.has_uncommitted_specs);
        assert_eq!(repo.dirty_worktrees.len(), 1);
    }

    #[test]
    fn repo_dirty_from_non_spec_files_does_not_set_uncommitted_specs() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) =
            build_workspace(&tmp.path().join("main"), &[("foo", "x")], &[]);
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: ws,
                branch: Some("main".into()),
                active_changes: active,
                archived_changes: archived,
                // Dirty worktree, but no change directory is uncommitted.
                status: status(true, &[]),
            }],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert!(repo.dirty);
        assert!(!repo.has_uncommitted_specs);
        assert_eq!(
            repo.active[0].instances[0].spec_commit_state,
            SpecCommitState::Committed
        );
    }

    #[test]
    fn clean_repo_has_no_dirt_rollups() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) =
            build_workspace(&tmp.path().join("main"), &[("foo", "x")], &[]);
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: ws,
                branch: Some("main".into()),
                active_changes: active,
                archived_changes: archived,
                status: WorktreeStatus::clean(),
            }],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert!(!repo.dirty);
        assert!(!repo.has_uncommitted_specs);
        assert!(repo.dirty_worktrees.is_empty());
    }

    #[test]
    fn only_dirty_worktree_is_listed_in_rollup() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[("foo", "same")], &[]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("foo", "same")], &[]);
        let dirty_path = ws_b.uri.clone();
        let snap = RepoSnapshot {
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    active_changes: active_b,
                    archived_changes: archived_b,
                    status: status(true, &[("foo", SpecCommitState::Modified)]),
                },
            ],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };
        assert!(repo.dirty);
        assert!(repo.has_uncommitted_specs);
        assert_eq!(repo.dirty_worktrees, vec![dirty_path]);
    }

    fn dummy_instance(wt_path: &str, change_id: &str, archived: bool) -> ChangeInstance {
        let ws = WorkspaceFolder {
            uri: PathBuf::from(wt_path),
            name: "ws".into(),
        };
        ChangeInstance {
            worktree_path: PathBuf::from(wt_path),
            branch: Some("any".into()),
            is_main_worktree: false,
            is_default_branch: false,
            is_archived_here: archived,
            change: make_change(change_id, "x", &ws, 0, 0),
            modified_at: 0,
            divergence: None,
            spec_commit_state: SpecCommitState::Committed,
        }
    }

    #[test]
    fn flat_workspace_is_passed_through_untouched() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, _) = build_workspace(&tmp.path().join("flat"), &[("foo", "x")], &[]);
        let views = aggregate(vec![ViewInput::Flat(ws.clone(), active.clone())]);
        assert_eq!(views.len(), 1);
        let WorkspaceView::Flat {
            workspace, changes, ..
        } = &views[0]
        else {
            panic!()
        };
        assert_eq!(workspace, &ws);
        assert_eq!(changes, &active);
    }
}
