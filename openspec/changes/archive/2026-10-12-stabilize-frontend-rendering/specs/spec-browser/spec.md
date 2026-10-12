## ADDED Requirements

### Requirement: Rendered Blocks Survive Document Re-renders

When a document surface renders its document again, every rendered block whose source is unchanged SHALL keep its existing DOM element: headings, list items, task checkboxes, tables, code fences, links, diagrams and figures. This holds whether the new render came from new content elsewhere in the document or from a transient indication such as a link that could not be opened. A block SHALL be replaced only in three cases: its own source changes, a value that decides its output changes, or a block of the same kind is inserted or removed before it within the same parent. This applies to workspace markdown and pull-request content alike (see the *Pull-Request Content Is Untrusted* requirement in the `pull-request-viewer` capability).

Under that same rule, a `mermaid` diagram whose fence source is unchanged SHALL stay drawn across such a render. It SHALL NOT fall back to its pending presentation, SHALL NOT invoke the diagram engine again, and SHALL NOT change its height. A diagram SHALL still be drawn again when its own source changes and when the colour scheme changes (see the *Mermaid Diagram Rendering* requirement). Keeping unchanged blocks in place is part of how a refresh the user did not initiate preserves the reading position (see the *Reactive Updates from Filesystem* requirement).

Link classification SHALL keep reading the heading identifiers of the render in progress. A fragment link SHALL be classified against the document as currently rendered and never against the headings of an earlier render (see the *Fragment Links Resolve Within the Document* requirement in the `document-outline` capability).

The quiet indication that a link could not be opened SHALL NOT render the document again, either when it is shown or when it is cleared.

#### Scenario: An edit elsewhere leaves an unchanged diagram drawn

- **WHEN** a document surface shows an artifact holding a drawn `mermaid` diagram, and the artifact's file is modified on disk in a paragraph after the diagram
- **THEN** the surface renders the updated paragraph
- **AND** the diagram keeps its drawn element and never shows its pending presentation
- **AND** the diagram engine is not invoked for it

#### Scenario: A changed diagram is drawn again

- **WHEN** a document surface shows a drawn `mermaid` diagram and the diagram's own fence source is modified on disk
- **THEN** the diagram is drawn again from its new source

#### Scenario: A scheme change still redraws the diagram

- **WHEN** a drawn diagram is visible and the operating system switches between light and dark
- **THEN** the diagram is drawn again with the active scheme's tokens

#### Scenario: A failed link click does not re-render the document

- **WHEN** the reader activates a workspace link whose target cannot be opened, in a document holding headings, a table and a drawn diagram
- **THEN** the quiet "could not open" indication is shown and later cleared
- **AND** neither its showing nor its clearing replaces any element of the rendered document or invokes the diagram engine

#### Scenario: Fragment links follow the headings of the current render

- **WHEN** an edit on disk adds a heading whose identifier an existing fragment link in the same document names
- **THEN** after the refresh that link scrolls to the new heading rather than giving the dangling-fragment indication

#### Scenario: Pull-request content keeps its blocks too

- **WHEN** a pull request's description holding headings, a list and a table is shown, and the view renders again with the description unchanged
- **THEN** those headings, list items and table keep their elements
- **AND** the headings still carry no identifier and the description's links, images and fences still follow the pull-request rules

### Requirement: Unchanged Workspace Views Keep Their Rows

A frontend's read of the workspace views SHALL NOT disturb the parts of the tree whose data it did not change. When a read returns data equal to what the frontend holds, the frontend SHALL keep what it holds, and no tree row, header or address resolution derived from it SHALL be rebuilt. When a read changes some repositories, changes or instances, only the rows showing changed data SHALL render again, with one exception. The held data is matched to the read by position within each list, so when a row is inserted into or removed from a list, the rows after it in that same list MAY render again once. Every row before it, and every row of every other list and repository, SHALL NOT render again. Every row SHALL keep its element either way.

This SHALL never hide a real change. Every field that differs between the held data and the read SHALL reach the rows that show it on that same refresh, as the *Reactive Updates from Filesystem* requirement demands: a status dot, a dirty flag, a display name or colour, an instance, a task count, a favourite. The views' loading flag SHALL still flip once after the first read completes, whatever that read returns (see the *Cold-Load Address Resolution* requirement in the `view-routing` capability).

