# spec-browser Delta — Maximize Diagrams and SVG Figures

## ADDED Requirements

### Requirement: Maximized Figure View

A figure the detail pane has rendered successfully — a `mermaid` diagram (see the *Mermaid Diagram Rendering* requirement) or an `svg` image (see the *SVG Fence Rendering* requirement) — SHALL be openable in a **maximized view**: a surface presented above the entire application window in which that single figure can be enlarged, reduced, and moved. A fence that degraded to its source, and a diagram whose rendering has not yet completed, SHALL NOT offer the maximized view, because neither has a figure to show.

**Affordance.** Each maximizable figure SHALL present a control that opens the maximized view. The control SHALL be operable by keyboard as well as by pointer (see the *Shell Keyboard Operability* requirement). On a device that reports no hover capability it SHALL be rendered visibly at rest, and on a device whose primary pointer is coarse it SHALL present an enlarged hit area, per the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability. The figure's inline presentation SHALL be unchanged by the presence of the control: a figure still fits the detail pane's width while reading, and the maximized view is an addition to that default rather than a replacement for it.

**Initial scale.** The maximized view SHALL open with the figure fully visible — scaled so that neither dimension exceeds the surface's content area, with the scale taken from whichever axis constrains it more. For a surface of extents $W_v \times H_v$ with padding $p$, displaying content of extents $W_c \times H_c$:

$$s_{\text{fit}} = \min\left(\frac{W_v - 2p}{W_c},\ \frac{H_v - 2p}{H_c}\right)$$

**Zoom.** The maximized view SHALL support continuous zoom by wheel and by two-contact pinch, and SHALL provide explicit controls to return to the fit scale and to display the figure at actual size. Zoom driven by a pointer gesture SHALL be anchored at that pointer: the point of the figure beneath the pointer SHALL remain beneath it as the scale changes. Scale SHALL be bounded — never reduced below the fit scale (or actual size, whichever is smaller) and never increased beyond a fixed ceiling — so the figure can neither be lost in the surface nor enlarged without limit.

**Pan.** While the figure exceeds the surface's content area it SHALL be movable by dragging, and dragging SHALL be driven by pointer input so that a mouse, a touch contact, and a pen all move it through the same path (see the *Drag Interactions Accept Pointer Input* requirement in the `touch-input` capability).

**Fidelity.** An enlarged figure SHALL be re-rendered at the size at which it is displayed, rather than by magnifying a fixed-resolution rendering of it. Enlarging SHALL NOT degrade a figure's sharpness, in either the diagram path or the image path.

**Security posture is preserved.** The maximized view SHALL NOT relax the rendering guarantees of either path. An `svg` fence SHALL continue to be presented through an image context and SHALL NOT be injected into the host document's live DOM at any scale, and a `mermaid` diagram SHALL continue to be rendered under the strict security posture its own requirement specifies. The maximized view SHALL offer no means of editing, exporting, or otherwise writing the figure, consistent with the *Read-Only Viewer* requirement.

**Colour scheme.** While the maximized view is open the figure SHALL follow the active colour scheme exactly as it does inline, re-rendering when the operating system switches between light and dark. That re-render SHALL preserve the current scale and position, so a scheme change does not displace what the reader is looking at.

**Dismissal.** The maximized view SHALL be dismissable by the Escape key, by an explicit close control, and by activating the surface outside the figure. Dismissal SHALL return the reader to the artifact with its scroll position unchanged. Escape SHALL dismiss only the maximized view: any Settings or Archive pane open behind it SHALL remain open, and a second Escape SHALL be required to dismiss that (see the *Archive Entrypoint in Sidebar Footer* and *Settings Entrypoint in Sidebar Footer* requirements).

**The maximized view is ambient view state.** It SHALL NOT be part of the Address, the URL, or navigation history (see the `view-routing` capability), consistent with how side-pane visibility is treated by the *Side-Pane Visibility Toggles* requirement. Navigating to a different artifact SHALL close it. A change to the artifact's content on disk while the view is open SHALL NOT close it: the maximized figure SHALL follow the reparsed content, consistent with the *Reactive Updates from Filesystem* requirement.

