## MODIFIED Requirements

### Requirement: Master-Detail Layout

The main application window SHALL present a master-detail layout of two primary panes — a tree-navigation pane on the left and a content-rendering (detail) pane in the center — plus an optional commit-graph rail on the far right (see the *Commit-Graph Rail Pane* requirement in the `commit-graph` capability). Resizable dividers separate the panes. The tree pane and the rail are each independently hideable (see the *Side-Pane Visibility Toggles* requirement); the detail pane is always visible.

Dragging a divider SHALL be driven by pointer input, so that a mouse, a touch contact, and a pen all resize the panes through the same clamps (see the *Drag Interactions Accept Pointer Input* requirement in the `touch-input` capability).

The shell SHALL size itself to the viewport that is actually visible to the user, and SHALL NOT size itself to a viewport height that assumes retractable browser chrome has been retracted. Because the shell suppresses document scrolling, any part of the layout that exceeds the visible viewport is permanently unreachable — there is no scroll with which to recover it — so the shell SHALL never exceed the visible viewport. In particular, content anchored to the bottom of the sidebar SHALL remain on screen and operable at every viewport height at which the application is usable, including the sidebar footer entrypoints covered by the *Settings Entrypoint in Sidebar Footer* and *Archive Entrypoint in Sidebar Footer* requirements, and any usage-quota strips rendered beneath them.

The detail (center) pane SHALL render one of five targets: an OpenSpec artifact's markdown, a commit's detail view when a commit is selected in the rail, the **pull-request view** when the address names a pull request (see the *Pull-Request Addresses* requirement in the `view-routing` capability and the *Pull-Request View* requirement in the `pull-request-viewer` capability), the **Dashboard** (see the *Dashboard Home Surface* requirement in the `dashboard` capability), or the **Archive view** (see the *Archive View* requirement in the `archive-browser` capability) when the Archive entrypoint is active. The Dashboard SHALL be the default target: it is rendered at startup and whenever no artifact and no commit is selected, the address names no pull request, and the Archive view is not open, in place of any "nothing selected" placeholder. The Archive view and the Settings view are modal pane targets toggled from their sidebar entrypoints; while either is open it takes precedence over the artifact/commit/pull-request/Dashboard target, and closing it returns the pane to whichever of those was selected most recently. The tree drives the artifact target, the rail drives the commit target, and a pull-request panel row, a pull-request chip in the change header (see the *Pull-Request Chip in the Change Header* requirement) or a pull-request address drives the pull-request target.

While the address names a pull request, the pull-request target SHALL hold the pane even when that pull request cannot be shown: the pane SHALL then show the outcome the address resolves to (see the *Pull-Request Addresses* and *Cold-Load Address Resolution* requirements in the `view-routing` capability), and never the Dashboard in its place.

#### Scenario: Panes visible at startup

- **WHEN** the user opens the main window for the first time
- **THEN** the tree pane and the detail pane are visible side by side
- **AND** the commit-graph rail is visible on the far right
- **AND** the detail pane renders the Dashboard (no artifact or commit having been selected, and the Archive view not open)
- **AND** the dividers between the panes can be dragged to adjust their widths

#### Scenario: Shell fits a browser viewport with persistent chrome

- **WHEN** the served web UI is loaded in a browser whose chrome occupies part of the screen and does not retract
- **THEN** the shell's height matches the viewport the browser actually exposes
- **AND** no part of the layout extends below the bottom edge of that viewport

#### Scenario: Sidebar footer entrypoints stay reachable on a short viewport

- **WHEN** the served web UI is loaded at a viewport height short enough that the sidebar tree must scroll
- **THEN** the Settings entrypoint, the Archive entrypoint, and any usage-quota strips beneath them are fully visible
- **AND** each of them can be activated
- **AND** the sidebar tree above them absorbs the reduced height by scrolling

#### Scenario: Detail pane renders the Dashboard by default

