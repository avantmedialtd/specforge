## ADDED Requirements

### Requirement: Pull-Request View

The **pull-request view** SHALL render one pull request from the detail the application service reads for it (see *Detail Reads Are Scoped to the Snapshot*), and SHALL render it the same way in two presentations, in the desktop application and in the browser skin alike:

- **the center pane** of the main window, at the pull request's address (see the *Pull-Request Addresses* requirement in the `view-routing` capability), which is where a pull request opens by default, as an artifact does;
- **the pull-request window**, which detaches it (see *Pull-Request Window*).

**Content.** The view SHALL show:

- **a header** naming the pull request by its number, title and repository, with its head and base branch names, and carrying the signals its row shows in its provider's pull-request panel (see the *GitHub Pull-Request Panel* requirement in the `github-pull-requests` capability and the *Pull-Request Panel* requirement in the `bitbucket-pull-requests` capability);
- **the description**, rendered as untrusted content (see *Pull-Request Content Is Untrusted*);
- **the conversation**, under the description: the pull request's own comments and every submitted review summary that has a body, as one list in submission order. A review without a body SHALL add no entry. A comment or review that GitHub reports as minimised SHALL render collapsed behind GitHub's stated reason;
- **the checks**, each listed by its name and its state, with a link out to its page when its provider gives one. On either host a check SHALL offer a link only for an absolute `http` or `https` URL with a host; a check whose URL is anything else SHALL show no link. The link SHALL open as a link in pull-request content does (see *Desktop Link Opener*): in the desktop application only through `open_pull_request_link`, and in the browser skin in a new opener-isolated tab;
- **the changed files** (see *Changed Files in the Pull-Request View*).

A detail the service reports as no longer listed SHALL be shown marked "no longer listed". The view SHALL offer a manual refresh, which asks for a read under the freshness rule of *Detail Reads Are Scoped to the Snapshot*.

**The provider's page.** The header SHALL carry an "Open on GitHub" or "Open on BitBucket" control, naming the pull request's provider, that opens the pull request's web page outside SpecForge:

- in the desktop application, through `open_pull_request` while the pull request is listed, and once it is no longer listed, through `open_pull_request_link` with the pull request's own URL from its cached detail (see *Desktop Link Opener*);
- in the browser skin, as a link that opens a new opener-isolated tab.

**The pop-out control.** In the center pane only, the header SHALL also carry the shared "Open in its own window" control. Activating it SHALL open the pull-request window exactly as the new-window gesture does (see *Opening a Pull Request Like a Document*), and SHALL leave the center pane's contents, reading position, selection and history unchanged. The pull-request window SHALL carry the provider control and no pop-out control, since its pull request is already detached. On a device that reports no hover capability both controls SHALL be visible at rest, and on a device whose primary pointer is coarse the pop-out control SHALL keep the enlarged hit area the shared control presents (see the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability).

**Clearance.** In the native desktop main window on macOS, the header SHALL keep its controls clear of the titlebar drag region at every scroll position, leaving the region itself draggable, as the change header does (see the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability). The pull-request window has a native titlebar and SHALL take no such inset, and neither SHALL the served web UI.

**Read-only.** The view SHALL offer no control that posts a comment, approves, requests changes, merges, or marks anything on either host. Marking a file viewed is local to this machine (see *Review Progress*).

#### Scenario: One view in both presentations

- **WHEN** the same pull request is shown in the center pane and in its pull-request window
- **THEN** both show its header, description, conversation, checks and changed files, from the same detail

#### Scenario: The header carries the row's signals

- **WHEN** a GitHub pull request's panel row shows failing checks and a conflict marker
- **THEN** the view's header shows the same failing checks treatment and conflict marker

#### Scenario: The conversation follows submission order

- **WHEN** a pull request has a comment posted at 10:00, a review submitted at 10:05 with the body "Looks close", and a comment posted at 10:10
- **THEN** the conversation under the description lists the first comment, the review summary and the second comment, in that order

#### Scenario: A review without a body adds nothing to the conversation

- **WHEN** a reviewer approves the pull request without writing a body
- **THEN** no entry for that review appears in the conversation

#### Scenario: A minimised comment stays collapsed

- **WHEN** a comment in the conversation is minimised on GitHub with the reason "outdated"
- **THEN** it renders collapsed, showing that reason in place of its body

#### Scenario: Checks link out

- **WHEN** the view lists a check named `build` that failed and whose provider gives it a details page
- **THEN** the check shows its name and its failed state
- **AND** activating its link opens that page outside SpecForge without navigating the view

#### Scenario: A check link with another scheme opens nothing

- **WHEN** a check's details URL uses a `file:`, `data:` or custom scheme
- **THEN** the check shows no link and nothing is opened

#### Scenario: The provider's page opens from the header

- **WHEN** the user activates "Open on GitHub" in the desktop application for a listed pull request
- **THEN** the pull request's web page opens in the system browser through `open_pull_request`
- **AND** the SpecForge window does not navigate

#### Scenario: A pull request no longer listed still opens its page

- **WHEN** the view shows a cached pull request marked "no longer listed" and the user activates its provider control in the desktop application
- **THEN** the page opens through `open_pull_request_link` with the pull request's URL from its cached detail

#### Scenario: The provider's page in the browser skin

- **WHEN** the user activates "Open on BitBucket" in the browser skin
- **THEN** the pull request's web page opens in a new tab with `noopener noreferrer` semantics
- **AND** the SpecForge page does not navigate

#### Scenario: Only the center pane offers the pop-out control

- **WHEN** a pull request is shown in the center pane and in its pull-request window
- **THEN** the center pane's header carries "Open in its own window"
- **AND** the pull-request window's header carries the provider control and no "Open in its own window" control

#### Scenario: The pop-out control disturbs nothing

- **WHEN** the user activates "Open in its own window" while the center pane is scrolled to the changed files
- **THEN** the pull request's window opens, or is focused if it is already open
- **AND** the center pane's contents, reading position, selection and history are unchanged

#### Scenario: The controls are reachable without hover

- **WHEN** the view is displayed on a device that reports no hover capability
- **THEN** the provider control, and in the center pane "Open in its own window", are visible at rest

#### Scenario: The header clears the macOS titlebar

- **WHEN** the center pane shows a pull request in the native window on macOS, scrolled to any position
- **THEN** the header's controls lie clear of the titlebar drag region and respond to a click
- **AND** the drag region remains draggable

#### Scenario: Nothing can be posted

- **WHEN** the view shows a pull request
- **THEN** it offers no control that comments, approves, requests changes, merges or marks anything on either host

### Requirement: Changed Files in the Pull-Request View

The pull-request view SHALL render the pull request's changed files through the `diff-view` capability, as one of its hosts (see the *Diff View Hosts* requirement in the `diff-view` capability), from the model the detail reads build (see *GitHub Detail Reads* and *BitBucket Detail Reads*), with the same line and byte budgets applied as for commit detail (see the *Line and Byte Budgets With On-Request Loading* requirement in the `diff-view` capability). The files SHALL read as commit detail's do, unified or side by side, under the per-surface layout choice that commit detail and pull requests share and that is not an application setting (see the *The Layout Choice Is Per Surface* requirement in the `diff-view` capability). The view SHALL pass the pull request's base and head branch names as the names of the diff's old and new sides.

**Review threads.** Each review thread on a listed file, which on BitBucket is an inline comment with its replies, SHALL render in its file's preamble slot: above that file's diff and across the section's full width, in either layout, rather than between the diff's lines. A thread whose file is not among the listed files, such as one on a file past GitHub's thousandth or an outdated thread on a file the pull request no longer changes, SHALL render below the diff view, after its files. Each thread SHALL name its file, its side and its line. The side SHALL be old or new: GitHub's `diffSide` gives it, `LEFT` for old and `RIGHT` for new, and BitBucket's anchor gives it, `inline.from` for old and `inline.to` for new. A thread comment that GitHub reports as minimised SHALL render collapsed behind GitHub's stated reason, as a conversation entry does.

**Withheld files.** A file the budgets withheld SHALL show its counts and a control that loads it. Loading it SHALL go through `get_pull_request_file`, which answers from the cached detail without a request to the provider (see *Detail Reads Are Scoped to the Snapshot*).

**Viewed marks.** Each file's header extra SHALL carry its viewed mark and, when it applies, its changed-since-viewed flag (see *Review Progress*).

#### Scenario: Pull requests read in the layout commit detail uses

- **WHEN** the reader has chosen side by side for commit detail on this surface, and opens a pull request in a view wide enough for two columns
- **THEN** the pull request's files render side by side
- **AND** the diff view names its two sides by the pull request's base and head branch names

