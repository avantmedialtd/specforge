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
use crate::presentation::PresentationKey;
use crate::registry::{WorkspaceOrigin, WorkspaceRegistry};
use crate::types::{ChangeData, PaletteColor, WorkspaceFolder};
use crate::watcher::CacheEvent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
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
        /// See [`RepoView::disabled`]. Never serialized — the IPC layer filters
        /// disabled rows out before any frontend sees the list.
        #[serde(default, skip_serializing)]
        disabled: bool,
    },
}

impl WorkspaceView {
    /// Whether the user has parked this top-level row. Disabled rows are still
    /// present in the aggregated snapshot (gathered cold, so the Dashboard's
    /// cache-derived figures stay whole) but are filtered out of the tree pane,
    /// the tray badge, and desktop notifications.
    pub fn is_disabled(&self) -> bool {
        match self {
            Self::Repo(r) => r.disabled,
            Self::Flat { disabled, .. } => *disabled,
        }
    }
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
    /// True when the user has parked this repository from the Settings view.
    ///
    /// A disabled row is aggregated *cold*: its cache-derived content (`active`,
    /// `archived`, task rollups) is exact, but every git-derived field above —
    /// `default_branch`, `dirty`, `dirty_worktrees`, `has_uncommitted_specs`,
    /// and each instance's `branch` / `is_default_branch` / `spec_commit_state`
    /// — holds its default rather than a value read from git. That is what lets
    /// the Dashboard keep counting a parked repository for free while the tree
    /// pane and tray badge drop it.
    ///
    /// Never serialized: the IPC layer filters disabled rows out, so no frontend
    /// ever receives one and can never mistake a defaulted field for a real
    /// clean/branchless repository.
    #[serde(default, skip_serializing)]
    pub disabled: bool,
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
    /// True when this snapshot was gathered cold — see [`RepoView::disabled`],
    /// which [`build_repo_view`] copies this into.
    pub cold: bool,
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
    Flat {
        workspace: WorkspaceFolder,
        changes: Vec<ChangeData>,
        disabled: bool,
    },
}

/// Aggregate pre-gathered snapshots into the views the frontend consumes,
/// preserving the order of `inputs`. Pure function — no I/O, no git invocations,
/// no global state.
pub fn aggregate(inputs: Vec<ViewInput>) -> Vec<WorkspaceView> {
    inputs
        .into_iter()
        .map(|input| match input {
            ViewInput::Repo(repo) => WorkspaceView::Repo(build_repo_view(repo)),
            ViewInput::Flat {
                workspace,
                changes,
                disabled,
            } => WorkspaceView::Flat {
                workspace,
                changes,
                display_name: None,
                color: None,
                disabled,
            },
        })
        .collect()
}

/// One top-level row's gathered inputs for a full recompute — no git I/O.
/// Mirrors [`ViewInput`]'s repo/flat split; produced by [`gather_views`] and
/// consumed by [`compute_views_from_gathered`].
pub enum GatheredInput {
    Repo(RepoGatherInput),
    Flat {
        workspace: WorkspaceFolder,
        changes: Vec<ChangeData>,
        disabled: bool,
    },
}

/// Gather phase for a full recompute: walk the registry in config order,
/// grouping worktrees by repository, and clone every input
/// [`compute_views_from_gathered`] needs — no git I/O. Locks (registry,
/// cache, and whatever `default_branch_for` reads) are only needed for the
/// duration of this call, never across the git invocations that follow.
///
/// `is_disabled` reports whether a top-level row has been parked by the user.
/// It is consulted here, in the gather phase, so a parked repository's row is
/// marked cold *before* [`compute_views_from_gathered`] builds its job list —
/// which is what keeps its git subprocesses from ever being spawned.
pub fn gather_views(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
    is_disabled: impl Fn(&PresentationKey) -> bool,
) -> Vec<GatheredInput> {
    // Build the top-level rows in config first-appearance order, interleaving
    // repo groups and flat workspaces. A repo claims its slot at the position of
    // its earliest user-registered worktree; later worktrees of the same repo
    // fold into that slot. Iteration over `registry.entries()` is in config
    // order (the registry is insertion-ordered), so the result is deterministic.
    enum Slot {
        Repo(RepoId),
        Flat(WorkspaceFolder, Vec<ChangeData>, bool),
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
                    let disabled = is_disabled(&PresentationKey::Flat(entry.folder.uri.clone()));
                    slots.push(Slot::Flat(entry.folder.clone(), changes, disabled));
                }
            }
        }
    }

    slots
        .into_iter()
        .map(|slot| match slot {
            Slot::Flat(workspace, changes, disabled) => GatheredInput::Flat {
                workspace,
                changes,
                disabled,
            },
            Slot::Repo(repo_id) => {
                let entries_in_repo = entries_by_repo.remove(&repo_id).unwrap_or_default();
                let cold = is_disabled(&PresentationKey::Repo(repo_id.as_path().to_path_buf()));
                GatheredInput::Repo(gather_repo_inputs(
                    repo_id,
                    entries_in_repo,
                    cache,
                    &default_branch_for,
                    cold,
                ))
            }
        })
        .collect()
}

