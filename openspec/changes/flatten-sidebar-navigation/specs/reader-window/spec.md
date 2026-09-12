## MODIFIED Requirements

### Requirement: Reader Window Surface

A **reader window** SHALL present exactly one markdown document and nothing that navigates **to another document**. It SHALL render that document with the same renderer, and under the same guarantees, as the detail pane — see the *Markdown Rendering of Leaf Artifacts*, *Mermaid Diagram Rendering*, *SVG Fence Rendering*, *Mathematical Notation Rendering*, *Maximized Figure View*, and *Link Handling in Rendered Artifacts* requirements in the `spec-browser` capability — together with the document's identity, so the window states what it is showing.

Navigation **within** the document is not navigation to another document, and a reader window SHALL offer it exactly as the detail pane does: the document outline and fragment links defined by the `document-outline` capability. The outline is subject to that capability's placement rule, so a reader window too narrow to seat it beside the prose column shows none, and a window widened later gains it; neither state is remembered per window.

A reader window SHALL NOT contain the workspace tree, the commit rail, the sidebar footer, the Settings or Archive entry points, the quota indicators, the change header's artifact tab strip or instance switcher, or any other affordance that changes which document is displayed. A reader window displays the document it was opened for, for its whole life.

A reader window SHALL be read-only, consistent with the *Read-Only Viewer* requirement in the `spec-browser` capability. Opening a document in a reader window SHALL NOT change what the main window is displaying, and SHALL NOT alter the main window's tree selection, scroll position, or pane visibility.

#### Scenario: A reader window shows only the document

- **WHEN** the user opens a document in a reader window
- **THEN** the window renders that document's markdown and its identity
- **AND** it contains no workspace tree, no commit rail, no artifact tab strip, no instance switcher, and no Settings or Archive entry point

#### Scenario: Rich content renders as it does in the pane

- **WHEN** a document opened in a reader window contains a `mermaid` fence, an `svg` fence, or display math
- **THEN** each renders exactly as it does in the detail pane, including the maximize affordance on a successfully rendered figure

#### Scenario: Opening a reader window leaves the main window alone

- **WHEN** the user opens a document in a reader window while the main window is displaying a different artifact
- **THEN** the main window continues to display that artifact
- **AND** its tree selection and scroll position are unchanged

#### Scenario: A reader window offers no way to another document

- **WHEN** a reader window is open
- **THEN** it presents no control, list, or link that would display a different document in that window

#### Scenario: A reader window navigates within its document

- **WHEN** a reader window is wide enough to seat the outline beside the prose column and its document has headings
- **THEN** the outline is shown and activating an entry scrolls the window's document to that heading
- **AND** the window continues to display the same document