#### Scenario: A thread names its file, side and line

- **WHEN** a GitHub review thread on `src/api.ts` has the `diffSide` `LEFT` and the line 12
- **THEN** it renders above that file's diff, across the section's full width, naming `src/api.ts`, the old side and line 12
- **AND** it renders the same way in the unified and the side-by-side layout

#### Scenario: A BitBucket inline comment takes its side from its anchor

- **WHEN** a BitBucket inline comment on `README.md` carries `inline.to` 7 and no `inline.from`
- **THEN** its thread renders above that file's diff, naming `README.md`, the new side and line 7

#### Scenario: A minimised thread comment stays collapsed

- **WHEN** a comment in a review thread on `src/api.ts` is minimised on GitHub with the reason "spam"
- **THEN** it renders collapsed above that file's diff, showing that reason in place of its body

#### Scenario: A thread on an unlisted file follows the files

- **WHEN** a GitHub pull request changes 1,200 files and a review thread sits on a file past the thousandth
- **THEN** that thread renders below the diff view, after the listed files, naming its file, its side and its line

#### Scenario: A withheld file loads from the cache

- **WHEN** the budgets withheld a file and the reader asks to load it
- **THEN** its hunks render from the cached detail
- **AND** no request is sent to the provider

#### Scenario: Each file carries its viewed mark

- **WHEN** the reader has marked one of a pull request's files viewed
- **THEN** that file's header shows it viewed
- **AND** every other file's header shows it unviewed

### Requirement: Linked Change in the Pull-Request View

When the pull request is linked to one or more worktrees (see the *Matching a Pull Request to a Worktree* requirement in the `pull-request-worktree-links` capability), the view's header SHALL name the OpenSpec change the panel's worktree marker would land on (see the *Pull-Request Rows Lead to Their Worktree* requirement in the `pull-request-worktree-links` capability): the first linked worktree's one active change, else its most recently modified active change. When that worktree hosts no active change, the header SHALL name its branch only. A pull request linked to no worktree SHALL name no change.

- **In the center pane**, activating the change's name SHALL navigate the main window to that change exactly as the worktree marker does, adding a history entry, so a back gesture returns to the pull request.
- **In the pull-request window**, the name SHALL be passive text. Activating it SHALL navigate nothing and SHALL NOT open a reader window, since the window cannot navigate the main window and a reader window opens only from the places the `reader-window` capability defines.

#### Scenario: The center pane leads to the linked change

- **WHEN** the center pane shows a pull request whose linked worktree hosts the single active change `add-rate-limits`, and the user activates that name
- **THEN** the center pane shows `add-rate-limits`'s default artifact read from that worktree
- **AND** a back gesture shows the pull request again

#### Scenario: Several changes name the most recently modified

- **WHEN** the linked worktree hosts the active changes `add-rate-limits` and `fix-cache`, and `fix-cache` was modified more recently
- **THEN** the header names `fix-cache`

#### Scenario: The window names the change passively

- **WHEN** the same pull request is shown in its pull-request window and the user clicks the change's name
- **THEN** neither that window nor the main window navigates
- **AND** no reader window opens

#### Scenario: A worktree without a change shows its branch

- **WHEN** the linked worktree hosts no active change
- **THEN** the header names that worktree's branch and no change

### Requirement: Opening a Pull Request Like a Document

A pull request SHALL open the way a document opens, from its row in either provider's pull-request panel and from its chip in the change header (see the *Pull-Request Chip in the Change Header* requirement in the `spec-browser` capability):

- **A click.** A plain click, Enter or Space on a row or a chip SHALL first clear the tree's unaddressed state, which is the empty-change pane and the clicked tree row, and SHALL then navigate the center pane to the pull request's address, adding a history entry (see the *History Entry Discipline* requirement in the `view-routing` capability). The clearing comes first because navigating to the address already shown changes nothing.
- **The new-window gesture.** A click held with the platform's new-window modifier, Cmd on macOS and Ctrl elsewhere, SHALL open the pull-request window for that pull request (see *Pull-Request Window*). The modifier SHALL be chosen by platform rather than accepting either: on macOS a Ctrl-click is the secondary click, and it SHALL open nothing. The window SHALL be opened synchronously inside the click, so no popup blocker intervenes, and before any change to the selection, the center pane or the history. The gesture SHALL change nothing else.
- **The address** SHALL be built only from a row whose provider URL is not empty, with the owner and repository spelled as the row spells them. A row whose URL is empty SHALL open nothing, by a click or by the gesture.

**Elements.**

- In the desktop application, rows and chips SHALL remain buttons rather than links, so the webview's own link items cannot reload the main window at a pull-request path and lose its in-memory history.
- In the browser skin, rows and chips SHALL be links whose target is the pull request's SpecForge address path, never the provider's URL. Their click and their new-window gesture SHALL be handled as above, and Space SHALL activate them as a click does, since a link activates on Enter only. The browser's own middle-click, "Open Link in New Tab" and Copy Link therefore give a full SpecForge tab, or a shareable deep link, at that address.
- A row SHALL NOT carry the provider's URL as its tooltip.

#### Scenario: A click shows the pull request in the center pane

- **WHEN** the user clicks the row of GitHub pull request 42 in `acme/api`
- **THEN** the center pane shows that pull request at `/pr/github/acme/api/42`
- **AND** a back gesture returns to the view shown before

#### Scenario: Enter and Space open as a click does

- **WHEN** a pull-request row or chip has keyboard focus, in the desktop application or the browser skin, and the user presses Enter or Space
- **THEN** the center pane shows that pull request, as a click would

#### Scenario: A chip opens the pull request in the center pane

- **WHEN** the user clicks a pull-request chip in the change header
- **THEN** the center pane shows that pull request at its address
- **AND** no web page opens

#### Scenario: The tree's unaddressed state is cleared first

- **WHEN** the center pane shows pull request 42, the user clicks a change row that shows the empty-change pane without changing the address, and the user then clicks the row of pull request 42
- **THEN** the change row is no longer selected
- **AND** the center pane shows pull request 42 again

#### Scenario: The new-window gesture opens the pull request's own window

- **WHEN** the user Cmd-clicks a pull-request row on macOS, or Ctrl-clicks one elsewhere
- **THEN** that pull request's window opens, or is focused if it is already open
- **AND** the center pane, the tree's selection and the navigation history are unchanged

#### Scenario: The secondary click opens nothing on macOS

- **WHEN** the user Ctrl-clicks a pull-request row or chip on macOS
- **THEN** no pull-request window opens and the center pane does not navigate

#### Scenario: A row without a provider URL opens nothing

- **WHEN** a GitHub row's URL is empty because its link was foreign
- **THEN** neither a click nor the new-window gesture on it opens anything

#### Scenario: Desktop rows and chips are buttons

- **WHEN** the desktop application renders a pull-request row or chip
- **THEN** it is a button rather than a link

#### Scenario: Browser-skin rows link to SpecForge addresses

- **WHEN** the user middle-clicks a pull-request row in the browser skin
- **THEN** a new tab opens on the full SpecForge application at that pull request's address
- **AND** no tab opens on the provider's page

#### Scenario: The gesture in the browser skin opens the pull request's tab

- **WHEN** the user Cmd/Ctrl-clicks a pull-request row in the browser skin
- **THEN** the pull request's own tab opens without the browser's popup blocker intervening
- **AND** the serving page does not navigate

#### Scenario: Rows carry no provider-URL tooltip

- **WHEN** the user hovers a pull-request row
- **THEN** no tooltip shows the provider's URL

### Requirement: Pull-Request Window

A **pull-request window** SHALL present the pull-request view of one pull request apart from the main window: a native window in the desktop application, and a browser tab in the browser skin. It SHALL render that view, or one of its address's resolution notices, and SHALL NOT render the workspace tree, the commit rail, the pull-request panels or any other surface of the main window.

**Identity.** A pull-request window SHALL be identified by its pull request's encoded address. The presentation SHALL ride outside the address's path, so the same path names the same pull request in both presentations and presentation never becomes part of the address:

- in the desktop application, a window labelled `pull-request-<hash>` that loads `index.html?pullRequest=1&at=<address>`, opened through the desktop-only `open_pull_request_window` command, which the web transport SHALL NOT expose;
- in the browser skin, a tab at the address's path with `?pullRequest=1`, named `specforge-pull-request:<hash>`;

