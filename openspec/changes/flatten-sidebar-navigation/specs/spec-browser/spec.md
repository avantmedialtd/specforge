## MODIFIED Requirements

### Requirement: Workspace Tree Hierarchy

The tree pane SHALL display tracked workspaces grouped by repository where applicable, as a tree of exactly **two levels**: top-level rows and change rows. For each git repository with at least one tracked workspace, the tree SHALL render a top-level Repo group node containing one row per active logical change. For each non-git workspace, the tree SHALL render a top-level workspace node containing that workspace's changes directly.

A logical change groups every `ChangeInstance` in one repository that shares the same **logical change identifier**: the change's directory name for an active instance, and that directory name with at most one leading `YYYY-MM-DD-` prefix removed for an archived one. The two forms are grouped together deliberately, so that an active instance and an archived instance of one change are comparable — which is what the *Per-Instance Divergence Label* requirement needs in order to report `[stale]` at all.

Grouping them, however, SHALL NOT put an archived instance in front of a consumer that renders active changes. Within a logical change, instances SHALL be partitioned by whether they are archived in their worktree, and only the non-archived partition counts as rendered. An archived instance is summarised from a directory listing rather than parsed — it carries no title, no task counts and no artifacts, because the archive is deliberately never read on the aggregation path (see *On-Demand, Off-Hot-Path Loading* in the `archive-browser` capability).

Inside a Repo group, each logical change SHALL be rendered as exactly **one change row**, whatever its rendered instance count. A change with one rendered instance renders as a two-line row naming its branch; a change with two or more renders the same single row carrying an instance-count chip in the branch chip's place (see *Two-Line Sole-Change-Row Layout*), and the instance to read is chosen in the change header (see *Instance Switcher in the Change Header*). An archived instance SHALL NOT contribute to that count.

A change row is a **leaf**. It SHALL expose no artifact, capability, section or task rows beneath it and no disclosure affordance. Activating a change row SHALL navigate to the change's **default artifact** of its **default instance**: the first artifact present on disk in the fixed order Proposal, Design, Tasks, then the first capability spec in listing order; and the main worktree's instance when it hosts the change, otherwise the first rendered instance in aggregation order. The change's other artifacts are reached from the change header (see *Artifact Tab Strip in the Change Header*). A change with no artifact present SHALL still be selectable, and the detail pane SHALL show its empty state.

The tree SHALL render **active changes only**. Archived logical changes SHALL NOT appear anywhere in the tree — neither in an Active section nor in a separate Archive section. Archived changes are browsed exclusively in the dedicated Archive view (see the *Archive View* requirement in the `archive-browser` capability).

A top-level row is a **disclosure row**, open by default. A top-level row the user closes SHALL stay closed for the rest of the session and SHALL NOT be persisted: no disclosure state of any kind survives a restart, and the tree performs no settings write on a toggle. A top-level row (a Repo group node or a non-git workspace node) with no active changes SHALL be rendered as a leaf row with no disclosure chevron and no toggle affordance. The row SHALL continue to display its count badge with the value `0` and SHALL remain selectable, where "selectable" means a click on the row updates the tree's selected-node state — applying the same visual selection treatment a non-empty top-level row receives — and opens the workspace file browser for the row's workspace in the detail pane (see the `workspace-file-browser` capability). No placeholder child row SHALL be rendered beneath an empty top-level row. A top-level row whose only changes are archived SHALL therefore render as a leaf with a `0` active count, the same as a row with no changes at all.

#### Scenario: Git repo with multiple worktrees shown as one Repo group

- **WHEN** a repository has three tracked worktrees, two of which contain a change with the same directory name
- **THEN** the tree shows one top-level Repo group for that repository
- **AND** the two-instance change appears as exactly one change row carrying an instance-count chip
- **AND** any single-instance change appears as one change row naming its branch

#### Scenario: An archived instance is not rendered beside its active twin

- **WHEN** a change is archived in one worktree of a repository and still active in another
- **THEN** the tree renders exactly one row for it, counting the active instance only
- **AND** that row carries no instance-count chip
- **AND** no unlabelled or artifact-less row is rendered for the archived copy

#### Scenario: Non-git workspace shown as a standalone top-level node

- **WHEN** a tracked workspace is not inside a git repository
- **THEN** the workspace is rendered as a top-level node (not a Repo group)
- **AND** the workspace's changes are rendered directly underneath without instance aggregation

#### Scenario: Activating a change row opens its default artifact

- **WHEN** the user activates the row of a change whose `proposal.md` is absent and whose `design.md` is present
- **THEN** the detail pane renders that change's design
- **AND** the change header's tab strip shows Design as the active tab

#### Scenario: Activating a multi-instance change row opens the default instance

- **WHEN** the user activates the row of a change hosted in the repository's main worktree and in one feature worktree
- **THEN** the detail pane renders the main worktree's copy of the default artifact
- **AND** the change header's instance switcher marks the main worktree's instance as selected

#### Scenario: A change row exposes nothing beneath it

- **WHEN** a change row is rendered
- **THEN** it has no disclosure chevron
- **AND** no artifact, capability, section or task row is rendered beneath it

#### Scenario: Archived logical changes are not shown in the tree

- **WHEN** every instance of a logical change is under `openspec/changes/archive/` in its worktree
- **THEN** the logical change is not shown anywhere in the tree
- **AND** it is browsable in the Archive view instead

#### Scenario: Empty top-level row renders as a leaf

- **WHEN** a top-level Repo group node or non-git workspace node has zero active changes
- **THEN** the row renders as a leaf with no disclosure chevron and no toggle affordance
- **AND** the row's count badge displays `0`
- **AND** clicking the row updates the tree's selected-node state and applies the same visual selection treatment a non-empty top-level row receives
- **AND** the detail pane shows the workspace file browser for the row's workspace (see the `workspace-file-browser` capability)
- **AND** no placeholder child row (such as "no active changes") is rendered beneath the row

#### Scenario: Top-level row with only archived changes renders as a leaf

- **WHEN** a top-level row has zero active changes but one or more archived changes
- **THEN** the row renders as a leaf with a `0` active count
- **AND** no archived change is rendered beneath it in the tree

#### Scenario: Empty top-level row becomes non-empty when a change is added

