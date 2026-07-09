## ADDED Requirements

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every other fenced code block SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

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