where `<hash>` is derived from the encoded address. Requesting the window of a pull request that already has one SHALL bring that window to the front and focus it rather than open a second, and two pull requests SHALL be able to have windows open at once. Both launches of a listed pull request, the new-window gesture and the "Open in its own window" control, SHALL encode the address from the matched snapshot row, so one pull request has one window however a link spelled its address. For a pull request shown from its cached detail after it left its provider's list, the control SHALL encode the address with the spelling of the row that detail was read through, which the detail SHALL keep.

**Root.** The `pullRequest` flag SHALL select the window's own root before the application's router runs, as the reader flag selects the reader window's root. A root SHALL render only its own kind of address: a pull-request window given an address that is not a pull request's SHALL read "Pull request not found", and a reader window given a pull-request address SHALL read "Document not found".

**Resolution.** The window SHALL resolve its own address with the lookup the center pane uses, against provider snapshots and enabled flags it reads and keeps current itself (see *Provider Enabled Flags Stay Current*), and SHALL show the same pending, provider-off, unavailable and not-listed outcomes as the center pane (see the *Cold-Load Address Resolution* requirement in the `view-routing` capability). Its notices SHALL name Settings › Integrations as text rather than link to it, since the window cannot navigate the main window. When its provider is switched off, the window SHALL replace the pull request with the provider-off notice.

**Closing.** Escape and the platform's close-window shortcut, Cmd-W on macOS and Ctrl-W elsewhere, SHALL close the window, as they close a reader window, unless a control inside the window claims Escape first. Closing SHALL destroy the window.

**The main window.** Opening a pull-request window, or focusing one already open, SHALL change nothing in the main window: not what its center pane shows, its selection, its scroll position or its history.

**Titlebar.** The window SHALL carry a native titlebar (see *Pull-Request Window Title*), and the main window's titlebar clearance SHALL NOT apply to it.

#### Scenario: The window shows only the pull request

- **WHEN** a pull-request window opens for a listed pull request
- **THEN** it renders that pull request's view
- **AND** it renders no workspace tree, commit rail or pull-request panel

#### Scenario: Reopening focuses the existing window

- **WHEN** a pull-request window is open for a pull request and the user performs the new-window gesture on its row again
- **THEN** the existing window is brought to the front and focused
- **AND** no second window opens

#### Scenario: Different pull requests get different windows

- **WHEN** the user opens windows for two different pull requests
- **THEN** both windows are open at once, each showing its own pull request

#### Scenario: One window however the address was spelled

- **WHEN** the center pane was loaded at a pull-request address whose owner differs from its row's only in case, and the user activates "Open in its own window" while that pull request's window is already open
- **THEN** the open window is focused
- **AND** no second window opens

#### Scenario: The browser skin reuses the pull request's tab

- **WHEN** the user performs the new-window gesture on the same pull request twice in the browser skin
- **THEN** the first gesture opens one tab at the pull request's path with `?pullRequest=1`
- **AND** the second reuses and focuses that tab

#### Scenario: Both presentations share one path

- **WHEN** the browser skin shows a pull request in the center pane of one tab and in its own pull-request tab
- **THEN** both locations carry the same path
- **AND** they differ only outside it

#### Scenario: The web transport cannot open a window on the host

- **WHEN** `open_pull_request_window` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command and no window opens on the serving host

#### Scenario: The window resolves its own address

- **WHEN** a pull-request window opens before its provider's first list has completed
- **THEN** it shows "Loading…" until the list arrives
- **AND** then shows the pull request, or the not-listed outcome when the list does not hold it

#### Scenario: The window's notices do not link to Settings

- **WHEN** a pull-request window's provider is unauthenticated
- **THEN** the window shows the unavailable notice, naming Settings › Integrations as text
- **AND** the notice carries no control that navigates

#### Scenario: Switching the provider off replaces the window's content

- **WHEN** the user switches GitHub off in Settings while a pull-request window shows a GitHub pull request
- **THEN** that window replaces the pull request with the provider-off notice
- **AND** the window stays open

#### Scenario: Each root renders only its own kind of address

- **WHEN** a pull-request window is given an address that names a document
- **THEN** it reads "Pull request not found"
- **AND** a reader window given a pull-request address reads "Document not found"

#### Scenario: Escape closes the window

- **WHEN** a pull-request window has focus and no control inside it has claimed Escape
- **AND** the user presses Escape
- **THEN** the window is destroyed

#### Scenario: A control that claims Escape keeps the window open

- **WHEN** a control inside a pull-request window has claimed Escape
- **AND** the user presses Escape
- **THEN** that control handles the key and the window stays open

#### Scenario: The close shortcut closes the window

- **WHEN** a pull-request window has focus and the user presses Cmd-W on macOS, or Ctrl-W elsewhere
- **THEN** the window is destroyed

#### Scenario: Opening the window leaves the main window alone

- **WHEN** the user opens a pull-request window while the main window's center pane shows an artifact
- **THEN** the center pane still shows that artifact
- **AND** its selection, scroll position and history are unchanged

### Requirement: Pull-Request Window Title

The title of a pull-request window SHALL be `#<number> <title> — <owner>/<repo>`, or `#<number> — <owner>/<repo>` while the pull request's title is not yet known, where the owner of a BitBucket pull request is its workspace. Every Unicode default-ignorable character and every control character SHALL be removed from the title, and the title SHALL be capped in length. It SHALL update when a detail read brings a new title.

In the desktop application the title SHALL be set when the window is built, and SHALL afterwards follow the page's title through the window builder's title-change hook, which sanitises it again and sets the native title from the Rust side, so the window needs no permission to set its own title. In the browser skin the title SHALL be the tab's page title.

#### Scenario: The title names the pull request

- **WHEN** a pull-request window shows GitHub pull request 42, titled "Add rate limits", in `acme/api`
- **THEN** the window's title is `#42 Add rate limits — acme/api`

#### Scenario: The title before the detail arrives

- **WHEN** the window of that pull request opens before its title is known
- **THEN** the window's title is `#42 — acme/api`

#### Scenario: Hidden and control characters never reach the title

- **WHEN** the pull request's title contains a right-to-left override (U+202E), a zero-width space (U+200B) and a newline
- **THEN** the window's title contains none of them

#### Scenario: An overlong title is capped

- **WHEN** the pull request's title is several thousand characters long
- **THEN** the window's title is cut to the cap

#### Scenario: A new title follows a read

- **WHEN** the pull request is retitled and a later detail read brings the new title
- **THEN** the window's title changes to the new one

#### Scenario: The browser skin titles the tab

- **WHEN** the browser skin shows a pull request in its own tab
- **THEN** the tab's title follows the same rule

### Requirement: Pull-Request Window Geometry

Pull-request windows SHALL share one remembered size of their own, separate from the reader windows' size and held in the application settings so it survives a restart. Only the desktop-only `set_pull_request_window_size` command SHALL set it, and the web transport SHALL NOT expose that command. A resized pull-request window SHALL save its size 400 milliseconds after the resize ends, and the next pull-request window SHALL adopt it.

The size SHALL default to about $$1280 \times 860$$, wide enough for the file navigator beside a side-by-side diff. A new window SHALL be clamped to the work area of the monitor the launching window is on:

$$w = \min(w_{\text{size}},\ w_{\text{work area}}), \qquad h = \min(h_{\text{size}},\ h_{\text{work area}})$$

The window's minimum size SHALL be $$600 \times 400$$, and the setter SHALL floor what it stores at the same size. A pull-request window that opens while another pull-request window is visible SHALL be offset from it, and reader windows SHALL NOT count for that offset.

Geometry SHALL NOT be remembered per pull request: pull-request windows SHALL be excluded from the persisted window state, so no per-pull-request entry accumulates.

#### Scenario: The first window fits a side-by-side diff

- **WHEN** a pull-request window opens with no remembered size, on a monitor whose work area exceeds $$1280 \times 860$$
- **THEN** it opens at about $$1280 \times 860$$

#### Scenario: A small work area clamps the window

- **WHEN** the remembered size is $$1280 \times 860$$ and the launching window's monitor has a work area of $$1024 \times 700$$
- **THEN** the new pull-request window opens no larger than $$1024 \times 700$$

#### Scenario: A resize sets the size for the next window and survives a restart

- **WHEN** the user resizes a pull-request window, quits SpecForge, relaunches it and opens a pull-request window
- **THEN** the new window opens at the resized dimensions
- **AND** the size was saved 400 milliseconds after the resize ended, not while it was in progress

#### Scenario: Reader windows keep their own size

- **WHEN** the user resizes a pull-request window and then opens a reader window
- **THEN** the reader window opens at the reader windows' remembered size, unchanged

