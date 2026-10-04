# commit-graph Specification

## Purpose

Defines the commit-graph rail: the third, resizable right-hand pane of the main window that renders a faithful `git log --all` graph — lanes, branch/merge topology, ref decorations, and local-calendar day separators — for the repository owning the current tree selection, together with the commit-detail view (metadata, git trailers, changed-file list, diff) that a selected commit renders in the centre pane. It owns the `get_commit_graph` / `get_commit_detail` / `get_commit_diff` command surface and its guarantees across every frontend: reads confined to registered repositories, commit references passed as injection-safe non-option arguments, refresh within the watcher's debounce window, no git work while the rail is hidden, and strictly read-only operation with no OpenSpec semantics in the graph. The Dashboard's today-scoped, author-coloured per-workspace graphs are the separate `commit-garden` capability.
## Requirements
### Requirement: Commit-Graph Rail Pane

While commit history is on (see the *Commit History Can Be Turned Off* requirement), the main window SHALL present the commit graph in a rail: a third, resizable pane positioned to the far right of the tree and detail panes. The rail SHALL render the commit graph of the repository that owns the current tree selection. The rail SHALL be hideable and restorable by the user (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability); while visible it behaves as specified here. While commit history is off, the commit graph is neither rendered nor one of the rail's occupants, and the rail exists only while a pull-request panel occupies it (see the *Rail Exists Only While Occupied* requirement in the `spec-browser` capability).

When the current tree selection belongs to a git repository, the rail SHALL render that repository's graph. When the selection is a non-git (flat) workspace, or when no node is selected, the rail SHALL render an empty placeholder state and SHALL NOT error. As the tree selection moves between nodes belonging to different repositories, the rail SHALL re-target to the newly selected repository.

While the rail is hidden, the application SHALL NOT perform commit-graph reads (git subprocess work) on the rail's behalf: hiding the rail suspends graph fetching, and re-targeting the rail's repository while hidden SHALL cost nothing. Restoring the rail SHALL fetch and render the graph for the repository that owns the current tree selection at that moment.

The divider between the detail pane and the rail SHALL be draggable to resize the rail, and the chosen width SHALL persist across sessions, consistent with the existing master-detail divider.

#### Scenario: Rail shows the selected repository's graph

- **WHEN** commit history is on
- **AND** the user selects any node belonging to a git-backed workspace
- **THEN** the rail renders the commit graph of that node's repository

#### Scenario: Rail re-targets when selection moves to another repository

- **WHEN** the rail is showing repository A's graph
- **AND** the user selects a node belonging to a different repository B
- **THEN** the rail re-renders with repository B's graph

#### Scenario: Rail is empty for non-git workspaces

- **WHEN** commit history is on
- **AND** the user selects a node belonging to a non-git (flat) workspace, or no node is selected
- **THEN** the rail renders an empty placeholder state
- **AND** the rest of the application is unaffected

#### Scenario: Rail width is resizable and persists

- **WHEN** the user drags the divider between the detail pane and the rail
- **THEN** the rail resizes to the dragged width
- **AND** the width is restored on the next launch

#### Scenario: Hidden rail performs no graph fetching

- **WHEN** the rail is hidden
- **AND** the user selects nodes belonging to different repositories
- **THEN** no commit-graph read is performed for any of those selections

#### Scenario: Restoring the rail fetches the current repository's graph

- **WHEN** commit history is on and the rail is hidden while the tree selection belongs to repository A
- **AND** the user moves the selection to repository B and then restores the rail
- **THEN** the rail fetches and renders repository B's graph

### Requirement: Faithful Commit Graph Rendering

The rail SHALL render a faithful git commit graph built from `git log --all`: one node per commit, placed in a lane (column), with vertical edges where a lane continues and diagonal edges where a branch is created or a merge collapses. The rail SHALL render ref decorations — local branch heads, remote branch heads, tags, and the HEAD pointer — and a commit subject for each row. Author, full commit date, and abbreviated hash SHALL be available on hover.

The rail SHALL group commit rows into calendar-day sections by author date in the viewer's local time zone, inserting a labelled day-separator row above the first commit of each day (including the newest day at the top). A day-separator is presentation only: it carries no commit node, and lane edges that are alive across the day boundary SHALL pass straight through the separator band so that branch lines are never visually broken. Day grouping SHALL NOT reorder commits, alter lane assignment, or change which decorations a commit carries.

