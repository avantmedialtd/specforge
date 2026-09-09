# workspace-file-browser Specification

## Purpose

Defines the read-only markdown file browser opened by selecting a top-level row (a repository group or a flat workspace) in the tree: how the browse root is authorized and enumerated without traversing ignored directories, how a repository's listing is pooled across **every tracked worktree** of it and de-duplicated on the root-relative path, how the folder hierarchy is derived on the client and marks paths whose copies differ, how the previewed file's copy is chosen and how relative links are contained by that copy's worktree, how a file is guarded and read, how the listing stays fresh without a watcher, and how the previewed file — a single open document rather than a workspace-sized tree — stays fresh through a document watch.
## Requirements

### Requirement: File Browser Surface

Clicking a top-level Repo group row or a flat workspace node in the tree SHALL render the workspace file browser in the detail pane, replacing the pane's current contents (artifact, commit, or Dashboard). The browser SHALL present two regions: a navigable folder tree of the workspace's markdown files, and a read-only preview that renders the selected file with the same markdown renderer used for change artifacts. For a Repo group the browse root SHALL be the **repository**, whose listing is pooled across its tracked worktrees (see *Union Markdown Listing Across a Repository's Worktrees*); for a flat workspace it SHALL be the workspace folder itself. Opening the browser SHALL dismiss the Settings and Archive panes, and SHALL NOT alter the commit rail's existing re-scoping behaviour for the clicked row. The browser is read-only: it SHALL NOT provide any editing affordance.

While a file is selected, the preview region SHALL display that file's **root-relative path** above the rendered markdown, so the preview identifies what it is showing. The path SHALL be shown in full, exactly as it addresses the file beneath the browse root, with no leading separator and no truncation. The browser is workspace-scoped and has no change context of its own, so the path — not a change name — is the identity available here; for a file under `openspec/changes/`, the path contains the change's directory name.

The path SHALL **copy itself when clicked**, exactly as specified by the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability: a single primary click SHALL place exactly that path on the clipboard and select it as confirmation, the outcome SHALL be indicated and announced, a refused write SHALL leave the value selected, and the path SHALL be keyboard-activatable as a tab stop without introducing any global chord.

The displayed path is root-relative and therefore identical for every copy of a file. Which worktree the rendered bytes came from SHALL be named by the copy control (see *Copy Selection for a Previewed File*) rather than folded into the path, so the path stays copyable as a filesystem identifier.

The preview's path sits below the browser's own header rather than flush with the top of the window, so it takes no titlebar-strip clearance.

When no file is selected, the preview region SHALL continue to show its existing empty state and SHALL render no path.

#### Scenario: Repo group click opens the file browser

- **WHEN** the user clicks a top-level Repo group row
- **THEN** the detail pane renders the file browser rooted at that repository, listing markdown pooled across its tracked worktrees
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

#### Scenario: The path does not name the worktree

- **WHEN** a file whose copies live in several worktrees is previewed
- **THEN** the displayed path is the root-relative path alone
- **AND** the worktree the bytes came from is named by the copy control instead

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

This enumeration is **per worktree**, and it is the unit the repository-scoped union pools. Each tracked worktree is enumerated by its own git index and its own ignore rules, so a file untracked in one worktree and committed in another is included from both, and a file ignored in one is excluded only there.

#### Scenario: Gitignored directory is excluded without being visited

- **WHEN** a repository's `.gitignore` ignores `node_modules/` and that directory contains `.md` files
- **THEN** no path under `node_modules/` appears in the listing
- **AND** the enumeration reads the git index rather than traversing the directory

#### Scenario: Untracked draft appears

- **WHEN** a new `.md` file exists in the repository but has never been committed and is not gitignored
- **THEN** the file appears in the listing

#### Scenario: Each worktree is enumerated by its own ignore rules

- **WHEN** a path is ignored in one tracked worktree and tracked in another
- **THEN** the union contains that path
- **AND** the copy it records is the worktree that does not ignore it

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

A row whose path exists in more than one tracked worktree with **differing content** SHALL be marked as differing. The marker SHALL state only that the copies differ; it SHALL NOT rank them, name one as newer or authoritative, or imply which should be read. Every tracked worktree sits on its own branch, so this marker is what tells the reader where the branches have actually drifted — the reason the file browser surfaces it where the archive browser deliberately does not.

Determining that copies differ SHALL NOT delay the tree: the hierarchy SHALL render from the path list alone, with markers applied when known.

#### Scenario: Folder expansion is purely client-side

- **WHEN** the user expands or collapses a folder in the browser's tree
- **THEN** no backend command is invoked

#### Scenario: Empty directories never appear

- **WHEN** the workspace contains a directory with no `.md` file anywhere in its subtree
- **THEN** that directory does not appear in the browser's tree

#### Scenario: A file differing between worktrees is marked

- **WHEN** two tracked worktrees hold the same path with different content
- **THEN** that row is marked as differing

#### Scenario: Identical copies are not marked

- **WHEN** every tracked worktree holding a path holds identical content for it
- **THEN** that row carries no differing marker

#### Scenario: The tree renders before divergence is known

- **WHEN** the listing has loaded and divergence has not yet been determined
- **THEN** the folder hierarchy is already rendered and navigable

### Requirement: Path Filter

The browser SHALL provide a filter input that narrows the tree to files whose relative path contains the filter text (case-insensitive substring match). Matching files SHALL be shown with their ancestor folders revealed. Clearing the filter SHALL restore the unfiltered tree.

#### Scenario: Filter reveals a nested match

- **WHEN** the user types a fragment matching a file deep inside collapsed folders
- **THEN** the matching file is visible with its ancestor folders revealed
- **AND** non-matching files are hidden

### Requirement: Browsing Is Confined to Registered Workspaces

Both the enumeration and the read SHALL authorize the caller-supplied browse root against the workspace registry before touching the filesystem, and SHALL refuse a root that is neither a registered (or registry-discovered) workspace nor a path inside a registered repository — even when real markdown files exist there. Any worktree of a repository SHALL be accepted as a browse root when that repository has a registered workspace, because a Repo group's listing pools every tracked worktree and the user may have registered only one of them — which need not be the main worktree. This authorization SHALL be enforced at the shared application boundary so it holds for every frontend and transport, and SHALL be applied *in addition to* the path guard: this requirement bounds *which* roots may be browsed, the path guard bounds *where within* a root a read may reach. The root SHALL be matched by canonical path using the same canonicalization the registry keys on, and the canonical root SHALL be the one used for resolution, so the path that was authorized is the path that is read.

A repository-scoped listing SHALL be authorized by its repository identifier, accepted only when that identifier matches the canonical git directory of a registered workspace. A repository the user has not registered SHALL be refused rather than enumerated, and no worktree of it SHALL be read.

#### Scenario: An unregistered root is refused

- **WHEN** an enumeration or read is requested for a root that is neither a registered workspace nor inside a registered repository
- **THEN** the request is refused with an error
- **AND** no directory is enumerated and no file is read, even though markdown files exist under that root

#### Scenario: A repository registered only by a linked worktree authorizes its other worktrees

- **WHEN** the user has registered a linked worktree of a repository and browses the corresponding Repo group row
- **THEN** every tracked worktree of that repository is accepted as a browse root and its markdown files are listed

#### Scenario: A registry-discovered worktree is an acceptable browse root

- **WHEN** a repository's sibling worktree was auto-discovered rather than registered by the user
- **AND** an enumeration or read is requested for that worktree
- **THEN** it is authorized
- **AND** it is not refused for being absent from the user-registered workspace listing

#### Scenario: An unregistered repository is refused

- **WHEN** a repository-scoped listing is requested for an identifier matching no registered workspace's git directory
- **THEN** the request is refused with an error
- **AND** no worktree of that repository is enumerated

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

The **listing** SHALL be fetched when the browser opens and when its browse root changes, and the browser SHALL provide a manual refresh control that re-runs the enumeration. For a repository the refresh SHALL re-run every tracked worktree's enumeration, so a file created in any of them appears. The application SHALL NOT register any filesystem watcher for the listing: enumeration walks or queries the whole browse root, and keeping it continuously fresh would mean watching a workspace-sized tree — the cost this requirement exists to refuse. Pooling across worktrees multiplies that cost and therefore strengthens the refusal rather than weakening it.

The **preview** is not bound by that refusal, because it concerns exactly one file. While a file is selected, the preview SHALL keep that file's rendered content fresh through a document watch (see the `document-watch` capability), registered when the selection is made and released when the selection changes or the browser closes. The watch SHALL follow the **selected copy**, so switching copies moves it to the newly selected worktree's file. A refresh the user did not initiate SHALL preserve the reading position, SHALL NOT present a loading indicator, and SHALL NOT alter the rendered document when the file's bytes are unchanged, per the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

The distinction is between *which files exist*, which stays pull-based, and *what one open file says*, which does not. A file created while the browser is open therefore still requires a refresh to appear in the tree; a file already selected updates in place.

#### Scenario: New file appears after refresh

- **WHEN** a `.md` file is created in the workspace while the browser is open
- **AND** the user activates the refresh control
- **THEN** the new file appears in the tree

#### Scenario: Refresh spans every tracked worktree

- **WHEN** a `.md` file is created in a tracked worktree other than the one whose copy is being previewed
- **AND** the user activates the refresh control
- **THEN** the new file appears in the tree

#### Scenario: No watcher is registered for the listing

- **WHEN** the file browser is opened for a workspace and no file is selected
- **THEN** no filesystem watcher is created beyond the existing `openspec/`-scoped watcher

#### Scenario: A new file does not appear without a refresh

- **WHEN** a `.md` file is created in the workspace while the browser is open
- **AND** the user does not activate the refresh control
- **THEN** the tree does not yet list the new file

#### Scenario: The previewed file updates in place

- **WHEN** a file is selected in the browser and its contents are modified on disk
- **THEN** the preview re-renders with the updated content without user action
- **AND** the reading position is preserved and no loading indicator is presented

#### Scenario: Switching copy moves the watch

- **WHEN** the user selects a different worktree's copy of the previewed file
- **THEN** the previously watched copy is no longer watched
- **AND** the newly selected copy is

#### Scenario: Selecting a different file moves the watch

- **WHEN** the user selects a different file in the browser
- **THEN** the previously previewed file is no longer watched
- **AND** the newly selected file is

#### Scenario: Closing the browser releases the watch

- **WHEN** the file browser is closed or its browse root changes
- **THEN** no document watch remains registered for the file it was previewing

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

Markdown rendered in the file browser's preview SHALL be governed by the *Link Handling in Rendered Artifacts* requirement of the `spec-browser` capability. The resolution base and the containment root SHALL be the worktree of the **copy currently being previewed** — not the repository, and not its main worktree.

This follows the bytes. A relative link is written against the tree the file lives in, so resolving it against a different worktree would open the wrong file where the target exists in both, and refuse the link as outside the root where the target exists only alongside its source. Containment is a boundary, not a presentational detail, so it moves with the copy rather than being fixed for the browser.

The browser's enumeration and read contracts are unchanged: listings remain markdown-only and the guarded read remains markdown-only — a linked `.html` file is opened through the validated open operation, never listed or read through the browsing surface.

#### Scenario: A mockup link in a previewed file opens

- **WHEN** a previewed markdown file in the file browser contains a relative link to an `.html` file that exists inside the authorized browse root
- **THEN** clicking the link opens the file via the operating system's default handler
- **AND** the preview pane neither navigates nor blanks

#### Scenario: A link resolves within the previewed copy's worktree

- **WHEN** a file previewed from a feature worktree contains a relative link to a file that exists in that worktree
- **AND** the repository's main worktree does not contain the link's target
- **THEN** the link opens the target in the previewed copy's worktree
- **AND** it is not refused as outside the containment root

#### Scenario: Switching copy moves the containment root

- **WHEN** the user switches the preview to a different worktree's copy
- **THEN** subsequent relative links resolve against that worktree
- **AND** a target outside it is refused

#### Scenario: Preview links under a worktree accepted by repository membership open

- **WHEN** the browse root is a worktree accepted because a worktree of that repository is registered
- **THEN** a valid mockup link in a previewed file opens rather than being refused

### Requirement: The Selected File Is Addressable

Selecting a file in the browser SHALL form an Address naming that file within its browse root, per the *File Addresses* requirement in the `view-routing` capability. The selection SHALL therefore be linkable, SHALL be reflected in the served UI's browser location, and SHALL be restored when the application loads at that address — opening the browser rooted at the named root with that file selected and previewed.

The Address SHALL name the repository and the root-relative path, and SHALL NOT name which worktree's copy is shown. The copy is a per-preview choice, not part of what is being addressed: the address identifies a file in a repository, and resolution opens a default copy of it (see *Copy Selection for a Previewed File*). A link to a file that today exists only in a feature worktree therefore opens the main worktree's copy once that branch merges, which is the intended reading.

Selecting a file SHALL create a history entry, and selecting a different file SHALL create another, so that the back gesture returns to the previously previewed file. Expanding or collapsing a folder, typing in the path filter, and **switching which copy is previewed** SHALL NOT form an Address or create a history entry, because none of them changes which document is shown (see the *History Entry Discipline* requirement in the `view-routing` capability).

A file address that resolves to a root but names a file that exists in no tracked worktree SHALL report not found while leaving the browser's tree usable, consistent with the *Empty and Error States* requirement.

#### Scenario: Selecting a file forms an address

- **WHEN** the user selects a `.md` file in the browser's folder tree
- **THEN** an Address naming that file within the browse root is formed

#### Scenario: A file address restores the browser and the selection

- **WHEN** the application loads at an address naming a file within a registered browse root
- **THEN** the file browser opens rooted at that root
- **AND** that file is selected and its markdown is previewed

#### Scenario: Switching copy forms no address

- **WHEN** the user switches the preview to a different worktree's copy
- **THEN** no new Address is formed and no history entry is created
- **AND** the browser location is unchanged

#### Scenario: Back returns to the previously previewed file

- **WHEN** the user selects one file and then another
- **AND** invokes the back gesture
- **THEN** the first file is selected and previewed again

#### Scenario: Folder expansion forms no address

- **WHEN** the user expands or collapses a folder, or types in the path filter
- **THEN** no new Address is formed and no history entry is created

#### Scenario: An address naming a file in no worktree reports not found

- **WHEN** the application loads at a file address whose root resolves but whose path exists in no tracked worktree
- **THEN** it reports not found
- **AND** the browser's folder tree remains usable

### Requirement: A Previewed File Opens in a Reader Window

A file row in the browser's folder tree SHALL be openable in a reader window by the launch gesture specified in the *Launching a Reader Window* requirement of the `reader-window` capability. Doing so SHALL NOT change the browser's own selection, its preview, or its scroll position.

#### Scenario: Opening a file row in a reader leaves the browser alone

- **WHEN** the user performs the reader launch gesture on a file row while a different file is selected
- **THEN** a reader window opens for the clicked file
- **AND** the browser's selection and preview are unchanged

### Requirement: Union Markdown Listing Across a Repository's Worktrees

For a Repo group, the file browser's listing SHALL be the union of the markdown enumerations of **every tracked worktree of that repository**, including worktrees SpecForge auto-discovered rather than ones the user registered directly. Each worktree SHALL be enumerated exactly as a single browse root is today (see *Ignore-Respecting Markdown Enumeration*), and the results pooled.

The union SHALL be de-duplicated on the **root-relative path**, so one path is one row however many worktrees hold it. Each row SHALL retain every copy it collapsed, each copy identified by the worktree holding it; that worktree, paired with the row's path, is what addresses a read.

A path present in only one worktree SHALL appear in the listing exactly as one present in all of them. This is the case the union exists for: a document written in a feature worktree — committed or not — is otherwise unreachable from its repository's row until the branch merges.

One unreadable worktree SHALL contribute nothing rather than failing the listing, so a worktree on an unmounted volume or with an unreadable tree cannot blank every other worktree's files.

For a flat workspace the union is over that single folder, and the listing is exactly what it is today.

#### Scenario: A file in only one worktree is listed

- **WHEN** a markdown file exists in one tracked worktree of a repository and in no other
- **AND** the user opens the file browser for that repository
- **THEN** the file appears in the tree
- **AND** it appears whether that worktree was registered by the user or auto-discovered by SpecForge

#### Scenario: The same path in several worktrees is one row

- **WHEN** three tracked worktrees each contain `docs/guide.md`
- **THEN** the tree shows exactly one `docs/guide.md` row
- **AND** that row records all three copies

#### Scenario: An uncommitted draft is reachable

- **WHEN** a markdown file exists in a feature worktree, is not gitignored, and has never been committed
- **THEN** it appears in the repository's listing

#### Scenario: One unreadable worktree does not blank the listing

- **WHEN** one tracked worktree of a repository cannot be enumerated
- **THEN** the listing still contains every other worktree's markdown files
- **AND** no error replaces the tree

#### Scenario: A flat workspace is unaffected

- **WHEN** the file browser is opened for a flat (non-git) workspace
- **THEN** the listing contains that folder's markdown files and no others

### Requirement: Copy Selection for a Previewed File

When a previewed file exists in more than one tracked worktree, the preview SHALL present a control for choosing which worktree's copy is rendered, and choosing a copy SHALL re-point **only that preview**. Selecting a copy SHALL NOT change the listing, SHALL NOT clear the path filter, and SHALL NOT collapse the folder tree.

When the file exists in exactly one tracked worktree, the control SHALL be rendered as a plain, non-interactive label naming that worktree rather than as a chooser.

Copies SHALL be labelled by the workspace's display name, falling back to the worktree folder's basename.

The control SHALL NOT present copies as interchangeable. Every tracked worktree is on its own branch, so two copies of one path routinely differ; the file browser reads the working tree, so the difference is whatever is on disk right now.

Which copy opens first SHALL be deterministic: the repository's main worktree when it holds the file, and otherwise the first copy in the row's copy order.

#### Scenario: Switching to another worktree's copy

- **WHEN** a file present in two worktrees is previewed
- **AND** the user selects the second worktree's copy
- **THEN** the preview renders that worktree's copy of the file
- **AND** the tree, its expansion state and the path filter are unchanged

#### Scenario: A single-copy file shows a label, not a chooser

- **WHEN** a previewed file exists in exactly one tracked worktree
- **THEN** that worktree is named as a plain label
- **AND** no copy chooser is offered

#### Scenario: The main worktree's copy opens first

- **WHEN** a file present in the main worktree and in a feature worktree is selected
- **THEN** the main worktree's copy is previewed first

#### Scenario: A file absent from the main worktree still opens

- **WHEN** a file present only in a feature worktree is selected
- **THEN** that worktree's copy is previewed
- **AND** the copy control names that worktree
