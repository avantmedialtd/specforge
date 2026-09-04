# document-width

## ADDED Requirements

### Requirement: Reading Width Is a Selectable Preference

The application SHALL offer the reader a choice of reading width for the markdown
reading surface, as a named preset drawn from a fixed ladder: `compact`,
`default`, `wide`, and `full`.

The choice SHALL be a single application-wide value. It SHALL NOT be recorded per
document and SHALL NOT be recorded per window — the same reasoning `reader-window`
gives for its shared geometry, that per-document memory accrues an unbounded set
of entries keyed by opaque identifiers with nothing to prune them.

The value SHALL be persisted in the application settings and SHALL survive a
restart. Its default SHALL be `default`, which SHALL reproduce the rendering the
reading surface had before this capability existed, so that no existing
installation's documents move.

The choice SHALL be presented in Settings, and SHALL be presented in **both** the
desktop application and the browser skin. It is not a desktop-only affordance
within the meaning of `web-ui`'s *Desktop-Only Settings Are Hidden in the Web UI*:
it is expressed entirely in the served stylesheet and behaves identically in a
browser tab.

The Settings section SHALL render a sample of body prose and a fenced code block
at the width under consideration, because Settings replaces the document rather
than overlaying it and the reader would otherwise have no way to judge a width
without leaving Settings. The sample SHALL be scoped to itself, so previewing a
width does not apply it to the reading surfaces.

#### Scenario: A chosen width survives a restart

- **WHEN** the reader selects a reading width and the application is restarted
- **THEN** the reading surface renders at the selected width
- **AND** no reading surface renders at the default width first

#### Scenario: An installation that has never chosen renders as before

- **WHEN** the application settings contain no reading-width value
- **THEN** the reading surface renders at the default rung
- **AND** its object column and prose measure are those the surface had before this capability existed

#### Scenario: The browser skin offers the same choice

- **WHEN** Settings is opened in the browser skin
- **THEN** the reading-width choice is present
- **AND** it is not hidden alongside the desktop-only affordances

#### Scenario: Previewing a width does not apply it

- **WHEN** the reader views the Settings sample at a width they have not selected
- **THEN** the sample renders at that width
- **AND** the reading surfaces continue to render at the selected width

### Requirement: The Preset Ladder Moves Both Tiers Together

Each rung SHALL set **both** tiers of the two-tier content column described by
`visual-identity`'s *Markdown Body Adopts the Type System* — the object column and
the prose measure — so that the two keep their relationship at every rung and
neither is stranded by the other:

| Preset | Prose measure | Object column | Prose chars (measured) |
|---|---|---|---|
| `compact` | `50ch` | `720px` | ~65 |
| `default` | `74ch` | `880px` | ~97 |
| `wide` | `86ch` | `1040px` | ~113 |
| `full` | `96ch` | unbounded — see the following requirement | ~125 |

The character counts are informative, not normative, and are recorded because
they cannot be read off the `ch` figures: `ch` is the advance of the digit zero,
which in the prose font is about 1.3× an average prose character, so each rung
renders roughly a quarter more characters than its number suggests.

The narrowest rung SHALL reach a measure inside the range conventionally called
comfortable for continuous reading. Stepping the measure evenly with the column
does not achieve this, so the ladder SHALL NOT be constrained to an even step.

At every rung the prose measure SHALL be bounded, and SHALL NOT exceed the object
column. Across the three bounded rungs the ladder SHALL be monotonic in both
tiers, so that a rung named as wider is wider in both. The prose-to-column ratio
is NOT required to be constant across rungs — `compact` tightens the text
proportionally more than the container, because the reason to select it is the
text.

The tiers SHALL be delivered to the stylesheet as tokens carried on a single
attribute of the document, so one write reaches every reading surface, and SHALL
NOT be applied as per-element inline styles.

The mapping from preset to the two widths SHALL be implemented as a pure function
of the preset, separable from any rendering, so it is testable without a DOM.

#### Scenario: Each rung sets both tiers

- **WHEN** any rung of the ladder is in force
- **THEN** the object column takes that rung's width
- **AND** the prose measure takes that rung's measure

#### Scenario: Prose never exceeds the objects

- **WHEN** any rung is in force
- **THEN** the prose measure is bounded
- **AND** the prose measure is not wider than the object column

#### Scenario: The bounded rungs are ordered

- **WHEN** `compact`, `default` and `wide` are compared
- **THEN** each is wider than the one before it in its object column
- **AND** each is wider than the one before it in its prose measure

### Requirement: The Widest Preset Fills the Surface and Still Bounds Prose

At `full` the object column SHALL be unbounded, taking whatever width the
containing reading surface offers, so that wide content — a table, or a diagram
whose natural width exceeds every bounded rung — renders without being fitted to a
column narrower than the available space.

Prose SHALL remain bounded at `full`, at the measure the ladder gives it. The
widest rung SHALL NOT produce unbounded body text.

Headings SHALL continue to keep the full column at `full`, so the hairline rules
beneath `h1` and `h2` span the reading surface. The headings exemption SHALL NOT
become conditional on the selected width.

`full` SHALL NOT be assumed to be the widest rung in rendered pixels. On a
reading surface narrower than a bounded rung's column — a reader window at its
default geometry, for instance — `full` renders narrower than `default`. The
ladder is an ordering of intent, and only `full`'s result depends on the surface.

#### Scenario: A wide diagram is not fitted to a narrower column

- **WHEN** a diagram whose natural width exceeds every bounded rung is rendered at `full` on a reading surface wider than every bounded rung
- **THEN** the diagram is scaled to the surface rather than to a bounded column
- **AND** it is scaled less than it would be at `default`

#### Scenario: Prose is still bounded at the widest rung