The day-separator label SHALL be relative to the viewer's current calendar day in their local time zone. The current day SHALL be labelled `Today` and the immediately preceding day `Yesterday`. Days two through six before the current day SHALL be labelled by weekday name alone (e.g. `Wednesday`); within this window a bare weekday name is unambiguous because the current day and the prior six days span all seven weekday names exactly once. Days seven or more before the current day SHALL be labelled by absolute date in the existing compact form (e.g. `Mon, May 25`). Relative wording SHALL be locale-aware, consistent with the rail's other locale-respecting date formatting. The label text SHALL NOT change grouping, ordering, lane assignment, or which day a commit falls under.

The graph SHALL carry no OpenSpec semantics: commits SHALL NOT be tinted, grouped, filtered, or otherwise annotated by their OpenSpec change, change-id trailer, or archive status. Calendar-day grouping is a neutral temporal affordance and is explicitly NOT an OpenSpec annotation; the only relationship between the rail and OpenSpec state is the repository scope inherited from the tree selection.

#### Scenario: Graph matches git's own view

- **WHEN** the rail renders a repository's graph
- **THEN** its commits, lanes, branch/merge topology, refs, and tags correspond to the output of `git log --graph --all` for that repository

#### Scenario: Decorations are shown

- **WHEN** a commit is a branch head, a tag target, or the current HEAD
- **THEN** the rail renders the corresponding ref/tag/HEAD decoration on that commit's row

#### Scenario: Hover reveals commit metadata

- **WHEN** the user hovers a commit row
- **THEN** the author, full date, and abbreviated hash are surfaced

#### Scenario: Commits are grouped under day separators

- **WHEN** two consecutive commit rows have author dates on different calendar days in the viewer's local time zone
- **THEN** a labelled day-separator row is rendered between them identifying the newer day
- **AND** the first commit of the newest day also has a day-separator above it

#### Scenario: Current and prior days read Today and Yesterday

- **WHEN** a day-separator marks the viewer's current calendar day in their local time zone
- **THEN** the separator is labelled `Today`
- **AND** a separator for the immediately preceding calendar day is labelled `Yesterday`

#### Scenario: Recent days within the week read as weekday names

- **WHEN** a day-separator marks a calendar day two to six days before the viewer's current day
- **THEN** the separator is labelled with that day's weekday name (e.g. `Wednesday`) and no absolute date

#### Scenario: Older days keep an absolute date

- **WHEN** a day-separator marks a calendar day seven or more days before the viewer's current day
- **THEN** the separator is labelled with the existing compact absolute date (e.g. `Mon, May 25`)

#### Scenario: Lanes pass through a day separator

- **WHEN** a lane is alive across a day boundary (a branch with commits on both days)
- **THEN** that lane's edge is drawn continuously through the separator band without a break
- **AND** the commit nodes remain aligned with their subject rows

#### Scenario: Grouping preserves faithful topology

- **WHEN** day separators are inserted
- **THEN** the commits' order, lane assignment, branch/merge edges, and decorations are identical to the ungrouped graph

#### Scenario: No OpenSpec semantics in the graph

- **WHEN** the rail renders commits that carry an `OpenSpec-Id` trailer or that archived a change
- **THEN** those commits are rendered identically to any other commit, with no change-based tint, grouping, or marker

### Requirement: Lane Layout and Overflow

Commit lanes SHALL be assigned by a deterministic layout that reclaims a lane as soon as its branch merges, so that the visible lane count at any vertical position reflects only the branches concurrently alive there rather than the total branch count. When the concurrently-alive lane count exceeds the rail's current width, the graph region SHALL scroll horizontally without scrolling the commit subject out of view, and the rail SHALL remain resizable so the user can widen it to view more lanes at once.

#### Scenario: Compaction keeps the common case narrow

- **WHEN** a repository has many branches that were created and merged over time
- **THEN** rows away from active merge points render with only the lanes alive at that point, not one lane per branch in the repository

#### Scenario: Overflow scrolls horizontally without losing the subject

