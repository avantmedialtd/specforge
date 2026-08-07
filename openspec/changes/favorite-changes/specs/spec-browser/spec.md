# spec-browser Delta: Favorite Changes in the Workspace Tree

## ADDED Requirements

### Requirement: Change-Row Favorite Toggle

Exactly three row types are **favoritable rows** — the rows that aggregate a whole change: the flattened singleton logical-change row, the multi-instance logical-change disclosure parent row, and the flat-workspace change row. (This set is distinct from the "change-aggregating rows" of the *Change-Row Completion Glyph* requirement, which includes per-instance rows; a favorite always attaches to the change, never to one worktree's instance.) Every favoritable row SHALL render a favorite toggle (a star glyph) in a reserved slot at the extreme trailing edge of the row's primary line. On rows that already render trailing meta on that line (for example the multi-instance parent's instance-count badge), the slot sits after the existing meta, and because the slot is reserved, revealing or hiding the star SHALL NOT shift any other row content. Instance child rows beneath a multi-instance parent SHALL NOT render the toggle.

The toggle SHALL present two visual states: while the change is not a favorite, an outline star in the faint ink colour (`--text-faint`) that is hidden at rest and revealed while the row is hovered or holds the tree's roving focus; while the change is a favorite, a solid star in the accent ink (`--accent`) that is always visible — at rest, on hover, and while the row is selected. The filled star carries no glow and is the sole indicator of favorite status; no other badge, label, or row treatment conveys it. (The solid accent star is sanctioned by the *Accent Color* and *Outlined Chip Badges* censuses in the `visual-identity` capability, as modified by this change.)

Activating the toggle SHALL flip the change's favorite state and SHALL NOT select the row, change the tree's selected-node state, or alter the detail pane — mirroring the disclosure chevron's contract that a nested row control never triggers row selection. The toggle SHALL NOT join the tab order: the tree retains its roving-focus, single-Tab-stop keyboard model, and the toggle itself is never focusable. The nested button SHALL expose `aria-pressed` and an accessible label, and the favorite state SHALL additionally be conveyed at the treeitem level (in the row's accessible name or description), so assistive technology that flattens nested-control state still announces it.

Favorite state SHALL additionally be togglable by keyboard: Cmd+D (macOS) / Ctrl+D (Windows, Linux) toggles the favorite state of the focused favoritable row, with the same binding active in the served web UI, where it SHALL suppress the browser's native bookmark shortcut. The chord SHALL take precedence over first-letter typeahead: a keypress carrying the platform command modifier SHALL NOT move typeahead focus (see the *Workspace Tree Keyboard Navigation* requirement). When the focused row is not a favoritable row, the binding SHALL have no effect.

#### Scenario: Hover reveals the outline star on a non-favorite row

- **WHEN** the pointer hovers a favoritable row whose change is not a favorite
- **THEN** an outline star appears in the reserved slot at the trailing edge of the row's primary line
- **AND** the star is not visible on that row at rest
- **AND** no other content on the row shifts when the star appears

#### Scenario: Keyboard focus reveals the outline star

- **WHEN** a favoritable row whose change is not a favorite receives the tree's roving focus
- **THEN** the outline star is visible on that row
- **AND** the toggle itself is not focusable and the tree's single Tab stop is preserved

#### Scenario: Favorite rows show a persistent filled star

- **WHEN** a favoritable row's change is a favorite
- **THEN** the row renders a solid `--accent` star, with no glow, in its reserved trailing slot at rest, on hover, and while selected

#### Scenario: Toggling never selects the row

- **WHEN** the user clicks the star on a favoritable row while another node is selected
- **THEN** the change's favorite state flips
- **AND** the tree's selected node is unchanged
- **AND** the detail pane's contents are unchanged

#### Scenario: Instance child rows carry no star

- **WHEN** a multi-instance logical change is expanded
- **THEN** none of its instance child rows renders a favorite toggle
- **AND** the disclosure parent row renders the toggle for the logical change

#### Scenario: Cmd/Ctrl+D toggles the focused change row

- **WHEN** a favoritable row has keyboard focus
- **AND** the user presses Cmd+D (macOS) / Ctrl+D (Windows, Linux)
- **THEN** that change's favorite state flips
- **AND** focus and selection are unchanged
- **AND** typeahead does not move focus in response to the chord's letter

#### Scenario: Cmd/Ctrl+D elsewhere is inert

- **WHEN** a non-favoritable row (an artifact, section, task, capability spec, instance, or top-level row) has keyboard focus
- **AND** the user presses Cmd+D (macOS) / Ctrl+D (Windows, Linux)
- **THEN** no favorite state changes anywhere in the tree

### Requirement: Favorite-First Change Ordering

Within each top-level group — the logical changes under a Repo group, and the changes under a flat workspace — the tree SHALL render favorite changes before non-favorite changes, preserving the existing name order within each partition. The partition SHALL introduce no divider, section header, or count row: the filled star on each floated row is the only indicator of the grouping.

This ordering governs the favoritable rows of the tree pane only. It SHALL NOT reorder top-level rows, instance child rows (which keep their existing order), artifact nodes (fixed order per the *Workspace Tree Hierarchy* requirement), the Archive view (date-ordered), or the Dashboard's feeds. The terminal frontend is outside this capability and keeps the shared core's order.

#### Scenario: Favorite changes float to the front of their group

- **WHEN** a Repo group contains changes `alpha`, `mid`, and `zulu`, and `zulu` is a favorite
- **THEN** the group renders its change rows in the order `zulu`, `alpha`, `mid`
- **AND** no divider or header row separates `zulu` from `alpha`

#### Scenario: Name order is preserved within each partition

- **WHEN** a group contains favorites `delta` and `bravo` and non-favorites `charlie` and `alpha`
- **THEN** the rows render in the order `bravo`, `delta`, `alpha`, `charlie`

#### Scenario: Unfavoriting returns a row to its name-order slot

- **WHEN** the user removes the favorite state from a floated change row
- **THEN** the row re-renders in its name-order position among the non-favorite rows within the same top-level group

#### Scenario: Ordering applies per group, not across groups

- **WHEN** changes are favorites in two different top-level groups
- **THEN** each group floats only its own favorites to its own front
- **AND** the order of the top-level rows themselves is unchanged

### Requirement: Favorite Identity and Persistence

A favorite SHALL be keyed on the logical change's position-independent identity: for a repo-group change, the repository identity plus the change directory name; for a flat-workspace change, the workspace identity plus the change directory name. The favorite SHALL therefore be unaffected by singleton↔multi-instance promotion, by which worktrees currently host the change, and by tree position.

Favorite state SHALL persist across application restarts in application settings, alongside the collapse-state overrides — never inside any workspace's `openspec/` tree. A settings file written by a version predating this feature SHALL load cleanly with an empty favorites set. Writes SHALL be coalesced so rapid toggling does not write a settings file per intermediate state, and the persisted state eventually reflects the final toggled positions.

A persisted favorite whose change is not currently rendered — because the change is archived, its workspace is unregistered, or no change by that name exists — SHALL be inert: it is ignored while unmatched and applies again if a matching change reappears. The application is not required to garbage-collect inert entries. Favorite state is ambient view preference: it SHALL NOT be part of the Address, the URL, or navigation history, and navigating SHALL NOT change any favorite.

In the served web UI, favorite state is backed by the serving machine's application settings, shared with the desktop app and every connected client of that machine, consistent with how the collapse-state overrides behave. A concurrently connected client reflects another client's toggle the next time it loads the tree (for example a page reload); no push update or same-session convergence is required.

#### Scenario: Favorites survive a restart

- **WHEN** the user favorites a change and quits and relaunches the application
- **THEN** the change renders as a favorite, floated to the front of its group, without further user action

#### Scenario: Favorite survives singleton-to-multi promotion

- **WHEN** a favorited singleton logical change gains a second worktree instance
- **THEN** the resulting multi-instance disclosure parent row renders as a favorite
- **AND** removing all but one instance leaves the flattened row still a favorite

#### Scenario: Favorite goes inert on archive and returns on reappearance

- **WHEN** a favorited change is archived on disk
- **THEN** the change leaves the tree (per the *Workspace Tree Hierarchy* requirement) and its favorite entry has no visible effect
- **AND** when a change with the same identity is active again, its row renders as a favorite

#### Scenario: Pre-feature settings file loads cleanly

- **WHEN** the application starts against a settings file with no favorites field
- **THEN** settings load successfully
- **AND** the tree renders with no favorites and all other persisted preferences intact

#### Scenario: Rapid toggling coalesces writes

- **WHEN** the user toggles the same change's favorite state several times in rapid succession
- **THEN** the persisted state eventually reflects the final position
- **AND** the application does not write a settings file for every intermediate state

#### Scenario: Web clients share the serving machine's favorites

- **WHEN** a change is favorited in the desktop app on the serving machine
- **AND** a browser client subsequently loads the served web UI
- **THEN** the web tree renders that change as a favorite, floated to the front of its group

#### Scenario: Navigation does not alter favorites

- **WHEN** the user follows any address, including Back/Forward
- **THEN** no favorite state changes
- **AND** no favorite state appears in the address

## MODIFIED Requirements

### Requirement: Two-Line Sole-Change-Row Layout

A change row that is the **sole row for its change** SHALL render across two stacked lines within a single selectable row. Exactly two row types are sole change rows:

- a **flattened singleton instance row** — a git logical change with exactly one instance, rendered flat (no disclosure parent) per *Singleton Logical-Change Flattening and Promotion*; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Multi-instance child rows (governed by *Instance Row Chrome*), multi-instance logical-change disclosure parents, Repo-group and workspace header rows, the Proposal/Specs/Design/Tasks artifact rows, capability rows, Section rows, and task rows are all excluded and SHALL remain single-line.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than its artifact-row siblings so it reads as the row's heading, and SHALL own the full row width — no trailing branch chip or status meta shares the line — except for the favorite toggle's reserved trailing slot (see *Change-Row Favorite Toggle*); it SHALL ellipsize against that slot when it exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and *Tasks Artifact Node Progress*), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label. The active-instance indicator is a multi-instance-child element and SHALL NOT appear on a sole change row.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. The disclosure chevron SHALL toggle the row's artifact subtree exactly as it does today and SHALL remain associated with the row as a whole. The favorite toggle (see *Change-Row Favorite Toggle*) is the row's only other nested control; like the chevron, activating it SHALL NOT select the change.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than the artifact rows below it
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

