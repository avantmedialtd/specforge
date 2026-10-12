## ADDED Requirements

### Requirement: Detail Re-reads Reuse Unchanged Files

A pull request's changed files are fixed by its head commit and its base commit: the merge base follows from the two, and the files are the difference between the merge base and the head. A detail read SHALL therefore **reuse** the cached detail's files, rather than read them again, when every one of these conditions holds:

- the pull request has a cached detail stored under the credential generation the read started with (see *Detail Reads Are Scoped to the Snapshot*);
- the read's first reply names a head commit and a base commit, and reports the same ones as that cached detail. On GitHub these are the detail query's `headRefOid` and `baseRefOid`. On BitBucket they are the pull-request payload's `source.commit.hash` and `destination.commit.hash`. A reply that names no head or no base never reuses;
- on GitHub, the query's `files.totalCount`, when its reply carries one, equals the number of files the cached detail accounts for: those it lists and those it counts as unlisted. A files list that lagged the head commit it was read for then disagrees with the next query, and is read again rather than kept;
- on BitBucket, the payload's `links.diff` and `links.diffstat` are byte-identical to the links the cached detail was read by, and each names the payload's source and destination commits as its diff spec: its path ends `:{source}%0D{destination}`, as BitBucket writes a pull request's links, compared case-insensitively, the `%0D` separator and the hashes alike. An unchanged link then names unchanged content, whether or not BitBucket moves the payload's destination commit when the destination branch moves;
- the read is not a manual refresh.

The check SHALL be made after the first reply and before any file request. It SHALL be made under the cache's lock against the read's own generation, as storing a read's result is.

**What a reusing read sends.** A read that reuses SHALL send none of its provider's file requests: GitHub's files pages, or BitBucket's diffstat pages and diff. It SHALL still send every other request its provider's detail read sends.

**What the reused detail carries.** The read's result SHALL replace everything else the detail carries with what it read: the description, the conversation, the checks, the threads, the branches, the row and the read time. Its files SHALL be the cached detail's files exactly as they were cached, including:

- each file as the line and byte budgets left it, shown or withheld;
- each withheld file's hunks;
- what a file read found for a file sent without its patch;
- each file's hunk keys;
- the count of unlisted files;
- the merge base a file read or an image read learned.

A file read or an image read may keep its result for the cached detail while a reusing read is in flight. When the reusing read is then stored, that result SHALL be kept too.

**What other reads keep.** A read that does not reuse but reports the same head and base commits, under the same generation, SHALL keep the cached merge base and nothing else of the cached files. A read that reports a different head or base commit SHALL drop both the merge base and everything the cached files held.

**Manual refresh.** A manual refresh SHALL read the files in full, keeping the merge base on the terms above, so the files a reusing read carried across can always be replaced from the provider. At most one read of a pull request is in flight (see *Detail Reads Are Scoped to the Snapshot*): a manual refresh asked while another read of the same pull request is in flight joins that read and receives its outcome, which may have reused the files. A manual refresh asked once that read has ended reads the files in full.

Files that are not requested are not counted against the provider's hourly detail budget (see *Shared Backoff and Detail Budget*). The worst-case requests per read are unchanged.

#### Scenario: A push reads the files again

- **WHEN** a GitHub pull request's detail is cached, and a push brings a read whose query reports a different `headRefOid`
- **THEN** the read requests the files pages
- **AND** the cached merge base is dropped, and a file the reader had loaded must be loaded again

#### Scenario: A retargeted base reads the files again

- **WHEN** a pull request's detail is cached, and a read reports the same head commit but a different base commit
- **THEN** the read requests its provider's file requests in full

#### Scenario: A loaded file stays loaded across a re-read of the same commits

- **WHEN** the reader has loaded a file GitHub sent without its patch, and a CI status change brings a read that reports the same head and base commits
- **THEN** the file is answered from the cached detail with no request when the view asks for it again
- **AND** its hunks stay known, so its hunk marks keep their states

#### Scenario: A manual refresh reads the files again

- **WHEN** the user refreshes a pull request manually and the read reports the same head and base commits as its cached detail
- **THEN** the read requests its provider's file requests in full
- **AND** the merge base learned for the cached detail is kept

#### Scenario: A changed BitBucket diff link reads the files again

- **WHEN** a BitBucket read's payload reports the cached detail's commits but names a `links.diff` different from the one the cached detail was read by
- **THEN** the read requests the diffstat and the diff

#### Scenario: A BitBucket link that does not name the commits reads the files again

