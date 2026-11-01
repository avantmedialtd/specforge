## MODIFIED Requirements

### Requirement: Pull-Request View

The **pull-request view** SHALL render one pull request from the detail the application service reads for it (see *Detail Reads Are Scoped to the Snapshot*), and SHALL render its read content the same way in two presentations, in the desktop application and in the browser skin alike:

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

**Review actions.** In the native desktop application, both presentations SHALL offer the same draft and explicitly confirmed review controls when current capabilities allow them (see the *Desktop-Only Pull-Request Reviews* requirement in the `pull-request-actions` capability). In the browser skin the view SHALL offer no provider-write control. No presentation SHALL offer a merge, decline, close, thread-resolution or thread-reopening control. Marking a file or hunk viewed SHALL remain local to this machine (see *Review Progress*).

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

#### Scenario: Review controls follow the host boundary

- **WHEN** the same actionable pull request is shown in the native desktop and the browser skin
- **THEN** the native view offers the review composer and the browser view does not
- **AND** neither view offers merge, decline, close, thread-resolution or thread-reopening controls

### Requirement: Changed Files in the Pull-Request View

The pull-request view SHALL render the pull request's changed files through the `diff-view` capability, as one of its hosts (see the *Diff View Hosts* requirement in the `diff-view` capability), from the model the detail reads build (see *GitHub Detail Reads* and *BitBucket Detail Reads*), with the same line and byte budgets applied as for commit detail (see the *Line and Byte Budgets With On-Request Loading* requirement in the `diff-view` capability). The files SHALL read as commit detail's do, unified or side by side, under the per-surface layout choice that commit detail and pull requests share and that is not an application setting (see the *The Layout Choice Is Per Surface* requirement in the `diff-view` capability). The view SHALL pass the pull request's base and head branch names as the names of the diff's old and new sides.

**Review threads.** A current, non-outdated review thread anchored to exactly one line of a listed file SHALL render in that file's matching old/new line slot, immediately with the line in either layout; its replies SHALL render with it. On GitHub, `diffSide` SHALL map `LEFT` to old and `RIGHT` to new. On BitBucket, `inline.from` SHALL map to old and `inline.to` to new. A thread SHALL instead render in its listed file's preamble, across the section's full width and naming its original anchor, when it is outdated, spans a range, has no usable side/line, or its matching line is not rendered. A thread whose file is not among the listed files, such as one on a file past GitHub's thousandth or a file the pull request no longer changes, SHALL render below the diff view, after its files. A thread comment that GitHub reports as minimised SHALL render collapsed behind GitHub's stated reason, wherever its thread renders.

**Draft comments and replies.** In the native desktop, a placed single-line draft SHALL render in the same line slot as submitted single-line content, and the current line's review affordance — a comment action in the line's gutter, shown while the line is hovered or focused — SHALL create or move a draft only through the action service. A line SHALL gain a line slot only for content: a thread, a draft, or the one new-comment editor the reviewer opened on it (see the *Draft Items and Decisions Are Explicit and Valid* requirement in the `pull-request-actions` capability). A draft reply SHALL render with its target thread. An unplaced inline draft SHALL render in the former file's preamble when that file is listed, otherwise below the files, retaining its former path, side and line. An unplaced former reply SHALL retain and name its former target beside the files. Either kind SHALL offer explicit conversion onto a current line or current replyable thread and SHALL remain ineligible for submission until converted. No draft control SHALL render in the browser skin.

**Withheld files.** A file the budgets withheld, and a file GitHub sent without its patch, SHALL show its counts and a control that loads it. Loading it SHALL go through `get_pull_request_file` (see *Detail Reads Are Scoped to the Snapshot*). A file the budgets withheld SHALL load from the cached detail without a request to the provider. A file GitHub sent without its patch SHALL be read from GitHub the first time it is loaded (see *GitHub Detail Reads*) and from the cached detail after that. An answer of `changed` SHALL make the view read the pull request again. An answer of `failed` SHALL leave the file withheld, show its reason beside the control in the view's words, and SHALL NOT make the view read again:

