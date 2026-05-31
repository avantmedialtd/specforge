// Mirrors the public structs in `openspec-core`.
// Field names use camelCase per the serde rename_all on each struct.

export interface WorkspaceFolder {
    uri: string
    name: string
}

/// Curated tint palette for top-level workspace/repo rows. Mirrors the
/// `PaletteColor` enum in `crates/openspec-core/src/types.rs`.
export type PaletteColor =
    | "indigo"
    | "blue"
    | "teal"
    | "green"
    | "amber"
    | "orange"
    | "rose"
    | "purple"

export const PALETTE_COLORS: PaletteColor[] = [
    "indigo",
    "blue",
    "teal",
    "green",
    "amber",
    "orange",
    "rose",
    "purple",
]

export interface RegisteredWorkspace {
    uri: string
    name: string
    isMissing: boolean
    /// Configured display-name override from the presentation store, if any.
    displayName: string | null
    /// Configured tint colour from the presentation store, if any.
    color: PaletteColor | null
    /// Canonical path to the workspace's git common directory if it lives
    /// inside a repository; null for flat workspaces. The frontend uses this
    /// to decide whether to address the per-workspace or per-repo
    /// presentation key when editing this row.
    repoId: string | null
}

export interface Task {
    text: string
    completed: boolean
    indent: number
    lineNumber: number
}

export interface Section {
    title: string
    tasks: Task[]
}

export interface ArtifactStatus {
    proposal: boolean
    specs: string[]
    design: boolean
    tasks: boolean
}

export interface ChangeData {
    changeId: string
    title: string | null
    sections: Section[]
    totalTasks: number
    completedTasks: number
    artifacts: ArtifactStatus
    workspace: WorkspaceFolder
}

// -------------------------------------------------------------------------
// Repo / logical-change / instance shapes (mirrors crates/openspec-core/src/repo_view.rs)
// -------------------------------------------------------------------------

export type DivergenceLabel = "diverged" | "staleVsArchived"

export interface ChangeInstance {
    worktreePath: string
    branch: string | null
    isMainWorktree: boolean
    isDefaultBranch: boolean
    isArchivedHere: boolean
    change: ChangeData
    modifiedAt: number
    divergence: DivergenceLabel | null
}

export interface LogicalChange {
    name: string
    instances: ChangeInstance[]
}

export interface RepoView {
    repoId: string
    mainWorktree: string
    name: string
    defaultBranch: string | null
    active: LogicalChange[]
    archived: LogicalChange[]
    /// Configured display-name override; null falls back to `name`.
    displayName: string | null
    /// Configured tint colour for the top-level row.
    color: PaletteColor | null
}

export type WorkspaceView =
    | ({ kind: "repo" } & RepoView)
    | {
          kind: "flat"
          workspace: WorkspaceFolder
          changes: ChangeData[]
          displayName: string | null
          color: PaletteColor | null
      }

// -------------------------------------------------------------------------
// Commit-graph shapes (mirrors crates/openspec-core/src/git.rs + graph.rs)
// -------------------------------------------------------------------------

/// Kind of a ref decoration on a commit. Mirrors the `RefKind` enum
/// (serialised camelCase as a bare string).
export type RefKind = "localBranch" | "remoteBranch" | "tag" | "head"

export interface CommitRef {
    name: string
    kind: RefKind
}

/// A git trailer — a `Key: value` line from a commit message's last paragraph.
export interface Trailer {
    key: string
    value: string
}

export interface LaidOutCommit {
    id: string
    parents: string[]
    author: string
    /// Author date, ISO-8601.
    date: string
    subject: string
    refs: CommitRef[]
    /// Git trailers from the message's last paragraph, in git's order.
    trailers: Trailer[]
    /// Index in display order (0 = newest).
    row: number
    /// Lane the commit occupies.
    column: number
}

/// A line segment in the band between rows `band` and `band + 1`, running
/// from `fromColumn` (top) to `toColumn` (bottom).
export interface EdgeSegment {
    band: number
    fromColumn: number
    toColumn: number
}

export interface CommitGraph {
    commits: LaidOutCommit[]
    edges: EdgeSegment[]
    /// Columns the renderer must size for.
    laneCount: number
    /// True when the window was capped — older history exists below.
    truncated: boolean
}

/// One file changed by a commit. `additions`/`deletions` are null for binary
/// files (git reports `-`).
export interface CommitFile {
    path: string
    status: string
    additions: number | null
    deletions: number | null
}

// -------------------------------------------------------------------------
// Dashboard shapes (mirrors crates/openspec-core/src/dashboard.rs)
// -------------------------------------------------------------------------

export interface SummaryMetrics {
    activeChanges: number
    completedTasks: number
    totalTasks: number
    /// 0..=100; 0 when totalTasks is 0.
    taskPercent: number
    specsTouching: number
    repoCount: number
    worktreeCount: number
    flatCount: number
}

export interface RepoBreakdown {
    label: string
    activeCount: number
    archivedCount: number
}

export interface ActivityBucket {
    /// `YYYY-MM-DD` (the commit author date's day prefix).
    day: string
    commitCount: number
}

export interface LifecycleMetrics {
    archivedInWindow: number
    /// Mean seconds between creation and archival; null when none recoverable.
    avgTimeToArchiveSecs: number | null
}

export interface RecentEntry {
    changeId: string
    title: string | null
    workspaceLabel: string
    /// Worktree (or flat workspace) path the change's artifacts are read from.
    worktreePath: string
    modifiedAt: number
}