/// Compute phase for a full recompute: perform every row's git I/O and
/// aggregate into the final views. No registry or cache lock held.
///
/// Unlike the scoped path's [`compute_repo_snapshot`] (one repo's worktrees
/// fanned out among up to [`MAX_CONCURRENT_WORKTREE_GIT`] workers), this
/// pools *every* repo row's work — one `worktree_list` job per repo, one
/// status/archived-stubs job per worktree, across every repo — into a
/// single flat job list run through one scoped thread pool, capped at
/// `min(available_parallelism, MAX_CONCURRENT_WORKTREE_GIT)` workers
/// *globally* (see [`compute_repo_rows_pooled`]). This matters because the
/// real registry this design targets (12 repos, 17 worktrees — ~1.4
/// worktrees per repo on average) has almost no parallelism to exploit
/// *within* any single repo: fanning out per-repo and processing repos
/// serially would barely use the worker cap and leave the measured
/// 576→179ms win unreachable. Pooling at the registry level is what
/// actually saturates it.
pub fn compute_views_from_gathered(gathered: Vec<GatheredInput>) -> Vec<WorkspaceView> {
    let total_slots = gathered.len();

    // Split repo rows (need git I/O) from flat rows (already complete, no
    // I/O), keeping each row's original slot index so the final
    // `Vec<ViewInput>` can be reassembled in config order regardless of the
    // pooled processing order below.
    let mut repo_rows: Vec<(usize, RepoGatherInput)> = Vec::new();
    let mut flat_rows: Vec<(usize, WorkspaceFolder, Vec<ChangeData>, bool)> = Vec::new();
    for (slot, g) in gathered.into_iter().enumerate() {
        match g {
            GatheredInput::Repo(input) => repo_rows.push((slot, input)),
            GatheredInput::Flat {
                workspace,
                changes,
                disabled,
            } => flat_rows.push((slot, workspace, changes, disabled)),
        }
    }

    let computed_repos = compute_repo_rows_pooled(repo_rows);

    let mut inputs: Vec<Option<ViewInput>> = (0..total_slots).map(|_| None).collect();
    for (slot, snapshot) in computed_repos {
        inputs[slot] = Some(ViewInput::Repo(snapshot));
    }
    for (slot, workspace, changes, disabled) in flat_rows {
        inputs[slot] = Some(ViewInput::Flat {
            workspace,
            changes,
            disabled,
        });
    }
    let inputs: Vec<ViewInput> = inputs
        .into_iter()
        .map(|slot| slot.expect("every slot filled by exactly one of repo_rows/flat_rows"))
        .collect();

    aggregate(inputs)
}

/// Full recompute in one call — [`gather_views`] followed immediately by
/// [`compute_views_from_gathered`]. Convenient for tests and any caller that
/// doesn't need the two phases split across a lock boundary; only the live
/// watcher does (`Inner::refresh_aggregated_view` in `watcher.rs`), so it
/// calls the split functions directly instead of this wrapper.
pub fn compute_views(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
    is_disabled: impl Fn(&PresentationKey) -> bool,
) -> Vec<WorkspaceView> {
    compute_views_from_gathered(gather_views(
        registry,
        cache,
        default_branch_for,
        is_disabled,
    ))
}

/// Owned inputs [`compute_repo_snapshot`] needs for one repository, gathered
/// from the registry and cache with no git I/O. Produced by
/// [`gather_repo_inputs`].
pub struct RepoGatherInput {
    repo_id: RepoId,
    default_branch: Option<String>,
    worktrees: Vec<WorktreeGatherInput>,
    /// True when the repository is disabled and must be computed without any
    /// git invocation. Set during gather so the compute phase never even builds
    /// the git jobs for this row.
    cold: bool,
}

/// Owned per-worktree inputs to [`compute_repo_snapshot`] — everything about
/// a worktree that comes from the registry/cache rather than git or the
/// filesystem (contrast [`WorktreeComputeResult`], resolved during compute).
struct WorktreeGatherInput {
    workspace: WorkspaceFolder,
    active_changes: Vec<ChangeData>,
}

/// Gather phase: clone a single repository's inputs out of its registry
/// entries and the cache — no I/O of any kind, so this only needs whatever
/// locks the caller holds for the duration of this call, never across the
/// git invocations (or the archived-stubs directory read — see
/// [`WorktreeComputeResult`]) the compute phase performs afterward. Shared
/// by [`gather_views`] (the full recompute) and [`gather_repo_view`] (the
/// scoped recompute) so the two paths cannot drift. `default_branch_for` is
/// invoked here too — it's a cheap `RepoMonitor` lock read, not a
/// subprocess, but resolving it during gather means that lock is also
/// released before any git call, rather than held open underneath the
/// registry/cache locks.
fn gather_repo_inputs(
    repo_id: RepoId,
    entries_in_repo: Vec<crate::registry::RegistryEntry>,
    cache: &WorkspaceCache,
    default_branch_for: &impl Fn(&RepoId) -> Option<String>,
    cold: bool,
) -> RepoGatherInput {
    // A cold row reports no default branch. Beyond matching the "git-derived
    // fields hold their defaults" contract, this is load-bearing: with no
    // default branch, no instance is tagged `is_default_branch`, so
    // `annotate_divergence` returns early and the recursive directory
    // comparisons in `dirs_differ` never run for a parked repository either.
    let default_branch = if cold {
        None
    } else {
        default_branch_for(&repo_id)
    };

    let worktrees = entries_in_repo
        .into_iter()
        .map(|entry| {
            let active_changes = cache.changes_for(&entry.folder.uri).to_vec();
            WorktreeGatherInput {
                workspace: entry.folder,
                active_changes,
            }
        })
        .collect();

    RepoGatherInput {
        repo_id,
        default_branch,
        worktrees,
        cold,
    }
}

/// Compute phase: given [`gather_repo_inputs`]'s owned output, perform the
/// repository's git I/O (`worktree_list` for the main worktree, then branch +
/// status per worktree, concurrently — see [`compute_worktree_snapshots`])
/// with no registry or cache lock held, and build the [`RepoSnapshot`] the
/// aggregator consumes. Used by the *scoped* (single-repository) recompute
/// path; the full-registry path uses [`compute_repo_rows_pooled`] instead,
/// which pools this same per-worktree work across every repo rather than
/// fanning out one repo at a time (see that function's doc comment).
pub(crate) fn compute_repo_snapshot(input: RepoGatherInput) -> RepoSnapshot {
    let main_worktree = resolve_main_worktree(&input.repo_id, input.cold);
    let worktrees = compute_worktree_snapshots(input.worktrees, input.cold);

    RepoSnapshot {
        repo_id: input.repo_id,
        main_worktree,
        default_branch: input.default_branch,
        worktrees,
        cold: input.cold,
    }
}

