## ADDED Requirements

### Requirement: Pull-Request Image Reads

`get_pull_request_file_image(reference, path, head, base)` SHALL read one image file's two versions (see the *Image Comparison* requirement in the `diff-view` capability). It SHALL name the head and base commits the view rendered, and SHALL answer with one of three outcomes:

- `images`, carrying the old side and the new side, each an image, absent or refused, as *Image Comparison* defines them;
- `changed`, when no detail is cached, either commit differs from the cached detail's, or no file of the cached detail has that path. The view then reads the pull request again;
- `failed`, carrying a reason, when the versions could not be read. The view does not read again. The reason is one of:
  - `deferred`, with the time a read becomes possible;
  - `unauthenticated`;
  - `unavailable`;
  - `refused`;
  - `transient`;
  - `redirected`, when the provider answered a request with a redirect.

**What the caller supplies.** The caller supplies only the reference, the path and the two commits. The pull request's owner and repository SHALL come from the matched row, as a detail read takes them (see *Detail Reads Are Scoped to the Snapshot*). The file's old and new paths, its status and the commits SHALL come from the cached detail.

**Which versions.** An image read SHALL read the file's old path at the merge base of the cached detail's base and head commits, unless the file was added. It SHALL read the file's new path at the head commit, unless the file was deleted.
- **The merge base.** When a file read or an image read of the same cached detail has learned it, the read SHALL use that merge base and send no request for it. Otherwise it SHALL learn the merge base and keep it with the detail on the terms a file read keeps it, and a new read of the pull request SHALL drop it.
- **The ceiling.** Each version SHALL be read to at most 8 MiB and one byte, so a version past 8 MiB is refused as too large.
- **The checks.** Each version SHALL be checked as *Image Comparison* says, by its bytes and never by its name or by any type the provider reports.

**Governance.** An image read SHALL be governed as a file read is:
- it is admitted, deferred and counted as *Shared Backoff and Detail Budget* says, each of its requests counting against the provider's hourly budget;
- it sends only while the provider is enabled under the credential generation it started with;
- it keeps the merge base only under that same check.

A deferred image read SHALL send nothing. While a provider is disabled, `get_pull_request_file_image` SHALL answer `failed` with the reason `refused` for its pull requests.

**No bytes kept.** The service SHALL keep no version's bytes. A later image read of the same file sends its version requests again. Only the view keeps what a read returned (see *Image Comparison*).

**GitHub.** An image read SHALL send only requests 3 and 4 of *GitHub Detail Reads*: the compare for the merge base, and the raw contents of each version. They SHALL be sent and classified exactly as a file read's.

**BitBucket.** An image read SHALL send only these GETs, each to `api.bitbucket.org`, none following a redirect, and each carrying the credential only in its `Authorization` header. Each URL SHALL be built from the matched row's workspace and repository and the cached detail's commits and paths, never from a link in a payload.

1. `/2.0/repositories/{workspace}/{repo}/merge-base/{head}..{base}`, naming the cached detail's head and base commits, sent at most once per cached detail. Its reply SHALL be read as JSON whose `hash` is a 40-character hexadecimal commit.
2. `/2.0/repositories/{workspace}/{repo}/src/{commit}/{path}`, with each path segment percent-encoded. One request reads the old path at the merge base, unless the file was added. Another reads the new path at the head commit, unless it was deleted.

$$\text{requests per image read} \le 1 + 2$$

BitBucket's replies SHALL be classified as follows:
- **Unauthenticated:** a 401, or a 403.
- **Rate-limited:** a 429, which sets the shared deadline as *BitBucket Detail Reads* says.
- **Unavailable:** a 404.
- **Redirected:** a redirect. It SHALL NOT be followed.
- **Transient:** a merge-base reply that is not JSON with such a `hash`, a transport error, or any other status.

**Transports.** `get_pull_request_file_image` SHALL be served on both the desktop and the web transport.

#### Scenario: A GitHub image read sends at most three requests

