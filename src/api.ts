import { invoke as tauriInvoke } from "@tauri-apps/api/core"
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event"
import type {
    ArchiveScope,
    ArchivedChangeRow,
    ArtifactRead,
    ArtifactReadKind,
    ArtifactStatus,
    Author,
    BitbucketConfigView,
    BitbucketPullRequestsState,
    CacheUpdatedPayload,
    DocumentChangedPayload,
    DocumentWidth,
    ChangeAddedPayload,
    ChangeArchivedPayload,
    ChangeData,
    ChatGptQuotaState,
    ClaudeQuotaState,
    CommitGraph,
    DashboardData,
    DiffFile,
    FileScope,
    GithubConfigView,
    GithubPullRequestsState,
    GraphChangedPayload,
    IdentityInfo,
    ImageVersions,
    InstancePayload,
    LogicalChangePayload,
    PaletteColor,
    PanelMovedPayload,
    PanelPosition,
    PullRequestDetailOutcome,
    PullRequestFileOutcome,
    PullRequestImageOutcome,
    PullRequestLinks,
    PullRequestProviderChangedPayload,
    PullRequestReference,
    RegisteredWorkspace,
    ReviewProgress,
    ReviewSkipPatterns,
    ReviewSkipPatternsChangedPayload,
    SkipPatternsOutcome,
    WebServerConfig,
    WorkspaceFileRow,
    WorkspaceGarden,
    WorkspaceRemovedPayload,
    WorkspaceView,
} from "./types"
import {
    EVENT_BITBUCKET_PULL_REQUESTS_UPDATED,
    EVENT_CACHE_UPDATED,
    EVENT_COMMIT_HISTORY_ENABLED_CHANGED,
    EVENT_DOCUMENT_CHANGED,
    EVENT_DOCUMENT_WIDTH_CHANGED,
    EVENT_CHANGE_ADDED,
    EVENT_CHANGE_ARCHIVED,
    EVENT_GITHUB_PULL_REQUESTS_UPDATED,
    EVENT_GRAPH_CHANGED,
    EVENT_INSTANCE_ADDED,
    EVENT_INSTANCE_REMOVED,
    EVENT_LOGICAL_CHANGE_ADDED,
    EVENT_LOGICAL_CHANGE_ARCHIVED,
    EVENT_OPEN_SETTINGS,
    EVENT_PULL_REQUEST_PANEL_MOVED,
    EVENT_PULL_REQUEST_PROVIDER_CHANGED,
    EVENT_QUOTA_UPDATED,
    EVENT_REVIEW_PROGRESS_CHANGED,
    EVENT_REVIEW_SKIP_PATTERNS_CHANGED,
    EVENT_TOGGLE_COMMIT_RAIL,
    EVENT_TOGGLE_SIDEBAR,
    EVENT_WORKSPACE_PRESENTATION_UPDATED,
    EVENT_WORKSPACE_REMOVED,
} from "./types"
import { CLIENT_ID, subscribeToEventStream } from "./eventStream"
import { imageWindowName, imageWindowPath } from "./imageWindow"
import { pullRequestWindowName, pullRequestWindowPath } from "./pullRequestOpen"
import { shortHash } from "./routing/slug"

// Re-exported for call sites that import the artifact-kind union from the
// API surface (the canonical definition lives in ./types).
export type { ArtifactRead, ArtifactReadKind } from "./types"

// -------------------------------------------------------------------------
// Transport — host detection
// -------------------------------------------------------------------------
//
// The same bundle runs in two hosts. Inside the Tauri desktop shell it uses
// in-process `invoke`/`listen`; served over HTTP by `specforge-web` it uses
// `fetch` + `EventSource`. Everything below `invokeLogged`/`listenLogged` is
// transport-agnostic, so the entire command surface is shared.

/// True when running inside the native Tauri shell. Tauri v2 injects
/// `__TAURI_INTERNALS__`; the legacy `__TAURI__` global is checked too.
export function isTauri(): boolean {
    return (
        typeof window !== "undefined" &&
        ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
    )
}

/// True when running as a browser tab served by the local web server.
export function isWeb(): boolean {
    return !isTauri()
}

// The web command transport: POST { command, args } to the server's invoke
// endpoint, mirroring Tauri's `invoke(command, args)` shape. A non-2xx response
// carries a `{ error }` envelope which becomes a thrown Error, matching how a
// rejected Tauri command surfaces to callers.
async function webInvoke<T>(
    command: string,
    args?: Record<string, unknown>,
): Promise<T> {
    const res = await fetch("/api/invoke", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ command, args: args ?? {} }),
    })
    if (!res.ok) {
        let message = `${command} failed (${res.status})`
        try {
            const body = await res.json()
            if (body && typeof body.error === "string") message = body.error
        } catch {
            // Non-JSON error body — keep the status-based message.
        }
        throw new Error(message)
    }
    // 200: the body is the raw result JSON (`null` for unit-returning commands).
    return (await res.json()) as T
}

// The web event stream. Its lifecycle — including recovery after the document
// has been suspended — lives in ./eventStream; this only adapts SSE frames into
// typed payloads.

function webListen<T>(
    event: string,
    handler: (payload: T) => void,
): Promise<UnlistenFn> {
    const listener = ((e: MessageEvent) => {
        let payload: T
        try {
            payload = e.data ? (JSON.parse(e.data) as T) : (undefined as T)
        } catch {
            payload = undefined as T
        }
        handler(payload)
    }) as EventListener
    const unlisten: UnlistenFn = subscribeToEventStream(event, listener)
    return Promise.resolve(unlisten)
}

