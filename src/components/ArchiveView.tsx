import { useEffect, useMemo, useState } from "react"
import {
    archivedArtifactStatus,
    listArchivedRows,
    onChangeArchived,
    onLogicalChangeArchived,
} from "../api"
import type {
    ArchiveScope,
    ArchivedChangeCopy,
    ArchivedChangeRow,
    ArtifactReadKind,
    ArtifactRenderTarget,
    ArtifactStatus,
    RegisteredWorkspace,
    WorkspaceView,
} from "../types"
import { DetailPane } from "./DetailPane"
import { EmptyState } from "./EmptyState"
import { ChevronRight } from "./icons"

interface ArchiveViewProps {
    /// The top-level rows — repository groups and flat workspaces — that the
    /// scope selector offers. The archive is browsed one row at a time, pooled
    /// across that row's tracked worktrees.
    views: WorkspaceView[]
    /// The registered listing, consulted only to label a copy's workspace.
    /// Discovered worktrees are absent from it by design; they fall back to
    /// their folder basename.
    workspaces: RegisteredWorkspace[]
    /// Optional deep-link from the dashboard's today's-ships feed: on mount,
    /// scope to the row holding this worktree and open this archive directory,
    /// preferring that worktree's copy of it.
    initialSelection?: { workspaceUri: string; archiveDir: string } | null
}

/// One selectable listing scope: a repository group or a flat workspace.
interface ArchiveScopeRow {
    key: string
    label: string
    scope: ArchiveScope
    /// Every worktree the scope pools over — used to map a deep-linked
    /// worktree back to the row that contains it.
    worktrees: string[]
}

/// Mirrors `rowKey` in `workspaceRows.ts`: the `repo:`/`flat:` prefix keeps the
/// key total, so a flat workspace registered at a path equal to another row's
/// `repoId` can never collide with it.
///
/// Built from `views` PLUS the parked rows only `workspaces` still carries.
/// `get_workspace_views` strips disabled rows before any frontend sees them, so
/// scoping the selector to `views` alone would make a parked row's archive
/// unbrowsable — a regression, since the archive readers authorize on registry
/// membership and know nothing about the presentation store, so a parked row's
/// archive was always readable. Parking is a tree-pane decision, not a
/// retraction of the archive.
function scopeRowsFor(
    views: WorkspaceView[],
    workspaces: RegisteredWorkspace[],
): ArchiveScopeRow[] {
    const rows: ArchiveScopeRow[] = views.map((v) =>
        v.kind === "repo"
            ? {
                  key: `repo:${v.repoId}`,
                  label: v.displayName ?? v.name,
                  scope: { kind: "repo", repoId: v.repoId },
                  // `worktrees` is registry-derived while `mainWorktree` comes
                  // from `git worktree list`; they disagree when the main
                  // checkout has no `openspec/` dir, or when the two spellings
                  // differ. Both are matchable so a deep link resolved to
                  // `mainWorktree` still finds this row instead of silently
                  // listing an unrelated repository.
                  worktrees: [...new Set([...v.worktrees, v.mainWorktree])],
              }
            : {
                  key: `flat:${v.workspace.uri}`,
                  label: v.displayName ?? v.workspace.name,
                  scope: { kind: "flat", workspace: v.workspace.uri },
                  worktrees: [v.workspace.uri],
              },
    )

    // Append the parked rows, keyed exactly as an enabled row would be so a
    // row that is later un-parked keeps its identity. One entry per row, not
    // per registered folder: two registered worktrees of one repository share
    // a single `repo:` key.
    const seen = new Set(rows.map((r) => r.key))
    for (const ws of workspaces) {
        if (!ws.disabled) continue
        const key = ws.repoId !== null ? `repo:${ws.repoId}` : `flat:${ws.uri}`
        if (seen.has(key)) continue
        seen.add(key)
        rows.push({
            key,
            label: `${ws.displayName ?? ws.name} (parked)`,
            scope:
                ws.repoId !== null
                    ? { kind: "repo", repoId: ws.repoId }
                    : { kind: "flat", workspace: ws.uri },
            // Only the registered folders are known for a parked row — it has
            // no view to enumerate worktrees from. The backend still pools
            // every tracked worktree of the repository, so this narrows only
            // deep-link matching, never the listing itself.
            worktrees: workspaces
                .filter((w) =>
                    ws.repoId !== null
                        ? w.repoId === ws.repoId
                        : w.uri === ws.uri,
                )
                .map((w) => w.uri),
        })
    }
    return rows
}

