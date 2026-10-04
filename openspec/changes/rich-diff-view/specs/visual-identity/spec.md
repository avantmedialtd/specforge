## MODIFIED Requirements

### Requirement: Syntax Highlight Palette

The syntax-highlight palette SHALL be scheme-aware, and SHALL be one palette for fenced code in the markdown view and for the lines of the diff view (see the `diff-view` capability). Every token class the palette colours (keywords, strings, numbers, types, comments, titles) SHALL clear the AA 4.5:1 contrast floor, in BOTH the light and the dark scheme, against every background it is drawn on: the code well's background, and the diff view's context, added and removed line backgrounds, in the unified and the side-by-side layout alike. The floor SHALL hold for a class coloured by a design token, such as comments at `--text-faint` and titles at `--accent`, exactly as for a class coloured by a literal value. With $$K$$ the palette's token classes, $$B_s$$ the backgrounds above in scheme $$s$$, and $$c_s(k, b)$$ the colour class $$k$$ takes on background $$b$$ in scheme $$s$$:

$$\forall\, s \in \{\text{light},\ \text{dark}\},\ \forall\, k \in K,\ \forall\, b \in B_s:\quad \operatorname{contrast}\big(c_s(k, b),\ b\big) \ge 4.5$$

where $$\operatorname{contrast}$$ is the contrast ratio of a text colour against its background.

Palette colours MAY be literal values (the *Design Token Layer* requirement's syntax-palette carve-out stands), but a literal SHALL then be defined per scheme rather than shared, whenever one value cannot clear the floor on both wells. Where a class's colour, literal or token-driven, cannot clear the floor on every background in $$B_s$$, the diff view SHALL give that class a per-scheme, per-background value on each background where it falls short; such values fall under the same carve-out.

The side-by-side layout's filler cell, the empty cell that a line with no partner faces, SHALL take its background from a neutral design token of its own, distinct in each scheme from each of the context, added and removed line backgrounds. A filler holds no text, so the floor does not apply to it and its background is not in $$B_s$$; that background, with the filler's missing line number, is what tells a filler from an empty line.

#### Scenario: Token colours clear AA on the light code well

- **WHEN** the light scheme is active and the markdown view renders a highlighted fence containing strings, numbers, keywords, types, comments, and titles
- **THEN** each token colour measures at least 4.5:1 against the code well background

#### Scenario: Token colours clear AA on the dark code well

- **WHEN** the dark scheme is active and the markdown view renders the same fence
- **THEN** each token colour measures at least 4.5:1 against the dark code well background

#### Scenario: Token colours clear AA on light diff lines

- **WHEN** the light scheme is active and the diff view renders a highlighted hunk in which a context line, an added line and a removed line each contain strings, numbers, keywords, types, comments, and titles
- **THEN** each token colour measures at least 4.5:1 against the background of the line it is drawn on
- **AND** this holds in the unified layout and in the side-by-side layout

#### Scenario: Token colours clear AA on dark diff lines

- **WHEN** the dark scheme is active and the diff view renders the same hunk
- **THEN** each token colour measures at least 4.5:1 against the background of the line it is drawn on
- **AND** this holds in the unified layout and in the side-by-side layout

#### Scenario: A token-driven class takes its own value where its token falls short

- **WHEN** the diff view draws a comment on an added or removed line whose background `--text-faint` does not clear at 4.5:1 in the active scheme (a removed-line tint such as `#ffebe9` would take it to 4.21:1 in the light scheme)
- **THEN** the comment takes a value for that scheme and that background which measures at least 4.5:1 against it
- **AND** the `--text-faint` token keeps its own value

#### Scenario: The filler cell has its own background

- **WHEN** side by side is in effect, in either scheme, and the diff view renders a change block that removes three lines and adds one blank line
- **THEN** the blank added line faces the first removed line, with its line number, on the added line background
- **AND** the two rows below it show a filler cell in the new column, with no line number, on the filler's own neutral background, which differs from the context, added and removed line backgrounds
- **AND** no contrast floor is measured against the fillers, which hold no text