// Wraps the active transport so every command logs its name, args, and
// result/error in dev. `import.meta.env.DEV` is constant-folded out of
// production builds so the logging has zero runtime cost there.
async function invokeLogged<T>(
    command: string,
    args?: Record<string, unknown>,
    // What the dev log shows in place of `args`, for the one command whose
    // arguments carry a secret: a credential is sent, never logged
    // (`bitbucket-pull-requests`: *Privacy and Safety*).
    loggedArgs: Record<string, unknown> | undefined = args,
): Promise<T> {
    if (import.meta.env.DEV) {
        console.log(`[api] → ${command}`, loggedArgs ?? {})
    }
    try {
        const result = isTauri()
            ? await tauriInvoke<T>(command, args)
            : await webInvoke<T>(command, args)
        if (import.meta.env.DEV) {
            console.log(`[api] ← ${command}`, result)
        }
        return result
    } catch (err) {
        if (import.meta.env.DEV) {
            console.warn(`[api] ✗ ${command}`, err)
        }
        throw err
    }
}

function listenLogged<T>(
    event: string,
    handler: (payload: T) => void,
): Promise<UnlistenFn> {
    const wrapped = (payload: T) => {
        if (import.meta.env.DEV) {
            console.log(`[event] ${event}`, payload)
        }
        handler(payload)
    }
    if (isTauri()) {
        return tauriListen<T>(event, (e) => wrapped(e.payload))
    }
    return webListen<T>(event, wrapped)
}

// -------------------------------------------------------------------------
// Commands
// -------------------------------------------------------------------------

export async function registerWorkspace(path: string): Promise<RegisteredWorkspace> {
    return invokeLogged<RegisteredWorkspace>("register_workspace", { path })
}

export async function unregisterWorkspace(path: string): Promise<boolean> {
    return invokeLogged<boolean>("unregister_workspace", { path })
}

export async function listWorkspaces(): Promise<RegisteredWorkspace[]> {
    return invokeLogged<RegisteredWorkspace[]>("list_workspaces")
}

export async function getChanges(workspace: string): Promise<ChangeData[]> {
    return invokeLogged<ChangeData[]>("get_changes", { workspace })
}

/// The Archive view's listing for one top-level row: the union of archived
/// changes across every tracked worktree of a repository (or the single folder
/// of a flat workspace), de-duplicated on the bare logical change id, newest
/// first. Read on demand when the view opens or its scope changes.
export async function listArchivedRows(
    scope: ArchiveScope,
): Promise<ArchivedChangeRow[]> {
    return invokeLogged<ArchivedChangeRow[]>("list_archived_rows", { scope })
}

/// The file browser's listing for one top-level row: the union of the markdown
/// enumerations of every tracked worktree of a repository (or the single folder
/// of a flat workspace), de-duplicated on the root-relative path, path
/// ascending. Rows whose copies differ on disk arrive marked. Read on demand
/// when the browser opens, its browse root changes, or refresh is activated.
export async function listWorkspaceFileRows(
    scope: FileScope,
): Promise<WorkspaceFileRow[]> {
    return invokeLogged<WorkspaceFileRow[]>("list_workspace_file_rows", {
        scope,
    })
}

/// Reports which artifacts an archived change has on disk, so the Archive view
/// can offer per-artifact navigation. `dirName` is the archive directory name
/// (`<YYYY-MM-DD>-<id>`).
export async function archivedArtifactStatus(
    workspace: string,
    dirName: string,
): Promise<ArtifactStatus> {
    return invokeLogged<ArtifactStatus>("archived_artifact_status", {
        workspace,
        dirName,
    })
}

export async function getWorkspaceViews(): Promise<WorkspaceView[]> {
    return invokeLogged<WorkspaceView[]>("get_workspace_views")
}

export async function getActiveCount(): Promise<number> {
    return invokeLogged<number>("get_active_count")
}

/// Aggregate the global Dashboard payload across every registered workspace.
/// The progress layer is always resolved to the canonical developer over all
/// available history — there is no audience (Me/Everyone) selector.
export async function getDashboard(): Promise<DashboardData> {
    return invokeLogged<DashboardData>("get_dashboard")
}

/// The commit garden: one stylized plant per top-level entry, grown from today's
/// commits.
export async function getCommitGarden(): Promise<WorkspaceGarden[]> {
    return invokeLogged<WorkspaceGarden[]>("get_commit_garden")
}

/// The developer-identity configuration plus detected candidate identities.
export async function getIdentity(): Promise<IdentityInfo> {
    return invokeLogged<IdentityInfo>("get_identity")
}

/// Set the canonical display name (pass null to clear it).
export async function setDisplayName(name: string | null): Promise<void> {
    return invokeLogged<void>("set_display_name", { name })
}

/// Replace the set of alias identities that resolve to "me".
export async function setIdentityAliases(aliases: Author[]): Promise<void> {
    return invokeLogged<void>("set_identity_aliases", { aliases })
}

export async function readArtifact(
    workspace: string,
    changeId: string,
    artifactKind: ArtifactReadKind,
    capability?: string,
): Promise<ArtifactRead> {
    return invokeLogged<ArtifactRead>("read_artifact", {
        workspace,
        changeId,
        artifactKind,
        capability,
    })
}

