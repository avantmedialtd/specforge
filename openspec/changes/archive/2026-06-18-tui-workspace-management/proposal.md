# Terminal UI Workspace Management

## Why

The terminal frontend (`specforge-tui`) can browse every registered workspace but cannot change the set — there is no way to add a workspace, remove one, or rename/recolor it from the terminal. A user working over SSH, or who simply lives in the terminal, must switch to the desktop app to register a folder. The desktop already exposes the full registry surface — add via a native folder picker, remove, an inline display-name field, and a palette swatch — through its Settings view; the terminal has only a read-only tree plus a two-toggle Settings screen.

The capability the terminal lacks is not in the core. `WorkspaceRegistry` and `WorkspacePresentationStore` already expose registration, removal (with the discovered-worktree cascade), and validated name/color writes, and `AppService` holds both as public fields. What is missing is (a) the orchestration that wires a registration into the watcher, which today lives inside the Tauri command layer rather than the shared service, and (b) a terminal UI to drive it.

## What Changes

- Extend the terminal Settings screen with a **Workspaces** section below the existing toggles: an "Add workspace" action, and one row per user-registered workspace showing its name, path, missing/stale indicator, and (when set) palette color.
- **Add** a workspace by typing or pasting an absolute path into a one-line prompt; the registry's existing validation (path exists, is a directory, contains an `openspec/` subdirectory) is surfaced inline on failure.
- **Remove** a user-registered workspace with a confirmation step that names the discovered worktrees the cascade will also drop.
- **Rename** a workspace (display name; an empty value clears it back to the default basename) and **set its color** (cycle through the eight curated palette tokens plus "none"), both persisted immediately to the presentation store with no separate save action.
- Lift the register/unregister watcher orchestration out of the Tauri command layer and into `AppService::add_workspace` / `remove_workspace`, so both frontends call one tested code path and the terminal's existing watcher subscription refreshes the tree automatically. Lift the presentation write likewise into `AppService::set_workspace_presentation`.
- Add a small reusable terminal overlay primitive (text prompt + yes/no confirm) and generalize the Settings screen's cursor from a fixed two-row model to a typed, scrollable row list.

All writes target the shared application configuration directory (the workspace registry and the presentation store), never the contents of any registered workspace — consistent with the terminal frontend's read-only-with-respect-to-workspaces guarantee.

## Impact

- **Affected specs:** `terminal-ui` — modified *Settings Screen* and *Read-Only Operation* requirements; new *Workspace Management from the Terminal* requirement. `workspace-registry` is untouched (the registry is caller-agnostic; its desktop *Settings View* requirement stays desktop-specific).
- **Affected code:** `crates/openspec-app` (new `AppService` workspace/presentation methods), `crates/specforge` (Tauri commands become thin callers of those methods), `crates/specforge-tui` (Settings screen, overlay primitive, key handling, render and persistence tests). No change to `openspec-core` registry, presentation, or parse logic.
- **Cross-frontend:** desktop behavior is unchanged; it routes through the same new service methods.
- **Known limitation (unchanged by this work):** a desktop instance and a terminal instance running concurrently still co-write the registry file with last-writer-wins semantics and no cross-process notification.
