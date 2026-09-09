import { useEffect, useMemo, useState } from "react"
import type { ReactNode } from "react"
import { listWorkspaceFileRows } from "../api"
import {
    copyLabels,
    copyWorktrees,
    divergentPaths,
    resolveCopy,
    rowForPath,
} from "../fileCopies"
import { isNewWindowModifier } from "../platform"
import type {
    FileScope,
    RegisteredWorkspace,
    WorkspaceFileRow,
} from "../types"
import { CopyableIdentity } from "./CopyableIdentity"
import { EmptyState } from "./EmptyState"
import {
    DocumentView,
    IdentityTrailing,
    MissingDocumentLabel,
} from "./DocumentView"
import { ChevronDown, ChevronRight } from "./icons"

interface FileBrowserViewProps {
    /// The browse root: a **repository** (pooled across its tracked worktrees)
    /// or a flat workspace folder. Re-fetches the listing whenever this
    /// changes.
    scope: FileScope
    /// The repository's main worktree, or null for a flat workspace. Decides
    /// which copy of a multi-copy file opens first, and is the root a read
    /// falls back to when the address names a path no tracked worktree holds.
    mainWorktree: string | null
    /// The registered listing, consulted only to label a copy's workspace.
    /// Discovered worktrees are absent from it by design; they fall back to
    /// their folder basename.
    workspaces: RegisteredWorkspace[]
    /// The row's display label, shown in the header.
    label: string
    /// The file the address names, or null for none. The selection is
    /// controlled from the address rather than owned here, so it is linkable,
    /// restorable on load, and part of navigation history
    /// (`workspace-file-browser`: *The Selected File Is Addressable*).
    selectedPath?: string | null
    /// Publish a new selection. The caller navigates, and the new address comes
    /// back in as `selectedPath` — the browser never sets it directly, so there
    /// is one source of truth for what is shown.
    onSelectFile?: (path: string) => void
    /// Open a file in its own reader window, without changing this selection.
    onOpenReader?: (path: string) => void
}

interface FileEntry {
    kind: "file"
    name: string
    /// Root-relative path with forward slashes — the identity `readWorkspaceFile`
    /// and the filter both key on.
    path: string
}

interface FolderEntry {
    kind: "folder"
    name: string
    path: string
    children: TreeEntry[]
}

type TreeEntry = FileEntry | FolderEntry

const collator = (a: string, b: string) =>
    a.localeCompare(b, undefined, { sensitivity: "base" })

/// Derive a folder tree from a flat relative-path list. A folder only ever
/// materialises where a file implies it, so directories with no markdown
/// anywhere beneath them never appear. Folders sort before files; both sort
/// case-insensitively.
function buildTree(paths: string[]): TreeEntry[] {
    interface MutableFolder {
        name: string
        path: string
        folders: Map<string, MutableFolder>
        files: FileEntry[]
    }
    const root: MutableFolder = { name: "", path: "", folders: new Map(), files: [] }

    for (const filePath of paths) {
        const segments = filePath.split("/").filter((s) => s.length > 0)
        if (segments.length === 0) continue
        let cursor = root
        let cursorPath = ""
        for (let i = 0; i < segments.length - 1; i++) {
            const seg = segments[i]
            cursorPath = cursorPath ? `${cursorPath}/${seg}` : seg
            let next = cursor.folders.get(seg)
            if (!next) {
                next = { name: seg, path: cursorPath, folders: new Map(), files: [] }
                cursor.folders.set(seg, next)
            }
            cursor = next
        }
        const fileName = segments[segments.length - 1]
        cursor.files.push({ kind: "file", name: fileName, path: filePath })
    }

    function finalize(folder: MutableFolder): TreeEntry[] {
        const folders: FolderEntry[] = Array.from(folder.folders.values())
            .sort((a, b) => collator(a.name, b.name))
            .map((f) => ({
                kind: "folder" as const,
                name: f.name,
                path: f.path,
                children: finalize(f),
            }))
        const files = [...folder.files].sort((a, b) => collator(a.name, b.name))
        return [...folders, ...files]
    }

    return finalize(root)
}

