## ADDED Requirements

### Requirement: Wide Block Containment

No single block of a rendered artifact SHALL widen the document column or cause the detail pane to scroll horizontally. Every block-level element whose natural width exceeds the content column — a GFM table, a fenced code block, display mathematics, a diagram held at its legibility floor, or an image — SHALL be contained within its own bounds and SHALL scroll horizontally inside those bounds when its content cannot shrink to fit. The prose around such a block SHALL remain fixed in place while the block is scrolled.

Containment SHALL NOT introduce vertical clipping or a vertical scrollbar on the contained block: content that overhangs the block's line box vertically (a summation limit, a subscript, a descender) SHALL remain fully visible.

#### Scenario: A wide table scrolls within its own block

- **WHEN** an artifact contains a GFM table whose columns cannot fit the content column at their readable widths
- **THEN** the table scrolls horizontally within its own block
- **AND** the document column and the detail pane do not scroll horizontally
- **AND** the surrounding prose keeps its position while the table is scrolled

#### Scenario: A contained block never grows a vertical scrollbar

- **WHEN** an artifact contains display mathematics or a table contained by this requirement
- **THEN** the containing block shows no vertical scrollbar
- **AND** no vertical overhang of the content is clipped

## MODIFIED Requirements

### Requirement: Mathematical Notation Rendering

The detail pane SHALL render mathematical notation as typeset formulas using double-dollar and fence delimiters only: a double-dollar-delimited expression (`$$…$$`) standing alone as its own paragraph — in either its single-line or multi-line block form — SHALL render as display (block) mathematics, a double-dollar expression embedded within surrounding prose SHALL render as inline mathematics, and a fenced code block whose info string is `math` SHALL render as display mathematics rather than as syntax-highlighted source. A single-dollar-delimited span (`$…$`) SHALL NOT be treated as mathematics: it SHALL render as literal text, dollar signs included, so prose that mentions dollar amounts can never be silently consumed and re-typeset as a formula. Mathematics rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend SHALL continue to present mathematical source as plain text.

Dollar delimiters SHALL NOT be recognised inside code spans or fenced code blocks (other than the `math` fence itself), so a literal dollar sign in backticked text — for example a `\\wsl$\<distro>` path — is never parsed as mathematics. A dollar sign with no valid closing delimiter SHALL render as a literal dollar sign.

Inline mathematics SHALL render at a size visually harmonized with the surrounding prose: the rendering engine's default enlargement relative to the surrounding font SHALL be overridden so a formula sits on the same optical line as the words around it without inflating the line's height. Display mathematics SHALL NOT be vertically clipped by its own block: limits, subscripts, and descenders SHALL remain fully visible, and the block SHALL NOT display a vertical scrollbar. Display mathematics wider than the pane's content width SHALL scroll horizontally within its own block rather than widening the artifact (see the *Wide Block Containment* requirement).

Rendered mathematics SHALL inherit the surrounding text colour, so it follows the active colour scheme in both light and dark without any repainting or re-rendering machinery. Rendered mathematics SHALL carry a machine-readable representation (MathML) alongside the visual output so assistive technology can consume it. Rendering SHALL work without network access: the mathematics engine and its assets are part of the application bundle.

Invalid input SHALL degrade gracefully and locally: a double-dollar-delimited expression that is not valid mathematical source SHALL present its raw source in place with a quiet visual indication of the error, while the rest of the artifact renders normally; a `math` fence whose body cannot be rendered SHALL likewise present the fence's raw source with a quiet visual indication that the formula could not be rendered. Neither case SHALL blank or crash the pane.

Mathematics rendering SHALL run under a non-trusting posture so mathematical source cannot inject active content: commands that would emit hyperlinks, external references, or scripts (for example `\href`) SHALL NOT produce live links, fetch external resources, or execute.

#### Scenario: Inline math renders within prose via double dollars

- **WHEN** an artifact contains a double-dollar expression such as `$$O(n \log n)$$` embedded mid-sentence
- **THEN** the detail pane renders it as typeset inline mathematics flowing with the surrounding text
- **AND** the raw LaTeX source is not shown
- **AND** its rendered size sits with the surrounding prose rather than enlarged above it

#### Scenario: Display math renders as a block

- **WHEN** an artifact contains a double-dollar-delimited expression standing alone as its own paragraph (single-line or multi-line block form) or a fenced code block with the `math` info string
- **THEN** the detail pane renders it as display mathematics in its own block
- **AND** a formula wider than the pane's content width scrolls horizontally within that block without widening the artifact

#### Scenario: Prose dollar amounts are never mathematics

- **WHEN** an artifact contains prose with multiple single-dollar amounts, such as "the plan costs $50 per seat and $60 with add-ons"
- **THEN** the sentence renders exactly as written, dollar signs and spacing intact
- **AND** no part of it is typeset as mathematics

#### Scenario: Display math is never vertically clipped

- **WHEN** an artifact contains display mathematics with under-limits or deep subscripts, such as a summation with a bound beneath it
- **THEN** every limit, subscript, and descender is fully visible
- **AND** the block shows no vertical scrollbar