- **WHEN** no artifact and no commit is selected, the address names no pull request, and the Archive view is not open
- **THEN** the detail pane renders the Dashboard
- **AND** no "nothing selected" placeholder is shown

#### Scenario: Detail pane renders artifact markdown by default

- **WHEN** the user selects a renderable artifact node in the tree
- **THEN** the detail pane renders that artifact's markdown

#### Scenario: Detail pane renders commit detail when a commit is selected

- **WHEN** the user selects a commit in the commit-graph rail
- **THEN** the detail pane renders that commit's detail view
- **AND** selecting an artifact node in the tree afterwards returns the detail pane to artifact markdown

#### Scenario: Detail pane renders a pull request at its address

- **WHEN** the user clicks an openable row in a pull-request panel
- **THEN** the detail pane renders the pull-request view for that pull request
- **AND** the address names that pull request
- **AND** selecting an artifact node in the tree afterwards returns the detail pane to artifact markdown

#### Scenario: A pending pull-request address does not show the Dashboard

- **WHEN** the application is loaded at a pull-request address while that provider is enabled and its first poll is still running
- **THEN** the detail pane shows "Loading…"
- **AND** the Dashboard is not rendered in the interim

#### Scenario: Detail pane renders the Archive view when its entrypoint is active

- **WHEN** the user activates the Archive entrypoint
- **THEN** the detail pane renders the Archive view in place of the artifact/commit/pull-request/Dashboard target
- **AND** closing the Archive view returns the detail pane to the most recently selected artifact, commit, pull request, or the Dashboard

### Requirement: Mermaid Diagram Rendering

In **workspace markdown** — change artifacts, archived artifacts and workspace file-browser previews — the detail pane SHALL render a fenced code block whose info string is `mermaid` as a graphical diagram rather than as syntax-highlighted source. Every fenced code block whose info string is not special-cased by this capability (`mermaid` here, `svg` in the *SVG Fence Rendering* requirement, `math` in the *Mathematical Notation Rendering* requirement) SHALL continue to render as syntax-highlighted source, unchanged. Diagram rendering is a client-side concern of the rich (WebView / browser) frontend bundle; the raw artifact markdown returned by the backend SHALL be unchanged, and the `terminal-ui` frontend, which cannot render SVG, SHALL continue to present `mermaid` fences as code text.

**Pull-request content** — a pull request's description, conversation and review-thread comments shown in the detail pane — SHALL draw no diagram: each `mermaid` fence in it SHALL render as its source, as plain unhighlighted text, because the diagram engine, while laying a diagram out, fetches any remote image its source declares, whatever the security level (see the *Pull-Request Content Is Untrusted* requirement in the `pull-request-viewer` capability). The rest of this requirement governs the diagrams drawn from workspace markdown.

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

#### Scenario: A mermaid fence in pull-request content shows its source

- **WHEN** the detail pane shows a pull request whose description contains a `mermaid` fence of valid diagram source, one of whose nodes declares a remote image
- **THEN** the fence renders as its source, as plain unhighlighted text
- **AND** no diagram is drawn and no request is made for the image
- **AND** the same fence in a change artifact renders as a diagram

### Requirement: Link Handling in Rendered Artifacts

A link click inside markdown rendered by the shared markdown renderer SHALL never navigate the application's webview, whether that markdown is **workspace markdown** — change artifacts, archived artifacts, and workspace file-browser previews alike — or **pull-request content**, a pull request's description, conversation and review-thread comments shown in the detail pane. Activation paths that bypass the renderer's click handling (such as the webview's native context menu or link drag-out) SHALL be denied by a shell-level navigation guard that permits only the application's own origin.

In workspace markdown, every anchor activation SHALL be intercepted and dispatched by link class, and any class without a defined behaviour SHALL be inert. The link classes, the opening rules and the authorization that follow govern workspace markdown only: pull-request content SHALL instead follow the *Pull-Request Content Is Untrusted* and *Desktop Link Opener* requirements of the `pull-request-viewer` capability, and none of its links SHALL reach this requirement's open operation.