| Reason | The view says |
|---|---|
| `deferred` | the provider's rate limit holds, and the local time a load becomes possible |
| `unauthenticated` | the provider refused the credential |
| `unavailable` | the provider no longer has the file at these commits |
| `refused` | the provider is switched off |
| `transient` | the provider did not answer, and to try again |

**Image files.** The view SHALL be a host whose image reader reads on request (see the *Image Comparison* and *Diff View Hosts* requirements in the `diff-view` capability).
- **Showing an image.** An image file that is not a Git LFS pointer SHALL show "Show image" in place of its state row. Activating it SHALL read the file's versions through `get_pull_request_file_image` (see *Pull-Request Image Reads*).
- **`changed`.** It SHALL make the view read the pull request again.
- **`failed`.** It SHALL leave "Show image" in place with its reason beside it, in the words of the table above. The reason `redirected`, which only an image read gives, reads that the provider sent the file from somewhere SpecForge does not follow. A failed read SHALL NOT make the view read again.
- **Keeping a read.** The view SHALL keep a read's versions until it renders another detail.
- **Review progress.** An image file SHALL keep its viewed mark as a whole file: it has no hunk marks, and a read of its versions brings no hunks, so the view does not read the progress again after one.

**Too-large files.** A file too large to preview SHALL carry, in its preamble slot, a link to that file's diff on its provider's page. On GitHub the link SHALL be the pull request's page followed by `/files#diff-` and the lowercase hexadecimal SHA-256 digest of the file's path. On BitBucket it SHALL be the pull request's page followed by `/diff#chg-` and the path. The path SHALL be the file's new path, or its old path when it was deleted. The link SHALL be built from the provider page the detail carries, and SHALL open as the view's other provider links do: an opener-isolated tab in the browser skin, and the desktop link opener in the desktop application (see *Desktop Link Opener*). An image file SHALL carry the same link when any of these holds:
- it reads "Stored in Git LFS";
- a read of it failed as `redirected` or `unavailable`;
- one of its read versions is refused, or cannot be drawn.

Any other binary file SHALL carry no such link.

**Viewed marks.** Each file's header extra SHALL carry its viewed mark (see *Review Progress*): a checkbox, checked when the file is viewed, mixed when it is partly viewed or is changed since viewed with at least one hunk still viewed, and unchecked otherwise. Activating an unchecked or mixed box SHALL mark the file viewed, and activating a checked one SHALL unmark it. Beside the box the header SHALL say:

| State | Hunk states given | The header says |
|---|---|---|
| partly viewed | yes | "4 of 9 hunks viewed" |
| partly viewed | no | "some hunks viewed" |
| changed since viewed | yes | "changed since viewed · 3 hunks to review" |
| changed since viewed | no | "changed since viewed", and why when the file is keyed by the head commit |
| skipped | either | "skipped · matches `<pattern>`" and a **Review** control |

**Hunk marks.** For each file whose hunk states the progress gives, each hunk SHALL carry a checkbox in its heading extra (see the *Diff View Hosts* requirement in the `diff-view` capability). The checkbox SHALL be checked when the hunk is viewed, and named by its file and its first line identity ("Viewed: src/big.rs, hunk from new line 143"). The view SHALL fold every viewed hunk (see the *Folded Hunks* requirement in the `diff-view` capability). An unviewed hunk of more than 40 lines SHALL end with a row reading "Mark hunk viewed", which marks it:

$$\text{end row}(h) \iff \lnot\,\text{viewed}(h) \wedge |\text{lines}(h)| > 40$$

The view SHALL apply hunk states only to the detail whose head and base commits the progress names. For any other detail it SHALL treat every file's hunk states as not given until progress is read for that detail. A hunk being marked SHALL show its new state, folded or not, until the progress that follows the mark lands. A refused hunk mark SHALL say why in its file's header, beside the file's mark, as a refused file mark does, until the file or one of its hunks is marked again or a new detail arrives. The heading row's gutter holds the checkbox alone.

