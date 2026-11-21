## MODIFIED Requirements

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
