# touch-input Specification

## Purpose
TBD - created by archiving change add-web-ui-touch-support. Update Purpose after archive.
## Requirements
### Requirement: Drag Interactions Accept Pointer Input

Every interaction in the served web UI that is driven by dragging SHALL be implemented with pointer input rather than mouse events alone, so that a mouse, a touch contact, and a pen drive it identically through the same code path and the same clamps.

A drag SHALL capture the pointer for the duration of the gesture, so that movement continuing outside the originating element still drives the drag, and SHALL release that capture when the gesture ends or is cancelled. An element that initiates a drag SHALL suppress the browser's default touch behaviours (panning, scrolling, and double-tap zoom) for that element alone, so that the page's own scroll handling cannot steal the gesture.

Existing keyboard-driven equivalents of a drag SHALL be unchanged, and SHALL continue to move through the same clamps as the pointer path.

#### Scenario: Touch drag resizes a side pane

- **WHEN** the user presses a pane divider with a touch contact in the served web UI and moves the contact horizontally
- **THEN** the adjacent pane resizes to follow the contact
- **AND** the resize is clamped by the same minimum and maximum widths that constrain a mouse drag
- **AND** the page does not scroll or zoom in response to the gesture

#### Scenario: Mouse drag behaviour is unchanged

- **WHEN** the user drags a pane divider with a mouse
- **THEN** the pane resizes exactly as it did before this capability existed
- **AND** the resulting width is clamped identically to the touch path

#### Scenario: An interrupted gesture releases the pointer

- **WHEN** a divider drag is in progress and the gesture is cancelled by the system or the pointer is lost
- **THEN** the drag ends without leaving the divider in a captured or dragging state
- **AND** a subsequent drag on the same divider starts normally

#### Scenario: Keyboard resize still works

- **WHEN** a pane divider has keyboard focus and the user presses the arrow keys
- **THEN** the pane resizes through the same clamps as a pointer drag

### Requirement: Essential Controls Are Discoverable Without Hover

On a device that reports no hover capability, a control SHALL NOT be hidden at rest when it is the only on-screen means of performing its action or of restoring hidden content. Such controls SHALL render in their visible state at rest, with chrome sufficient to distinguish them from the surface behind them rather than relying on a hover highlight that cannot occur.

A keyboard chord SHALL NOT count as an available alternative when evaluating this requirement, because a touch device may have no hardware keyboard attached.

On devices that do report hover capability, at-rest hiding SHALL be preserved exactly as specified by the requirement that governs each control, so this constraint introduces no change on desktop.

#### Scenario: Pane visibility affordances are visible at rest on a touch device

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **THEN** each visible side pane's collapse affordance is rendered visibly at rest
- **AND** a hidden pane's restore affordance is rendered visibly at rest
- **AND** each is distinguishable from the surface behind it without being hovered

#### Scenario: Favorite toggle is visible at rest on a touch device

- **WHEN** the served web UI is loaded on a device that reports no hover capability
- **THEN** the favorite toggle on a favoritable row whose change is not a favorite is visible at rest
- **AND** its reserved slot still prevents any other row content from shifting

#### Scenario: Hover-capable devices are unaffected

- **WHEN** the UI is loaded on a device that reports hover capability
- **THEN** controls specified as hidden at rest remain hidden at rest
- **AND** they are revealed on hover or focus exactly as before

### Requirement: Interactive Targets Meet a Minimum Size on Coarse Pointers

On a device whose primary pointer is coarse, every icon-only control and every drag handle in the served web UI SHALL present an enlarged hit area, independent of the size of the glyph or hairline rendered inside it.

A control whose surroundings do not bound it SHALL present a hit area of at least $44 \times 44$ CSS pixels. Where neighbouring interactive targets bound a control, its hit area SHALL instead be the largest that does not overlay one of them, and SHALL still be at least $24 \times 24$ CSS pixels. Two bounds apply:

- a control embedded in a fixed-height list row SHALL be bounded by that row's height, so that it never overhangs the rows above or below and intercepts input meant for them;
- a drag handle SHALL be bounded so that it does not cover a control rendered near the edge of a pane it divides.

Enlarging a hit area SHALL NOT change the control's rendered visual size and SHALL NOT shift surrounding layout, so the coarse-pointer treatment is an input concern only and never a visual redesign.

#### Scenario: Free-standing icon controls are touch-sized

- **WHEN** the served web UI is loaded on a device whose primary pointer is coarse
- **THEN** each free-standing icon-only control presents a hit area of at least $44 \times 44$ CSS pixels
- **AND** the glyph drawn inside it renders at its original size

#### Scenario: Dividers are graspable without swallowing edge controls

- **WHEN** the served web UI is loaded on a device whose primary pointer is coarse
- **THEN** each pane divider presents a hit area of at least $24 \times 24$ CSS pixels
- **AND** that area does not cover the centre of any control rendered at the adjoining pane's edge
- **AND** the divider's rendered hairline width is unchanged
- **AND** the panes on either side are not displaced by the enlarged hit area

#### Scenario: A row-embedded control stays inside its row

- **WHEN** the served web UI is loaded on a device whose primary pointer is coarse
- **THEN** the favorite toggle's hit area is no taller than the row that contains it
- **AND** activating the row directly above or below it is unaffected

#### Scenario: Fine-pointer devices keep their current geometry

- **WHEN** the UI is loaded on a device whose primary pointer is fine
- **THEN** control hit areas and divider widths are exactly as they were before this capability existed

