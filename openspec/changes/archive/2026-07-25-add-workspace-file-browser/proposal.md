# Add Workspace File Browser

## Why

Clicking a repository or flat-workspace row in the sidebar is a dead click today: the `spec-browser` capability's *Deferred Interaction Nodes* requirement explicitly reserves top-level rows "for later UX work", so the center pane doesn't change. Meanwhile every registered workspace is full of markdown the app can already render — docs, READMEs, design notes — with no way to reach any of it. This change cashes in the reserved click: selecting a top-level row opens a read-only markdown file browser for that workspace in the content area.

## What Changes

- **New center-pane surface.** Clicking a Repo group row or a flat workspace row swaps the content area to a two-column file browser: a folder tree of the workspace's markdown files on the left, the selected file rendered with the existing `MarkdownView` on the right. The `RenderTarget` union gains a `files` variant. The commit rail keeps re-scoping to the clicked repository exactly as it does today.
- **Gitignore-respecting, walk-free enumeration.** For git repositories the file list comes from `git ls-files --cached --others --exclude-standard -- '*.md'` through the existing `git_command` chokepoint: git reads its index rather than the directory tree, so ignored directories (`node_modules/`, `target/`, …) are never visited, untracked-but-not-ignored drafts still appear, and WSL workspaces run the enumeration inside the VM instead of stat-ing over 9P. For flat (non-git) workspaces — where `.gitignore` does not exist — a bounded filesystem walk skips dot-directories and common junk directories. Only `.md` files are returned in either mode.
- **Client-derived folder hierarchy.** The full relative-path list arrives in one round trip; the frontend derives the folder tree from it. Expanding a folder costs no IPC, and directories with no markdown anywhere beneath them never appear, with no pruning logic.
- **Guarded workspace-wide reads.** A new `read_workspace_file` command reads one enumerated file, with the same canonicalise-then-prefix-check guard `read_artifact` uses — widened to the workspace root and restricted to `.md` files.
- **No new watchers.** The list is fetched when the browser opens and on manual refresh; the enumeration is a single cheap git spawn, so freshness is pulled, not pushed. The existing `openspec/`-scoped watcher is untouched.
- **Web parity.** Both new commands are added to the `specforge-web` dispatch table, per the web UI's existing command-transport parity requirement.

## Capabilities

### New Capabilities

- `workspace-file-browser`: the center-pane markdown file browser — what opens it, the enumeration contract (gitignore-respecting, `.md`-only, index-read for repos / bounded walk for flat workspaces), the client-side folder-tree derivation, the guarded read, and the pull-based freshness model.

### Modified Capabilities

- `spec-browser`: the *Deferred Interaction Nodes* requirement changes — clicking a top-level Repo group or flat workspace row now opens the file browser in the detail pane instead of producing no observable effect (logical-change parent rows, change nodes, and the Specs artifact node remain deferred). The empty-top-level-row requirement's "without changing the detail pane" clause changes the same way: a row with zero active changes still has files worth browsing.

## Impact

- `crates/openspec-core` — new enumeration function beside the other `git.rs` porcelain (ls-files variant) plus the flat-workspace walk; no new dependencies.
- `crates/openspec-app` — `AppService` methods for `list_markdown_files` and `read_workspace_file` (shared by desktop, web, and any future frontend).
- `crates/specforge` — two thin `#[tauri::command]` wrappers in `commands.rs`.
- `crates/specforge-web` — two entries in the `dispatch.rs` command table.
- Frontend — `files` variant on `RenderTarget` in `src/types.ts`; `App.tsx` `handleSelect` gains behaviour for the `repo` / `workspace` cases; new `FileBrowserView` component reusing `MarkdownView`; wrappers in `src/api.ts`.
- Untouched: watcher, cache, settings schema, notifications, TUI.
