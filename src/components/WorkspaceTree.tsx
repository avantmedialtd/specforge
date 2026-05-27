import { useEffect, useState, type ReactNode } from "react"
import type {
    ArtifactKind,
    ChangeData,
    ChangeInstance,
    DivergenceLabel,
    LogicalChange,
    PaletteColor,
    RepoView,
    Section,
    Task,
    TreeSelection,
    WorkspaceFolder,
    WorkspaceView,
} from "../types"
import { stripInlineMarkdown } from "../markdown"
import { EmptyState } from "./EmptyState"
import {
    Check,
    CheckSquare,
    ChevronDown,
    ChevronRight,
    Square,
} from "./icons"
import {
    getCollapsedTreeNodeIds,
    getExpandedTreeNodeIds,
    setCollapsedTreeNodeIds,
    setExpandedTreeNodeIds,
} from "../api"

interface WorkspaceTreeProps {
    views: WorkspaceView[]
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

// -------------------------------------------------------------------------
// Node-ID helpers — stable React keys + entries in the collapsed-set (also
// persisted to settings, so they need to round-trip across app restarts). Each
// helper composes on the one above so a section node ID embeds its
// containing change, artifact, etc.
// -------------------------------------------------------------------------

const flatWorkspaceId = (uri: string) => `flat:${uri}`
const repoId = (id: string) => `repo:${id}`
const logicalChangeId = (rid: string, name: string) =>
    `${repoId(rid)}/lc:${name}`
const instanceId = (rid: string, name: string, wt: string) =>
    `${logicalChangeId(rid, name)}/inst:${wt}`
/// `containerId` is either a flat-workspace id, a logical-change id (when
/// singleton-flattened), or an instance id. It scopes the artifact/section/
/// task subtree to its host.
const changeRowId = (containerId: string, changeId: string) =>
    `${containerId}/change:${changeId}`
const artifactNodeId = (
    containerId: string,
    changeId: string,
    kind: ArtifactKind,
) => `${changeRowId(containerId, changeId)}/artifact:${kind}`
const sectionNodeId = (
    containerId: string,
    changeId: string,
    sectionIndex: number,
) => `${artifactNodeId(containerId, changeId, "tasks")}/section:${sectionIndex}`
const taskNodeId = (
    containerId: string,
    changeId: string,
    sectionIndex: number,
    taskIndex: number,
) => `${sectionNodeId(containerId, changeId, sectionIndex)}/task:${taskIndex}`
const specNodeId = (
    containerId: string,
    changeId: string,
    capability: string,
) => `${artifactNodeId(containerId, changeId, "specs")}/spec:${capability}`

// -------------------------------------------------------------------------
// Per-node default expansion. Most nodes default to open; the two below
// flip to "closed" when their work is complete, so finished groups stop
// crowding out in-progress work. A user override (collapse against a
// default-open node, or expand against a default-closed node) is honoured
// by `WorkspaceTree`'s two persisted ID sets.
// -------------------------------------------------------------------------

function defaultIsOpenForTasksArtifact(change: ChangeData): boolean {
    return !(
        change.totalTasks > 0 && change.completedTasks === change.totalTasks
    )
}

function defaultIsOpenForSection(section: Section): boolean {
    return !(
        section.tasks.length > 0 && section.tasks.every((t) => t.completed)
    )
}

/// True iff every parsed task in the change is complete (and at least one
/// task exists). Drives the trailing completion glyph on both the
/// flat-change row and the per-instance row; centralised so the rule
/// can't drift between the two paths.
function allTasksDone(change: ChangeData): boolean {
    return (
        change.artifacts.tasks &&
        change.totalTasks > 0 &&
        change.completedTasks === change.totalTasks
    )
}

// -------------------------------------------------------------------------
// Tree root
// -------------------------------------------------------------------------

export function WorkspaceTree({
    views,
    selectedNodeId,
    onSelect,
}: WorkspaceTreeProps) {
    // Two override sets, one per direction of default:
    //   `collapsed` — user-closed against a default-open node.
    //   `expanded`  — user-opened against a default-closed node (today only
    //                 a completed Tasks artifact or completed Section).
    // Most nodes default to open, so most clicks land in `collapsed`. The
    // `expanded` set only fills up as users opt back into seeing done work.
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
    const [expanded, setExpanded] = useState<Set<string>>(new Set())
    const [hydrated, setHydrated] = useState(false)

    // Hydrate both sets in parallel. While the call is in flight the tree
    // renders against empty override sets, which means default-closed nodes
    // (completed groups) appear closed and default-open nodes appear open —
    // both correct under the new default — and only the user's persisted
    // overrides snap once hydration lands. Errors still flip `hydrated` so
    // the tree becomes interactive even if settings are unreadable.
    useEffect(() => {
        let cancelled = false
        Promise.all([
            getCollapsedTreeNodeIds().catch(() => [] as string[]),
            getExpandedTreeNodeIds().catch(() => [] as string[]),
        ])
            .then(([collapsedIds, expandedIds]) => {
                if (cancelled) return
                setCollapsed(new Set(collapsedIds))
                setExpanded(new Set(expandedIds))
                setHydrated(true)
            })
            .catch(() => {
                if (cancelled) return
                setHydrated(true)
            })
        return () => {
            cancelled = true
        }
    }, [])

    // Two debounced-persistence effects, identical shape. Each writes its own
    // set; a click against a default-open node never touches `expanded`, so
    // the writes naturally stay independent.
    useEffect(() => {
        if (!hydrated) return
        const timer = setTimeout(() => {
            void setCollapsedTreeNodeIds([...collapsed])
        }, 150)
        return () => clearTimeout(timer)
    }, [collapsed, hydrated])

    useEffect(() => {
        if (!hydrated) return
        const timer = setTimeout(() => {
            void setExpandedTreeNodeIds([...expanded])
        }, 150)
        return () => clearTimeout(timer)
    }, [expanded, hydrated])

    const toggle = (id: string, defaultOpen: boolean) => {
        const setter = defaultOpen ? setCollapsed : setExpanded
        setter((prev) => {
            const next = new Set(prev)
            if (next.has(id)) next.delete(id)
            else next.add(id)
            return next
        })
    }

    if (views.length === 0) {
        return (
            <EmptyState
                title="No workspaces registered"
                body="Add a folder containing an openspec/ directory from settings."
            />
        )
    }

    return (
        <div className="tree">
            {views.map((view) =>
                view.kind === "repo" ? (
                    <RepoNode
                        key={repoId(view.repoId)}
                        repo={view}
                        collapsed={collapsed}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                ) : (
                    <FlatWorkspaceNode
                        key={flatWorkspaceId(view.workspace.uri)}
                        workspace={view.workspace}
                        changes={view.changes}
                        displayName={view.displayName}
                        color={view.color}
                        collapsed={collapsed}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                ),
            )}
        </div>
    )
}

// -------------------------------------------------------------------------
// Row primitive
// -------------------------------------------------------------------------

interface RowProps {
    depth: number
    isLeaf?: boolean
    isExpanded?: boolean
    isSelected: boolean
    icon?: ReactNode
    label: ReactNode
    meta?: ReactNode
    onToggle?: () => void
    onSelect?: () => void
    /// Tint palette token. Renders as a dim background on the row only —
    /// child rows are unaffected. `null` / undefined = no tint, identical to
    /// the row's default background.
    tint?: PaletteColor | null
    /// Optional `title` attribute for the row — used to surface the path on
    /// renamed top-level rows so they remain disambiguatable.
    title?: string
    /// Renders the row in a dim + inert state — used as a slot indicator
    /// for missing artifacts. The row layout footprint is preserved, but
    /// the click handler is suppressed at the React level and (via CSS)
    /// `pointer-events: none` cancels hover/cursor as well.
    dim?: boolean
}

function Row({
    depth,
    isLeaf,
    isExpanded,
    isSelected,
    icon,
    label,
    meta,
    onToggle,
    onSelect,
    tint,
    title,
    dim,
}: RowProps) {
    const tintClass = tint ? ` tree-row--tinted tree-row--tint-${tint}` : ""
    const dimClass = dim ? " tree-row--dim" : ""
    return (
        <div
            className={`tree-row${isSelected ? " selected" : ""}${tintClass}${dimClass}`}
            style={{ paddingLeft: depth * 12 + 4 }}
            onClick={dim ? undefined : onSelect}
            title={title}
        >
            {isLeaf ? (
                <span className="chevron chevron-spacer" />
            ) : (
                <span
                    className={`chevron${isExpanded ? " open" : ""}`}
                    onClick={(e) => {
                        e.stopPropagation()
                        onToggle?.()
                    }}
                >
                    {isExpanded ? <ChevronDown /> : <ChevronRight />}
                </span>
            )}
            {icon && <span className="row-icon">{icon}</span>}
            <span className="row-label">{label}</span>
            {meta != null && <span className="row-meta">{meta}</span>}
        </div>
    )
}

// -------------------------------------------------------------------------
// Common props shared by container nodes
// -------------------------------------------------------------------------

interface NodeProps {
    collapsed: Set<string>
    expanded: Set<string>
    /// `defaultOpen` selects which override set the click mutates:
    /// `true`  → xor into `collapsed` (the default for almost every node).
    /// `false` → xor into `expanded`  (used only by the two auto-collapse
    ///           node types — Tasks artifact and Section — when their work
    ///           is complete).
    toggle: (id: string, defaultOpen: boolean) => void
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

// -------------------------------------------------------------------------
// Repo group + descendants
// -------------------------------------------------------------------------

interface RepoNodeProps extends NodeProps {
    repo: RepoView & { kind: "repo" }
}

function RepoNode({
    repo,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: RepoNodeProps) {
    const nodeId = repoId(repo.repoId)
    const isEmpty = repo.active.length === 0
    const isOpen = !collapsed.has(nodeId)
    const label = repo.displayName ?? repo.name

    return (
        <div>
            <Row
                depth={0}
                isLeaf={isEmpty}
                isExpanded={!isEmpty && isOpen}
                isSelected={selectedNodeId === nodeId}
                label={label}
                tint={repo.color}
                title={repo.mainWorktree}
                meta={
                    <>
                        {repo.defaultBranch && (
                            <span className="row-branch">
                                {repo.defaultBranch}
                            </span>
                        )}
                        <span className="row-count">{repo.active.length}</span>
                    </>
                }
                onToggle={isEmpty ? undefined : () => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, { kind: "repo", repoId: repo.repoId })
                }
            />
            {!isEmpty && isOpen && (
                <div>
                    {repo.active.map((lc) => (
                        <LogicalChangeRow
                            key={logicalChangeId(repo.repoId, lc.name)}
                            repoId={repo.repoId}
                            logical={lc}
                            collapsed={collapsed}
                            expanded={expanded}
                            toggle={toggle}
                            selectedNodeId={selectedNodeId}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
}

interface LogicalChangeRowProps extends NodeProps {
    repoId: string
    logical: LogicalChange
}

/// Either a flattened single-instance row (no parent disclosure) or a
/// parent disclosure with one child per instance.
function LogicalChangeRow({
    repoId: rid,
    logical,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: LogicalChangeRowProps) {
    if (logical.instances.length === 1) {
        return (
            <InstanceNode
                repoId={rid}
                changeName={logical.name}
                instance={logical.instances[0]!}
                isPrimary={true}
                isSingleton={true}
                depth={1}
                collapsed={collapsed}
                expanded={expanded}
                toggle={toggle}
                selectedNodeId={selectedNodeId}
                onSelect={onSelect}
            />
        )
    }

    const nodeId = logicalChangeId(rid, logical.name)
    const isOpen = !collapsed.has(nodeId)

    return (
        <DisclosureGroup
            id={nodeId}
            depth={1}
            label={logical.name}
            meta={
                <span className="row-count">
                    {logical.instances.length}
                </span>
            }
            isOpen={isOpen}
            isSelected={selectedNodeId === nodeId}
            onToggle={() => toggle(nodeId, true)}
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "logicalChange",
                    repoId: rid,
                    changeName: logical.name,
                })
            }
        >
            {logical.instances.map((inst, idx) => (
                <InstanceNode
                    key={instanceId(rid, logical.name, inst.worktreePath)}
                    repoId={rid}
                    changeName={logical.name}
                    instance={inst}
                    isPrimary={idx === 0}
                    isSingleton={false}
                    depth={2}
                    collapsed={collapsed}
                    expanded={expanded}
                    toggle={toggle}
                    selectedNodeId={selectedNodeId}
                    onSelect={onSelect}
                />
            ))}
        </DisclosureGroup>
    )
}

interface DisclosureGroupProps {
    id: string
    depth: number
    label: ReactNode
    meta?: ReactNode
    isOpen: boolean
    isSelected: boolean
    onToggle: () => void
    onSelect: () => void
    children: ReactNode
}

function DisclosureGroup({
    depth,
    label,
    meta,
    isOpen,
    isSelected,
    onToggle,
    onSelect,
    children,
}: DisclosureGroupProps) {
    return (
        <div>
            <Row
                depth={depth}
                isExpanded={isOpen}
                isSelected={isSelected}
                label={label}
                meta={meta}
                onToggle={onToggle}
                onSelect={onSelect}
            />
            {isOpen && <div>{children}</div>}
        </div>
    )
}

interface InstanceNodeProps extends NodeProps {
    repoId: string
    changeName: string
    instance: ChangeInstance
    isPrimary: boolean
    isSingleton: boolean
    depth: number
}

function InstanceNode({
    repoId: rid,
    changeName,
    instance,
    isPrimary,
    isSingleton,
    depth,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: InstanceNodeProps) {
    const nodeId = instanceId(rid, changeName, instance.worktreePath)
    const isOpen = !collapsed.has(nodeId)
    const label = labelForInstance(instance, isSingleton ? changeName : null)
    const meta = (
        <>
            {/* Active indicator: only on the primary of multi-instance
                logical changes — singletons are unambiguous, no dot needed. */}
            {isPrimary && !isSingleton && (
                <span
                    className="status-dot status-dot--ok"
                    title="Most recently modified"
                    aria-label="Most recently modified"
                />
            )}
            {instance.change.artifacts.tasks && instance.change.totalTasks > 0 && (
                <span className="row-progress">
                    {instance.change.completedTasks}/{instance.change.totalTasks}
                </span>
            )}
            {allTasksDone(instance.change) && (
                <Check className="icon-checked" />
            )}
            <span className="row-mtime" title={new Date(instance.modifiedAt * 1000).toISOString()}>
                {formatRelativeTime(instance.modifiedAt)}
            </span>
            {instance.divergence && (
                <DivergenceChip label={instance.divergence} />
            )}
        </>
    )

    return (
        <div>
            <Row
                depth={depth}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={label}
                meta={meta}
                onToggle={() => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "instance",
                        repoId: rid,
                        changeName,
                        worktreePath: instance.worktreePath,
                    })
                }
            />
            {isOpen && (
                <ArtifactSubtree
                    containerId={nodeId}
                    workspaceUri={instance.worktreePath}
                    change={instance.change}
                    depth={depth + 1}
                    collapsed={collapsed}
                    expanded={expanded}
                    toggle={toggle}
                    selectedNodeId={selectedNodeId}
                    onSelect={onSelect}
                />
            )}
        </div>
    )
}

function labelForInstance(
    instance: ChangeInstance,
    fallbackName: string | null,
): ReactNode {
    // For singletons we surface the change name as the primary label since
    // the instance row IS the change row visually. For multi-instance rows
    // the parent already shows the change name; the instance label
    // distinguishes worktrees by branch (or path basename).
    if (fallbackName) {
        return (
            <>
                <span>{stripInlineMarkdown(fallbackName)}</span>
                {instance.branch && (
                    <span className="row-branch">{instance.branch}</span>
                )}
            </>
        )
    }
    const primary =
        instance.branch ?? basename(instance.worktreePath) ?? instance.worktreePath
    return primary
}

function basename(path: string): string | null {
    const parts = path.split("/").filter(Boolean)
    return parts.length > 0 ? parts[parts.length - 1]! : null
}

const REL_THRESHOLDS: [number, string][] = [
    [60, "s"],
    [3600, "m"],
    [86400, "h"],
    [604800, "d"],
    [2592000, "w"],
]

function formatRelativeTime(unixSeconds: number): string {
    if (unixSeconds === 0) return "—"
    const nowSec = Math.floor(Date.now() / 1000)
    const delta = Math.max(0, nowSec - unixSeconds)
    for (let i = 0; i < REL_THRESHOLDS.length; i++) {
        const [threshold, unit] = REL_THRESHOLDS[i]!
        if (delta < threshold) {
            const prev = i === 0 ? 1 : REL_THRESHOLDS[i - 1]![0]
            return `${Math.max(1, Math.floor(delta / prev))}${unit} ago`
        }
    }
    const months = Math.floor(delta / 2592000)
    return `${months}mo ago`
}

function DivergenceChip({ label }: { label: DivergenceLabel }) {
    const text = label === "diverged" ? "diverged" : "stale"
    const tone = label === "diverged" ? "chip--warn" : "chip--muted"
    return <span className={`chip ${tone}`}>{text}</span>
}

// -------------------------------------------------------------------------
// Flat (non-git) workspace + descendants
// -------------------------------------------------------------------------

interface FlatWorkspaceNodeProps extends NodeProps {
    workspace: WorkspaceFolder
    changes: ChangeData[]
    displayName: string | null
    color: PaletteColor | null
}

function FlatWorkspaceNode({
    workspace,
    changes,
    displayName,
    color,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: FlatWorkspaceNodeProps) {
    const nodeId = flatWorkspaceId(workspace.uri)
    const isEmpty = changes.length === 0
    const isOpen = !collapsed.has(nodeId)
    const label = displayName ?? workspace.name

    return (
        <div>
            <Row
                depth={0}
                isLeaf={isEmpty}
                isExpanded={!isEmpty && isOpen}
                isSelected={selectedNodeId === nodeId}
                label={label}
                tint={color}
                title={workspace.uri}
                meta={<span className="row-count">{changes.length}</span>}
                onToggle={isEmpty ? undefined : () => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "workspace",
                        workspaceUri: workspace.uri,
                    })
                }
            />
            {!isEmpty && isOpen && (
                <div>
                    {changes.map((change) => (
                        <FlatChangeNode
                            key={changeRowId(nodeId, change.changeId)}
                            containerId={nodeId}
                            workspaceUri={workspace.uri}
                            change={change}
                            collapsed={collapsed}
                            expanded={expanded}
                            toggle={toggle}
                            selectedNodeId={selectedNodeId}
                            onSelect={onSelect}
                        />
                    ))}
                </div>
            )}
        </div>
    )
}

interface FlatChangeNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    change: ChangeData
}

function FlatChangeNode({
    containerId,
    workspaceUri,
    change,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: FlatChangeNodeProps) {
    const nodeId = changeRowId(containerId, change.changeId)
    const isOpen = !collapsed.has(nodeId)
    const isCompleted = allTasksDone(change)

    const label = change.title
        ? stripInlineMarkdown(change.title)
        : change.changeId

    return (
        <div>
            <Row
                depth={1}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={label}
                meta={
                    <>
                        {isCompleted && (
                            <Check className="icon-checked" />
                        )}
                        <span className="row-changeid" title={change.changeId}>
                            {change.changeId}
                        </span>
                    </>
                }
                onToggle={() => toggle(nodeId, true)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "change",
                        workspaceUri,
                        changeId: change.changeId,
                    })
                }
            />
            {isOpen && (
                <ArtifactSubtree
                    containerId={nodeId}
                    workspaceUri={workspaceUri}
                    change={change}
                    depth={2}
                    collapsed={collapsed}
                    expanded={expanded}
                    toggle={toggle}
                    selectedNodeId={selectedNodeId}
                    onSelect={onSelect}
                />
            )}
        </div>
    )
}

// -------------------------------------------------------------------------
// Artifact / Specs / Sections / Tasks subtree (shared by Flat and Instance)
// -------------------------------------------------------------------------

interface ArtifactSubtreeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    change: ChangeData
    depth: number
}

function ArtifactSubtree({
    containerId,
    workspaceUri,
    change,
    depth,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: ArtifactSubtreeProps) {
    return (
        <>
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="proposal"
                label="Proposal"
                present={change.artifacts.proposal}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                toggle={toggle}
                selectedNodeId={selectedNodeId}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="specs"
                label="Specs"
                present={change.artifacts.specs.length > 0}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                toggle={toggle}
                selectedNodeId={selectedNodeId}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="design"
                label="Design"
                present={change.artifacts.design}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                toggle={toggle}
                selectedNodeId={selectedNodeId}
                onSelect={onSelect}
            />
            <ArtifactNode
                containerId={containerId}
                workspaceUri={workspaceUri}
                kind="tasks"
                label={
                    change.artifacts.tasks
                        ? `Tasks (${change.completedTasks}/${change.totalTasks})`
                        : "Tasks"
                }
                present={change.artifacts.tasks}
                change={change}
                depth={depth}
                collapsed={collapsed}
                expanded={expanded}
                toggle={toggle}
                selectedNodeId={selectedNodeId}
                onSelect={onSelect}
            />
        </>
    )
}

interface ArtifactNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    kind: ArtifactKind
    label: string
    present: boolean
    change: ChangeData
    depth: number
}

function ArtifactNode({
    containerId,
    workspaceUri,
    kind,
    label,
    present,
    change,
    depth,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: ArtifactNodeProps) {
    const nodeId = artifactNodeId(containerId, change.changeId, kind)
    const hasChildren =
        (kind === "specs" && change.artifacts.specs.length > 0) ||
        (kind === "tasks" && change.sections.length > 0)
    // Tasks artifact flips to default-closed when every task is complete;
    // every other artifact stays default-open.
    const defaultOpen =
        kind === "tasks" ? defaultIsOpenForTasksArtifact(change) : true
    const isOpen = defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId)

    return (
        <div>
            <Row
                depth={depth}
                isLeaf={!hasChildren}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={label}
                dim={!present}
                onToggle={
                    present && hasChildren
                        ? () => toggle(nodeId, defaultOpen)
                        : undefined
                }
                onSelect={
                    present
                        ? () =>
                              onSelect(nodeId, {
                                  kind: "artifact",
                                  workspaceUri,
                                  changeId: change.changeId,
                                  artifactKind: kind,
                              })
                        : undefined
                }
            />
            {isOpen && hasChildren && (
                <>
                    {kind === "specs" &&
                        change.artifacts.specs.map((capability) => (
                            <CapabilitySpecNode
                                key={specNodeId(containerId, change.changeId, capability)}
                                containerId={containerId}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                capability={capability}
                                depth={depth + 1}
                                selectedNodeId={selectedNodeId}
                                onSelect={onSelect}
                            />
                        ))}
                    {kind === "tasks" &&
                        change.sections.map((section, index) => (
                            <SectionNode
                                key={sectionNodeId(containerId, change.changeId, index)}
                                containerId={containerId}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                section={section}
                                sectionIndex={index}
                                depth={depth + 1}
                                collapsed={collapsed}
                                expanded={expanded}
                                toggle={toggle}
                                selectedNodeId={selectedNodeId}
                                onSelect={onSelect}
                            />
                        ))}
                </>
            )}
        </div>
    )
}

interface CapabilitySpecNodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    capability: string
    depth: number
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

function CapabilitySpecNode({
    containerId,
    workspaceUri,
    changeId: cid,
    capability,
    depth,
    selectedNodeId,
    onSelect,
}: CapabilitySpecNodeProps) {
    const nodeId = specNodeId(containerId, cid, capability)
    return (
        <Row
            depth={depth}
            isLeaf
            isSelected={selectedNodeId === nodeId}
            label={capability}
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "spec",
                    workspaceUri,
                    changeId: cid,
                    capability,
                })
            }
        />
    )
}