- **WHEN** the reader asks to see a modified image file of a GitHub pull request, and no merge base is known for its cached detail
- **THEN** the read sends the compare GET, the contents GET of its old path at the merge base, and the contents GET of its new path at the head commit
- **AND** it answers `images` with both versions

#### Scenario: A merge base already learned is reused

- **WHEN** a file read of the same cached detail has already learned its merge base, and the reader asks to see an image file
- **THEN** the image read sends only its two contents GETs

#### Scenario: An added image reads only its new version

- **WHEN** the reader asks to see an image file the pull request adds
- **THEN** no request is sent for an old version
- **AND** the answer carries an absent old side

#### Scenario: A BitBucket image read reads the merge base and both versions

- **WHEN** the reader asks to see a modified image file of a BitBucket pull request for the first time since it was read
- **THEN** the read sends the `merge-base` GET naming its head and base commits, then the `src` GET of its old path at the merge base and of its new path at the head commit, all to `api.bitbucket.org`

#### Scenario: A BitBucket redirect is not followed

- **WHEN** BitBucket answers an image read's `src` GET with a redirect
- **THEN** the redirect is not followed and the credential is sent nowhere else
- **AND** the read answers `failed` with the reason `redirected`

#### Scenario: An image read counts against the budget

- **WHEN** a provider's hourly detail budget is spent and the reader asks to see an image file
- **THEN** no request is sent
- **AND** the read answers `failed` with the reason `deferred` and the time the budget next has room

#### Scenario: An image read after a push answers changed

- **WHEN** the view asks for an image file naming a head commit that differs from the cached detail's, because a push has been read since
- **THEN** the service answers `changed` and sends no request
- **AND** the view reads the pull request again

#### Scenario: The service keeps no bytes

- **WHEN** an image file was read once, and a second view of the same pull request asks for it
- **THEN** the second read sends its version requests again, and no compare or `merge-base` request

#### Scenario: The provider's word is not trusted

- **WHEN** an image read's new version of `icons/app.png` holds HTML text
- **THEN** that side is refused as not an image, and no image element is made for it

#### Scenario: A disabled provider reads no image

- **WHEN** the user has disabled GitHub and a view asks for a GitHub image file
- **THEN** no request is sent, and the service answers `failed` with the reason `refused`

#### Scenario: The browser skin reads images

- **WHEN** the browser skin sends `get_pull_request_file_image` for an image file of a listed pull request
- **THEN** the web transport dispatches it and returns the outcome the desktop application would receive

## MODIFIED Requirements

### Requirement: Changed Files in the Pull-Request View

The pull-request view SHALL render the pull request's changed files through the `diff-view` capability, as one of its hosts (see the *Diff View Hosts* requirement in the `diff-view` capability), from the model the detail reads build (see *GitHub Detail Reads* and *BitBucket Detail Reads*), with the same line and byte budgets applied as for commit detail (see the *Line and Byte Budgets With On-Request Loading* requirement in the `diff-view` capability). The files SHALL read as commit detail's do, unified or side by side, under the per-surface layout choice that commit detail and pull requests share and that is not an application setting (see the *The Layout Choice Is Per Surface* requirement in the `diff-view` capability). The view SHALL pass the pull request's base and head branch names as the names of the diff's old and new sides.

**Review threads.** Each review thread on a listed file, which on BitBucket is an inline comment with its replies, SHALL render in its file's preamble slot: above that file's diff and across the section's full width, in either layout, rather than between the diff's lines. A thread whose file is not among the listed files, such as one on a file past GitHub's thousandth or an outdated thread on a file the pull request no longer changes, SHALL render below the diff view, after its files. Each thread SHALL name its file, its side and its line. The side SHALL be old or new: GitHub's `diffSide` gives it, `LEFT` for old and `RIGHT` for new, and BitBucket's anchor gives it, `inline.from` for old and `inline.to` for new. A thread comment that GitHub reports as minimised SHALL render collapsed behind GitHub's stated reason, as a conversation entry does.

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

