// Mirrors the public structs in `openspec-core`.
// Field names use camelCase per the serde rename_all on each struct.

export interface WorkspaceFolder {
    uri: string
    name: string
}

export interface RegisteredWorkspace {
    uri: string
    name: string
    isMissing: boolean
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

// Tauri event payloads (mirrors crates/specforge/src/events.rs)

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

export const EVENT_CACHE_UPDATED = "cache-updated"
export const EVENT_CHANGE_ADDED = "change-added"
export const EVENT_CHANGE_ARCHIVED = "change-archived"

// Tree-selection discriminated union — used by the master pane to tell the
// detail pane what was clicked. Group 9 reads this to decide what to render.

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
