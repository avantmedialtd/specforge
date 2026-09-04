## ADDED Requirements

### Requirement: The header marks its off-site link rather than labelling it

The site header's primary navigation contains exactly one link that leaves the
site: the one targeting the SpecForge repository. It SHALL be presented as
GitHub's own mark rather than as a text label, so that it reads as a departure
rather than as a peer of the site's own destinations.

The mark SHALL be rendered as inline vector artwork that inherits the navigation's
existing colour token through `currentColor`, so its resting and hover colours
match the header's other navigation links in both the light and dark themes
without a second asset and without introducing a colour of its own.

Replacing the label SHALL NOT change the navigation's link set, their targets,
their order, or their tab order.

#### Scenario: The repository link renders as a mark

- **WHEN** the header renders on any route
- **THEN** the primary navigation's repository link SHALL contain inline vector
  artwork and SHALL NOT contain a visible text label

#### Scenario: The mark follows the navigation's own colour

- **WHEN** the header renders
- **THEN** the repository link's resting colour SHALL match that of the
  documentation link in the same navigation
- **AND** the mark SHALL take its fill from the link's own colour rather than from
  a colour declared on the artwork

#### Scenario: The navigation is otherwise unchanged

- **WHEN** the header renders
- **THEN** the primary navigation SHALL still offer the documentation link, the
  repository link and the download link, in that order, at the same targets

### Requirement: A control reduced to a glyph keeps its name and its target

Any interactive control in the site chrome whose visible label is a glyph rather
than text SHALL expose a text alternative naming its destination, and the
decorative artwork inside it SHALL NOT contribute to that name. Removing the
visible word SHALL therefore leave the control's announced name unchanged.

Its activation target SHALL measure at least 24×24 CSS pixels, and enlarging that
target SHALL NOT widen the row it sits in. For a glyph of size $s$ with symmetric
padding $p$ and horizontal margin $m$, the activation target and the layout
contribution are

$$w_{\text{hit}} = s + 2p, \qquad w_{\text{layout}} = s + 2p + 2m$$

so the enlargement SHALL be offset — $m = -p$, giving $w_{\text{layout}} = s$ — and
the header's existing spacing SHALL remain valid.

The site's tests SHALL assert the control's accessible name, not only its target,
so that a nameless control fails rather than passing silently.

#### Scenario: The mark's link is announced by name

- **WHEN** the header renders
- **THEN** the repository link's accessible name SHALL be "GitHub"
- **AND** it SHALL be the same name the link carried when it was a text label

#### Scenario: The artwork is hidden from assistive technology

- **WHEN** the header renders
- **THEN** the vector artwork inside the repository link SHALL be hidden from
  assistive technology, so the link is announced once rather than twice

#### Scenario: The activation target meets the minimum

- **WHEN** the header renders
- **THEN** the repository link's activation target SHALL measure at least 24 CSS
  pixels in each dimension

#### Scenario: The enlarged target does not widen the row

- **WHEN** the header renders
- **THEN** the repository link's bounding box combined with its horizontal margins
  SHALL not exceed the width of the glyph itself
- **AND** no page SHALL scroll horizontally at the narrow viewport widths the site
  already guards

#### Scenario: A missing name fails the suite

- **WHEN** the repository link's text alternative is removed
- **THEN** the site's end-to-end suite SHALL fail rather than pass on the link's
  target alone
