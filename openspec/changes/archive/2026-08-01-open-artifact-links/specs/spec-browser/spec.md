## ADDED Requirements

### Requirement: Link Handling in Rendered Artifacts

A link click inside markdown rendered by the shared markdown renderer — change artifacts, archived artifacts, and workspace file-browser previews alike — SHALL never navigate the application's webview: every anchor activation SHALL be intercepted and dispatched by link class, any class without a defined behaviour SHALL be inert, and activation paths that bypass the renderer's click handling (such as the webview's native context menu or link drag-out) SHALL be denied by a shell-level navigation guard that permits only the application's own origin.

An absolute link with an `http` or `https` scheme SHALL open in the system default browser, and a `mailto:` or `tel:` link SHALL open via the operating system's default handler, in each case leaving the application view unchanged.

A relative link to a non-markdown file SHALL be resolved against the directory of the markdown file being viewed — after stripping any fragment and query and percent-decoding the path exactly once — and opened with the operating system's default handler for the target's type; for an `.html` mockup that is the default browser, which resolves the mockup's sibling assets (stylesheets, scripts, images) itself. The target MAY live anywhere inside the authorized root; it is not confined to the change directory the linking artifact belongs to. This boundary is deliberately wider than the `openspec/changes/` subtree that confines artifact reads (see *Artifact Reads Are Confined to Registered Workspaces*): the open operation reads and returns no file content — its effect is limited to asking the OS to display an allow-listed document inside a folder the user brought into the application.

Opening SHALL be authorized at the shared application boundary before any opener is invoked:

- The root SHALL be authorized by the same rule that authorizes file browsing (see *Browsing Is Confined to Registered Workspaces* in the `workspace-file-browser` capability): a registered or registry-discovered workspace, or a repository main worktree accepted because a worktree of that repository is registered. An unauthorized root SHALL be refused before any path is resolved.
- The canonicalised target SHALL be contained within the canonicalised authorized root — so a `..` traversal (encoded or not) or a symlink pointing outside the root is refused rather than opened.
- The target SHALL match a case-insensitive allow-list of document types — initially `.html`, `.htm`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`, `.css`, `.pdf`, `.txt`, `.json`, `.csv` — and directories SHALL be refused. Executable and script targets are therefore never opened: following a link SHALL NOT be able to execute a file.

The frontend SHALL NOT hold a general open-URL or open-path capability; the only open operation reachable from rendered content is this validated one.

Relative links to markdown files (matched case-insensitively) SHALL be inert in v1, reserved for future in-app navigation. Fragment-only links and links with any other scheme (including `javascript:` and `file:`) SHALL be inert. Inert links SHALL carry a visual affordance distinguishing them from openable links, so a dead link reads as policy rather than breakage.

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