**Skipped files.** A skipped file's box SHALL be unchecked, and activating it SHALL mark the file viewed as for any unviewed file. Its **Review** control SHALL include the file (`set_file_included` with `included` true). A file that a skip pattern matches and whose path is included SHALL say "matches `<pattern>`" beside whatever its state gives, and offer a **Skip** control, which excludes it. Each time a progress read shows a file skipped and the view's previous progress read of this pull request, for any detail, did not, the view SHALL ask the diff view to collapse that file's section once (see the *File Sections* requirement in the `diff-view` capability). Opening the pull request, a change to the skip patterns and the reader's Skip, in this view or another, are the ways a file becomes skipped. A file still skipped after a push SHALL NOT be collapsed again, so a section the reader expanded stays expanded. Expanding a skipped section SHALL store nothing, and its hunks SHALL carry their checkboxes as any unviewed file's do. Including a file SHALL NOT expand its section.

**Completing a file.** When a mark the reader made in this view is stored, through a file's box or a hunk's checkbox or end row, and the progress read that follows shows that file viewed, the view SHALL ask the diff view to collapse that file's section once (see the *File Sections* requirement in the `diff-view` capability). A mark arriving from another window or tab, a progress read for any other reason, and opening the pull request SHALL collapse nothing.

**After a load.** When a load brings a file's hunks, the view SHALL read the pull request's review progress again.

#### Scenario: Pull requests read in the layout commit detail uses

- **WHEN** the reader has chosen side by side for commit detail on this surface, and opens a pull request in a view wide enough for two columns
- **THEN** the pull request's files render side by side
- **AND** the diff view names its two sides by the pull request's base and head branch names

#### Scenario: A current single-line thread follows its line

- **WHEN** a current GitHub review thread on `src/api.ts` has `diffSide` `LEFT` and line 12, which is rendered
- **THEN** it renders with old line 12 in unified and in the old half of its row side by side
- **AND** its replies render with it

#### Scenario: A BitBucket inline comment takes its side from its anchor

- **WHEN** a current BitBucket inline comment on `README.md` carries `inline.to` 7 and no `inline.from`
- **THEN** its thread renders with new line 7 in either layout

#### Scenario: A ranged or outdated thread stays in the preamble

- **WHEN** a thread spans more than one line or the provider reports it outdated
- **THEN** it renders in its listed file's preamble across the section's full width
- **AND** it names its original path, side and line or range

#### Scenario: A minimised thread comment stays collapsed

- **WHEN** a comment in a current single-line review thread on `src/api.ts` is minimised on GitHub with the reason "spam"
- **THEN** it renders collapsed at its line, showing that reason in place of its body

#### Scenario: A thread on an unlisted file follows the files

- **WHEN** a GitHub pull request changes 1,200 files and a review thread sits on a file past the thousandth
- **THEN** that thread renders below the diff view, after the listed files, naming its file, its side and its line

#### Scenario: A placed draft follows its line

- **WHEN** the native draft contains a placed comment on new line 27 of `src/api.ts`
- **THEN** that draft renders with new line 27 in unified and side-by-side layouts
- **AND** the browser skin renders no draft or draft control

#### Scenario: An unplaced draft stays in the preamble

- **WHEN** stale recovery leaves a comment formerly on old line 9 of a still-listed file unplaced
- **THEN** its body and former anchor render in that file's preamble
- **AND** it moves to a line slot only after the reviewer explicitly chooses a current line

#### Scenario: An orphaned draft reply keeps prose but no target

- **WHEN** stale recovery finds that a draft reply's former thread no longer exists
- **THEN** its body and former target remain visible beside the files and it cannot be submitted
- **AND** only an explicit current line or replyable thread converts it into a placed draft item

#### Scenario: A withheld file loads from the cache

- **WHEN** the budgets withheld a file and the reader asks to load it
- **THEN** its hunks render from the cached detail
- **AND** no request is sent to the provider

#### Scenario: A file GitHub sent without its patch loads from GitHub

