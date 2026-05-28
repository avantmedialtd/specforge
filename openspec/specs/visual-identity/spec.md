# visual-identity Specification

## Purpose

Defines SpecForge's desktop visual identity: the design-token layer (color, type, space, radii, borders), the typography system, the single accent color, the row-selection model, the outlined chip / status-dot vocabulary, the inline SVG icon set, the macOS window-chrome treatment (sidebar vibrancy + hidden inset titlebar), and the markdown-body type pass. Cross-cuts every UI surface — the workspace tree, detail pane, settings view, and any future list surface all consume the same tokens and follow the same row grammar. Source of truth for implementation choices like Linear indigo `#5e6ad2` as the accent, Inter + JetBrains Mono as the type families, and the 2px-accent-bar selection style.
## Requirements
### Requirement: Design Token Layer

The application SHALL expose a single source of design tokens as CSS custom properties declared on `:root`, with dark-scheme overrides scoped to `@media (prefers-color-scheme: dark)`. All UI chrome (sidebar, tree rows, detail pane, settings, badges, buttons, dividers) SHALL consume these tokens rather than inline literal color, size, spacing, or radius values.

The token set SHALL include, at minimum:

- Color: `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, `--text-faint`, `--accent`, `--accent-hover`, `--accent-tint`, `--ok`, `--warn`.
- Type sizes: `--text-xs` (12px), `--text-sm` (13px), `--text-base` (14px), `--text-md` (15px), `--text-lg` (16px), `--text-xl` (22px), `--text-2xl` (30px).
- Type families: `--font-ui`, `--font-mono`.
- Line heights: `--leading-tight` (1.5), `--leading-prose` (1.65), `--leading-code` (1.5).
- Space: `--space-1` (4px), `--space-2` (8px), `--space-3` (12px), `--space-4` (16px), `--space-5` (24px), `--space-6` (32px), `--space-7` (48px).
- Radii: `--radius-sm` (4px), `--radius` (6px), `--radius-md` (8px).

The pre-existing legacy custom properties `--row-hover`, `--row-selected`, `--text-muted` (in its previous untyped form), `--divider`, and `--divider-hover` SHALL be removed once their callers consume the new tokens.

#### Scenario: Tokens defined on :root

- **WHEN** the application stylesheet is loaded
- **THEN** `:root` declares the full color, type, space, radii, and border token set
- **AND** every UI rule in the stylesheet references at least one token rather than a literal value (color literals are permitted only inside token definitions themselves)

#### Scenario: Type-size tokens render at the retuned px values

- **WHEN** the application stylesheet is loaded
- **THEN** `--text-xs` resolves to 12px, `--text-sm` to 13px, `--text-base` to 14px, `--text-md` to 15px, `--text-lg` to 16px, `--text-xl` to 22px, and `--text-2xl` to 30px
- **AND** `--leading-tight` resolves to 1.5

#### Scenario: Dark mode token overrides

- **WHEN** the operating system reports `prefers-color-scheme: dark`
- **THEN** the dark-scheme media query overrides at least `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, and `--text-faint`
- **AND** the accent and status tokens remain consistent across light and dark

### Requirement: Accent Color

The application SHALL use Linear indigo `#5e6ad2` as its single accent color, exposed as the `--accent` token, with a hover variant `--accent-hover` of `#4f5bbf` and a low-opacity tint `--accent-tint` of `rgba(94, 106, 210, 0.10)`. The accent SHALL be used for selection emphasis, focus rings, primary-action buttons, and inline markdown links. The raw macOS system blue (`rgb(0, 122, 255)`) MUST NOT appear in user-visible chrome.

#### Scenario: Selected tree row uses the accent

- **WHEN** a row in the workspace tree is selected
- **THEN** the row renders a 2px left border in `--accent`
- **AND** the row background is unchanged by the selected state (selection lives entirely in the inline-start border slot)

#### Scenario: Primary button uses the accent

- **WHEN** a primary button is rendered (for example "Add workspace" in settings)
- **THEN** its background is `--accent`
- **AND** its hover background is `--accent-hover`

#### Scenario: Links in rendered markdown use the accent

- **WHEN** the detail pane renders an `<a>` element from markdown
- **THEN** the link color is `--accent`

