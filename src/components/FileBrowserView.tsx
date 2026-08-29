import { useEffect, useMemo, useState } from "react"
import type { ReactNode } from "react"
import { listMarkdownFiles } from "../api"
import { CopyableIdentity } from "./CopyableIdentity"
import { EmptyState } from "./EmptyState"
import { DocumentView, MissingDocumentLabel } from "./DocumentView"
import { ChevronDown, ChevronRight } from "./icons"

interface FileBrowserViewProps {
    /// The browse root: a repository's main worktree, or a flat workspace
    /// folder. Re-fetches the listing whenever this changes.
    root: string
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
                        if (e.metaKey || e.ctrlKey) {
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
                </button>,
            )
        }
    }
    return rows
}

/// The workspace file browser: a folder tree of the browse root's markdown
/// files on the left, the selected file rendered with `MarkdownView` on the
/// right. Fetches its own listing on mount and whenever `root` changes,
/// mirroring `ArchiveView`'s lifecycle — no watcher, freshness is pulled via
/// the refresh control.
export function FileBrowserView({
    root,
    label,
    selectedPath = null,
    onSelectFile,
    onOpenReader,
}: FileBrowserViewProps) {
    const [files, setFiles] = useState<string[] | null>(null)
    const [listLoading, setListLoading] = useState(false)
    const [listError, setListError] = useState<string | null>(null)
    // Bumped by the refresh control to force a re-fetch of the listing.
    const [reload, setReload] = useState(0)

    const [filter, setFilter] = useState("")
    const [expanded, setExpanded] = useState<Set<string>>(new Set())

    // Reset the root-scoped UI state this component still owns when the browse
    // root changes — otherwise a stale filter/expansion from the previous
    // workspace would linger until the user interacts again. The selection is
    // no longer among them: it belongs to the address, which changes with the
    // root anyway.
    useEffect(() => {
        setFilter("")
        setExpanded(new Set())
    }, [root])

    // Fetch the listing on mount, when the root changes, and on refresh.
    useEffect(() => {
        let cancelled = false
        setListLoading(true)
        setListError(null)
        listMarkdownFiles(root)
            .then((rows) => {
                if (cancelled) return
                setFiles(rows)
                setListLoading(false)
            })
            .catch((e) => {
                if (cancelled) return
                setListError(String(e))
                setFiles(null)
                setListLoading(false)
            })
        return () => {
            cancelled = true
        }
    }, [root, reload])

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

    const tree = useMemo(() => buildTree(files ?? []), [files])
    const query = filter.trim().toLowerCase()
    const visibleTree = useMemo(() => filterTree(tree, query), [tree, query])

    const toggleFolder = (path: string) => {
        setExpanded((prev) => {
            const next = new Set(prev)
            if (next.has(path)) next.delete(path)
            else next.add(path)
            return next
        })
    }

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
            ) : (files?.length ?? 0) === 0 ? (
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
                                    revealed,
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
                            source={
                                selectedPath ? { kind: "file", root, path: selectedPath } : null
                            }
                            className="file-browser-document"
                            errorTitle="Couldn't load file"
                            empty={
                                <EmptyState
                                    title="No file selected"
                                    body="Pick a markdown file from the tree to preview it."
                                />
                            }
                            header={(status, headerRef) => (
                                /* The browser is workspace-scoped and has no
                                   change context of its own, so the path — not a
                                   change name — is the identity available here.
                                   For a file under openspec/changes/ it contains
                                   the change's directory name anyway
                                   (`workspace-file-browser`: *File Browser
                                   Surface*). */
                                <div className="detail-identity" ref={headerRef}>
                                    <div className="detail-identity-inner">
                                        <CopyableIdentity
                                            value={selectedPath ?? ""}
                                            noun="file path"
                                        />
                                        {status.missing && <MissingDocumentLabel />}
                                    </div>
                                </div>
                            )}
                        />
                    </div>
                </div>
            )}
        </div>
    )
}