- **WHEN** a BitBucket read's payload reports the cached detail's commits and names the cached links, but a link's path does not end with `:{source}%0D{destination}` for the payload's commits
- **THEN** the read requests the diffstat and the diff

#### Scenario: A GitHub total that disagrees with the cached files reads them again

- **WHEN** a GitHub read's query reports the cached detail's `headRefOid` and `baseRefOid`, but a `files.totalCount` different from the number of files the cached detail lists and counts
- **THEN** the read requests the files pages

#### Scenario: A detail cached under another credential is not reused

- **WHEN** the user saves a GitHub credential, which drops GitHub's cached details, and a view then reads a pull request whose commits have not moved
- **THEN** the read requests the files pages

#### Scenario: A file read during a reusing read is kept

- **WHEN** a file read keeps its result for a cached detail while a reusing read of the same pull request is in flight
- **THEN** the detail the reusing read stores still carries that file read's result

## MODIFIED Requirements

### Requirement: GitHub Detail Reads

A GitHub detail read SHALL send only these requests, each to `api.github.com`:

1. one `POST` to `https://api.github.com/graphql` carrying a detail query fixed at compile time: a read-only `query` operation, never a mutation or subscription, byte-identical on every read. Its only variables SHALL be the owner, repository name and number taken from the matched row, carried in GraphQL's `variables` and never interpolated into the query's text. Among the pull request's other fields, the query SHALL read its submitted reviews in the `APPROVED`, `CHANGES_REQUESTED`, `COMMENTED` and `DISMISSED` states only, and each review-thread comment's state, so that nothing of the account's own pending review, which nobody else can see, is shown; each review thread's and comment's id; each thread's path, lines, `diffSide` and `startDiffSide`; whether each comment and review is minimised, and why; and the total number of changed files.
2. `GET https://api.github.com/repos/{owner}/{name}/pulls/{number}/files?per_page=50&page=n`, for at most twenty pages, which is a thousand files. Pages SHALL be requested in order. The read SHALL stop at the first of these three points:
   - after the last page that the query's total number of changed files $$n$$ calls for, which is page $$\lceil n/50 \rceil$$ and never before page 1;
   - after the twentieth page;
   - after a page holding fewer than 50 entries.

   When the query's reply carries no total, only the last two bounds SHALL apply. Files beyond the thousandth SHALL be counted rather than listed, and the view SHALL say how many there are, with a pointer to the provider's page. A read that reuses the cached files SHALL send no files GET (see *Detail Re-reads Reuse Unchanged Files*).

$$\text{requests per read} = 1 + p_{\text{files}}, \qquad p_{\text{files}} = 0 \text{ on reuse, otherwise } p_{\text{files}} \le \min\!\left(\max\!\left(\left\lceil \tfrac{n}{50} \right\rceil, 1\right), 20\right)$$

A **file read** loads one file GitHub sent without its patch (see *Detail Reads Are Scoped to the Snapshot*). It SHALL send only these requests, each a `GET` to `api.github.com`, its commits and path taken from the cached detail:

3. `GET https://api.github.com/repos/{owner}/{name}/compare/{base}...{head}?per_page=1`, naming the cached detail's base and head commits, for `merge_base_commit.sha`: the commit GitHub diffs the pull request against. It SHALL be sent at most once per cached detail's head and base commits. The merge base SHALL be kept with the detail. A new read of the pull request SHALL drop it when that read reports a different head or base commit. A read that reports the same commits under the same credential generation SHALL keep it (see *Detail Re-reads Reuse Unchanged Files*).
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

#### Scenario: An exact multiple of fifty sends no empty page

- **WHEN** a pull request with no cached detail changes 100 files and its query reports a total of 100
- **THEN** two files pages are requested, and no third

#### Scenario: A short page still ends the list

- **WHEN** the query reports a total of 120 changed files, and the second files page holds 30 entries
- **THEN** no third page is requested

#### Scenario: A re-read of the same commits sends only the query

- **WHEN** a 300-file pull request's detail is cached, and a new comment brings a read whose query reports the same `headRefOid` and `baseRefOid`, and a `files.totalCount` of 300, under the same credential generation
- **THEN** the read sends the detail query and no files GET, spending one request of the hourly detail budget
- **AND** the view shows the new comment beside the same 300 files

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

- **WHEN** the reader loads a modified file GitHub sent without its patch, for the first time since the pull request was read, and no file read or image read has learned the merge base of the cached detail's head and base commits
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

### Requirement: BitBucket Detail Reads

A BitBucket detail read SHALL send only these GETs, each to `api.bitbucket.org`, none following a redirect, and each carrying the credential only in its `Authorization` header:

1. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`, for the description, author, branches and commits, participants, and the `links.diff` and `links.diffstat` URLs;
2. the diffstat, at `links.diffstat`, for counts, and for the status and rename of every file the diff does not reach, up to ten pages;
3. the diff, at `links.diff`: raw unified text, parsed by the `diff-view` model's unified-diff parser and read up to 8 MiB. Each parsed file SHALL keep the status, paths and modes its diff text gives, since the diffstat has no mode-only or type-change status, and SHALL take its counts from the diffstat entry for its path. Files past that ceiling SHALL be too large to preview and SHALL take their status, rename and counts from the diffstat, and the line and byte budgets SHALL then be applied;
4. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}/comments?pagelen=100`, up to ten pages, for general comments; inline comments, each with its path and its old-side line `inline.from` or new-side line `inline.to`, with `start_from` and `start_to` for a range; replies; resolution; and the deleted flag;
5. `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}/statuses`, for build statuses.

A read SHALL NOT send requests 2 and 3 when it reuses the cached files. It reuses them only under the conditions *Detail Re-reads Reuse Unchanged Files* sets, which on BitBucket include that the payload of request 1 reports the cached detail's source and destination commits and names the `links.diff` and `links.diffstat` the cached detail was read by. The links SHALL be checked as the paragraph below says before the reuse check is made, so a payload whose links may not be followed is unavailable whether or not the files would be reused.

$$\text{requests per read} = \begin{cases} 2 + p_{\text{comments}} & \text{on reuse} \\ 3 + p_{\text{diffstat}} + p_{\text{comments}} & \text{otherwise} \end{cases} \qquad 1 \le p_{\text{diffstat}},\ p_{\text{comments}} \le 10$$

The read SHALL NOT request the pull-request-scoped `/diff` or `/diffstat` endpoints, which answer with a redirect; the payload's links name the destination directly. A link taken from a payload, a page's `next` link included, SHALL be followed only when it parses as an `https` URL whose host is exactly `api.bitbucket.org`, with no user information, no explicit port, and a path under `/2.0/`. Any other link SHALL NOT be requested.

**Replies.**

- A 401, or a 403 on any detail GET, SHALL be unauthenticated and SHALL point to Settings, since a 403 means the token lacks a scope the read needs.
- A 429 SHALL set the shared deadline by the poller's rule: its `Retry-After`, else 300 seconds, and never more than an hour (see *Shared Backoff and Detail Budget*).
- Unlike the poller, a redirect or a 404 SHALL be unavailable for that pull request.
- A payload that lacks its `links.diff` or `links.diffstat`, or names one this read would not follow, SHALL be unavailable.
- A successful reply whose body cannot be read as the JSON its request expects SHALL be transient, as on GitHub.
- A transport error or any other status SHALL be transient.

#### Scenario: A read sends its five GETs

- **WHEN** a listed BitBucket pull request whose diffstat and comments each fit one page is read, with no cached detail of the same commits and links
- **THEN** exactly five GETs are sent, all to `api.bitbucket.org`

#### Scenario: A re-read of the same commits sends three GETs

- **WHEN** a BitBucket pull request's detail is cached, and a build status change brings a read under the same credential generation whose payload reports the same source and destination commits and the same `links.diff` and `links.diffstat`, each naming those commits as BitBucket writes them (`…:{source}%0D{destination}`, compared case-insensitively), with comments that fit one page
- **THEN** exactly three GETs are sent: the pull request, its comments and its statuses
- **AND** neither the diffstat nor the diff is requested, and the view shows the new status beside the cached files

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
- **The merge base.** When a file read or an image read of the same cached detail has learned it, the read SHALL use that merge base and send no request for it. Otherwise it SHALL learn the merge base and keep it with the detail on the terms a file read keeps it. A new read of the pull request SHALL drop it when that read reports a different head or base commit. A read that reports the same commits under the same credential generation SHALL keep it (see *Detail Re-reads Reuse Unchanged Files*).
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

#### Scenario: A merge base survives a re-read of the same commits

- **WHEN** an image read has learned a cached detail's merge base, and a new comment brings a read that reports the same head and base commits, and the reader then asks to see another image file
- **THEN** the image read sends no compare or `merge-base` request, only its version requests

#### Scenario: An added image reads only its new version

- **WHEN** the reader asks to see an image file the pull request adds
- **THEN** no request is sent for an old version
- **AND** the answer carries an absent old side

#### Scenario: A BitBucket image read reads the merge base and both versions

- **WHEN** the reader asks to see a modified image file of a BitBucket pull request for the first time since it was read, and no merge base is known for its cached detail
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