- **WHEN** a GitHub pull request's `apps/uk/+Page.tsx` arrived without its patch, with 158 added and 1,477 removed lines, and the reader activates "Load diff"
- **THEN** the file is read from GitHub and its hunks render, with its counts unchanged
- **AND** loading it again, in either presentation, sends no request

#### Scenario: A load that cannot complete does not read the pull request again

- **WHEN** GitHub's REST deadline holds until 14:05 and the reader loads a file GitHub sent without its patch
- **THEN** no request is sent, the file stays withheld, and the view says GitHub's rate limit holds until 14:05
- **AND** the view does not read the pull request again

#### Scenario: A too-large file links to its diff on the host

- **WHEN** a GitHub pull request at `https://github.com/acme/api/pull/42` has a file `src/huge.json` that is too large to preview
- **THEN** its preamble carries a link to `https://github.com/acme/api/pull/42/files#diff-dd89c4cf549b7418f9dde6cfa5beea228450b2df2f433c9d98d2c949c9c3fc2e`
- **AND** in the desktop application the link opens through the desktop link opener

#### Scenario: Each file carries its viewed mark

- **WHEN** the reader has marked one of a pull request's files viewed
- **THEN** that file's header shows it viewed
- **AND** every other file's header shows it unviewed

#### Scenario: A viewed hunk folds

- **WHEN** the reader checks the box of the second of a file's three hunks
- **THEN** that hunk folds to its heading row, with its box checked
- **AND** the file's box is mixed and its header says "1 of 3 hunks viewed"

#### Scenario: A long hunk is marked where it ends

- **WHEN** an unviewed hunk has 120 lines
- **THEN** a row reading "Mark hunk viewed" follows its last line
- **WHEN** the reader activates that row
- **THEN** the hunk is viewed and folds

#### Scenario: A hunk of 40 lines has no end row

- **WHEN** an unviewed hunk has 40 lines
- **THEN** no row follows its last line

#### Scenario: Marking the last hunk collapses the file

- **WHEN** two of a file's three hunks are viewed and the reader marks the third
- **THEN** once the progress that follows shows the file viewed, its section collapses
- **AND** its box is checked

#### Scenario: A mixed box marks the whole file

- **WHEN** a file is partly viewed and the reader activates its box
- **THEN** every hunk of the file is viewed and folded
- **AND** its section collapses

#### Scenario: A mark from elsewhere collapses nothing

- **WHEN** the center pane and a pull-request window show the same pull request, and the reader marks the last unviewed hunk of a file in the window
- **THEN** the file's section collapses in the window
- **AND** in the center pane the hunk folds and the file's section stays expanded

#### Scenario: A changed file says how many hunks to review

- **WHEN** a file marked viewed through its box has two of its five hunks changed by a later push
- **THEN** its header says "changed since viewed · 2 hunks to review" and its box is mixed
- **AND** only those two hunks are unfolded

#### Scenario: Hunk states of another detail are not applied

- **WHEN** the progress the view holds names the head commit `abc1234` and the view shows a detail whose head commit is `def5678`
- **THEN** no hunk carries a checkbox and none is folded until progress is read for `def5678`

#### Scenario: A loaded file gets its hunk marks

- **WHEN** the reader loads a file GitHub sent without its patch
- **THEN** the view reads the pull request's review progress again
- **AND** the file's hunks carry their checkboxes

#### Scenario: An image file shows its versions on request

- **WHEN** a GitHub pull request modifies `icons/app.png` and the reader activates its "Show image"
- **THEN** the view reads its versions through `get_pull_request_file_image` and shows them as the `diff-view` capability's *Image Comparison* requirement says
- **AND** collapsing and expanding its section reads nothing again

#### Scenario: A failed image read can be tried again

- **WHEN** the reader activates "Show image" while GitHub's REST deadline holds until 14:05
- **THEN** no request is sent, "Show image" stays, and the view says GitHub's rate limit holds until 14:05
- **AND** the view does not read the pull request again

#### Scenario: A redirected image links to its host