An absolute link with an `http` or `https` scheme SHALL open in the system default browser, and a `mailto:` or `tel:` link SHALL open via the operating system's default handler, in each case leaving the application view unchanged.

A relative link to a non-markdown file SHALL be resolved against the directory of the markdown file being viewed — after stripping any fragment and query and percent-decoding the path exactly once — and opened with the operating system's default handler for the target's type; for an `.html` mockup that is the default browser, which resolves the mockup's sibling assets (stylesheets, scripts, images) itself. The target MAY live anywhere inside the authorized root; it is not confined to the change directory the linking artifact belongs to. This boundary is deliberately wider than the `openspec/changes/` subtree that confines artifact reads (see *Artifact Reads Are Confined to Registered Workspaces*): the open operation reads and returns no file content — its effect is limited to asking the OS to display an allow-listed document inside a folder the user brought into the application.

Opening SHALL be authorized at the shared application boundary before any opener is invoked:

- The root SHALL be authorized by the same rule that authorizes file browsing (see *Browsing Is Confined to Registered Workspaces* in the `workspace-file-browser` capability): a registered or registry-discovered workspace, or a repository main worktree accepted because a worktree of that repository is registered. An unauthorized root SHALL be refused before any path is resolved.
- The canonicalised target SHALL be contained within the canonicalised authorized root — so a `..` traversal (encoded or not) or a symlink pointing outside the root is refused rather than opened.
- The target SHALL match a case-insensitive allow-list of document types — initially `.html`, `.htm`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`, `.css`, `.pdf`, `.txt`, `.json`, `.csv` — and directories SHALL be refused. Executable and script targets are therefore never opened: following a link SHALL NOT be able to execute a file.

The frontend SHALL NOT hold a general open-URL or open-path capability. The only open operation reachable from workspace markdown is this validated one, and the one other open operation reachable from rendered content is `open_pull_request_link` (see the *Desktop Link Opener* requirement in the `pull-request-viewer` capability), which serves only the pull-request view (its content's links, its checks' links, and its provider's page once the pull request is no longer listed) and hands the platform opener nothing but an absolute `http` or `https` URL with a host.

Relative links to markdown files (matched case-insensitively) SHALL be inert in v1, reserved for future in-app navigation. Links with any other scheme (including `javascript:` and `file:`) SHALL be inert. A **fragment-only** link is dispatched by the `document-outline` capability's *Fragment Links Resolve Within the Document* requirement: it scrolls within the rendered document when its fragment names a heading, and fails quietly as a dangling link when it does not; it never navigates the application. Inert links SHALL carry a visual affordance distinguishing them from openable links, so a dead link reads as policy rather than breakage; a fragment link that resolves to a heading is not inert and SHALL NOT carry it.

A click whose target does not exist or is refused SHALL produce a quiet indication that the link could not be opened, SHALL NOT navigate or blank the pane, and SHALL leave the rendered artifact fully usable.

Opening files is a desktop-frontend concern; other frontends degrade per their own capability specs, and the raw artifact markdown returned by the backend is unchanged by this requirement.

#### Scenario: An external link opens in the system browser

- **WHEN** the user clicks a link with an `http` or `https` URL in a rendered artifact
- **THEN** the URL opens in the system default browser
- **AND** the application view does not navigate away

#### Scenario: A relative HTML mockup link opens externally

- **WHEN** a change's `proposal.md` contains a relative link to `./mockups/login.html` and that file exists
- **THEN** clicking the link opens the mockup via the operating system's default handler for HTML
- **AND** the detail pane still shows the rendered proposal

#### Scenario: A mockup outside the change directory opens

- **WHEN** an artifact links to an `.html` file that resolves inside the authorized root but outside the linking change's directory
- **THEN** clicking the link opens the file via the operating system's default handler

#### Scenario: A fragment- or query-suffixed file link opens the file

- **WHEN** an artifact links to `./mockups/login.html#hero` or `./mockups/login.html?v=2`, or to a target whose name is percent-encoded (such as `./my%20mockup.html`)
- **THEN** the underlying file resolves and opens as if linked plainly