/// The markdown files under a workspace browse root (a repo's main worktree
/// or a flat workspace folder) — gitignore-aware for a git repository, a
/// bounded walk otherwise. Sorted, forward-slash relative paths.
export async function listMarkdownFiles(root: string): Promise<string[]> {
    return invokeLogged<string[]>("list_markdown_files", { root })
}

/// Read one markdown file from a workspace browse root. Unlike
/// `readArtifact`, not confined to `openspec/changes/`.
export async function readWorkspaceFile(
    root: string,
    relPath: string,
): Promise<string> {
    return invokeLogged<string>("read_workspace_file", { root, relPath })
}

/// Opens a link clicked in rendered artifact markdown via the OS default
/// handler — an external URL in the system browser, or a validated,
/// allow-listed workspace file. `root` is the authorized root the rendering
/// surface already holds (a registered workspace, or a file-browser's browse
/// root); `basePath` is the root-relative path of the markdown file being
/// viewed, which relative hrefs in `href` resolve against. Desktop-only: not
/// present on the web dispatch surface (see `isWeb()` call sites), so this
/// must never be invoked there. Rejects — never navigates or throws
/// synchronously — when the link is refused, dangling, or inert at the
/// service layer; callers surface that as a quiet failure.
export async function openArtifactLink(
    root: string,
    basePath: string,
    href: string,
): Promise<void> {
    return invokeLogged<void>("open_artifact_link", { root, basePath, href })
}

/// Build the commit-graph for a repository (identified by its git common dir
/// `repoId`), reading up to `limit` commits across all refs.
export async function getCommitGraph(
    repoId: string,
    limit: number,
): Promise<CommitGraph> {
    return invokeLogged<CommitGraph>("get_commit_graph", { repoId, limit })
}

/// The files a commit changed, as the diff model under the line and byte
/// budgets: each eager file with its hunks, every other patched file withheld
/// with its counts, against the first parent, or the empty tree for a root
/// commit (`commit-graph`: *Commit Detail View*).
export async function getCommitDetail(repoId: string, sha: string): Promise<DiffFile[]> {
    return invokeLogged<DiffFile[]>("get_commit_detail", { repoId, sha })
}

/// One file of a commit read alone, as a withheld file's "Load diff" asks,
/// against the same base as the rest of the commit's diff: its hunks, or too
/// large to preview past the per-file ceiling. `path` is the file's key, and a
/// renamed file passes its old path too, so it loads as one renamed file.
export async function getCommitDiff(
    repoId: string,
    sha: string,
    path: string,
    oldPath?: string,
): Promise<DiffFile> {
    return invokeLogged<DiffFile>("get_commit_diff", { repoId, sha, path, oldPath })
}

/// An image file's two versions in a commit (`commit-graph`: *Commit Detail
/// View*), read as its section nears the view. A renamed file passes its old
/// path too.
export async function getCommitFileImage(
    repoId: string,
    sha: string,
    path: string,
    oldPath?: string,
): Promise<ImageVersions> {
    return invokeLogged<ImageVersions>("get_commit_file_image", { repoId, sha, path, oldPath })
}

export async function getLaunchOnLogin(): Promise<boolean> {
    return invokeLogged<boolean>("get_launch_on_login")
}

export async function setLaunchOnLogin(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_launch_on_login", { enabled })
}

/// The latest opt-in Claude usage-quota snapshot (`status: "disabled"` when off).
export async function getClaudeQuota(): Promise<ClaudeQuotaState> {
    return invokeLogged<ClaudeQuotaState>("get_claude_quota")
}

export async function getClaudeQuotaEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_claude_quota_enabled")
}

export async function setClaudeQuotaEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_claude_quota_enabled", { enabled })
}

/// The latest opt-in ChatGPT usage-quota snapshot (`status: "disabled"` when
/// off). A twin of `getClaudeQuota`.
export async function getChatGptQuota(): Promise<ChatGptQuotaState> {
    return invokeLogged<ChatGptQuotaState>("get_chatgpt_quota")
}

export async function getChatGptQuotaEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_chatgpt_quota_enabled")
}

export async function setChatGptQuotaEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_chatgpt_quota_enabled", { enabled })
}

/// The BitBucket pull-request configuration. The token is write-only: this
/// reports `tokenSet`, never the token itself, on either transport.
export async function getBitbucketConfig(): Promise<BitbucketConfigView> {
    return invokeLogged<BitbucketConfigView>("get_bitbucket_config")
}

export async function setBitbucketEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_bitbucket_enabled", { enabled })
}

/// Replace the stored BitBucket credential pair; an empty token clears the
/// stored one. The token is sent, but the dev log only ever sees a placeholder.
export async function setBitbucketCredentials(
    username: string,
    apiToken: string,
): Promise<void> {
    return invokeLogged<void>(
        "set_bitbucket_credentials",
        { username, apiToken },
        { username, apiToken: apiToken ? "<redacted>" : "" },
    )
}

/// Persist the BitBucket panel's slot. The backend emits
/// `pull-request-panel-moved` with `provider: "bitbucket"`, so windows already
/// open — and connected browser skins — re-seat the panel; the caller does not
/// have to tell them.
export async function setBitbucketPanelPosition(position: PanelPosition): Promise<void> {
    return invokeLogged<void>("set_bitbucket_panel_position", { position })
}

