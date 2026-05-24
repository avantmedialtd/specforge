import { useEffect, useState, type ReactNode } from "react"
import type {
    ArtifactKind,
    ChangeData,
    RegisteredWorkspace,
    Section,
    Task,
    TreeSelection,
} from "../types"
import { stripInlineMarkdown } from "../markdown"
import { EmptyState } from "./EmptyState"

interface WorkspaceTreeProps {
    workspaces: RegisteredWorkspace[]
    changesByWorkspace: Map<string, ChangeData[]>
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

// -------------------------------------------------------------------------
// Node-ID helpers (used as React keys and as entries in the expanded-set)
// -------------------------------------------------------------------------

const workspaceId = (uri: string) => `workspace:${uri}`
const changeId = (uri: string, id: string) => `${workspaceId(uri)}/change:${id}`
const artifactId = (uri: string, id: string, kind: ArtifactKind) =>
    `${changeId(uri, id)}/artifact:${kind}`
const sectionId = (uri: string, id: string, sectionIndex: number) =>
    `${artifactId(uri, id, "tasks")}/section:${sectionIndex}`
const taskId = (
    uri: string,
    id: string,
    sectionIndex: number,
    taskIndex: number,
) => `${sectionId(uri, id, sectionIndex)}/task:${taskIndex}`
const specId = (uri: string, id: string, capability: string) =>
    `${artifactId(uri, id, "specs")}/spec:${capability}`

// -------------------------------------------------------------------------
// Tree root
// -------------------------------------------------------------------------

export function WorkspaceTree({
    workspaces,
    changesByWorkspace,
    selectedNodeId,
    onSelect,
}: WorkspaceTreeProps) {
    const [expanded, setExpanded] = useState<Set<string>>(new Set())

    // Auto-expand workspace nodes the first time we see them so the tree
    // is useful out of the box. User can collapse them manually afterwards.
    useEffect(() => {
        setExpanded((prev) => {
            const next = new Set(prev)
            for (const ws of workspaces) {
                next.add(workspaceId(ws.uri))
            }
            return next
        })
    }, [workspaces])

    const toggle = (id: string) =>
        setExpanded((prev) => {
            const next = new Set(prev)
            if (next.has(id)) next.delete(id)
            else next.add(id)
            return next
        })

    if (workspaces.length === 0) {
        return (
            <EmptyState
                title="No workspaces registered"
                body="Add a folder containing an openspec/ directory from settings."
            />
        )
    }

    return (
        <div className="tree">
            {workspaces.map((ws) => (
                <WorkspaceNode
                    key={workspaceId(ws.uri)}
                    workspace={ws}
                    changes={changesByWorkspace.get(ws.uri) ?? []}
                    expanded={expanded}
                    toggle={toggle}
                    selectedNodeId={selectedNodeId}
                    onSelect={onSelect}
                />
            ))}
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
}: RowProps) {
    return (
        <div
            className={`tree-row${isSelected ? " selected" : ""}`}
            style={{ paddingLeft: depth * 14 + 4 }}
            onClick={onSelect}
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
                    {isExpanded ? "▾" : "▸"}
                </span>
            )}
            {icon && <span className="row-icon">{icon}</span>}
            <span className="row-label">{label}</span>
            {meta != null && <span className="row-meta">{meta}</span>}
        </div>
    )
}

// -------------------------------------------------------------------------
// Per-level node components
// -------------------------------------------------------------------------

interface NodeProps {
    expanded: Set<string>
    toggle: (id: string) => void
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

interface WorkspaceNodeProps extends NodeProps {
    workspace: RegisteredWorkspace
    changes: ChangeData[]
}

function WorkspaceNode({
    workspace,
    changes,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: WorkspaceNodeProps) {
    const nodeId = workspaceId(workspace.uri)
    const isOpen = expanded.has(nodeId)

    return (
        <div>
            <Row
                depth={0}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={workspace.name}
                meta={
                    <>
                        {workspace.isMissing && (
                            <span className="row-badge-missing">missing</span>
                        )}
                        <span className="row-count">{changes.length}</span>
                    </>
                }
                onToggle={() => toggle(nodeId)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "workspace",
                        workspaceUri: workspace.uri,
                    })
                }
            />
            {isOpen && (
                <div>
                    {changes.length === 0 ? (
                        <Row
                            depth={1}
                            isLeaf
                            isSelected={false}
                            label={
                                <span className="row-empty">
                                    no active changes
                                </span>
                            }
                        />
                    ) : (
                        changes.map((change) => (
                            <ChangeNode
                                key={changeId(workspace.uri, change.changeId)}
                                workspaceUri={workspace.uri}
                                change={change}
                                expanded={expanded}
                                toggle={toggle}
                                selectedNodeId={selectedNodeId}
                                onSelect={onSelect}
                            />
                        ))
                    )}
                </div>
            )}
        </div>
    )
}

interface ChangeNodeProps extends NodeProps {
    workspaceUri: string
    change: ChangeData
}

