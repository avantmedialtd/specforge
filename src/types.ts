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
    // view, loaded lazily per top-level row via `listArchivedRows` (see
    // `ArchivedChangeRow`). The core keeps an in-memory archived set for
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
    /// Every tracked worktree of the repository — user-registered *and*
    /// registry-discovered — in the order the aggregator saw them.
    ///
    /// The frontend's only sight of a discovered worktree. `active` carries
    /// only worktrees hosting an ACTIVE change, and `list_workspaces` returns
    /// only user-registered folders, so neither pool contains the worktree a
    /// change was archived from once its branch stops hosting active work —
    /// which is exactly the worktree a today's-ships link names.
    worktrees: string[]
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
    /// The directory's own name under `openspec/changes/archive/`, verbatim.
    /// Carried rather than reassembled from `id` + `date`: the date strip is a
    /// single anchored match, so an id that itself begins with a date-shaped
    /// prefix only round-trips when stripped exactly once.
    dirName: string
}

/// Which top-level row an archive listing is scoped to. Mirrors `ArchiveScope`
/// in `crates/openspec-core/src/types.rs`.
export type ArchiveScope =
    | { kind: "repo"; repoId: string }
    | { kind: "flat"; workspace: string }

/// One worktree's copy of an archived change. Mirrors `ArchivedChangeCopy` in
/// `crates/openspec-core/src/types.rs`. The `(worktreePath, archiveDir)` pair —
/// not the logical id — is what addresses a read.
export interface ArchivedChangeCopy {
    /// Canonical path of the tracked worktree holding this copy.
    worktreePath: string
    /// This copy's archive directory name within that worktree.
    archiveDir: string
    /// This copy's own archive date; two worktrees can archive one change on
    /// different days, so a copy's date need not be the row's.
    date: string | null
}

/// One logical archived change, pooled across a top-level row's tracked
/// worktrees. Mirrors `ArchivedChangeRow` in `crates/openspec-core/src/types.rs`.
export interface ArchivedChangeRow {
    /// The bare logical change id every copy shares.
    id: string
    /// Display/ordering date — the NEWEST across the row's copies; null only
    /// when every copy is an un-dated legacy directory.
    date: string | null
    /// Title from the first copy (in `copies` order) that has one.
    title: string | null
    /// Every copy this row collapsed, in a deterministic total order.
    copies: ArchivedChangeCopy[]
}

/// Which top-level row a workspace-file listing is scoped to. Mirrors
/// `FileScope` in `crates/openspec-core/src/types.rs` — the file browser's
/// counterpart to `ArchiveScope`, and shaped identically so both surfaces send
/// one discriminated union.
export type FileScope =
    | { kind: "repo"; repoId: string }
    | { kind: "flat"; workspace: string }

/// One worktree's copy of a workspace markdown file. Mirrors
/// `WorkspaceFileCopy` in `crates/openspec-core/src/types.rs`. The
/// `(worktreePath, row path)` pair — not the path alone — is what addresses a
/// read.
export interface WorkspaceFileCopy {
    /// Canonical path of the tracked worktree holding this copy.
    worktreePath: string
}