function basename(path: string): string {
    const parts = path.split(/[\\/]/).filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : path
}

/// Identity of one copy within its row. The worktree path alone is not enough:
/// a single worktree can hold a dated archive directory and its legacy un-dated
/// twin, which are two copies of one logical change.
function copyKey(copy: ArchivedChangeCopy): string {
    return `${copy.worktreePath}\u0000${copy.archiveDir}`
}

/// A copy's base label: the worktree's own display name when the override
/// names that folder alone, else the worktree folder's basename. **Never the
/// branch** (`archive-browser`: *Copies are named by workspace, never by
/// branch*) — the worktree an archived change is read from routinely hosts
/// other, active changes whose branch was never this change's.
///
/// A repository group's display-name override is stored per repository, so
/// every worktree of it shares one. Labelling copies with that would print the
/// same name against each and name nothing, so it is used only for a flat
/// workspace, whose presentation key is its own path.
function baseCopyLabel(
    copy: ArchivedChangeCopy,
    workspaces: RegisteredWorkspace[],
): string {
    const ws = workspaces.find((w) => w.uri === copy.worktreePath)
    if (ws && ws.repoId === null && ws.displayName) return ws.displayName
    return basename(copy.worktreePath)
}

/// Labels for a row's copies, one per copy in `copies` order.
///
/// Copies are never presented as interchangeable: archived content is read from
/// the working tree rather than from git, so two copies can genuinely differ.
/// Whenever their on-disk directories differ — different archive dates, or a
/// legacy un-dated twin — each label carries its own directory name, which is
/// also what tells apart two copies that would otherwise read identically.
function copyLabels(
    copies: ArchivedChangeCopy[],
    workspaces: RegisteredWorkspace[],
): string[] {
    const bases = copies.map((c) => baseCopyLabel(c, workspaces))
    const dirsDiffer = new Set(copies.map((c) => c.archiveDir)).size > 1
    if (dirsDiffer) {
        return bases.map((b, i) => `${b} · ${copies[i]!.archiveDir}`)
    }
    // Two copies can share a base label without differing directories — two
    // worktrees whose folders have the same basename, in different parents.
    // Appending the directory there appends the SAME string to both and
    // disambiguates nothing, so fall back to the one field that is unique by
    // construction: the worktree path.
    const collides = bases.some((b, i) => bases.indexOf(b) !== i)
    if (!collides) return bases
    return bases.map((b, i) =>
        bases.indexOf(b) === bases.lastIndexOf(b)
            ? b
            : `${b} · ${copies[i]!.worktreePath}`,
    )
}

/// The first artifact a copy actually has, in the order the tab strip renders
/// them, or `null` for a copy with nothing on disk. Mirrors `App.tsx`'s
/// `firstPresentArtifact` for the active-change reader.
function firstPresentArtifact(
    status: ArtifactStatus,
): { kind: ArtifactReadKind; capability?: string } | null {
    if (status.proposal) return { kind: "proposal" }
    if (status.design) return { kind: "design" }
    if (status.tasks) return { kind: "tasks" }
    const cap = status.specs[0]
    return cap ? { kind: "spec", capability: cap } : null
}

/// Whether the artifact `kind` (with `capability`, for a spec) is present in
/// `status`.
function artifactPresent(
    status: ArtifactStatus,
    kind: ArtifactReadKind,
    capability?: string,
): boolean {
    switch (kind) {
        case "proposal":
            return status.proposal
        case "design":
            return status.design
        case "tasks":
            return status.tasks
        case "spec":
            return capability !== undefined && status.specs.includes(capability)
    }
}