function ChangeNode({
    workspaceUri,
    change,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: ChangeNodeProps) {
    const nodeId = changeId(workspaceUri, change.changeId)
    const isOpen = expanded.has(nodeId)
    const allTasksDone =
        change.artifacts.tasks &&
        change.totalTasks > 0 &&
        change.completedTasks === change.totalTasks

    const label = change.title
        ? stripInlineMarkdown(change.title)
        : change.changeId

    return (
        <div>
            <Row
                depth={1}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                icon={allTasksDone ? "✓" : null}
                label={label}
                meta={
                    <span className="row-changeid" title={change.changeId}>
                        {change.changeId}
                    </span>
                }
                onToggle={() => toggle(nodeId)}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "change",
                        workspaceUri,
                        changeId: change.changeId,
                    })
                }
            />
            {isOpen && (
                <>
                    <ArtifactNode
                        kind="proposal"
                        label="Proposal"
                        present={change.artifacts.proposal}
                        workspaceUri={workspaceUri}
                        change={change}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                    <ArtifactNode
                        kind="specs"
                        label="Specs"
                        present={change.artifacts.specs.length > 0}
                        workspaceUri={workspaceUri}
                        change={change}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                    <ArtifactNode
                        kind="design"
                        label="Design"
                        present={change.artifacts.design}
                        workspaceUri={workspaceUri}
                        change={change}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                    <ArtifactNode
                        kind="tasks"
                        label={
                            change.artifacts.tasks
                                ? `Tasks (${change.completedTasks}/${change.totalTasks})`
                                : "Tasks"
                        }
                        present={change.artifacts.tasks}
                        workspaceUri={workspaceUri}
                        change={change}
                        expanded={expanded}
                        toggle={toggle}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                </>
            )}
        </div>
    )
}

interface ArtifactNodeProps extends NodeProps {
    kind: ArtifactKind
    label: string
    present: boolean
    workspaceUri: string
    change: ChangeData
}

function ArtifactNode({
    kind,
    label,
    present,
    workspaceUri,
    change,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: ArtifactNodeProps) {
    const nodeId = artifactId(workspaceUri, change.changeId, kind)
    const hasChildren =
        (kind === "specs" && change.artifacts.specs.length > 0) ||
        (kind === "tasks" && change.sections.length > 0)
    const isOpen = expanded.has(nodeId)
    const icon = present ? (
        <span className="icon-present">✓</span>
    ) : (
        <span className="icon-absent">○</span>
    )

    return (
        <div>
            <Row
                depth={2}
                isLeaf={!hasChildren}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                icon={icon}
                label={label}
                onToggle={hasChildren ? () => toggle(nodeId) : undefined}
                onSelect={() =>
                    onSelect(nodeId, {
                        kind: "artifact",
                        workspaceUri,
                        changeId: change.changeId,
                        artifactKind: kind,
                    })
                }
            />
            {isOpen && hasChildren && (
                <>
                    {kind === "specs" &&
                        change.artifacts.specs.map((capability) => (
                            <CapabilitySpecNode
                                key={specId(workspaceUri, change.changeId, capability)}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                capability={capability}
                                selectedNodeId={selectedNodeId}
                                onSelect={onSelect}
                            />
                        ))}
                    {kind === "tasks" &&
                        change.sections.map((section, index) => (
                            <SectionNode
                                key={sectionId(workspaceUri, change.changeId, index)}
                                workspaceUri={workspaceUri}
                                changeId={change.changeId}
                                section={section}
                                sectionIndex={index}
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
    workspaceUri: string
    changeId: string
    capability: string
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

function CapabilitySpecNode({
    workspaceUri,
    changeId: cid,
    capability,
    selectedNodeId,
    onSelect,
}: CapabilitySpecNodeProps) {
    const nodeId = specId(workspaceUri, cid, capability)
    return (
        <Row
            depth={3}
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
    workspaceUri: string
    changeId: string
    section: Section
    sectionIndex: number
}

function SectionNode({
    workspaceUri,
    changeId: cid,
    section,
    sectionIndex,
    expanded,
    toggle,
    selectedNodeId,
    onSelect,
}: SectionNodeProps) {
    const nodeId = sectionId(workspaceUri, cid, sectionIndex)
    const isOpen = expanded.has(nodeId)
    return (
        <div>
            <Row
                depth={3}
                isLeaf={section.tasks.length === 0}
                isExpanded={isOpen}
                isSelected={selectedNodeId === nodeId}
                label={stripInlineMarkdown(section.title)}
                onToggle={
                    section.tasks.length > 0 ? () => toggle(nodeId) : undefined
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
                        key={taskId(workspaceUri, cid, sectionIndex, idx)}
                        workspaceUri={workspaceUri}
                        changeId={cid}
                        sectionIndex={sectionIndex}
                        taskIndex={idx}
                        task={task}
                        selectedNodeId={selectedNodeId}
                        onSelect={onSelect}
                    />
                ))}
        </div>
    )
}

interface TaskNodeProps {
    workspaceUri: string
    changeId: string
    sectionIndex: number
    taskIndex: number
    task: Task
    selectedNodeId: string | null
    onSelect: (nodeId: string, selection: TreeSelection) => void
}

function TaskNode({
    workspaceUri,
    changeId: cid,
    sectionIndex,
    taskIndex,
    task,
    selectedNodeId,
    onSelect,
}: TaskNodeProps) {
    const nodeId = taskId(workspaceUri, cid, sectionIndex, taskIndex)
    return (
        <Row
            depth={4}
            isLeaf
            isSelected={selectedNodeId === nodeId}
            icon={
                task.completed ? (
                    <span className="icon-checked">☑</span>
                ) : (
                    <span className="icon-unchecked">☐</span>
                )
            }
            label={stripInlineMarkdown(task.text)}
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