/// Prune the tree to entries matching `query` (case-insensitive substring on
/// the full relative path). A folder survives only if a descendant matches.
function filterTree(nodes: TreeEntry[], query: string): TreeEntry[] {
    if (!query) return nodes
    const result: TreeEntry[] = []
    for (const node of nodes) {
        if (node.kind === "file") {
            if (node.path.toLowerCase().includes(query)) result.push(node)
        } else {
            const children = filterTree(node.children, query)
            if (children.length > 0) {
                result.push({ ...node, children })
            }
        }
    }
    return result
}

interface RenderProps {
    expanded: Set<string>
    /// While a filter is active every surviving folder renders open — the
    /// spec requires matches to be visible with their ancestors revealed,
    /// without mutating (and later having to restore) the real expansion
    /// state.
    forceOpen: boolean
    selectedPath: string | null
    /// Ancestor folders of the selected file, opened transiently so a file
    /// reached by address is visible in the tree rather than hidden inside
    /// collapsed folders. A TRANSIENT overlay, exactly like `forceOpen` above
    /// and like the workspace tree's own reveal: it never writes the user's
    /// expansion state, so following a link cannot silently rewrite what they
    /// had open (`view-routing`: *Navigation Reveal Is Transient*).
    revealed: Set<string>
    /// Paths whose copies differ between tracked worktrees. Consulted per row
    /// rather than folded into the tree, so the hierarchy is derived from the
    /// path list alone (`workspace-file-browser`: *The tree renders before
    /// divergence is known*).
    divergent: Set<string>
    onToggleFolder: (path: string) => void
    onSelectFile: (path: string) => void
    /// Open the file in its own reader window instead of selecting it here.
    onOpenReader: (path: string) => void
}

function renderRows(
    nodes: TreeEntry[],
    depth: number,
    props: RenderProps,
): ReactNode[] {
    const rows: ReactNode[] = []
    for (const node of nodes) {
        if (node.kind === "folder") {
            const isOpen =
                props.forceOpen || props.expanded.has(node.path) || props.revealed.has(node.path)
            rows.push(
                <button
                    key={`folder:${node.path}`}
                    className="tree-row file-browser-row"
                    style={{ paddingLeft: depth * 12 + 4 }}
                    onClick={() => props.onToggleFolder(node.path)}
                    aria-expanded={isOpen}
                >
                    <span
                        className={`chevron${isOpen ? " open" : ""}`}
                        aria-hidden="true"
                    >
                        {isOpen ? <ChevronDown /> : <ChevronRight />}
                    </span>
                    <span className="row-label">{node.name}</span>
                </button>,
            )
            if (isOpen) {
                rows.push(...renderRows(node.children, depth + 1, props))
            }
        } else {
            const isSelected = props.selectedPath === node.path
            rows.push(
                <button
                    key={`file:${node.path}`}
                    className={`tree-row file-browser-row${isSelected ? " selected" : ""}`}
                    style={{ paddingLeft: depth * 12 + 4 }}
                    // Cmd/Ctrl-click opens the file in its own reader window
                    // and leaves this browser's selection and preview exactly
                    // as they were (`reader-window`: *Launching a Reader
                    // Window*). No other modifier chord is bound on these rows,
                    // so the browser's own "open in a new window" convention is
                    // free to take.
                    onClick={(e) => {
                        if (isNewWindowModifier(e)) {
                            e.preventDefault()
                            props.onOpenReader(node.path)
                            return
                        }
                        props.onSelectFile(node.path)
                    }}
                    aria-current={isSelected}
                >
                    <span className="chevron chevron-spacer" aria-hidden="true" />
                    <span className="row-label">{node.name}</span>
                    {/* States only THAT the copies differ — never which is
                        newer or authoritative. Every tracked worktree is on its
                        own branch, so this is what tells the reader where the
                        branches have actually drifted
                        (`workspace-file-browser`: *A file differing between
                        worktrees is marked*). */}
                    {props.divergent.has(node.path) && (
                        <span
                            className="file-browser-differs"
                            title="Copies of this file differ between worktrees"
                            aria-label="Copies differ between worktrees"
                        >
                            ≠
                        </span>
                    )}
                </button>,
            )
        }
    }
    return rows
}