- **WHEN** a BitBucket image read answers `failed` with the reason `redirected`
- **THEN** the file says BitBucket sent it from somewhere SpecForge does not follow
- **AND** its preamble carries a link to the file's diff on BitBucket

#### Scenario: A Git LFS image links to its host without a read

- **WHEN** an image file's hunks are a Git LFS pointer
- **THEN** it reads "Stored in Git LFS", offers no "Show image", and carries a link to its diff on the host
- **AND** no image read is made

#### Scenario: An image file is viewed as a whole

- **WHEN** the reader shows an image file's versions and marks it viewed
- **THEN** its box is checked, it carries no hunk checkbox, and its section collapses as any completed file's does

#### Scenario: A skipped file opens collapsed

- **WHEN** the reader opens a pull request that changes `crates/core/tests/parse.rs` while the skip patterns hold `**/tests/**`
- **THEN** that file's section is collapsed, its box unchecked, and its header says "skipped · matches `**/tests/**`" beside a Review control

#### Scenario: Reviewing a skipped file

- **WHEN** the reader activates Review on a skipped file that `**/tests/**` matches
- **THEN** the file is unviewed and included, and its header says "matches `**/tests/**`" beside a Skip control
- **AND** the header's count of files grows by one and its count of skipped files falls by one

#### Scenario: Skipping again collapses

- **WHEN** the reader has expanded an included file a pattern matches and activates its Skip
- **THEN** the file is skipped and its section collapses

#### Scenario: A skipped section the reader expanded stays expanded

- **WHEN** the reader expands a skipped file's section and a later push changes that file
- **THEN** once the new detail and its progress are read, the file is still skipped and its section stays expanded

#### Scenario: Peeking stores nothing

- **WHEN** the reader expands a skipped file's section and collapses it again
- **THEN** nothing is stored and the file is still skipped

### Requirement: Pull-Request Window Permissions

The pull-request window SHALL hold only the permissions it uses, and SHALL add two backstops to the untrusted-content handling every window applies (see *Pull-Request Content Is Untrusted*): a narrow capability and a content-security policy.

**Capability.** In the desktop application, one capability SHALL match the `pull-request-*` windows. Its plugin permissions SHALL be exactly `core:event:allow-listen`, `core:event:allow-unlisten` and `core:window:allow-close`; it SHALL grant no dialog, autostart, notification, menu or tray plugin permission. The main, reader and image capabilities SHALL NOT match a pull-request window. Its application-command permissions SHALL include only the pull-request reads, local review-progress operations, checked link/window operations and review-action operations the window uses (see the *Application Commands Are Granted Per Window Kind* requirement in the `pull-request-actions` capability). It SHALL include no workspace/settings mutation and no main-, reader- or image-only command. The window SHALL save its size through its permitted application command and have its title set from the Rust side, so it needs no other window permission.

**Navigation guard.** The window SHALL install the same navigation guard as the main and reader windows: only the application's own origin, and in a development build its development server, SHALL load in it, so a link the webview activates itself never loads another page in the window.

**Content-security policy.** On both hosts, before the window renders anything, its page's `head` SHALL carry a content-security policy of `img-src 'self' data: blob:; font-src 'self' data:; media-src 'none'; object-src 'none'`, in place before any fetch it governs starts. The policy SHALL be installed only for a pull-request window, never for the main window or a reader window.

#### Scenario: The capability grants only what the window uses

- **WHEN** the desktop application's capabilities are inspected
- **THEN** the capability matching `pull-request-*` windows grants exactly the three named plugin permissions plus its explicit pull-request application-command allowlist
- **AND** no capability granting the dialog, autostart or notification plugin matches a `pull-request-` window

#### Scenario: The narrow capability is enough

- **WHEN** a pull-request window is opened on a cold start of the desktop application
- **THEN** it loads, reads and reviews its pull request, receives the native events it listens for, and closes on Escape

#### Scenario: An unrelated application command is refused

- **WHEN** a pull-request window invokes a workspace-registration, settings-credential or reader-only command directly
- **THEN** Tauri refuses it before the handler runs

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
