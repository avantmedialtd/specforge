# Workspace File Browser

## ADDED Requirements

### Requirement: File Browser Surface

Clicking a top-level Repo group row or a flat workspace node in the tree SHALL render the workspace file browser in the detail pane, replacing the pane's current contents (artifact, commit, or Dashboard). The browser SHALL present two regions: a navigable folder tree of the workspace's markdown files, and a read-only preview that renders the selected file with the same markdown renderer used for change artifacts. For a Repo group the browse root SHALL be the repository's main worktree; for a flat workspace it SHALL be the workspace folder itself. Opening the browser SHALL dismiss the Settings and Archive panes, and SHALL NOT alter the commit rail's existing re-scoping behaviour for the clicked row. The browser is read-only: it SHALL NOT provide any editing affordance.

#### Scenario: Repo group click opens the file browser

- **WHEN** the user clicks a top-level Repo group row
- **THEN** the detail pane renders the file browser rooted at the repository's main worktree
- **AND** the commit rail re-scopes to that repository exactly as before this feature

#### Scenario: Flat workspace click opens the file browser

- **WHEN** the user clicks a top-level flat workspace node
- **THEN** the detail pane renders the file browser rooted at the workspace folder

#### Scenario: Selecting a file renders its markdown

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** the preview region renders that file's markdown with the same renderer used for artifacts

#### Scenario: Opening the browser dismisses modal panes

- **WHEN** the Settings or Archive pane is open and the user clicks a top-level row
- **THEN** the pane closes and the detail pane shows the file browser

### Requirement: Ignore-Respecting Markdown Enumeration

For a browse root inside a git repository, the file listing SHALL be produced from the git index via `ls-files` with `--cached`, `--others`, and `--exclude-standard`, pathspec-limited to markdown files, executed through the shared git chokepoint (`git_command`). The listing SHALL NOT recursively traverse the working tree. Untracked files matched by gitignore rules SHALL be excluded; untracked files not matched SHALL be included. For a non-git browse root, the listing SHALL come from a bounded filesystem walk that skips dot-prefixed entries, does not follow directory symlinks, and skips a fixed set of well-known dependency/build directory names. In both modes the listing SHALL contain only files with a `.md` extension (matched case-insensitively) and SHALL be returned as sorted, de-duplicated, forward-slash relative paths.

#### Scenario: Gitignored directory is excluded without being visited

- **WHEN** a repository's `.gitignore` ignores `node_modules/` and that directory contains `.md` files
- **THEN** no path under `node_modules/` appears in the listing
- **AND** the enumeration reads the git index rather than traversing the directory

#### Scenario: Untracked draft appears

- **WHEN** a new `.md` file exists in the repository but has never been committed and is not gitignored
- **THEN** the file appears in the listing

#### Scenario: Non-markdown files are excluded

- **WHEN** the workspace contains source files, images, and other non-markdown files
- **THEN** none of them appear in the listing

#### Scenario: WSL workspace enumerates through the git chokepoint

- **WHEN** the browse root is a Windows-registered WSL workspace (`\\wsl.localhost\…`)
- **THEN** the enumeration is routed through the same `wsl.exe` git chokepoint as every other git call for that workspace

#### Scenario: Flat workspace walk skips junk directories

- **WHEN** a non-git workspace contains a `node_modules/` directory and a `.hidden/` directory, each with `.md` files inside
- **THEN** no path under either directory appears in the listing

### Requirement: Client-Derived Folder Tree

The frontend SHALL derive the browser's folder hierarchy from the returned flat path list. Expanding or collapsing a folder SHALL NOT trigger any backend request. Directories with no markdown file anywhere beneath them SHALL NOT appear. Folders SHALL sort before files, each group ordered case-insensitively.

#### Scenario: Folder expansion is purely client-side

- **WHEN** the user expands or collapses a folder in the browser's tree
- **THEN** no backend command is invoked

#### Scenario: Empty directories never appear

- **WHEN** the workspace contains a directory with no `.md` file anywhere in its subtree
- **THEN** that directory does not appear in the browser's tree

### Requirement: Path Filter

The browser SHALL provide a filter input that narrows the tree to files whose relative path contains the filter text (case-insensitive substring match). Matching files SHALL be shown with their ancestor folders revealed. Clearing the filter SHALL restore the unfiltered tree.

#### Scenario: Filter reveals a nested match

- **WHEN** the user types a fragment matching a file deep inside collapsed folders
- **THEN** the matching file is visible with its ancestor folders revealed
- **AND** non-matching files are hidden

### Requirement: Guarded Workspace File Read

The file-read operation SHALL resolve the requested relative path against the browse root and reject: absolute paths, paths containing parent-directory components, resolved paths that do not remain under the canonicalised browse root (including symlink escapes), files without a case-insensitive `.md` extension, and files larger than the size cap. A rejected or failed read SHALL surface a readable error in the preview region while the browser remains usable.

#### Scenario: Path traversal is rejected

- **WHEN** a read is requested for a relative path containing `..` that would resolve outside the browse root
- **THEN** the read is rejected with an error
- **AND** no file content outside the root is returned

#### Scenario: Symlink escaping the workspace is rejected

- **WHEN** a listed path resolves through a symlink to a file outside the browse root
- **THEN** the read is rejected with an error

#### Scenario: Missing file degrades gracefully

- **WHEN** a listed file no longer exists on disk at read time
- **THEN** the preview region shows a readable error
- **AND** the folder tree remains usable

### Requirement: Pull-Based Freshness

The listing SHALL be fetched when the browser opens and when its browse root changes, and the browser SHALL provide a manual refresh control that re-runs the enumeration. The application SHALL NOT register any filesystem watcher for the browser.

#### Scenario: New file appears after refresh

- **WHEN** a `.md` file is created in the workspace while the browser is open
- **AND** the user activates the refresh control
- **THEN** the new file appears in the tree

#### Scenario: No watcher is registered for browsing

- **WHEN** the file browser is opened for a workspace
- **THEN** no additional filesystem watcher is created beyond the existing `openspec/`-scoped watcher

### Requirement: Empty and Error States

A workspace with no markdown files SHALL render an explicit empty state in the browser. A failed enumeration (for example, git unavailable or the root unreadable) SHALL render a non-crashing error state in the detail pane.

#### Scenario: Workspace with no markdown

- **WHEN** the enumeration returns zero files
- **THEN** the browser shows an empty-state message instead of an empty tree

#### Scenario: Enumeration failure degrades to an error state

- **WHEN** the enumeration fails
- **THEN** the detail pane shows a readable error state and the application does not crash

### Requirement: Cross-Frontend Command Surface

The listing and read operations SHALL be exposed as `AppService` methods and dispatched through both the Tauri command layer and the web server's command table, per the web UI's command-transport parity contract.

#### Scenario: Browser works over the web transport

- **WHEN** the frontend is served by the embedded web server
- **THEN** the file browser's listing and read commands dispatch through the web command table and behave as on desktop