interface SectionNodeProps extends NodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    section: Section
    sectionIndex: number
    depth: number
}

function SectionNode({
    containerId,
    workspaceUri,
    changeId: cid,
    section,
    sectionIndex,
    depth,
    collapsed,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: SectionNodeProps) {
    const nodeId = sectionNodeId(containerId, cid, sectionIndex)
    const allTasksDone =
        section.tasks.length > 0 && section.tasks.every((t) => t.completed)
    const defaultOpen = defaultIsOpenForSection(section)
    const isOpen = defaultOpen ? !collapsed.has(nodeId) : expanded.has(nodeId)
    return (
        <div>
            <Row
                depth={depth}
                isLeaf={section.tasks.length === 0}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={stripInlineMarkdown(section.title)}
                title={stripInlineMarkdown(section.title)}
                meta={
                    allTasksDone ? (
                        <Check className="icon-checked" />
                    ) : undefined
                }
                onToggle={
                    section.tasks.length > 0
                        ? () => toggle(nodeId, defaultOpen)
                        : undefined
                }
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "section",
                        workspaceUri,
                        changeId: cid,
                        sectionIndex,
                    })
                }
            />
            {isOpen &&
                section.tasks.map((task, idx) => (
                    <TaskNode
                        key={taskNodeId(containerId, cid, sectionIndex, idx)}
                        containerId={containerId}
                        workspaceUri={workspaceUri}
                        changeId={cid}
                        sectionIndex={sectionIndex}
                        taskIndex={idx}
                        task={task}
                        depth={depth + 1}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                ))}
        </div>
    )
}

interface TaskNodeProps {
    containerId: string
    workspaceUri: string
    changeId: string
    sectionIndex: number
    taskIndex: number
    task: Task
    depth: number
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

function TaskNode({
    containerId,
    workspaceUri,
    changeId: cid,
    sectionIndex,
    taskIndex,
    task,
    depth,
    selectedNodeId,
    onSelect,
}: TaskNodeProps) {
    const nodeId = taskNodeId(containerId, cid, sectionIndex, taskIndex)
    return (
        <Row
            depth={depth}
            isLeaf
            isSelected={selectedNodeId === nodeId}
            icon={
                task.completed ? (
                    <CheckSquare className="icon-checked" />
                ) : (
                    <Square className="icon-unchecked" />
                )
            }
            label={stripInlineMarkdown(task.text)}
            title={stripInlineMarkdown(task.text)}
            onSelect={() =>
                onSelect(nodeId, {
                    kind: "task",
                    workspaceUri,
                    changeId: cid,
                    sectionIndex,
                    taskIndex,
                    lineNumber: task.lineNumber,
                })
            }
        />
    )
}
