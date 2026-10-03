## ADDED Requirements

### Requirement: Settings Switch Control

Every on/off setting in the settings view SHALL be operated by one **settings switch** (see the *Settings Rows* requirement in the `settings-view` capability). The switch SHALL be a native checkbox input exposed to assistive technology with `role="switch"`. It thereby keeps the native control's keyboard operation, label association and checked state, but SHALL be drawn as a fully rounded track holding a round knob rather than with the platform's checkbox appearance.

The switch SHALL draw its two states as follows:

- **Off.** The track SHALL render `--surface-2` with a `--border-strong` edge, and the knob SHALL sit at the leading end in `--text-faint`.
- **On.** The track's edge and the knob SHALL render in `--accent`, with the knob at the trailing end, and the track SHALL keep a neutral `--surface` background.

In neither state SHALL the switch carry an accent fill or glow, which preserves the accent-fill discipline of the *Accent Color* requirement. The accent is the switch's ink, exactly as it is the selected segment's border in the settings view's multi-way choices. Because the knob's position differs between the states, the state SHALL remain legible without relying on colour.

When focused from the keyboard, the switch SHALL show the application's keyboard-focus ring. The knob SHALL move with a short transition, and with no transition when `prefers-reduced-motion: reduce` is set. A switch whose row title does not also operate it SHALL present the enlarged hit area the *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirement in the `touch-input` capability requires of a bounded control, on a device whose primary pointer is coarse. An example is the switch in a registered-workspace row.

#### Scenario: A switch that is on is drawn in accent ink

- **WHEN** a settings switch is on
- **THEN** its track's edge and its knob render in `--accent`
- **AND** its track carries no accent fill and no glow
- **AND** its knob sits at the trailing end

#### Scenario: A switch that is off is neutral

- **WHEN** a settings switch is off
- **THEN** its track renders `--surface-2` with a `--border-strong` edge
- **AND** its knob renders in `--text-faint` at the leading end

#### Scenario: The switch is a native control exposed as a switch

- **WHEN** a settings switch is rendered
- **THEN** it is an `<input type="checkbox">` exposed with `role="switch"`
- **AND** pressing Space while it is focused flips it

#### Scenario: The state reads without colour

- **WHEN** a settings switch is rendered in either state
- **THEN** the knob's position alone distinguishes on from off

#### Scenario: Reduced motion moves the knob without a transition

- **WHEN** `prefers-reduced-motion: reduce` is set and a settings switch flips
- **THEN** the knob moves to its new end without a transition

## MODIFIED Requirements

### Requirement: Accent Color

The application SHALL use a vivid indigo accent system in the indigo/violet family. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

In the dark scheme the accent tokens SHALL be: `--accent` `#7c8cff` (the ink/line color used for links, the selection bar, focus rings, syntax-highlight titles, and the edge and knob of a settings switch that is on — see the *Settings Switch Control* requirement — while markdown task checkboxes are status glyphs in the ok family per the *Markdown Task-Checkbox Treatment* requirement, not accent-coloured controls); `--accent-hover` `#93a1ff` (hover BRIGHTENS on dark); `--accent-active` `#5d6ef0` (pressed); `--accent-strong` `#4f5fe0` (the fill-under-white-text surface, used ONLY as the primary-button background, where the white label SHALL reach at least 4.5:1 — `#4f5fe0` yields 5.19:1); `--accent-tint` `rgba(124, 140, 255, 0.14)`; `--accent-tint-strong` `rgba(124, 140, 255, 0.22)`; `--accent-glow` `rgba(124, 140, 255, 0.35)`.

In the light scheme the accent SHALL hold its indigo identity with the INVERSE hover direction (on a light background, lift = darken): `--accent` `#4f5bd9`, `--accent-hover` `#3f4bc4`, `--accent-active` `#3a46b8`.

The accent SHALL appear FILLED or GLOWING in exactly four places — the selected tree row, the primary button, the in-progress task-progress meter, and the favorite star on a favorited change row (a solid `--accent` star glyph, glow-free; see the *Change-Row Favorite Toggle* requirement in the `spec-browser` capability) — plus focused inputs and the focus ring. Everywhere else (links, informational chips, status dots, settings switches) the accent or status color SHALL be ink/outline only and SHALL carry no fill or glow. The sanctioned *done* fills — the completion mark in the row grammar and the checked markdown task checkbox in the document surface (see the *Outlined Chip Badges* and *Markdown Task-Checkbox Treatment* requirements) — are the only status-colour fills beyond the in-progress meter's sanctioned `--ok` fill (see the *Task Progress Meter* requirement), and both carry no glow.

#### Scenario: Accent stays in the indigo family

- **WHEN** the application stylesheet is inspected
- **THEN** the `--accent` family resolves to an indigo/violet hue (approximately hue 231)
- **AND** the raw macOS system blue `rgb(0, 122, 255)` does not appear in any user-visible chrome

#### Scenario: Accent is filled or glowing in exactly four places

- **WHEN** the UI is rendered
- **THEN** an accent fill or glow appears only on the selected tree row, the primary button, the in-progress meter, the favorited row's solid star, focused inputs, and the focus ring
- **AND** the favorite star's fill carries no glow
- **AND** links, informational chips, status dots and settings switches render their color as ink/outline with no fill or glow

#### Scenario: Primary button uses the accent

- **WHEN** a primary button is rendered (for example "Add workspace" in settings)
- **THEN** its background is `--accent-strong` with a white label at at least 4.5:1
- **AND** its hover background is `--accent-hover` and its pressed background is `--accent-active`

#### Scenario: Links in rendered markdown use the accent

- **WHEN** the detail pane renders an `<a>` element from markdown
- **THEN** the link color is `--accent`
- **AND** a link rendered on a selected row uses `--accent-hover` so it clears AA on the selection wash