/// The latest BitBucket pull-request snapshot (`status: "disabled"` when off).
export async function getBitbucketPullRequests(): Promise<BitbucketPullRequestsState> {
    return invokeLogged<BitbucketPullRequestsState>("get_bitbucket_pull_requests")
}

/// The GitHub pull-request configuration. The token is write-only: this
/// reports `tokenSet`, never the token itself, on either transport.
export async function getGithubConfig(): Promise<GithubConfigView> {
    return invokeLogged<GithubConfigView>("get_github_config")
}

export async function setGithubEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_github_enabled", { enabled })
}

/// Replace the stored GitHub token; an empty token clears it. The token is
/// sent, but the dev log only ever sees a placeholder.
export async function setGithubToken(token: string): Promise<void> {
    return invokeLogged<void>(
        "set_github_token",
        { token },
        { token: token ? "<redacted>" : "" },
    )
}

/// Persist the GitHub panel's slot. The backend emits
/// `pull-request-panel-moved` with `provider: "github"`, so windows already
/// open re-seat the GitHub panel and leave the BitBucket one where it is.
export async function setGithubPanelPosition(position: PanelPosition): Promise<void> {
    return invokeLogged<void>("set_github_panel_position", { position })
}

/// The latest GitHub pull-request snapshot (`status: "disabled"` when off).
export async function getGithubPullRequests(): Promise<GithubPullRequestsState> {
    return invokeLogged<GithubPullRequestsState>("get_github_pull_requests")
}

/// Which pull requests are linked to which tracked worktrees, joined locally
/// from both providers' snapshots and the repositories' remotes and branches
/// (`pull-request-worktree-links`). No network, no host effect, so it is served
/// on both transports. There is no event of its own: re-read it whenever the
/// workspace views or either pull-request snapshot change.
export async function getPullRequestLinks(): Promise<PullRequestLinks> {
    return invokeLogged<PullRequestLinks>("get_pull_request_links")
}

/// One pull request's detail, as its view renders it (`pull-request-viewer`:
/// *Detail Reads Are Scoped to the Snapshot*). Served on both transports. The
/// service looks `reference` up in its provider's snapshot and reads only
/// through the matched row, so nothing else the caller says reaches a
/// request. `manual` says only that the ask is a manual refresh, which may
/// pass the 60-second freshness rule; `cachedOnly` asks only for what the
/// cache holds, whatever its age, and never sends. Every answer is an outcome,
/// a deferral and a refusal included; only the transport failing rejects.
export async function getPullRequestDetail(
    reference: PullRequestReference,
    manual: boolean,
    cachedOnly: boolean,
): Promise<PullRequestDetailOutcome> {
    return invokeLogged<PullRequestDetailOutcome>("get_pull_request_detail", {
        reference,
        manual,
        cachedOnly,
    })
}

/// One withheld file of a pull request's detail, as its "Load diff" asks for
/// it (`pull-request-viewer`: *Detail Reads Are Scoped to the Snapshot*). A
/// file the budgets withheld comes from the cached detail with no request; a
/// file GitHub sent without its patch is read from GitHub the first time. `path`
/// is the file's key (`newPath ?? oldPath`), and `head` and `base` are the
/// commits of the detail the view rendered. Answers `changed` with nothing
/// cached, for a path not among the detail's files, and when either commit
/// differs from the cached detail's, after which the view reads the pull
/// request again; and `failed` with a reason when the file could not be read.
export async function getPullRequestFile(
    reference: PullRequestReference,
    path: string,
    head: string,
    base: string,
): Promise<PullRequestFileOutcome> {
    return invokeLogged<PullRequestFileOutcome>("get_pull_request_file", {
        reference,
        path,
        head,
        base,
    })
}

/// An image file's two versions in a pull request (`pull-request-viewer`:
/// *Pull-Request Image Reads*), read when the reader asks. `head` and `base`
/// are the commits the view rendered.
export async function getPullRequestFileImage(
    reference: PullRequestReference,
    path: string,
    head: string,
    base: string,
): Promise<PullRequestImageOutcome> {
    return invokeLogged<PullRequestImageOutcome>("get_pull_request_file_image", {
        reference,
        path,
        head,
        base,
    })
}

/// A pull request's review progress on this machine, for the files of its
/// cached detail (`pull-request-viewer`: *Review Progress*). Served on both
/// transports, since progress is the reader's local state and never reaches
/// either host. Rejects while the provider is off.
export async function getReviewProgress(reference: PullRequestReference): Promise<ReviewProgress> {
    return invokeLogged<ReviewProgress>("get_review_progress", { reference })
}

/// Mark one file of a pull request viewed, or unmark it. `path` is the file's
/// key, and `head` and `base` are the commits of the detail the view rendered:
/// the service computes the file's key from its cached detail, never from the
/// caller, and refuses when nothing is cached, the path is not among its
/// files, or either commit differs. Each stored change raises
/// `review-progress-changed` for every view of the pull request.
export async function setFileViewed(
    reference: PullRequestReference,
    path: string,
    viewed: boolean,
    head: string,
    base: string,
): Promise<void> {
    return invokeLogged<void>("set_file_viewed", { reference, path, viewed, head, base })
}