/// Determine a repository's main worktree from `git worktree list`. If the
/// call fails (e.g. the git binary went missing), fall back to
/// [`git::main_worktree_for_common_dir`] — not a heuristic but git's own
/// derivation, so it returns the very path `git worktree list` would have,
/// including for submodule, `--separate-git-dir` and bare layouts.
/// A `cold` row skips the `git worktree list` entirely and goes straight to that
/// fallback, so a parked repository's identity — `main_worktree`, the `name`
/// derived from it, and each instance's `is_main_worktree` — is byte-identical
/// to the identity it had while enabled, at no subprocess cost.
fn resolve_main_worktree(repo_id: &RepoId, cold: bool) -> PathBuf {
    if !cold {
        if let Some(path) = git::worktree_list(repo_id)
            .into_iter()
            .find(|wt| wt.is_main)
            .map(|wt| wt.path)
        {
            return path;
        }
    }
    git::main_worktree_for_common_dir(repo_id.as_path())
}

/// Upper bound on simultaneously outstanding git subprocesses in
/// [`compute_worktree_snapshots`] (per repo, for the scoped path) and
/// [`compute_repo_rows_pooled`] (across the whole registry, for the full
/// path). Measured on the real 17-worktree registry this design is
/// calibrated against: above 8 there was no further gain (the calls are
/// I/O-bound on process creation, not CPU), and an unbounded fan-out on a
/// large registry would spawn that many processes at once.
const MAX_CONCURRENT_WORKTREE_GIT: usize = 8;

/// What the compute phase resolves for one worktree beyond what gather
/// already owns: branch + status (one git spawn — see
/// [`git::worktree_branch_and_status`]) and the archived-change stubs (a
/// directory read, not git I/O, but still I/O the gather phase's
/// registry/cache locks must not be held across — see the `gather` phase's
/// "no I/O" contract on [`gather_repo_inputs`]).
struct WorktreeComputeResult {
    branch: Option<String>,
    status: WorktreeStatus,
    /// Cheap stubs (directory listing only) — enough for the logical diff
    /// to tell archived from deleted, without parsing the archive. The
    /// archive's content is loaded lazily by the Archive browser.
    archived_changes: Vec<ChangeData>,
}

/// Resolve one worktree's [`WorktreeComputeResult`]. Archived changes are
/// not classified against the branch/status (they live under `archive/`
/// and carry no active-tree chip).
/// A `cold` worktree skips the branch/status git spawn and reports the same
/// clean, branchless result the graceful-degradation path uses when git is
/// unavailable. The archived-stub directory read still happens either way — the
/// Dashboard's archived counts and today's ships depend on it, and it is a
/// `read_dir`, not a subprocess.
fn compute_worktree(wt: &WorktreeGatherInput, cold: bool) -> WorktreeComputeResult {
    let (branch, status) = if cold {
        (None, WorktreeStatus::clean())
    } else {
        let change_ids: Vec<String> = wt
            .active_changes
            .iter()
            .map(|c| c.change_id.clone())
            .collect();
        git::worktree_branch_and_status(&wt.workspace.uri, &change_ids)
    };
    let archived_changes = list_archived_stubs(&wt.workspace).unwrap_or_default();
    WorktreeComputeResult {
        branch,
        status,
        archived_changes,
    }
}

/// Compute every worktree's [`WorktreeComputeResult`] concurrently, bounded
/// to `min(available_parallelism, MAX_CONCURRENT_WORKTREE_GIT)` simultaneous
/// git subprocesses — each worktree's status is independent of every
/// other's, so there is no correctness reason to serialize them. The
/// *scoped* (single-repository) recompute's fan-out; the full-registry path
/// pools across all repos instead — see [`compute_repo_rows_pooled`].
///
/// Runs on a scoped thread pool (`std::thread::scope`, stable since 1.63, no
/// new dependency) rather than nested `spawn_blocking` calls: this already
/// executes inside the caller's single outer `spawn_blocking`
/// (`Inner::refresh_aggregated_view` in `watcher.rs`), and nesting the tokio
/// blocking pool inside itself risks starving it. `openspec-core` must also
/// keep computing views correctly with no tokio runtime present at all — it
/// is unit-tested that way — which a `spawn_blocking`-based fan-out could not
/// support.
///
/// Results are written back to `results[i]` by the worktree's original
/// index — never by completion order — so the returned `Vec` is
/// byte-identical to what a serial loop over `gathered` would produce,
/// worktree ordering included (the `Concurrent Per-Worktree Status
/// Invocation` requirement).
fn compute_worktree_snapshots(
    gathered: Vec<WorktreeGatherInput>,
    cold: bool,
) -> Vec<WorktreeSnapshot> {
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_CONCURRENT_WORKTREE_GIT);
    let chunks = index_chunks(gathered.len(), worker_count);

    let mut results: Vec<Option<WorktreeComputeResult>> =
        (0..gathered.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                let gathered = &gathered;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&i| (i, compute_worktree(&gathered[i], cold)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for (i, result) in handle.join().expect("worktree git worker panicked") {
                results[i] = Some(result);
            }
        }
    });

    gathered
        .into_iter()
        .zip(results)
        .map(|(wt, result)| {
            let result = result.expect("every worktree index is assigned to exactly one chunk");
            WorktreeSnapshot {
                workspace: wt.workspace,
                branch: result.branch,
                active_changes: wt.active_changes,
                archived_changes: result.archived_changes,
                status: result.status,
            }
        })
        .collect()
}