Every window that reads the workspace views (the main window, a reader window and a pull-request window) SHALL apply this rule.

#### Scenario: A refresh that changes nothing re-renders no row

- **WHEN** a cache notification causes a views read whose result equals the held views
- **THEN** no tree row renders again
- **AND** the detail pane's header and address resolution are not recomputed

#### Scenario: One repository's change renders only its rows

- **WHEN** two repositories are registered and a task checkbox is toggled on disk in a change of the first
- **THEN** that change's row shows the new task progress on the first refresh after the edit
- **AND** the second repository's rows keep their elements and do not render again

#### Scenario: A new change renders at most the rows after it

- **WHEN** two repositories are registered and a new change appears in the first
- **THEN** the new change's row is shown on the first refresh after it appears
- **AND** the first repository's rows before it and every row of the second repository do not render again

#### Scenario: A presentation change still reaches its row

- **WHEN** a workspace is renamed or recoloured
- **THEN** its row shows the new name or swatch on the next refresh, while every other workspace's rows keep their elements

### Requirement: Surfaces and Rendering Engines Load on Demand

Each window kind SHALL load the code of its own root and the surfaces that root shows. A reader window, a pull-request window and an image zoom window SHALL NOT load the main window's dashboard, settings, commit-graph rail or commit garden. In the main window, the surfaces that are not shown by default (a pull request with its diff view, commit detail, settings, the workspace file browser and the archive) SHALL load when first shown or when warmed after the first paint. Their code SHALL NOT be on the path to the first paint.

While a surface's code is loading, the detail pane SHALL present what it presents for a pending address. It SHALL NOT present the Dashboard for an address that names another surface (see the *Master-Detail Layout* requirement). It SHALL NOT present an error.

The mathematics engine and its stylesheet SHALL load only when a rendered document contains mathematics, or when warmed after the first paint. Mathematics SHALL NOT be shown as its source while the engine loads. A document whose mathematics cannot yet be typeset SHALL keep its previous rendering until the engine is ready, or render nothing if it has no previous rendering. If the engine cannot be loaded at all, the document SHALL stop waiting for it and render with its mathematics as code, so it never stays blank or stops following later edits. A surface whose code cannot be loaded SHALL say so in place, with a way to reload, rather than leave the window empty. All of this SHALL work without network access, because every chunk is part of the application bundle (see the *Mathematical Notation Rendering* requirement).

The pull-request and zoom windows' head policies SHALL still be installed before any fetch they govern, including the fetch of any code those windows load on demand (see the *Pull-Request Window Permissions* requirement in the `pull-request-viewer` capability).

#### Scenario: A reader window does not load the dashboard

- **WHEN** a reader window opens on a change artifact
- **THEN** the code for the dashboard, settings, commit-graph rail and commit garden is not loaded in that window

#### Scenario: A pending surface never shows the Dashboard

- **WHEN** the main window cold-loads at a pull request's address and the pull-request surface's code has not loaded yet
- **THEN** the detail pane shows its pending presentation until the surface renders
- **AND** the Dashboard is not shown in between

#### Scenario: A document without maths does not need the engine

- **WHEN** the first document a window shows holds no double-dollar expression and no `math` fence
- **THEN** that document renders without waiting for the mathematics engine

#### Scenario: Maths never flashes its source

- **WHEN** the first document a window shows holds display mathematics and the mathematics engine has not loaded yet
- **THEN** the formula is never shown as its LaTeX source
- **AND** once the engine is ready the formula renders typeset

#### Scenario: A failed engine load shows maths as code

- **WHEN** a document holding display mathematics is shown and the mathematics engine cannot be loaded
- **THEN** the document renders with that mathematics as code rather than staying blank
- **AND** later edits to the document still reach the screen

#### Scenario: A surface that cannot load offers a reload

- **WHEN** the reader opens Settings and its code cannot be loaded
- **THEN** the detail pane says the view could not be loaded and offers Reload
- **AND** the tree and the rest of the window stay as they were

#### Scenario: The pull-request window's policy precedes its code

- **WHEN** a pull-request window opens
- **THEN** its content-security policy is in the document's head before the pull-request surface's code is requested