**Hunk marks.** For each file whose hunk states the progress gives, each hunk SHALL carry a checkbox in its heading extra (see the *Diff View Hosts* requirement in the `diff-view` capability). The checkbox SHALL be checked when the hunk is viewed, and named by its file and its first line identity ("Viewed: src/big.rs, hunk from new line 143"). The view SHALL fold every viewed hunk (see the *Folded Hunks* requirement in the `diff-view` capability). An unviewed hunk of more than 40 lines SHALL end with a row reading "Mark hunk viewed", which marks it:

$$\text{end row}(h) \iff \lnot\,\text{viewed}(h) \wedge |\text{lines}(h)| > 40$$

The view SHALL apply hunk states only to the detail whose head and base commits the progress names. For any other detail it SHALL treat every file's hunk states as not given until progress is read for that detail. A hunk being marked SHALL show its new state, folded or not, until the progress that follows the mark lands. A refused hunk mark SHALL say why in its file's header, beside the file's mark, as a refused file mark does, until the file or one of its hunks is marked again or a new detail arrives. The heading row's gutter holds the checkbox alone.

**Completing a file.** When a mark the reader made in this view is stored, through a file's box or a hunk's checkbox or end row, and the progress read that follows shows that file viewed, the view SHALL ask the diff view to collapse that file's section once (see the *File Sections* requirement in the `diff-view` capability). A mark arriving from another window or tab, a progress read for any other reason, and opening the pull request SHALL collapse nothing.

**After a load.** When a load brings a file's hunks, the view SHALL read the pull request's review progress again.

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

### Requirement: GitHub Detail Reads

A GitHub detail read SHALL send only these requests, each to `api.github.com`:

1. one `POST` to `https://api.github.com/graphql` carrying a detail query fixed at compile time: a read-only `query` operation, never a mutation or subscription, byte-identical on every read. Its only variables SHALL be the owner, repository name and number taken from the matched row, carried in GraphQL's `variables` and never interpolated into the query's text. Among the pull request's other fields, the query SHALL read its submitted reviews in the `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED` and `DISMISSED` states only, and each review-thread comment's state, so that nothing of the account's own pending review, which nobody else can see, is shown; each review thread's and comment's id; each thread's path, lines, `diffSide` and `startDiffSide`; whether each comment and review is minimised, and why; and the total number of changed files.
2. `GET https://api.github.com/repos/{owner}/{name}/pulls/{number}/files?per_page=50&page=n`, for at most twenty pages, which is a thousand files. Files beyond the thousandth SHALL be counted rather than listed, and the view SHALL say how many there are, with a pointer to the provider's page.

$$\text{requests per read} = 1 + p_{\text{files}}, \qquad p_{\text{files}} \le 20$$

A **file read** loads one file GitHub sent without its patch (see *Detail Reads Are Scoped to the Snapshot*). It SHALL send only these requests, each a `GET` to `api.github.com`, its commits and path taken from the cached detail:

3. `GET https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}?per_page=1`, naming the cached detail's base and head commits, for `merge_base_commit.sha`: the commit GitHub diffs the pull request against. It SHALL be sent at most once per cached detail. The merge base SHALL be kept with the detail, and a new read of the pull request SHALL drop it.
4. `GET https://api.github.com/repos/{owner}/{name}/contents/{path}?ref={commit}`, with the media type `application/vnd.github.raw`. One request reads the file's old path at the merge base, unless the file was added. Another reads its new path at the head commit, unless it was deleted. Each path segment SHALL be percent-encoded.

$$\text{requests per file read} \le 1 + 2$$

An **image read** (see *Pull-Request Image Reads*) SHALL send only requests 3 and 4, for the image file's old path at the merge base and its new path at the head. It shares the merge base a file read keeps with the detail, and it never diffs its versions.

The two versions SHALL be diffed locally, line by line, into hunks with three lines of context, an added file against an empty old version and a deleted file against an empty new one. A version without a final newline SHALL carry the no-newline flag on its last line. Each version SHALL be read to at most 8 MiB. Past that, or when the diff's text passes 8 MiB, the file SHALL be too large to preview. A version holding a NUL byte, or one that is not valid UTF-8, SHALL make the file binary. The file SHALL keep the counts its files entry reported, whatever the local diff counts.

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