/// The workspace file browser: a folder tree of the browse root's markdown
/// files on the left, the selected file rendered with `MarkdownView` on the
/// right. For a repository the listing is the union across its tracked
/// worktrees, so a row can have several copies and the preview names — and
/// chooses — which one it renders. Fetches its own listing on mount and
/// whenever the scope changes, mirroring `ArchiveView`'s lifecycle — no
/// watcher, freshness is pulled via the refresh control.
export function FileBrowserView({
    scope,
    mainWorktree,
    workspaces,
    label,
    selectedPath = null,
    onSelectFile,
    onOpenReader,
}: FileBrowserViewProps) {
    const [rows, setRows] = useState<WorkspaceFileRow[] | null>(null)
    const [listLoading, setListLoading] = useState(false)
    const [listError, setListError] = useState<string | null>(null)
    // Bumped by the refresh control to force a re-fetch of the listing. For a
    // repository that re-runs EVERY tracked worktree's enumeration, since the
    // listing is the union of them (`workspace-file-browser`: *Refresh spans
    // every tracked worktree*).
    const [reload, setReload] = useState(0)

    const [filter, setFilter] = useState("")
    const [expanded, setExpanded] = useState<Set<string>>(new Set())
    // Which copy the PREVIEW renders, pinned at open time. Deliberately
    // separate from the listing scope above: switching it re-points only the
    // preview — it never refetches the listing, clears the filter or collapses
    // the tree, which is exactly what a scope change does do.
    //
    // Carries the browse root and file it was chosen FOR, so a new selection is
    // simply not covered by it. Resetting the pin from an effect instead would
    // leave one render in which the previous file's copy is the active one, and
    // the preview would issue a read against it before correcting itself.
    const [pinnedCopy, setPinnedCopy] = useState<{
        scopeKey: string
        path: string
        worktree: string
    } | null>(null)

    // A stable identity for the browse root, so the effects below depend on the
    // scope's value rather than on the object rebuilt every render.
    const scopeKey =
        scope.kind === "repo" ? `repo:${scope.repoId}` : `flat:${scope.workspace}`
    // The root a read falls back to when no copy holds the path — the address
    // resolved to this browse root, so the not-found comes from the read rather
    // than from the browser refusing to try.
    const fallbackRoot = scope.kind === "repo" ? mainWorktree : scope.workspace

    // Reset the root-scoped UI state this component still owns when the browse
    // root changes — otherwise a stale filter/expansion from the previous
    // workspace would linger until the user interacts again. The selection is
    // no longer among them: it belongs to the address, which changes with the
    // root anyway.
    useEffect(() => {
        setFilter("")
        setExpanded(new Set())
    }, [scopeKey])

    // Fetch the listing on mount, when the root changes, and on refresh.
    useEffect(() => {
        let cancelled = false
        setListLoading(true)
        setListError(null)
        listWorkspaceFileRows(scope)
            .then((next) => {
                if (cancelled) return
                setRows(next)
                setListLoading(false)
            })
            .catch((e) => {
                if (cancelled) return
                setListError(String(e))
                setRows(null)
                setListLoading(false)
            })
        return () => {
            cancelled = true
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps -- `scope` is
        // rebuilt every render; `scopeKey` is its value.
    }, [scopeKey, reload])

    // ---- the previewed file's copies --------------------------------------

    const selectedRow = rowForPath(rows ?? [], selectedPath)
    const copies = copyWorktrees(selectedRow)
    const pinnedHere =
        pinnedCopy &&
        pinnedCopy.scopeKey === scopeKey &&
        pinnedCopy.path === selectedPath
            ? pinnedCopy.worktree
            : null
    // Resolution by lookup rather than by an effect that rewrites the pin: a
    // pinned copy the refresh says no longer holds the file falls back instead
    // of reading a file that is gone.
    const activeCopy = resolveCopy(copies, pinnedHere, mainWorktree)

    // Pin the resolved choice as soon as the listing can answer it. Reading
    // `copies[0]` afresh on every render instead would let a refresh that adds
    // a worktree ahead of the one being read silently re-point the preview
    // (`workspace-file-browser`: *The main worktree's copy opens first*).
    useEffect(() => {
        if (!selectedPath || activeCopy === null) return
        if (pinnedHere !== null) return
        setPinnedCopy({ scopeKey, path: selectedPath, worktree: activeCopy })
    }, [scopeKey, selectedPath, activeCopy, pinnedHere])

    // The folders that must be open for the selected file to be visible.
    const revealed = useMemo(() => {
        const out = new Set<string>()
        if (!selectedPath) return out
        const segments = selectedPath.split("/")
        // Every ancestor, not just the immediate parent: a file three folders
        // deep is invisible unless all three are open.
        for (let i = 1; i < segments.length; i++) {
            out.add(segments.slice(0, i).join("/"))
        }
        return out
    }, [selectedPath])

    const [dismissedReveal, setDismissedReveal] = useState<Set<string>>(new Set())
    useEffect(() => setDismissedReveal(new Set()), [selectedPath, scopeKey])

    const effectiveReveal = useMemo(() => {
        if (dismissedReveal.size === 0) return revealed
        const out = new Set(revealed)
        for (const path of dismissedReveal) out.delete(path)
        return out
    }, [revealed, dismissedReveal])

    // The hierarchy is derived from the pooled PATH LIST alone — the divergence
    // markers are a separate lookup applied per row, so the tree is navigable
    // whether or not divergence has been determined.
    const paths = useMemo(() => (rows ?? []).map((r) => r.path), [rows])
    const divergent = useMemo(() => divergentPaths(rows ?? []), [rows])
    const tree = useMemo(() => buildTree(paths), [paths])
    const query = filter.trim().toLowerCase()
    const visibleTree = useMemo(() => filterTree(tree, query), [tree, query])

    // Folders opened only by the reveal are dropped from it on the way past,
    // so the chevron on an ancestor of the selected file responds immediately.
    // Without this the reveal keeps forcing the row open: the glyph would not
    // change, the children would not hide, and a collapse the user never saw
    // happen would still be persisted. `WorkspaceTree`'s own `toggle` solves
    // exactly this for its forced-open reveal (see its A1 comment) — the same
    // hazard follows the same overlay here.
    const toggleFolder = (path: string) => {
        if (revealed.has(path) && !dismissedReveal.has(path)) {
            setDismissedReveal((prev) => new Set(prev).add(path))
            // The row is open *because* the reveal is overriding whatever
            // `expanded` says, so XOR-ing that value could coincidentally
            // compute "open" again. Close it explicitly, which also makes the
            // persisted write match what the click visibly did.
            setExpanded((prev) => {
                if (!prev.has(path)) return prev
                const next = new Set(prev)
                next.delete(path)
                return next
            })
            return
        }
        setExpanded((prev) => {
            const next = new Set(prev)
            if (next.has(path)) next.delete(path)
            else next.add(path)
            return next
        })
    }

    const previewRoot = activeCopy ?? fallbackRoot

    const labels = useMemo(
        () => copyLabels(copies, workspaces),
        // eslint-disable-next-line react-hooks/exhaustive-deps -- `copies` is
        // derived per render; its value is what matters.
        [copies.join("\u0000"), workspaces],
    )

    // The per-file copy control. A chooser when the file has several copies, a
    // plain non-interactive label when it has one
    // (`workspace-file-browser`: *Copy Selection for a Previewed File*).
    //
    // Repository scope only. A flat workspace has no worktrees to choose
    // between — it is one folder — so there is nothing for the control to name
    // and it browses exactly as it did before this feature.
    //
    // Switching copy writes local state and nothing else: it forms no Address
    // and creates no history entry, because it does not change WHICH document
    // is shown (`view-routing`: *Switching copy forms no address*).
    const copyControl =
        scope.kind === "repo" && activeCopy !== null ? (
            <div className="file-browser-copy-row">
                <span className="file-browser-copy-label">Worktree</span>
                {copies.length > 1 ? (
                    <select
                        className="file-browser-copy-select"
                        value={activeCopy}
                        onChange={(e) =>
                            setPinnedCopy({
                                scopeKey,
                                path: selectedPath ?? "",
                                worktree: e.target.value,
                            })
                        }
                        aria-label="Worktree copy"
                    >
                        {copies.map((worktree, i) => (
                            <option key={worktree} value={worktree}>
                                {labels[i]}
                            </option>
                        ))}
                    </select>
                ) : (
                    <span className="file-browser-copy-single">{labels[0]}</span>
                )}
            </div>
        ) : null

    return (
        <div className="file-browser-view">
            <div className="file-browser-header">
                <h2 className="file-browser-title">{label}</h2>
                <input
                    className="file-browser-filter"
                    type="text"
                    placeholder="Filter files…"
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    aria-label="Filter files"
                />
                <button
                    className="file-browser-refresh"
                    onClick={() => setReload((n) => n + 1)}
                    title="Refresh"
                >
                    Refresh
                </button>
            </div>

            {listLoading ? (
                <div className="detail-pane-status">Loading…</div>
            ) : listError ? (
                <EmptyState
                    title="Couldn't list files"
                    body={<code className="detail-pane-error">{listError}</code>}
                />
            ) : paths.length === 0 ? (
                <EmptyState
                    title="No markdown files"
                    body="This workspace has no .md files to browse."
                />
            ) : (
                <div className="file-browser-body">
                    <div className="file-browser-tree-col">
                        {visibleTree.length === 0 ? (
                            <div className="detail-pane-status">
                                No files match “{filter}”.
                            </div>
                        ) : (
                            <div className="tree file-browser-tree">
                                {renderRows(visibleTree, 0, {
                                    expanded,
                                    forceOpen: query !== "",
                                    selectedPath,
                                    revealed: effectiveReveal,
                                    divergent,
                                    onToggleFolder: toggleFolder,
                                    onSelectFile: (path: string) => onSelectFile?.(path),
                                    onOpenReader: (path: string) => onOpenReader?.(path),
                                })}
                            </div>
                        )}
                    </div>
                    <div className="file-browser-preview-col">
                        {/* The preview goes through the shared document view,
                            so the open file stays fresh through a document
                            watch exactly as the detail pane and a reader window
                            do. The LISTING above is untouched and stays
                            pull-based with its refresh control: this is the
                            distinction between *which files exist* and *what
                            one open file says* (`workspace-file-browser`:
                            *Pull-Based Freshness*). */}
                        <DocumentView
                            // Rooted at the SELECTED COPY's worktree, not at
                            // the repository. That root is the read's root, the
                            // document watch's root (so switching copy moves
                            // the watch), and — through `MarkdownView` — the
                            // resolution base and containment root for relative
                            // links, which must follow the bytes being rendered
                            // rather than stay fixed for the browser
                            // (`workspace-file-browser`: *Preview Link
                            // Handling*). A path no tracked worktree holds has
                            // no copy to read, and falls back to the browse
                            // root so the not-found comes from the read.
                            source={
                                selectedPath && previewRoot
                                    ? {
                                          kind: "file",
                                          root: previewRoot,
                                          path: selectedPath,
                                      }
                                    : null
                            }
                            onOpenReader={
                                selectedPath && onOpenReader
                                    ? () => onOpenReader(selectedPath)
                                    : undefined
                            }
                            className="file-browser-document"
                            errorTitle="Couldn't load file"
                            empty={
                                <EmptyState
                                    title="No file selected"
                                    body="Pick a markdown file from the tree to preview it."
                                />
                            }
                            header={(status, headerRef, readerControl) => (
                                /* The browser is workspace-scoped and has no
                                   change context of its own, so the path — not a
                                   change name — is the identity available here.
                                   For a file under openspec/changes/ it contains
                                   the change's directory name anyway
                                   (`workspace-file-browser`: *File Browser
                                   Surface*). */
                                <>
                                <div className="detail-identity" ref={headerRef}>
                                    <div className="detail-identity-inner">
                                        <CopyableIdentity
                                            value={selectedPath ?? ""}
                                            noun="file path"
                                        />
                                        {status.missing && <MissingDocumentLabel />}
                                        {/* Grouped even though this surface has
                                            only one trailing element: the auto
                                            margin that reaches the trailing edge
                                            belongs to the cluster, not to any
                                            one member, so a surface that later
                                            gains a second one cannot split it
                                            (see `IdentityTrailing`). */}
                                        <IdentityTrailing>
                                            {readerControl}
                                        </IdentityTrailing>
                                    </div>
                                </div>
                                {/* Directly beneath the path, which it
                                    qualifies: the path is root-relative and
                                    therefore identical for every copy, so the
                                    control is what names which worktree the
                                    bytes above came from
                                    (`workspace-file-browser`: *The path does
                                    not name the worktree*). */}
                                {copyControl}
                                </>
                            )}
                        />
                    </div>
                </div>
            )}
        </div>
    )
}