#### Scenario: Dollar signs in code are never math

- **WHEN** an artifact contains dollar signs inside a code span or a fenced code block in another language — for example `` `\\wsl$\Ubuntu\home` `` or `` `releases/${tag}.md` ``
- **THEN** they render as literal dollar signs, unchanged
- **AND** a dollar sign in prose with no valid closing delimiter renders as a literal dollar sign

#### Scenario: Invalid inline math degrades in place

- **WHEN** an artifact contains a double-dollar-delimited expression whose content is not valid mathematical source
- **THEN** the detail pane presents that expression's raw source in place with a quiet indication of the error
- **AND** the rest of the artifact still renders normally

#### Scenario: An invalid math fence degrades to source

- **WHEN** an artifact contains a `math` fence whose body cannot be rendered
- **THEN** the detail pane shows the fence's raw source
- **AND** shows a quiet indication that the formula could not be rendered
- **AND** the rest of the artifact still renders

#### Scenario: Math source cannot inject active content

- **WHEN** a mathematical expression attempts to emit a hyperlink, an external reference, or a script (for example via `\href`)
- **THEN** the rendered output contains no live link and no active content
- **AND** no external resource is fetched and no script executes

#### Scenario: Math follows the colour scheme

- **WHEN** rendered mathematics is visible and the operating system switches between light and dark
- **THEN** the mathematics renders with the active scheme's surrounding text colour in both schemes

### Requirement: Mermaid Diagram Rendering

The detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement, `math` in the *Mathematical Notation Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

A rendered diagram SHALL derive its colours and fonts from the application's design tokens (see the *Design Token Layer* and *Typography System* requirements in the `visual-identity` capability) so that it reads as part of the same surface as the surrounding prose in both the light and dark schemes. This obligation extends to colours the diagram engine derives on its own for values the application does not map explicitly: the application SHALL inform the engine of the active scheme so that every derived colour is derived in the direction of that scheme, rather than under an assumed light palette. Diagram text SHALL remain legible against every filled surface the engine draws — including alternating table-row fills such as entity-relationship attribute rows, whose fills SHALL come from the design tokens' surface colours. When the operating system colour scheme changes while a diagram is visible, the diagram SHALL re-render so its colours follow the active scheme.

A diagram whose natural width exceeds the detail pane's content width SHALL scale down to fit — but only to a **legibility floor**. With an authored diagram label size $$f_{\text{label}}$$ and a fit-to-pane scale $$s_{\text{fit}}$$, the diagram SHALL render at

$$s_{\text{render}} = \max\left(s_{\text{fit}},\ \frac{f_{\min}}{f_{\text{label}}}\right), \qquad f_{\min} = 10\,\text{px}$$

so rendered label text never falls below $$f_{\min}$$. A diagram held at the floor is wider than the pane and SHALL scroll horizontally within its own block per the *Wide Block Containment* requirement. Every successfully rendered diagram — scaled, floor-held, or natural size — SHALL remain openable in the maximized view described by the *Maximized Figure View* requirement.

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

#### Scenario: A moderately wide diagram scales down to fit

- **WHEN** an artifact contains a diagram whose natural width exceeds the pane's content width by less than the legibility floor allows
- **THEN** it is displayed scaled down to fit the pane, with no horizontal scrolling
- **AND** its rendered label text is no smaller than the legibility floor

#### Scenario: A very wide diagram stops shrinking at the legibility floor

- **WHEN** an artifact contains a diagram so wide that fitting the pane would render its label text below the legibility floor
- **THEN** the diagram renders at the floor scale instead of fitting the pane
- **AND** it scrolls horizontally within its own block while the pane and document column do not
- **AND** a control to open it in the maximized view is available on it

### Requirement: Maximized Figure View

A figure the detail pane has rendered successfully — a `mermaid` diagram (see the *Mermaid Diagram Rendering* requirement) or an `svg` image (see the *SVG Fence Rendering* requirement) — SHALL be openable in a **maximized view**: a surface presented above the entire application window in which that single figure can be enlarged, reduced, and moved. A fence that degraded to its source, and a diagram whose rendering has not yet completed, SHALL NOT offer the maximized view, because neither has a figure to show.

**Affordance.** Each maximizable figure SHALL present a control that opens the maximized view. The control SHALL be operable by keyboard as well as by pointer (see the *Shell Keyboard Operability* requirement). On a figure that is rendered below its natural size — scaled down to fit the pane, or held at the legibility floor of the *Mermaid Diagram Rendering* requirement — the control SHALL be visible at rest on every device, because a reduced figure is exactly the one whose reader needs the escape to full size. On a device that reports no hover capability it SHALL be rendered visibly at rest regardless, and on a device whose primary pointer is coarse it SHALL present an enlarged hit area, per the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability. The figure's inline presentation SHALL be unchanged by the presence of the control: a figure still fits the detail pane's width while reading (or scrolls within its block at the legibility floor), and the maximized view is an addition to that default rather than a replacement for it.

