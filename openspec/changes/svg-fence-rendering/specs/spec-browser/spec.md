# spec-browser — Delta for svg-fence-rendering

## ADDED Requirements

### Requirement: SVG Fence Rendering

The detail pane SHALL render a fenced code block whose info string is `svg` as an image rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability — including `xml` — SHALL continue to render as syntax-highlighted source, unchanged; the `mermaid` info string remains governed by the *Mermaid Diagram Rendering* requirement. Image rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `svg` fences as code text.

The fence body SHALL be presented through an image context (an `<img>` element whose source is derived from the fence body) so that active content is structurally impossible: scripts, event handlers, and references to external resources appearing in the fence body SHALL NOT execute or load. The renderer SHALL NOT inject the fence body into the host document's live DOM.

The fence body SHALL be validated as an SVG document before display. A fence whose body is not well-formed SVG SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the image could not be rendered, SHALL NOT blank or crash the pane, and the rest of the artifact SHALL render normally. The same source fallback SHALL apply if the image context itself fails to load the derived source.

A valid fence body SHALL be normalized before display, and only in the following ways:

- A missing `xmlns` declaration on the root `svg` element SHALL be injected (it is mandatory for a standalone SVG document but routinely omitted by authors), and its absence alone SHALL NOT be treated as invalid SVG.
- When the root element lacks usable absolute `width` AND lacks usable absolute `height` — both must be missing or unusable, not merely one — but declares a `viewBox`, the width and height SHALL be derived from the viewBox extents at one user unit per CSS pixel; the displayed image SHALL be capped at the pane's content width while preserving its aspect ratio. When exactly one of `width` or `height` is authored and usable, both SHALL be left as authored: the image context SHALL derive the missing dimension from the viewBox ratio natively.
- When the root `svg` element does not already declare a `color`, the application's text design token (see the *Design Token Layer* requirement in the `visual-identity` capability) SHALL be set as the root's `color`, so that `currentColor` occurrences resolve to it through ordinary CSS inheritance within the image document; when the operating system colour scheme changes while such an image is visible, it SHALL re-render with the newly active token. A `color` the author declared — on the root or any descendant — SHALL take precedence, and the fence body SHALL NOT otherwise be rewritten.

Colours the author wrote explicitly SHALL NOT be altered: the renderer SHALL NOT invert, matte, or otherwise repaint fence content for the active scheme beyond the root `color` injection above. When the SVG document contains a root-level `<title>` element, its text SHALL be used as the image's alternative text; otherwise a generic alternative text SHALL identify the image as an embedded SVG.

#### Scenario: A valid svg fence renders as an image

- **WHEN** an artifact contains a fenced code block with the `svg` info string and a well-formed SVG body
- **THEN** the detail pane renders it as an image
- **AND** the raw SVG source text is not shown

#### Scenario: Other fenced code blocks are unaffected

- **WHEN** an artifact contains a fenced code block in another language (for example `xml` or `rust`)
- **THEN** it renders as syntax-highlighted source as before
- **AND** it is not treated as an image

#### Scenario: An invalid svg fence degrades to source

- **WHEN** an artifact contains an `svg` fence whose body is not well-formed SVG
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the image could not be rendered
- **AND** the rest of the artifact still renders

#### Scenario: Fence content cannot inject active content

- **WHEN** an `svg` fence body contains a script element, an event-handler attribute, or a reference to an external resource
- **THEN** no script executes and no external resource is fetched
- **AND** the fence body is not inserted into the host document's live DOM

#### Scenario: A naïve fence still renders correctly

- **WHEN** an `svg` fence body omits the `xmlns` declaration and declares only a `viewBox` with no `width` or `height`
- **THEN** it renders as an image sized from the viewBox extents
- **AND** it is not treated as invalid SVG

#### Scenario: currentColor follows the active colour scheme

- **WHEN** an `svg` fence body uses `currentColor` for fills or strokes without declaring its own `color`
- **THEN** those fills and strokes render with the application's text design token
- **AND** when the operating system switches between light and dark while the image is visible, it re-renders with the newly active token
- **AND** colours the author wrote explicitly — including `currentColor` resolved under an author-declared `color` — are unchanged in both schemes

## MODIFIED Requirements

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A `mermaid` fence whose content is not valid diagram source SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the diagram could not be rendered, SHALL NOT blank or crash the pane, and SHALL NOT surface the diagram engine's own error graphic. The rest of the artifact SHALL render normally. Diagram rendering SHALL run under a strict security posture so that diagram source cannot inject active content (scripts or click-through handlers) into the application.

#### Scenario: A valid mermaid fence renders as a diagram

- **WHEN** an artifact contains a fenced code block with the `mermaid` info string and valid diagram source
- **THEN** the detail pane renders it as a graphical diagram
- **AND** the raw mermaid source text is not shown

#### Scenario: Other fenced code blocks are unaffected

- **WHEN** an artifact contains a fenced code block in another language (for example `rust` or `ts`)
- **THEN** it renders as syntax-highlighted source as before
- **AND** it is not treated as a diagram

#### Scenario: An invalid mermaid fence degrades to source

- **WHEN** an artifact contains a `mermaid` fence whose content is not valid diagram source
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the diagram could not be rendered
- **AND** the rest of the artifact still renders
- **AND** the diagram engine's default error graphic is not shown

#### Scenario: Diagrams follow the design tokens and colour scheme

- **WHEN** a diagram is rendered
- **THEN** its colours and font derive from the application's design tokens rather than the diagram engine's stock palette
- **AND** when the operating system switches between light and dark while the diagram is visible, the diagram re-renders with the active scheme's tokens

#### Scenario: Diagram source cannot inject active content

- **WHEN** a `mermaid` fence contains content that attempts to embed a script or a click-through handler
- **THEN** the rendered diagram contains no active content
- **AND** no script from the diagram source executes