- **WHEN** a top-level row was rendering as a leaf because it had zero active changes
- **AND** the watcher reports a new active change for that workspace
- **THEN** the row re-renders as an open disclosure parent
- **AND** the count badge advances from `0` to the new count

#### Scenario: Top-level disclosure does not survive a restart

- **WHEN** the user closes a top-level row and restarts the application
- **THEN** the row renders open
- **AND** no settings write was performed when it was closed

### Requirement: Workspace Tree Keyboard Navigation

The workspace tree SHALL be fully operable from the keyboard as a WAI-ARIA tree with a roving tabindex over its two levels: the tree occupies exactly one position in the window's Tab order, and within it a single current row carries focus, movable with the keyboard. Keyboard activation SHALL reuse the same selection contract as pointer clicks — a change row activated by keyboard navigates exactly as a click on it does, and a top-level row, whose click is disclosure-only, remains disclosure-only. Switching between a change's artifacts is not a tree operation: it is done in the change header's tab strip, whose keyboard model *Artifact Tab Strip in the Change Header* defines.

#### Scenario: Tree is a single Tab stop with a roving current row

- **WHEN** the user presses Tab from the control preceding the tree (or Shift+Tab from the control following it)
- **THEN** focus lands on the tree's current row — the last row focused in this session, or the first visible row if none — rather than entering every row in sequence
- **AND** pressing Tab again moves focus out of the tree to the next control in the window's Tab order

#### Scenario: Arrow keys traverse visible rows

- **WHEN** the tree has focus and the user presses ArrowDown or ArrowUp
- **THEN** focus moves to the next or previous visible row in rendered order, crossing workspace boundaries, without wrapping at either end
- **AND** the newly focused row scrolls into view if it is outside the sidebar's viewport

#### Scenario: Home and End jump to the extremes

- **WHEN** the tree has focus and the user presses Home or End
- **THEN** focus moves to the first or last visible row of the tree

#### Scenario: ArrowRight and ArrowLeft drive disclosure and parent jumps

- **WHEN** the user presses ArrowRight on a closed top-level row
- **THEN** the row opens for the session, identically to a chevron click
- **WHEN** the user presses ArrowRight on an open top-level row
- **THEN** focus moves to its first change row
- **WHEN** the user presses ArrowLeft on an open top-level row
- **THEN** the row closes
- **WHEN** the user presses ArrowLeft on a change row, or on a closed or leaf top-level row
- **THEN** focus moves to the row's top-level row, or does not move when the row is already top-level

#### Scenario: Enter and Space activate the current row

- **WHEN** the user presses Enter or Space on a change row
- **THEN** the row is selected and the detail pane renders exactly what a pointer click on that row would render — the change's default artifact of its default instance
- **WHEN** the user presses Enter or Space on a top-level row with changes
- **THEN** the row's disclosure toggles, identically to a chevron click
- **WHEN** the user presses Enter or Space on an empty top-level row
- **THEN** the row is selected and the detail pane shows the workspace file browser, as a click would

#### Scenario: Debounced follow-focus opens content without per-keystroke reads

- **WHEN** keyboard focus comes to rest on a change row and remains there for a short settle delay (approximately 150 ms)
- **THEN** the detail pane renders that change's default artifact as if the row had been activated
- **WHEN** focus passes over change rows more quickly than the settle delay (for example while an arrow key is held down)
- **THEN** no intermediate change's artifact is loaded or rendered
- **WHEN** keyboard focus rests on a top-level row
- **THEN** the detail pane does not change

#### Scenario: First-letter typeahead

- **WHEN** the tree has focus and the user types a printable character
- **THEN** focus moves to the next visible row after the current one whose label starts with that character, comparing case-insensitively and wrapping past the end of the tree
- **AND** if no visible row label starts with that character, focus does not move

#### Scenario: Tree rows expose ARIA tree semantics

- **WHEN** the tree is rendered
- **THEN** the container exposes `role="tree"`, every row exposes `role="treeitem"` with `aria-level` of `1` for a top-level row and `2` for a change row, top-level rows with changes expose `aria-expanded` reflecting their disclosure state, the selected row exposes `aria-selected="true"`, and each top-level row's change rows are wrapped in a `role="group"` container
- **AND** no row exposes `aria-disabled`: every rendered row responds to activation

#### Scenario: Focus survives the focused row disappearing

- **WHEN** a tree refresh (for example a filesystem cache event) removes the change row that currently holds keyboard focus
- **THEN** focus falls back to that change's top-level row, rather than being lost to the document body

#### Scenario: Keyboard focus movement does not re-render the whole tree

- **WHEN** the user moves keyboard focus between rows
- **THEN** only the rows whose visual state changed re-render; unaffected subtrees are not re-rendered

### Requirement: Two-Line Sole-Change-Row Layout

A change row — the **sole row for its change**, since the tree renders exactly one row per change (see *Workspace Tree Hierarchy*) — SHALL render across two stacked lines within a single selectable row. Exactly two row types are change rows:

- a **logical-change row** — one row per git logical change, whatever its rendered instance count; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Repo-group and workspace header rows are excluded and SHALL remain single-line. No other row type exists in the tree.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than the tree's meta text so it reads as the row's heading, and SHALL own the full row width — no trailing branch chip or status meta shares the line — except for the favorite toggle's reserved trailing slot (see *Change-Row Favorite Toggle*); it SHALL ellipsize against that slot when it exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. For a **logical-change row with two or more rendered instances** the leading edge SHALL instead show an **instance-count chip** (`2 worktrees`) in the same outlined-chip treatment, untinted, and SHALL name no branch: the instances' branches are named in the change header's instance switcher (see *Instance Switcher in the Change Header*). A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and the *Task Progress Meter* requirement in `visual-identity`), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **multi-instance logical-change row** the status elements are those of the change's **default instance** (see *Workspace Tree Hierarchy*), and no divergence label is shown on the row: divergence is a per-instance property and is shown per instance in the change header's switcher. For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. A change row has no disclosure chevron: it is a leaf. The favorite toggle (see *Change-Row Favorite Toggle*) is the row's only nested control; activating it SHALL NOT select the change.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than line 2
- **AND** the label is not truncated by any branch or status element on the same line; only the favorite toggle's reserved trailing slot bounds it
- **AND** the row's hover tooltip carries the logical change name

#### Scenario: Git singleton without an extractable title falls back to the change name

