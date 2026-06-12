## ADDED Requirements

### Requirement: Proposal Title Extraction

The title of a change SHALL be extracted from its `proposal.md` as follows. The parser SHALL skip ignorable preamble at the top of the document: blank lines, one leading YAML frontmatter block (when the first content line is exactly `---`, through its closing `---`), and HTML comment blocks (`<!--` through `-->`, single- or multi-line). The first content line after the preamble SHALL yield a title only when it is a level-1 Markdown heading — a single `#` followed by whitespace and non-empty text after trimming leading whitespace. An optional case-insensitive `Proposal:` prefix SHALL be stripped from the heading text, and the result trimmed. Any other first content line — a deeper heading such as `## Why`, body text, or an unterminated preamble block — SHALL yield no title, and the parser SHALL NOT examine any further line of the document. A change with no extractable title SHALL continue to be labelled by its change ID wherever titles are displayed (sidebar rows, archive browser, dashboard). A missing or unreadable `proposal.md` SHALL yield no title.

#### Scenario: Title on the first line parses as before

- **WHEN** a `proposal.md` begins with `# Add User Auth` on line 1
- **THEN** the extracted title is "Add User Auth"
- **AND** a legacy `# Proposal: Add User Auth` first line also yields "Add User Auth"

#### Scenario: Title found below ignorable preamble

- **WHEN** a `proposal.md` opens with blank lines, a YAML frontmatter block, or HTML comments (in any combination), followed by `# Add User Auth`
- **THEN** the extracted title is "Add User Auth"

#### Scenario: Template-faithful proposal yields no title

- **WHEN** a `proposal.md` follows the spec-driven template and its first content line is `## Why`
- **THEN** no title is extracted (never "Why")
- **AND** the change's rows display its change ID

#### Scenario: Non-heading first content line yields no title

- **WHEN** the first content line after the preamble is body text, a deeper heading, or `#` without a following space
- **THEN** no title is extracted and no later line of the document is considered
- **AND** an h1 appearing only later in the body (for example inside a fenced code block) is never mistaken for the title

## MODIFIED Requirements

### Requirement: Two-Line Sole-Change-Row Layout

A change row that is the **sole row for its change** SHALL render across two stacked lines within a single selectable row. Exactly two row types are sole change rows:

- a **flattened singleton instance row** — a git logical change with exactly one instance, rendered flat (no disclosure parent) per *Singleton Logical-Change Flattening and Promotion*; and
- a **flat-workspace change row** — a `ChangeData` row rendered directly under a non-git workspace node.

Multi-instance child rows (governed by *Instance Row Chrome*), multi-instance logical-change disclosure parents, Repo-group and workspace header rows, the Proposal/Specs/Design/Tasks artifact rows, capability rows, Section rows, and task rows are all excluded and SHALL remain single-line.

**Line 1 (primary).** Line 1 SHALL display the change's `proposal.md` title when one is extractable (see *Proposal Title Extraction*) — falling back, for a git singleton, to the logical change name, and for a flat-workspace change row, to its directory name. When a git singleton's line 1 shows the proposal title, the row SHALL expose the logical change name via its hover tooltip so the directory identity stays recoverable. The label SHALL render with slightly heavier weight than its artifact-row siblings so it reads as the row's heading, and SHALL own the full row width so it is no longer truncated by a trailing branch chip or status meta; it SHALL ellipsize against the row edge only when it alone exceeds the available width. Line 1 carries no worktree identity, swatch, or colour tint on its text.

**Line 2 (detail).** Line 2 SHALL render at the tree's dense meta type tier, visually subordinate, and SHALL be indented to begin at line 1's text origin (past the chevron) so it reads as belonging to the row above it. Line 2 SHALL place worktree identity on its leading edge and status on its trailing edge:

- **Leading edge.** For a git singleton row the leading edge SHALL show the instance's branch name as an outlined chip (per *visual-identity → Outlined Chip Badges*) tinted to the owning workspace's palette colour — chip text and border rendered in a contrast-safe (≥4.5:1) shade of that colour. When the branch is not known (detached HEAD, bare worktree), the chip SHALL show the worktree folder basename instead. A flat-workspace change row has no git worktree identity; in its place the leading edge SHALL show the change's identifier (`changeId`), the same identifier the row shows today.
- **Status (trailing).** Line 2 SHALL carry the row's existing status elements, with their existing presence rules, on its trailing edge. For a **git singleton row** these are the task-progress meter while work is in progress or the completion ✓ when every task is complete (per *Change-Row Completion Glyph* and *Tasks Artifact Node Progress*), the relative modification time, and the divergence label when present (per *Per-Instance Divergence Label*). For a **flat-workspace change row** the only status element is the completion ✓ when every task is complete; a flat-workspace row carries no progress meter, modification time, or divergence label. The active-instance indicator is a multi-instance-child element and SHALL NOT appear on a sole change row.

**Workspace-colour rail.** A sole change row SHALL tint its inline-start border — the 2px slot the selection bar occupies — with the owning workspace's palette colour, so each change reads as belonging to its workspace and the colour ties the row to its branch chip top-to-bottom. While the row is selected the selection bar (the 2px `--accent` border, per *visual-identity → Tree Row Selection Model*) SHALL take precedence and replace the rail; the rail SHALL reappear when the row is deselected. A workspace with no configured palette colour renders no rail. Header rows and the other excluded row types do not render the rail.

**One interaction unit.** The two lines SHALL form a single interaction unit: one click target that selects the change and one selection unit. The selection treatment (the 2px `--accent` inline-start bar plus its tint wash) and the hover wash SHALL span both lines. The disclosure chevron SHALL toggle the row's artifact subtree exactly as it does today and SHALL remain associated with the row as a whole.

#### Scenario: Git singleton renders its proposal title on the first line

- **WHEN** a git logical change has exactly one instance and its `proposal.md` yields a title
- **THEN** line 1 shows that title across the full row width, in a slightly heavier weight than the artifact rows below it
- **AND** the label is not truncated by any branch or status element on the same line
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
- **AND** a click anywhere on either line selects the change and updates the detail pane

#### Scenario: Workspace-colour rail marks each change row

- **WHEN** a sole change row is rendered for a workspace that has a configured palette colour
- **AND** the row is not selected
- **THEN** the row's inline-start border (the selection-bar slot) is tinted to that workspace's palette colour
- **AND** every change row under the same workspace shares that colour, matching the workspace's top-level swatch

#### Scenario: Selection bar overrides the rail

- **WHEN** a sole change row that is showing its workspace-colour rail becomes selected
- **THEN** the inline-start border renders the 2px `--accent` selection bar instead of the workspace colour
- **AND** the workspace-colour rail reappears once the row is deselected