#### Scenario: The size has a floor

- **WHEN** `set_pull_request_window_size` is asked to store $$300 \times 200$$
- **THEN** it stores $$600 \times 400$$
- **AND** no pull-request window can be resized below $$600 \times 400$$

#### Scenario: A second window is offset from the first

- **WHEN** a pull-request window is visible and the user opens one for a different pull request
- **THEN** the new window is offset from the visible one rather than placed exactly over it

#### Scenario: Many pull requests accumulate no window state

- **WHEN** the user has opened pull-request windows for many pull requests over time
- **THEN** the persisted window state contains no entry for any of them

#### Scenario: The web transport cannot set the size

- **WHEN** `set_pull_request_window_size` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command

### Requirement: Pull-Request Window Permissions

The pull-request window SHALL hold only the permissions it uses, and SHALL add two backstops to the untrusted-content handling every window applies (see *Pull-Request Content Is Untrusted*): a narrow capability and a content-security policy.

**Capability.** In the desktop application, one capability SHALL match the `pull-request-*` windows and SHALL grant exactly `core:event:allow-listen`, `core:event:allow-unlisten` and `core:window:allow-close`. It SHALL grant no dialog, autostart, notification, menu or tray permission, and the capability the main window and reader windows share SHALL NOT match a pull-request window. The window SHALL save its size through an application command and have its title set from the Rust side, so it needs no other window permission.

**Navigation guard.** The window SHALL install the same navigation guard as the main and reader windows: only the application's own origin, and in a development build its development server, SHALL load in it, so a link the webview activates itself never loads another page in the window.

**Content-security policy.** On both hosts, before the window renders anything, its page's `head` SHALL carry a content-security policy of `img-src 'self' data: blob:; font-src 'self' data:; media-src 'none'; object-src 'none'`, in place before any fetch it governs starts. The policy SHALL be installed only for a pull-request window, never for the main window or a reader window.

#### Scenario: The capability grants only what the window uses

- **WHEN** the desktop application's capabilities are inspected
- **THEN** the capability matching `pull-request-*` windows grants exactly `core:event:allow-listen`, `core:event:allow-unlisten` and `core:window:allow-close`
- **AND** no capability granting the dialog, autostart or notification plugin matches a `pull-request-` window

#### Scenario: The narrow capability is enough

- **WHEN** a pull-request window is opened on a cold start of the desktop application
- **THEN** it loads, receives the notices it listens for, and closes on Escape

#### Scenario: A link the webview follows itself does not load

- **WHEN** a navigation to an external page is attempted in a pull-request window by a path that bypasses the view's link handling, such as the webview's native context menu
- **THEN** the window does not load that page

#### Scenario: The policy is in place before anything renders

- **WHEN** a pull-request window renders its first content, on either host
- **THEN** its page's `head` already carries the content-security policy
- **AND** the window's own bundled fonts and icons still load

#### Scenario: Only the pull-request window carries the policy

- **WHEN** the main window and a reader window are loaded
- **THEN** neither page's `head` carries a content-security policy, and no policy governs what either loads; in the browser skin their only policy is the served shell's `frame-ancestors 'none'` (see the *Localhost Trust Boundary* requirement in the `web-ui` capability)

#### Scenario: The policy refuses a remote image

- **WHEN** an element requesting an image from a remote host appears in a pull-request window's page
- **THEN** the image is not fetched

### Requirement: Provider Enabled Flags Stay Current

Every root that resolves pull-request addresses, the main window's application and the pull-request window's root, SHALL read each provider's enabled flag from `get_github_config` and `get_bitbucket_config` when it mounts, and SHALL keep it current through a `pull-request-provider-changed` notice carrying the provider and its enabled flag. The application service SHALL raise that notice whenever a provider's enabled flag is set, on the same service-owned broadcast as `review-progress-changed` (see *Review Progress*), so it reaches every window and every tab that one service serves, including a tab served by the desktop's embedded server. The notice SHALL NOT be emitted directly by the command that sets the flag, which would reach only that command's own transport, and SHALL NOT be a variant of the cache-event stream.

#### Scenario: A root reads the flags when it mounts

- **WHEN** a pull-request window opens
- **THEN** it reads both providers' enabled flags
- **AND** it shows "Loading…" rather than an outcome until it has them

#### Scenario: Switching a provider off reaches the center pane

- **WHEN** the center pane shows a BitBucket pull request and the user switches BitBucket off in Settings
- **THEN** the main window receives `pull-request-provider-changed` for BitBucket with the flag off
- **AND** the pull request's address resolves to the provider-off notice

#### Scenario: A flag set from a browser tab reaches the desktop

- **WHEN** a browser tab served by the desktop's embedded server switches GitHub off
- **THEN** the desktop main window and every open pull-request window receive the notice
- **AND** a pull-request window showing a GitHub pull request shows the provider-off notice

### Requirement: Detail Reads Are Scoped to the Snapshot

A pull request's **reference** SHALL be its provider, its owner (a GitHub owner or a BitBucket workspace), its repository and its number. Two references SHALL be equal when their providers and numbers are equal and their owners and repositories are equal ignoring ASCII case. The commands that read a pull request's detail, its withheld files and its review progress SHALL take a reference, and no new command SHALL identify a pull request by a URL.

`get_pull_request_detail(reference, manual, cachedOnly)`, where `manual` says only whether the read is a manual refresh and `cachedOnly` asks only for what the cache holds, neither reaching a request, SHALL look the reference up in its provider's current snapshot, fresh or stale, for a row with an equal reference and a non-empty URL, and SHALL take the repository, the number and the URL from that row. Nothing the caller supplies SHALL reach a request beyond the reference itself, and no URL from a frontend SHALL reach a provider request. A detail read therefore spends the credential only on pull requests the account already lists: on GitHub those it authored or whose review is requested from it, and on BitBucket those it authored. A detail read SHALL never write, SHALL follow no redirect, and SHALL never return the credential.

**The cache.** The service SHALL keep the last detail of each pull request in memory only, keyed by reference, holding at most 32 and dropping the least recently used. A view reopened while its pull request's detail is cached SHALL paint that detail at once whenever the service answers without a request: to a `cachedOnly` call, which SHALL answer from the cache alone, with the cached detail and its read time or with none, whatever the detail's age; under the freshness rule below; because the pull request is no longer listed; or because a deadline or the budget holds, in which case the answer SHALL carry the cached detail beside the time a read becomes possible (see *Shared Backoff and Detail Budget*). When the service reads instead, the view SHALL paint the detail that read returns. When a reference is not listed, the service SHALL answer with its cached detail, marked "no longer listed", while its provider stays enabled, and with no detail when none is cached, which the view reports as not in its provider's list. A reference that is not listed SHALL cause no request.

**Freshness.** A view SHALL ask for a read only when it opens, in either presentation; when its provider's snapshot announcement shows that the pull request's row changed its updated time, its checks or, on GitHub, its count of unresolved conversations, or is the first announcement after the time a deferral named (see *Shared Backoff and Detail Budget*); when the service refuses a withheld file the view asked for, as **Withheld files** below says; and on a manual refresh. It SHALL NOT read on a timer. The service SHALL answer from the cache, with no request, while the cached detail is under 60 seconds old and the row is unchanged since it was read. A manual refresh SHALL bypass that rule unless a manual refresh of the same pull request sent a read less than 30 seconds before, $$\text{lastManualRead}$$ being when one last did:

$$\text{fromCache} \iff \text{age} < 60\,\text{s} \;\wedge\; \text{row unchanged} \;\wedge\; \neg\bigl(\text{manual} \;\wedge\; \text{now} - \text{lastManualRead} \ge 30\,\text{s}\bigr)$$

At most one read per pull request SHALL be in flight, whichever presentation asks for it: a caller that asks for a pull request whose read is in flight SHALL receive that read's outcome rather than start another.

**Credential changes.** Disabling a provider, or saving a credential for it, SHALL drop that provider's cached details at once and advance its credential generation. A read SHALL record the generation it started under, and before each of its requests SHALL check that the generation is unchanged and the provider still enabled. A read that finds either changed SHALL send nothing more, and its result SHALL be neither cached nor returned. While a provider is disabled, every read of its pull requests SHALL refuse without content.

**Withheld files.** `get_pull_request_file(reference, path, head, base)` SHALL return a file the budgets withheld, from the cached detail and with no request. It SHALL name the head and base commits the view rendered, and SHALL refuse when no detail is cached or either commit differs from the cached detail's, after which the view re-reads the pull request.