**Initial scale.** The maximized view SHALL open with the figure fully visible — scaled so that neither dimension exceeds the surface's content area, with the scale taken from whichever axis constrains it more. For a surface of extents $$W_v \times H_v$$ with padding $$p$$, displaying content of extents $$W_c \times H_c$$:

$$s_{\text{fit}} = \min\left(\frac{W_v - 2p}{W_c},\ \frac{H_v - 2p}{H_c}\right)$$

**Zoom.** The maximized view SHALL support continuous zoom by wheel and by two-contact pinch, and SHALL provide explicit controls to return to the fit scale and to display the figure at actual size. Zoom driven by a pointer gesture SHALL be anchored at that pointer: the point of the figure beneath the pointer SHALL remain beneath it as the scale changes. Scale SHALL be bounded — never reduced below the fit scale (or actual size, whichever is smaller) and never increased beyond a fixed ceiling — so the figure can neither be lost in the surface nor enlarged without limit.

**Pan.** While the figure exceeds the surface's content area it SHALL be movable by dragging, and dragging SHALL be driven by pointer input so that a mouse, a touch contact, and a pen all move it through the same path (see the *Drag Interactions Accept Pointer Input* requirement in the `touch-input` capability).

**Fidelity.** An enlarged figure SHALL be re-rendered at the size at which it is displayed, rather than by magnifying a fixed-resolution rendering of it. Enlarging SHALL NOT degrade a figure's sharpness, in either the diagram path or the image path.

**Security posture is preserved.** The maximized view SHALL NOT relax the rendering guarantees of either path. An `svg` fence SHALL continue to be presented through an image context and SHALL NOT be injected into the host document's live DOM at any scale, and a `mermaid` diagram SHALL continue to be rendered under the strict security posture its own requirement specifies. The maximized view SHALL offer no means of editing, exporting, or otherwise writing the figure, consistent with the *Read-Only Viewer* requirement.

**Colour scheme.** While the maximized view is open the figure SHALL follow the active colour scheme exactly as it does inline, re-rendering when the operating system switches between light and dark. That re-render SHALL preserve the current scale and position, so a scheme change does not displace what the reader is looking at.

**Dismissal.** The maximized view SHALL be dismissable by the Escape key, by an explicit close control, and by activating the surface outside the figure. Dismissal SHALL return the reader to the artifact with its scroll position unchanged. Escape SHALL dismiss only the maximized view: any Settings or Archive pane open behind it SHALL remain open, and a second Escape SHALL be required to dismiss that (see the *Archive Entrypoint in Sidebar Footer* and *Settings Entrypoint in Sidebar Footer* requirements).

**The maximized view is ambient view state.** It SHALL NOT be part of the Address, the URL, or navigation history (see the `view-routing` capability), consistent with how side-pane visibility is treated by the *Side-Pane Visibility Toggles* requirement. Navigating to a different artifact SHALL close it.

A change to the artifact's content on disk while the view is open SHALL also close it. The maximized view SHALL NOT continue to present a figure rendered from source the artifact no longer contains, and SHALL NOT re-open itself on the reader's behalf. Holding it open across a reparse would require identifying one figure within an artifact across an edit to that artifact — which this capability deliberately does not do, for the same reason the view carries no Address. Closing is the honest outcome: the artifact behind it has already updated in place per the *Reactive Updates from Filesystem* requirement, and the affordance to maximize the new figure is immediately available.

#### Scenario: A rendered diagram can be maximized

- **WHEN** the detail pane has rendered a `mermaid` fence as a diagram
- **THEN** a control to maximize that diagram is available on it
- **AND** activating the control opens the diagram in a surface above the application window
- **AND** the diagram is initially shown fully visible within that surface

#### Scenario: A rendered svg image can be maximized

- **WHEN** the detail pane has rendered an `svg` fence as an image
- **THEN** a control to maximize that image is available on it
- **AND** activating the control opens the image in the same maximized surface the diagram path uses

#### Scenario: A reduced figure shows its maximize control at rest

- **WHEN** the detail pane renders a diagram scaled below its natural size — fit to the pane or held at the legibility floor
- **THEN** the maximize control on that figure is visible without hovering or focusing it
- **AND** a figure rendered at its natural size continues to reveal the control on hover or keyboard focus

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

#### Scenario: A live edit closes the maximized view rather than showing superseded source

- **WHEN** a figure is maximized and the artifact's file changes on disk so that its content is reparsed
- **THEN** the maximized view closes
- **AND** it never displays a figure rendered from source the artifact no longer contains
- **AND** the artifact behind it shows the reparsed content with its maximize affordance available

#### Scenario: A scheme change preserves scale and position

- **WHEN** a diagram is maximized and enlarged, and the operating system switches between light and dark
- **THEN** the diagram re-renders with the active scheme's design tokens
- **AND** its scale and visible region are unchanged

#### Scenario: Maximizing preserves the image path's inertness

- **WHEN** an `svg` fence whose body contains a script element or an event-handler attribute is maximized
- **THEN** the fence body is still not inserted into the host document's live DOM
- **AND** no script executes and no external resource is fetched at any scale
