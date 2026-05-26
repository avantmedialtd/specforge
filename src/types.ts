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

export const EVENT_CACHE_UPDATED = "cache-updated"
export const EVENT_CHANGE_ADDED = "change-added"
export const EVENT_CHANGE_ARCHIVED = "change-archived"
export const EVENT_WORKSPACE_REMOVED = "workspace-removed"
export const EVENT_LOGICAL_CHANGE_ADDED = "logical-change-added"
export const EVENT_LOGICAL_CHANGE_ARCHIVED = "logical-change-archived"
export const EVENT_INSTANCE_ADDED = "instance-added"
export const EVENT_INSTANCE_REMOVED = "instance-removed"
export const EVENT_WORKSPACE_PRESENTATION_UPDATED = "workspace-presentation-updated"

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
