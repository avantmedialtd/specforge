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
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Top-level entry the frontend renders. Either a git-backed repository
/// with logical changes aggregated across its worktrees, or a standalone
/// non-git workspace rendered flat as before.
///
/// `display_name` and `color` on both variants are populated by the IPC
/// layer from the presentation store after aggregation; the pure aggregator
/// always leaves them `None`.
///
/// `rename_all_fields` is load-bearing, not decorative: on an *enum*,
/// `rename_all` renames the variants, so without it `Flat`'s `display_name`
/// goes out as `display_name` while `src/types.ts` reads `displayName`, and
/// every flat workspace's name override is silently dropped. Covered by
/// `tests/wire_shape.rs` here, and by the cross-crate camelCase guard in
/// `openspec-app/tests/wire_shape.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceView {
    Repo(RepoView),
    Flat {
        workspace: WorkspaceFolder,
        changes: Vec<ChangeData>,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        color: Option<PaletteColor>,
        /// See [`RepoView::has_open_spec`]; for a flat workspace, whether its
        /// own folder holds `openspec/`.
        #[serde(default)]
        has_open_spec: bool,
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
    /// loads them lazily and per top-level row — so the instances here carry
    /// stub `ChangeData` with no parsed content.
    ///
    /// Holds every logical change with at least one archived instance, and
    /// only its archived instances. A change archived in one worktree while
    /// still active in another is one logical change with a mixed instance
    /// set, and [`build_repo_view`] partitions it by flavour: its active
    /// instances go to `active` and its archived ones here, under the same
    /// name. `crate::dashboard`'s `repo_ships` therefore scans this section
    /// alone. Instances here are ordered by archive date, newest first (see
    /// [`ChangeInstance::modified_at`]).
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
    /// Every tracked worktree of the repository — user-registered *and*
    /// registry-discovered — in snapshot order.
    ///
    /// Serialized because it is the frontend's ONLY sight of a discovered
    /// worktree: `active` carries only worktrees hosting an active change, and
    /// `list_workspaces` returns only user-registered folders, so neither pool
    /// contains the worktree a change was archived from once its branch hosts
    /// no active work — which is exactly the worktree a today's-ships link
    /// names and the Archive view has to scope to.
    #[serde(default)]
    pub worktrees: Vec<PathBuf>,
    /// True when any tracked worktree holds an `openspec/` subdirectory. A
    /// repository registered without OpenSpec reports `false` until one of its
    /// worktrees gains the folder. Stamped from the filesystem after
    /// aggregation by [`stamp_openspec_presence`] — a `stat` per worktree, no
    /// git, so a cold row carries it too — and never persisted anywhere.
    #[serde(default)]
    pub has_open_spec: bool,
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
    /// Every tracked worktree's branch and upstream, in snapshot order — the
    /// branch of a worktree hosting no active change included, which
    /// `active` cannot carry. Read by the application layer's pull-request
    /// join (`pull-request-worktree-links`: *Matching a Pull Request to a
    /// Worktree*); never serialized, so the wire shape is unchanged.
    #[serde(default, skip_serializing)]
    pub worktree_refs: Vec<WorktreeRef>,
}

/// One tracked worktree's branch and the upstream it tracks, as the
/// repository view carries them for the pull-request join.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRef {
    pub path: PathBuf,
    /// `None` for a detached HEAD, or a cold (disabled) repository.
    pub branch: Option<String>,
    /// The raw `<remote>/<branch>` upstream, `None` when untracked.
    pub upstream: Option<String>,
}

/// A change identified by `(repo_id, change_name)`, with one entry per
/// worktree that contains it. Active instances are ordered by
/// most-recently-modified first; the first is the "primary" the active
/// indicator pins to. Archived instances are ordered by archive date, newest
/// first, and same-date copies keep the repository's worktree order (see
/// [`ChangeInstance::modified_at`]).
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
    /// Used to order instances and to pick the primary. For an active
    /// instance, the Unix-epoch seconds of the most recent modification time
    /// across the change directory's files. For an archived instance, 00:00
    /// UTC of the `YYYY-MM-DD` date its archive directory is named with, or
    /// `0` for an undated legacy directory: the aggregation never reads
    /// beneath `archive/<dir>/` (`archive-browser`: *Archived Instances Are
    /// Ordered Without Reading the Archive*). Archived instances are never
    /// serialized, so the value only orders them among themselves.
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
    /// The raw upstream of `branch` (`<remote>/<branch>`), read from the same
    /// status invocation; `None` when untracked, detached, or cold.
    pub upstream: Option<String>,
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
                has_open_spec: false,
                disabled,
            },
        })
        .collect()
}

/// Whether `dir` holds an `openspec/` subdirectory: the one filesystem read
/// behind `has_open_spec`.
fn holds_openspec(dir: &Path) -> bool {
    dir.join("openspec").is_dir()
}

/// Stamp every row's `has_open_spec` from the filesystem (`workspace-registry`:
/// *OpenSpec Presence Is Derived on Every Aggregation*). Applied after
/// aggregation, the way display name and colour are, so [`aggregate`] stays
/// I/O-free.
pub fn stamp_openspec_presence(views: &mut [WorkspaceView]) {
    for view in views {
        match view {
            WorkspaceView::Repo(repo) => stamp_repo_openspec_presence(repo),
            WorkspaceView::Flat {
                workspace,
                has_open_spec,
                ..
            } => *has_open_spec = holds_openspec(&workspace.uri),
        }
    }
}

