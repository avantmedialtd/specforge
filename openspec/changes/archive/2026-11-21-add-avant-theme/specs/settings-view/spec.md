## MODIFIED Requirements

### Requirement: The Appearance Group Offers Theme and Scheme

The Appearance group SHALL offer two choices and a sample:

- **Theme** — one card per theme of the `theme-system` capability, in the order Forge, Blueprint, Folio, Mycelium, Avant. Each card SHALL show the theme's name, a one-line description and a preview rendered in that theme's own tokens and in the currently resolved scheme, so that a card looks like the theme it selects. Exactly one card SHALL be marked as selected, and the marking SHALL NOT rely on colour alone.
- **Scheme** — a choice among System, Light and Dark, with a line of text stating that System follows the operating system.
- **Sample** — a short fragment of document content (a heading, a sentence containing inline code and a link) rendered in the active appearance.

Both choices are choices among fixed values and SHALL persist when changed, under the *Settings Persist by One Rule* requirement. A change SHALL take effect immediately in the Settings view itself and in every other open window (see the *The Appearance Persists and Reaches Every Window* requirement in the `theme-system` capability).

Both choices SHALL be operable from the keyboard, and each card SHALL expose its theme's name and whether it is selected to assistive technology.

#### Scenario: Selecting a theme card applies it at once

- **WHEN** the user activates the Folio card
- **THEN** the Settings view and every other open window render in Folio
- **AND** the Folio card is marked as selected and no other card is

#### Scenario: A card previews its own theme

- **WHEN** the Appearance group is shown while Forge is active
- **THEN** the Blueprint card's preview is rendered with Blueprint's colours, corners and typeface
- **AND** the preview is in the scheme the window is currently resolved to

#### Scenario: Choosing a scheme applies it at once

- **WHEN** the user chooses Dark while the operating system reports light
- **THEN** the interface renders in the active theme's dark scheme
- **AND** the theme card previews are redrawn in their dark schemes

#### Scenario: The selected card is identifiable without colour

- **WHEN** the Appearance group is rendered in greyscale
- **THEN** the selected theme card can still be told apart from the others

#### Scenario: Theme cards are keyboard operable

- **WHEN** the user moves focus to a theme card with the keyboard and activates it
- **THEN** that theme is selected
- **AND** assistive technology is told the card's name and that it is selected

#### Scenario: Avant is the fifth card

- **WHEN** the Appearance group is shown
- **THEN** five theme cards are offered, the last of them Avant
- **AND** the Avant card's preview is rendered with Avant's colours and square corners