- **WHEN** a long paragraph is rendered at `full` on a reading surface far wider than the prose measure
- **THEN** the paragraph wraps at the rung's measure
- **AND** it does not fill the surface

#### Scenario: The widest rung can render narrower than a bounded one

- **WHEN** `full` is in force in a reading surface narrower than the `default` rung's column
- **THEN** the object column is the surface's width
- **AND** this is narrower than `default` would have produced, which is correct rather than a defect

### Requirement: The Reading Width Applies to Every Reading Surface

The selected width SHALL apply to every surface that renders a markdown document:
the detail pane, a reader window, the archive reader, and the file browser's
preview. It SHALL apply in both the desktop application and the browser skin,
which share one frontend bundle.

The identity header rendered above a document SHALL track the object column at
every rung, so it continues to head the document rather than floating beside it.

#### Scenario: A reader window honours the selected width

- **WHEN** a reader window is opened while a non-default width is selected
- **THEN** its document renders at the selected width

#### Scenario: The file browser preview honours the selected width

- **WHEN** a markdown file is previewed in the file browser while a non-default width is selected
- **THEN** the preview renders at the selected width

#### Scenario: The identity header stays aligned with the column

- **WHEN** any rung is in force
- **THEN** the identity header's width matches the object column of that rung

### Requirement: The Reading Width Is In Effect On the First Paint

A reading surface SHALL render at the selected width from its first frame. It
SHALL NOT render at one width and then reflow to another once a stored value has
been retrieved.

The application settings SHALL remain the authoritative store. Because retrieving
them is asynchronous and the width must be known synchronously before the first
render, the selected width SHALL additionally be mirrored in a store the frontend
can read synchronously at startup, written on every change.

The mirror SHALL be treated as a first-paint hint and never as the source of
truth. Once the authoritative value is available it SHALL be reconciled against
the mirror, so a width changed by another instance of the application corrects
itself rather than persisting.

When no mirror is available — a first run, or a cleared store — the surface SHALL
render at the default rung and SHALL still not reflow.

#### Scenario: A cold launch does not reflow

- **WHEN** the application is launched with a non-default width selected
- **THEN** the first painted frame of the reading surface is at that width
- **AND** the document does not re-lay-out after the settings are read

#### Scenario: A stale mirror is corrected

- **WHEN** the width was changed by another instance since this one last ran
- **THEN** the authoritative value takes effect once read
- **AND** the mirror is updated to match it

#### Scenario: A missing mirror falls back without a flash

- **WHEN** no mirrored value is available at startup
- **THEN** the surface renders at the default rung from its first frame

### Requirement: A Reading-Width Change Reaches Windows Already Open

Changing the reading width SHALL take effect in windows that are already open,
without reopening them.

The change SHALL be announced as a dedicated event rather than as a variant of the
cache-event stream, so that no existing consumer of that stream in any frontend
gains a case it must ignore. The event SHALL be delivered on both the desktop and
the browser event transports.

#### Scenario: An open reader window re-lays out

- **WHEN** the reading width is changed in Settings while a reader window is open
- **THEN** the open reader window's document re-lays out at the new width

#### Scenario: The browser skin receives the change over its event stream

- **WHEN** the reading width is changed while the browser skin is connected
- **THEN** the browser skin's reading surfaces re-lay out at the new width

### Requirement: An Unrecognised Reading Width Degrades to the Default

A stored reading width that the running version does not recognise — a settings
file written by a newer version, or one edited by hand — SHALL be treated as the
default rung.

It SHALL NOT fail the load of the settings as a whole. Settings are loaded by
parsing the file in one piece and falling back to the complete defaults when that
parse fails, so a value that could not be deserialized would silently discard
every other preference stored beside it — favourited changes, the developer
identity, the contributor roster, tree collapse state, the web-server
configuration and the reader-window geometry. All of those SHALL survive an
unrecognised reading-width value intact.

The frontend SHALL apply the same rule to an unrecognised mirrored value.

The tolerance is read-side only. An unrecognised value SHALL NOT be expected to
survive a write: settings are persisted by serializing the whole record, so the
next write of any setting — including one unrelated to the reading width —
replaces it with the default on disk. A reader who selected a rung on a newer
build and then opens an older one therefore loses that selection permanently
rather than temporarily. This is accepted: what the tolerance exists to protect
is the neighbouring settings, and a preference whose degraded state is a legible
default the reader can re-select is not in the same class as the data beside it.

#### Scenario: An unrecognised value does not survive an unrelated write

- **WHEN** a settings file containing an unrecognised reading width is loaded and any other preference is then changed
- **THEN** the stored reading width is the default rung
- **AND** the unrecognised value is no longer present in the file

#### Scenario: An unknown value loads as the default

- **WHEN** the settings file contains a reading width the running version does not recognise
- **THEN** the reading surface renders at the default rung

#### Scenario: Other settings survive an unknown value

- **WHEN** the settings file contains an unrecognised reading width alongside favourited changes, a developer identity and other preferences
- **THEN** the settings load successfully
- **AND** those preferences are unchanged rather than reset to their defaults

### Requirement: The Terminal Frontend Does Not Apply the Reading Width

The terminal frontend renders its detail pane wrapped to the pane's own width and
has no prose measure. It SHALL NOT be required to implement the reading width, and
the absence is deliberate rather than an omission: a measure expressed in
characters of a proportional font has no direct equivalent in terminal cells, and
giving the terminal frontend a measure is a separate change.

The terminal frontend SHALL nonetheless load a settings file carrying a reading
width without error, ignoring the field.

#### Scenario: The terminal frontend ignores the field

- **WHEN** the terminal frontend loads a settings file containing a reading width
- **THEN** it starts successfully
- **AND** its detail pane renders as it did before this capability existed