**Transports.** `get_pull_request_detail` and `get_pull_request_file` SHALL be served on both the desktop and the web transport.

#### Scenario: A listed pull request is read through its row

- **WHEN** the view asks for GitHub pull request 42 of `ACME/Api`, and the snapshot lists it as `acme/api`
- **THEN** the read's requests name `acme/api` and number 42, as the row spells them

#### Scenario: A pull request outside the snapshot spends nothing

- **WHEN** `get_pull_request_detail` is invoked, over either transport, with a reference to a pull request that is in no snapshot and has no cached detail
- **THEN** no request is sent to the provider
- **AND** no detail is returned, and the view reports that the pull request is not in its provider's list

#### Scenario: A pull request that left the list keeps its last detail

- **WHEN** a pull request whose detail is cached is merged and leaves its provider's snapshot
- **THEN** its view keeps showing that detail, marked "no longer listed"
- **AND** no request is sent for it

#### Scenario: The cache keeps the 32 most recently used

- **WHEN** details for 32 pull requests are cached and a 33rd pull request is read
- **THEN** the least recently used detail is dropped and the other 31 are kept

#### Scenario: A quick reopen sends nothing

- **WHEN** a pull request's detail was read 20 seconds ago, its row is unchanged, and the user reopens its view
- **THEN** the view paints the cached detail at once
- **AND** no request is sent

#### Scenario: A changed row brings a read

- **WHEN** a view is open and its provider's snapshot announcement shows that the row's checks changed
- **THEN** the view asks for a read
- **AND** the service reads the pull request again, as far as *Shared Backoff and Detail Budget* allows

#### Scenario: Manual refreshes are bounded per pull request

- **WHEN** the user refreshes a pull request manually, then refreshes it again 10 seconds later with its row unchanged
- **THEN** the first refresh reads from the provider
- **AND** the second is answered from the cache with no request

#### Scenario: A manual read of a stale entry starts the bound

- **WHEN** a pull request's cached detail is 70 seconds old, and the user refreshes it manually, then again 10 seconds later with its row unchanged
- **THEN** the first refresh reads from the provider
- **AND** the second is answered from the cache with no request

#### Scenario: A cache-only call answers whatever the entry's age

- **WHEN** a view opens a pull request whose detail was read five minutes ago, and asks with `cachedOnly`
- **THEN** the service answers with that detail and its read time, and sends no request
- **AND** the view paints it at once, then asks for a read under the freshness rule

#### Scenario: An idle view does not poll

- **WHEN** a view stays open for ten minutes with no change to its row and no manual refresh
- **THEN** no detail request is sent for it after its first read

#### Scenario: Two presentations share one read

- **WHEN** the center pane and the pull-request window ask for the same pull request at the same moment
- **THEN** at most one read of it is in flight
- **AND** both presentations receive that read's outcome

#### Scenario: A credential saved mid-read discards the read

- **WHEN** a BitBucket read is between two of its requests and the user saves a BitBucket credential
- **THEN** the read sends no further request
- **AND** its result is neither cached nor returned
- **AND** BitBucket's cached details are dropped at once

#### Scenario: A disabled provider serves nothing

- **WHEN** the user disables GitHub while a GitHub pull request's detail is cached
- **THEN** the cached detail is dropped
- **AND** `get_pull_request_detail` refuses without content for every GitHub reference while GitHub stays disabled

#### Scenario: A withheld file is served from the cache

- **WHEN** the view asks for a withheld file, naming the head and base commits it rendered
- **THEN** the file's hunks are returned from the cached detail with no request

#### Scenario: A withheld-file request after a push is refused

- **WHEN** the view asks for a withheld file naming a head commit that differs from the cached detail's, because a push has been read since
- **THEN** the request is refused
- **AND** the view re-reads the pull request

#### Scenario: The browser skin reads detail

- **WHEN** the browser skin sends `get_pull_request_detail` for a listed pull request
- **THEN** the web transport dispatches it and returns the detail the desktop application would receive

### Requirement: GitHub Detail Reads

A GitHub detail read SHALL send only these requests, each to `api.github.com`:

1. one `POST` to `https://api.github.com/graphql` carrying a detail query fixed at compile time: a read-only `query` operation, never a mutation or subscription, byte-identical on every read. Its only variables SHALL be the owner, repository name and number taken from the matched row, carried in GraphQL's `variables` and never interpolated into the query's text. Among the pull request's other fields, the query SHALL read its submitted reviews in the `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED` and `DISMISSED` states only, and each review-thread comment's state, so that nothing of the account's own pending review, which nobody else can see, is shown; each review thread's and comment's id; each thread's path, lines, `diffSide` and `startDiffSide`; whether each comment and review is minimised, and why; and the total number of changed files.
2. `GET https://api.github.com/repos/{owner}/{name}/pulls/{number}/files?per_page=50&page=n`, for at most twenty pages, which is a thousand files. Files beyond the thousandth SHALL be counted rather than listed, and the view SHALL say how many there are, with a pointer to the provider's page.

$$\text{requests per read} = 1 + p_{\text{files}}, \qquad p_{\text{files}} \le 20$$

**Pending comments.** A review-thread comment whose state is `PENDING` SHALL be dropped before the detail is cached or returned, and a thread left with no comment SHALL NOT be shown.

**Files.** Each files entry SHALL become a file of the `diff-view` model: its paths from `filename` and `previous_filename`, its counts from its own fields, no file modes, and its hunks parsed from its `patch` as a per-file patch without a file header. Its status SHALL map explicitly; GitHub reports a mode-only change as `modified`, with no patch and no counted lines.

