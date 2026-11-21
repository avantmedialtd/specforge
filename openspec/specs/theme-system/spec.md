# theme-system Specification

## Purpose
Defines the selectable appearance of the SpecForge desktop application and browser skin: the five themes (Forge, the default, plus Blueprint, Folio, Mycelium and Avant) and the scheme choice (System, Light, Dark); how the pair is stored, reaches every open window and served tab, and is in effect on the first paint; the rule that one component tree serves every theme; the legibility floors every theme is tested against; and how token-derived renderings follow a change. The Mycelium theme's own values are specified by the `visual-identity` capability, the control that selects the appearance by `settings-view`, and the marketing site's fixed use of Forge by `marketing-site`. The terminal frontend is out of scope.
## Requirements
### Requirement: The Theme Is a Selectable Preference

The desktop application and the browser skin SHALL offer exactly five themes — **Forge**, **Blueprint**, **Folio**, **Mycelium** and **Avant** — and SHALL render the whole interface in exactly one of them at a time. Forge SHALL be the default: an installation with no stored theme SHALL render in Forge.

Each theme SHALL supply both a light and a dark scheme. The themes SHALL be distinguishable by more than colour:

| Theme | Identity |
|---|---|
| Forge | warm graphite neutrals, an ember-orange accent, the Geist and Geist Mono typefaces, flat surfaces with no shadow or glow |
| Blueprint | drafting-blue neutrals, a sky-blue accent, IBM Plex Sans and IBM Plex Mono, square corners, uppercase mono section labels |
| Folio | true-grey neutrals, a violet accent, documents set in the Newsreader serif with chrome in Instrument Sans |
| Mycelium | the indigo identity specified by the `visual-identity` capability, at Forge's sizes and spacing, with Inter for the interface and Meslo for code |
| Avant | Avant Media's editorial look: untinted black-and-white neutrals, a hot-pink accent kept for what can be acted on, square corners, unblurred offset shadows, uppercase mono section labels, the two top document heading levels in Archivo Black capitals, Inter for the interface and prose and JetBrains Mono for code, at Forge's sizes and spacing |

In every theme no text SHALL render smaller than 13px.

Avant SHALL NOT rotate or skew any element, SHALL NOT stack text shadows, and SHALL NOT fill text or surfaces with a gradient.

The theme is a property of the application, not of a workspace, a window or a document: every surface of every window SHALL render in the selected theme.

#### Scenario: A fresh installation opens in Forge

- **WHEN** the application starts with no stored theme
- **THEN** the interface renders in the Forge theme

#### Scenario: Choosing a theme restyles the whole interface

- **WHEN** the user selects Blueprint
- **THEN** the sidebar, the reading surface, the diff view, the pull-request view, the dashboard and the Settings view all render in Blueprint
- **AND** no surface remains in the previous theme

#### Scenario: Every theme holds a minimum text size

- **WHEN** any surface is rendered in any of the five themes
- **THEN** no text on it is smaller than 13px

#### Scenario: Mycelium keeps the indigo identity

- **WHEN** the user selects Mycelium
- **THEN** the interface renders with the colours, depth and treatments the `visual-identity` capability specifies
- **AND** with Forge's type sizes and spacing, and Meslo as its monospaced typeface

#### Scenario: Avant carries the brand's ink and edges

- **WHEN** the user selects Avant in the dark scheme
- **THEN** the interface renders on a near-black ground with white text, square corners and a hot-pink accent
- **AND** a document's first- and second-level headings are set in Archivo Black capitals

#### Scenario: Avant stays a reading surface

- **WHEN** a document is rendered in Avant
- **THEN** its type sizes, line heights and spacing are Forge's
- **AND** no element is rotated or skewed, no text carries a stacked shadow, and nothing is filled with a gradient

### Requirement: The Scheme Is a Selectable Preference

The application SHALL offer three scheme choices — **System**, **Light** and **Dark** — with System the default. Under System the interface SHALL render in the light or dark scheme the operating system reports and SHALL follow a change to that report while running. Under Light or Dark the interface SHALL render in that scheme regardless of the operating system.

The scheme choice is independent of the theme: every theme SHALL honour every scheme choice. Native scrollbars and form controls SHALL follow the resolved scheme, and in the desktop application the native window chrome of every window SHALL follow it as well.

#### Scenario: System follows the operating system

- **WHEN** the scheme is System and the operating system switches from light to dark while the application is running
- **THEN** the interface switches to the active theme's dark scheme without a restart

#### Scenario: An explicit scheme overrides the operating system

- **WHEN** the scheme is Light and the operating system reports dark
- **THEN** the interface renders in the active theme's light scheme
- **AND** a later change in the operating system's setting does not alter it

