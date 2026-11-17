## MODIFIED Requirements

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

With **Automatically load large diffs** on, eligible near-viewport withheld files SHALL load through the same reader, following the *Automatic Loads Follow Visible Content* and *Automatic Preview Loads Preserve Review State* requirements in the `pr-content-loading` capability. With it off, loading SHALL remain manual. No automatic load SHALL target a too-large or binary non-image file, raise a hard ceiling, expand a collapsed section or remove its manual retry.

**Image files.** The view SHALL select its image-reader timing from **Automatically show images** (see the *Pull-Request Loading Preferences* requirement in the `pr-content-loading` capability), using the *Image Comparison* and *Diff View Hosts* requirements in the `diff-view` capability.
- **Showing an image.** With the preference off, an image file that is not a Git LFS pointer SHALL show "Show image" in place of its state row. Activating it SHALL read the file's versions through `get_pull_request_file_image` (see *Pull-Request Image Reads*). With the preference on, an eligible expanded image section SHALL read through that same command as it nears the viewport, under the shared automatic-work queue.
- **`changed`.** It SHALL make the view read the pull request again. Several stale automatic outcomes for one old model SHALL cause at most one refresh for that model.
- **`failed`.** It SHALL leave the manual image control in place with its reason beside it, in the words of the table above: "Show image" for manual mode and "Try again" for automatic mode. The reason `redirected`, which only an image read gives, reads that the provider sent the file from somewhere SpecForge does not follow. A failed read SHALL NOT make the view read again or automatically retry itself.
- **Keeping a read.** The view SHALL keep a read's versions for the file model they belong to; layout, tab, collapse and preference changes and a detail reread that retains that model SHALL not reread them. A replacement model SHALL discard its obsolete reads.
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

**After a load.** When a load brings a file's hunks, the view SHALL read the pull request's review progress again. Concurrent automatic loads SHALL coalesce progress reconciliation without losing a final read after all newly discovered hunks are known.

#### Scenario: Pull requests read in the layout commit detail uses

- **WHEN** the reader has chosen side by side for commit detail on this surface, and opens a pull request in a view wide enough for two columns
- **THEN** the pull request's files render side by side
- **AND** the diff view names its two sides by the pull request's base and head branch names

#### Scenario: A thread names its file, side and line

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

- **WHEN** the PR image preference is off and a GitHub pull request modifies `icons/app.png` and the reader activates its "Show image"
- **THEN** the view reads its versions through `get_pull_request_file_image` and shows them as the `diff-view` capability's *Image Comparison* requirement says
- **AND** collapsing and expanding its section reads nothing again

#### Scenario: A failed image read can be tried again

- **WHEN** the PR image preference is off and the reader activates "Show image" while GitHub's REST deadline holds until 14:05
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

#### Scenario: Enabled image automation uses the existing pinned reader

- **WHEN** the image preference is on and an expanded changed-image section nears the visible Files viewport
- **THEN** the shared queue reads it with the rendered commits through `get_pull_request_file_image`
- **AND** a failure leaves an explicit retry without another automatic attempt

#### Scenario: Enabled diff automation reconciles new hunks

- **WHEN** the large-diff preference is on and eligible withheld files load automatically
- **THEN** their original counts and all editor identities remain intact
- **AND** progress is reconciled for the final discovered hunk set without a read storm

### Requirement: Pull-Request Window Permissions

The pull-request window SHALL hold only the permissions it uses, and SHALL add two backstops to the untrusted-content handling every window applies (see *Pull-Request Content Is Untrusted*): a narrow capability and a content-security policy.

**Capability.** In the desktop application, one capability SHALL match the `pull-request-*` windows. Its plugin permissions SHALL be exactly `core:event:allow-listen`, `core:event:allow-unlisten` and `core:window:allow-close`; it SHALL grant no dialog, autostart, notification, menu or tray plugin permission. The main, reader and image capabilities SHALL NOT match a pull-request window. Its application-command permissions SHALL include only the pull-request reads, local review-progress operations, checked link/window operations and review-action operations the window uses (see the *Application Commands Are Granted Per Window Kind* requirement in the `pull-request-actions` capability), plus the read-only `get_pr_content_loading` and checked `get_pull_request_content_image` commands. It SHALL NOT grant `set_pr_content_loading`, workspace/settings mutation or main-, reader- or image-only commands. The window SHALL save its size through its permitted application command and have its title set from the Rust side, so it needs no other window permission.

**Navigation guard.** The window SHALL install the same navigation guard as the main and reader windows: only the application's own origin, and in a development build its development server, SHALL load in it, so a link the webview activates itself never loads another page in the window.

**Content-security policy.** On both hosts, before the window renders anything, its page's `head` SHALL carry a content-security policy of `img-src 'self' data: blob:; font-src 'self' data:; media-src 'none'; object-src 'none'`, in place before any fetch it governs starts. The policy SHALL be installed only for a pull-request window, never for the main window or a reader window. Enabling automatic images SHALL NOT broaden this policy: accepted embedded previews SHALL render from checked service bytes and object URLs, not remote DOM image URLs.

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

#### Scenario: PR windows read preferences but cannot change them

- **WHEN** a detached PR window invokes the loading getter and then the loading setter
- **THEN** the getter is permitted and the setter is refused before its handler runs