### Requirement: Cool Neutral Palette

Neutral tokens SHALL carry a slight blue tint rather than pure gray. The `--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-muted`, and `--text-faint` values SHALL match the design document's specified hex values for both light and dark schemes. The previous `rgba(127, 127, 127, …)` helper pattern MUST NOT appear in the stylesheet.

#### Scenario: No translucent gray helpers remain

- **WHEN** the final stylesheet is inspected
- **THEN** no rule uses `rgba(127, 127, 127, …)` as a fill, border, or text color
- **AND** all neutral surfaces resolve to one of the declared neutral tokens

### Requirement: Typography System

The application SHALL adopt Inter Variable as its UI typeface (`--font-ui`) and JetBrains Mono Variable as its monospace typeface (`--font-mono`). Both fonts SHALL be vendored locally as `woff2` files under `src/assets/fonts/` and declared via `@font-face` in `index.html` with `font-display: swap` and a metric-compatible system fallback stack.

Mono SHALL be used as a *type system* across the chrome — not only for code blocks but also for change identifiers, branch names, file paths, and timestamps — so that these elements line up vertically across rows. Task progress is no longer a textual element in the tree row (it renders as a fill meter — see the *Task Progress Meter* requirement), so it is not part of this mono type system.

#### Scenario: Fonts loaded from local assets, not the network

- **WHEN** the application is started offline
- **THEN** Inter and JetBrains Mono render correctly
- **AND** no network request is made for font files

#### Scenario: Mono applies to identifier-like elements in the tree

- **WHEN** a workspace tree row displays a change ID, branch name, or mtime
- **THEN** that element is rendered in `--font-mono`
- **AND** identifier glyphs of equal character count align vertically across rows

### Requirement: Tree Row Selection Model

A selected row in the workspace tree (and in any other list surface that conforms to the row grammar, such as the settings workspaces list) SHALL render a 2px solid `--accent` left border and no background change relative to its unselected state. Hover state SHALL render `background: var(--surface-2)` uniformly on every row, regardless of depth or whether the row is a top-level workspace row. Keyboard focus SHALL render an `outline: 2px solid var(--accent)` with `outline-offset: -2px`.