An entry without a `patch` SHALL be withheld, to be loaded by a file read, when it has added or removed lines. Otherwise it SHALL be a file with no hunks, shown by its status alone, because GitHub does not say whether it is a rename or type change without content changes, a mode-only change, or a binary or empty file; it SHALL never be called too large or binary. Such a file that is an image file (see the *Image Comparison* requirement in the `diff-view` capability) SHALL show its versions on request instead (see *Pull-Request Image Reads*), still called neither too large nor binary. The line and byte budgets SHALL then be applied.

**Replies.** Every detail request SHALL follow no redirect and SHALL carry the token only in its `Authorization` header (see the *GitHub Privacy and Safety* requirement in the `github-pull-requests` capability). Replies SHALL be classified by the status half of the *GitHub Failure Classification* requirement in the `github-pull-requests` capability: a 401, and a 403 without a rate-limit signal, SHALL be unauthenticated; a 403 with a rate-limit signal, and a 429, SHALL be rate-limited, setting a deadline by that requirement's delay formula (see *Shared Backoff and Detail Budget*). The query's reply SHALL follow the poller's GraphQL rules: an error of type `RATE_LIMITED` SHALL be rate-limited, and no data with an error of type `INSUFFICIENT_SCOPES` SHALL be unauthenticated; otherwise `data.repository.pullRequest` SHALL be read. While `data` is present, a null `repository` or a null `pullRequest` SHALL be unavailable. A null or absent `data` is GitHub's answer to an execution failure such as a timeout, so it SHALL be transient. A files page SHALL be read as a JSON array. Unlike the poller, a redirect or a 404 on a files GET SHALL be unavailable for that pull request rather than transient, so a moved or deleted repository is reported instead of retried. A file read's GETs SHALL be classified as a files GET is, with three additions:

- a rate-limited reply sets the deadlines a files GET's would;
- a redirect or a 404 makes the file unavailable;
- a compare reply SHALL be read as a JSON object whose `merge_base_commit.sha` is a 40-character hexadecimal commit, and anything else in a 2xx compare reply SHALL be transient. Any other reply, a transport error, a redirect on the query or any other non-success status, SHALL be transient.

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

- **WHEN** a files entry for `bin/run.sh` has no `patch` and reports no added or removed lines
- **THEN** the file is shown by its status alone, with no hunks
- **AND** it is called neither too large nor binary

#### Scenario: A patchless image shows its versions on request

- **WHEN** a files entry for `icons/app.png` has no `patch` and reports no added or removed lines
- **THEN** the file shows "Show image" rather than "No textual changes"
- **AND** it is called neither too large nor binary

#### Scenario: A patchless file with lines is too large

- **WHEN** a files entry has no `patch` and reports 4,000 added lines, and the file's new version is 9 MiB
- **THEN** the file arrives withheld, and once loaded it is too large to preview and keeps its counts

#### Scenario: A patchless file with lines is read on request

- **WHEN** the reader loads a modified file GitHub sent without its patch, for the first time since the pull request was read
- **THEN** the file read sends the compare GET, then the contents GET of its old path at the merge base, then of its new path at the head commit
- **AND** the file shows the local diff of the two versions

#### Scenario: The merge base is read once per detail

- **WHEN** the reader loads a second file GitHub sent without its patch, from the same cached detail
- **THEN** its file read sends only the two contents GETs

#### Scenario: An added file reads only its new version

- **WHEN** the reader loads an added file GitHub sent without its patch
- **THEN** no contents GET is sent for the merge base, and every line of the file shows as added

#### Scenario: A version that is not text is binary

- **WHEN** one version of a loaded file holds a NUL byte
- **THEN** the file is binary, and no hunks are shown

#### Scenario: A rate-limited file read sets the REST deadline