/// Compute every repo row's git I/O — one [`resolve_main_worktree`] job per
/// repo plus one [`compute_worktree`] job per worktree, across *all* rows —
/// via a single global scoped thread pool. See
/// [`compute_views_from_gathered`] for why this must be registry-wide
/// rather than per-repo.
///
/// Runs on a scoped thread pool (`std::thread::scope`) rather than nested
/// `spawn_blocking` calls, for the same reasons as the scoped path's
/// [`compute_worktree_snapshots`]: this already executes inside the
/// caller's single outer `spawn_blocking`, and `openspec-core` must keep
/// computing views correctly with no tokio runtime present (unit-tested
/// that way).
///
/// Results are written back by `(row, job)` index — never by completion
/// order — so the output is byte-identical to a serial computation,
/// worktree ordering included, matching every other concurrent path in this
/// module.
fn compute_repo_rows_pooled(
    repo_rows: Vec<(usize, RepoGatherInput)>,
) -> Vec<(usize, RepoSnapshot)> {
    enum Job<'a> {
        MainWorktree {
            row: usize,
            repo_id: &'a RepoId,
            cold: bool,
        },
        Worktree {
            row: usize,
            worktree: usize,
            input: &'a WorktreeGatherInput,
            cold: bool,
        },
    }
    enum JobResult {
        MainWorktree {
            row: usize,
            main_worktree: PathBuf,
        },
        Worktree {
            row: usize,
            worktree: usize,
            result: WorktreeComputeResult,
        },
    }

    let mut jobs: Vec<Job> = Vec::new();
    for (row, (_, input)) in repo_rows.iter().enumerate() {
        jobs.push(Job::MainWorktree {
            row,
            repo_id: &input.repo_id,
            cold: input.cold,
        });
        for (worktree, wt) in input.worktrees.iter().enumerate() {
            jobs.push(Job::Worktree {
                row,
                worktree,
                input: wt,
                cold: input.cold,
            });
        }
    }

    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_CONCURRENT_WORKTREE_GIT);
    let chunks = index_chunks(jobs.len(), worker_count);

    let mut main_worktrees: Vec<Option<PathBuf>> = (0..repo_rows.len()).map(|_| None).collect();
    let mut worktree_results: Vec<Vec<Option<WorktreeComputeResult>>> = repo_rows
        .iter()
        .map(|(_, input)| (0..input.worktrees.len()).map(|_| None).collect())
        .collect();

    std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                let jobs = &jobs;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&i| match &jobs[i] {
                            Job::MainWorktree { row, repo_id, cold } => JobResult::MainWorktree {
                                row: *row,
                                main_worktree: resolve_main_worktree(repo_id, *cold),
                            },
                            Job::Worktree {
                                row,
                                worktree,
                                input,
                                cold,
                            } => JobResult::Worktree {
                                row: *row,
                                worktree: *worktree,
                                result: compute_worktree(input, *cold),
                            },
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for result in handle.join().expect("worktree git worker panicked") {
                match result {
                    JobResult::MainWorktree { row, main_worktree } => {
                        main_worktrees[row] = Some(main_worktree);
                    }
                    JobResult::Worktree {
                        row,
                        worktree,
                        result,
                    } => {
                        worktree_results[row][worktree] = Some(result);
                    }
                }
            }
        }
    });

    repo_rows
        .into_iter()
        .zip(main_worktrees)
        .zip(worktree_results)
        .map(|(((slot, input), main_worktree), row_results)| {
            let main_worktree =
                main_worktree.expect("every repo row is assigned exactly one MainWorktree job");
            let worktrees = input
                .worktrees
                .into_iter()
                .zip(row_results)
                .map(|(wt, result)| {
                    let result = result.expect("every worktree is assigned exactly one job");
                    WorktreeSnapshot {
                        workspace: wt.workspace,
                        branch: result.branch,
                        active_changes: wt.active_changes,
                        archived_changes: result.archived_changes,
                        status: result.status,
                    }
                })
                .collect();
            (
                slot,
                RepoSnapshot {
                    repo_id: input.repo_id,
                    main_worktree,
                    default_branch: input.default_branch,
                    worktrees,
                    cold: input.cold,
                },
            )
        })
        .collect()
}

/// Split `0..len` into up to `workers` contiguous, non-empty, roughly-equal
/// index chunks for the scoped worker pools in [`compute_worktree_snapshots`]
/// and [`compute_repo_rows_pooled`]. Never more chunks than `len` (an idle
/// worker thread has nothing to gain).
fn index_chunks(len: usize, workers: usize) -> Vec<Vec<usize>> {
    let workers = workers.clamp(1, len.max(1));
    let base = len / workers;
    let extra = len % workers;
    let mut out = Vec::with_capacity(workers);
    let mut start = 0;
    for w in 0..workers {
        let size = base + usize::from(w < extra);
        if size == 0 {
            continue;
        }
        out.push((start..start + size).collect());
        start += size;
    }
    out
}

/// Gather phase for a scoped (single-repository) recompute. `None` when
/// `repo_id` has no entries in the registry — the caller should fall back to
/// a full recompute (the repo is appearing for the first time).
pub fn gather_repo_view(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    repo_id: &RepoId,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
    is_disabled: impl Fn(&PresentationKey) -> bool,
) -> Option<RepoGatherInput> {
    // Preserve registry (config) order for this repo's worktrees so the result
    // is byte-identical to the repo's slot in a full recompute.
    let entries_in_repo: Vec<crate::registry::RegistryEntry> = registry
        .entries()
        .into_iter()
        .filter(|e| e.repo_id.as_ref() == Some(repo_id))
        .collect();
    if entries_in_repo.is_empty() {
        return None;
    }
    let cold = is_disabled(&PresentationKey::Repo(repo_id.as_path().to_path_buf()));
    Some(gather_repo_inputs(
        repo_id.clone(),
        entries_in_repo,
        cache,
        &default_branch_for,
        cold,
    ))
}