#### Scenario: A link escaping the root is refused

- **WHEN** an artifact's relative link resolves outside the authorized root — via `..` traversal (plain or percent-encoded) or via a symlink inside the root whose target lies outside it
- **THEN** nothing is opened
- **AND** a quiet indication is shown that the link could not be opened
- **AND** the pane neither navigates nor blanks

#### Scenario: An executable or directory target is refused

- **WHEN** an artifact links to an executable or script file (such as `./run.sh`, `./setup.command`, or `./tool.exe`) or to a directory (including an `.app` bundle)
- **THEN** nothing is opened or executed
- **AND** a quiet indication is shown that the link could not be opened

#### Scenario: The open operation refuses an unauthorized root

- **WHEN** the open operation is invoked with a root that is neither a registered or registry-discovered workspace nor a repository main worktree accepted by the browsing rule
- **THEN** it is refused with an error before any path is resolved or opened

#### Scenario: A relative markdown link is inert

- **WHEN** the user clicks a relative link to a markdown file in a rendered artifact, regardless of extension casing (`./notes.md`, `./NOTES.MD`)
- **THEN** nothing opens and the application view does not change

#### Scenario: A script-scheme link is inert

- **WHEN** a rendered artifact contains a link with a `javascript:` or `file:` href
- **THEN** clicking it executes nothing, opens nothing, and does not navigate the webview

#### Scenario: Bypassing the click handler cannot navigate the app

- **WHEN** a link is activated through a path that bypasses the renderer's click handling — the webview context menu's open-link action, or dragging the link
- **THEN** the application webview does not navigate away from the app UI

#### Scenario: A dangling link fails quietly

- **WHEN** the user clicks a relative link whose target file does not exist
- **THEN** a quiet indication is shown that the link could not be opened
- **AND** the rendered artifact remains fully usable

#### Scenario: A fragment link scrolls within the document

- **WHEN** the user clicks a fragment-only link whose fragment names a heading of the rendered artifact
- **THEN** the artifact scrolls to that heading
- **AND** the application view, the address and the history are unchanged

#### Scenario: Pull-request content follows its own link rules

- **WHEN** the detail pane in the desktop application shows a pull request whose description contains an `https` link and a `mailto:` link
- **THEN** activating the `https` link opens it in the system default browser through `open_pull_request_link`, not through this requirement's open operation
- **AND** activating the `mailto:` link opens nothing
- **AND** the application view does not navigate away

### Requirement: Pull-Request Chip in the Change Header

When the worktree an active change's artifact is read from is linked to one or more pull requests (see the *Matching a Pull Request to a Worktree* requirement in the `pull-request-worktree-links` capability), the identity row of the change header (see *Change Identity Header in the Detail Pane*) SHALL show a **pull-request chip** for each linked pull request, following the branch chip, up to two; any further linked pull requests SHALL be summarised by one passive `+N` chip whose tooltip lists them.

A pull-request chip SHALL read `#` followed by the pull request's number, and SHALL carry the pull request's draft marker, its checks state and its conflicting marker using the same treatments the pull-request panels use. Its tooltip and accessible name SHALL state the provider, the destination repository, the title, whether it is the viewer's own or awaiting their review, the review summary in words, and the checks state in words. The chip SHALL be an outlined chip in neutral ink, never tinted with the workspace's palette colour, so it is not mistaken for the branch chip.

A pull-request chip is a **control**: activating it SHALL open the pull request exactly as activating its row in a pull-request panel does (see the *Opening a GitHub Pull Request* requirement in the `github-pull-requests` capability and the *Opening a Pull Request* requirement in the `bitbucket-pull-requests` capability):