- **WHEN** a git logical change has exactly one instance and its `proposal.md` is missing or yields no title
- **THEN** line 1 shows the logical change name, exactly as before

#### Scenario: Branch appears on the detail line as a workspace-tinted chip

- **WHEN** a git singleton instance's worktree is on a named branch
- **THEN** line 2 shows the branch name as an outlined chip on its leading edge, with chip text and border tinted to the owning workspace's palette colour (a contrast-safe shade)
- **AND** line 2 shows the task-progress meter (or completion ✓) and relative modification time on its trailing edge

#### Scenario: Detached-HEAD singleton shows the folder basename only

- **WHEN** a git singleton instance's worktree is not on a named branch
- **THEN** line 2's worktree-identity segment shows the worktree folder basename alone, with no branch name

#### Scenario: Flat-workspace change row uses two lines with a meta-only detail line

- **WHEN** a change is rendered as a flat-workspace change row under a non-git workspace node
- **THEN** line 1 shows the change's title (or its change-id when no title is present)
- **AND** line 2 shows the change's `changeId` on its leading edge and the completion ✓ (when complete) on its trailing edge, with no branch, worktree folder, progress meter, modification time, or divergence label

#### Scenario: Multi-instance change renders one row with an instance count

- **WHEN** a logical change has two or more rendered instances
- **THEN** it renders as exactly one two-line row
- **AND** line 2's leading edge shows an untinted instance-count chip naming the number of worktrees and no branch
- **AND** line 2's trailing edge shows the default instance's progress meter (or completion ✓) and relative modification time, and no divergence label

#### Scenario: Completed sole change row shows its completion glyph on the detail line

- **WHEN** a sole change row's change has at least one task and every task is complete
- **THEN** line 2's trailing edge shows the completion ✓ in place of the progress meter

#### Scenario: Selection and hover span both lines of a sole change row

- **WHEN** a sole change row is selected, or the pointer hovers over either of its two lines
- **THEN** the selection bar and tint (or the hover wash) cover both lines as one contiguous row
- **AND** a click anywhere on either line — outside the favorite toggle — selects the change and updates the detail pane

#### Scenario: Workspace-colour rail marks each change row

- **WHEN** a sole change row is rendered for a workspace that has a configured palette colour
- **AND** the row is not selected
- **THEN** the row's inline-start border (the selection-bar slot) is tinted to that workspace's palette colour
- **AND** every change row under the same workspace shares that colour, matching the workspace's top-level swatch

#### Scenario: Selection bar overrides the rail

- **WHEN** a sole change row that is showing its workspace-colour rail becomes selected
- **THEN** the inline-start border renders the 2px `--accent` selection bar instead of the workspace colour
- **AND** the workspace-colour rail reappears once the row is deselected

### Requirement: Per-Instance Divergence Label

For every `ChangeInstance` that is not on the repository's default branch, the application SHALL compute and display at most one divergence label by comparing the instance's change directory contents against the default-branch instance of the same logical change. The labels are:

- `[diverged]` — the change exists in both the default-branch instance and the non-default instance, but the file contents differ at the byte level.
- `[stale]` — the change is archived on the default branch (under `openspec/changes/archive/`) but is still active in the non-default instance.

If the change does not exist on the default branch at all, or if no default branch is known, or if the contents are identical, the instance SHALL display no divergence label.

The label is a property of an **instance**, and is displayed where instances are named: on the instance's entry in the change header's instance switcher (see *Instance Switcher in the Change Header*) when the change has several rendered instances, and on the change row's detail line when the change has exactly one (see *Two-Line Sole-Change-Row Layout*). A multi-instance change row itself SHALL show no divergence label, because the row names no instance.

Two instances SHALL be recognised as instances of the same logical change whether or not they are archived, and whether or not their archive directories carry a date prefix. An archived instance's directory is named `<YYYY-MM-DD>-<id>` in ordinary use and `<id>` in the legacy un-dated form; the logical change it belongs to is identified by `<id>` in both cases. Comparing an active instance against an archived one SHALL therefore key on the change's bare identifier, never on the archive directory's raw name — keying the two forms differently makes the `[stale]` label unreachable for every dated archive directory, which is the form that occurs in practice.

#### Scenario: Diverged content gets the diverged label

- **WHEN** an instance on a non-default branch has different content under `openspec/changes/<name>/` than the default-branch instance of the same logical change
- **THEN** the instance's switcher entry (or, for a singleton, its change row) displays the `[diverged]` label

#### Scenario: Stale-vs-archive gets the stale label

- **WHEN** the default-branch instance of a logical change is in `openspec/changes/archive/<name>/`
- **AND** a non-default instance of the same logical change is in `openspec/changes/<name>/` (still active)
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays the `[stale]` label

#### Scenario: Stale label fires against a dated archive directory

- **WHEN** the default-branch instance of a logical change `add-thing` is archived at `openspec/changes/archive/2026-09-05-add-thing/`
- **AND** a non-default instance is still active at `openspec/changes/add-thing/`
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays the `[stale]` label
- **AND** the date prefix on the archive directory does not prevent the two instances from being recognised as the same logical change

#### Scenario: Branch-only change gets no label

- **WHEN** a logical change has no instance on the default branch (it was created only on a feature branch)
- **THEN** every non-default instance displays no divergence label

#### Scenario: Identical content gets no label

- **WHEN** a non-default instance has byte-identical content to the default-branch instance of the same logical change
- **THEN** the non-default instance's switcher entry (or, for a singleton, its change row) displays no divergence label

#### Scenario: No default branch produces no labels

- **WHEN** the repository has no detected default branch
- **THEN** no instance of any logical change in that repository displays a divergence label

### Requirement: Change Identity Header in the Detail Pane

While the detail pane's target is an OpenSpec artifact, the pane SHALL render a **change header** above the artifact's markdown, naming the change the artifact belongs to and carrying the change's navigation. The header applies to the artifact target only: the commit detail view, the Dashboard, the workspace file browser, and the Settings view each carry their own header and SHALL be unaffected. The **Archive reader** renders this same header, in a read-only form, as the `archive-browser` capability's *Read-Only Artifact Navigation* requirement specifies; it carries no header of its own.

