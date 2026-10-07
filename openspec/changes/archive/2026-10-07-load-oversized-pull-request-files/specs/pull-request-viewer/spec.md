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

**Too-large files.** A file too large to preview SHALL carry, in its preamble slot, a link to that file's diff on its provider's page. On GitHub the link SHALL be the pull request's page followed by `/files#diff-` and the lowercase hexadecimal SHA-256 digest of the file's path. On BitBucket it SHALL be the pull request's page followed by `/diff#chg-` and the path. The path SHALL be the file's new path, or its old path when it was deleted. The link SHALL be built from the provider page the detail carries, and SHALL open as the view's other provider links do: an opener-isolated tab in the browser skin, and the desktop link opener in the desktop application (see *Desktop Link Opener*). A binary file SHALL carry no such link.

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

### Requirement: Detail Reads Are Scoped to the Snapshot

A pull request's **reference** SHALL be its provider, its owner (a GitHub owner or a BitBucket workspace), its repository and its number. Two references SHALL be equal when their providers and numbers are equal and their owners and repositories are equal ignoring ASCII case. The commands that read a pull request's detail, its withheld files and its review progress SHALL take a reference, and no new command SHALL identify a pull request by a URL.

`get_pull_request_detail(reference, manual, cachedOnly)`, where `manual` says only whether the read is a manual refresh and `cachedOnly` asks only for what the cache holds, neither reaching a request, SHALL look the reference up in its provider's current snapshot, fresh or stale, for a row with an equal reference and a non-empty URL, and SHALL take the repository, the number and the URL from that row. Nothing the caller supplies SHALL reach a request beyond the reference itself, and no URL from a frontend SHALL reach a provider request. A detail read therefore spends the credential only on pull requests the account already lists: on GitHub those it authored or whose review is requested from it, and on BitBucket those it authored. A detail read SHALL never write, SHALL follow no redirect, and SHALL never return the credential.

**The cache.** The service SHALL keep the last detail of each pull request in memory only, keyed by reference, holding at most 32 and dropping the least recently used. A view reopened while its pull request's detail is cached SHALL paint that detail at once whenever the service answers without a request: to a `cachedOnly` call, which SHALL answer from the cache alone, with the cached detail and its read time or with none, whatever the detail's age; under the freshness rule below; because the pull request is no longer listed; or because a deadline or the budget holds, in which case the answer SHALL carry the cached detail beside the time a read becomes possible (see *Shared Backoff and Detail Budget*). When the service reads instead, the view SHALL paint the detail that read returns. When a reference is not listed, the service SHALL answer with its cached detail, marked "no longer listed", while its provider stays enabled, and with no detail when none is cached, which the view reports as not in its provider's list. A reference that is not listed SHALL cause no request.

**Freshness.** A view SHALL ask for a read only when it opens, in either presentation; when its provider's snapshot announcement shows that the pull request's row changed its updated time or, on GitHub, its checks or its count of unresolved conversations, or is the first announcement after the time a deferral named (see *Shared Backoff and Detail Budget*); when the service answers `changed` for a withheld file the view asked for, as **Withheld files** below says; and on a manual refresh. It SHALL NOT read on a timer. The service SHALL answer from the cache, with no request, while the cached detail is under 60 seconds old and the row is unchanged since it was read. A manual refresh SHALL bypass that rule unless a manual refresh of the same pull request sent a read less than 30 seconds before, $$\text{lastManualRead}$$ being when one last did:

$$\text{fromCache} \iff \text{age} < 60\,\text{s} \;\wedge\; \text{row unchanged} \;\wedge\; \neg\bigl(\text{manual} \;\wedge\; \text{now} - \text{lastManualRead} \ge 30\,\text{s}\bigr)$$

At most one read per pull request SHALL be in flight, whichever presentation asks for it: a caller that asks for a pull request whose read is in flight SHALL receive that read's outcome rather than start another.

**Credential changes.** Disabling a provider, or saving a credential for it, SHALL drop that provider's cached details at once and advance its credential generation. A read SHALL record the generation it started under, and before each of its requests SHALL check that the generation is unchanged and the provider still enabled. A read that finds either changed SHALL send nothing more, and its result SHALL be neither cached nor returned. While a provider is disabled, every read of its pull requests SHALL refuse without content.

**Withheld files.** `get_pull_request_file(reference, path, head, base)` SHALL name the head and base commits the view rendered, and SHALL answer with one of three outcomes:

- `file`, carrying the file;
- `changed`, when no detail is cached, either commit differs from the cached detail's, or no file of the cached detail has that path, after which the view reads the pull request again;
- `failed`, carrying a reason (`deferred` with the time a load becomes possible, `unauthenticated`, `unavailable`, `refused` or `transient`), when the file could not be read, after which the view does not read again.

A file the budgets withheld SHALL be returned from the cached detail with no request. A file GitHub sent without its patch SHALL be read from GitHub the first time it is asked for, as *GitHub Detail Reads* says. Its hunks, or its too-large or binary state, SHALL then be kept with the cached detail, so a later ask sends nothing. That **file read** SHALL be governed as a detail read is:

- it is admitted, deferred and counted as *Shared Backoff and Detail Budget* says;
- it sends only while the provider is enabled under the credential generation it started with;
- what it read is kept only under that same check.

A deferred file read SHALL send nothing. While a provider is disabled, `get_pull_request_file` SHALL answer `failed` with the reason `refused` for its pull requests.

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
- **THEN** the service answers `changed`
- **AND** the view re-reads the pull request

#### Scenario: A file read is read once and kept

- **WHEN** the view asks twice for a file GitHub sent without its patch, naming the commits it rendered
- **THEN** the first ask reads it from GitHub and answers `file` with its hunks
- **AND** the second answers the same file from the cached detail with no request

#### Scenario: A file read counts against the budget

- **WHEN** GitHub's hourly detail budget has room for one more request and the view loads a file GitHub sent without its patch
- **THEN** the file read sends at most that one request, and is then deferred
- **AND** the service answers `failed` with the reason `deferred` and the time the budget next has room

#### Scenario: The browser skin reads detail

- **WHEN** the browser skin sends `get_pull_request_detail` for a listed pull request
- **THEN** the web transport dispatches it and returns the detail the desktop application would receive

### Requirement: GitHub Detail Reads

A GitHub detail read SHALL send only these requests, each to `api.github.com`:

1. one `POST` to `https://api.github.com/graphql` carrying a detail query fixed at compile time: a read-only `query` operation, never a mutation or subscription, byte-identical on every read. Its only variables SHALL be the owner, repository name and number taken from the matched row, carried in GraphQL's `variables` and never interpolated into the query's text. Among the pull request's other fields, the query SHALL read its submitted reviews in the `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED` and `DISMISSED` states only, and each review-thread comment's state, so that nothing of the account's own pending review, which nobody else can see, is shown; each review thread's and comment's id; each thread's path, lines, `diffSide` and `startDiffSide`; whether each comment and review is minimised, and why; and the total number of changed files.
2. `GET https://api.github.com/repos/{owner}/{name}/pulls/{number}/files?per_page=50&page=n`, for at most twenty pages, which is a thousand files. Files beyond the thousandth SHALL be counted rather than listed, and the view SHALL say how many there are, with a pointer to the provider's page.

$$\text{requests per read} = 1 + p_{\text{files}}, \qquad p_{\text{files}} \le 20$$

A **file read** loads one file GitHub sent without its patch (see *Detail Reads Are Scoped to the Snapshot*). It SHALL send only these requests, each a `GET` to `api.github.com`, its commits and path taken from the cached detail:

3. `GET https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}?per_page=1`, naming the cached detail's base and head commits, for `merge_base_commit.sha`: the commit GitHub diffs the pull request against. It SHALL be sent at most once per cached detail. The merge base SHALL be kept with the detail, and a new read of the pull request SHALL drop it.
4. `GET https://api.github.com/repos/{owner}/{name}/contents/{path}?ref={commit}`, with the media type `application/vnd.github.raw`. One request reads the file's old path at the merge base, unless the file was added. Another reads its new path at the head commit, unless it was deleted. Each path segment SHALL be percent-encoded.

$$\text{requests per file read} \le 1 + 2$$

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

An entry without a `patch` SHALL be withheld, to be loaded by a file read, when it has added or removed lines. Otherwise it SHALL be a file with no hunks, shown by its status alone, because GitHub does not say whether it is a rename or type change without content changes, a mode-only change, or a binary or empty file; it SHALL never be called too large or binary. The line and byte budgets SHALL then be applied.

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

- **WHEN** a files entry has no `patch` and reports no added or removed lines
- **THEN** the file is shown by its status alone, with no hunks
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