/// One markdown file of a repository, pooled across its tracked worktrees and
/// de-duplicated on the root-relative path. Mirrors `WorkspaceFileRow` in
/// `crates/openspec-core/src/types.rs`.
export interface WorkspaceFileRow {
    /// Root-relative, forward-slash path every copy shares.
    path: string
    /// Every copy this row collapsed, in a deterministic total order.
    copies: WorkspaceFileCopy[]
    /// True when the row has more than one copy and their on-disk contents are
    /// not all identical. States only *that* they differ — never which is newer
    /// or authoritative.
    differs: boolean
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

// -------------------------------------------------------------------------
// Diff model (mirrors crates/openspec-core/src/diff.rs)
// -------------------------------------------------------------------------

/// What a diff did to one file. `similarity` is git's percentage for a rename
/// or a copy, null when the source gives none.
export type FileStatus =
    | { kind: "added" }
    | { kind: "modified" }
    | { kind: "deleted" }
    | { kind: "renamed"; similarity: number | null }
    | { kind: "copied"; similarity: number | null }
    | { kind: "modeChanged" }
    | { kind: "typeChanged" }

/// A file's content. `hunks` may be empty (no textual change); `withheld` was
/// held back by the budgets and loads on request; `tooLarge` lies past a
/// ceiling or had its patch omitted by a provider; `binary` has no text.
export type DiffContent =
    | { kind: "hunks"; hunks: Hunk[] }
    | { kind: "withheld" }
    | { kind: "tooLarge" }
    | { kind: "binary" }

export type LineKind = "context" | "added" | "removed"

/// One diff line, numbered from its hunk's ranges: a context line carries both
/// numbers, a removed line only `oldNo`, an added line only `newNo`.
/// `noNewline` is present, and true, only on a line that ends its file without
/// a newline.
export interface Line {
    kind: LineKind
    oldNo: number | null
    newNo: number | null
    text: string
    noNewline?: true
}

export interface Hunk {
    oldStart: number
    oldLines: number
    newStart: number
    newLines: number
    /// The heading after the second `@@`, null when the header has none.
    section: string | null
    lines: Line[]
}

/// One file of a diff. `oldPath` is null for an added file and `newPath` for a
/// deleted one; modes are git's octal strings, null when the source gives
/// none; counts are null when the source gives none (a binary file).
export interface DiffFile {
    oldPath: string | null
    newPath: string | null
    oldMode: string | null
    newMode: string | null
    status: FileStatus
    additions: number | null
    deletions: number | null
    content: DiffContent
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

/// One top-level entry's counts. Unordered: nothing renders these as a list any
/// more, so the payload carries no ordering to depend on.
export interface RepoBreakdown {
    label: string
    activeCount: number
    archivedCount: number
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
    /// Every top-level entry's counts, in full. Pure data with no presentation
    /// of its own — no surface renders it as a breakdown; the Dashboard's
    /// footnote reduces it for the registry-wide archived total.
    repos: RepoBreakdown[]
    todaysShips: ShipEntry[]
    progress: ProgressData
}

// -------------------------------------------------------------------------
// Commit garden (mirrors crates/openspec-core/src/garden.rs)
// -------------------------------------------------------------------------

/// One commit, laid out as a node in a workspace's today-graph and attributed
/// to an author (mirrors the rail's laid-out commit plus attribution fields).
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
    authorKey: string
    /// Whether this commit resolves to "me" (rendered in the app accent).
    isMe: boolean
}

/// One workspace's plot in the garden: a faithful today-scoped commit graph.
export interface WorkspaceGarden {
    /// Display label for the entry (matches the tree label).
    label: string
    /// Stable identity for the entry — a repository id, or a flat workspace's
    /// URI. Labels are display names and are not unique, so this is what the
    /// backend's third sort key and this list's React `key` are built on.
    entryKey: string
    /// The entry's registry-wide count of active (non-archived) changes, shown
    /// as the caption's last segment when non-zero. A live state count, not a
    /// today-scoped one — and not the hero's developer-scoped in-flight tile.
    activeCount: number
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

/// Payload of `get_identity` — the saved config plus the git identities detected
/// across registered workspaces (offered as alias suggestions in Settings).
export interface IdentityInfo {
    config: IdentityConfig
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
/// The macOS application menu's Settings… item (Cmd+,) asked the main window
/// to show Settings. Desktop-only and payload-less, like the pane toggles.
export const EVENT_OPEN_SETTINGS = "open-settings"
export const EVENT_DOCUMENT_WIDTH_CHANGED = "document-width-changed"
/// The BitBucket pull-request snapshot changed; re-read it with
/// `get_bitbucket_pull_requests`. Payload-less, like `quota-updated`.
export const EVENT_BITBUCKET_PULL_REQUESTS_UPDATED = "bitbucket-pull-requests-updated"
/// The GitHub pull-request snapshot changed; re-read it with
/// `get_github_pull_requests`. Payload-less, like `quota-updated`.
export const EVENT_GITHUB_PULL_REQUESTS_UPDATED = "github-pull-requests-updated"
/// A pull-request panel's position setting changed; carries
/// `PanelMovedPayload` — which panel, and its new slot — so a listener
/// re-seats that panel directly and leaves the other where it is.
export const EVENT_PULL_REQUEST_PANEL_MOVED = "pull-request-panel-moved"
/// The Commit history switch changed; carries the new value as a bare
/// boolean, so a listener applies it directly. A direct emit on both
/// transports, like `document-width-changed` — never a cache event.
export const EVENT_COMMIT_HISTORY_ENABLED_CHANGED = "commit-history-enabled-changed"
/// A pull request's review progress changed in this service; carries its
/// `PullRequestReference`, and a view showing that pull request re-reads its
/// progress with `get_review_progress`. A service notice: raised on the
/// service's own broadcast, so it reaches every window and every served tab,
/// and never a cache event or a direct emit.
export const EVENT_REVIEW_PROGRESS_CHANGED = "review-progress-changed"
/// A provider's enabled flag was set; carries
/// `PullRequestProviderChangedPayload`. A service notice, like
/// `review-progress-changed`.
export const EVENT_PULL_REQUEST_PROVIDER_CHANGED = "pull-request-provider-changed"

/// Which provider a pull-request panel, or a panel-moved event, is about.
/// Mirrors `PullRequestProvider` in `crates/openspec-app/src/events.rs`.
export type PullRequestProvider = "bitbucket" | "github"

export interface PanelMovedPayload {
    provider: PullRequestProvider
    position: PanelPosition
}

/// A pull request as every pull-request command and address names it, and
/// never by its URL. Mirrors `PullRequestReference` in
/// `crates/openspec-app/src/pull_request_detail.rs`. Two references are equal
/// when their providers and numbers are equal and their owners and
/// repositories are equal ignoring ASCII case.
export interface PullRequestReference {
    provider: PullRequestProvider
    /// A GitHub owner, or a BitBucket workspace.
    owner: string
    repo: string
    number: number
}

/// The `pull-request-provider-changed` payload.
export interface PullRequestProviderChangedPayload {
    provider: PullRequestProvider
    enabled: boolean
}

/// The reading width of the markdown content column — a rung on a fixed ladder,
/// mirroring `DocumentWidth` in `crates/openspec-app/src/settings.rs`. There is
/// no codegen, so these four strings and that enum's `rename_all` output are
/// kept matched by hand; the widths themselves live in `src/docWidth.ts`.
export type DocumentWidth = "compact" | "default" | "wide" | "full"

// Opt-in BitBucket and GitHub pull-request panels (mirrors
// `openspec_app::settings`'s `PanelPosition` / `BitbucketConfigView` /
// `GithubConfigView`, `openspec_app::pull_requests`'s shared row types, and
// the per-provider snapshots in `openspec_app::bitbucket` / `::github`).
// No codegen: the kebab-case slot names, the camelCase keys and the status
// strings below are kept matched with the Rust side by hand, and pinned there
// by `crates/openspec-app/tests/wire_shape.rs`.

/// Which of the four side-pane slots a pull-request panel renders in. A
/// persisted application setting per provider, not per-window view state.
export type PanelPosition = "left-top" | "left-bottom" | "right-top" | "right-bottom"

/// The BitBucket configuration as the frontend may see it. The token is
/// write-only: no command returns it, only whether one is set.
export interface BitbucketConfigView {
    enabled: boolean
    username: string | null
    tokenSet: boolean
    /** The poll cadence in seconds (not exposed in Settings). */
    refreshSecs: number
    panelPosition: PanelPosition
}

/// The GitHub configuration as the frontend may see it. The token is
/// write-only: no command returns it, only whether one is set.
export interface GithubConfigView {
    enabled: boolean
    tokenSet: boolean
    /** The poll cadence in seconds (not exposed in Settings). */
    refreshSecs: number
    panelPosition: PanelPosition
}

/** Status of the latest pull-request refresh. `disabled` renders no panel. */
export type PullRequestsStatus = "disabled" | "unauthenticated" | "unavailable" | "ok"

/** A pull request's review state. Absent (`null` on a row) when the response
 *  carried no participants, so "unknown" stays distinct from "no activity". */
export interface ReviewSummary {
    /** Participants who approved, excluding the author. */
    approvals: number
    /** Participants who requested changes, excluding the author. */
    changesRequested: number
    /** Reviewers who have neither approved nor requested changes. */
    pending: number
}

/** The latest commit's check rollup, reduced to three states. Absent (`null`
 *  on a row) when no checks ran, or when the provider reports none at all. */
export type ChecksState = "passing" | "failing" | "pending"

/** One open pull request, shared by both providers' snapshots. A field a
 *  provider cannot know carries its "nothing to show" value: BitBucket rows
 *  have `author: null`, `checks: null`, `conflicting: false` and
 *  `unresolvedThreads: 0`; GitHub rows have `openTasks: 0`. */
export interface PullRequestSummary {
    /** Unique within its repository only (on GitHub, the PR number). */
    id: number
    title: string
    /** The destination repository's `owner/repo` name. */
    repoFullName: string
    /** The head repository's `owner/repo` name — the fork a fork's pull
     *  request comes from; empty when the provider reports none (a deleted
     *  fork). Used only to link rows to worktrees. */
    sourceRepoFullName: string
    sourceBranch: string
    destinationBranch: string
    /** The pull request's web page; empty when the provider gave no https
     *  link on its own host. */
    url: string
    draft: boolean
    /** Updated time, Unix epoch seconds. */
    updatedAtUnix: number
    review: ReviewSummary | null
    /** Open (unresolved) BitBucket tasks. */
    openTasks: number
    /** The author's login, when the provider reports one. */
    author: string | null
    /** The latest commit's check rollup; `null` when no checks ran. */
    checks: ChecksState | null
    /** True only when the provider reports the PR as conflicting: a
     *  mergeability not yet computed is not a conflict. */
    conflicting: boolean
    /** Unresolved GitHub review conversations. */
    unresolvedThreads: number
}

/** The BitBucket snapshot its panel renders (`get_bitbucket_pull_requests`). */
export interface BitbucketPullRequestsState {
    status: PullRequestsStatus
    /** Rows kept from an earlier refresh after a transient failure. */
    stale: boolean
    /** When the list was fetched, Unix epoch seconds (kept while stale);
     *  `null` when there is no list at all. */
    fetchedAtUnix: number | null
    /** Newest-updated first, across every workspace. */
    pullRequests: PullRequestSummary[]
    /** Workspaces that answered 403/404 and were skipped (informational). */
    skippedWorkspaces: string[]
}

/** The GitHub snapshot its panel renders (`get_github_pull_requests`). */
export interface GithubPullRequestsState {
    status: PullRequestsStatus
    /** Rows kept from an earlier refresh after a transient failure. */
    stale: boolean
    /** When the lists were fetched, Unix epoch seconds (kept while stale);
     *  `null` when there is no list at all. */
    fetchedAtUnix: number | null
    /** The account's own open pull requests, newest-updated first. */
    authored: PullRequestSummary[]
    /** Open pull requests awaiting the account's review, newest-updated
     *  first. Never repeats a URL already in `authored`. */
    reviewRequested: PullRequestSummary[]
    /** Entries GitHub withheld (returned as null): typically an organisation
     *  enforcing single sign-on the token is not authorised for. */
    withheld: number
}

// Pull-request ↔ worktree links (mirrors `openspec_app::pull_request_links`;
// `get_pull_request_links`). A local join of both providers' rows with the
// tracked worktrees; no network. Pull requests are keyed by web URL.

/** Whether a linked pull request is the viewer's own or awaiting their
 *  review. */
export type PullRequestRole = "authored" | "reviewRequested"

/** A pull request as a worktree's header chip needs it. */
export interface LinkedPullRequest {
    provider: PullRequestProvider
    role: PullRequestRole
    id: number
    title: string
    url: string
    /** The destination repository's `owner/repo` name. */
    repoFullName: string
    draft: boolean
    checks: ChecksState | null
    conflicting: boolean
    review: ReviewSummary | null
}

/** One linked worktree and the pull requests linked to it, BitBucket first,
 *  then GitHub authored, then GitHub review-requested. */
export interface WorktreePullRequests {
    worktreePath: string
    pullRequests: LinkedPullRequest[]
}

/** A worktree a pull request is linked to. */
export interface LinkedWorktree {
    repoId: string
    worktreePath: string
    branch: string | null
}

/** One linked pull request (by web URL) and its worktrees, main worktrees
 *  first, then by path. */
export interface PullRequestWorktrees {
    url: string
    worktrees: LinkedWorktree[]
}

/** The links snapshot (`get_pull_request_links`). */
export interface PullRequestLinks {
    worktrees: WorktreePullRequests[]
    pullRequests: PullRequestWorktrees[]
}

// Pull-request detail (mirrors `openspec_app::pull_request_detail`;
// `get_pull_request_detail`, and a withheld file's `get_pull_request_file`,
// which answers a `DiffFile`). One model whichever provider it came from. No
// codegen: every key, `kind` and string below is pinned by
// `crates/openspec-app/tests/wire_shape.rs`.

/** The side of a diff a review thread is anchored on: GitHub's `LEFT` and
 *  BitBucket's `inline.from` are old, `RIGHT` and `inline.to` new. */
export type DiffSide = "old" | "new"

/** A submitted GitHub review's state. A pending review never arrives. */
export type ReviewState = "approved" | "changesRequested" | "commented" | "dismissed"

/** One check's state, mapped from every value its provider reports;
 *  `unknown` for a value SpecForge does not know. */
export type PullRequestCheckState =
    | "passing"
    | "failing"
    | "pending"
    | "neutral"
    | "skipped"
    | "cancelled"
    | "unknown"

/** One comment, in the conversation or in a review thread. `body` is
 *  untrusted markdown. */
export interface PullRequestComment {
    /** GitHub's node id, or BitBucket's comment id as text. */
    id: string
    /** A GitHub login or a BitBucket display name; null for a deleted
     *  account. */
    author: string | null
    body: string
    /** When it was posted (a review: submitted), Unix epoch seconds; 0 when
     *  unreadable. */
    postedAtUnix: number
    /** Its web page on its provider's own site, else null. */
    url: string | null
    /** Non-null when GitHub reports it minimised: render it collapsed behind
     *  this reason, which is "" when GitHub states none. */
    minimizedReason: string | null
    /** BitBucket reports it deleted. */
    deleted: boolean
}

/** One conversation entry: a comment, or a submitted review's summary,
 *  which names the review's state. */
export interface ConversationEntry {
    review: ReviewState | null
    comment: PullRequestComment
}

/** One check of the head commit. `url` is as its provider gives it, whatever
 *  the scheme: link it only when it is an absolute http(s) URL with a host. */
export interface PullRequestCheck {
    name: string
    state: PullRequestCheckState
    url: string | null
}

/** One review thread: a GitHub review thread, or a BitBucket inline comment
 *  with its replies. Never without a comment. */
export interface ReviewThread {
    id: string
    path: string
    /** The new side when the provider names none. */
    side: DiffSide
    /** Null for a comment on the whole file, or a line that no longer
     *  exists. */
    line: number | null
    /** The first line of a range. */
    startSide: DiffSide | null
    startLine: number | null
    /** GitHub's lines when the thread was started, for an outdated thread's
     *  label; null on BitBucket. */
    originalLine: number | null
    originalStartLine: number | null
    resolved: boolean
    outdated: boolean
    comments: PullRequestComment[]
}

/** One pull request as the view renders it. */
export interface PullRequestDetail {
    /** Spelt as the row it was read through spells it. */
    reference: PullRequestReference
    /** The row it was read through: the header's signals once the live row
     *  is gone. */
    row: PullRequestSummary
    headBranch: string
    baseBranch: string
    /** The commits a withheld file's load and a viewed mark name. */
    headCommit: string
    baseCommit: string
    author: string | null
    /** Untrusted markdown. */
    description: string
    /** Comments and review summaries, in submission order. */
    conversation: ConversationEntry[]
    checks: PullRequestCheck[]
    threads: ReviewThread[]
    /** Under the line and byte budgets; a `withheld` file loads through
     *  `get_pull_request_file`. */
    files: DiffFile[]
    /** Files changed but not listed (GitHub's past the thousandth). */
    unlistedFiles: number
    /** When it was read, Unix epoch seconds. */
    readAtUnix: number
    noLongerListed: boolean
}

/** What `get_pull_request_detail` answers. Only `detail` and `deferred` carry
 *  a detail; `deferred` names when a read becomes possible. */
/// Why a file read could not complete (`FileReadFailure` in
/// `pull_request_detail.rs`). The view words each (`fileFailureText`).
export type FileReadFailure = "deferred" | "unauthenticated" | "unavailable" | "refused" | "transient"

/// What `get_pull_request_file` answers (`PullRequestFileOutcome` in
/// `pull_request_detail.rs`): the file; `changed`, after which the view reads
/// the pull request again; or `failed` with its reason, `untilUnix` set only
/// when `deferred`.
export type PullRequestFileOutcome =
    | { kind: "file"; file: DiffFile }
    | { kind: "changed" }
    | { kind: "failed"; reason: FileReadFailure; untilUnix: number | null }

export type PullRequestDetailOutcome =
    | { kind: "detail"; detail: PullRequestDetail }
    | { kind: "notListed" }
    | { kind: "notCached" }
    | { kind: "refused" }
    | { kind: "unauthenticated" }
    | { kind: "unavailable" }
    | { kind: "deferred"; untilUnix: number; detail: PullRequestDetail | null }
    | { kind: "transient" }

/** A file's review state, derived from its keys alone: `viewed` when its
 *  stored key equals its current one or every known hunk is viewed,
 *  `changedSinceViewed` when a stored key differs and some hunk is not
 *  viewed, `partlyViewed` when no key is stored but some hunks are viewed (or,
 *  its hunks not known, some hunk keys are stored), and `unviewed` otherwise.
 *  Mirrors `FileReviewState` in `crates/openspec-app/src/review_progress.rs`. */
export type FileReviewState = "viewed" | "changedSinceViewed" | "partlyViewed" | "unviewed"

/** One file of a pull request's review progress, by its key path
 *  (`newPath ?? oldPath`). */
export interface FileReviewProgress {
    path: string
    state: FileReviewState
    /** True when the file is keyed by the head commit and the base branch, for
     *  want of patch text or a blob id (as every BitBucket file without text
     *  is), so any push or retarget marks it changed since viewed and the view
     *  says why. */
    keyedByHead: boolean
    /** Whether each of its hunks is viewed, in the order the view renders
     *  them; null while its hunks are not known (a file GitHub sent without its
     *  patch, before it is loaded, or a file without hunks). Positional, so it
     *  holds only for the detail `ReviewProgress.headCommit`/`baseCommit`
     *  name. */
    hunks: boolean[] | null
}

/** What `get_review_progress` answers for one pull request, from the cached
 *  detail's files. Mirrors `ReviewProgress` in
 *  `crates/openspec-app/src/review_progress.rs`. */
export interface ReviewProgress {
    files: FileReviewProgress[]
    /** Files whose state is `viewed`. */
    viewed: number
    /** Files whose state is `changedSinceViewed`. A `partlyViewed` file counts
     *  in neither. */
    changedSinceViewed: number
    /** Every file of the cached detail. */
    total: number
    /** The head commit at the last mark, which dates the changed count
     *  ("since you last marked, at abc1234"); null before any mark. */
    lastMarkedHead: string | null
    /** The cached detail's head and base commits, which the files' positional
     *  `hunks` belong to: a view applies them only to a detail with both. */
    headCommit: string
    baseCommit: string
}

// -------------------------------------------------------------------------
// Tree-selection discriminated union.
//
// `workspaceUri` is the path used to read artifacts. For Flat workspaces
// that's the registered workspace path; for git worktrees it's the
// individual worktree path. Either way the detail pane uses it as-is.
// -------------------------------------------------------------------------

/// The top-level row a change row hangs beneath — a repository group or a
/// non-git workspace. A change row names its container rather than a worktree:
/// which INSTANCE is read is chosen in the change header, not in the tree
/// (`spec-browser`: *Instance Switcher in the Change Header*).
export type TreeContainer =
    | { kind: "repo"; repoId: string }
    | { kind: "flat"; workspaceUri: string }

/// What the tree can select, now that it stops at the change row
/// (`spec-browser`: *Workspace Tree Hierarchy*). Three variants, no depth:
/// the two top-level row kinds and the change row. Artifact, spec, section,
/// task, logical-change and instance selections are gone — artifacts and
/// instances are chosen in the detail pane's change header, and there are no
/// section or task rows to select.
export type TreeSelection =
    | { kind: "workspace"; workspaceUri: string }
    | { kind: "repo"; repoId: string }
    | { kind: "change"; container: TreeContainer; changeName: string }

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
    /// The browse root's identifier: a flat workspace's folder path, or — for a
    /// Repo group — the **repository** identifier, since the listing is pooled
    /// across every tracked worktree of it rather than rooted at one. Which
    /// worktree's copy a selected file is read from is resolved at load time
    /// from that listing and is deliberately not part of the address
    /// (`view-routing`: *A file address carries no worktree segment*).
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