/// The Archive view: a global, footer-reached surface for browsing one
/// top-level row's archived changes, pooled across every tracked worktree of it
/// and de-duplicated on the bare logical change id. Loads on mount and on scope
/// change, so the archive never touches the tree render or the watcher hot path.
export function ArchiveView({
    views,
    workspaces,
    initialSelection,
}: ArchiveViewProps) {
    const scopes = useMemo(
        () => scopeRowsFor(views, workspaces),
        [views, workspaces],
    )

    // Which top-level row the LISTING is scoped to. Deliberately separate from
    // `activeCopy` below (design D6): this one's `onChange` closes the open
    // change and refetches, which is exactly what a copy switch must not do.
    const [scopeKey, setScopeKey] = useState<string | null>(
        () =>
            scopes.find((s) =>
                s.worktrees.includes(initialSelection?.workspaceUri ?? ""),
            )?.key ?? null,
    )
    // Resolved by lookup rather than by an effect that rewrites `scopeKey`: an
    // effect that snapped an unrecognised value to the first row is what used
    // to discard a perfectly valid deep link into a discovered worktree.
    const scope = scopes.find((s) => s.key === scopeKey) ?? scopes[0] ?? null

    const [rows, setRows] = useState<ArchivedChangeRow[]>([])
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const [filter, setFilter] = useState("")
    // The open change is held by its logical id, not as a snapshot, so a live
    // refresh of the listing updates its copy set in place instead of closing it.
    const [openId, setOpenId] = useState<string | null>(null)
    // Which copy the READER renders. Per-open state, not part of the listing
    // scope: switching it re-points only this reader (design D6).
    const [activeCopy, setActiveCopy] = useState<string | null>(null)
    // Bumped by archive events to force a re-fetch of the open listing.
    const [reload, setReload] = useState(0)
    // Which artifacts the open copy has on disk, and which one is shown.
    const [artifactStatus, setArtifactStatus] = useState<ArtifactStatus | null>(
        null,
    )
    const [activeArtifact, setActiveArtifact] = useState<{
        kind: ArtifactReadKind
        capability?: string
    }>({ kind: "proposal" })
    // A deep-linked archive directory (from the dashboard) to open once the
    // scope's listing has loaded. Consumed and cleared on first match.
    const [pendingOpen, setPendingOpen] = useState<{
        archiveDir: string
        worktreeUri: string
    } | null>(
        initialSelection
            ? {
                  archiveDir: initialSelection.archiveDir,
                  worktreeUri: initialSelection.workspaceUri,
              }
            : null,
    )

    const openChange = rows.find((r) => r.id === openId) ?? null
    const copies = openChange?.copies ?? []
    const activeCopyEntry =
        copies.find((c) => copyKey(c) === activeCopy) ?? copies[0] ?? null

    // Auto-open a deep-linked change once the scope's listing has loaded. The
    // hint chooses which COPY opens first; the change itself is in the union
    // whichever worktree holds it, so an unmatched hint costs nothing.
    useEffect(() => {
        if (!pendingOpen) return
        const match = rows.find((r) =>
            r.copies.some((c) => c.archiveDir === pendingOpen.archiveDir),
        )
        if (!match) return
        const preferred =
            match.copies.find(
                (c) =>
                    c.worktreePath === pendingOpen.worktreeUri &&
                    c.archiveDir === pendingOpen.archiveDir,
            ) ??
            match.copies.find(
                (c) => c.worktreePath === pendingOpen.worktreeUri,
            ) ??
            match.copies.find((c) => c.archiveDir === pendingOpen.archiveDir)
        setOpenId(match.id)
        setActiveCopy(preferred ? copyKey(preferred) : null)
        setActiveArtifact({ kind: "proposal" })
        setPendingOpen(null)
    }, [rows, pendingOpen])

    // Load the selected scope's union listing on demand.
    useEffect(() => {
        if (!scope) {
            setRows([])
            return
        }
        let cancelled = false
        setLoading(true)
        setError(null)
        listArchivedRows(scope.scope)
            .then((next) => {
                if (cancelled) return
                setRows(next)
                setLoading(false)
                // A deep link that this listing does not contain is spent, not
                // pending: keeping it armed lets a later refresh fire it and
                // pull the reader off whatever the user is reading. One
                // completed load for the scope is the whole window in which it
                // could legitimately match.
                setPendingOpen((p) =>
                    p &&
                    next.some((r) =>
                        r.copies.some((c) => c.archiveDir === p.archiveDir),
                    )
                        ? p
                        : null,
                )
            })
            .catch((e) => {
                if (cancelled) return
                setError(String(e))
                setLoading(false)
            })
        return () => {
            cancelled = true
        }
        // `scope.scope` is rebuilt each render; the key is its stable identity.
        // The worktree set is a dependency too: a worktree added to or removed
        // from this repository changes what the union pools, and archive
        // events alone would not report it.
    }, [scope?.key, scope?.worktrees.join(" "), reload])

    // Live refresh while open: re-fetch the WHOLE scope when a change is
    // archived, so an archival in any tracked worktree of it lands — including
    // one in a worktree other than the copy currently being read. Only `rows`
    // is replaced, so the open change and its selected copy survive. Closing
    // the view unmounts this effect, so no archive work happens while it's
    // closed.
    // One archival emits BOTH `change-archived` and `logical-change-archived`
    // in a git repository, so bumping on each would refetch the union twice —
    // an N-worktree fan-out, run twice, for one event. Coalesced to one bump
    // per microtask batch.
    useEffect(() => {
        let queued = false
        const bump = () => {
            if (queued) return
            queued = true
            queueMicrotask(() => {
                queued = false
                setReload((n) => n + 1)
            })
        }
        const unsubs = [onChangeArchived(bump), onLogicalChangeArchived(bump)]
        return () => {
            for (const u of unsubs) void u.then((f) => f())
        }
    }, [])

    // Which artifacts the SELECTED COPY has on disk. Re-run when the copy
    // changes, since two copies of one archived change need not contain the
    // same artifacts. On-demand and per-copy — never on the aggregation path.
    const readWorktree = activeCopyEntry?.worktreePath ?? null
    const readDir = activeCopyEntry?.archiveDir ?? null
    useEffect(() => {
        if (!readWorktree || !readDir) {
            setArtifactStatus(null)
            return
        }
        setArtifactStatus(null)
        let cancelled = false
        archivedArtifactStatus(readWorktree, readDir)
            .then((s) => {
                if (!cancelled) setArtifactStatus(s)
            })
            .catch(() => {
                if (!cancelled) setArtifactStatus(null)
            })
        return () => {
            cancelled = true
        }
    }, [readWorktree, readDir])

    // Keep the shown artifact reachable: a copy that lacks the one currently
    // displayed falls back to an artifact it actually HAS, rather than
    // rendering a read error.
    //
    // Not an unconditional fall back to the proposal: a copy need not have one
    // (a change archived mid-write, or one that only ever had `tasks.md`), and
    // the tab strip renders the Proposal tab only when `status.proposal` is not
    // false. Falling back to it there would leave no tab highlighted, no tab to
    // click, and a read error in the pane — exactly the outcome this effect
    // exists to prevent. Terminates: the chosen artifact is present by
    // construction, so the guard below is false on the next run.
    useEffect(() => {
        if (!artifactStatus) return
        if (
            artifactPresent(
                artifactStatus,
                activeArtifact.kind,
                activeArtifact.capability,
            )
        ) {
            return
        }
        const fallback = firstPresentArtifact(artifactStatus)
        if (fallback) setActiveArtifact(fallback)
    }, [artifactStatus, activeArtifact])

    // Pure client-side narrowing of the already-loaded rows — no further read.
    const filtered = useMemo(() => {
        const q = filter.trim().toLowerCase()
        if (!q) return rows
        return rows.filter(
            (r) =>
                r.id.toLowerCase().includes(q) ||
                (r.title?.toLowerCase().includes(q) ?? false),
        )
    }, [rows, filter])

    // Parked rows are listed above, so reaching here really does mean nothing
    // is registered — the message is not covering for a row that exists but is
    // merely hidden.
    if (scopes.length === 0) {
        return (
            <EmptyState
                title="No workspaces registered"
                body="Add a workspace from Settings to browse its archive."
            />
        )
    }

    // Reading one archived change: reuse the artifact renderer with a change_id
    // that points into the archive subtree (read_artifact permits it), bound to
    // the SELECTED copy's worktree and directory name.
    if (openChange && activeCopyEntry) {
        const target: ArtifactRenderTarget = {
            kind: "artifact",
            workspace: activeCopyEntry.worktreePath,
            changeId: `archive/${activeCopyEntry.archiveDir}`,
            artifactKind: activeArtifact.kind,
            capability: activeArtifact.capability,
        }
        const isActive = (kind: ArtifactReadKind, capability?: string) =>
            activeArtifact.kind === kind &&
            activeArtifact.capability === capability
        const labels = copyLabels(copies, workspaces)
        return (
            <div className="archive-view archive-view--reading">
                <div className="archive-header">
                    <button
                        className="archive-back"
                        onClick={() => {
                            setOpenId(null)
                            setActiveCopy(null)
                        }}
                    >
                        ← Archive
                    </button>
                    {/* Dated from the COPY being read, not from the row. A
                        row's date is the newest across its copies and its
                        title is the first copy that has one, so after a copy
                        switch a row-derived header would name a different
                        copy than the document below it. */}
                    <span className="archive-reading-title">
                        {activeCopyEntry.date ? `${activeCopyEntry.date} · ` : ""}
                        {openChange.title ?? openChange.id}
                    </span>
                </div>
                <div className="archive-copy-row">
                    <span className="archive-copy-label">Worktree</span>
                    {copies.length > 1 ? (
                        <select
                            className="archive-copy-select"
                            value={copyKey(activeCopyEntry)}
                            onChange={(e) => setActiveCopy(e.target.value)}
                            aria-label="Worktree copy"
                        >
                            {copies.map((c, i) => (
                                <option key={copyKey(c)} value={copyKey(c)}>
                                    {labels[i]}
                                </option>
                            ))}
                        </select>
                    ) : (
                        <span className="archive-copy-single">{labels[0]}</span>
                    )}
                </div>
                <div className="archive-artifact-tabs">
                    {/* Proposal is the default; show it unless we know it's absent. */}
                    {artifactStatus?.proposal !== false && (
                        <button
                            className={`archive-tab${isActive("proposal") ? " archive-tab--active" : ""}`}
                            onClick={() =>
                                setActiveArtifact({ kind: "proposal" })
                            }
                        >
                            Proposal
                        </button>
                    )}
                    {artifactStatus?.design && (
                        <button
                            className={`archive-tab${isActive("design") ? " archive-tab--active" : ""}`}
                            onClick={() => setActiveArtifact({ kind: "design" })}
                        >
                            Design
                        </button>
                    )}
                    {artifactStatus?.tasks && (
                        <button
                            className={`archive-tab${isActive("tasks") ? " archive-tab--active" : ""}`}
                            onClick={() => setActiveArtifact({ kind: "tasks" })}
                        >
                            Tasks
                        </button>
                    )}
                    {artifactStatus?.specs.map((cap) => (
                        <button
                            key={cap}
                            className={`archive-tab${isActive("spec", cap) ? " archive-tab--active" : ""}`}
                            onClick={() =>
                                setActiveArtifact({
                                    kind: "spec",
                                    capability: cap,
                                })
                            }
                        >
                            {cap}
                        </button>
                    ))}
                </div>
                <DetailPane target={target} scrollAnchor={null} />
            </div>
        )
    }

    return (
        <div className="archive-view">
            <div className="archive-header">
                <h2 className="archive-title">Archive</h2>
                {scopes.length > 1 ? (
                    <select
                        className="archive-workspace-select"
                        value={scope?.key ?? ""}
                        onChange={(e) => {
                            setScopeKey(e.target.value)
                            setOpenId(null)
                            setActiveCopy(null)
                            // Disarm any deep link that never matched. Left
                            // armed it survives into the new scope and fires on
                            // the next archive event, yanking the reader to an
                            // unrelated change minutes after the link was
                            // issued.
                            setPendingOpen(null)
                            setFilter("")
                        }}
                        aria-label="Workspace"
                    >
                        {scopes.map((s) => (
                            <option key={s.key} value={s.key}>
                                {s.label}
                            </option>
                        ))}
                    </select>
                ) : (
                    <span className="archive-workspace-single">
                        {scope?.label}
                    </span>
                )}
            </div>

            <input
                className="archive-search"
                type="text"
                placeholder="Search the archive…"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
            />

            {loading ? (
                <div className="archive-status">Loading…</div>
            ) : error ? (
                <div className="archive-status archive-status--error">
                    {error}
                </div>
            ) : rows.length === 0 ? (
                <EmptyState
                    title="No archived changes"
                    body="None of this workspace's tracked worktrees has anything in openspec/changes/archive/."
                />
            ) : filtered.length === 0 ? (
                <div className="archive-status">
                    No changes match “{filter}”.
                </div>
            ) : (
                <ul className="archive-list">
                    {filtered.map((r) => (
                        // The logical id is unique within a scope by
                        // construction — the raw directory name is not, now
                        // that a row can pool copies from several worktrees.
                        <li key={r.id}>
                            <button
                                className="archive-row"
                                onClick={() => {
                                    setOpenId(r.id)
                                    // PIN the copy at open time. Leaving it
                                    // null makes the reader track whatever
                                    // `copies[0]` currently is, so a live
                                    // refresh that reorders the copy set — a
                                    // sibling worktree archiving the same
                                    // change under a newer date, which sorts
                                    // first — would silently re-point the open
                                    // reader at a different worktree with no
                                    // user gesture.
                                    setActiveCopy(
                                        r.copies[0]
                                            ? copyKey(r.copies[0])
                                            : null,
                                    )
                                    setActiveArtifact({ kind: "proposal" })
                                }}
                            >
                                <span className="archive-date">
                                    {r.date ?? "—"}
                                </span>
                                <span className="archive-name">
                                    {r.title ?? r.id}
                                </span>
                                {r.copies.length > 1 && (
                                    <span className="archive-copy-count">
                                        {r.copies.length} copies
                                    </span>
                                )}
                                <ChevronRight
                                    className="archive-row-chevron"
                                    width={14}
                                    height={14}
                                />
                            </button>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    )
}