- **WHEN** the concurrently-alive lanes at some row exceed the rail's width
- **THEN** the graph region for that row can be scrolled horizontally to reveal the additional lanes
- **AND** the commit subject remains visible

### Requirement: Commit Selection Drives the Detail Pane

Selecting a commit in the rail SHALL render that commit's detail in the center detail pane. The tree and the rail SHALL both drive the detail pane, and the pane SHALL render whichever was selected most recently. The tree and the rail SHALL each retain their own selection highlight independently. Returning to an artifact view SHALL require only selecting an artifact in the tree — no separate "back" action.

Day-separator rows SHALL be non-interactive: they SHALL NOT be selectable, SHALL NOT carry a selection highlight, and SHALL NOT become a detail-pane render target.

#### Scenario: Clicking a commit shows its detail

- **WHEN** the user clicks a commit in the rail
- **THEN** the detail pane renders that commit's detail view

#### Scenario: Selecting a tree artifact restores the markdown

- **WHEN** the detail pane is showing a commit's detail
- **AND** the user selects an artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Tree and rail keep independent highlights

- **WHEN** the user has a tree artifact highlighted and then clicks a commit in the rail
- **THEN** the rail's commit is highlighted in the rail
- **AND** the tree's previously selected node remains highlighted in the tree

#### Scenario: Day separators are not selectable

- **WHEN** the user clicks a day-separator row in the rail
- **THEN** no commit is selected and the detail pane is unchanged

### Requirement: Commit Detail View

The commit-detail view rendered in the center pane SHALL show the commit's metadata (abbreviated and full hash, author, date, and full message), the list of files the commit changed with each file's status and per-file added/removed line counts, and the textual diff of the change. The metadata SHALL include the commit's git trailers — the `Key: value` lines of the message's last paragraph as recognized by git's own trailer parser — rendered as a list of key/value pairs in git's emitted order, with every value shown when a key appears more than once. Trailers SHALL be presented as neutral commit metadata: the `OpenSpec-Id` trailer SHALL receive no styling, link, or marker that distinguishes it from any other trailer, and a commit whose message carries no trailers SHALL render no trailer section. A breadcrumb SHALL indicate the commit context and that selecting an artifact returns to the artifact view.