- **A click, Enter or Space** SHALL navigate the detail pane to the pull request's address (see the *Pull-Request Addresses* requirement in the `view-routing` capability), so the pane shows the pull request in place of the artifact whose header carries the chip. The navigation SHALL add a history entry, so a back gesture returns the pane to that artifact.
- **A click held with the platform's new-window modifier** — Cmd on macOS, Ctrl elsewhere — SHALL open the pull request in its own window: a desktop window in the desktop application, a browser tab in the browser skin (see the *Opening a Pull Request Like a Document* and *Pull-Request Window* requirements in the `pull-request-viewer` capability). It SHALL NOT alter what the detail pane displays, the tree's selection or the navigation history, so the artifact stays displayed. The modifier SHALL be selected by platform: on macOS a Ctrl-click is the secondary click, and SHALL open nothing.

A chip whose row is not openable (see the *GitHub Row Signals* requirement in the `github-pull-requests` capability) SHALL open nothing, as that row opens nothing.

In the desktop application the chip SHALL remain a button, so the webview's own link menu cannot reload the main window at the pull request's path and lose its in-memory history. In the browser skin it SHALL be a link whose target is SpecForge's own path for the pull request's address, never the provider's page, so the browser's own ways of opening or copying a link lead to the pull request in SpecForge; its click and its modifier click SHALL be handled as above, and Space SHALL activate it as Enter does. The provider's web page is one control away, in the pull-request view's header (see the *Pull-Request View* requirement in the `pull-request-viewer` capability), rather than behind the chip.

The chip SHALL be a separate element from the change name, so the name's copy-on-click, its selection, its single tab stop and its confirmation are unaffected, and it SHALL follow the change name in keyboard order.

An archived change, a change in a flat workspace, and a change whose worktree has no branch SHALL show no pull-request chip. A change whose worktree is linked to nothing SHALL render its header exactly as without this requirement.

#### Scenario: A linked worktree shows its pull request

- **WHEN** the detail pane renders an artifact read from a worktree linked to GitHub pull request 42, which has failing checks
- **THEN** the identity row shows the branch chip followed by a chip reading `#42` with the failing checks treatment
- **AND** the chip's accessible name states GitHub, the repository, the title and that the checks are failing

#### Scenario: The chip opens the pull request

- **WHEN** the detail pane shows an artifact whose header carries a chip reading `#42`, and the user activates the chip by click, Enter or Space
- **THEN** the detail pane shows pull request 42 at its address, in place of the artifact
- **AND** the provider's web page does not open
- **AND** a back gesture returns the detail pane to the artifact

#### Scenario: Copying the name ignores the chip

- **WHEN** the user clicks the change name in a header that shows a pull-request chip
- **THEN** the clipboard contains the change name only

#### Scenario: More than two linked pull requests are summarised

- **WHEN** the worktree is linked to three pull requests
- **THEN** the identity row shows two pull-request chips and a `+1` chip whose tooltip names the third

#### Scenario: An archived change shows no pull-request chip

- **WHEN** the detail pane renders an artifact of an archived change whose worktree path is linked to a pull request
- **THEN** no pull-request chip is rendered

#### Scenario: A modifier click opens the pull request's own window

- **WHEN** the detail pane shows an artifact whose header carries a chip reading `#42`, and the user clicks the chip holding Cmd on macOS or Ctrl elsewhere
- **THEN** pull request 42 opens in its own window: a desktop window in the desktop application, a browser tab in the browser skin
- **AND** the detail pane still shows the artifact
- **AND** the tree's selection and the navigation history are unchanged

#### Scenario: The secondary click opens nothing on macOS

- **WHEN** the user Ctrl-clicks a pull-request chip on macOS, which is that platform's secondary click
- **THEN** no pull-request window opens
- **AND** the detail pane still shows the artifact, and no history entry is added

#### Scenario: The browser-skin chip links to SpecForge's address

- **WHEN** the change header shows a pull-request chip in the browser skin
- **THEN** the chip is a link whose target is SpecForge's own path for the pull request's address, not the provider's page
- **AND** opening that link in a new tab through the browser gives a SpecForge tab at that address
- **AND** pressing Space on the focused chip shows the pull request in the detail pane, as Enter does