#### Scenario: Multi-instance child row is excluded and stays single-line

- **WHEN** a logical change has two or more instances and is rendered as a disclosure parent with child rows
- **THEN** each child row remains a single line per *Instance Row Chrome*
- **AND** no child row adopts the two-line layout

#### Scenario: Completed sole change row shows its completion glyph on the detail line

- **WHEN** a sole change row's change has at least one task and every task is complete
- **THEN** line 2's trailing edge shows the completion ✓ in place of the progress meter

#### Scenario: Selection and hover span both lines of a sole change row

- **WHEN** a sole change row is selected, or the pointer hovers over either of its two lines
- **THEN** the selection bar and tint (or the hover wash) cover both lines as one contiguous row
- **AND** a click anywhere on either line — outside the disclosure chevron and the favorite toggle — selects the change and updates the detail pane

#### Scenario: Workspace-colour rail marks each change row

- **WHEN** a sole change row is rendered for a workspace that has a configured palette colour
- **AND** the row is not selected
- **THEN** the row's inline-start border (the selection-bar slot) is tinted to that workspace's palette colour
- **AND** every change row under the same workspace shares that colour, matching the workspace's top-level swatch

#### Scenario: Selection bar overrides the rail

- **WHEN** a sole change row that is showing its workspace-colour rail becomes selected
- **THEN** the inline-start border renders the 2px `--accent` selection bar instead of the workspace colour
- **AND** the workspace-colour rail reappears once the row is deselected