/// [`stamp_openspec_presence`] for one repository: any tracked worktree with
/// `openspec/` counts, so OpenSpec on a feature branch alone is enough.
pub fn stamp_repo_openspec_presence(repo: &mut RepoView) {
    repo.has_open_spec = repo.worktrees.iter().any(|w| holds_openspec(w));
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
                // A full recompute re-statuses every warm worktree.
                GatheredInput::Repo(gather_repo_inputs(
                    repo_id,
                    entries_in_repo,
                    cache,
                    &default_branch_for,
                    cold,
                    &Restatus::All,
                    &WorktreeGitMemo::new(),
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
/// pools *every* repo row's work — one status/archived-stubs job per
/// worktree, across every repo — into a
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
    compute_views_with_states(gathered).0
}

/// [`compute_views_from_gathered`], also returning the git state of every
/// warm worktree it statused (see [`worktree_git_states`]): exactly what the
/// watcher's memo holds after a full recompute.
pub(crate) fn compute_views_with_states(
    gathered: Vec<GatheredInput>,
) -> (Vec<WorkspaceView>, WorktreeGitMemo) {
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

    let (computed_repos, unread) = compute_repo_rows_pooled(repo_rows);

    let mut inputs: Vec<Option<ViewInput>> = (0..total_slots).map(|_| None).collect();
    let mut states = WorktreeGitMemo::new();
    for (slot, snapshot) in computed_repos {
        states.extend(worktree_git_states(&snapshot, &unread));
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

    let mut views = aggregate(inputs);
    stamp_openspec_presence(&mut views);
    (views, states)
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

impl RepoGatherInput {
    /// The repository this input belongs to.
    pub(crate) fn repo_id(&self) -> &RepoId {
        &self.repo_id
    }

    /// The tracked worktrees gathered for it, in config order.
    pub(crate) fn worktree_paths(&self) -> impl Iterator<Item = &Path> + '_ {
        self.worktrees.iter().map(|wt| wt.workspace.uri.as_path())
    }
}

/// Owned per-worktree inputs to [`compute_repo_snapshot`] — everything about
/// a worktree that comes from the registry/cache rather than git or the
/// filesystem (contrast [`WorktreeComputeResult`], resolved during compute).
struct WorktreeGatherInput {
    workspace: WorkspaceFolder,
    active_changes: Vec<ChangeData>,
    /// The git state this worktree carried at its last status computation,
    /// set by gather only when that state may stand in for a fresh one (see
    /// [`Restatus::Worktree`]). [`compute_worktree`] then returns it without
    /// spawning git. Always `None` on a cold row.
    reuse: Option<WorktreeGitState>,
}

/// One warm worktree's git state as its last status computation produced it:
/// branch, upstream and working-tree status, plus the sorted active change ids
/// that status classified. The watcher keeps one per warm tracked worktree
/// (`Inner::worktree_git`) so a file batch can re-status its own worktree
/// alone (`working-tree-status`: *Status Freshness*). Never recorded for a
/// cold row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorktreeGitState {
    pub(crate) branch: Option<String>,
    pub(crate) upstream: Option<String>,
    pub(crate) status: WorktreeStatus,
    pub(crate) classified_ids: Vec<String>,
}

/// Which worktrees of a repository a scoped recompute re-statuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Restatus {
    /// Every tracked worktree: a git event (index, refs, `HEAD`, config), the
    /// re-enable path, and every full recompute.
    All,
    /// Only this worktree, the origin of an `openspec/` file batch. Every
    /// other warm worktree whose recorded state classified the same active
    /// change ids it holds now reuses that state; any other is statused.
    Worktree(PathBuf),
}

/// The per-worktree git-state memo, keyed by worktree path.
pub(crate) type WorktreeGitMemo = HashMap<PathBuf, WorktreeGitState>;

/// A worktree's active change ids, sorted: the key a recorded
/// [`WorktreeGitState`] is matched on.
fn sorted_change_ids(changes: &[ChangeData]) -> Vec<String> {
    let mut ids: Vec<String> = changes.iter().map(|c| c.change_id.clone()).collect();
    ids.sort_unstable();
    ids
}

/// The git state every worktree of a warm snapshot now carries, for the
/// watcher to record. Empty for a cold snapshot, and without the worktrees in
/// `unread`, whose git read failed: those clean, branchless defaults were
/// never read from git and must never stand in for a status.
pub(crate) fn worktree_git_states(
    snap: &RepoSnapshot,
    unread: &[PathBuf],
) -> Vec<(PathBuf, WorktreeGitState)> {
    if snap.cold {
        return Vec::new();
    }
    snap.worktrees
        .iter()
        .filter(|wt| !unread.contains(&wt.workspace.uri))
        .map(|wt| {
            (
                wt.workspace.uri.clone(),
                WorktreeGitState {
                    branch: wt.branch.clone(),
                    upstream: wt.upstream.clone(),
                    status: wt.status.clone(),
                    classified_ids: sorted_change_ids(&wt.active_changes),
                },
            )
        })
        .collect()
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
///
/// `restatus` and `memo` decide which warm worktrees may reuse their recorded
/// git state instead of being statused (see [`Restatus`]). A map lookup, so
/// gather stays free of I/O. A cold row never reuses anything, and neither
/// does a WSL repository: reuse is only sound while the repository's git
/// events are observed, and the native watcher behind its `RepoMonitor`
/// hears nothing on the 9P share, so its git events never re-status it.
fn gather_repo_inputs(
    repo_id: RepoId,
    entries_in_repo: Vec<crate::registry::RegistryEntry>,
    cache: &WorkspaceCache,
    default_branch_for: &impl Fn(&RepoId) -> Option<String>,
    cold: bool,
    restatus: &Restatus,
    memo: &WorktreeGitMemo,
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

    let may_reuse = !cold && !crate::wsl::is_wsl_path(repo_id.as_path());
    let worktrees = entries_in_repo
        .into_iter()
        .map(|entry| {
            let active_changes = cache.changes_for(&entry.folder.uri).to_vec();
            let reuse = match restatus {
                Restatus::Worktree(origin) if may_reuse && entry.folder.uri != *origin => memo
                    .get(&entry.folder.uri)
                    .filter(|state| state.classified_ids == sorted_change_ids(&active_changes))
                    .cloned(),
                _ => None,
            };
            WorktreeGatherInput {
                workspace: entry.folder,
                active_changes,
                reuse,
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
/// repository's git I/O (branch + status per worktree, concurrently — see
/// [`compute_worktree_snapshots`]; the main worktree needs none, see
/// [`resolve_main_worktree`])
/// with no registry or cache lock held, and build the [`RepoSnapshot`] the
/// aggregator consumes. Used by the *scoped* (single-repository) recompute
/// path; the full-registry path uses [`compute_repo_rows_pooled`] instead,
/// which pools this same per-worktree work across every repo rather than
/// fanning out one repo at a time (see that function's doc comment).
///
/// Also returns the warm worktrees whose git state could not be read (see
/// [`worktree_git_states`]).
pub(crate) fn compute_repo_snapshot(input: RepoGatherInput) -> (RepoSnapshot, Vec<PathBuf>) {
    let main_worktree = resolve_main_worktree(&input.repo_id);
    let mut unread = Vec::new();
    let worktrees = compute_worktree_snapshots(input.worktrees, input.cold, &mut unread);

    let snapshot = RepoSnapshot {
        repo_id: input.repo_id,
        main_worktree,
        default_branch: input.default_branch,
        worktrees,
        cold: input.cold,
    };
    (snapshot, unread)
}

/// Determine a repository's main worktree with no subprocess, through
/// [`git::main_worktree_for_common_dir`]. That is not a heuristic but git's
/// own derivation (`worktree.c: get_main_worktree`), pinned against
/// `git worktree list --porcelain` for the plain, `--separate-git-dir`,
/// submodule and bare layouts, so it returns the very path git would have
/// (`workspace-registry`: *Main Worktree Resolved Without a Subprocess*).
///
/// Warm and cold rows share it, so a parked repository's identity —
/// `main_worktree`, the `name` derived from it, and each instance's
/// `is_main_worktree` — is byte-identical to the identity it had while
/// enabled. A WSL repository's `RepoId` is already the canonical UNC common
/// directory (`git::git_common_dir` joins git's answer onto the UNC workspace
/// path), and the derivation only strips a trailing `.git` component, so the
/// result is the UNC worktree path `git worktree list` would have translated
/// to.
pub(crate) fn resolve_main_worktree(repo_id: &RepoId) -> PathBuf {
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
    upstream: Option<String>,
    status: WorktreeStatus,
    /// Cheap stubs (directory listing only) — enough for the logical diff
    /// to tell archived from deleted, without parsing the archive. The
    /// archive's content is loaded lazily by the Archive browser.
    archived_changes: Vec<ChangeData>,
    /// Whether `branch`, `upstream` and `status` were read from git, freshly
    /// or as a reused record. `false` for a cold row and for a failed read,
    /// whose clean, branchless defaults must never be recorded.
    git_read: bool,
}

/// Build a worktree's snapshot from its gathered and computed halves, noting
/// in `unread` a worktree whose git state was not read.
fn worktree_snapshot(
    wt: WorktreeGatherInput,
    result: WorktreeComputeResult,
    unread: &mut Vec<PathBuf>,
) -> WorktreeSnapshot {
    if !result.git_read {
        unread.push(wt.workspace.uri.clone());
    }
    WorktreeSnapshot {
        workspace: wt.workspace,
        branch: result.branch,
        upstream: result.upstream,
        active_changes: wt.active_changes,
        archived_changes: result.archived_changes,
        status: result.status,
    }
}

/// Resolve one worktree's [`WorktreeComputeResult`]. Archived changes are
/// not classified against the branch/status (they live under `archive/`
/// and carry no active-tree chip).
/// A `cold` worktree skips the branch/status git spawn and reports the same
/// clean, branchless result the graceful-degradation path uses when git is
/// unavailable. A warm worktree gather marked for reuse returns its recorded
/// state unchanged, also without a spawn. The archived-stub directory read
/// still happens either way — the Dashboard's archived counts and today's
/// ships depend on it, and it is a `read_dir`, not a subprocess.
fn compute_worktree(wt: &WorktreeGatherInput, cold: bool) -> WorktreeComputeResult {
    let (branch, status, git_read) = if cold {
        (git::BranchState::default(), WorktreeStatus::clean(), false)
    } else if let Some(reused) = &wt.reuse {
        (
            git::BranchState {
                head: reused.branch.clone(),
                upstream: reused.upstream.clone(),
            },
            reused.status.clone(),
            true,
        )
    } else {
        let change_ids: Vec<String> = wt
            .active_changes
            .iter()
            .map(|c| c.change_id.clone())
            .collect();
        match git::try_worktree_branch_and_status(&wt.workspace.uri, &change_ids) {
            Some((branch, status)) => (branch, status, true),
            None => (git::BranchState::default(), WorktreeStatus::clean(), false),
        }
    };
    let archived_changes = list_archived_stubs(&wt.workspace).unwrap_or_default();
    WorktreeComputeResult {
        branch: branch.head,
        upstream: branch.upstream,
        status,
        archived_changes,
        git_read,
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
    unread: &mut Vec<PathBuf>,
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
            worktree_snapshot(wt, result, unread)
        })
        .collect()
}

/// Compute every repo row's git I/O — one [`compute_worktree`] job per
/// worktree, across *all* rows — via a single global scoped thread pool. The
/// main worktree is resolved inline ([`resolve_main_worktree`] spawns
/// nothing, so it needs no worker). See
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
///
/// Also returns every warm worktree whose git state could not be read (see
/// [`worktree_git_states`]).
fn compute_repo_rows_pooled(
    repo_rows: Vec<(usize, RepoGatherInput)>,
) -> (Vec<(usize, RepoSnapshot)>, Vec<PathBuf>) {
    struct Job<'a> {
        row: usize,
        worktree: usize,
        input: &'a WorktreeGatherInput,
        cold: bool,
    }

    let mut jobs: Vec<Job> = Vec::new();
    for (row, (_, input)) in repo_rows.iter().enumerate() {
        for (worktree, wt) in input.worktrees.iter().enumerate() {
            jobs.push(Job {
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
                        .map(|&i| {
                            let job = &jobs[i];
                            (job.row, job.worktree, compute_worktree(job.input, job.cold))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for (row, worktree, result) in handle.join().expect("worktree git worker panicked") {
                worktree_results[row][worktree] = Some(result);
            }
        }
    });

    let mut unread = Vec::new();
    let rows = repo_rows
        .into_iter()
        .zip(worktree_results)
        .map(|((slot, input), row_results)| {
            let main_worktree = resolve_main_worktree(&input.repo_id);
            let worktrees = input
                .worktrees
                .into_iter()
                .zip(row_results)
                .map(|(wt, result)| {
                    let result = result.expect("every worktree is assigned exactly one job");
                    worktree_snapshot(wt, result, &mut unread)
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
        .collect();
    (rows, unread)
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
///
/// `restatus` and `memo` pick the worktrees whose recorded git state is
/// reused rather than re-read (see [`Restatus`]); [`Restatus::All`] with any
/// memo statuses every warm worktree.
pub(crate) fn gather_repo_view(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    repo_id: &RepoId,
    default_branch_for: impl Fn(&RepoId) -> Option<String>,
    is_disabled: impl Fn(&PresentationKey) -> bool,
    restatus: &Restatus,
    memo: &WorktreeGitMemo,
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
        restatus,
        memo,
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
    let input = gather_repo_view(
        registry,
        cache,
        repo_id,
        default_branch_for,
        is_disabled,
        &Restatus::All,
        &WorktreeGitMemo::new(),
    )?;
    Some(compute_gathered_repo_view(input))
}

/// The scoped recompute's compute phase: git I/O for one repository, then its
/// [`RepoView`] with OpenSpec presence stamped. Shared by [`compute_repo_view`]
/// and the watcher's split path so neither can skip the stamp.
pub fn compute_gathered_repo_view(input: RepoGatherInput) -> RepoView {
    compute_gathered_repo_view_with_states(input).0
}

/// [`compute_gathered_repo_view`], also returning the git state each warm
/// worktree now carries (see [`worktree_git_states`]) so the watcher can
/// record it.
pub(crate) fn compute_gathered_repo_view_with_states(
    input: RepoGatherInput,
) -> (RepoView, Vec<(PathBuf, WorktreeGitState)>) {
    let (snapshot, unread) = compute_repo_snapshot(input);
    let states = worktree_git_states(&snapshot, &unread);
    let mut view = build_repo_view(snapshot);
    stamp_repo_openspec_presence(&mut view);
    (view, states)
}

/// Replace the [`WorkspaceView::Repo`] whose `repo_id` matches `new_view`'s in
/// `views`, in place, preserving the position (and therefore the config order)
/// of every other entry. Returns the replaced view if a matching repo was
/// found; `None` if no repo with that id is present — in which case the
/// caller should fall back to a full recompute (the repo is new to the
/// snapshot).
///
/// This is the splice that lets a repo-scoped recompute update a single
/// repository's view without rebuilding the whole aggregated snapshot. The
/// caller diffs the returned old view against the new one alone (see
/// [`LogicalIndex`]): every other row is untouched, so it contributes no
/// event.
pub fn replace_repo_view(views: &mut [WorkspaceView], new_view: RepoView) -> Option<RepoView> {
    let slot = views.iter_mut().find_map(|view| match view {
        WorkspaceView::Repo(existing) if existing.repo_id == new_view.repo_id => Some(existing),
        _ => None,
    })?;
    Some(std::mem::replace(slot, new_view))
}

/// The flat twin of [`replace_repo_view`]: replace the [`WorkspaceView::Flat`]
/// whose workspace URI matches `new_view`'s, in place, keeping its config
/// position. Returns `false` (and changes nothing) when no such row is
/// present, or when `new_view` is not a flat row.
pub fn replace_flat_view(views: &mut [WorkspaceView], new_view: WorkspaceView) -> bool {
    let WorkspaceView::Flat { workspace, .. } = &new_view else {
        return false;
    };
    let Some(slot) = views.iter_mut().find(|view| {
        matches!(view, WorkspaceView::Flat { workspace: existing, .. } if existing.uri == workspace.uri)
    }) else {
        return false;
    };
    *slot = new_view;
    true
}

/// Gather a flat (non-git) workspace's top-level row from the registry and
/// cache, exactly as [`gather_views`] and [`aggregate`] build it, for a
/// file batch that touches that workspace alone. No I/O: the caller stamps
/// its OpenSpec presence afterwards ([`stamp_openspec_presence`]) with no
/// lock held. `None` when `uri` is not a user-registered flat entry (not
/// registered, or now part of a repository), in which case the caller falls
/// back to a full recompute.
pub fn gather_flat_view(
    registry: &WorkspaceRegistry,
    cache: &WorkspaceCache,
    uri: &Path,
    is_disabled: impl Fn(&PresentationKey) -> bool,
) -> Option<WorkspaceView> {
    let entry = registry.entry(uri)?;
    if entry.repo_id.is_some() || !matches!(entry.origin, WorkspaceOrigin::UserRegistered) {
        return None;
    }
    let changes = cache.changes_for(&entry.folder.uri).to_vec();
    let disabled = is_disabled(&PresentationKey::Flat(entry.folder.uri.clone()));
    aggregate(vec![ViewInput::Flat {
        workspace: entry.folder.clone(),
        changes,
        disabled,
    }])
    .pop()
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
    LogicalIndex::of_views(old).diff(&LogicalIndex::of_views(new))
}

/// What [`diff_views`] compares: every logical change keyed by
/// `(repo_id, change name)`, with its instance paths and whether any instance
/// was active. Flat rows contribute nothing.
///
/// Built per repository as well as per snapshot, so a scoped recompute can
/// diff the one repository it replaced: every other repository is identical
/// in the old and new snapshots, so its keys produce no event, and the
/// one-repository diff equals the whole-snapshot one.
#[derive(Default)]
pub(crate) struct LogicalIndex(HashMap<(PathBuf, String), LogicalState>);

impl LogicalIndex {
    fn of_views(views: &[WorkspaceView]) -> Self {
        let mut index = Self::default();
        for view in views {
            if let WorkspaceView::Repo(repo) = view {
                index.add_repo(repo);
            }
        }
        index
    }

    /// The index of a single repository's view.
    pub(crate) fn of_repo(repo: &RepoView) -> Self {
        let mut index = Self::default();
        index.add_repo(repo);
        index
    }

    fn add_repo(&mut self, repo: &RepoView) {
        let active_iter = repo.active.iter().map(|lc| (lc, true));
        let archived_iter = repo.archived.iter().map(|lc| (lc, false));
        for (lc, in_active_section) in active_iter.chain(archived_iter) {
            let key = (repo.repo_id.clone(), lc.name.clone());
            let entry = self.0.entry(key).or_insert_with(|| LogicalState {
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

    /// The events that take `self` (the old state) to `new` — see
    /// [`diff_views`] for the rules.
    pub(crate) fn diff(&self, new: &LogicalIndex) -> Vec<CacheEvent> {
        diff_logical_indexes(&self.0, &new.0)
    }
}

fn diff_logical_indexes(
    old_map: &HashMap<(PathBuf, String), LogicalState>,
    new_map: &HashMap<(PathBuf, String), LogicalState>,
) -> Vec<CacheEvent> {
    let mut events = Vec::new();
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
            // From the directory name alone: the aggregation never reads
            // beneath `archive/<dir>/` (`archive-browser`: *Archived Instances
            // Are Ordered Without Reading the Archive*).
            let modified_at = archive_dir_recency(&change.change_id);
            // Keyed on the BARE logical id, not the raw directory name. An
            // archived change's directory is `<YYYY-MM-DD>-<id>` in ordinary
            // use and `<id>` in the legacy form; keying the two differently
            // from the active `<id>` would put an archived instance and an
            // active one of the same logical change under different keys, and
            // `[stale]` — which needs both in one `LogicalChange` — could then
            // never fire for a dated archive directory, i.e. for the form that
            // actually occurs (`spec-browser`: *Per-Instance Divergence
            // Label*). The dated name stays on `change.change_id`, because it
            // is what addresses a read.
            by_name
                .entry(crate::parser::archive_dir_logical_id(&change.change_id).to_string())
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
    // by `modified_at` descending, and partition into active/archived
    // sections. The sort is stable and stage 1 pushed instances in snapshot
    // worktree order (the registry's config order for this repository), so
    // archived copies sharing a date keep that order on every recompute.
    let mut active = Vec::new();
    let mut archived = Vec::new();
    for (name, mut instances) in by_name {
        // Divergence is computed over the JOINED set — an active instance and
        // an archived one of the same logical change must be comparable for
        // `[stale]` to fire at all (`spec-browser`: *Per-Instance Divergence
        // Label*). That is the whole reason stage 1 keys both flavours on the
        // bare logical id.
        annotate_divergence(&mut instances);
        instances.sort_by_key(|i| std::cmp::Reverse(i.modified_at));

        // …but the sections are then PARTITIONED by flavour, never bucketed
        // whole. `active` is serialized and `archived` is not, and an archived
        // instance carries a `list_archived_stubs` stub — no title, no tasks,
        // no artifacts — because the archive is deliberately never parsed on
        // the aggregation path (`archive-browser`: *On-Demand, Off-Hot-Path
        // Loading*). Letting a mixed logical change fall into `active` whole
        // therefore ships those stubs to every frontend, and no consumer of
        // `active` checks `is_archived_here`: the tree would render an
        // unlabelled, artifact-less, unclickable row, `primary_change` would
        // fold a 0/0 task count into the Dashboard, the address resolver would
        // see a second instance and demand a disambiguating segment for a
        // change the user sees as a singleton, and the TUI's
        // `find(is_main_worktree)` would read `openspec/changes/<dated-dir>/`,
        // a path that does not exist. Partitioning keeps the join available to
        // `annotate_divergence` while leaving each section's contract exactly
        // what its consumers already assume.
        //
        // A mixed logical change therefore appears in BOTH sections — its
        // active instances in `active` (carrying the `[stale]` label the join
        // produced) and its archived instances in `archived`. `diff_views`
        // merges the two by `(repo_id, name)` and tracks `had_active_instance`,
        // so this is exactly the shape its event diff already expects.
        let (archived_insts, active_insts): (Vec<_>, Vec<_>) =
            instances.into_iter().partition(|i| i.is_archived_here);
        if !active_insts.is_empty() {
            active.push(LogicalChange {
                name: name.clone(),
                instances: active_insts,
            });
        }
        if !archived_insts.is_empty() {
            archived.push(LogicalChange {
                name,
                instances: archived_insts,
            });
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
    let worktrees: Vec<PathBuf> = snap
        .worktrees
        .iter()
        .map(|wt| wt.workspace.uri.clone())
        .collect();
    let worktree_refs: Vec<WorktreeRef> = snap
        .worktrees
        .iter()
        .map(|wt| WorktreeRef {
            path: wt.workspace.uri.clone(),
            branch: wt.branch.clone(),
            upstream: wt.upstream.clone(),
        })
        .collect();

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
        worktrees,
        has_open_spec: false,
        disabled: snap.cold,
        worktree_refs,
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

/// Whether two change directories differ at any regular file: a differing
/// relative path list, a differing file size, or differing bytes.
///
/// Sizes come from the metadata the walk already reads, so a size mismatch
/// rejects before any file is opened. Equal-sized files are then compared in
/// streamed [`COMPARE_CHUNK`]-byte chunks with an early exit, never read whole
/// (`spec-browser`: *Per-Instance Divergence Label* requires the byte-level
/// comparison). An unreadable file counts as differing, and so does a file
/// whose length moved between the walk and the read (see [`same_contents`]).
///
/// Symlinks and special files are skipped by the walk on both sides:
/// `DirEntry::metadata` does not follow a symlink, so it reports neither a
/// directory nor a regular file.
fn dirs_differ(a: &std::path::Path, b: &std::path::Path) -> bool {
    let mut a_files = collect_relative_files(a);
    let mut b_files = collect_relative_files(b);
    a_files.sort();
    b_files.sort();
    if a_files != b_files {
        return true;
    }
    a_files
        .iter()
        .any(|(rel, len)| !same_contents(&a.join(rel), &b.join(rel), *len))
}

/// Chunk size of [`same_contents`]'s streamed comparison.
const COMPARE_CHUNK: usize = 8192;

/// Whether `a` and `b` hold the same bytes, given the length `len` the walk
/// recorded for both. Reads both in [`COMPARE_CHUNK`]-byte chunks and stops at
/// the first differing chunk.
///
/// The walk's length is a snapshot, so the files may have changed since. A
/// file that cannot be opened, a short read before `len` on either side, and
/// either side still yielding bytes after `len` all count as differing:
/// equality is never decided on a prefix. The loop runs a fixed number of
/// iterations derived from `len`, so it terminates whatever the files do.
fn same_contents(a: &std::path::Path, b: &std::path::Path, len: u64) -> bool {
    use std::io::Read;
    let (Ok(mut file_a), Ok(mut file_b)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    let mut buf_a = [0u8; COMPARE_CHUNK];
    let mut buf_b = [0u8; COMPARE_CHUNK];
    let chunk = COMPARE_CHUNK as u64;
    let mut remaining = len;
    for _ in 0..len.div_ceil(chunk) {
        let want = remaining.min(chunk) as usize;
        if file_a.read_exact(&mut buf_a[..want]).is_err() {
            return false;
        }
        if file_b.read_exact(&mut buf_b[..want]).is_err() {
            return false;
        }
        if buf_a[..want] != buf_b[..want] {
            return false;
        }
        remaining -= want as u64;
    }
    at_end(&mut file_a) && at_end(&mut file_b)
}

/// Whether `file` has nothing left to read. A read error counts as not at the
/// end, so [`same_contents`] reports the pair as differing.
fn at_end(file: &mut std::fs::File) -> bool {
    use std::io::Read;
    matches!(file.read(&mut [0u8; 1]), Ok(0))
}

/// Every regular file under `root`, as its path relative to `root` and its
/// length in bytes.
fn collect_relative_files(root: &std::path::Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(PathBuf, u64)>) {
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
                out.push((rel.to_path_buf(), meta.len()));
            }
        }
    }
}

/// An archived instance's recency, from its directory name alone: the Unix
/// seconds of 00:00 UTC on the `YYYY-MM-DD` date it is prefixed with
/// ([`crate::parser::archive_dir_date`]), or `0` for an undated legacy name or
/// a date that does not exist. Pure, so the aggregation stats nothing beneath
/// `archive/<dir>/` and an undated copy orders after every dated one.
fn archive_dir_recency(dir_name: &str) -> u64 {
    crate::parser::archive_dir_date(dir_name)
        .and_then(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .and_then(|midnight| u64::try_from(midnight.and_utc().timestamp()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
thread_local! {
    /// How many times [`newest_mtime`] was entered from outside its own
    /// recursion on this thread: the aggregation's per-instance stat walks.
    static NEWEST_MTIME_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Newest modification time under an active change's directory. Called once
/// per active instance, never for an archived one.
fn newest_mtime(dir: &std::path::Path) -> u64 {
    #[cfg(test)]
    NEWEST_MTIME_CALLS.with(|calls| calls.set(calls.get() + 1));
    newest_mtime_within(dir)
}

fn newest_mtime_within(dir: &std::path::Path) -> u64 {
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
            let sub = newest_mtime_within(&entry.path());
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
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
        }
    }

    #[test]
    fn replace_repo_view_swaps_matching_repo_in_place_preserving_order() {
        let a = PathBuf::from("/repo/a/.git");
        let b = PathBuf::from("/repo/b/.git");
        let flat = WorkspaceFolder::from_path(PathBuf::from("/flat"));
        let mut views = vec![
            WorkspaceView::Flat {
                has_open_spec: true,
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

        assert_eq!(
            replaced,
            Some(minimal_repo_view(&a, false)),
            "the replaced view is handed back for the one-repository diff"
        );
        assert_eq!(views.len(), 3);
        assert!(matches!(&views[0], WorkspaceView::Flat { .. }));
        match &views[1] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, a);
                assert!(r.dirty, "repo a should now be dirty");
            }
            WorkspaceView::Flat { .. } => panic!("expected repo a at index 1"),
        }
        match &views[2] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, b);
                assert!(!r.dirty, "repo b must be left untouched");
            }
            WorkspaceView::Flat { .. } => panic!("expected repo b at index 2"),
        }
    }

    #[test]
    fn replace_repo_view_returns_none_when_repo_absent() {
        let a = PathBuf::from("/repo/a/.git");
        let c = PathBuf::from("/repo/c/.git");
        let mut views = vec![WorkspaceView::Repo(minimal_repo_view(&a, false))];

        let replaced = replace_repo_view(&mut views, minimal_repo_view(&c, true));

        assert_eq!(replaced, None);
        assert_eq!(views.len(), 1);
        match &views[0] {
            WorkspaceView::Repo(r) => {
                assert_eq!(r.repo_id, a);
                assert!(!r.dirty);
            }
            WorkspaceView::Flat { .. } => panic!("expected repo a unchanged"),
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

    fn sole_repo(views: &[WorkspaceView]) -> &RepoView {
        match views {
            [WorkspaceView::Repo(r)] => r,
            other => panic!("expected exactly one repo row: {other:?}"),
        }
    }

    #[test]
    fn openspec_on_any_worktree_marks_the_repository() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let main = tmp.path().join("api");
        fs::create_dir_all(&main).unwrap();
        run_git(&["init", "-b", "main"], &main);
        run_git(&["config", "user.email", "t@t"], &main);
        run_git(&["config", "user.name", "t"], &main);
        run_git(&["commit", "--allow-empty", "-m", "init"], &main);
        let feature = tmp.path().join("api-feature");
        run_git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.to_str().unwrap(),
            ],
            &main,
        );

        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(main.clone()).unwrap();
        let cache = WorkspaceCache::new();
        let repo_id = reg.repos().into_iter().next().unwrap();

        let views = compute_views(&reg, &cache, |_| None, |_| false);
        let repo = sole_repo(&views);
        assert_eq!(repo.worktrees.len(), 2, "both worktrees are tracked");
        assert!(!repo.has_open_spec);
        assert!(repo.active.is_empty() && repo.archived.is_empty());

        // OpenSpec on the feature branch alone is enough, on both paths.
        fs::create_dir_all(feature.join("openspec")).unwrap();
        let views = compute_views(&reg, &cache, |_| None, |_| false);
        assert!(sole_repo(&views).has_open_spec);
        let scoped = compute_repo_view(&reg, &cache, &repo_id, |_| None, |_| false).unwrap();
        assert!(scoped.has_open_spec);
    }

    #[test]
    fn a_flat_workspace_follows_its_own_openspec_folder() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("notes");
        fs::create_dir_all(ws.join("openspec")).unwrap();
        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(ws.clone()).unwrap();
        let cache = WorkspaceCache::new();

        let presence =
            |reg: &WorkspaceRegistry| match compute_views(reg, &cache, |_| None, |_| false)
                .as_slice()
            {
                [WorkspaceView::Flat { has_open_spec, .. }] => *has_open_spec,
                other => panic!("expected one flat row: {other:?}"),
            };
        assert!(presence(&reg));
        fs::remove_dir_all(ws.join("openspec")).unwrap();
        assert!(!presence(&reg), "a deleted openspec/ reads as absent");
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
                upstream: None,
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
            WorkspaceView::Repo(_) => panic!("expected flat at index 0"),
        }
        match &views[1] {
            WorkspaceView::Repo(r) => assert_eq!(r.main_worktree, repo_ws.uri),
            WorkspaceView::Flat { .. } => panic!("expected repo at index 1"),
        }
        match &views[2] {
            WorkspaceView::Flat { workspace, .. } => assert_eq!(workspace.uri, flat2.uri),
            WorkspaceView::Repo(_) => panic!("expected flat at index 2"),
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
                // The worktree root is registered as-is either way; the
                // change directory above is what gives it a change to view.
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
                upstream: None,
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
        // A DATED archive directory, which is what `openspec archive` writes
        // and therefore the only form that occurs in practice. An un-dated
        // `archive/foo` fixture would let this pass with the active and
        // archived instances keyed identically by accident, hiding the very
        // defect the requirement is about.
        let (ws_main, active_main, archived_main) = build_workspace(
            &tmp.path().join("main"),
            &[],
            &[("2026-09-05-foo", "merged")],
        );
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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

    /// `spec-browser`: *Stale label fires against a dated archive directory*.
    ///
    /// The identity half of the requirement, which the label assertion alone
    /// does not pin: the two instances have to land in ONE `LogicalChange`
    /// named by the bare id, while the archived instance keeps its dated
    /// directory name — that name is what addresses a read, and re-deriving it
    /// from the logical name is exactly what is not possible.
    #[test]
    fn stale_label_fires_against_a_dated_archive_directory() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) = build_workspace(
            &tmp.path().join("main"),
            &[],
            &[("2026-09-05-add-thing", "merged")],
        );
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("add-thing", "stale-active")], &[]);

        let snap = RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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

        // Both sections name the change by its BARE id — the date prefix does
        // not split `add-thing` into two logical changes.
        assert_eq!(repo.active.len(), 1);
        assert_eq!(repo.active[0].name, "add-thing");
        assert_eq!(repo.archived.len(), 1);
        assert_eq!(repo.archived[0].name, "add-thing");

        // The join that lets `[stale]` fire does NOT put the archived instance
        // in the serialized `active` section: `active` holds only the still-
        // active instance, and the archived one is in `archived`, which is not
        // serialized.
        assert_eq!(repo.active[0].instances.len(), 1);
        let secondary = &repo.active[0].instances[0];
        assert!(!secondary.is_archived_here);
        assert!(!secondary.is_default_branch);
        assert_eq!(secondary.divergence, Some(DivergenceLabel::StaleVsArchived));

        // The archived instance still carries its on-disk directory name, which
        // is what addresses a read.
        assert_eq!(repo.archived[0].instances.len(), 1);
        let archived = &repo.archived[0].instances[0];
        assert!(archived.is_archived_here);
        assert_eq!(archived.change.change_id, "2026-09-05-add-thing");
    }

    /// The serialized `active` section is the one every frontend and the TUI
    /// read, and none of them checks `is_archived_here`. An archived instance
    /// carries a content-less stub (no title, no tasks, no artifacts) because
    /// the archive is never parsed on the aggregation path, so one reaching
    /// `active` renders an unlabelled, unclickable row, folds a 0/0 task count
    /// into the Dashboard, and makes the address resolver demand an instance
    /// segment for a change the user sees as a singleton.
    #[test]
    fn no_archived_instance_ever_reaches_the_serialized_active_section() {
        let tmp = TempDir::new().unwrap();
        // Every mixed shape at once: archived-on-default + active-on-branch,
        // active-on-default + archived-on-branch, and one worktree holding an
        // active change alongside its own archived twin.
        let (ws_main, active_main, archived_main) = build_workspace(
            &tmp.path().join("main"),
            &[("both-here", "active-in-main")],
            &[("2026-09-05-add-thing", "merged")],
        );
        let (ws_b, active_b, archived_b) = build_workspace(
            &tmp.path().join("b"),
            &[("add-thing", "stale-active"), ("both-here", "also-active")],
            &[("2026-09-06-both-here", "archived-in-b")],
        );

        let snap = RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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

        for lc in &repo.active {
            for inst in &lc.instances {
                assert!(
                    !inst.is_archived_here,
                    "archived instance {} leaked into the serialized active section under {}",
                    inst.change.change_id, lc.name
                );
            }
        }
        for lc in &repo.archived {
            for inst in &lc.instances {
                assert!(inst.is_archived_here, "active instance in archived section");
            }
        }

        // Nothing is lost by partitioning: both logical changes are still
        // reachable from both sections.
        let active_names: Vec<&str> = repo.active.iter().map(|lc| lc.name.as_str()).collect();
        let archived_names: Vec<&str> = repo.archived.iter().map(|lc| lc.name.as_str()).collect();
        assert_eq!(active_names, vec!["add-thing", "both-here"]);
        assert_eq!(archived_names, vec!["add-thing", "both-here"]);
    }

    /// One worktree can hold an active change and its own archived twin (the
    /// archive-then-revert case, and the legacy un-dated twin). Partitioning
    /// keeps those in different sections, so no consumer that keys a row on
    /// `(repo, change, worktree_path)` — the tree's React key, the address
    /// codec's instance token — can ever see the same triple twice.
    #[test]
    fn one_worktree_holding_both_flavours_yields_no_duplicate_instance_key() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) = build_workspace(
            &tmp.path().join("solo"),
            &[("fix-thing", "active")],
            &[("2026-06-04-fix-thing", "archived")],
        );
        let snap = RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![WorktreeSnapshot {
                workspace: ws,
                branch: Some("main".into()),
                upstream: None,
                active_changes: active,
                archived_changes: archived,
                status: WorktreeStatus::clean(),
            }],
        };
        let views = aggregate(vec![ViewInput::Repo(snap)]);
        let WorkspaceView::Repo(repo) = &views[0] else {
            panic!()
        };

        // Within the section a consumer renders, the (change, worktree) pair is
        // unique — one row, not two colliding ones.
        for lc in repo.active.iter().chain(repo.archived.iter()) {
            let mut seen = HashSet::new();
            for inst in &lc.instances {
                assert!(
                    seen.insert(inst.worktree_path.clone()),
                    "duplicate worktree path within one section's logical change {}",
                    lc.name
                );
            }
        }
        assert_eq!(repo.active.len(), 1);
        assert_eq!(repo.active[0].instances.len(), 1);
        assert_eq!(repo.archived.len(), 1);
        assert_eq!(repo.archived[0].instances.len(), 1);
    }

    /// The legacy un-dated archive form the fixtures above used to rely on.
    /// Re-pointing them at dated directories must not quietly drop it: the
    /// requirement names both forms, and `<id>` is what pre-dated tooling
    /// wrote.
    #[test]
    fn stale_label_still_fires_for_a_legacy_undated_archive_directory() {
        let tmp = TempDir::new().unwrap();
        let (ws_main, active_main, archived_main) =
            build_workspace(&tmp.path().join("main"), &[], &[("add-thing", "merged")]);
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[("add-thing", "stale-active")], &[]);

        let snap = RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
        assert_eq!(repo.active.len(), 1);
        assert_eq!(repo.active[0].name, "add-thing");
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
        let (ws_main, active_main, archived_main) = build_workspace(
            &tmp.path().join("main"),
            &[],
            &[("2026-09-05-foo", "merged")],
        );
        let (ws_b, active_b, archived_b) =
            build_workspace(&tmp.path().join("b"), &[], &[("2026-09-06-foo", "merged")]);

        let snap = RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: ws_main.uri.clone(),
            default_branch: Some("main".into()),
            worktrees: vec![
                WorktreeSnapshot {
                    workspace: ws_main,
                    branch: Some("main".into()),
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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
        // Two worktrees archived the same change on DIFFERENT days, so the
        // directory names differ. Keyed on the bare logical id they are one
        // row named `foo`; keyed on the raw directory name they would be two.
        assert_eq!(repo.archived.len(), 1);
        assert_eq!(repo.archived[0].name, "foo");
        assert_eq!(
            repo.archived[0]
                .instances
                .iter()
                .map(|i| i.change.change_id.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            ["2026-09-05-foo", "2026-09-06-foo"].into_iter().collect(),
            "each instance keeps the dated directory that addresses its read"
        );
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
                    upstream: None,
                    active_changes: active_old,
                    archived_changes: archived_old,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_new.clone(),
                    branch: Some("feature".into()),
                    upstream: None,
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
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
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
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
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
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
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
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
        })];
        let new = vec![WorkspaceView::Repo(RepoView {
            has_open_spec: true,
            disabled: false,
            worktree_refs: Vec::new(),
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
            worktrees: vec![],
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
                upstream: None,
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
                upstream: None,
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
                upstream: None,
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
                    upstream: None,
                    active_changes: active_main,
                    archived_changes: archived_main,
                    status: WorktreeStatus::clean(),
                },
                WorktreeSnapshot {
                    workspace: ws_b,
                    branch: Some("feature".into()),
                    upstream: None,
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

    // ---------------------------------------------------------------------
    // scope-status-recompute: archive recency, divergence compare, flat
    // splice, worktree git-state reuse, and the one-repository diff.
    // ---------------------------------------------------------------------

    fn warm_worktree(
        ws: WorkspaceFolder,
        branch: &str,
        active: Vec<ChangeData>,
        archived: Vec<ChangeData>,
    ) -> WorktreeSnapshot {
        WorktreeSnapshot {
            workspace: ws,
            branch: Some(branch.into()),
            upstream: None,
            active_changes: active,
            archived_changes: archived,
            status: WorktreeStatus::clean(),
        }
    }

    fn snapshot_of(tmp: &TempDir, main: &Path, worktrees: Vec<WorktreeSnapshot>) -> RepoSnapshot {
        RepoSnapshot {
            cold: false,
            repo_id: RepoId(tmp.path().join(".git")),
            main_worktree: main.to_path_buf(),
            default_branch: Some("main".into()),
            worktrees,
        }
    }

    fn archived_order(repo: &RepoView) -> Vec<PathBuf> {
        assert_eq!(repo.archived.len(), 1, "one archived logical change");
        repo.archived[0]
            .instances
            .iter()
            .map(|i| i.worktree_path.clone())
            .collect()
    }

    /// Push a file's modification time a year into the future.
    fn touch_into_the_future(path: &Path) {
        let future = SystemTime::now() + std::time::Duration::from_secs(365 * 24 * 60 * 60);
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(future)
            .unwrap();
    }

    #[test]
    fn archive_dir_recency_reads_the_date_from_the_directory_name() {
        assert_eq!(archive_dir_recency("2026-06-04-foo"), 1_780_531_200);
        assert_eq!(archive_dir_recency("2026-06-05-foo"), 1_780_617_600);
        assert!(archive_dir_recency("2026-06-05-foo") > archive_dir_recency("2026-06-04-foo"));
        assert_eq!(archive_dir_recency("1970-01-02-foo"), 86_400);
        assert_eq!(archive_dir_recency("foo"), 0, "an undated legacy name");
        assert_eq!(
            archive_dir_recency("2026-13-40-foo"),
            0,
            "a date that does not exist"
        );
        assert_eq!(
            archive_dir_recency("1969-12-31-foo"),
            0,
            "a date before the epoch"
        );
    }

    /// `archive-browser`: *File timestamps inside an archived change do not
    /// affect ordering*.
    #[test]
    fn archived_copies_order_by_directory_date_not_by_file_timestamps() {
        let tmp = TempDir::new().unwrap();
        let (w1, a1, r1) = build_workspace(&tmp.path().join("w1"), &[], &[("2026-06-04-foo", "x")]);
        let (w2, a2, r2) = build_workspace(&tmp.path().join("w2"), &[], &[("2026-06-05-foo", "x")]);
        touch_into_the_future(
            &w1.uri
                .join("openspec/changes/archive/2026-06-04-foo/proposal.md"),
        );

        let repo = build_repo_view(snapshot_of(
            &tmp,
            &w1.uri,
            vec![
                warm_worktree(w1.clone(), "main", a1, r1),
                warm_worktree(w2.clone(), "feature", a2, r2),
            ],
        ));

        assert_eq!(archived_order(&repo), vec![w2.uri.clone(), w1.uri.clone()]);
        assert_eq!(
            repo.archived[0].instances[0].modified_at,
            archive_dir_recency("2026-06-05-foo")
        );
    }

    /// `archive-browser`: *Same-day copies keep worktree order*.
    #[test]
    fn same_day_archived_copies_keep_worktree_order_on_every_build() {
        let tmp = TempDir::new().unwrap();
        let (w1, a1, r1) = build_workspace(&tmp.path().join("w1"), &[], &[("2026-06-04-foo", "x")]);
        let (w2, a2, r2) = build_workspace(&tmp.path().join("w2"), &[], &[("2026-06-04-foo", "x")]);
        // W2's copy is the more recently touched one, which file-time ordering
        // would have put first.
        touch_into_the_future(
            &w2.uri
                .join("openspec/changes/archive/2026-06-04-foo/proposal.md"),
        );
        let snapshot = snapshot_of(
            &tmp,
            &w1.uri,
            vec![
                warm_worktree(w1.clone(), "main", a1, r1),
                warm_worktree(w2.clone(), "feature", a2, r2),
            ],
        );

        for _ in 0..2 {
            let repo = build_repo_view(snapshot.clone());
            assert_eq!(archived_order(&repo), vec![w1.uri.clone(), w2.uri.clone()]);
        }
    }

    /// `archive-browser`: *A recompute does not descend into archived
    /// changes*. Counted rather than provoked: a directory the walk cannot
    /// read would be skipped silently by the old code as well.
    #[test]
    fn the_aggregation_stats_nothing_beneath_an_archived_change() {
        let tmp = TempDir::new().unwrap();
        let (ws, active, archived) = build_workspace(
            &tmp.path().join("main"),
            &[("one", "x"), ("two", "y")],
            &[
                ("2026-06-01-a", "x"),
                ("2026-06-02-b", "x"),
                ("legacy", "x"),
            ],
        );
        let nested = ws
            .uri
            .join("openspec/changes/archive/2026-06-01-a/specs/cap");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("spec.md"), "x").unwrap();

        NEWEST_MTIME_CALLS.with(|calls| calls.set(0));
        let repo = build_repo_view(snapshot_of(
            &tmp,
            &ws.uri,
            vec![warm_worktree(ws.clone(), "main", active, archived)],
        ));

        assert_eq!(
            NEWEST_MTIME_CALLS.with(|calls| calls.get()),
            2,
            "one walk per active instance and none for an archived one"
        );
        assert_eq!(repo.active.len(), 2);
        assert_eq!(repo.archived.len(), 3);
    }

    /// Two change directories `a` and `b` under `root`, holding `files` in
    /// both.
    fn twin_dirs(root: &Path, files: &[(&str, &[u8])]) -> (PathBuf, PathBuf) {
        let a = root.join("a");
        let b = root.join("b");
        for dir in [&a, &b] {
            for (rel, bytes) in files {
                let path = dir.join(rel);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, bytes).unwrap();
            }
        }
        (a, b)
    }

    /// A file more than two compare chunks long, so the streamed loop runs
    /// three times.
    fn multi_chunk() -> Vec<u8> {
        (0..2 * COMPARE_CHUNK + 5)
            .map(|i| (i % 251) as u8)
            .collect()
    }

    #[test]
    fn identical_directories_do_not_differ() {
        let tmp = TempDir::new().unwrap();
        let big = multi_chunk();
        let (a, b) = twin_dirs(
            tmp.path(),
            &[
                ("proposal.md", b"same"),
                ("specs/cap/spec.md", b"nested"),
                ("big.md", &big),
                ("empty.md", b""),
            ],
        );
        assert!(!dirs_differ(&a, &b));
    }

    #[test]
    fn a_same_length_one_byte_edit_differs() {
        let tmp = TempDir::new().unwrap();
        let (a, b) = twin_dirs(tmp.path(), &[("proposal.md", b"abcd")]);
        fs::write(b.join("proposal.md"), b"abce").unwrap();
        assert!(dirs_differ(&a, &b));
    }

    #[test]
    fn a_length_change_differs() {
        let tmp = TempDir::new().unwrap();
        let (a, b) = twin_dirs(tmp.path(), &[("proposal.md", b"abcd")]);
        fs::write(b.join("proposal.md"), b"abcde").unwrap();
        assert!(dirs_differ(&a, &b));
        assert!(dirs_differ(&b, &a));
    }

    #[test]
    fn an_extra_or_renamed_file_differs() {
        let tmp = TempDir::new().unwrap();
        let (a, b) = twin_dirs(tmp.path(), &[("proposal.md", b"x")]);
        fs::write(b.join("tasks.md"), b"").unwrap();
        assert!(dirs_differ(&a, &b), "an extra file");
        assert!(dirs_differ(&b, &a), "a missing file");

        let tmp = TempDir::new().unwrap();
        let (a, b) = twin_dirs(tmp.path(), &[("proposal.md", b"x")]);
        fs::rename(b.join("proposal.md"), b.join("design.md")).unwrap();
        assert!(dirs_differ(&a, &b), "a renamed file");
    }

    #[test]
    fn files_differing_only_in_their_last_chunk_differ() {
        let tmp = TempDir::new().unwrap();
        let big = multi_chunk();
        let (a, b) = twin_dirs(tmp.path(), &[("big.md", &big)]);
        let mut edited = big.clone();
        *edited.last_mut().unwrap() ^= 0xff;
        fs::write(b.join("big.md"), edited).unwrap();
        assert!(dirs_differ(&a, &b));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_file_differs() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let (a, b) = twin_dirs(tmp.path(), &[("proposal.md", b"same")]);
        let locked = a.join("proposal.md");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::File::open(&locked).is_ok() {
            // Running as root: permissions do not lock anything out.
            return;
        }
        assert!(dirs_differ(&a, &b));
        assert!(dirs_differ(&b, &a));
    }

    /// The walk's lengths are a snapshot. A file that grew or shrank before
    /// the read must not compare equal on the prefix both share — driven here
    /// by handing [`same_contents`] the stale length directly, which is the
    /// state a concurrent write leaves behind.
    #[test]
    fn a_length_that_moved_after_the_walk_never_compares_equal() {
        let tmp = TempDir::new().unwrap();
        let short = tmp.path().join("short");
        let long = tmp.path().join("long");
        fs::write(&short, b"0123456789").unwrap();
        fs::write(&long, b"0123456789abcde").unwrap();

        assert!(
            same_contents(&short, &short, 10),
            "the control: equal at the true length"
        );
        assert!(!same_contents(&short, &long, 10), "the second file grew");
        assert!(!same_contents(&long, &short, 10), "the first file grew");
        assert!(!same_contents(&short, &long, 15), "the first file shrank");
        assert!(!same_contents(&long, &short, 15), "the second file shrank");

        let big = tmp.path().join("big");
        fs::write(&big, multi_chunk()).unwrap();
        let len = multi_chunk().len() as u64;
        assert!(same_contents(&big, &big, len));
        assert!(
            !same_contents(&big, &big, len - 1),
            "bytes left past the walk's length"
        );
        assert!(
            !same_contents(&big, &big, len + 1),
            "a short read before it"
        );
        assert!(
            !same_contents(&big, &tmp.path().join("missing"), len),
            "an unopenable file"
        );
    }

    fn flat_row(uri: &str, has_open_spec: bool, changes: Vec<ChangeData>) -> WorkspaceView {
        WorkspaceView::Flat {
            workspace: WorkspaceFolder::from_path(PathBuf::from(uri)),
            changes,
            display_name: None,
            color: None,
            has_open_spec,
            disabled: false,
        }
    }

    #[test]
    fn replace_flat_view_swaps_in_place_and_keeps_order() {
        let repo = WorkspaceView::Repo(minimal_repo_view(Path::new("/repo/a/.git"), false));
        let mut views = vec![
            flat_row("/flat/x", true, vec![]),
            repo.clone(),
            flat_row("/flat/y", true, vec![]),
        ];
        let ws = WorkspaceFolder::from_path(PathBuf::from("/flat/y"));
        let edited = flat_row("/flat/y", false, vec![make_change("foo", "x", &ws, 0, 0)]);

        assert!(replace_flat_view(&mut views, edited.clone()));
        assert_eq!(
            views,
            vec![flat_row("/flat/x", true, vec![]), repo, edited],
            "only the matching row changed, in its own position"
        );
    }

    #[test]
    fn replace_flat_view_returns_false_when_the_row_is_absent() {
        let mut views = vec![flat_row("/flat/x", true, vec![])];
        assert!(!replace_flat_view(
            &mut views,
            flat_row("/flat/z", false, vec![])
        ));
        assert!(!replace_flat_view(
            &mut views,
            WorkspaceView::Repo(minimal_repo_view(Path::new("/flat/x"), true))
        ));
        assert_eq!(views, vec![flat_row("/flat/x", true, vec![])]);
    }

    #[test]
    fn gather_flat_view_builds_the_row_a_full_recompute_builds() {
        use crate::cache::WorkspaceCache;
        let tmp = TempDir::new().unwrap();
        let notes = tmp.path().join("notes");
        fs::create_dir_all(notes.join("openspec")).unwrap();
        let api = init_git_openspec(&tmp.path().join("api"));
        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(notes.clone()).unwrap();
        reg.register(api.clone()).unwrap();
        let notes = notes.canonicalize().unwrap();
        let mut cache = WorkspaceCache::new();
        let ws = WorkspaceFolder::from_path(notes.clone());
        cache.insert(notes.clone(), vec![make_change("foo", "x", &ws, 1, 2)]);

        let full = compute_views(&reg, &cache, |_| None, |_| false);
        let mut row = gather_flat_view(&reg, &cache, &notes, |_| false).expect("a flat row");
        stamp_openspec_presence(std::slice::from_mut(&mut row));
        assert_eq!(full[0], row);

        let parked_key = PresentationKey::Flat(notes.clone());
        let parked = gather_flat_view(&reg, &cache, &notes, |key| key == &parked_key).unwrap();
        assert!(parked.is_disabled());

        assert_eq!(
            gather_flat_view(&reg, &cache, &api, |_| false),
            None,
            "a repository entry"
        );
        assert_eq!(
            gather_flat_view(&reg, &cache, &tmp.path().join("nope"), |_| false),
            None,
            "an unregistered path"
        );
    }

    #[test]
    fn worktree_git_states_cover_every_warm_worktree_and_no_cold_one() {
        let tmp = TempDir::new().unwrap();
        let (ws, _, _) = build_workspace(&tmp.path().join("main"), &[], &[]);
        let changes = vec![
            make_change("zeta", "x", &ws, 0, 0),
            make_change("alpha", "x", &ws, 0, 0),
        ];
        let mut status = WorktreeStatus::clean();
        status.dirty = true;
        let mut snapshot = snapshot_of(
            &tmp,
            &ws.uri,
            vec![WorktreeSnapshot {
                workspace: ws.clone(),
                branch: Some("main".into()),
                upstream: Some("origin/main".into()),
                active_changes: changes,
                archived_changes: vec![],
                status: status.clone(),
            }],
        );

        assert_eq!(
            worktree_git_states(&snapshot, &[]),
            vec![(
                ws.uri.clone(),
                WorktreeGitState {
                    branch: Some("main".into()),
                    upstream: Some("origin/main".into()),
                    status,
                    classified_ids: vec!["alpha".into(), "zeta".into()],
                }
            )]
        );
        assert!(
            worktree_git_states(&snapshot, std::slice::from_ref(&ws.uri)).is_empty(),
            "a worktree whose git read failed records nothing"
        );
        snapshot.cold = true;
        assert!(
            worktree_git_states(&snapshot, &[]).is_empty(),
            "a cold row records nothing"
        );
    }

    /// A repository with a main and a feature worktree, both tracked, and the
    /// registry and cache over them: `main` holds `foo`, `feature` holds
    /// `baz` and `bar` (in that order, so matching has to sort).
    fn two_worktree_repo(
        tmp: &TempDir,
    ) -> (
        WorkspaceRegistry,
        crate::cache::WorkspaceCache,
        RepoId,
        PathBuf,
        PathBuf,
    ) {
        let main = init_git_openspec(&tmp.path().join("api"));
        let feature = tmp.path().join("api-feature");
        run_git(
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                feature.to_str().unwrap(),
            ],
            &main,
        );
        let feature = feature.canonicalize().unwrap();
        let mut reg = WorkspaceRegistry::new(tmp.path().join("ws.json"));
        reg.register(main.clone()).unwrap();
        assert!(
            reg.entry(&feature).is_some(),
            "the feature worktree is tracked"
        );
        let repo_id = reg.repos().into_iter().next().unwrap();
        let mut cache = crate::cache::WorkspaceCache::new();
        let main_ws = WorkspaceFolder::from_path(main.clone());
        let feature_ws = WorkspaceFolder::from_path(feature.clone());
        cache.insert(main.clone(), vec![make_change("foo", "x", &main_ws, 0, 0)]);
        cache.insert(
            feature.clone(),
            vec![
                make_change("baz", "x", &feature_ws, 0, 0),
                make_change("bar", "x", &feature_ws, 0, 0),
            ],
        );
        (reg, cache, repo_id, main, feature)
    }

    fn recorded(branch: &str, ids: &[&str]) -> WorktreeGitState {
        WorktreeGitState {
            branch: Some(branch.into()),
            upstream: Some(format!("origin/{branch}")),
            status: WorktreeStatus {
                dirty: true,
                spec_states: HashMap::new(),
            },
            classified_ids: ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn reuse_of(gathered: &RepoGatherInput, worktree: &Path) -> Option<WorktreeGitState> {
        gathered
            .worktrees
            .iter()
            .find(|wt| wt.workspace.uri == worktree)
            .expect("worktree gathered")
            .reuse
            .clone()
    }

    #[test]
    fn a_file_batch_reuses_only_siblings_whose_change_set_is_unchanged() {
        let tmp = TempDir::new().unwrap();
        let (reg, cache, repo_id, main, feature) = two_worktree_repo(&tmp);
        let memo: WorktreeGitMemo = [
            (main.clone(), recorded("old-main", &["foo"])),
            (feature.clone(), recorded("old-feature", &["bar", "baz"])),
        ]
        .into_iter()
        .collect();
        let gather = |restatus: &Restatus, memo: &WorktreeGitMemo, cold: bool| {
            gather_repo_view(&reg, &cache, &repo_id, |_| None, |_| cold, restatus, memo).unwrap()
        };
        let from_main = Restatus::Worktree(main.clone());

        let gathered = gather(&from_main, &memo, false);
        assert_eq!(
            reuse_of(&gathered, &main),
            None,
            "the origin is always statused"
        );
        assert_eq!(
            reuse_of(&gathered, &feature),
            Some(recorded("old-feature", &["bar", "baz"]))
        );

        let gathered = gather(&Restatus::All, &memo, false);
        assert_eq!(
            reuse_of(&gathered, &feature),
            None,
            "a git event re-statuses all"
        );

        let mut moved = memo.clone();
        moved.insert(feature.clone(), recorded("old-feature", &["bar"]));
        let gathered = gather(&from_main, &moved, false);
        assert_eq!(reuse_of(&gathered, &feature), None, "the change set moved");

        let gathered = gather(&from_main, &memo, true);
        assert_eq!(
            reuse_of(&gathered, &feature),
            None,
            "a cold row reuses nothing"
        );

        let gathered = gather(&from_main, &WorktreeGitMemo::new(), false);
        assert_eq!(reuse_of(&gathered, &feature), None, "no prior computation");
    }

    /// A WSL repository's git events are never observed (the native watcher
    /// hears nothing on the 9P share), so nothing may stand in for a status
    /// there, however complete the memo.
    #[test]
    fn a_wsl_repository_never_reuses_a_recorded_state() {
        use crate::cache::WorkspaceCache;
        let entry = |path: &str| crate::registry::RegistryEntry {
            folder: WorkspaceFolder::from_path(PathBuf::from(path)),
            origin: WorkspaceOrigin::UserRegistered,
            repo_id: None,
        };
        let main = r"\\wsl$\Ubuntu\home\dev\api";
        let feature = r"\\wsl$\Ubuntu\home\dev\api-feature";
        let memo: WorktreeGitMemo = [main, feature]
            .into_iter()
            .map(|path| (PathBuf::from(path), recorded("main", &[])))
            .collect();
        let gather = |repo_id: &str| {
            gather_repo_inputs(
                RepoId(PathBuf::from(repo_id)),
                vec![entry(main), entry(feature)],
                &WorkspaceCache::new(),
                &|_: &RepoId| None,
                false,
                &Restatus::Worktree(PathBuf::from(main)),
                &memo,
            )
        };

        let wsl = gather(r"\\wsl$\Ubuntu\home\dev\api\.git");
        assert!(wsl.worktrees.iter().all(|wt| wt.reuse.is_none()));
        // The control: the same memo under a local repository is reused.
        let local = gather("/home/dev/api/.git");
        assert_eq!(
            reuse_of(&local, Path::new(feature)),
            Some(recorded("main", &[]))
        );
    }

    /// A worktree whose `git status` failed reports clean and branchless, the
    /// graceful-degradation result, but that was never read from git: it is
    /// not recorded, so the next batch statuses it again.
    #[test]
    fn a_failed_status_read_is_not_recorded() {
        let tmp = TempDir::new().unwrap();
        let (reg, cache, repo_id, main, _feature) = two_worktree_repo(&tmp);
        let not_a_repo = tmp.path().join("plain");
        fs::create_dir_all(&not_a_repo).unwrap();
        let not_a_repo = not_a_repo.canonicalize().unwrap();
        let mut gathered = gather_repo_view(
            &reg,
            &cache,
            &repo_id,
            |_| None,
            |_| false,
            &Restatus::All,
            &WorktreeGitMemo::new(),
        )
        .unwrap();
        gathered.worktrees[1].workspace = WorkspaceFolder::from_path(not_a_repo.clone());

        let (view, states) = compute_gathered_repo_view_with_states(gathered);

        assert!(view.worktrees.contains(&not_a_repo));
        let recorded: Vec<PathBuf> = states.into_iter().map(|(path, _)| path).collect();
        assert_eq!(recorded, vec![main], "only the worktree git answered for");
    }

    /// A reused worktree carries its recorded branch and status into the view
    /// unchanged — `old-feature` is not a branch git could report — while the
    /// origin is read from git.
    #[test]
    fn a_reused_worktree_carries_its_recorded_state_into_the_view() {
        let tmp = TempDir::new().unwrap();
        let (reg, cache, repo_id, main, feature) = two_worktree_repo(&tmp);
        let memo: WorktreeGitMemo = [(feature.clone(), recorded("old-feature", &["bar", "baz"]))]
            .into_iter()
            .collect();
        let gathered = gather_repo_view(
            &reg,
            &cache,
            &repo_id,
            |_| None,
            |_| false,
            &Restatus::Worktree(main.clone()),
            &memo,
        )
        .unwrap();

        let (view, states) = compute_gathered_repo_view_with_states(gathered);

        let refs: Vec<(PathBuf, Option<String>)> = view
            .worktree_refs
            .iter()
            .map(|r| (r.path.clone(), r.branch.clone()))
            .collect();
        assert_eq!(
            refs,
            vec![
                (main.clone(), Some("main".into())),
                (feature.clone(), Some("old-feature".into())),
            ]
        );
        assert_eq!(
            view.dirty_worktrees,
            vec![feature.clone()],
            "the recorded dirty bit"
        );
        let states: WorktreeGitMemo = states.into_iter().collect();
        assert_eq!(states[&feature], recorded("old-feature", &["bar", "baz"]));
        assert_eq!(states[&main].branch.as_deref(), Some("main"));
        assert_eq!(states[&main].classified_ids, vec!["foo".to_string()]);
        assert_eq!(view.main_worktree, main, "resolved with no subprocess");
    }

    fn repo_with(id: &str, active: Vec<LogicalChange>, archived: Vec<LogicalChange>) -> RepoView {
        let mut repo = minimal_repo_view(Path::new(id), false);
        repo.active = active;
        repo.archived = archived;
        repo
    }

    fn logical(name: &str, instances: Vec<ChangeInstance>) -> LogicalChange {
        LogicalChange {
            name: name.into(),
            instances,
        }
    }

    fn sorted_events(events: Vec<CacheEvent>) -> Vec<String> {
        let mut out: Vec<String> = events.iter().map(|e| format!("{e:?}")).collect();
        out.sort();
        out
    }

    /// The scoped merge diffs the one replaced repository; every other row is
    /// identical in both snapshots, so the events must equal the
    /// whole-snapshot diff's.
    #[test]
    fn a_one_repository_diff_equals_the_whole_snapshot_diff() {
        let old_a = repo_with(
            "/a/.git",
            vec![
                logical(
                    "kept",
                    vec![
                        dummy_instance("/a", "kept", false),
                        dummy_instance("/a-wt", "kept", false),
                    ],
                ),
                logical("shipping", vec![dummy_instance("/a", "shipping", false)]),
                logical("moving", vec![dummy_instance("/a", "moving", false)]),
                logical("dropped", vec![dummy_instance("/a-wt", "dropped", false)]),
            ],
            vec![],
        );
        let new_a = repo_with(
            "/a/.git",
            vec![
                logical("kept", vec![dummy_instance("/a", "kept", false)]),
                logical("moving", vec![dummy_instance("/a-wt", "moving", false)]),
                logical("fresh", vec![dummy_instance("/a-wt", "fresh", false)]),
            ],
            vec![logical(
                "shipping",
                vec![dummy_instance("/a", "shipping", true)],
            )],
        );
        let b = repo_with(
            "/b/.git",
            vec![logical("other", vec![dummy_instance("/b", "other", false)])],
            vec![],
        );
        let flat = flat_row("/flat", true, vec![]);
        let old_full = vec![
            WorkspaceView::Repo(old_a.clone()),
            flat.clone(),
            WorkspaceView::Repo(b.clone()),
        ];
        let new_full = vec![
            WorkspaceView::Repo(new_a.clone()),
            flat,
            WorkspaceView::Repo(b),
        ];

        let mut spliced = old_full.clone();
        let replaced = replace_repo_view(&mut spliced, new_a.clone()).expect("A is present");
        assert_eq!(spliced, new_full);
        assert_eq!(replaced, old_a);

        let scoped = LogicalIndex::of_repo(&replaced).diff(&LogicalIndex::of_repo(&new_a));
        let full = diff_views(&old_full, &new_full);
        assert!(!full.is_empty());
        assert_eq!(sorted_events(scoped), sorted_events(full));
    }
}