/// Mark one hunk of a pull request's file viewed, or unmark it. `hunk` counts
/// the file's hunks from zero, in the order the view renders them; `head` and
/// `base` are the commits of the detail it rendered. The service keys the hunk
/// by its content from its cached detail, and refuses as `setFileViewed` does,
/// and also while the file's hunks have not been read or past its last hunk.
/// Marking a file's last unviewed hunk marks the file.
export async function setHunkViewed(
    reference: PullRequestReference,
    path: string,
    hunk: number,
    viewed: boolean,
    head: string,
    base: string,
): Promise<void> {
    return invokeLogged<void>("set_hunk_viewed", { reference, path, hunk, viewed, head, base })
}

/// Include one file of a pull request in the review, or exclude it: the
/// Review and Skip controls of a file the skip patterns match
/// (`pull-request-viewer`: *Review Progress*). `path` is the file's key, and
/// `head` and `base` are the commits of the detail the view rendered; the
/// service refuses as it refuses `setFileViewed`. Kept by path, so no push
/// undoes it. Each stored change raises `review-progress-changed` for every
/// view of the pull request.
export async function setFileIncluded(
    reference: PullRequestReference,
    path: string,
    included: boolean,
    head: string,
    base: string,
): Promise<void> {
    return invokeLogged<void>("set_file_included", { reference, path, included, head, base })
}

/// The review skip patterns (`pull-request-viewer`: *Review Skip Patterns*):
/// the stored list, empty until one is stored, and each pattern a hand-edited
/// settings file holds that a list would not be accepted with.
export async function getReviewSkipPatterns(): Promise<ReviewSkipPatterns> {
    return invokeLogged<ReviewSkipPatterns>("get_review_skip_patterns")
}

/// Store the review skip patterns as given, the empty list included. Answers
/// the list now stored, or, when the list is not accepted, every refused
/// pattern and why, in which case nothing was stored. The backend emits
/// `review-skip-patterns-changed` for a stored list, so every window of this
/// transport adopts it without being told.
export async function setReviewSkipPatterns(patterns: string[]): Promise<SkipPatternsOutcome> {
    return invokeLogged<SkipPatternsOutcome>("set_review_skip_patterns", { patterns })
}

/// Open a pull request's web page in the system browser: the view header's
/// "Open on GitHub"/"Open on BitBucket" for a listed pull request. Desktop-only:
/// the web transport has no such command (a browser-skin row links to its
/// `/pr/...` address, and the header's provider control is an opener-isolated
/// link), so never call this under `isWeb()`. Rejects when the URL is not a row
/// of the current BitBucket or GitHub snapshot.
export async function openPullRequest(url: string): Promise<void> {
    return invokeLogged<void>("open_pull_request", { url })
}

/// Open a link from a pull request in the system browser: a link in its
/// description, conversation or review threads, a check's link, and its own
/// web page once it has left its provider's list (`pull-request-viewer`:
/// *Desktop Link Opener*). The only way out of pull-request content, never
/// `openArtifactLink`. The service hands the platform opener only an absolute
/// `http` or `https` URL with a host, only while `reference` has a cached
/// detail, and never fetches it; it rejects anything else.
///
/// Desktop-only: the web transport has no such command, and the browser skin
/// opens these links as opener-isolated tabs instead. Called there, this sends
/// nothing and rejects, since nothing was opened.
export async function openPullRequestLink(
    reference: PullRequestReference,
    href: string,
): Promise<void> {
    if (isWeb()) throw new Error("open_pull_request_link is desktop-only")
    return invokeLogged<void>("open_pull_request_link", { reference, href })
}

export async function getNotificationsEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_notifications_enabled")
}

export async function setNotificationsEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_notifications_enabled", { enabled })
}

/// The reading width every markdown surface renders at. Authoritative — the
/// `localStorage` mirror `docWidth.ts` maintains is only a first-paint hint,
/// and is reconciled against this.
export async function getDocumentWidth(): Promise<DocumentWidth> {
    return invokeLogged<DocumentWidth>("get_document_width")
}

/// Persist the reading width. The backend emits `document-width-changed` so
/// windows already open adopt it too — the caller does not have to tell them.
export async function setDocumentWidth(width: DocumentWidth): Promise<void> {
    return invokeLogged<void>("set_document_width", { width })
}

/// Whether the main window renders the commit graph (`commit-graph`: *Commit
/// History Can Be Turned Off*). Authoritative — the `localStorage` mirror
/// `commitHistory.ts` maintains is only a first-paint hint, reconciled
/// against this.
export async function getCommitHistoryEnabled(): Promise<boolean> {
    return invokeLogged<boolean>("get_commit_history_enabled")
}

/// Persist the Commit history switch. The backend emits
/// `commit-history-enabled-changed` so windows already open — and connected
/// browser skins — adopt it too.
export async function setCommitHistoryEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_commit_history_enabled", { enabled })
}

/// The WSL polling-watcher interval in seconds, or `null` on platforms where
/// WSL workspaces can't occur (macOS, Linux). `null` means "hide the control".
export async function getWslPollIntervalSecs(): Promise<number | null> {
    return invokeLogged<number | null>("get_wsl_poll_interval_secs")
}

export async function setWslPollIntervalSecs(secs: number): Promise<void> {
    return invokeLogged<void>("set_wsl_poll_interval_secs", { secs })
}

// The tree's collapse/expand override sets are deliberately UNREACHABLE from
// here. Top-level disclosure is session state now (design D6 — `spec-browser`:
// *Workspace Tree Hierarchy*), so there is no wrapper for
// `get`/`set_collapsed_tree_node_ids` or their `expanded` twins: a reveal has
// no API left to call, which is what makes *Navigation Reveal Is Transient*'s
// "performs no settings write" true by construction rather than by review.
// Their Rust handlers, dispatch arms and settings fields stay in place so an
// existing settings file still parses; nothing reads them.

