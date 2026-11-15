## MODIFIED Requirements

### Requirement: Site carries its own visual identity

The site SHALL carry the visual identity of the SpecForge application's default theme, **Forge** (see the *The Theme Is a Selectable Preference* requirement in the `theme-system` capability): Forge's neutrals and accent in both schemes, and the Geist and Geist Mono typefaces. The identity SHALL be the site's own copy of those values, not a shared design system and not a file shared with the application. It SHALL import no external component library or theme, and SHALL use its own favicons and its own Open Graph image rather than a studio asset.

The site's identity is fixed. It SHALL follow the visitor's operating-system light or dark setting, and SHALL NOT offer a theme control or a scheme control. The application's other themes SHALL NOT appear as the site's own styling; the site MAY show them in screenshots or describe them in documentation.

The site SHALL meet the legibility floors the application's themes meet (see the *Every Theme Meets the Same Legibility Floors* requirement in the `theme-system` capability) in both schemes.

Studio attribution SHALL be understated: a closing "built by Avant Media" of no more than two sentences, linking the studio site, with no services pitch and no additional call to action.

#### Scenario: The stylesheet is self-contained

- **WHEN** the site is built
- **THEN** its stylesheet SHALL resolve entirely from within `site/`
- **AND** the rendered head SHALL reference the site's own icon and Open Graph assets

#### Scenario: Attribution stays understated

- **WHEN** any page renders
- **THEN** studio attribution SHALL be at most two sentences
- **AND** it SHALL NOT present a services offer

#### Scenario: The site renders in Forge

- **WHEN** any page renders
- **THEN** its accent, neutrals and typefaces SHALL be those of the Forge theme
- **AND** its fonts SHALL be served from the site's own origin

#### Scenario: The site follows the visitor's scheme and offers no control

- **WHEN** a visitor whose operating system reports dark opens any page
- **THEN** the page SHALL render in Forge's dark scheme
- **AND** the page SHALL offer no control that changes the theme or the scheme