#### Scenario: Native controls follow the override

- **WHEN** the scheme is Dark and the operating system reports light
- **THEN** native scrollbars and form controls render dark
- **AND** in the desktop application the titlebar of a detached window renders dark

### Requirement: The Appearance Persists and Reaches Every Window

The theme and the scheme together form the *appearance*. The appearance SHALL be stored by the shared settings store, SHALL survive a restart, and SHALL be the same for the desktop application and for a browser tab served by the same installation.

A successful change SHALL reach every window already open in the running application — the main window, reader windows, pull-request windows and image windows — and every browser tab that application serves, without a reload. A separately running instance sharing the same settings SHALL adopt the stored appearance the next time it starts or reads its settings, as it does the reading width (see the *A Reading-Width Change Reaches Windows Already Open* requirement in the `document-width` capability); it is not notified while running. A change that cannot be persisted SHALL be reported on the control the user operated, and the interface SHALL return to the stored appearance.

A stored theme or scheme the application does not recognise SHALL be treated as the default for that value, and SHALL NOT prevent the rest of the settings from loading.

#### Scenario: The appearance survives a restart

- **WHEN** the user selects Folio and Dark, quits the application and starts it again
- **THEN** the interface renders in Folio's dark scheme

#### Scenario: An open reader window adopts a change

- **WHEN** a reader window is open and the user selects Blueprint in the main window's Settings
- **THEN** the reader window renders in Blueprint without being reloaded

#### Scenario: A browser tab adopts a change made on the desktop

- **WHEN** the browser skin is open in a tab and the theme is changed in the desktop application of the same installation
- **THEN** the tab renders in the new theme without being reloaded

#### Scenario: A failed write restores the stored appearance

- **WHEN** the user selects a theme and the settings write fails
- **THEN** the failure is reported beside the theme control
- **AND** the interface renders in the previously stored theme

#### Scenario: An unrecognised stored theme degrades to the default

- **WHEN** the settings file holds a theme name the application does not recognise
- **THEN** the interface renders in Forge
- **AND** every other setting in the file is honoured

### Requirement: The Appearance Is In Effect On the First Paint

A window SHALL paint its first frame in the appearance last known to that browser profile or webview, rather than painting the default and then switching. When the authoritative stored appearance differs from that last-known value — because another instance changed it while this window was closed — the window SHALL adopt the stored appearance as soon as it has been read.

A window in which no appearance could be applied at all SHALL render in Forge, in the scheme the operating system reports.

#### Scenario: A cold start does not flash the default

- **WHEN** the stored theme is Mycelium and the application is started
- **THEN** the first painted frame is in Mycelium
- **AND** no frame is painted in Forge

#### Scenario: A stale last-known value is corrected

- **WHEN** the theme was changed to Folio by another instance while this window was closed, and this window is then opened
- **THEN** the window renders in Folio once the stored appearance has been read

#### Scenario: A window with no applied appearance still renders

- **WHEN** the appearance could not be applied to a window
- **THEN** the window renders in Forge, light or dark as the operating system reports

### Requirement: One Component Tree Serves Every Theme

A theme SHALL be expressed entirely in stylesheets. The application SHALL render the same components and the same markup under every theme, and SHALL NOT select a component, a branch of markup, or a layout according to the active theme.

Within that constraint a theme MAY restyle anything a stylesheet can reach — colours, typefaces, type sizes, corner radii, depth, the selected-row treatment, hover and focus treatments, label casing, and the shapes of task marks, chips and tabs. Structure that is not a matter of identity — layout, scrolling and overflow behaviour, pointer and touch behaviour, and the reading-width ladder of the `document-width` capability — SHALL be the same in every theme.

A theme's rules SHALL apply to any element subtree that names that theme, so that a fragment of the interface can be rendered in a theme other than the active one.

#### Scenario: The same markup under two themes

- **WHEN** the same view is rendered first in Forge and then in Blueprint
- **THEN** the rendered element tree is identical in both
- **AND** only computed styles differ

#### Scenario: Behaviour does not vary by theme

- **WHEN** the user performs the same interaction — selecting a row, resizing a pane, choosing a reading width — in any two themes
- **THEN** the outcome is the same in both

#### Scenario: A fragment can render in another theme

- **WHEN** an element inside a Forge window is marked as belonging to Folio
- **THEN** that element and its descendants render with Folio's tokens and rules
- **AND** the rest of the window remains in Forge

### Requirement: Every Theme Meets the Same Legibility Floors

Every theme, in both schemes, SHALL satisfy the same contrast floors:

- Body text, informational secondary text and inline code SHALL reach at least 4.5:1 against every surface on which the theme places them, including the hovered-row and selected-row surfaces.
- The label of a filled primary button SHALL reach at least 4.5:1 against its fill.
- An edge that is the only signal of a control's boundary, and the keyboard focus indicator, SHALL reach at least 3:1 against the surfaces adjacent to them.
- Every syntax-highlight colour that carries code SHALL reach at least 4.5:1 on the code surface and on the added and removed line tints of the diff view; comment colours are exempt on the tints only.

For a foreground of relative luminance $$L_1$$ and a background of relative luminance $$L_2$$ with $$L_1 \ge L_2$$, the ratio is

$$\frac{L_1 + 0.05}{L_2 + 0.05}$$

Colours a theme uses to distinguish states from one another — the accent, the "done" colour, and the error and removal colours — SHALL differ in lightness as well as hue: any two of them SHALL differ by at least 8 units of CIE lightness, $$|L^*_a - L^*_b| \ge 8$$.

Forge, Blueprint, Folio and Avant SHALL meet every floor above with no exception. Mycelium's colours are those the application shipped before themes existed and are deliberately not retuned (see the *Pinned Values Describe the Mycelium Theme* requirement in the `visual-identity` capability); it falls short on a fixed set of pairs, and that set SHALL be recorded in the automated test as a closed list. The list SHALL NOT grow: a Mycelium value that newly falls below a floor SHALL fail the test, and so SHALL a recorded shortfall that no longer exists, so that the list always states exactly where Mycelium stands.

These floors SHALL be verified by an automated test that reads each theme's own token values, so that a theme whose values fall below a floor fails the test suite.

#### Scenario: A theme below a floor fails the suite

- **WHEN** a theme's secondary-text token is changed to a value below 4.5:1 on that theme's surface
- **THEN** the automated contrast test fails and names the theme, the scheme and the pair

#### Scenario: The three new themes pass in both schemes

- **WHEN** the automated contrast test runs against Forge, Blueprint and Folio in light and in dark
- **THEN** every required pair in all six palettes meets its floor

#### Scenario: Avant passes in both schemes

- **WHEN** the automated contrast test runs against Avant in light and in dark
- **THEN** every required pair in both palettes meets its floor

#### Scenario: Mycelium's shortfalls are a closed list

- **WHEN** the automated contrast test runs against Mycelium in light and in dark
- **THEN** the pairs below a floor are exactly the recorded ones
- **AND** a Mycelium value changed so that a further pair falls below a floor fails the test

#### Scenario: The accent is not mistaken for an error

- **WHEN** Forge's accent and Forge's removal colour are rendered side by side in either scheme
- **THEN** they differ by at least 8 units of CIE lightness as well as in hue

#### Scenario: Avant's pink is not mistaken for an error

- **WHEN** Avant's accent and Avant's removal colour are rendered side by side in either scheme
- **THEN** they differ by at least 8 units of CIE lightness as well as in hue

### Requirement: Token-Derived Renderings Follow the Active Appearance

Everything the application draws from token values at run time SHALL be redrawn when the theme changes or the resolved scheme changes, so that no part of a window is left in the previous appearance. This covers at least rendered diagrams, embedded SVG that inherits the text colour, mathematical notation, the dashboard's contribution heatmap, and syntax highlighting in documents and diffs.

A redraw caused by an appearance change SHALL preserve the reader's place: scroll position, and the scale and position of a maximised figure.

In the browser skin, the colour the browser is told to use for its own chrome SHALL follow the active theme's background in the resolved scheme.

#### Scenario: A visible diagram follows a theme change

- **WHEN** a rendered diagram is visible and the user selects another theme
- **THEN** the diagram is redrawn with the new theme's colours and typeface

#### Scenario: A maximised figure keeps its place

- **WHEN** a diagram is maximised and enlarged, and the theme is changed
- **THEN** the diagram is redrawn in the new theme at the same scale and position

#### Scenario: The heatmap follows the theme

- **WHEN** the dashboard is visible and the user selects another theme
- **THEN** the contribution heatmap is drawn in the new theme's ramp

#### Scenario: The browser chrome colour follows the theme

- **WHEN** the browser skin is rendered in Forge's dark scheme
- **THEN** the theme colour reported to the browser is Forge's dark background

### Requirement: The Terminal Frontend Does Not Apply the Appearance

The terminal frontend SHALL NOT apply the theme or the scheme. It SHALL continue to use its own palette, and a stored appearance SHALL have no effect on it.

#### Scenario: The terminal UI ignores the stored theme

- **WHEN** the stored theme is Blueprint and the terminal frontend is started
- **THEN** it renders with its own palette, unchanged

