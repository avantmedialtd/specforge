# workspace-file-browser Specification

## Purpose

Defines the read-only markdown file browser opened by selecting a top-level row (a repository group or a flat workspace) in the tree: how the browse root is authorized and enumerated without traversing ignored directories, how the folder hierarchy is derived on the client, how a file is guarded and read, and how the listing stays fresh without a watcher.
## Requirements
### Requirement: File Browser Surface

Clicking a top-level Repo group row or a flat workspace node in the tree SHALL render the workspace file browser in the detail pane, replacing the pane's current contents (artifact, commit, or Dashboard). The browser SHALL present two regions: a navigable folder tree of the workspace's markdown files, and a read-only preview that renders the selected file with the same markdown renderer used for change artifacts. For a Repo group the browse root SHALL be the repository's main worktree; for a flat workspace it SHALL be the workspace folder itself. Opening the browser SHALL dismiss the Settings and Archive panes, and SHALL NOT alter the commit rail's existing re-scoping behaviour for the clicked row. The browser is read-only: it SHALL NOT provide any editing affordance.

While a file is selected, the preview region SHALL display that file's **root-relative path** above the rendered markdown, so the preview identifies what it is showing. The path SHALL be shown in full, exactly as it addresses the file beneath the browse root, with no leading separator and no truncation. The browser is workspace-scoped and has no change context of its own, so the path — not a change name — is the identity available here; for a file under `openspec/changes/`, the path contains the change's directory name.

The path SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability: a single primary click SHALL place exactly that path on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the path SHALL be keyboard-activatable as a tab stop without introducing any global chord. (This supersedes the previous contract, which specified selection only and forbade an application clipboard write.)

The preview's path sits below the browser's own header rather than flush with the top of the window, so it takes no titlebar-strip clearance.

When no file is selected, the preview region SHALL continue to show its existing empty state and SHALL render no path.

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

#### Scenario: Preview names the selected file's path

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** the preview region shows that file's root-relative path above the rendered markdown
- **AND** the path is shown in full, with no truncation

#### Scenario: A change artifact's path carries the change directory name

- **WHEN** the selected file is an artifact under `openspec/changes/<name>/`
- **THEN** the displayed path contains `<name>`, the change's directory name

#### Scenario: One click copies the whole path

- **WHEN** the user clicks once on the displayed path
- **THEN** exactly that path is placed on the clipboard
- **AND** it is also selected, as confirmation of what was copied
- **AND** the outcome is indicated and announced

#### Scenario: No file selected shows no path

- **WHEN** the file browser is open and no file has been selected
- **THEN** the preview region shows its empty state
- **AND** no path is displayed

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

### Requirement: Browsing Is Confined to Registered Workspaces

Both the enumeration and the read SHALL authorize the caller-supplied browse root against the workspace registry before touching the filesystem, and SHALL refuse a root that is neither a registered (or registry-discovered) workspace nor a path inside a registered repository — even when real markdown files exist there. A repository's main worktree SHALL be accepted when any worktree of that repository is registered, because a Repo group browses the main worktree and the user may have registered only a linked worktree. This authorization SHALL be enforced at the shared application boundary so it holds for every frontend and transport, and SHALL be applied *in addition to* the path guard: this requirement bounds *which* roots may be browsed, the path guard bounds *where within* a root a read may reach. The root SHALL be matched by canonical path using the same canonicalization the registry keys on, and the canonical root SHALL be the one used for resolution, so the path that was authorized is the path that is read.

#### Scenario: An unregistered root is refused

- **WHEN** an enumeration or read is requested for a root that is neither a registered workspace nor inside a registered repository
- **THEN** the request is refused with an error
- **AND** no directory is enumerated and no file is read, even though markdown files exist under that root

#### Scenario: A repository registered only by a linked worktree authorizes its main worktree

- **WHEN** the user has registered a linked worktree of a repository and browses the corresponding Repo group row
- **THEN** the repository's main worktree is accepted as the browse root and its markdown files are listed

#### Scenario: The confinement holds across transports

- **WHEN** an enumeration or read is reached through the web command endpoint rather than the desktop command surface
- **THEN** the same registered-root requirement applies, because it is enforced at the shared application boundary

### Requirement: Guarded Workspace File Read

The file-read operation SHALL resolve the requested relative path against the authorized browse root and reject: absolute paths, paths containing parent-directory components, resolved paths that do not remain under the canonicalised browse root (including symlink escapes), files without a case-insensitive `.md` extension, and files larger than the size cap. This guard SHALL apply independently of the registry authorization above, so a traversal-shaped path is refused even within an authorized root. A rejected or failed read SHALL surface a readable error in the preview region while the browser remains usable.

The read guard SHALL NOT consult ignore rules: ignore rules govern *what the browser enumerates*, not what may be read, so a caller naming an ignored `.md` file inside an authorized root is served it. This mirrors the existing artifact read, which is likewise ignore-agnostic, and keeps the read free of a per-read `check-ignore` spawn or a server-side listing cache. Membership in a previously returned listing SHALL NOT be required.

#### Scenario: An ignored file is not listed but is readable when named

- **WHEN** a caller requests a `.md` file that lies inside an authorized root but is excluded from the listing by ignore rules
- **THEN** the file's contents are returned, because ignore rules bound enumeration rather than read authorization
- **AND** the file still does not appear in the browser's tree

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

### Requirement: Preview Link Handling

Markdown rendered in the file browser's preview SHALL be governed by the *Link Handling in Rendered Artifacts* requirement of the `spec-browser` capability, with the authorized browse root serving as both the resolution base's root and the containment root for relative file links. The browser's enumeration and read contracts are unchanged: listings remain markdown-only and the guarded read remains markdown-only — a linked `.html` file is opened through the validated open operation, never listed or read through the browsing surface.

#### Scenario: A mockup link in a previewed file opens

- **WHEN** a previewed markdown file in the file browser contains a relative link to an `.html` file that exists inside the authorized browse root
- **THEN** clicking the link opens the file via the operating system's default handler
- **AND** the preview pane neither navigates nor blanks

#### Scenario: Preview links under an accepted main worktree open

- **WHEN** the browse root is a repository main worktree accepted because a worktree of that repository is registered
- **THEN** a valid mockup link in a previewed file opens rather than being refused