#### Scenario: A rendered diagram can be maximized

- **WHEN** the detail pane has rendered a `mermaid` fence as a diagram
- **THEN** a control to maximize that diagram is available on it
- **AND** activating the control opens the diagram in a surface above the application window
- **AND** the diagram is initially shown fully visible within that surface

#### Scenario: A rendered svg image can be maximized

- **WHEN** the detail pane has rendered an `svg` fence as an image
- **THEN** a control to maximize that image is available on it
- **AND** activating the control opens the image in the same maximized surface the diagram path uses

#### Scenario: A degraded fence offers no maximized view

- **WHEN** an artifact contains a `mermaid` fence whose content is not valid diagram source, or an `svg` fence whose body is not well-formed SVG
- **THEN** the fence's raw source is shown with its quiet indication as before
- **AND** no maximize control is offered on it

#### Scenario: Zoom is anchored at the pointer

- **WHEN** the reader zooms in with the pointer resting over a particular node of a maximized diagram
- **THEN** that node remains beneath the pointer as the scale increases
- **AND** the rest of the figure expands around it

#### Scenario: Scale is bounded at both ends

- **WHEN** the reader zooms out repeatedly in the maximized view
- **THEN** the figure stops reducing once it is fully visible and does not shrink further
- **AND** zooming in repeatedly stops at the maximum scale rather than continuing without limit

#### Scenario: An enlarged image stays sharp

- **WHEN** the reader enlarges a maximized `svg` image well beyond its inline size
- **THEN** the image is re-rendered at the displayed size
- **AND** it is not shown as a magnified low-resolution rendering

#### Scenario: An enlarged figure can be moved

- **WHEN** a maximized figure has been enlarged beyond the surface's content area
- **THEN** dragging it moves the visible region
- **AND** a mouse drag, a touch drag, and a pen drag each move it the same way

#### Scenario: Escape dismisses only the maximized view

- **WHEN** the Archive view is open, an artifact is rendered behind it, and a figure in that artifact has been maximized
- **THEN** pressing Escape closes the maximized view
- **AND** the Archive view remains open
- **AND** pressing Escape again closes the Archive view

#### Scenario: Maximizing does not change the address

- **WHEN** the reader maximizes a figure and then dismisses it
- **THEN** the Address, the URL, and the navigation history are unchanged throughout
- **AND** the artifact's scroll position is unchanged when the view is dismissed

#### Scenario: Navigating away closes the maximized view

- **WHEN** a figure is maximized and the reader selects a different artifact in the tree
- **THEN** the maximized view closes
- **AND** the newly selected artifact is rendered in the detail pane with no figure maximized

#### Scenario: A live edit updates the maximized figure without closing it

- **WHEN** a figure is maximized and the artifact's file changes on disk so that the figure's source is reparsed
- **THEN** the maximized view remains open
- **AND** it shows the figure rendered from the new source

#### Scenario: A scheme change preserves scale and position

- **WHEN** a diagram is maximized and enlarged, and the operating system switches between light and dark
- **THEN** the diagram re-renders with the active scheme's design tokens
- **AND** its scale and visible region are unchanged

#### Scenario: Maximizing preserves the image path's inertness

- **WHEN** an `svg` fence whose body contains a script element or an event-handler attribute is maximized
- **THEN** the fence body is still not inserted into the host document's live DOM
- **AND** no script executes and no external resource is fetched at any scale

## MODIFIED Requirements

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement, `math` in the *Mathematical Notation Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. This obligation extends to colours the diagram engine derives on its own for values the application does not map explicitly: the application SHALL inform the engine of the active scheme so that every derived colour is derived in the direction of that scheme, rather than under an assumed light palette. Diagram text SHALL remain legible against every filled surface the engine draws — including alternating table-row fills such as entity-relationship attribute rows, whose fills SHALL come from the design tokens' surface colours. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A rendered diagram SHALL be capped at the detail pane's content width while reading, and SHALL be openable in the maximized view described by the *Maximized Figure View* requirement, so that a diagram scaled down to fit a narrow pane remains readable.

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
- **AND** the diagram engine is informed of the active colour scheme, so colours it derives on its own follow that scheme
- **AND** when the operating system switches between light and dark while the diagram is visible, the diagram re-renders with the active scheme's tokens