export async function getFavoriteChangeIds(): Promise<string[]> {
    return invokeLogged<string[]>("get_favorite_change_ids")
}

/// Apply a favorites delta (ids to star / unstar) and get back the merged
/// list. A delta, not a whole-list write, so one client's toggle can never
/// erase favorites another client persisted since this one hydrated.
export async function updateFavoriteChangeIds(
    add: string[],
    remove: string[],
): Promise<string[]> {
    return invokeLogged<string[]>("update_favorite_change_ids", {
        add,
        remove,
    })
}

/// Best-effort favorites flush for page dismissal. Over the web transport a
/// plain fetch can be killed with the page, so use sendBeacon (built for
/// exactly this); in the native shell fall back to a fire-and-forget invoke.
export function updateFavoriteChangeIdsOnPageHide(
    add: string[],
    remove: string[],
): void {
    if (isWeb() && typeof navigator.sendBeacon === "function") {
        navigator.sendBeacon(
            "/api/invoke",
            new Blob(
                [
                    JSON.stringify({
                        command: "update_favorite_change_ids",
                        args: { add, remove },
                    }),
                ],
                { type: "application/json" },
            ),
        )
        return
    }
    void updateFavoriteChangeIds(add, remove)
}

/// Persists the display-name and tint-colour overrides for a top-level row.
/// Pass `repoId` to address a repository group's shared presentation key, or
/// leave it `null` to address a flat workspace's own key.
export async function setWorkspacePresentation(
    uri: string,
    repoId: string | null,
    displayName: string | null,
    color: PaletteColor | null,
): Promise<void> {
    return invokeLogged<void>("set_workspace_presentation", {
        uri,
        repoId,
        displayName,
        color,
    })
}

/// Parks or un-parks a top-level row. Keyed exactly like
/// `setWorkspacePresentation` — pass `repoId` for a repository group, or `null`
/// for a flat workspace — but a separate command so toggling this cannot clobber
/// the row's display name or tint. A parked row leaves the tree pane, the tray
/// badge, and desktop notifications; it stays in this Settings listing and in
/// every Dashboard figure.
export async function setWorkspaceDisabled(
    uri: string,
    repoId: string | null,
    disabled: boolean,
): Promise<void> {
    return invokeLogged<void>("set_workspace_disabled", {
        uri,
        repoId,
        disabled,
    })
}

/// The embedded web-UI configuration, for the desktop-only "Web UI" settings
/// section. Not available in the web frontend (the section is hidden there).
export async function getWebConfig(): Promise<WebServerConfig> {
    return invokeLogged<WebServerConfig>("get_web_config")
}

/// Enable/disable the embedded web server. Persisted; applied on next launch.
export async function setWebEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_web_enabled", { enabled })
}

/// Set the embedded web server's loopback port. Persisted; applied on next launch.
export async function setWebPort(port: number): Promise<void> {
    return invokeLogged<void>("set_web_port", { port })
}

/// Enable/disable Tailscale Serve access (trusting the host's tailnet name in
/// the web guard). Persisted; applied when the server next builds its router.
export async function setWebTailscaleEnabled(enabled: boolean): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_enabled", { enabled })
}

/// Set the manual Tailscale MagicDNS-name override (null/empty restores
/// auto-discovery).
export async function setWebTailscaleName(name: string | null): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_name", { name })
}

/// Replace the Tailscale per-user login allow-list (empty = trust the whole
/// tailnet).
export async function setWebTailscaleAllowedLogins(
    logins: string[],
): Promise<void> {
    return invokeLogged<void>("set_web_tailscale_allowed_logins", { logins })
}

/// The tailnet name the web server would currently trust (manual override, else
/// discovered, else null) — shown read-only so a stale/missing name is visible.
export async function resolveTailscaleName(): Promise<string | null> {
    return invokeLogged<string | null>("resolve_tailscale_name")
}

// -------------------------------------------------------------------------
// Document watches, reader windows and pull-request windows
// -------------------------------------------------------------------------

/// Who owns a document registration, as the two hosts each name it.
///
/// The desktop shell needs no identifier at all: the command runs in a window,
/// and `window.label()` on the Rust side is both the owner and the thing that
/// can go away. The browser has no such handle, so the page names itself.
function documentWatchArgs(root: string, relPath: string): Record<string, unknown> {
    return isTauri() ? { root, relPath } : { clientId: CLIENT_ID, root, relPath }
}

/// Register interest in one markdown document, so this surface is notified
/// when the file changes on disk. Reference-counted in the shared layer:
/// several surfaces may hold the same document, and each must release its own.
export async function watchDocument(root: string, relPath: string): Promise<void> {
    return invokeLogged<void>("watch_document", documentWatchArgs(root, relPath))
}

/// Release one registration taken by `watchDocument`. Safe to call for a
/// document that is not registered.
export async function unwatchDocument(root: string, relPath: string): Promise<void> {
    return invokeLogged<void>("unwatch_document", documentWatchArgs(root, relPath))
}