**Rows.** The header is composed of stacked rows in a fixed order: the **identity row** this requirement defines; then the **instance switcher** (see *Instance Switcher in the Change Header*), rendered only when the change has more than one rendered instance; then the **artifact tab strip** (see *Artifact Tab Strip in the Change Header*). In the read-only form a leading row precedes the identity row, carrying the control that returns to the archive listing together with the archived change's date and title. Every row lies inside the single sticky, measured element this requirement's *Persistence while reading*, *Clearance* and *Anchoring* clauses describe, so those clauses bind the whole header and a scroll anchor clears all of it.

**Content.** The header SHALL display the change's **directory name** — the `openspec/changes/<name>` folder name, which is the identifier a user hands to external tooling — rendered verbatim and in full, with no truncation, ellipsis, or transformation. It SHALL NOT substitute the change's `proposal.md` title, which the tree already shows (see *Two-Line Sole-Change-Row Layout*) and which is not the change's filesystem identity. Following the name, the header SHALL show the owning worktree's branch as an outlined chip (per *visual-identity → Outlined Chip Badges*). When the artifact belongs to a flat (non-git) workspace, or the worktree's branch is otherwise not known, no chip SHALL be rendered and the header SHALL show the name alone. An **archived** change SHALL render no chip: it has no live worktree, and the worktree path its artifact is read from routinely hosts other, active changes whose branch was never the archived change's.

**Branch chip colour.** The branch chip SHALL be **tinted to the owning workspace's palette colour** — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour — so the artifact under the header reads as belonging to the workspace it came from. The workspace whose colour applies is the one owning the worktree the artifact was read from, which is the same worktree whose branch the chip names; no other workspace's colour SHALL be substituted. When the owning workspace has no configured palette colour, the chip SHALL render in the neutral ink it renders in today, and SHALL NOT fall back to an arbitrary or derived colour.