#### Scenario: Entity-relationship attribute rows stay legible in the dark scheme

- **WHEN** an artifact contains an `erDiagram` fence whose entities carry attributes and the dark colour scheme is active
- **THEN** every attribute row's fill comes from the design tokens' surface colours
- **AND** the row text remains legible against its row fill
- **AND** no row renders as a near-white fill beneath near-white text

#### Scenario: Diagram source cannot inject active content

- **WHEN** a `mermaid` fence contains content that attempts to embed a script or a click-through handler
- **THEN** the rendered diagram contains no active content
- **AND** no script from the diagram source executes

#### Scenario: A diagram too wide for the pane remains readable

- **WHEN** an artifact contains a diagram whose natural width exceeds the detail pane's content width
- **THEN** it is displayed scaled down to fit the pane, as before
- **AND** a control to open it in the maximized view is available on it

### Requirement: SVG Fence Rendering

The detail pane SHALL render a fenced code block whose info string is `svg` as an image rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability — including `xml` — SHALL continue to render as syntax-highlighted source, unchanged; the `mermaid` and `math` info strings remain governed by the *Mermaid Diagram Rendering* and *Mathematical Notation Rendering* requirements respectively. Image rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `svg` fences as code text.

The fence body SHALL be presented through an image context (an `<img>` element whose source is derived from the fence body) so that active content is structurally impossible: scripts, event handlers, and references to external resources appearing in the fence body SHALL NOT execute or load. The renderer SHALL NOT inject the fence body into the host document's live DOM. This obligation holds at every displayed size, including within the maximized view.

The fence body SHALL be validated as an SVG document before display. A fence whose body is not well-formed SVG SHALL degrade gracefully: the detail pane SHALL present the fence's raw source together with a quiet indication that the image could not be rendered, SHALL NOT blank or crash the pane, and the rest of the artifact SHALL render normally. The same source fallback SHALL apply if the image context itself fails to load the derived source.

A valid fence body SHALL be normalized before display, and only in the following ways:

- A missing `xmlns` declaration on the root `svg` element SHALL be injected (it is mandatory for a standalone SVG document but routinely omitted by authors), and its absence alone SHALL NOT be treated as invalid SVG.
- When the root element lacks usable absolute `width` AND lacks usable absolute `height` — both must be missing or unusable, not merely one — but declares a `viewBox`, the width and height SHALL be derived from the viewBox extents at one user unit per CSS pixel; the displayed image SHALL be capped at the pane's content width while preserving its aspect ratio. When exactly one of `width` or `height` is authored and usable, both SHALL be left as authored: the image context SHALL derive the missing dimension from the viewBox ratio natively.
- When the root `svg` element does not already declare a `color`, the application's text design token (see the *Design Token Layer* requirement in the `visual-identity` capability) SHALL be set as the root's `color`, so that `currentColor` occurrences resolve to it through ordinary CSS inheritance within the image document; when the operating system colour scheme changes while such an image is visible, it SHALL re-render with the newly active token. A `color` the author declared — on the root or any descendant — SHALL take precedence, and the fence body SHALL NOT otherwise be rewritten.

Colours the author wrote explicitly SHALL NOT be altered: the renderer SHALL NOT invert, matte, or otherwise repaint fence content for the active scheme beyond the root `color` injection above. When the SVG document contains a root-level `<title>` element, its text SHALL be used as the image's alternative text; otherwise a generic alternative text SHALL identify the image as an embedded SVG.

A rendered image SHALL be openable in the maximized view described by the *Maximized Figure View* requirement, so that an image capped at the pane's content width remains legible.

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

#### Scenario: A rendered image offers the maximized view

- **WHEN** an artifact contains an `svg` fence that renders as an image
- **THEN** a control to open it in the maximized view is available on it
- **AND** the image's inline size and aspect ratio are unchanged by the presence of that control