/// Open — or focus — a reader window for `addressPath` (an `encodeAddress`
/// result). `title` names the document in the window's titlebar.
///
/// Must be called **synchronously inside the click handler** in the browser
/// host: `window.open` is discarded by popup blockers when it is reached from
/// a promise continuation rather than from the user gesture. That is why this
/// is not `async` even though the desktop path invokes a command — the invoke
/// is fired and not awaited, so both hosts open from the gesture itself.
export function openReaderWindow(addressPath: string, title: string): void {
    if (isTauri()) {
        void invokeLogged<void>("open_reader_window", { addressPath, title }).catch((err) => {
            console.warn("failed to open reader window:", err)
        })
        return
    }
    // The window NAME is the deduplication: opening the same document again
    // targets the window that already shows it instead of making a second.
    const opened = window.open(
        `${addressPath}?reader=1`,
        `specforge-reader:${shortHash(addressPath)}`,
    )
    // A reused window is not raised by `open` alone; the page it already holds
    // has to ask. Blocked or cross-origin access simply leaves it where it is.
    try {
        opened?.focus()
    } catch {
        /* a blocked popup returns null, and a focus refusal is not an error */
    }
}

/// Persist the size a reader window was resized to, so the next one adopts it.
/// Desktop-only: a browser window's size is the browser's business.
export async function setReaderWindowSize(width: number, height: number): Promise<void> {
    if (!isTauri()) return
    return invokeLogged<void>("set_reader_window_size", { width, height })
}

/// Open — or focus — the pull-request window for `addressPath` (an
/// `encodeAddress` result for a pull-request address). `title` is its
/// `pullRequestTitle`, which the desktop window carries from the moment it is
/// built (`pull-request-viewer`: *Pull-Request Window*).
///
/// Synchronous for the reason `openReaderWindow` is: the browser skin's
/// `window.open` must run inside the click, so the desktop's invoke is fired
/// and not awaited. There the window is a tab at the address's path with the
/// `pullRequest` flag beside it, and its name, from the path's hash, makes a
/// second gesture on the same pull request reuse and focus that tab.
export function openPullRequestWindow(addressPath: string, title: string): void {
    if (isTauri()) {
        void invokeLogged<void>("open_pull_request_window", { addressPath, title }).catch(
            (err) => {
                console.warn("failed to open pull-request window:", err)
            },
        )
        return
    }
    const opened = window.open(
        pullRequestWindowPath(addressPath),
        pullRequestWindowName(addressPath),
    )
    // A reused tab is not raised by `open` alone, as a reader's is not.
    try {
        opened?.focus()
    } catch {
        /* a blocked popup returns null, and a focus refusal is not an error */
    }
}

/// Open — or focus — the zoom window of an image file (`diff-view`: *Image
/// Comparison*, Zooming). `address` is an `imageWindowAddress` result, and
/// `title` its `imageWindowTitle`.
///
/// Synchronous for the reason `openPullRequestWindow` is: the browser skin's
/// `window.open` must run inside the click. There the window is a tab of the
/// app's own document with the `imageWindow` flag and the address beside it,
/// named from the address's hash so a second Zoom on one file reuses it.
export function openImageWindow(address: string, title: string): void {
    if (isTauri()) {
        void invokeLogged<void>("open_image_window", { addressPath: address, title }).catch(
            (err) => {
                console.warn("failed to open the zoom window:", err)
            },
        )
        return
    }
    const opened = window.open(imageWindowPath(address), imageWindowName(address))
    try {
        opened?.focus()
    } catch {
        /* a blocked popup returns null, and a focus refusal is not an error */
    }
}

/// Persist the size a pull-request window was resized to, so the next one
/// adopts it. Pull-request windows share one size of their own, apart from the
/// readers' (`pull-request-viewer`: *Pull-Request Window Geometry*).
/// Desktop-only, like `setReaderWindowSize`: the web transport has no such
/// command, and a browser tab's size is the browser's business.
export async function setPullRequestWindowSize(width: number, height: number): Promise<void> {
    if (!isTauri()) return
    return invokeLogged<void>("set_pull_request_window_size", { width, height })
}

// -------------------------------------------------------------------------
// Events
// -------------------------------------------------------------------------

