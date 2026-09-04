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
    /// True when the user has parked this row. Disabled workspaces are omitted
    /// from the tree pane's aggregated view but kept here, flagged, because
    /// Settings is where the toggle that brings them back lives.
    disabled: boolean
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

/// Git commit state of one worktree's copy of a change's
/// `openspec/changes/<id>/` directory. `untracked` means the directory exists
/// on disk but git has never seen it (a brand-new spec living only in a
/// worktree); `modified` means tracked files have uncommitted edits.
/// Mirrors `SpecCommitState` in `crates/openspec-core/src/git.rs`.
export type SpecCommitState = "committed" | "modified" | "untracked"

export interface ChangeInstance {
    worktreePath: string
    branch: string | null
    isMainWorktree: boolean
    isDefaultBranch: boolean
    isArchivedHere: boolean
    change: ChangeData
    modifiedAt: number
    divergence: DivergenceLabel | null
    /// Commit state of this instance's spec directory in its worktree.
    specCommitState: SpecCommitState
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
    // Archived changes are not carried here — they are browsed in the Archive
    // view, loaded lazily per workspace via `listArchived` (see
    // `ArchivedChangeSummary`). The core keeps an in-memory archived set for
    // its event diff but does not serialize it.
    /// Configured display-name override; null falls back to `name`.
    displayName: string | null
    /// Configured tint colour for the top-level row.
    color: PaletteColor | null
    /// True when any worktree of the repository has an uncommitted change
    /// (staged, unstaged, or untracked) — the whole-repo dirty rollup.
    dirty: boolean
    /// Worktree paths that are individually dirty; powers the rollup tooltip.
    dirtyWorktrees: string[]
    /// True when any change instance in the repository has a spec commit state
    /// other than `committed`.
    hasUncommittedSpecs: boolean
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

/// Lightweight summary of one archived change for the Archive browser.
/// Mirrors `ArchivedChangeSummary` in `crates/openspec-core/src/types.rs`.
/// Built from the archive directory name plus a heading-only read of
/// `proposal.md` — never a full change parse.
export interface ArchivedChangeSummary {
    /// Logical change id (the directory name with any `YYYY-MM-DD-` prefix stripped).
    id: string
    /// Archive date `YYYY-MM-DD` from the directory-name prefix; null for a
    /// legacy archive directory with no date prefix.
    date: string | null
    /// Title from the change's `proposal.md` heading, if present.
    title: string | null
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
    /// Ordered by the payload: active count descending, then archived count
    /// descending, then label. The frontend caps the list but never re-sorts
    /// it — the comparator lives in `repo_breakdowns` (openspec-core).
    label: string
    activeCount: number
    archivedCount: number
}

export interface LifecycleMetrics {
    archivedInWindow: number
    /// Mean seconds between creation and archival; null when none recoverable.
    avgTimeToArchiveSecs: number | null
}

export interface ShipEntry {
    /// Bare logical change id (date prefix stripped) — for display and to
    /// address the change in navigation.
    changeId: string
    title: string | null
    workspaceLabel: string
    /// Git common dir of the owning repository — the identity of the top-level
    /// row this ship belongs to, matching `RepoView.repoId` and
    /// `RegisteredWorkspace.repoId`. Ships come only from repositories (a flat
    /// workspace has no archive section), so this is never null.
    repoId: string
    /// Registered workspace (worktree) path whose openspec/changes/archive/
    /// holds the change — the Archive browser opens scoped to it.
    worktreePath: string
    /// The dated `YYYY-MM-DD-<id>` archive directory name, addressing the
    /// archive entry for the Archive reader.
    archiveDir: string
    /// Git-recovered archival instant (epoch seconds); null when git could not
    /// supply it (then no relative time is shown).
    archivedAt: number | null
}

export interface DashboardData {
    summary: SummaryMetrics
    repos: RepoBreakdown[]
    /// Days the lifecycle throughput window spans. Presented alongside the
    /// figures it bounds — nothing else on screen defines it.
    lifecycleWindowDays: number
    lifecycle: LifecycleMetrics
    todaysShips: ShipEntry[]
    progress: ProgressData
    /// Per-author leaderboard; render only when it has more than one author.
    leaderboard: LeaderboardEntry[]
}

// -------------------------------------------------------------------------
// Commit garden (mirrors crates/openspec-core/src/garden.rs)
// -------------------------------------------------------------------------

/// One commit, laid out as a node in a workspace's today-graph and attributed
/// to a person (mirrors the rail's laid-out commit plus person fields).
export interface GardenCommit {
    /// Commit sha — stable node identity / React key. Never displayed as text.
    id: string
    /// Row in display order (0 = newest).
    row: number
    /// Lane (column) the node occupies.
    column: number
    subject: string
    /// Branch/tag/HEAD decorations on this commit.
    refs: CommitRef[]
    /// Author date, ISO-8601; the frontend formats the local time on hover.
    date: string
    /// Raw author display, surfaced on hover.
    author: string
    /// Stable attribution key seeding the node's colour.
    personKey: string
    /// Display label for the committer (custom person name, or raw author).
    label: string
    /// Whether this commit resolves to "me" (rendered in the app accent).
    isMe: boolean
}

/// One workspace's plot in the garden: a faithful today-scoped commit graph.
export interface WorkspaceGarden {
    /// Display label for the entry (matches the tree / breakdown label).
    label: string
    /// True when there is nothing to draw today (no commits, non-git, or git
    /// unavailable) — the plot renders a dormant placeholder.
    dormant: boolean
    /// Today's commits, laid out newest-first into lanes.
    commits: GardenCommit[]
    /// Edge segments connecting commits to their parents.
    edges: EdgeSegment[]
    /// Lanes the renderer must size for.
    laneCount: number
}

// -------------------------------------------------------------------------
// Progress layer — mirrors ProgressData in dashboard.rs
// -------------------------------------------------------------------------

/// What was achieved today, with trailing-30-active-day averages. The `*Centi`
/// fields are the average ×100 (integer on the wire so the Rust type stays
/// `Eq`); divide by 100 for display. Change creation is intentionally absent:
/// the hero's second tile shows the live in-flight (active-change) count from
/// the summary metrics, not a today-flow created count. Mirrors
/// `TodayProgress` in `crates/openspec-core/src/dashboard.rs`.
export interface TodayProgress {
    tasksCompleted: number
    changesArchived: number
    commitsLanded: number
    tasksAvgCenti: number
    changesArchivedAvgCenti: number
    commitsAvgCenti: number
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

export interface ProgressData {
    today: TodayProgress
    streak: StreakInfo
    /// Ascending — oldest day first, today last.
    heatmap: HeatmapCell[]
    /// Scope-aware in-flight (active, non-archived) change count for the hero's
    /// second tile. Everyone → all active changes; Me → changes you created.
    inFlight: number
}

// -------------------------------------------------------------------------
// Developer identity (mirrors crates/openspec-core/src/identity.rs)
// -------------------------------------------------------------------------

/// A raw git identity. Either field may be absent (serde omits `None`).
export interface Author {
    name?: string
    email?: string
}

/// The developer's identity configuration: the canonical display name and every
/// alias identity that resolves to "me" (the first is primary / avatar source).
export interface IdentityConfig {
    displayName: string | null
    aliases: Author[]
}

/// A named person on the contributor roster: a custom display name plus the git
/// identities folded onto them, used to name and merge authors on the
/// leaderboard. Mirrors `Person` in `crates/openspec-core/src/identity.rs`.
export interface Person {
    displayName: string | null
    identities: Author[]
}

/// Payload of `get_identity` — the saved config, the contributor roster, and the
/// git identities detected across registered workspaces (offered as alias
/// suggestions in Settings).
export interface IdentityInfo {
    config: IdentityConfig
    people: Person[]
    candidates: Author[]
}

/// The embedded web-server configuration (the desktop "Web UI" toggle).
/// Mirrors `WebServerConfig` in `crates/openspec-app/src/settings.rs`.
export interface WebServerConfig {
    enabled: boolean
    port: number
    tailscale: TailscaleConfig
}

/// Tailscale Serve access settings. Mirrors `TailscaleConfig` in
/// `crates/openspec-app/src/settings.rs`.
export interface TailscaleConfig {
    enabled: boolean
    name: string | null
    allowedLogins: string[]
}

/// One author's standing on the per-author leaderboard, over the window.
/// Mirrors `LeaderboardEntry` in `crates/openspec-core/src/dashboard.rs`.
export interface LeaderboardEntry {
    authorKey: string
    display: string
    isMe: boolean
    ships: number
    tasks: number
    commits: number
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

// Opt-in Claude usage-quota status line (mirrors `openspec_app::quota`).

export type QuotaStatus = "disabled" | "unauthenticated" | "unavailable" | "ok"

export interface QuotaWindow {
    /** Utilization percent, 0..=100. */
    utilization: number
    /** When the window resets, Unix epoch seconds (for a live countdown). */
    resetsAtUnix: number | null
}

/** A per-model scoped weekly window (e.g. Fable), labeled by model display name. */
export interface ScopedQuotaWindow {
    /** The model this weekly limit is scoped to, by display name. */
    model: string
    /** Utilization percent, 0..=100. */
    utilization: number
    /** When the window resets, Unix epoch seconds (for a live countdown). */
    resetsAtUnix: number | null
}

export interface ClaudeQuotaState {
    status: QuotaStatus
    /** A cached snapshot served after a transient failure (de-emphasize it). */
    stale: boolean
    fiveHour: QuotaWindow | null
    sevenDay: QuotaWindow | null
    /** Per-model scoped weekly windows; empty when the response has none. */
    scoped: ScopedQuotaWindow[]
}

// Opt-in ChatGPT usage-quota status line (mirrors
// `openspec_app::chatgpt_quota`). A twin of the Claude mirror above: it
// reuses the same `QuotaStatus` union and the same `quota-updated` event —
// the ChatGPT poller emits the identical `CacheEvent::QuotaUpdated` variant,
// so no new event name was introduced for this provider.

/** One ChatGPT usage window. Unlike Claude's fixed 5h/7d windows, the server
 *  reports each window's actual length, so `windowSecs` drives the gauge's
 *  time axis instead of a hardcoded duration. */
export interface ChatGptQuotaWindow {
    /** Utilization percent, 0..=100. */
    utilization: number
    /** When the window resets, Unix epoch seconds (for a live countdown). */
    resetsAtUnix: number | null
    /** The window's length in seconds (`limit_window_seconds`). `null` when
     *  the response omits it — frontends fall back to 5h (primary) / 7d
     *  (secondary). */
    windowSecs: number | null
}

export interface ChatGptQuotaState {
    status: QuotaStatus
    /** A cached snapshot served after a transient failure (de-emphasize it). */
    stale: boolean
    primary: ChatGptQuotaWindow | null
    secondary: ChatGptQuotaWindow | null
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
export const EVENT_QUOTA_UPDATED = "quota-updated"
export const EVENT_DOCUMENT_CHANGED = "document-changed"
export const EVENT_TOGGLE_SIDEBAR = "toggle-sidebar"
export const EVENT_TOGGLE_COMMIT_RAIL = "toggle-commit-rail"
export const EVENT_DOCUMENT_WIDTH_CHANGED = "document-width-changed"

/// The reading width of the markdown content column — a rung on a fixed ladder,
/// mirroring `DocumentWidth` in `crates/openspec-app/src/settings.rs`. There is
/// no codegen, so these four strings and that enum's `rename_all` output are
/// kept matched by hand; the widths themselves live in `src/docWidth.ts`.
export type DocumentWidth = "compact" | "default" | "wide" | "full"

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

/// One artifact read: its markdown together with when the file it came from was
/// last written. Mirrors `ArtifactRead` in `crates/openspec-app/src/service.rs`.
///
/// The two arrive together because they must describe the same read — paired
/// from two calls, the body and the time could be taken at different instants
/// and nothing would say they had to match.
///
/// `modifiedAt` is unix **seconds** (not milliseconds — the same encoding
/// `ChangeInstance.modifiedAt` uses), and is `null` when the filesystem reports
/// no usable modification time. Null means "no time to show", never 1970: the
/// header renders no label rather than a date the application invented.
export interface ArtifactRead {
    body: string
    modifiedAt: number | null
}

/// Payload of the `document-changed` event. Mirrors `DocumentChangedPayload`
/// in `crates/openspec-app/src/events.rs` — identifiers only, never content:
/// a surface receiving one re-reads through the guarded read.
export interface DocumentChangedPayload {
    root: string
    relPath: string
}

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

/// The workspace file browser — opened by clicking a top-level Repo group or
/// flat workspace row. `root` is the browse root (a repo's main worktree or a
/// flat workspace folder) — identifier-only, so it is routable; the display
/// label is re-derived from `views` where this is rendered (`App.tsx`)
/// rather than carried here (`view-routing`: *Addressable Viewing State*).
export interface FilesRenderTarget {
    kind: "files"
    root: string
    /// The file the browser should have selected, root-relative and
    /// forward-slash separated — present when the address named one (a `file`
    /// address), absent when it named only the browse root (a `files`
    /// address). Identifier-only, like `root`, so it stays routable.
    selectedPath?: string
}

export type RenderTarget =
    | ArtifactRenderTarget
    | CommitRenderTarget
    | DashboardRenderTarget
    | FilesRenderTarget