export interface DashboardData {
    summary: SummaryMetrics
    repos: RepoBreakdown[]
    activity: ActivityBucket[]
    /// Length of the activity day-axis the frontend renders (newest = today).
    activityWindowDays: number
    lifecycle: LifecycleMetrics
    recent: RecentEntry[]
    progress: ProgressData
}

// -------------------------------------------------------------------------
// Progress layer (gamified) — mirrors ProgressData in dashboard.rs
// -------------------------------------------------------------------------

/// What was achieved today, with trailing-30-active-day averages. The `*Centi`
/// fields are the average ×100 (integer on the wire so the Rust type stays
/// `Eq`); divide by 100 for display.
export interface TodayProgress {
    tasksCompleted: number
    changesArchived: number
    commitsLanded: number
    changesCreated: number
    tasksAvgCenti: number
    changesArchivedAvgCenti: number
    commitsAvgCenti: number
    changesCreatedAvgCenti: number
}

export interface StreakInfo {
    /// Consecutive active days ending today.
    current: number
    /// Longest run anywhere in the heatmap window.
    longest: number
}

export interface HeatmapCell {
    /// `YYYY-MM-DD` local calendar day.
    day: string
    /// Combined achievements + commits on that day (drives cell intensity).
    count: number
    /// Per-kind breakdown for the drill-down detail strip.
    tasks: number
    ships: number
    commits: number
    created: number
}

export interface Milestone {
    id: string
    label: string
    /// One of `tasks` | `ships` | `firstShip` | `streak`.
    kind: string
    threshold: number
    achieved: boolean
    /// Unix seconds of the crossing event; null for streak milestones.
    achievedAt: number | null
    /// Recovered from git history rather than observed live — never celebrated.
    backfilled: boolean
}

export interface ProgressData {
    today: TodayProgress
    streak: StreakInfo
    /// Ascending — oldest day first, today last.
    heatmap: HeatmapCell[]
    milestones: Milestone[]
}

// -------------------------------------------------------------------------
// Tauri event payloads (mirrors crates/specforge/src/events.rs)
// -------------------------------------------------------------------------

export interface CacheUpdatedPayload {
    workspace: string
}

export interface ChangeAddedPayload {
    workspace: string
    changeId: string
}

export interface ChangeArchivedPayload {
    workspace: string
    changeId: string
}

export interface WorkspaceRemovedPayload {
    workspace: string
}

export interface LogicalChangePayload {
    repoId: string
    changeName: string
}

export interface InstancePayload {
    repoId: string
    changeName: string
    worktreePath: string
}

export interface GraphChangedPayload {
    repoId: string
}

export const EVENT_CACHE_UPDATED = "cache-updated"
export const EVENT_CHANGE_ADDED = "change-added"
export const EVENT_CHANGE_ARCHIVED = "change-archived"
export const EVENT_WORKSPACE_REMOVED = "workspace-removed"
export const EVENT_LOGICAL_CHANGE_ADDED = "logical-change-added"
export const EVENT_LOGICAL_CHANGE_ARCHIVED = "logical-change-archived"
export const EVENT_INSTANCE_ADDED = "instance-added"
export const EVENT_INSTANCE_REMOVED = "instance-removed"
export const EVENT_WORKSPACE_PRESENTATION_UPDATED = "workspace-presentation-updated"
export const EVENT_GRAPH_CHANGED = "graph-changed"

// -------------------------------------------------------------------------
// Tree-selection discriminated union.
//
// `workspaceUri` is the path used to read artifacts. For Flat workspaces
// that's the registered workspace path; for git worktrees it's the
// individual worktree path. Either way the detail pane uses it as-is.
// -------------------------------------------------------------------------

export type ArtifactKind = "proposal" | "design" | "tasks" | "specs"

export type TreeSelection =
    | { kind: "workspace"; workspaceUri: string }
    | { kind: "change"; workspaceUri: string; changeId: string }
    | {
          kind: "artifact"
          workspaceUri: string
          changeId: string
          artifactKind: ArtifactKind
      }
    | {
          kind: "spec"
          workspaceUri: string
          changeId: string
          capability: string
      }
    | {
          kind: "section"
          workspaceUri: string
          changeId: string
          sectionIndex: number
      }
    | {
          kind: "task"
          workspaceUri: string
          changeId: string
          sectionIndex: number
          taskIndex: number
          lineNumber: number
      }
    // Git-repo aggregated tree nodes.
    | { kind: "repo"; repoId: string }
    | { kind: "logicalChange"; repoId: string; changeName: string }
    | {
          kind: "instance"
          repoId: string
          changeName: string
          worktreePath: string
      }

// -------------------------------------------------------------------------
// Center-pane render target.
//
// The detail (center) pane renders either an OpenSpec artifact (driven by the
// tree) or a commit's detail (driven by the graph rail). Whichever was
// selected most recently wins — a single union, last-write-wins.
// -------------------------------------------------------------------------

export type ArtifactReadKind = "proposal" | "design" | "tasks" | "spec"

export interface ArtifactRenderTarget {
    kind: "artifact"
    workspace: string
    changeId: string
    artifactKind: ArtifactReadKind
    capability?: string
}

/// Carries the clicked commit's metadata (it's already loaded in the rail's
/// graph) so the detail view shows the header without a metadata round-trip;
/// `commit.id` is the sha used to fetch files and diffs.
export interface CommitRenderTarget {
    kind: "commit"
    repoId: string
    commit: LaidOutCommit
}

/// The global Dashboard — the default home surface, shown at startup and
/// whenever no artifact or commit is selected.
export interface DashboardRenderTarget {
    kind: "dashboard"
}

export type RenderTarget =
    | ArtifactRenderTarget
    | CommitRenderTarget
    | DashboardRenderTarget