The header's chip and the tree's chip naming the same branch of the same change SHALL render **identically** — the same tint, weight, and treatment (see the *Two-Line Sole-Change-Row Layout* requirement, which specifies the tree's chip). The two surfaces are visible simultaneously, so a single value SHALL NOT be presented two ways. This equivalence is a property of the rendered result and SHALL hold for every palette colour and for the untinted case, so that changing how one surface renders the chip cannot leave the other behind.

**Last changed.** The header SHALL report **when the artifact currently rendered last changed**, as an interval elapsed since that moment, expressed in relative terms (for example `just now`, `9m ago`, `12d ago`) rather than as an absolute clock time. The detail pane is already refreshed live (see *Reactive Updates from Filesystem*), so this value does not report whether the view is current; it reports how long the artifact has stood.

The label SHALL use the **same relative-time vocabulary** as every other surface in the application that presents an elapsed time — the tree's per-instance modification time (see *Multi-Instance Child Row* and *Two-Line Sole-Change-Row Layout*) and the Dashboard's relative archive time (see the `dashboard` capability). The header and the tree are visible simultaneously and routinely describe the same change, so one kind of value SHALL NOT be spelled two ways on one screen. This equivalence is a property of the rendered result and SHALL hold at every tier of the vocabulary, so that changing how one surface words an interval cannot leave the others behind.

The value SHALL be the modification time of **the artifact's own file** — not of the change's directory, and not of any sibling artifact. A write to `tasks.md` SHALL NOT be reported as a change to the `proposal.md` on screen, because the two are edited independently and reporting the directory's newest write would be wrong in exactly the case a reader is most likely to be watching.

Where **no** modification time is available for the artifact's file, the header SHALL render no label at all, and SHALL NOT substitute a default, derived, or epoch time in its place. The artifact itself SHALL still be displayed: an unreadable timestamp is not a failed read, and a reader SHALL NOT lose the document because the application could not date it. This mirrors the branch chip, which is likewise absent rather than defaulted when there is no branch to name.

The value SHALL be a filesystem modification time, and the header SHALL claim no more of it than that. Any operation that writes the file sets it, including a clone, a checkout, or a branch switch — so on a freshly cloned repository every artifact SHALL report having last changed at the time of that clone, regardless of when it was genuinely edited. This SHALL hold **uniformly for every artifact**, active and archived alike: no class of artifact SHALL substitute a different source for this value, because the property belongs to modification times in general and not to any one class, and an exception carved for one class would imply the others are trustworthy in a way they are not.

The label SHALL **advance without user action** for as long as the artifact remains on screen, so a reader who has not navigated away is never shown an interval that stopped counting when the pane was painted. It SHALL be updated at a cadence no finer than the smallest unit it displays. An elapsed interval that would be negative — a file whose modification time lies in the future, which clock skew, restored archives, and network filesystems all produce — SHALL be presented as the present moment rather than as a future time. Whatever advances the label SHALL NOT outlive the artifact it described.

The label SHALL occupy a **constant width**, sized to the widest text it can display, so that neither its advancing nor a change of unit alters the layout of the header. The change name shares a single flex row with the branch chip and yields to width pressure by re-wrapping mid-identifier; a label whose box changed size as it advanced would therefore re-lay-out the change name on a timer, unprompted. This is the same defect the *Confirmation* clause below forbids for a click-driven width change, arriving from a trigger the reader did not initiate at all.

**Copy on click.** The change name is a **control**. A single primary click on it SHALL place exactly that name on the clipboard. What is copied SHALL be the name alone — never the branch chip's text, never the last-changed label, and never any surrounding whitespace. The same click SHALL also select the name atomically, so the selection serves as immediate confirmation of exactly what was copied.

The application SHALL perform the clipboard write itself. (This supersedes the previous contract, under which the application performed no clipboard write and the user completed the copy with the platform's own gesture; that contract was adopted under constraints of the tree pane, which do not apply to the detail pane.) Where the asynchronous Clipboard API is not exposed — a non-loopback bind on a plain-HTTP origin is not a secure context, see the `web-ui` capability — the application SHALL still copy, using a synchronous copy over the selection it has just made. The two mechanisms SHALL NOT be chained such that a failure of the first is what triggers the second, because the synchronous mechanism is only permitted inside the originating user gesture and awaiting the asynchronous one ends it.

**Confirmation.** The header SHALL confirm the outcome. A successful copy SHALL be indicated visually and announced to assistive technology; a refused copy SHALL be distinguished from a successful one, and SHALL leave the name selected so the platform's own copy shortcut still completes the action. Confirmation SHALL NOT change the layout of the header — no label substitution, no added or removed glyph — because the name shares a flex row with the branch chip and may wrap, so any width change would move the row on every copy. Confirmation SHALL revert on its own, and SHALL NOT outlive the artifact it described.

**Keyboard.** The change name SHALL be reachable by keyboard as a single tab stop within the detail pane, SHALL expose an accessible name describing the copy action and the value, and SHALL be activated by Enter and by Space, performing the same copy as a click. It SHALL show the application's standard focus indicator. No global keyboard chord SHALL be introduced for this; in particular the platform's own copy shortcut SHALL retain its native meaning everywhere. The tree's roving-focus, single-Tab-stop model SHALL be unaffected.

**Persistence while reading.** The header SHALL remain visible while the artifact's content scrolls, so the change's identity is answerable at any scroll position rather than only at the top of the document.

**Clearance of the native titlebar strip.** In the native desktop window on macOS, a drag region spans the full width of the top of the window (see *visual-identity → macOS Hidden Inset Titlebar Layout*), and a press inside it enters window drag or zoom rather than reaching what is beneath. The header SHALL be positioned so that the change name lies **clear of that region**, so a click on the name copies it rather than starting a window drag, and a double-click does not toggle window zoom. That clearance SHALL hold at **every scroll position**, not only at scroll top. The header's own background SHALL continue to span the full pane width across the cleared area, so no document content is visible above the identity at any scroll position. The drag region SHALL be left intact: the area the header clears SHALL remain draggable, and no exception SHALL be carved out of it. This clearance is a property of the native window only; the served web UI renders no such region and SHALL receive no offset.

**Anchoring.** Because the header occupies the top of the pane's scroll port, scroll anchors (see *Section and Task Scroll Anchors*) SHALL account for its height: a section or task scrolled to SHALL come to rest fully visible below the header, never underneath it. The height SHALL be taken from the rendered header rather than from a fixed constant, because the change name renders in full and therefore wraps at narrow pane widths — and because the macOS clearance changes that height. Any clearance SHALL therefore be inside the measured element.

**Placement.** The header SHALL be horizontally aligned with the artifact's prose column — sharing its width bound and horizontal origin — so it reads as heading the document rather than floating in the pane.

#### Scenario: Detail pane names the change it is rendering

- **WHEN** the user selects any artifact of a change in the tree
- **THEN** the detail pane displays that change's directory name above the rendered markdown
- **AND** the name is shown in full, with no truncation or ellipsis
- **AND** the name shown is the change's directory name, not its proposal title

#### Scenario: Branch appears as a chip beside the name

- **WHEN** the detail pane renders an artifact of a change in a git worktree on a named branch
- **THEN** the header shows that branch name as an outlined chip following the change name

#### Scenario: The branch chip carries the workspace's palette colour

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has a configured palette colour
- **THEN** the branch chip's text and border render in a contrast-safe shade of that colour
- **AND** the colour is the one configured for the workspace owning the worktree the artifact was read from

#### Scenario: The branch chip stays neutral when no palette colour is configured

- **WHEN** the detail pane renders an artifact of a change whose owning workspace has no configured palette colour
- **THEN** the branch chip renders in the neutral ink it renders in today
- **AND** no colour is derived or substituted in place of the missing one

#### Scenario: The header chip and the tree chip agree

- **WHEN** the tree and the detail pane are both visible, and each shows a chip naming the same branch of the same change
- **THEN** the two chips render identically, in the same tint and treatment
- **AND** this holds for every palette colour and for a workspace with none configured

#### Scenario: A flat-workspace artifact shows no branch chip

- **WHEN** the detail pane renders an artifact of a change in a flat (non-git) workspace
- **THEN** the header shows the change's directory name alone
- **AND** no branch chip is rendered

#### Scenario: An archived change shows no branch chip

- **WHEN** the detail pane renders an artifact of an archived change
- **AND** the worktree path it was read from hosts active changes on a named branch
- **THEN** no branch chip is rendered
- **AND** that worktree's branch is not shown anywhere in the header
- **AND** no palette colour is applied, there being no chip to tint

#### Scenario: The header reports when the artifact last changed

- **WHEN** the detail pane renders an artifact whose file was last written some interval ago
- **THEN** the header displays that interval in relative terms
- **AND** the interval is measured from the modification time of that artifact's own file

#### Scenario: A sibling artifact's edit is not reported as this one's

- **WHEN** the detail pane is rendering a change's `proposal.md`
- **AND** `tasks.md` in the same change directory is written
- **THEN** the interval reported for the `proposal.md` on screen is unchanged
- **AND** it continues to reflect when `proposal.md` itself was last written

#### Scenario: The label advances while the reader stays on the artifact

- **WHEN** the detail pane has rendered an artifact and enough time passes for the reported interval to change
- **AND** the user has neither navigated away nor taken any action
- **AND** nothing on disk has changed
- **THEN** the displayed interval advances to reflect the time now elapsed

#### Scenario: A rewrite with identical bytes still updates the label

- **WHEN** the detail pane is rendering an artifact
- **AND** that artifact's file is rewritten with content identical to what is displayed, moving its modification time
- **THEN** the reported interval updates to reflect the new modification time
- **AND** the rendered document and the reading position are unchanged

#### Scenario: The advancing label never moves the change name

- **WHEN** the reported interval advances, including across a change of unit
- **THEN** the label occupies the same width as before
- **AND** the change name occupies the same width and wraps at the same points
- **AND** no element of the header changes position

#### Scenario: A modification time in the future is not shown as future

- **WHEN** the detail pane renders an artifact whose file carries a modification time later than the present
- **THEN** the header presents the artifact as having changed at the present moment
- **AND** no future interval is displayed

#### Scenario: The header and the tree word an interval the same way

- **WHEN** the tree and the detail pane are both visible, each showing a relative time
- **THEN** an interval of the same length is rendered in the same words on both
- **AND** this holds at every tier of the vocabulary

#### Scenario: An artifact with no readable modification time shows no label

- **WHEN** the detail pane renders an artifact whose file reports no usable modification time
- **THEN** the artifact's markdown is displayed as normal
- **AND** no last-changed label is rendered
- **AND** no default, derived, or epoch time is shown in its place

#### Scenario: An archived artifact reports its modification time like any other

- **WHEN** the detail pane renders an artifact of an archived change
- **THEN** the header reports that file's modification time under the same rule as an active change's
- **AND** no alternative source, such as the archive date in the directory name, is substituted

#### Scenario: The label stops when the artifact it described is gone

- **WHEN** the detail pane's artifact target changes or clears while the label is advancing
- **THEN** the label ceases to advance for the artifact that is no longer rendered
- **AND** no update is applied to a header describing a different artifact

#### Scenario: One click copies the whole name

- **WHEN** the user clicks once on the change name in the header
- **THEN** exactly that change name is placed on the clipboard
- **AND** the name is also selected, as confirmation of what was copied
- **AND** a successful copy is indicated and announced

#### Scenario: The copied value excludes the branch

- **WHEN** the user clicks once on the change name of a change whose branch chip is displayed
- **THEN** the clipboard contains the change name only
- **AND** the branch chip's text is not part of what was copied, nor of the selection

#### Scenario: The copied value excludes the last-changed label

- **WHEN** the user clicks once on the change name while the last-changed label is displayed
- **THEN** the clipboard contains the change name only
- **AND** the label's text is not part of what was copied, nor of the selection

#### Scenario: Copy works where the asynchronous clipboard API is unavailable

- **WHEN** the web UI is reached over a non-loopback bind on a plain-HTTP origin, where `navigator.clipboard` is not exposed
- **AND** the user clicks once on the change name
- **THEN** the name is still placed on the clipboard, by the synchronous mechanism over the selection
- **AND** no failure is reported to the user

#### Scenario: A refused copy leaves the value selected

- **WHEN** a copy is attempted and the clipboard write is refused
- **THEN** the failure is distinguished from a success, not reported as one
- **AND** the change name remains selected, so the platform's copy shortcut completes the action

#### Scenario: Confirming a copy does not move the header

- **WHEN** a copy succeeds and the header shows its confirmation
- **THEN** the change name occupies the same width as before the copy
- **AND** no element of the header changes position

#### Scenario: Keyboard copies without a chord

- **WHEN** the user moves focus to the change name and presses Enter or Space
- **THEN** the same copy occurs as for a click
- **AND** the focus indicator is visible on the name
- **AND** the platform's own copy shortcut retains its native meaning

#### Scenario: Identity survives scrolling a long artifact

- **WHEN** the user scrolls an artifact long enough that its first line leaves the viewport
- **THEN** the change-identity header remains visible

#### Scenario: An anchored section is not obscured by the header

- **WHEN** the user selects a section or task row that scrolls the artifact to that anchor
- **THEN** the anchored section or task comes to rest fully visible
- **AND** it is not positioned underneath the change-identity header, at any header height

#### Scenario: The change name is clickable in the native macOS window

- **WHEN** the application runs in the native window on macOS, where the titlebar drag region covers the top of the window
- **AND** the user clicks once on the change name
- **THEN** the name is copied
- **AND** the window does not begin a drag
- **AND** a double-click on the name does not toggle window zoom

#### Scenario: Clearance holds while the artifact is scrolled

- **WHEN** the application runs in the native window on macOS and the artifact is scrolled to any position
- **THEN** the change name remains clear of the titlebar drag region and remains clickable
- **AND** no document content is visible above the header

#### Scenario: The titlebar drag region keeps working

- **WHEN** the application runs in the native window on macOS
- **AND** the user presses in the top 32px of the window over the detail pane, outside the change name
- **THEN** the window enters native drag mode exactly as it does elsewhere along the strip

#### Scenario: The served web UI takes no titlebar offset

- **WHEN** the web UI is served in a browser, which renders no titlebar drag region
- **THEN** the header takes no clearance offset
- **AND** the identity sits at the top of the pane

#### Scenario: Non-artifact targets are unaffected

- **WHEN** the detail pane renders the Dashboard, a commit's detail view, the workspace file browser, the Archive view, or the Settings view
- **THEN** no change-identity header is rendered over it
- **AND** each of those views keeps the header it renders today
### Requirement: Link Handling in Rendered Artifacts

A link click inside markdown rendered by the shared markdown renderer — change artifacts, archived artifacts, and workspace file-browser previews alike — SHALL never navigate the application's webview: every anchor activation SHALL be intercepted and dispatched by link class, any class without a defined behaviour SHALL be inert, and activation paths that bypass the renderer's click handling (such as the webview's native context menu or link drag-out) SHALL be denied by a shell-level navigation guard that permits only the application's own origin.

An absolute link with an `http` or `https` scheme SHALL open in the system default browser, and a `mailto:` or `tel:` link SHALL open via the operating system's default handler, in each case leaving the application view unchanged.

A relative link to a non-markdown file SHALL be resolved against the directory of the markdown file being viewed — after stripping any fragment and query and percent-decoding the path exactly once — and opened with the operating system's default handler for the target's type; for an `.html` mockup that is the default browser, which resolves the mockup's sibling assets (stylesheets, scripts, images) itself. The target MAY live anywhere inside the authorized root; it is not confined to the change directory the linking artifact belongs to. This boundary is deliberately wider than the `openspec/changes/` subtree that confines artifact reads (see *Artifact Reads Are Confined to Registered Workspaces*): the open operation reads and returns no file content — its effect is limited to asking the OS to display an allow-listed document inside a folder the user brought into the application.

Opening SHALL be authorized at the shared application boundary before any opener is invoked:

- The root SHALL be authorized by the same rule that authorizes file browsing (see *Browsing Is Confined to Registered Workspaces* in the `workspace-file-browser` capability): a registered or registry-discovered workspace, or a repository main worktree accepted because a worktree of that repository is registered. An unauthorized root SHALL be refused before any path is resolved.
- The canonicalised target SHALL be contained within the canonicalised authorized root — so a `..` traversal (encoded or not) or a symlink pointing outside the root is refused rather than opened.
- The target SHALL match a case-insensitive allow-list of document types — initially `.html`, `.htm`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`, `.css`, `.pdf`, `.txt`, `.json`, `.csv` — and directories SHALL be refused. Executable and script targets are therefore never opened: following a link SHALL NOT be able to execute a file.

The frontend SHALL NOT hold a general open-URL or open-path capability; the only open operation reachable from rendered content is this validated one.

Relative links to markdown files (matched case-insensitively) SHALL be inert in v1, reserved for future in-app navigation. Links with any other scheme (including `javascript:` and `file:`) SHALL be inert. A **fragment-only** link is dispatched by the `document-outline` capability's *Fragment Links Resolve Within the Document* requirement: it scrolls within the rendered document when its fragment names a heading, and fails quietly as a dangling link when it does not; it never navigates the application. Inert links SHALL carry a visual affordance distinguishing them from openable links, so a dead link reads as policy rather than breakage; a fragment link that resolves to a heading is not inert and SHALL NOT carry it.

A click whose target does not exist or is refused SHALL produce a quiet indication that the link could not be opened, SHALL NOT navigate or blank the pane, and SHALL leave the rendered artifact fully usable.

Opening files is a desktop-frontend concern; other frontends degrade per their own capability specs, and the raw artifact markdown returned by the backend is unchanged by this requirement.

#### Scenario: An external link opens in the system browser

- **WHEN** the user clicks a link with an `http` or `https` URL in a rendered artifact
- **THEN** the URL opens in the system default browser
- **AND** the application view does not navigate away

#### Scenario: A relative HTML mockup link opens externally

- **WHEN** a change's `proposal.md` contains a relative link to `./mockups/login.html` and that file exists
- **THEN** clicking the link opens the mockup via the operating system's default handler for HTML
- **AND** the detail pane still shows the rendered proposal

#### Scenario: A mockup outside the change directory opens

- **WHEN** an artifact links to an `.html` file that resolves inside the authorized root but outside the linking change's directory
- **THEN** clicking the link opens the file via the operating system's default handler

#### Scenario: A fragment- or query-suffixed file link opens the file

- **WHEN** an artifact links to `./mockups/login.html#hero` or `./mockups/login.html?v=2`, or to a target whose name is percent-encoded (such as `./my%20mockup.html`)
- **THEN** the underlying file resolves and opens as if linked plainly

#### Scenario: A link escaping the root is refused

- **WHEN** an artifact's relative link resolves outside the authorized root — via `..` traversal (plain or percent-encoded) or via a symlink inside the root whose target lies outside it
- **THEN** nothing is opened
- **AND** a quiet indication is shown that the link could not be opened
- **AND** the pane neither navigates nor blanks

#### Scenario: An executable or directory target is refused

- **WHEN** an artifact links to an executable or script file (such as `./run.sh`, `./setup.command`, or `./tool.exe`) or to a directory (including an `.app` bundle)
- **THEN** nothing is opened or executed
- **AND** a quiet indication is shown that the link could not be opened

#### Scenario: The open operation refuses an unauthorized root

- **WHEN** the open operation is invoked with a root that is neither a registered or registry-discovered workspace nor a repository main worktree accepted by the browsing rule
- **THEN** it is refused with an error before any path is resolved or opened

#### Scenario: A relative markdown link is inert

- **WHEN** the user clicks a relative link to a markdown file in a rendered artifact, regardless of extension casing (`./notes.md`, `./NOTES.MD`)
- **THEN** nothing opens and the application view does not change

#### Scenario: A script-scheme link is inert

- **WHEN** a rendered artifact contains a link with a `javascript:` or `file:` href
- **THEN** clicking it executes nothing, opens nothing, and does not navigate the webview

#### Scenario: Bypassing the click handler cannot navigate the app

- **WHEN** a link is activated through a path that bypasses the renderer's click handling — the webview context menu's open-link action, or dragging the link
- **THEN** the application webview does not navigate away from the app UI

#### Scenario: A dangling link fails quietly

- **WHEN** the user clicks a relative link whose target file does not exist
- **THEN** a quiet indication is shown that the link could not be opened
- **AND** the rendered artifact remains fully usable

#### Scenario: A fragment link scrolls within the document

- **WHEN** the user clicks a fragment-only link whose fragment names a heading of the rendered artifact
- **THEN** the artifact scrolls to that heading
- **AND** the application view, the address and the history are unchanged

## ADDED Requirements

### Requirement: Artifact Tab Strip in the Change Header

While the detail pane's target is an OpenSpec artifact, the change header (see *Change Identity Header in the Detail Pane*, *Rows*) SHALL render a **tab strip** offering the change's artifacts, in the fixed order Proposal, Design, Tasks, then one tab per capability spec in listing order. Only artifacts **present on disk** for the rendered instance SHALL be offered: an absent artifact has no tab, dimmed or otherwise, so the strip never offers a control that cannot render. Presence is taken from the same aggregation the tree reads (`ChangeData.artifacts`), and the strip SHALL re-derive when that aggregation refreshes, so a tab appears when its file appears. In the read-only form, presence is the on-demand status the `archive-browser` capability determines for the selected copy.

The **active tab** is the artifact the current address names. Activating a tab is a **navigation**: it SHALL form the artifact address for that artifact in the same instance and navigate to it, so history, deep links and reader windows treat it exactly as a tree click was treated (see *History Entry Discipline* in the `view-routing` capability — one entry per artifact shown). When the file behind the active tab vanishes, the tab SHALL remain active while the document view reports the document missing (see *A Vanished Document Is Reported, Not Followed* in `reader-window`); the strip SHALL NOT switch tabs on the reader's behalf.

The **Tasks tab** SHALL carry the change's task progress in its trailing slot: the task-progress meter (per *Task Progress Meter* in `visual-identity`) while at least one task is incomplete, the completion ✓ glyph when every task is complete, and neither when the change parses no tasks — the same rule the change row applies, fed by the same counts, so the two never disagree on one screen.

**Keyboard.** The strip SHALL be a WAI-ARIA tab list with **manual activation**: it occupies one position in the pane's Tab order, focus moves among tabs with ArrowLeft, ArrowRight, Home and End without changing what is shown, and Enter or Space activates the focused tab. Each tab exposes `role="tab"` with `aria-selected` reflecting whether it is active; the strip exposes `role="tablist"`. Automatic activation is deliberately not used, because activation navigates and a history entry per traversed tab would make Back useless.

#### Scenario: Only present artifacts are offered

- **WHEN** the detail pane renders an artifact of a change that has a proposal, tasks and two capability specs but no design
- **THEN** the strip offers Proposal, Tasks and the two specs, in that order
- **AND** no Design tab is rendered

#### Scenario: Activating a tab navigates

- **WHEN** the user activates the Tasks tab while the proposal is shown
- **THEN** the detail pane renders the change's tasks
- **AND** a subsequent back gesture returns to the proposal

#### Scenario: The Tasks tab carries progress

- **WHEN** the change has at least one incomplete task
- **THEN** the Tasks tab shows the task-progress meter
- **WHEN** every task is complete
- **THEN** the Tasks tab shows the completion ✓ and no meter
- **WHEN** the change parses no tasks
- **THEN** the Tasks tab shows neither

#### Scenario: A newly written artifact gains a tab

- **WHEN** the detail pane renders a change's proposal and a `design.md` is then written into the change directory
- **THEN** the strip offers a Design tab once the aggregation refreshes
- **AND** the proposal remains shown

#### Scenario: Arrow keys move focus without navigating

- **WHEN** the strip has focus on the Proposal tab and the user presses ArrowRight twice
- **THEN** focus rests on the third tab
- **AND** the proposal is still shown and no history entry was created
- **WHEN** the user then presses Enter
- **THEN** the detail pane navigates to the focused tab's artifact

#### Scenario: The read-only form derives tabs from the selected copy

- **WHEN** the archive reader renders an archived change whose selected copy has a proposal and one spec on disk
- **THEN** the strip offers Proposal and that spec only

### Requirement: Instance Switcher in the Change Header

When the change the detail pane is rendering has **more than one rendered instance** (see *Workspace Tree Hierarchy*), the change header SHALL render an **instance switcher** row between the identity row and the tab strip, offering one control per rendered instance. When the change has exactly one rendered instance, no switcher row SHALL be rendered and no space reserved for it.

Each instance's control SHALL be labelled by its worktree folder basename, followed by the instance's branch as an outlined chip tinted exactly as the identity row's chip is (see *Change Identity Header in the Detail Pane*, *Branch chip colour*) — the basename alone when the branch is not known — and by its divergence label when it has one (see *Per-Instance Divergence Label*). The control for the instance currently rendered SHALL be marked as selected and exposed as such to assistive technology.

Activating an instance's control is a **navigation** to the same artifact kind (and capability) in the chosen instance, with the address carrying the instance segment per *Shortest Unambiguous Address* in the `view-routing` capability. When the chosen instance does not hold that artifact, the navigation SHALL open the chosen instance's default artifact instead (see *Workspace Tree Hierarchy*), never a read error.

In the **read-only form** the switcher offers the archived change's **copies**, labelled as *Copy Selection Within an Opened Archived Change* in the `archive-browser` capability specifies, with no branch chip and no divergence label; it renders as a plain label when there is one copy, as that requirement already demands.

#### Scenario: A singleton change shows no switcher

- **WHEN** the detail pane renders an artifact of a change with exactly one rendered instance
- **THEN** the header contains no instance switcher row

#### Scenario: A multi-instance change names each instance

- **WHEN** the detail pane renders an artifact of a change hosted in two worktrees on different branches, one of which is diverged
- **THEN** the header shows two controls, each labelled by its worktree basename and branch chip
- **AND** the diverged instance's control carries the `[diverged]` label
- **AND** the rendered instance's control is marked selected

#### Scenario: Switching instance keeps the artifact

- **WHEN** the design of the main worktree's instance is shown and the user activates the feature worktree's control
- **THEN** the detail pane renders the feature worktree's design
- **AND** the address names that instance

#### Scenario: Switching to an instance lacking the artifact opens its default

- **WHEN** the design of one instance is shown and the user switches to an instance that has no `design.md`
- **THEN** the detail pane renders that instance's default artifact
- **AND** no read error is shown

#### Scenario: The read-only form offers copies without branches

- **WHEN** the archive reader renders an archived change with two copies
- **THEN** the switcher offers the two copies under the archive's copy labels
- **AND** no branch chip or divergence label is rendered

## REMOVED Requirements

### Requirement: Section and Task Scroll Anchors

**Reason**: The tree no longer renders section or task rows. Navigation within a document is the `document-outline` capability's *Outline Navigation*, which scrolls to headings.

**Migration**: Open the Tasks tab and use the outline's section entries, which also show per-section progress.

### Requirement: Deferred Interaction Nodes

**Reason**: A change row now navigates to the change's default artifact (*Workspace Tree Hierarchy*); logical-change disclosure parents and the Specs artifact node no longer exist.

**Migration**: None; the click that did nothing now opens the change.

### Requirement: Default Expansion of Tree Nodes

**Reason**: The tree has one expandable level, open by default, with no per-node default rules.

**Migration**: None.

### Requirement: User Collapse State Persists Across Sessions

**Reason**: Top-level disclosure is session-only (*Workspace Tree Hierarchy*). The `collapsedTreeNodeIds` and `expandedTreeNodeIds` settings fields become unread; they are tolerated in an existing settings file, never written.

**Migration**: None; a closed row reopens on restart.

### Requirement: Tree Expansion Has No First-Sight Auto-Expansion Effect

**Reason**: Without persisted disclosure state there is no first-sight rule to hold.

**Migration**: None.

### Requirement: Auto-Collapse of Completed Task Groups

**Reason**: There are no task-group rows to collapse.

**Migration**: None.

### Requirement: Completed Section Row Shows a Completion Glyph

**Reason**: There are no section rows. The `document-outline` capability's *Section Progress in a Tasks Outline* shows the same glyph on a completed section's outline entry.

**Migration**: Read the Tasks tab's outline.

### Requirement: Artifact Row Presence Treatment

**Reason**: There are no artifact rows. *Artifact Tab Strip in the Change Header* offers present artifacts only, with no dimmed placeholder.

**Migration**: None.

### Requirement: Tasks Artifact Node Progress

**Reason**: There is no Tasks node. *Artifact Tab Strip in the Change Header* carries the same meter and glyph on the Tasks tab.

**Migration**: None.

### Requirement: Leaf-Task Completion Rendering

**Reason**: There are no leaf-task rows. Completed tasks continue to render struck-through in the document itself (*Markdown Rendering of Leaf Artifacts*).

**Migration**: None.

### Requirement: Singleton Logical-Change Flattening and Promotion

**Reason**: Every logical change renders as one row (*Workspace Tree Hierarchy*); there is no disclosure parent to flatten or promote.

**Migration**: None.

### Requirement: Instance Row Chrome

**Reason**: There are no per-instance rows. Instances are named in *Instance Switcher in the Change Header*.

**Migration**: None.

### Requirement: Active-Instance Indicator

**Reason**: There are no per-instance rows to mark. The switcher marks the rendered instance as selected.

**Migration**: None.