### Requirement: List-Row Vertical Rhythm Tuned for 4K @ 100%

The vertical padding of list-row-like surfaces SHALL be set so that rows read comfortably on a 4K display at 100% OS scale (one CSS px = one device px) without losing the dense-browser character of the sidebar. Specifically:

- The workspace tree row (`.tree-row`) SHALL use 5px vertical padding (top and bottom).
- The settings workspaces row (`.workspace-row`) SHALL use `--space-4` (16px) vertical padding.
- The settings row (`.settings-row`) SHALL use `--space-3` (12px) vertical padding. It is the row every setting other than a registered workspace renders in (see the *Settings Rows* requirement in the `settings-view` capability).

The horizontal padding of these rows is unchanged by this requirement; only vertical rhythm is constrained.

#### Scenario: Tree row breathes at the retuned padding

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `padding-top` and `padding-bottom` are both 5px
- **AND** the row's horizontal padding values are unchanged from the existing layout

#### Scenario: Settings workspaces row tracks the tree row

- **WHEN** a registered-workspace row is rendered in settings
- **THEN** the row's computed vertical padding resolves from `--space-4`
- **AND** the row does not feel visually tighter than a sidebar tree row at the same display scale

#### Scenario: Settings row breathes at its padding

- **WHEN** a settings row is rendered
- **THEN** the row's computed `padding-top` and `padding-bottom` both resolve from `--space-3`

### Requirement: Markdown Task-Checkbox Treatment

The markdown view SHALL render GFM task-list checkboxes as inline SVG glyphs from the application icon set, NOT as native `<input type="checkbox">` controls. The treatment SHALL apply on every surface that renders markdown through the shared `.markdown-view` renderer (the detail pane, the archive reading view, and the file-browser preview).

The glyphs SHALL render at 16px so they sit flush with the `--text-lg` markdown body text.

A checked task (`- [x]`) SHALL render as a solid `--ok-strong` rounded square with its check knocked out in `--bg` (the plane every markdown surface sits on) — the same knocked-out-check construction as the tree's completion mark (identical check geometry, stroke-width 2.5, round caps and joins in the 24×24 viewBox), squared off, so the two marks read as siblings. The checked glyph SHALL carry no `box-shadow` glow or halo. An unchecked task (`- [ ]`) SHALL render as an outlined square stroked in `--text-faint` with no fill. The pending box is a meaningful status boundary, so its ink SHALL clear the 3:1 non-text floor on `--bg` in both schemes (`--text-faint`: 4.70:1 light / 5.43:1 dark); the light scheme's `--border-strong` (1.53:1 on `--bg`) is decorative-grade there and MUST NOT carry this boundary.

The checkbox is a STATUS glyph, not a control: it SHALL NOT use the accent family, preserving the accent-fill discipline of the *Accent Color* requirement. The checked fill SHALL be `--ok-strong` (the AA-clearing foreground "done" green), NOT `--ok` (which stays reserved as the in-progress meter fill).

A checked task's line text SHALL render in `--text-faint` with NO line-through — on this surface the glyph is the colour-independent "done" signal (unlike the tree's leaf-task rows, which carry no glyph and strike their labels). The dimming cascades to nested content of the checked task, EXCEPT that a pending (unchecked) task line nested under a checked task SHALL keep the default `--text` colour.

The glyphs SHALL remain inert — satisfying the spec-browser *Read-Only Viewer* requirement structurally, with no click behaviour to suppress — and SHALL expose checkbox semantics to assistive technology: an element with `role="checkbox"`, `aria-checked` reflecting the task state, and `aria-disabled="true"`, without introducing a keyboard focus stop per task line.

The settings view's on/off controls are real interactive controls, not markdown status glyphs. They SHALL render as the settings switch (see the *Settings Switch Control* requirement), a native checkbox input exposed as a switch, unaffected by this glyph treatment.

#### Scenario: Checked task renders the filled done glyph

- **WHEN** the detail pane renders a `tasks.md` line `- [x] <label>`
- **THEN** the line's leading glyph is a 16px solid `--ok-strong` rounded square with a `--bg` knocked-out check matching the completion mark's check construction
- **AND** no native `<input>` element is rendered for the task line
- **AND** the glyph carries no glow

#### Scenario: Unchecked task renders the outlined pending glyph

- **WHEN** the detail pane renders a `tasks.md` line `- [ ] <label>`
- **THEN** the line's leading glyph is a 16px outlined square stroked in `--text-faint` with no fill
- **AND** the stroke ink clears 3:1 against `--bg` in both colour schemes

#### Scenario: Checked line text dims without strikethrough

- **WHEN** a task line is checked
- **THEN** its line text renders in `--text-faint`
- **AND** it carries no line-through decoration

#### Scenario: Pending subtask under a checked parent keeps full-strength text

- **WHEN** an unchecked task line is nested under a checked task line
- **THEN** the nested pending line's text renders in the default `--text` colour

#### Scenario: Checkbox state reaches assistive technology without a focus stop

- **WHEN** a task checkbox glyph is rendered
- **THEN** it exposes `role="checkbox"` with `aria-checked` matching the task state and `aria-disabled="true"`
- **AND** it is not keyboard-focusable

#### Scenario: Settings toggle keeps its native control rendering

- **WHEN** the settings view renders an on/off setting (for example notifications)
- **THEN** it renders a native checkbox input exposed with `role="switch"` and drawn as the settings switch
- **AND** it is unaffected by the markdown glyph treatment

#### Scenario: Treatment applies on every markdown surface

- **WHEN** the archive reading view or the file-browser preview renders markdown containing task lines
- **THEN** the task checkboxes render with the same glyph treatment as the detail pane