/// Recompute a single repository's [`RepoView`] — the scoped counterpart of
/// [`compute_views`]. Runs git I/O *only* for `repo_id`'s worktrees, never for
/// any other registered repository. Returns `None` when `repo_id` has no tracked
/// worktrees in the registry (the caller should fall back to a full recompute,
/// since the repo is not yet in the snapshot). Gathers then computes back to
/// back; `Inner::refresh_aggregated_view_for` in `watcher.rs` uses the split
/// [`gather_repo_view`] / [`compute_repo_snapshot`] directly instead, so it can
/// release its locks before the git I/O.
pub fn compute_repo_view(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    repo_id: &RepoId,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
    is_disabled: impl Fn(&PresentationKey) -> bool,
) -> Option<RepoView> {
    let input = gather_repo_view(registry, cache, repo_id, default_branch_for, is_disabled)?;
    Some(build_repo_view(compute_repo_snapshot(input)))
}

/// Replace the [`WorkspaceView::Repo`] whose `repo_id` matches `new_view`'s in
/// `views`, in place, preserving the position (and therefore the config order)
/// of every other entry. Returns `true` if a matching repo was found and
/// replaced; `false` if no repo with that id is present — in which case the
/// caller should fall back to a full recompute (the repo is new to the snapshot).
///
/// This is the splice that lets a repo-scoped recompute update a single
/// repository's view without rebuilding the whole aggregated snapshot.
pub fn replace_repo_view(views: &mut [WorkspaceView], new_view: RepoView) -> bool {
    for view in views.iter_mut() {
        if let WorkspaceView::Repo(existing) = view {
            if existing.repo_id == new_view.repo_id {
                *view = WorkspaceView::Repo(new_view);
                return true;
            }
        }
    }
    false
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

pub(crate) fn build_repo_view(snap: RepoSnapshot) -> RepoView {
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
        disabled: snap.cold,
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

    fn minimal_repo_view(id: &std::path::Path, dirty: bool) -> RepoView {
        RepoView {
            disabled: false,
            repo_id: id.to_path_buf(),
            main_worktree: id.to_path_buf(),
            name: "r".into(),
            default_branch: None,
            active: vec![],
            archived: vec![],
            display_name: None,
            color: None,
            dirty,
            dirty_worktrees: vec![],
            has_uncommitted_specs: false,
        }
    }

    #[test]
    fn replace_repo_view_swaps_matching_repo_in_place_preserving_order() {
        let a = PathBuf::from("/repo/a/.git");
        let b = PathBuf::from("/repo/b/.git");
        let flat = WorkspaceFolder::from_path(PathBuf::from("/flat"));
        let mut views = vec![
            WorkspaceView::Flat {
                disabled: false,
                workspace: flat,
                changes: vec![],
                display_name: None,
                color: None,
            },
            WorkspaceView::Repo(minimal_repo_view(&a, false)),
            WorkspaceView::Repo(minimal_repo_view(&b, false)),
        ];

        let replaced = replace_repo_view(&mut views, minimal_repo_view(&a, true));

        assert!(replaced);
        assert_eq!(views.len(), 3);
        assert!(matches!(&views[0], WorkspaceView::Flat { .. }));
        match &views[1] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, a);
                assert!(r.dirty, "repo a should now be dirty");
            }
            _ => panic!("expected repo a at index 1"),
        }
        match &views[2] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, b);
                assert!(!r.dirty, "repo b must be left untouched");
            }
            _ => panic!("expected repo b at index 2"),
        }
    }

    #[test]
    fn replace_repo_view_returns_false_when_repo_absent() {
        let a = PathBuf::from("/repo/a/.git");
        let c = PathBuf::from("/repo/c/.git");
        let mut views = vec![WorkspaceView::Repo(minimal_repo_view(&a, false))];

        let replaced = replace_repo_view(&mut views, minimal_repo_view(&c, true));

        assert!(!replaced);
        assert_eq!(views.len(), 1);
        match &views[0] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, a);
                assert!(!r.dirty);
            }
            _ => panic!("expected repo a unchanged"),
        }
    }

    fn run_git(args: &[&str], cwd: &std::path::Path) {
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

    fn init_git_openspec(root: &std::path::Path) -> PathBuf {
        fs::create_dir_all(root.join("openspec/changes")).unwrap();
        run_git(&["init", "-b", "main"], root);
        run_git(&["config", "user.email", "t@t"], root);
        run_git(&["config", "user.name", "t"], root);
        run_git(&["commit", "--allow-empty", "-m", "init"], root);
        root.canonicalize().unwrap()
    }

    #[test]
    fn compute_repo_view_equals_the_repo_slot_of_a_full_recompute() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let a = init_git_openspec(&tmp.path().join("a"));
        let b = init_git_openspec(&tmp.path().join("b"));
        // Make repo A dirty with an untracked file; leave B clean.
        let foo = a.join("openspec/changes/foo");
        fs::create_dir_all(&foo).unwrap();
        fs::write(foo.join("proposal.md"), "x").unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(a.clone()).unwrap();
        reg.register(b.clone()).unwrap();
        let cache = WorkspaceCache::new();

        let repo_id_a = reg.entry(&a).unwrap().repo_id.clone().unwrap();
        let repo_id_b = reg.entry(&b).unwrap().repo_id.clone().unwrap();

        let full = compute_views(&reg, &cache, |_| None, |_| false);
        let slot = |id: &RepoId| -> RepoView {
            full.iter()
                .find_map(|v| match v {
                    WorkspaceView::Repo(r) if r.repo_id.as_path() == id.as_path() => {
                        Some(r.clone())
                    }
                    _ => None,
                })
                .expect("repo slot present in full recompute")
        };
        let slot_a = slot(&repo_id_a);
        let slot_b = slot(&repo_id_b);

        // Scoped recompute of each repo equals that repo's slot in the full recompute.
        assert_eq!(
            compute_repo_view(&reg, &cache, &repo_id_a, |_| None, |_| false),
            Some(slot_a.clone())
        );
        assert_eq!(
            compute_repo_view(&reg, &cache, &repo_id_b, |_| None, |_| false),
            Some(slot_b.clone())
        );
        // And the rollup matches the on-disk truth: A dirty, B clean.
        assert!(slot_a.dirty, "A has an untracked file");
        assert!(!slot_b.dirty, "B is clean");
    }

    #[test]
    fn compute_repo_view_is_none_for_an_unregistered_repo() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let a = init_git_openspec(&tmp.path().join("a"));
        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(a.clone()).unwrap();
        let cache = WorkspaceCache::new();
        let bogus = RepoId(tmp.path().join("nope/.git"));
        assert_eq!(
            compute_repo_view(&reg, &cache, &bogus, |_| None, |_| false),
            None
        );
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
            cold: false,
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
            ViewInput::Flat {
                workspace: flat0.clone(),
                changes: changes0,
                disabled: false,
            },
            ViewInput::Repo(snap),
            ViewInput::Flat {
                workspace: flat2.clone(),
                changes: changes2,
                disabled: false,
            },
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
            compute_views(reg, &cache, |_| None, |_| false)
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
    fn compute_views_concurrent_recompute_is_deterministic_across_many_worktrees() {
        // `Concurrent Per-Worktree Status Invocation`: repeated recomputes
        // over a repo with enough worktrees to exercise the concurrent
        // per-worktree fan-out (`compute_worktree_snapshots`'s
        // `std::thread::scope` pool, not the degenerate 0/1-worktree path)
        // must produce byte-identical `Vec<WorkspaceView>` output every
        // time, worktree ordering included — results are written back by
        // index, never by completion order, so this must hold regardless of
        // which worker finishes first on any given run.
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let root = init_git_openspec(&tmp.path().join("repo"));

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        reg.register(root.clone()).unwrap();
        let mut cache = WorkspaceCache::new();
        const WORKTREES: usize = 12;
        for i in 0..WORKTREES {
            let wt = tmp.path().join(format!("wt{i}"));
            run_git(
                &[
                    "worktree",
                    "add",
                    "-b",
                    &format!("b{i}"),
                    wt.to_str().unwrap(),
                ],
                &root,
            );
            let change_id = format!("change-{i}");
            let change_dir = wt.join("openspec/changes").join(&change_id);
            fs::create_dir_all(&change_dir).unwrap();
            fs::write(change_dir.join("proposal.md"), format!("# change {i}\n")).unwrap();
            let wt_canonical = wt.canonicalize().unwrap();
            reg.register(wt_canonical.clone()).unwrap();
            // The aggregator reads changes from the cache, not the
            // filesystem — populate it the way the real watcher would after
            // parsing (`cache.insert` is what `handle_events` calls after
            // `parse_all_changes`).
            let ws = WorkspaceFolder::from_path(wt_canonical.clone());
            cache.insert(wt_canonical, vec![make_change(&change_id, "x", &ws, 0, 0)]);
        }

        let first = compute_views(&reg, &cache, |_| None, |_| false);

        // Sanity: this is exercising real aggregation across all worktrees,
        // not comparing empty/trivial output.
        let WorkspaceView::Repo(repo) = &first[0] else {
            panic!("expected a single Repo view");
        };
        assert_eq!(
            repo.active.len(),
            WORKTREES,
            "expected one logical change per added worktree"
        );

        for _ in 0..8 {
            let next = compute_views(&reg, &cache, |_| None, |_| false);
            assert_eq!(
                next, first,
                "concurrent recompute must be deterministic across repeated runs"
            );
        }
    }

    #[test]
    fn compute_views_pools_git_io_correctly_across_many_repos() {
        // The full-recompute path pools EVERY repo row's git I/O into one
        // flat job list processed by a single worker pool (rather than
        // fanning out one repo at a time) — see `compute_repo_rows_pooled`.
        // A registry with several small repos is exactly the shape that
        // restructure targets (the real registry this design is calibrated
        // against averages ~1.4 worktrees per repo), and it's also the shape
        // most likely to expose a `(row, worktree)` index mixup in the
        // pooled job/result bookkeeping — e.g. repo B's worktree ending up
        // with repo A's status, or a repo's `main_worktree` resolving to the
        // wrong repo. Assert both determinism AND per-repo correctness.
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("workspaces.json"));
        let mut cache = WorkspaceCache::new();
        const REPOS: usize = 5;
        const WORKTREES_PER_REPO: usize = 3;
        let mut expected_main: HashMap<PathBuf, PathBuf> = HashMap::new();

        for r in 0..REPOS {
            let root = init_git_openspec(&tmp.path().join(format!("repo{r}")));
            reg.register(root.clone()).unwrap();
            let repo_id = reg.entry(&root).unwrap().repo_id.clone().unwrap();
            expected_main.insert(repo_id.clone().into_path_buf(), root.clone());

            // One change directly in the main worktree, uniquely named per
            // repo so a cross-repo mixup is detectable.
            let change_id = format!("repo{r}-main-change");
            let change_dir = root.join("openspec/changes").join(&change_id);
            fs::create_dir_all(&change_dir).unwrap();
            fs::write(change_dir.join("proposal.md"), "x").unwrap();
            let ws = WorkspaceFolder::from_path(root.clone());
            cache.insert(root.clone(), vec![make_change(&change_id, "x", &ws, 0, 0)]);

            for w in 1..WORKTREES_PER_REPO {
                let wt = tmp.path().join(format!("repo{r}-wt{w}"));
                run_git(
                    &[
                        "worktree",
                        "add",
                        "-b",
                        &format!("repo{r}-b{w}"),
                        wt.to_str().unwrap(),
                    ],
                    &root,
                );
                let wt_canonical = wt.canonicalize().unwrap();
                let wt_change_id = format!("repo{r}-wt{w}-change");
                let wt_change_dir = wt_canonical.join("openspec/changes").join(&wt_change_id);
                fs::create_dir_all(&wt_change_dir).unwrap();
                fs::write(wt_change_dir.join("proposal.md"), "x").unwrap();
                // `register` requires an `openspec/` subdir to already
                // exist, so the change directory above must be created
                // before this call.
                reg.register(wt_canonical.clone()).unwrap();
                let wt_ws = WorkspaceFolder::from_path(wt_canonical.clone());
                cache.insert(
                    wt_canonical,
                    vec![make_change(&wt_change_id, "x", &wt_ws, 0, 0)],
                );
            }
        }

        let first = compute_views(&reg, &cache, |_| None, |_| false);
        assert_eq!(first.len(), REPOS, "expected one Repo view per repo");

        for view in &first {
            let WorkspaceView::Repo(repo) = view else {
                panic!("expected every view to be a Repo view: {view:?}");
            };
            let expected = expected_main
                .get(&repo.repo_id)
                .unwrap_or_else(|| panic!("unexpected repo_id in output: {:?}", repo.repo_id));
            assert_eq!(
                &repo.main_worktree, expected,
                "repo {:?} resolved the wrong main_worktree — a (row, job) index mixup \
                 in the pooled compute phase",
                repo.repo_id
            );
            assert_eq!(
                repo.active.len(),
                WORKTREES_PER_REPO,
                "repo {:?} should have exactly one logical change per its own worktree, \
                 no more and no fewer — a mismatched count suggests worktrees leaked \
                 across repos in the pooled job list",
                repo.repo_id
            );
            // Every logical change name for this repo must be prefixed with
            // this repo's own slot — catches a subtler mixup where the
            // count matches by coincidence but the content came from a
            // different repo's worktree.
            let repo_prefix = repo
                .main_worktree
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| format!("{s}-"))
                .unwrap();
            for lc in &repo.active {
                assert!(
                    lc.name.starts_with(&repo_prefix),
                    "change {:?} under repo {:?} does not carry that repo's own prefix — \
                     cross-repo data mixup in the pooled compute phase",
                    lc.name,
                    repo.repo_id
                );
            }
        }

        for _ in 0..8 {
            let next = compute_views(&reg, &cache, |_| None, |_| false);
            assert_eq!(
                next, first,
                "pooled multi-repo recompute must be deterministic across repeated runs"
            );
        }
    }

    #[test]
    fn single_worktree_with_single_change_produces_one_logical_change() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) =
            build_workspace(&tmp.path().join("main"), &[("foo", "x")], &[]);
        let snap = RepoSnapshot {
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            disabled: false,
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
            disabled: false,
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
            disabled: false,
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
            disabled: false,
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
            disabled: false,
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
            disabled: false,
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
            disabled: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
            cold: false,
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
        let views = aggregate(vec![ViewInput::Flat {
            workspace: ws.clone(),
            changes: active.clone(),
            disabled: false,
        }]);
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

    // ---------------------------------------------------------------------
    // Cold aggregation of disabled rows.
    //
    // The contract has two halves and both need asserting: a cold row's
    // cache-derived content is *exact* (the Dashboard keeps counting a parked
    // workspace) while its git-derived fields hold *defaults* (nothing stale
    // leaks). Asserting only the first half would let an inverted cold check
    // pass unnoticed.
    // ---------------------------------------------------------------------

    /// A predicate that parks exactly `repo_id`'s top-level row.
    fn parks(repo_id: &RepoId) -> impl Fn(&PresentationKey) -> bool {
        let key = PresentationKey::Repo(repo_id.as_path().to_path_buf());
        move |k: &PresentationKey| k == &key
    }

    /// One dirty git repo with two active changes in the cache and one
    /// archived change on disk. Returns `(registry, cache, repo_id)`.
    fn parkable_repo(tmp: &TempDir) -> (WorkspaceRegistry, WorkspaceCache, RepoId, PathBuf) {
        let root = init_git_openspec(&tmp.path().join("repo"));
        let (ws, active, _) = build_workspace(&root, &[("alpha", "a"), ("beta", "b")], &[]);
        // An archived stub on disk, so `archived` is non-empty via read_dir.
        let arch = root.join("openspec/changes/archive/2026-01-01-gamma");
        fs::create_dir_all(&arch).unwrap();
        fs::write(arch.join("proposal.md"), "g").unwrap();
        // Leave an untracked file so `git status` would report the repo dirty.
        fs::write(root.join("untracked.txt"), "dirty").unwrap();

        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(root.clone()).unwrap();
        let mut cache = WorkspaceCache::new();
        // Give the changes real task counts so the rollup is worth asserting.
        let active: Vec<ChangeData> = active
            .into_iter()
            .enumerate()
            .map(|(i, mut c)| {
                c.completed_tasks = i + 1;
                c.total_tasks = 4;
                c
            })
            .collect();
        cache.insert(ws.uri.clone(), active);
        let repo_id = reg.entry(&root).unwrap().repo_id.clone().unwrap();
        (reg, cache, repo_id, root)
    }

    #[test]
    fn a_cold_repo_row_keeps_its_cache_derived_counts() {
        let tmp = TempDir::new().unwrap();
        let (reg, cache, repo_id, _root) = parkable_repo(&tmp);

        let warm = compute_views(&reg, &cache, |_| None, |_| false);
        let cold = compute_views(&reg, &cache, |_| None, parks(&repo_id));

        let (WorkspaceView::Repo(warm), WorkspaceView::Repo(cold)) = (&warm[0], &cold[0]) else {
            panic!("expected a repo row in both computations");
        };

        assert!(!warm.disabled);
        assert!(cold.disabled, "the parked row is flagged");

        // The half that must be exact.
        assert_eq!(cold.active.len(), warm.active.len(), "active count");
        assert_eq!(cold.active.len(), 2);
        assert_eq!(cold.archived.len(), warm.archived.len(), "archived count");
        assert_eq!(cold.archived.len(), 1);
        assert_eq!(
            cold.active
                .iter()
                .map(|lc| lc.name.clone())
                .collect::<Vec<_>>(),
            warm.active
                .iter()
                .map(|lc| lc.name.clone())
                .collect::<Vec<_>>(),
        );
        // The same "primary instance" rollup the Dashboard's summary metrics do.
        let rollup = |v: &RepoView| -> (usize, usize) {
            v.active
                .iter()
                .filter_map(|lc| lc.instances.first())
                .fold((0, 0), |(c, t), inst| {
                    (c + inst.change.completed_tasks, t + inst.change.total_tasks)
                })
        };
        assert_eq!(
            rollup(cold),
            rollup(warm),
            "task rollup must survive parking"
        );
        assert_eq!(rollup(cold), (3, 8));
        assert_eq!(cold.name, warm.name, "display name is resolved without git");
        assert_eq!(cold.repo_id, warm.repo_id);
    }

    #[test]
    fn a_cold_repo_row_defaults_its_git_derived_fields() {
        let tmp = TempDir::new().unwrap();
        let (reg, cache, repo_id, _root) = parkable_repo(&tmp);

        let warm = compute_views(&reg, &cache, |_| Some("main".into()), |_| false);
        let cold = compute_views(&reg, &cache, |_| Some("main".into()), parks(&repo_id));

        let (WorkspaceView::Repo(warm), WorkspaceView::Repo(cold)) = (&warm[0], &cold[0]) else {
            panic!("expected a repo row in both computations");
        };

        // Control: warm really does observe the dirty tree and a branch, so the
        // assertions below are testing the cold path rather than an empty repo.
        assert!(warm.dirty, "control: the fixture repo is dirty when warm");
        assert!(!warm.dirty_worktrees.is_empty());
        assert_eq!(warm.default_branch.as_deref(), Some("main"));
        assert!(warm.active[0].instances[0].branch.is_some());

        assert!(!cold.dirty, "a parked row reports no dirty rollup");
        assert!(cold.dirty_worktrees.is_empty());
        assert!(!cold.has_uncommitted_specs);
        assert_eq!(cold.default_branch, None, "no default branch when parked");
        for lc in &cold.active {
            for inst in &lc.instances {
                assert_eq!(inst.branch, None, "no branch resolved for a parked row");
                assert!(!inst.is_default_branch);
                assert_eq!(inst.spec_commit_state, SpecCommitState::Committed);
                assert_eq!(inst.divergence, None);
            }
        }
    }

    #[test]
    fn a_cold_row_keeps_its_config_position() {
        let tmp = TempDir::new().unwrap();
        let flat_a = tmp.path().join("aaa");
        let flat_b = tmp.path().join("bbb");
        fs::create_dir_all(flat_a.join("openspec/changes")).unwrap();
        fs::create_dir_all(flat_b.join("openspec/changes")).unwrap();
        let repo = init_git_openspec(&tmp.path().join("repo"));

        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(flat_a.clone()).unwrap();
        reg.register(repo.clone()).unwrap();
        reg.register(flat_b.clone()).unwrap();
        let cache = WorkspaceCache::new();
        let repo_id = reg.entry(&repo).unwrap().repo_id.clone().unwrap();

        let ids = |views: &[WorkspaceView]| -> Vec<PathBuf> {
            views
                .iter()
                .map(|v| match v {
                    WorkspaceView::Repo(r) => r.repo_id.clone(),
                    WorkspaceView::Flat { workspace, .. } => workspace.uri.clone(),
                })
                .collect()
        };

        let warm = compute_views(&reg, &cache, |_| None, |_| false);
        let cold = compute_views(&reg, &cache, |_| None, parks(&repo_id));
        assert_eq!(
            ids(&cold),
            ids(&warm),
            "parking a row must not move it, or anything else"
        );
        assert_eq!(cold.len(), 3);
        assert!(cold[1].is_disabled(), "the middle row is the parked repo");
        assert!(!cold[0].is_disabled());
        assert!(!cold[2].is_disabled());
    }

    #[test]
    fn a_disabled_flat_workspace_is_flagged_but_otherwise_identical() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("flat");
        let (ws, active, _) = build_workspace(&path, &[("c1", "x")], &[]);
        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(path).unwrap();
        let mut cache = WorkspaceCache::new();
        cache.insert(ws.uri.clone(), active.clone());

        let key = PresentationKey::Flat(ws.uri.clone());
        let views = compute_views(&reg, &cache, |_| None, |k| k == &key);
        assert_eq!(views.len(), 1);
        let WorkspaceView::Flat {
            workspace,
            changes,
            disabled,
            ..
        } = &views[0]
        else {
            panic!("expected a flat row");
        };
        assert!(disabled, "the flat row is flagged");
        assert_eq!(workspace.uri, ws.uri);
        assert_eq!(
            changes, &active,
            "a flat row does no git work, so parking changes nothing but the flag"
        );
    }
}