**Rendering.** The changed files and their diff SHALL be rendered through the `diff-view` capability, in the layout the reader chose there — unified or side by side — as that capability puts the choice into effect. The commit-detail view SHALL render its own header (the metadata, trailers and breadcrumb above, and a merge's first-parent label) and SHALL NOT render or parse a diff of its own. It SHALL name the diff's two sides for the diff view: the old side by the first parent's abbreviated hash, or `empty tree` for a root commit, and the new side by the commit's own abbreviated hash. The layout SHALL NOT reach the application service: which files arrive with their hunks, what crosses IPC, and how a file loads on request SHALL be the same in either layout, and switching between the layouts SHALL read nothing again.

**Reading the commit.** Opening a commit's detail SHALL cost no more than a fixed number of `git` processes and IPC calls, whatever the number $$N$$ of files the commit changed. One `git` invocation SHALL list every changed file with its status, its old and new paths, its old and new modes, and its added/removed counts, and one further invocation, read as a stream, SHALL supply the patch text. Which files arrive with their hunks SHALL be decided by the budgets of the `diff-view` capability: its line budget from the file list, before any patch is read, and its byte limits as the patch streams, which only ever withhold more. Every other file with a patch SHALL arrive withheld, with its counts and without its hunks, and a binary file SHALL be listed without a diff. The application SHALL stop reading the stream after the last file that arrives with its hunks, and SHALL abandon it once 8 MiB of patch text has been read in all, withholding every patched file not yet reached; a `git` process the application stops deliberately SHALL count as a successful read, not as a failure. Every patch line and every file-list record SHALL be decoded on its own, with bytes that are not valid UTF-8 replaced, so a file in another encoding shows replacement characters in its own lines or path and no other file is affected; paths SHALL be read verbatim from the file list, never C-quoted.

**A truthful file list.**

- Renames SHALL be detected at git's default similarity, the heuristic `git show` and `git log -M` apply: a renamed file is one file, shown as its old path → its new path, with its hunks when its content also changed — never a deletion plus an addition.
- A change of file type, between a regular file, a symbolic link and a submodule, SHALL be one type-changed file, never a deletion plus an addition.
- A root commit SHALL be diffed against the empty tree, so it lists every file it added rather than reporting that it changed no files.
- A merge commit SHALL be diffed against its first parent as a two-tree diff, never as a combined diff, so it lists every file that differs from that parent and not only the files the merge resolved by hand. The view SHALL label that diff `Changes against first parent` followed by the parent's abbreviated hash. The application service SHALL read the first parent as a hexadecimal object id rather than accept it from the caller, and SHALL pass it to `git` after the end-of-options marker, as it does every commit reference (see the *Commit References Are Injection-Safe Arguments* requirement).

The diff SHALL be computed by git's plumbing, which honours git's core diff settings, `diff.renameLimit` among them, but not its display settings: hunks follow git's default algorithm and carry three lines of context even where the user has set `diff.algorithm` or `diff.context`.

**Loading a withheld file.** A withheld file SHALL load on request, alone, from the same commit and against the same base as the rest of its diff — the first parent for a merge, the empty tree for a root commit. The read SHALL pass the file's path, and for a renamed file its old path too, as literal pathspecs, so a path such as `pages/[id].tsx` reads only that file and a renamed file loads as a rename. A file whose diff text exceeds 8 MiB SHALL load as too large to preview, reading `too large to preview`, with no hunks.

**Commands.** `get_commit_detail` SHALL return the commit's files under the budgets, and `get_commit_diff` the one file read on request, with its hunks or as too large to preview, both in the diff model the `diff-view` capability defines. Both SHALL keep their names and existing arguments; `get_commit_diff` SHALL additionally accept the file's old path as an optional argument. Both SHALL be served on the desktop command surface and the web command endpoint alike, and both remain commit-reading operations under the *Commit References Are Injection-Safe Arguments* and *Commit Reading Is Restricted to Registered Repositories* requirements.

#### Scenario: Detail view lists changed files and diff

- **WHEN** the commit-detail view renders for a commit
- **THEN** it shows the commit's metadata, the changed-files list with each file's status and added/removed counts, and the diff
- **AND** the files and their diff are rendered through the `diff-view` capability, in the layout the reader chose

#### Scenario: Commit trailers are listed

- **WHEN** the commit-detail view renders for a commit whose message carries git trailers (e.g. `OpenSpec-Id` and `Co-Authored-By`)
- **THEN** each trailer is shown as a key/value pair in git's emitted order

#### Scenario: Repeated trailer keys are all shown

- **WHEN** a commit carries the same trailer key more than once (e.g. two `Co-Authored-By` lines)
- **THEN** every occurrence is listed and not collapsed to a single entry

#### Scenario: Body prose is not shown as a trailer

- **WHEN** a commit's message has a multi-paragraph body and only its last paragraph contains trailers
- **THEN** only the recognized trailers are listed and the body prose is not mistaken for a trailer

#### Scenario: OpenSpec-Id is rendered as a neutral trailer

- **WHEN** a commit carries an `OpenSpec-Id` trailer
- **THEN** it is displayed identically to any other trailer, with no link, tint, or marker distinguishing it

#### Scenario: A commit with no trailers shows no trailer section

- **WHEN** the commit-detail view renders for a commit whose message carries no trailers
- **THEN** no trailer list or empty trailer affordance is shown

#### Scenario: Breadcrumb indicates how to return

- **WHEN** the commit-detail view is shown
- **THEN** a breadcrumb identifies the commit and indicates that selecting an artifact returns to the artifact view

#### Scenario: A commit is read in a fixed number of git processes

- **WHEN** the commit-detail view renders for an ordinary commit that changed one text file, and then for an ordinary commit that changed two hundred text files
- **THEN** the two-hundred-file commit is read with no more `git` processes and no more IPC calls than the one-file commit
- **AND** no `git` process or IPC call is made per file
- **AND** where the budgets withhold the files after the last one that arrives with its hunks, reading stops there and the detail renders without an error

#### Scenario: A renamed file is one file

- **WHEN** the commit-detail view renders for a commit that renamed the forty-line file `src/old.ts` to `src/new.ts` and changed two of its lines
- **THEN** the changed-files list shows one renamed file, `src/old.ts → src/new.ts`
- **AND** its hunks show the two changed lines
- **AND** neither a deletion of `src/old.ts` nor an addition of `src/new.ts` is listed

#### Scenario: A type change is one file

- **WHEN** the commit-detail view renders for a commit that replaced the regular file `config` with a symbolic link of the same name
- **THEN** the changed-files list shows one type-changed file, `config`, with its old and new modes
- **AND** neither a deletion nor an addition of `config` is listed

#### Scenario: A root commit lists the files it added

- **WHEN** the commit-detail view renders for a repository's root commit, which added the two short files `README.md` and `src/main.rs`
- **THEN** both files are listed as added, with every line of each shown as an added line
- **AND** the view does not report that the commit changed no files
- **AND** while side by side is in effect, the old side is named `empty tree`

#### Scenario: A merge shows its changes against its first parent

- **WHEN** the commit-detail view renders for a merge commit whose tree differs from its first parent's in fifty files, two of which the merge resolved by hand
- **THEN** all fifty files are listed as changes against the first parent, and not only the two resolved by hand
- **AND** the diff is labelled `Changes against first parent` followed by the first parent's abbreviated hash
- **AND** a file of the merge loaded on request is diffed against that same parent

#### Scenario: A withheld file loads on request

- **WHEN** the budgets withheld a commit's renamed file `lib/a.rs → lib/b.rs`
- **AND** the reader activates that file's `Load diff` control
- **THEN** that file alone is read from the commit, by its new path and its old path
- **AND** it renders as one renamed file with its hunks, in the layout in effect
- **AND** no other file of the commit is read again

#### Scenario: A path with pattern characters loads only itself

- **WHEN** a commit changed both `pages/[id].tsx` and `pages/i.tsx`, and the budgets withheld `pages/[id].tsx`
- **AND** the reader loads `pages/[id].tsx`
- **THEN** only the changes to `pages/[id].tsx` are read and shown
- **AND** `pages/i.tsx`, which `pages/[id].tsx` would match as a pattern, is not read with it

#### Scenario: A file past the per-file ceiling is too large to preview

- **WHEN** the reader loads a withheld file whose diff text exceeds 8 MiB
- **THEN** the file reads `too large to preview` and shows no hunks
- **AND** the rest of the commit's detail is unchanged

#### Scenario: A file in another encoding does not blank the commit

- **WHEN** the commit-detail view renders for a commit that changed `docs/café.md`, then a file with one line in Latin-1, then another UTF-8 file
- **THEN** `docs/café.md` is listed under that name, not as a C-quoted escape
- **AND** the Latin-1 line shows replacement characters for its undecodable bytes, in its own file only
- **AND** both UTF-8 files show their hunks intact

#### Scenario: Another commit opens in the chosen layout

- **WHEN** the reader switches a commit's detail to side by side, in a pane wide enough for two columns
- **AND** then selects another commit in the rail
- **THEN** that commit's detail opens side by side
- **AND** the old side is named by its first parent's abbreviated hash and the new side by its own abbreviated hash

#### Scenario: Switching layout reads nothing again

- **WHEN** the reader has loaded one withheld file in a commit's detail
- **AND** switches the detail between unified and side by side
- **THEN** no `git` process runs and no IPC call is made
- **AND** the loaded file keeps its hunks, and every other file keeps its hunks or stays withheld exactly as before the switch

### Requirement: Live Graph Updates

The rail SHALL reflect changes to the repository's refs within the watcher's debounce window without user action. New commits, branch creation/deletion, branch-head movement, tag changes, and HEAD movement SHALL cause the rail to refresh.

#### Scenario: New commit appears

- **WHEN** a new commit is created in the repository on disk
- **THEN** the rail renders the new commit within the debounce window

#### Scenario: Branch movement is reflected

- **WHEN** a branch head moves, is created, or is deleted on disk
- **THEN** the rail's decorations and topology update within the debounce window

### Requirement: Read-Only Operation

The rail and commit-detail view SHALL expose no operation that mutates the repository. Checkout, branch create/delete, merge, rebase, cherry-pick, reset, and any other history- or working-tree-mutating git operation SHALL NOT be reachable from the rail.

#### Scenario: No mutating actions are offered

- **WHEN** the user interacts with the rail or the commit-detail view
- **THEN** no action that checks out, moves, rewrites, or deletes commits, branches, or working-tree state is available

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or the selected workspace is not inside a git repository, the rail SHALL render its empty placeholder state and the rest of the application SHALL continue to function, consistent with the degrade-to-empty behaviour of the existing git integration.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the rail renders empty
- **AND** the tree and detail panes continue to function

### Requirement: Commit References Are Injection-Safe Arguments

Any commit reference supplied to a commit-reading operation (the commit-detail file list and the per-file diff) SHALL be treated as untrusted data rather than as a command-line argument to the underlying git invocation, such that no reference value can cause git to write, delete, or otherwise mutate any file or the working tree, nor invoke an external program — it can only cause git to read the named commit. To achieve this the application SHALL both (a) reject a reference that is not a plausible git object id (a hexadecimal string of 4 to 64 characters) before it is used, and (b) pass the reference to git in a position that git cannot interpret as an option (after an end-of-options marker). Guarantee (b) SHALL hold at the point where the git command is constructed, so that it protects every frontend and transport that can reach these operations — the desktop command surface and the optional web command endpoint alike — independent of any per-frontend validation. This strengthens the *Read-Only Operation* requirement: that one ensures the UI offers no mutating action; this one ensures the argument-passing path cannot be coerced into a mutating action either.

#### Scenario: A reference shaped like an option cannot write a file

- **WHEN** a commit-reading operation is invoked with a reference value that resembles a git option that would write to a path (for example, a value requesting diff output be written to a file)
- **THEN** no file is created, truncated, or modified as a result
- **AND** the operation returns an error or an empty result rather than executing the option

#### Scenario: A malformed reference is rejected

- **WHEN** a commit-reading operation is invoked with a reference that is not a hexadecimal object id (for example, an empty string, a branch name, or a leading-dash string)
- **THEN** the operation is refused with an error indicating an invalid reference
- **AND** git is not asked to act on that value

#### Scenario: A legitimate commit hash still resolves

- **WHEN** a commit-reading operation is invoked with a valid commit hash from the graph
- **THEN** the operation returns that commit's file list or diff as before

#### Scenario: The guarantee holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same reference-safety guarantees apply, because they are enforced where the git command is constructed rather than in a single frontend

### Requirement: Commit Reading Is Restricted to Registered Repositories

A commit-reading operation (the graph, the commit-detail file list, and the per-file diff) SHALL act only on a repository that belongs to a registered workspace, and SHALL refuse a caller-supplied repository identifier that is not the git repository of any registered workspace rather than reading it. Authorization SHALL be decided by comparing the canonical form of the supplied identifier against the canonical git directories of the registered workspaces, using the same path-canonicalization the registry uses to key its entries, so that an equivalent but differently spelled path is neither wrongly refused nor able to evade the check. This authorization SHALL be enforced at the shared application boundary so that it holds identically for every frontend and transport — the desktop command surface and the optional web command endpoint alike — and not only for whichever transport happens to route through that boundary today. This complements *Graceful Degradation Without Git*: a registered-but-unreadable repository still degrades to an empty rail, whereas an unregistered repository is refused as unauthorized.

#### Scenario: An unregistered repository is refused

- **WHEN** a commit-reading operation is invoked with a repository identifier that is not the git repository of any registered workspace
- **THEN** the operation is refused and no commit history, file list, or diff of that repository is returned
- **AND** no `git` command is run against that repository

#### Scenario: A registered repository is read normally

- **WHEN** a commit-reading operation is invoked with the repository of a registered workspace
- **THEN** the operation returns that repository's graph, file list, or diff as before

#### Scenario: The restriction holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registration check applies, because it is enforced at the shared application boundary both transports use

#### Scenario: Path spelling does not defeat or trip the check

- **WHEN** a registered repository is identified by an equivalent but differently spelled path (for example with a trailing separator, a `..` segment, a symlink, or a platform verbatim prefix)
- **THEN** it is recognized as the same registered repository and read normally

### Requirement: Commit History Can Be Turned Off

The application SHALL offer a single, application-wide *Commit history* switch that turns the commit graph on or off. The switch SHALL be stored in the application settings and SHALL survive a restart. It SHALL default to on. A settings file that does not record the switch, including every file written before it existed, SHALL load with it on, so that no existing installation loses its graph.

The switch SHALL be presented in Settings in both the desktop application and the served web UI. It is not a desktop-only setting within the meaning of the *Desktop-Only Settings Are Hidden in the Web UI* requirement in the `web-ui` capability. Changing it SHALL take effect in main windows that are already open, without reopening or reloading them. The change SHALL be announced as a dedicated event rather than as a variant of the cache-event stream, so that no existing consumer of that stream in any frontend gains a case it must ignore. The event SHALL be delivered on both the desktop and the browser event transports.

While the switch is off, the commit graph SHALL NOT be rendered, and the application SHALL NOT perform commit-graph reads (git subprocess work) on its behalf, whatever the tree selection. This is the guarantee the *Commit-Graph Rail Pane* requirement gives a hidden rail. The commit graph is then not an occupant of the rail (see the *Rail Exists Only While Occupied* requirement in the `spec-browser` capability). Turning the switch on SHALL fetch and render the graph of the repository that owns the current tree selection at that moment.

A main window SHALL reflect the switch from its first frame. It SHALL NOT render the commit graph, or a rail that the graph alone would occupy, and then remove it once the stored value has been retrieved. The application settings SHALL remain the authoritative store. Because retrieving them is asynchronous, the switch SHALL additionally be mirrored in a store the frontend can read synchronously at startup, written on every change. The mirror SHALL be treated as a first-paint hint and never as the source of truth: once the authoritative value is available it SHALL be reconciled against the mirror, so a value changed by another instance of the application corrects itself rather than persisting. When no mirror is available (a first run, or a cleared store) a main window SHALL start with the switch on.

Turning the switch on from Settings SHALL show the rail on the surface where it was turned on, even if the rail had been hidden there (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability). Every other surface SHALL keep its own rail visibility.

The terminal frontend SHALL NOT consult the switch. Its commit graph is a screen the user opens, not a pane sharing the window with the reading surface, and it renders unconditionally (see the *Progress Surfaces in the Terminal* requirement in the `terminal-ui` capability).

#### Scenario: History is on by default

- **WHEN** SpecForge starts with a settings file that does not record the switch
- **THEN** the switch is on
- **AND** the rail renders the commit graph as it did before the switch existed

#### Scenario: Turning history off removes the graph

- **WHEN** the commit graph is rendered in the rail
- **AND** the user turns the switch off in Settings
- **THEN** the commit graph is no longer rendered in the main window

#### Scenario: No graph reads while history is off

- **WHEN** the switch is off
- **AND** the user selects nodes belonging to different repositories
- **THEN** no commit-graph read is performed for any of those selections

#### Scenario: The switch survives a restart

- **WHEN** the user turns the switch off and restarts the application
- **THEN** the switch is still off
- **AND** the commit graph is not rendered

#### Scenario: A window opened with history off never paints the graph

- **WHEN** the switch is off and no pull-request panel occupies the rail
- **AND** a main window is opened on a surface that last saw the switch off
- **THEN** no frame of that window renders the commit graph or the rail

#### Scenario: A stale mirror is corrected

- **WHEN** the switch was changed by another instance of the application since this surface last ran
- **THEN** the authoritative value takes effect once read
- **AND** the mirror is updated to match it

#### Scenario: Open windows adopt a change

- **WHEN** the switch is changed while the browser skin is connected
- **THEN** the browser skin's main window adopts the new value without a reload

#### Scenario: Turning history on shows a hidden rail

- **WHEN** the rail is hidden on a surface
- **AND** the switch is turned off and later turned on again in that surface's Settings
- **THEN** the rail is shown, rendering the commit graph of the repository that owns the current tree selection
- **AND** the rail's visibility on every other surface is unchanged

#### Scenario: The switch is offered in the browser

- **WHEN** the Settings view renders in the served web UI
- **THEN** the Commit history switch is shown
- **AND** changing it takes effect as it does in the desktop application

#### Scenario: The terminal ignores the switch

- **WHEN** the switch is off
- **THEN** the terminal frontend's History screen still renders the commit graph