| GitHub status | Model status |
|---|---|
| `added` | Added |
| `removed` | Deleted |
| `modified` | Modified |
| `renamed` | Renamed, with no similarity |
| `copied` | Copied, with no similarity |
| `changed` | TypeChanged (git's `T`) |
| `unchanged` | Modified, with no textual change |

An entry without a `patch` SHALL be too large to preview when it has added or removed lines. Otherwise it SHALL be a file with no hunks, shown by its status alone, because GitHub does not say whether it is a rename or type change without content changes, a mode-only change, or a binary or empty file; it SHALL never be called too large or binary. The line and byte budgets SHALL then be applied.

**Replies.** Every detail request SHALL follow no redirect and SHALL carry the token only in its `Authorization` header (see the *GitHub Privacy and Safety* requirement in the `github-pull-requests` capability). Replies SHALL be classified by the status half of the *GitHub Failure Classification* requirement in the `github-pull-requests` capability: a 401, and a 403 without a rate-limit signal, SHALL be unauthenticated; a 403 with a rate-limit signal, and a 429, SHALL be rate-limited, setting a deadline by that requirement's delay formula (see *Shared Backoff and Detail Budget*). The query's reply SHALL follow the poller's GraphQL rules: an error of type `RATE_LIMITED` SHALL be rate-limited, and no data with an error of type `INSUFFICIENT_SCOPES` SHALL be unauthenticated; otherwise `data.repository.pullRequest` SHALL be read, and a null there SHALL be unavailable. A files page SHALL be read as a JSON array. Unlike the poller, a redirect or a 404 on a files GET SHALL be unavailable for that pull request rather than transient, so a moved or deleted repository is reported instead of retried. Any other reply, a transport error, a redirect on the query or any other non-success status, SHALL be transient.

#### Scenario: The detail query cannot write and does not vary

- **WHEN** the detail queries sent for two different pull requests are inspected
- **THEN** their text is byte-identical, a `query` operation containing no `mutation` or `subscription`
- **AND** only their `variables` differ, each carrying its matched row's owner, repository name and number

#### Scenario: The account's pending review is never shown

- **WHEN** the account has started a review on the pull request and not submitted it
- **THEN** that review appears nowhere in the view
- **AND** none of its inline comments appears above any file's diff

#### Scenario: File pages stop at a thousand files

- **WHEN** a pull request changes 1,200 files
- **THEN** twenty pages of 50 files are requested, and no twenty-first
- **AND** the view lists 1,000 files and says that 200 more are not listed, pointing to the provider's page

#### Scenario: Statuses map explicitly

- **WHEN** a files page reports one entry as `changed` and another as `unchanged`
- **THEN** the first is a type change and the second is modified, with no textual change

#### Scenario: A patchless file is never mislabelled

- **WHEN** a files entry has no `patch` and reports no added or removed lines
- **THEN** the file is shown by its status alone, with no hunks
- **AND** it is called neither too large nor binary

#### Scenario: A patchless file with lines is too large

- **WHEN** a files entry has no `patch` and reports 4,000 added lines
- **THEN** the file is too large to preview and keeps its counts

#### Scenario: A moved or deleted repository is reported, not retried

- **WHEN** a files GET answers 404, or answers with a redirect
- **THEN** the redirect is not followed and the read is unavailable for that pull request

#### Scenario: A missing pull request is unavailable

- **WHEN** the query's reply carries a null `data.repository.pullRequest`
- **THEN** the read is unavailable for that pull request

#### Scenario: A missing scope is a credential problem

- **WHEN** the query's reply carries no data and an error of type `INSUFFICIENT_SCOPES`
- **THEN** the read is unauthenticated

### Requirement: BitBucket Detail Reads

A BitBucket detail read SHALL send only these GETs, each to `api.bitbucket.org`, none following a redirect, and each carrying the credential only in its `Authorization` header:

1. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`, for the description, author, branches and commits, participants, and the `links.diff` and `links.diffstat` URLs;
2. the diffstat, at `links.diffstat`, for counts, and for the status and rename of every file the diff does not reach, up to ten pages;
3. the diff, at `links.diff`: raw unified text, parsed by the `diff-view` model's unified-diff parser and read up to 8 MiB. Each parsed file SHALL keep the status, paths and modes its diff text gives, since the diffstat has no mode-only or type-change status, and SHALL take its counts from the diffstat entry for its path. Files past that ceiling SHALL be too large to preview and SHALL take their status, rename and counts from the diffstat, and the line and byte budgets SHALL then be applied;
4. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}/comments?pagelen=100`, up to ten pages, for general comments; inline comments, each with its path and its old-side line `inline.from` or new-side line `inline.to`, with `start_from` and `start_to` for a range; replies; resolution; and the deleted flag;
5. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}/statuses`, for build statuses.

$$\text{requests per read} = 3 + p_{\text{diffstat}} + p_{\text{comments}}, \qquad 1 \le p_{\text{diffstat}},\ p_{\text{comments}} \le 10$$

The read SHALL NOT request the pull-request-scoped `/diff` or `/diffstat` endpoints, which answer with a redirect; the payload's links name the destination directly. A link taken from a payload, a page's `next` link included, SHALL be followed only when it parses as an `https` URL whose host is exactly `api.bitbucket.org`, with no user information, no explicit port, and a path under `/2.0/`. Any other link SHALL NOT be requested.

**Replies.**

- A 401, or a 403 on any detail GET, SHALL be unauthenticated and SHALL point to Settings, since a 403 means the token lacks a scope the read needs.
- A 429 SHALL set the shared deadline by the poller's rule: its `Retry-After`, else 300 seconds, and never more than an hour (see *Shared Backoff and Detail Budget*).
- Unlike the poller, a redirect or a 404 SHALL be unavailable for that pull request.
- A transport error or any other status SHALL be transient.

#### Scenario: A read sends its five GETs

- **WHEN** a listed BitBucket pull request whose diffstat and comments each fit one page is read
- **THEN** exactly five GETs are sent, all to `api.bitbucket.org`

#### Scenario: A foreign payload link is not followed

- **WHEN** the pull request's `links.diff` names a host other than `api.bitbucket.org`, user information, an explicit port, or a path outside `/2.0/`
- **THEN** no request is sent to it

#### Scenario: A huge diff stops at its ceiling

- **WHEN** a pull request's diff text exceeds 8 MiB
- **THEN** no more than 8 MiB of it is read
- **AND** each file past the ceiling is too large to preview and keeps its diffstat counts

#### Scenario: Pagination is capped

- **WHEN** a pull request's comments span fourteen pages
- **THEN** ten pages are requested, and no eleventh

#### Scenario: Inline comments carry their side

- **WHEN** an inline comment carries `inline.from` 30
- **THEN** its thread names the old side and line 30

#### Scenario: A token without a needed scope is a credential problem

- **WHEN** the diffstat GET answers 403
- **THEN** the read is unauthenticated and the view points to Settings

#### Scenario: A redirect is reported rather than followed

- **WHEN** any detail GET answers with a redirect
- **THEN** the redirect is not followed, the credential is sent nowhere else, and the read is unavailable for that pull request

#### Scenario: A rate limit sets the shared deadline

- **WHEN** a detail GET answers 429 with no `Retry-After`
- **THEN** BitBucket's shared deadline is set 300 seconds ahead
- **AND** neither the poller nor a detail read sends a BitBucket request before it passes

### Requirement: Shared Backoff and Detail Budget

A rate-limited reply, to a poller or to a detail request, SHALL set its provider's deadline or deadlines as the list below says. Deadlines belong to the provider rather than to the caller, and each is set by that provider's existing delay rule and never more than an hour ahead: for GitHub, the delay formula of the *GitHub Failure Classification* requirement in the `github-pull-requests` capability; for BitBucket, the poller's rule (see the *Polling With Caching and Backoff* requirement in the `bitbucket-pull-requests` capability).

- **GitHub** SHALL keep two deadlines. The GraphQL deadline SHALL be set by a rate-limited reply to the poller's query or to a detail query. The REST deadline SHALL be set by a rate-limited reply to a files GET that reports no secondary limit, whether or not it signals its primary limit by `x-ratelimit-resource: core`. A secondary rate limit SHALL set both.
- **BitBucket** SHALL keep one deadline, shared by its poller and its detail reads.

Each provider SHALL also have an hourly budget of detail requests. Deadlines and budgets belong to the provider: they SHALL survive disabling and re-enabling it and saving a credential for it, and neither event SHALL reset them. Re-enabling a provider while its poller's deadline holds SHALL publish and announce an `unavailable` snapshot at once, as a rate-limited refresh with no previous rows does, and its first refresh SHALL then wait out the deadline, so its panel and any of its pull-request addresses say it is unavailable rather than loading.

A detail read $$r$$ of a pull request from provider $$p$$ SHALL be sent only when

$$\text{send}(r) \iff \text{enabled}(p) \;\wedge\; \text{now} \ge \max_{d \in D(r)} \text{deadline}(d) \;\wedge\; \text{inflight}(p) < 2 \;\wedge\; \text{spent}_{\text{hour}}(p) < \text{budget}(p)$$

where $$D(r)$$ is both GitHub deadlines for a GitHub read and BitBucket's deadline for a BitBucket read, $$\text{inflight}(p)$$ counts the provider's detail reads in flight, and $$\text{spent}_{\text{hour}}(p)$$ counts its detail requests within the hour. The rule SHALL govern detail reads only. A poller SHALL check only its own provider's deadline, which on GitHub is the GraphQL deadline, and never the detail budget or the in-flight count; a spent budget SHALL set no deadline, so it never stalls a panel. BitBucket's budget $$B$$ SHALL be a documented constant for which

$$B + (2 \times 23 - 1) + 60\,(2 + W) \le 1000$$

where 23 is the most requests one BitBucket read sends (the pull request, ten diffstat pages, the diff, ten comment pages and the statuses), so $$2 \times 23 - 1$$ is the most that two reads admitted just under $$B$$ can send past it, and $$60\,(2 + W)$$ is the poller's hourly worst case at its 60-second floor for a documented workspace count $$W$$. The 1,000 is BitBucket's hourly limit on `/2.0/repositories/*`, which the shared deadline assumes the poller draws on too (see the *Authored Pull-Request Discovery* requirement in the `bitbucket-pull-requests` capability).

A read held back only by the in-flight limit SHALL wait rather than be refused. It SHALL be sent once fewer than two of its provider's detail reads are in flight and the rest of the rule allows it, with waiting reads sent in the order they were asked. A read held back by a deadline or the budget is answered as the next paragraph says.

While a deadline or the budget holds, a view SHALL say when a read becomes possible, and SHALL send nothing. The first announcement after that time, whether or not it changes the view's row, or the first manual refresh after it, SHALL read.

#### Scenario: The poller waits out a deadline a detail query set

- **WHEN** a GitHub detail query is rate-limited and sets the GraphQL deadline ten minutes ahead
- **THEN** the GitHub poller sends no request for those ten minutes

#### Scenario: An exhausted REST quota leaves the panel polling

- **WHEN** a files GET answers 403 with `x-ratelimit-remaining: 0` and `x-ratelimit-resource: core`
- **THEN** the REST deadline is set, and GitHub detail reads send nothing until it passes
- **AND** the GitHub poller keeps refreshing on its interval

#### Scenario: A files 429 without a resource header sets the REST deadline

- **WHEN** a files GET answers 429 with no `x-ratelimit-resource` header and no report of a secondary rate limit
- **THEN** the REST deadline is set
- **AND** the GraphQL deadline is unchanged, and the GitHub poller keeps refreshing on its interval

#### Scenario: A secondary rate limit sets both deadlines

- **WHEN** a files GET answers 403 with a body reporting a secondary rate limit
- **THEN** both GitHub deadlines are set
- **AND** neither the poller nor a detail read sends a GitHub request before they pass

#### Scenario: A poller's rate limit holds back detail reads

- **WHEN** the BitBucket poller has been rate-limited, its deadline still holds, and the user opens a BitBucket pull request
- **THEN** no detail request is sent
- **AND** the view says when a read becomes possible

#### Scenario: A spent budget never stalls the panel

- **WHEN** GitHub's hourly detail budget is spent
- **THEN** further GitHub detail reads send nothing and say when a read becomes possible
- **AND** the GitHub poller keeps refreshing on its interval

#### Scenario: Toggling the provider resets nothing

- **WHEN** BitBucket's deadline holds and its detail budget is spent, and the user disables BitBucket, re-enables it and saves its credential again
- **THEN** the deadline and the spent budget are unchanged

#### Scenario: Re-enabling inside a deadline reads as unavailable

- **WHEN** the user re-enables GitHub while its GraphQL deadline holds twenty minutes ahead
- **THEN** an `unavailable` GitHub snapshot is published and announced at once
- **AND** a GitHub pull-request address resolves to the unavailable notice rather than "Loading…"
- **AND** the first refresh waits until the deadline passes

#### Scenario: At most two detail reads per provider

- **WHEN** two BitBucket detail reads are in flight and a view asks for a third BitBucket pull request
- **THEN** the third read is not sent while both are in flight
- **AND** it is sent when one of the two ends, with no further ask from the view

#### Scenario: A read follows the end of a deadline

- **WHEN** a deadline that held back a view's read has passed, and the view's row is then announced as changed or the user refreshes
- **THEN** the view reads the pull request

#### Scenario: A deferred view reads at the first announcement after its time

- **WHEN** a view's read was deferred until a deadline passed, and after it passes its provider announces a snapshot in which the view's row is unchanged
- **THEN** the view reads the pull request
- **AND** a later announcement with the row still unchanged brings no further read

### Requirement: Review Progress

The view SHALL let the reader mark each file viewed and unmark it, and SHALL keep that progress on this machine only. Progress SHALL NOT be sent to either host, and SpecForge SHALL NOT write GitHub's own viewed state, which would be an action on the pull request.

**Storage.** Progress SHALL live in `review-progress.json` in the shared configuration directory, owned by the application service as the activity log is, created on the first mark and written atomically. It SHALL be keyed by the canonical reference: the provider, the owner and repository in lowercase, and the number. Each entry SHALL hold the head commit at the last mark (`lastMarkedHead`), the key of each marked file by its path, and when the entry was last touched (`touchedAt`).

**Marking.** `set_file_viewed(reference, path, viewed, head, base)` SHALL name the head and base commits the view rendered: GitHub's head and base commit ids, or BitBucket's source and destination commits. It SHALL refuse when the reference has no cached detail, when the path is not among that detail's files, or when either commit differs from the cached detail's, since a retarget changes patches without a push just as a push does. An unmark SHALL never create an entry.

**Keys.** The service SHALL compute each file's key itself, and no caller SHALL supply one:

$$\text{key}(f) = \begin{cases} \text{SHA-256}\bigl(\text{patch}(f)\bigr) & \text{when } f \text{ has patch text} \\ \bigl(\text{sha}(f),\ \text{status}(f),\ \text{previous}(f),\ \text{base branch}\bigr) & \text{else when GitHub gives } f \text{ a blob sha} \\ \bigl(\text{head commit},\ \text{base branch}\bigr) & \text{otherwise} \end{cases}$$

- A file with patch text, including one the budgets withheld, whose patch sits in the cached detail, SHALL be keyed by the hex-encoded SHA-256 of its patch bytes, never by a non-cryptographic or shortened hash, since the author controls the patch and keys persist across runs and toolchains.
- A file without patch text, whether binary, too large, or a GitHub entry with no `patch`, SHALL be keyed by GitHub's blob `sha` together with its status, its previous filename and the base branch's name, when GitHub gives a `sha`.
- Any other file, which includes every BitBucket file without patch text, SHALL be keyed by the head commit and the base branch's name. Any push or retarget then flags it changed since viewed, and the view SHALL say why.

**States.** A file SHALL be **viewed** when its stored key equals its current key, **changed since viewed** when the two differ, and **unviewed** when nothing is stored for it. The view's header SHALL show "n of m files viewed" and how many files are changed since viewed, counted from the keys alone. `lastMarkedHead` SHALL advance only when a file is marked, and SHALL only date that count ("since you last marked, at `abc1234`").

**Reading.** `get_review_progress(reference)` SHALL answer for that one pull request, never for the whole store, and only while its provider is enabled, with the states of the cached detail's files. While the provider is disabled it SHALL refuse without content.

**Pruning.** An entry untouched for 90 days whose pull request its own provider no longer lists SHALL be pruned, but only while that provider is enabled, once every enabled provider has completed a successful, non-stale refresh in this run, and never at load, when no snapshot exists yet. A disabled provider's empty list proves nothing, so its entries SHALL be kept:

$$\text{prune}(e) \iff \text{now} - \text{touchedAt}(e) \ge 90\ \text{days} \;\wedge\; p(e) \in \text{enabled} \;\wedge\; e \notin S_{p(e)} \;\wedge\; \forall p \in \text{enabled}:\ \text{refreshedThisRun}(p)$$

where $$p(e)$$ is the entry's provider.

**Notifying.** Each stored mark or unmark SHALL raise a `review-progress-changed` notice carrying the reference, on a broadcast the application service owns. The desktop application SHALL forward it to every window, and the web transport's event stream SHALL carry it to every tab. It SHALL never be emitted directly by a command, and SHALL NOT be a variant of the cache-event stream. Every view of one pull request in one service, whether in the center pane, in a pull-request window or in a tab served by the desktop's embedded server, therefore stays in step.

**Transports.** `get_review_progress` and `set_file_viewed` SHALL be served on both transports, since progress is local state of the person using SpecForge rather than a write to either host.

**Two writers.** A standalone `specforge-serve` running beside the desktop application is a second writer of the same file, as it is of the activity log. Each process SHALL re-read the file before each write. This narrows lost marks without closing the race, and neither process sees the other's marks until it re-reads.

#### Scenario: A mark survives a restart and never leaves the machine

- **WHEN** the user marks a file viewed, restarts SpecForge and reopens the pull request
- **THEN** once its detail has been read again, the file is still viewed
- **AND** no request carrying the mark was sent to either host

#### Scenario: A push that changes a file flags it

- **WHEN** a viewed file's patch changes in a later push
- **THEN** the file is changed since viewed
- **AND** the header's changed-since-viewed count includes it

#### Scenario: A push that leaves a file alone keeps its mark

- **WHEN** a later push changes other files and leaves a viewed file's patch byte-identical
- **THEN** that file is still viewed

#### Scenario: A file keyed by the head commit says why it changed

- **WHEN** a binary file of a BitBucket pull request was marked viewed and a later push changes the head commit
- **THEN** the file is changed since viewed
- **AND** the view says why

#### Scenario: A file without a patch is keyed by GitHub's blob

- **WHEN** a GitHub file with no `patch` and with a blob `sha` was marked viewed, and a later push leaves its `sha`, status, previous filename and base branch unchanged
- **THEN** the file is still viewed

#### Scenario: A mark against a different head is refused

- **WHEN** `set_file_viewed` names a head commit that differs from the cached detail's
- **THEN** the mark is refused and nothing is stored

#### Scenario: A path outside the detail is refused

- **WHEN** `set_file_viewed` names a path that is not among the cached detail's files
- **THEN** the mark is refused and nothing is stored

#### Scenario: Marking needs a cached detail

- **WHEN** `set_file_viewed` names a reference with no cached detail
- **THEN** the mark is refused and nothing is stored

#### Scenario: Unmarking creates nothing

- **WHEN** `set_file_viewed` unmarks a file of a pull request that has no entry
- **THEN** no entry is created

#### Scenario: The header counts progress

- **WHEN** a pull request has ten files, of which four are viewed and one is changed since viewed
- **THEN** the header shows "4 of 10 files viewed" and one file changed since viewed

#### Scenario: Progress is answered only while the provider is enabled

- **WHEN** GitHub is disabled and `get_review_progress` is invoked for a GitHub reference
- **THEN** it refuses without content

#### Scenario: Pruning waits for the lists

- **WHEN** SpecForge starts with an entry untouched for 120 days whose provider is enabled
- **THEN** the entry is not pruned at load
- **AND** it is pruned once every enabled provider has completed a successful, non-stale refresh in which its pull request is not listed

#### Scenario: Switching every provider off prunes nothing

- **WHEN** both providers are disabled and an entry has been untouched for 120 days
- **THEN** the entry is kept

#### Scenario: A disabled provider's entries are kept

- **WHEN** BitBucket is disabled, GitHub is enabled and has completed a successful, non-stale refresh, and a BitBucket entry has been untouched for 120 days
- **THEN** the entry is kept

#### Scenario: A mark in one window reaches every view

- **WHEN** the user marks a file viewed in a pull-request window while the center pane and a tab served by the desktop's embedded server show the same pull request
- **THEN** both receive `review-progress-changed` and show the file viewed

#### Scenario: The browser skin marks files

- **WHEN** the browser skin sends `set_file_viewed` for a file of a pull request whose detail is cached
- **THEN** the web transport dispatches it and the mark is stored

#### Scenario: A second process's earlier mark is kept

- **WHEN** a standalone `specforge-serve` has stored a mark, and the desktop application then marks another file of the same pull request
- **THEN** the stored entry holds both marks

### Requirement: Pull-Request Content Is Untrusted

Descriptions and comments are written by others, and SHALL render through the shared markdown renderer in a **pull-request mode**, in both presentations and on both hosts. That mode SHALL be the guard wherever pull-request content renders, including the main window, which keeps its broad capability and has no content-security policy governing what it loads. The mode SHALL apply only to pull-request content; workspace markdown keeps its own rendering (see the *Mermaid Diagram Rendering* and *Link Handling in Rendered Artifacts* requirements in the `spec-browser` capability). In pull-request mode:

- **Raw HTML** SHALL stay unrendered.
- **HTML comments** SHALL be removed after parsing, by dropping each HTML node whose whole value is a comment. The source text SHALL never be edited, so removing a comment cannot create a fence, a link or an image, and a comment inside code SHALL stay visible.
- **Mermaid fences** SHALL render as their source, as plain unhighlighted text, and SHALL NOT be drawn, since a diagram can fetch remote images while it is laid out.
- **Maths** SHALL render as it does in artifacts, under the same non-trusting posture (see the *Mathematical Notation Rendering* requirement in the `spec-browser` capability).
- **`svg` fences** SHALL keep their inert image rendering (see the *SVG Fence Rendering* requirement in the `spec-browser` capability).
- **Remote images** SHALL NOT be loaded. A standalone image SHALL render as a labelled link showing its alt text and its host. An image inside a link SHALL render as its alt text and host only, as part of the enclosing link, with no link or handler of its own, so one activation opens one destination.
- **Links** SHALL follow *Desktop Link Opener*: in the desktop application they open only through its command, and in the browser skin in a new opener-isolated tab.

**Hidden characters.** The pull request's title and branch names in the view's header SHALL render each character that renders as nothing as a visible, marked escape, with the escapes and the exemptions the `diff-view` capability gives text outside changed lines (see the *Hidden Characters Are Shown* requirement in the `diff-view` capability).

**DNS prefetching** SHALL be off in every SpecForge page, the main window, reader windows and pull-request windows on both hosts, so no host is resolved merely because a link to it is displayed.

#### Scenario: Raw HTML stays unrendered

- **WHEN** a description contains `<img src="https://tracker.example/p.gif">`
- **THEN** no image element is created from it
- **AND** no request is made to `tracker.example`

#### Scenario: Template comments disappear and code keeps them

- **WHEN** a description contains `<!-- Describe your change -->` on its own line, and the same text inside a fenced code block
- **THEN** the standalone comment is not shown
- **AND** the one inside the code block stays visible

#### Scenario: Removing a comment creates nothing

- **WHEN** a description contains `[docs]<!-- -->(guide.md)`
- **THEN** it renders no link

#### Scenario: A mermaid fence shows its source

- **WHEN** a comment contains a `mermaid` fence declaring a node with `@{ img: "https://tracker.example/p.png" }`
- **THEN** the fence renders as its source text and no diagram is drawn
- **AND** no request is made to `tracker.example`

#### Scenario: A standalone image becomes a labelled link

- **WHEN** a description contains `![screenshot](https://img.example/s.png)`
- **THEN** it renders as a link labelled "screenshot" that shows the host `img.example`
- **AND** no request is made to `img.example`

#### Scenario: An image inside a link becomes part of that link

- **WHEN** a description contains `[![build](https://badge.example/b.svg)](https://ci.example/run/7)`
- **THEN** it renders one link to `https://ci.example/run/7` that shows "build" and `badge.example`
- **AND** activating it opens only `https://ci.example/run/7`
- **AND** no request is made to `badge.example`

#### Scenario: Maths cannot carry a live link

- **WHEN** a comment contains display maths that uses `\href` with an external URL
- **THEN** the maths renders with no live link

#### Scenario: Hidden characters in the header are visible

- **WHEN** a pull request's head branch name contains a zero-width space
- **THEN** the header shows a visible, marked escape in its place

#### Scenario: The main window is guarded by the mode alone

- **WHEN** the center pane renders a description containing a remote image, a `mermaid` fence and raw HTML
- **THEN** none of them causes a request, although no content-security policy governs what the main window loads

#### Scenario: No page prefetches DNS

- **WHEN** the main window, a reader window or a pull-request window loads, on either host
- **THEN** its page has DNS prefetching turned off

### Requirement: Desktop Link Opener

In the desktop application, a link in pull-request content, and a check's link, SHALL open only through `open_pull_request_link(reference, href)`. The command SHALL hand the platform opener only an href that parses as an absolute `http` or `https` URL with a host, and only while the reference has a cached detail. It SHALL refuse every other scheme, including `file`, `javascript`, `data` and custom application schemes, and SHALL never fetch the href. It SHALL check the href's form and the cached detail only, not whether the pull request's content contains the href.

`open_pull_request_link` SHALL be desktop-only: the web transport SHALL NOT expose it, and SHALL report it as an unknown command. It SHALL be the only open operation that pull-request content reaches, as the artifact opener is for workspace markdown.

In the browser skin, an absolute `http` or `https` link in pull-request content SHALL open in a new opener-isolated tab and SHALL NOT navigate the serving page (see the *Link Handling in the Browser Skin* requirement in the `web-ui` capability). A relative link in pull-request content SHALL show its target without navigating, on either host, and SHALL open nothing.

#### Scenario: An external link opens in the system browser

- **WHEN** the user activates an `https` link in a description in the desktop application, while the pull request's detail is cached
- **THEN** the link opens in the system browser through `open_pull_request_link`
- **AND** the window does not navigate

#### Scenario: Other schemes are refused

- **WHEN** `open_pull_request_link` is invoked with a `javascript:`, `file:`, `data:`, `mailto:` or custom-scheme href
- **THEN** it returns an error and nothing is opened

#### Scenario: Without a cached detail nothing opens

- **WHEN** `open_pull_request_link` is invoked for a reference with no cached detail
- **THEN** it returns an error and nothing is opened

#### Scenario: The web transport cannot open links on the host

- **WHEN** `open_pull_request_link` is sent to the web transport's dispatch surface
- **THEN** it is reported as an unknown command and nothing is opened on the serving host

#### Scenario: A relative link does not navigate

- **WHEN** the user activates a relative link such as `docs/setup.md` in a comment
- **THEN** its target is shown without navigating
- **AND** nothing is opened

#### Scenario: The browser skin opens links in an isolated tab

- **WHEN** the user activates an `https` link in pull-request content in the browser skin
- **THEN** it opens in a new tab with `noopener noreferrer` semantics
- **AND** the SpecForge page does not navigate

### Requirement: The Terminal Frontend Has No Pull-Request View

The terminal frontend SHALL NOT render a pull-request view, list or window, and SHALL make no detail read. The pull-request address kind SHALL NOT reach it, since it has no address bar. Its Settings screen SHALL gain no row for this capability: review progress is not a setting, and the pull-request window's size is a desktop-only setting.

#### Scenario: The terminal shows no pull request

- **WHEN** the terminal frontend runs while a pull-request provider is enabled
- **THEN** it renders no pull-request view or list
- **AND** it sends no detail request

#### Scenario: The terminal's Settings screen gains no row

- **WHEN** the terminal's Settings screen is shown
- **THEN** it offers no row for review progress or for the pull-request window's size