The previous selection treatment that composed an `--accent-tint` background fill MUST NOT be used; the 2px accent left bar is the sole selection signal. The previous hover treatment that composed `--surface-2` over a workspace tint via `background-blend-mode: multiply` is no longer applicable because top-level rows no longer carry a tint background (see the `spec-browser` capability's `Top-Level Row Display Name and Swatch` requirement); hover SHALL render `var(--surface-2)` on every row, no composition.

#### Scenario: Selected row in the tree

- **WHEN** the user clicks an unselected tree row
- **THEN** the row renders a 2px left bar in `--accent`
- **AND** the row background does not change relative to the unselected state — every row keeps the default row background regardless of depth

#### Scenario: Hover does not borrow selection styling

- **WHEN** the user hovers over an unselected tree row
- **THEN** the row background is `--surface-2` on every row, regardless of depth or whether the row is a top-level workspace row
- **AND** the row does not render an accent left bar

### Requirement: Outlined Chip Badges

Status badges (e.g., `DIVERGED`, branch-name labels, change-id labels) SHALL render as outlined chips with `border: 1px solid <color>`, `background: transparent`, `text-transform: uppercase`, `letter-spacing: 0.05em`, `font-family: var(--font-mono)`, and `font-size: var(--text-xs)`. The previous tinted-fill pill style for `row-divergence-*` MUST NOT be used.

Where horizontal space is tight, a status indicator MAY collapse to a 4px circular dot rendered in the same color (`--warn` for problem states, `--ok` for healthy states). A dot indicator SHALL always carry a `title` attribute with the full label for hover disclosure and accessibility.

The task-progress meter (see the *Task Progress Meter* requirement) is the single sanctioned exception to the "outlined, never filled" vocabulary: it renders a fill *inside* an outlined track. No other chip, badge, or status indicator SHALL use a filled background.

Missing-artifact rows are NOT represented as outlined chips; they follow the dim-row treatment defined in the spec-browser capability (see *Artifact Row Presence Treatment*) and the *Dim Row Style for Missing Artifacts* requirement below.

#### Scenario: Divergence dot replaces a chip in dense rows

- **WHEN** divergence is indicated in a dense row where a full chip would not fit
- **THEN** a 4px circular dot in `--warn` (for diverged) or another status color is rendered instead
- **AND** the dot's container exposes a `title` attribute carrying the human-readable status label

#### Scenario: Progress meter is the only filled row element

- **WHEN** a tree row renders the task-progress meter alongside outlined chips (change-id, divergence) and status dots
- **THEN** the meter is the only element with a filled (non-transparent) background, and that fill is contained inside its outlined track
- **AND** every other chip, badge, and dot in the row remains outlined or transparent-backed

### Requirement: Task Progress Meter

Task progress in workspace-tree rows SHALL be rendered as a fixed-width fill meter rather than as a textual `completed/total` count. The meter SHALL consist of an outlined track — `border: var(--border-width) solid var(--border)`, transparent background, `border-radius: var(--radius-sm)`, a fixed inline width (≈56px), and a small fixed block height (≈4–6px) — containing a fill element rendered in `--ok` whose inline width is `completed / total` of the track, clamped to `[0, 1]`. The meter SHALL render no inline digits.

The exact count SHALL be exposed non-visually rather than dropped: the meter SHALL carry `role="progressbar"` with `aria-valuemin` `0`, `aria-valuemax` equal to the total task count, `aria-valuenow` equal to the completed task count, and a `title` (and matching `aria-label`) of the form "N of M tasks".

The meter SHALL NOT be rendered when the change has no parseable tasks (`total === 0`), and SHALL NOT be rendered at full completion (`completed === total`). At full completion the consuming row surfaces its trailing `✓` glyph in place of the meter (see the spec-browser *Change-Row Completion Glyph* and *Tasks Artifact Node Progress* requirements), so the meter only ever depicts genuinely-in-progress work.

The fill width MAY animate via a CSS `transition` so that a watcher-driven completion change visibly nudges the bar. This transition SHALL be disabled under `@media (prefers-reduced-motion: reduce)`.

#### Scenario: In-progress meter is an outlined track with a green fill

- **WHEN** the meter is rendered for a change with at least one incomplete task
- **THEN** it renders an outlined transparent-background track with an inner fill in `--ok`
- **AND** the fill's inline width is proportional to `completed / total`
- **AND** no digits are rendered inside the meter

#### Scenario: Count is exposed via aria and tooltip, not digits

- **WHEN** the meter is rendered
- **THEN** its element carries `role="progressbar"` with `aria-valuenow` = completed, `aria-valuemax` = total
- **AND** hovering the meter discloses a `title` of the form "N of M tasks"

#### Scenario: Meter is omitted at zero tasks and at full completion

- **WHEN** a change has no parseable tasks (`total === 0`)
- **THEN** no meter is rendered
- **WHEN** a change has every task complete (`completed === total`, `total > 0`)
- **THEN** no meter is rendered and the consuming row shows its trailing `✓` glyph instead

#### Scenario: Reduced motion disables the fill transition

- **WHEN** the OS reports `prefers-reduced-motion: reduce`
- **THEN** the meter's fill width changes without an animated transition

### Requirement: Dim Row Style for Missing Artifacts

When the spec-browser capability indicates that a tree row represents a missing artifact, the row SHALL be rendered with a uniform opacity reduction applied to the entire row contents (label, chevron-spacer, and any other row chrome) rather than via a coloured chip, badge, or replacement icon. The opacity reduction SHALL resolve from a single design token so that the dim treatment is consistent across light and dark schemes and across tinted and untinted rows.

- The dim token SHALL resolve to `0.45` in the default theme.
- The dim treatment SHALL NOT alter the row's foreground colour, font, or layout footprint — only its opacity.
- The dim treatment SHALL compose under hover and selection states: an interactive row that becomes dim simultaneously loses pointer interactivity (per the spec-browser capability), so the hover and selection styles are never observed against a dim row in practice; the visual design need not paint a "dim + hovered" combined state.

#### Scenario: Missing artifact row uses the dim opacity token

- **WHEN** an artifact row is rendered for a missing artifact
- **THEN** the row's computed opacity is `0.45`
- **AND** no foreground colour shift, font change, or layout change is applied relative to a non-missing artifact row at the same depth

#### Scenario: Dim treatment composes consistently across tints

- **WHEN** a missing artifact row appears under a tinted top-level workspace row
- **THEN** the dim treatment is the same opacity reduction as for a missing artifact row under an untinted workspace
- **AND** the tint of the parent top-level row is unaffected (the tint is applied at top level only; the dimmed child row continues to sit on the default child-row background, now muted by the opacity reduction)

### Requirement: Uniform Row Grammar Across List Surfaces

All list-row-like surfaces in the application — including but not limited to the workspace tree row, the settings workspaces list row, and any future archived-changes list — SHALL share a common row template: same vertical padding, same horizontal padding, the same divider treatment between rows, and the same hover/selected states defined by the selection model requirement.

#### Scenario: Settings workspaces row matches tree row grammar

- **WHEN** the settings view renders a registered-workspace row
- **THEN** the row uses the same vertical padding, divider color, hover background, and selection border treatment as a workspace tree row

### Requirement: List-Row Vertical Rhythm Tuned for 4K @ 100%

The vertical padding of list-row-like surfaces SHALL be set so that rows read comfortably on a 4K display at 100% OS scale (one CSS px = one device px) without losing the dense-browser character of the sidebar. Specifically:

- The workspace tree row (`.tree-row`) SHALL use 5px vertical padding (top and bottom).
- The settings workspaces row (`.workspace-row`) SHALL use `--space-4` (16px) vertical padding.
- The settings toggle row (`.settings-toggle-row`) SHALL use 6px vertical padding.

The horizontal padding of these rows is unchanged by this requirement; only vertical rhythm is constrained.

#### Scenario: Tree row breathes at the retuned padding

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `padding-top` and `padding-bottom` are both 5px
- **AND** the row's horizontal padding values are unchanged from the existing layout

#### Scenario: Settings workspaces row tracks the tree row

- **WHEN** a registered-workspace row is rendered in settings
- **THEN** the row's computed vertical padding resolves from `--space-4`
- **AND** the row does not feel visually tighter than a sidebar tree row at the same display scale

### Requirement: Inline SVG Icon Set

The application SHALL replace the placeholder text glyphs `▸`, `▾`, `●`, `✕` with hand-rolled inline SVG components exported from `src/components/icons.tsx`. The set SHALL include, at minimum: `ChevronRight`, `ChevronDown`, `Settings`, `Close`, and `Dot` (filled and outlined variants). Icons SHALL accept `width` and `height` props (default 14px), use `currentColor` for `fill` or `stroke`, and use a consistent `stroke-width` of 1.5 for outlined glyphs. No third-party icon library SHALL be added.

#### Scenario: Chevron in tree rows is an SVG

- **WHEN** a tree row with children renders its disclosure indicator
- **THEN** the indicator is an inline `<svg>` from `icons.tsx`
- **AND** no `▸` or `▾` text character appears in the rendered DOM

### Requirement: macOS Hidden Inset Titlebar Layout

On macOS, the main application window SHALL use a hidden / overlay titlebar so that the system traffic lights float over the top-left of the sidebar. The sidebar background on every platform — including macOS — SHALL render `var(--surface)` via the application stylesheet; no platform-specific transparent fallback or operating-system vibrancy effect is applied beneath the sidebar.

The top of the sidebar SHALL reserve `--space-6` (32px) of safe-area padding on macOS so that traffic-light buttons do not overlap interactive content. The application SHALL provide an explicit drag region across the top 32px of the window on macOS so that the hidden inset titlebar remains draggable; the drag region MAY be either a `data-tauri-drag-region` element or an explicit `getCurrentWindow().startDragging()` call wired to mousedown. The `core:window:allow-start-dragging` permission SHALL be present in the Tauri capabilities ACL so the IPC drag call is allowed.

On Windows and Linux, the operating system's default titlebar SHALL be used. The sidebar background SHALL be `var(--surface)`, matching macOS.

#### Scenario: macOS sidebar renders a solid surface background

- **WHEN** the application launches on macOS
- **THEN** the sidebar element's computed background resolves to `var(--surface)`
- **AND** no `NSVisualEffectView` / `window-vibrancy` material is applied to the main window
- **AND** the traffic-light buttons still appear inset over the sidebar's top-left

#### Scenario: macOS sidebar reserves traffic-light safe area

- **WHEN** the application launches on macOS
- **THEN** the `.split-pane-left` element has top padding of `--space-6` (32px)
- **AND** the first sidebar row clears the traffic-light buttons

#### Scenario: Window draggable from the titlebar strip on macOS

- **WHEN** the user presses and holds the primary mouse button anywhere in the top 32px of the window on macOS, outside the settings-toggle button
- **THEN** the window enters native drag mode
- **AND** moving the mouse moves the window

#### Scenario: Windows and Linux render solid chrome

- **WHEN** the application launches on Windows or Linux
- **THEN** the sidebar background is `var(--surface)`
- **AND** the operating system's default titlebar is used

### Requirement: Markdown Body Adopts the Type System

The markdown view in the detail pane SHALL render body text in `--font-ui` (Inter) at `--text-lg` (16px) with `--leading-prose` (1.65) line-height, inline code in `--font-mono` with the outlined-chip treatment (`border: 1px solid var(--border)`, transparent background, `--radius-sm`), and fenced code blocks in `--font-mono` with `--leading-code` (1.5). The maximum content width SHALL be 880px — chosen so that body prose at `--text-lg` (16px) renders roughly 100 characters per line, and fenced code blocks at `--text-md` (15px mono) render roughly 97 characters per line. This balances a single-column prose measure with sufficient horizontal room for code-block content typical of OpenSpec proposals and specs on a 4K display at 100% OS scale.

Markdown-rendering changes beyond typography (callouts, anchor links, custom code-block chrome) are explicitly out of scope and MUST NOT be introduced by this change.

#### Scenario: Body text uses Inter at the prose size

- **WHEN** the detail pane renders any markdown paragraph
- **THEN** the paragraph computed `font-family` is `--font-ui`
- **AND** the computed `font-size` is `--text-lg`
- **AND** the computed `line-height` is `--leading-prose`

#### Scenario: Inline code is an outlined chip

- **WHEN** the detail pane renders a `<code>` element that is not inside a `<pre>`
- **THEN** the element renders with a 1px `--border` outline
- **AND** the element background is transparent
- **AND** the element font-family is `--font-mono`

### Requirement: Theme Follows System Preference

The application SHALL follow the operating system's `prefers-color-scheme` for light/dark theming and SHALL NOT expose an in-app theme override toggle. `:root` SHALL declare `color-scheme: light dark` so that native scrollbars and form controls also adapt to the system theme.

#### Scenario: System dark mode switches the app

- **WHEN** the operating system theme switches from light to dark while the app is running
- **THEN** the application updates its neutral tokens to the dark scheme
- **AND** the accent and status tokens remain unchanged

#### Scenario: No theme toggle in settings

- **WHEN** the user opens the Settings view
- **THEN** no control to override the OS theme is presented

### Requirement: Flat Tree Row Geometry

The workspace tree row (`.tree-row`) SHALL render without a `border-radius` and without an inline-axis margin (no side gutter between the row and the sidebar edge). The row's tint background, hover background, and selection left bar SHALL therefore fill the row edge-to-edge across the full sidebar width.

This geometry SHALL apply uniformly to every tree row regardless of depth or tint state, so tinted top-level rows and untinted child rows share the same horizontal footprint and the row grammar remains uniform across the tree.

The existing 2px inline-start transparent border (used to reserve space for the selection bar so selected and unselected rows do not shift horizontally) SHALL be preserved — only the corner radius and outer inline margin are removed.

#### Scenario: Tree row renders edge-to-edge

- **WHEN** a workspace tree row is rendered
- **THEN** the row's computed `border-radius` is `0`
- **AND** the row's computed inline-axis margin (left and right) is `0`
- **AND** the row's tint or hover background extends from the sidebar's inline-start edge to its inline-end edge without any gutter

#### Scenario: Tinted and untinted rows share row geometry

- **WHEN** a tinted top-level workspace row is rendered above an untinted child row
- **THEN** both rows resolve to the same `border-radius` (`0`) and the same inline margin (`0`)
- **AND** the visible difference between them is only the tint background on the parent, not the row footprint