#### Scenario: Checked previews need no wider CSP

- **WHEN** automatic images are enabled in a detached PR window
- **THEN** a supported embedded image can render from the checked read's object URL
- **AND** remote DOM image requests remain blocked

### Requirement: Pull-Request Content Is Untrusted

Descriptions and comments are written by others, and SHALL render through the shared markdown renderer in a **pull-request mode**, in both presentations and on both hosts. That mode SHALL be the guard wherever pull-request content renders, including the main window, which keeps its broad capability and has no content-security policy governing what it loads. The mode SHALL apply only to pull-request content; workspace markdown keeps its own rendering (see the *Mermaid Diagram Rendering* and *Link Handling in Rendered Artifacts* requirements in the `spec-browser` capability). In pull-request mode:

- **Raw HTML** SHALL stay unrendered.
- **HTML comments** SHALL be removed after parsing, by dropping each HTML node whose whole value is a comment. The source text SHALL never be edited, so removing a comment cannot create a fence, a link or an image, and a comment inside code SHALL stay visible.
- **Mermaid fences** SHALL render as their source, as plain unhighlighted text, and SHALL NOT be drawn, since a diagram can fetch remote images while it is laid out.
- **Maths** SHALL render as it does in artifacts, under the same non-trusting posture (see the *Mathematical Notation Rendering* requirement in the `spec-browser` capability).
- **`svg` fences** SHALL keep their inert image rendering (see the *SVG Fence Rendering* requirement in the `spec-browser` capability).
- **Embedded images** SHALL remain unloaded by default. A standalone unloaded image SHALL render as a labelled link showing its alt text and host. An unloaded image inside a link SHALL render as its alt text and host only, as part of the enclosing link, with no link or handler of its own, so one activation opens one destination. Only when **Automatically show images** is enabled SHALL supported published Markdown images become eligible for the checked credentialless read and in-memory preview defined by the *Embedded Image Reads Are Authorised by Published Content* and *Embedded Previews Preserve Untrusted-Content Boundaries* requirements in the `pr-content-loading` capability. Unsupported/blocked sources SHALL retain their safe fallback; author-controlled URLs SHALL never become DOM image-fetch URLs.
- **Links** SHALL follow *Desktop Link Opener*: in the desktop application they open only through its command, and in the browser skin in a new opener-isolated tab. A preview inside a link SHALL preserve that single destination.
- **Identifiers** SHALL NOT be taken from the content. No element rendered from it SHALL carry an `id` derived from its text, heading identifiers included, so content cannot collide with, or be styled as, the application's own elements.

**Hidden characters.** The pull request's title and branch names in the view's header SHALL render each character that renders as nothing as a visible, marked escape, with the escapes and the exemptions the `diff-view` capability gives text outside changed lines (see the *Hidden Characters Are Shown* requirement in the `diff-view` capability).

**DNS prefetching** SHALL be off in every SpecForge page, the main window, reader windows and pull-request windows on both hosts, so no host is resolved merely because a link to it is displayed.

**Changed image files** are not content of this mode, and no URL SHALL ever load them. An image file's versions SHALL be read only by an image read from the provider's API (see *Pull-Request Image Reads*). They SHALL be checked as the *Image Comparison* requirement in the `diff-view` capability says, and rendered from memory, which the pull-request window's content-security policy already allows (see *Pull-Request Window Permissions*). Embedded-image reads SHALL remain a separate credentialless operation; they SHALL neither reuse provider authentication nor change how changed-file versions are read.

#### Scenario: Raw HTML stays unrendered

- **WHEN** a description contains `<img src="https://tracker.example/p.gif">`
- **THEN** no image element is created from it
- **AND** no request is made to `tracker.example`, even with automatic images on

#### Scenario: A heading takes no identifier from the content

- **WHEN** a description contains the heading `# Root`
- **THEN** the heading renders with no `id`
- **AND** no element of the rendered content carries `id="root"`

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

- **WHEN** automatic images are off and a description contains `![screenshot](https://img.example/s.png)`
- **THEN** it renders as a link labelled "screenshot" that shows the host `img.example`
- **AND** no request is made to `img.example`

#### Scenario: An image inside a link becomes part of that link

- **WHEN** automatic images are off and a description contains `[![build](https://badge.example/b.svg)](https://ci.example/run/7)`
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

- **WHEN** automatic images are off and the center pane renders a description containing a remote image, a `mermaid` fence and raw HTML
- **THEN** none of them causes a request, although no content-security policy governs what the main window loads

#### Scenario: No page prefetches DNS

- **WHEN** the main window, a reader window or a pull-request window loads, on either host
- **THEN** its page has DNS prefetching turned off

#### Scenario: A changed image renders without a request of its own

- **WHEN** automatic images are off and a pull-request window shows the versions of a changed `icons/app.png`, and the description embeds `![app](https://img.example/app.png)`
- **THEN** the versions render from bytes the image read returned, and the only requests made for them went to the provider's API
- **AND** the description's image still renders as a labelled link, and no request is made to `img.example`

#### Scenario: Opted-in Markdown images use only the checked read

- **WHEN** automatic images are on and an eligible supported image appears in a current published description or comment
- **THEN** it can preview only after the service authorises its source and validates its bytes
- **AND** raw HTML, Mermaid, identifiers, external links and workspace Markdown retain their existing policies