- **WHEN** GitHub answers a file read's contents GET with a 429 that reports no secondary limit
- **THEN** the REST deadline is set by the delay formula, and the file read answers `failed` with the reason `deferred`

#### Scenario: A moved or deleted repository is reported, not retried

- **WHEN** a files GET answers 404, or answers with a redirect
- **THEN** the redirect is not followed and the read is unavailable for that pull request

#### Scenario: A missing pull request is unavailable

- **WHEN** the query's reply carries a null `data.repository.pullRequest`
- **THEN** the read is unavailable for that pull request

#### Scenario: A missing scope is a credential problem

- **WHEN** the query's reply carries no data and an error of type `INSUFFICIENT_SCOPES`
- **THEN** the read is unauthenticated

### Requirement: Pull-Request Content Is Untrusted

Descriptions and comments are written by others, and SHALL render through the shared markdown renderer in a **pull-request mode**, in both presentations and on both hosts. That mode SHALL be the guard wherever pull-request content renders, including the main window, which keeps its broad capability and has no content-security policy governing what it loads. The mode SHALL apply only to pull-request content; workspace markdown keeps its own rendering (see the *Mermaid Diagram Rendering* and *Link Handling in Rendered Artifacts* requirements in the `spec-browser` capability). In pull-request mode:

- **Raw HTML** SHALL stay unrendered.
- **HTML comments** SHALL be removed after parsing, by dropping each HTML node whose whole value is a comment. The source text SHALL never be edited, so removing a comment cannot create a fence, a link or an image, and a comment inside code SHALL stay visible.
- **Mermaid fences** SHALL render as their source, as plain unhighlighted text, and SHALL NOT be drawn, since a diagram can fetch remote images while it is laid out.
- **Maths** SHALL render as it does in artifacts, under the same non-trusting posture (see the *Mathematical Notation Rendering* requirement in the `spec-browser` capability).
- **`svg` fences** SHALL keep their inert image rendering (see the *SVG Fence Rendering* requirement in the `spec-browser` capability).
- **Remote images** SHALL NOT be loaded. A standalone image SHALL render as a labelled link showing its alt text and its host. An image inside a link SHALL render as its alt text and host only, as part of the enclosing link, with no link or handler of its own, so one activation opens one destination.
- **Links** SHALL follow *Desktop Link Opener*: in the desktop application they open only through its command, and in the browser skin in a new opener-isolated tab.
- **Identifiers** SHALL NOT be taken from the content. No element rendered from it SHALL carry an `id` derived from its text, heading identifiers included, so content cannot collide with, or be styled as, the application's own elements.

**Hidden characters.** The pull request's title and branch names in the view's header SHALL render each character that renders as nothing as a visible, marked escape, with the escapes and the exemptions the `diff-view` capability gives text outside changed lines (see the *Hidden Characters Are Shown* requirement in the `diff-view` capability).

**DNS prefetching** SHALL be off in every SpecForge page, the main window, reader windows and pull-request windows on both hosts, so no host is resolved merely because a link to it is displayed.

**Changed image files** are not content of this mode, and no URL SHALL ever load them. An image file's versions SHALL be read only by an image read from the provider's API (see *Pull-Request Image Reads*). They SHALL be checked as the *Image Comparison* requirement in the `diff-view` capability says, and rendered from memory, which the pull-request window's content-security policy already allows (see *Pull-Request Window Permissions*). A remote image in a description or comment SHALL stay a labelled link, whatever the pull request's changed files hold.

#### Scenario: Raw HTML stays unrendered

- **WHEN** a description contains `<img src="https://tracker.example/p.gif">`
- **THEN** no image element is created from it
- **AND** no request is made to `tracker.example`

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

#### Scenario: A changed image renders without a request of its own

- **WHEN** a pull-request window shows the versions of a changed `icons/app.png`, and the description embeds `![app](https://img.example/app.png)`
- **THEN** the versions render from bytes the image read returned, and the only requests made for them went to the provider's API
- **AND** the description's image still renders as a labelled link, and no request is made to `img.example`