/// Fires when a document some surface registered changes on disk. Carries
/// identifiers only — the receiver re-reads through the guarded read.
export function onDocumentChanged(
    handler: (payload: DocumentChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<DocumentChangedPayload>(EVENT_DOCUMENT_CHANGED, handler)
}

/// Fires when the reading width changes anywhere — including in another window
/// of this application. Carries the new rung, so a listener re-stamps directly
/// instead of making a round trip to read back what it was just told.
export function onDocumentWidthChanged(
    handler: (width: DocumentWidth) => void,
): Promise<UnlistenFn> {
    return listenLogged<DocumentWidth>(EVENT_DOCUMENT_WIDTH_CHANGED, handler)
}

/// Fires when the Commit history switch changes anywhere — another window of
/// this application, or a browser skin against the same service. Carries the
/// new value, so a listener applies it without a round trip.
export function onCommitHistoryEnabledChanged(
    handler: (enabled: boolean) => void,
): Promise<UnlistenFn> {
    return listenLogged<boolean>(EVENT_COMMIT_HISTORY_ENABLED_CHANGED, handler)
}

export function onCacheUpdated(
    handler: (payload: CacheUpdatedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<CacheUpdatedPayload>(EVENT_CACHE_UPDATED, handler)
}

export function onChangeAdded(
    handler: (payload: ChangeAddedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<ChangeAddedPayload>(EVENT_CHANGE_ADDED, handler)
}

export function onChangeArchived(
    handler: (payload: ChangeArchivedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<ChangeArchivedPayload>(EVENT_CHANGE_ARCHIVED, handler)
}

export function onWorkspaceRemoved(
    handler: (payload: WorkspaceRemovedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<WorkspaceRemovedPayload>(EVENT_WORKSPACE_REMOVED, handler)
}

export function onLogicalChangeAdded(
    handler: (payload: LogicalChangePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<LogicalChangePayload>(EVENT_LOGICAL_CHANGE_ADDED, handler)
}

export function onLogicalChangeArchived(
    handler: (payload: LogicalChangePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<LogicalChangePayload>(EVENT_LOGICAL_CHANGE_ARCHIVED, handler)
}

export function onInstanceAdded(
    handler: (payload: InstancePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<InstancePayload>(EVENT_INSTANCE_ADDED, handler)
}

export function onInstanceRemoved(
    handler: (payload: InstancePayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<InstancePayload>(EVENT_INSTANCE_REMOVED, handler)
}

export function onWorkspacePresentationUpdated(
    handler: () => void,
): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_WORKSPACE_PRESENTATION_UPDATED, () => handler())
}

export function onGraphChanged(
    handler: (payload: GraphChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<GraphChangedPayload>(EVENT_GRAPH_CHANGED, handler)
}

/// The quota snapshot was refreshed; the payload is empty, so callers re-read
/// via `getClaudeQuota`.
export function onQuotaUpdated(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_QUOTA_UPDATED, () => handler())
}

/// The BitBucket pull-request snapshot changed; the payload is empty, so
/// callers re-read via `getBitbucketPullRequests`.
export function onBitbucketPullRequestsUpdated(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_BITBUCKET_PULL_REQUESTS_UPDATED, () => handler())
}

/// The GitHub pull-request snapshot changed; the payload is empty, so callers
/// re-read via `getGithubPullRequests`.
export function onGithubPullRequestsUpdated(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_GITHUB_PULL_REQUESTS_UPDATED, () => handler())
}

/// A pull-request panel's position changed anywhere — including in another
/// window or a connected browser skin. Carries which panel and its new slot,
/// so a listener re-seats that panel without a round trip.
export function onPullRequestPanelMoved(
    handler: (payload: PanelMovedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<PanelMovedPayload>(EVENT_PULL_REQUEST_PANEL_MOVED, handler)
}

/// A provider's enabled flag was set anywhere: this window's Settings, another
/// window, or a browser tab the same service serves. Carries the provider and
/// its new flag, so a root resolving pull-request addresses re-resolves them
/// without a round trip (`pull-request-viewer`: *Provider Enabled Flags Stay
/// Current*). A service notice, so it reaches every window and tab of the
/// service; an unparseable SSE frame arrives as `undefined`.
export function onPullRequestProviderChanged(
    handler: (payload: PullRequestProviderChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<PullRequestProviderChangedPayload>(
        EVENT_PULL_REQUEST_PROVIDER_CHANGED,
        handler,
    )
}

/// A pull request's review progress changed: a file was marked or unmarked as
/// viewed in any window or tab of the service. Carries the pull request's
/// reference only, so a view showing it, compared ignoring ASCII case, re-reads
/// its progress, and every other view ignores it (`pull-request-viewer`:
/// *Review Progress*). A service notice, like `pull-request-provider-changed`.
export function onReviewProgressChanged(
    handler: (reference: PullRequestReference) => void,
): Promise<UnlistenFn> {
    return listenLogged<PullRequestReference>(EVENT_REVIEW_PROGRESS_CHANGED, handler)
}

/// The review skip patterns were stored, the empty list included, in any
/// window of this transport (`pull-request-viewer`: *Review Skip Patterns*).
/// Carries the list now stored, so Settings shows it without a round trip,
/// while a pull-request view reads its review progress again. A direct emit,
/// like `commit-history-enabled-changed`; an unparseable SSE frame arrives as
/// `undefined`.
export function onReviewSkipPatternsChanged(
    handler: (payload: ReviewSkipPatternsChangedPayload) => void,
): Promise<UnlistenFn> {
    return listenLogged<ReviewSkipPatternsChangedPayload>(
        EVENT_REVIEW_SKIP_PATTERNS_CHANGED,
        handler,
    )
}

/// The macOS View menu asked to toggle the sidebar. Desktop-only: only the
/// Tauri shell emits it (the web UI covers the same gesture with its own
/// keyboard binding), so subscribe only under `isTauri()`.
export function onToggleSidebar(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_TOGGLE_SIDEBAR, () => handler())
}

/// The macOS View menu asked to toggle the commit rail — see `onToggleSidebar`.
export function onToggleCommitRail(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_TOGGLE_COMMIT_RAIL, () => handler())
}

/// The macOS application menu's Settings… item (Cmd+,) asked the main window
/// to show Settings (`application-menu`: *Settings Menu Item*). Desktop-only,
/// like the pane toggles: only the Tauri shell emits it, and in a browser
/// Cmd+, belongs to the browser's own preferences — so subscribe only under
/// `isTauri()`, and never add a keydown handler for the same combination.
export function onOpenSettings(handler: () => void): Promise<UnlistenFn> {
    return listenLogged<unknown>(EVENT_OPEN_SETTINGS, () => handler())
}
