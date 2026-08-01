## MODIFIED Requirements

### Requirement: Artifact Markdown Rendering

The detail pane SHALL render OpenSpec artifact markdown (proposal, design, tasks, and capability specs) as styled terminal text, including headings, lists, code blocks, and task checkboxes. Content the terminal cannot display (such as images) SHALL degrade to a textual representation rather than being omitted silently.

Links SHALL be presented with their destination discoverable — as the link text with its target shown textually, or as a terminal hyperlink (OSC 8) whose target the hosting terminal emulator may offer to open. When the hosting terminal is not known to support OSC 8 hyperlinks, the textual presentation SHALL be used, so the destination remains discoverable rather than being swallowed with the escape sequence — the same capability-fallback shape as the *Graceful Degradation* requirement. The terminal frontend itself SHALL NOT spawn any opener process in response to link content; any opening is the terminal emulator's own click-through behaviour.

#### Scenario: Proposal renders as styled text

- **WHEN** the user views a proposal artifact in the detail pane
- **THEN** its headings, paragraphs, and lists are rendered as styled terminal text

#### Scenario: Task checkboxes render as state

- **WHEN** the user views a tasks artifact
- **THEN** complete and incomplete tasks are shown with distinct checkbox states

#### Scenario: Images degrade to text

- **WHEN** an artifact contains an image
- **THEN** the pane shows the image's alternate text instead of omitting it

#### Scenario: A link's destination is discoverable

- **WHEN** an artifact contains a link — external or to a workspace file such as an HTML mockup
- **THEN** the pane presents the link with its destination visible textually or as a terminal hyperlink
- **AND** on a terminal not known to support OSC 8 hyperlinks the destination is shown textually
- **AND** the terminal frontend spawns no opener process
