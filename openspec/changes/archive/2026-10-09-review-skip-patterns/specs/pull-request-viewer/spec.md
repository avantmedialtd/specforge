## ADDED Requirements

### Requirement: Review Skip Patterns

SpecForge SHALL keep one list of **skip patterns** as an application setting. The list is shared by every pull request of both providers and names the files a reader does not intend to review (see *Review Progress* for the state it gives them).

**Syntax.** Each pattern SHALL be one of the following:
- **A regular expression**, written between slashes: a pattern of at least three characters that starts and ends with `/`. What lies between the slashes SHALL be read in the syntax of the Rust `regex` crate. It SHALL match a path when it matches anywhere in it, and `^` anchors it at the repository root.
- **A glob**: any other pattern.
  - `*` matches any run of characters within one path segment, and `?` one such character.
  - `**` matches any number of whole segments, none included.
  - `[…]` matches one character of a class, and `{a,b}` either alternative.
  - A glob containing `/` SHALL match a path when it matches the whole path, from the repository root.
  - A glob without `/` SHALL match when it matches the path's last segment, the file's name, wherever the file is.

Matching SHALL be case-sensitive. A path is the file's path in the pull request as its provider gives it, with `/` between segments and no leading `/`. A file SHALL be matched by its new path and, when it was renamed or deleted, by its old path too, so moving a test file does not bring it into the review:

$$\text{match}(f) = \text{the first } p \in P \text{ such that } \exists\, q \in \text{paths}(f):\ \text{matches}(p, q)$$

where $$P$$ is the list in its order. The pattern a file's progress names is $$\text{match}(f)$$, as written in the list.

**Accepted values.** A list SHALL be accepted only when all of these hold:
- it holds at most 64 patterns;
- each pattern, once surrounding whitespace is removed, is non-empty and at most 256 bytes long;
- each pattern compiles;
- no glob starts with `/`, since paths are relative to the repository root.

The empty list SHALL be accepted, and skips nothing. A list that is not accepted SHALL NOT be stored, and the refusal SHALL name every refused pattern and why. Each pattern SHALL be stored without its surrounding whitespace.

$$\text{accepted}(P) \iff |P| \le 64 \;\wedge\; \forall p \in P:\ 1 \le |\text{trim}(p)| \le 256 \;\wedge\; \text{compiles}(p)$$

**Empty by default.** Until the reader stores a list, the list SHALL be empty and no file SHALL be skipped. A settings file written before skip patterns existed SHALL read as holding the empty list, so upgrading changes no pull request's view.

**Patterns that would not be accepted.** A settings file edited by hand may hold a pattern that would not be accepted. Matching SHALL ignore that pattern and apply the others, and the Settings view SHALL report it on its field.

**Commands.**
- `get_review_skip_patterns` SHALL answer the stored list, and each stored pattern that does not compile, with why.
- `set_review_skip_patterns(patterns)` SHALL store a list, the empty one included.
- Both SHALL be served on both transports, since the patterns are local state of the person using SpecForge.

**Notifying.** Each stored list SHALL raise `review-skip-patterns-changed` carrying the list now stored. It SHALL be emitted directly by the setting command on both transports, as `pull-request-panel-moved` and `commit-history-enabled-changed` are. The desktop application SHALL send it to every window and the web transport's event stream to every tab. Every pull-request view SHALL read its review progress again on it, and an open Settings view SHALL show the new list without being reopened.

**In Settings.** The Integrations group SHALL present the patterns as one settings row, "Skip in review", after the pull-request cards. The row SHALL be shown while at least one pull-request integration is on, and nothing SHALL stand in for it while both are off. Its description SHALL say that files whose path matches are collapsed in a pull request and left out of its viewed count, that a pattern without `/` matches the file's name, and that `/…/` is a regular expression. The row's controls:
- **One field per pattern**, single-line, each with a control that removes that pattern.
- **An empty field** after them, which adds a pattern. While the list is empty, this field alone SHALL be shown, and its placeholder SHALL suggest `**/tests/**`.

Each field SHALL persist as the *Settings Persist by One Rule* requirement in the `settings-view` capability says for a free-text field. Committing an edited or added pattern SHALL store the whole list with it. Committing a field emptied of its pattern SHALL remove that pattern. A pattern that is not accepted SHALL be reported on its own field, and nothing SHALL be stored. Removing a pattern SHALL persist when activated.

The terminal UI SHALL offer no skip patterns, since it has no pull-request view.

#### Scenario: The list starts empty

- **WHEN** the settings file was written before skip patterns existed
- **THEN** `get_review_skip_patterns` answers the empty list
- **AND** no file of any pull request is skipped
- **AND** the "Skip in review" row shows only the empty field, suggesting `**/tests/**`

#### Scenario: A glob without a slash matches the file's name anywhere

- **WHEN** the list holds `*_test.go` and a pull request changes `pkg/api/server_test.go`
- **THEN** that file matches `*_test.go`

#### Scenario: A glob with a slash matches from the root

- **WHEN** the list holds only `tests/**` and a pull request changes `tests/parse.rs` and `crates/core/tests/parse.rs`
- **THEN** `tests/parse.rs` matches and `crates/core/tests/parse.rs` does not

#### Scenario: A star stays within its segment

- **WHEN** the list holds only `src/*.test.ts` and a pull request changes `src/ui/button.test.ts`
- **THEN** that file does not match

#### Scenario: A regular expression is searched in the path

- **WHEN** the list holds `/(^|/)fixtures?//` and a pull request changes `spec/fixtures/user.json`
- **THEN** that file matches the pattern `/(^|/)fixtures?//`

#### Scenario: The first matching pattern is named

- **WHEN** the list holds `**/tests/**` and then `*.rs`, and a pull request changes `crates/core/tests/parse.rs`
- **THEN** the file's progress names `**/tests/**`

#### Scenario: A renamed test file is still matched

- **WHEN** the list holds `**/tests/**` and a pull request renames `tests/parse.rs` to `checks/parse.rs`
- **THEN** that file matches `**/tests/**`

#### Scenario: Matching is case-sensitive

- **WHEN** the list holds `**/tests/**` and a pull request changes `Tests/Parse.rs`
- **THEN** that file does not match

#### Scenario: A pattern that does not compile is refused on its field

- **WHEN** the reader commits `/(unclosed/` as a new pattern
- **THEN** nothing is stored
- **AND** that field reports that the regular expression does not compile
- **AND** the stored list remains in effect

#### Scenario: A glob with a leading slash is refused

- **WHEN** `set_review_skip_patterns` is sent a list holding `/build/**`
- **THEN** it is refused, naming `/build/**`, and nothing is stored

#### Scenario: Too many patterns are refused

- **WHEN** `set_review_skip_patterns` is sent a list of 65 patterns
- **THEN** it is refused and nothing is stored

#### Scenario: An empty list skips nothing

- **WHEN** the reader removes every pattern
- **THEN** the empty list is stored
- **AND** no file of any pull request is skipped

#### Scenario: A change reaches every open pull request

- **WHEN** a pull request that changes `docs/guide.md` is open in a pull-request window and the reader adds `docs/**` in the Settings view of the main window
- **THEN** the window receives `review-skip-patterns-changed`, reads its review progress again, and shows `docs/guide.md` skipped with its section collapsed

#### Scenario: A hand-edited pattern that does not compile is ignored

- **WHEN** the settings file holds the list `/(unclosed/` and `**/tests/**`
- **THEN** files under a `tests` directory are skipped
- **AND** the Settings view reports `/(unclosed/` on its field

#### Scenario: No row while both integrations are off

- **WHEN** both pull-request integrations are off and the reader opens the Integrations group
- **THEN** no "Skip in review" row is shown and nothing stands in for it

#### Scenario: The browser skin edits the patterns

- **WHEN** the browser skin sends `set_review_skip_patterns` with an accepted list
- **THEN** the web transport dispatches it, the list is stored, and every tab receives `review-skip-patterns-changed`

## MODIFIED Requirements

### Requirement: Review Progress

The view SHALL let the reader mark each file, and each hunk of a file, viewed and unmark it, and SHALL let the reader include in the review a file the skip patterns would skip (see *Review Skip Patterns*), and SHALL keep that progress on this machine only. Progress SHALL NOT be sent to either host, and SpecForge SHALL NOT write GitHub's own viewed state, which would be an action on the pull request.

**Storage.** Progress SHALL live in `review-progress.json` in the shared configuration directory, owned by the application service as the activity log is, created on the first mark and written atomically. It SHALL be keyed by the canonical reference: the provider, the owner and repository in lowercase, and the number. Each entry SHALL hold the head commit at the last mark (`lastMarkedHead`), the key of each marked file by its path, the keys of each file's viewed hunks by its path (`hunks`, left out while it is empty), the paths of the files the reader included in the review (`included`, left out while it is empty), and when the entry was last touched (`touchedAt`). An entry without `hunks`, as every entry written before hunk marks is, SHALL read as one with none, and an entry without `included`, as every entry written before skip patterns is, SHALL read as one that includes nothing.

**Marking.** `set_file_viewed(reference, path, viewed, head, base)` SHALL name the head and base commits the view rendered: GitHub's head and base commit ids, or BitBucket's source and destination commits. It SHALL refuse when the reference has no cached detail, when the path is not among that detail's files, or when either commit differs from the cached detail's, since a retarget changes patches without a push just as a push does. `set_hunk_viewed(reference, path, hunk, viewed, head, base)` SHALL name one hunk by its index among the file's hunks, counted from zero in the order the view renders them. It SHALL be refused in every case `set_file_viewed` is, when the file's hunks are not known (see *Keys*), and when the index is past the file's last hunk. An unmark of either kind SHALL never create an entry.

**Including.** `set_file_included(reference, path, included, head, base)` SHALL add the path to the entry's `included`, or remove it from them. It SHALL be refused in every case `set_file_viewed` is. Removing a path SHALL never create an entry. Including is kept by path, never by key, so no push or retarget undoes it. Including or excluding SHALL change no file or hunk key and SHALL NOT advance `lastMarkedHead`.

Every stored mark or unmark, of a file or of one of its hunks, SHALL also add the file's path to `included` when a skip pattern matches the file. A reader who has marked or unmarked a file has taken it into the review, so unmarking it leaves it unviewed rather than skipped again.

**Keys.** The service SHALL compute each file's key and each hunk's key itself, and no caller SHALL supply one:

$$\text{key}(f) = \begin{cases} \text{SHA-256}\bigl(\text{patch}(f)\bigr) & \text{when } f \text{ has patch text} \\ \bigl(\text{sha}(f),\ \text{status}(f),\ \text{previous}(f),\ \text{base branch}\bigr) & \text{else when GitHub gives } f \text{ a blob sha} \\ \bigl(\text{head commit},\ \text{base branch}\bigr) & \text{otherwise} \end{cases}$$

- A file with patch text, including one the budgets withheld, whose patch sits in the cached detail, SHALL be keyed by the hex-encoded SHA-256 of its patch bytes, never by a non-cryptographic or shortened hash, since the author controls the patch and keys persist across runs and toolchains.
- A file without patch text, whether binary, too large, or a GitHub entry with no `patch`, SHALL be keyed by GitHub's blob `sha` together with its status, its previous filename and the base branch's name, when GitHub gives a `sha`.
- Any other file, which includes every BitBucket file without patch text, SHALL be keyed by the head commit and the base branch's name. Any push or retarget then flags it changed since viewed, and the view SHALL say why.

A file's hunks are **known** when the cached detail holds their digests and the file has at least one hunk. That is a file with patch text, shown or withheld, from the detail read, and a file GitHub sent without its patch once a file read of it has been kept. A hunk's key SHALL be computed from its **body**: the bytes from just after its `@@` header line to the end of its last line, each line's marker and newline and any `\ No newline at end of file` line included. The bytes SHALL be exactly as the provider sent them, or as the local diff of a file read wrote them. The header, with its line ranges and its section heading, SHALL take no part, so a hunk whose lines a push leaves alone keeps its key wherever it moves. Identical bodies in one file SHALL be numbered in order, so marking one never marks another:

$$\text{key}(h_i) = \bigl(\text{SHA-256}(\text{body}(h_i)),\ n_i\bigr) \qquad n_i = \bigl|\{\, j \le i : \text{body}(h_j) = \text{body}(h_i) \,\}\bigr|$$

The digest SHALL be the full, hex-encoded SHA-256 of the bytes as received, never of decoded text and never shortened, for the file key's reasons.

**Writes.** For a file with current key $$k$$, stored key $$S_f$$ and stored hunk keys $$S_h$$, a hunk $$h$$ SHALL be viewed when $$S_f = k$$ or $$\text{key}(h) \in S_h$$. Let $$H$$ be the file's current hunks and $$V$$ the keys of those that are viewed. Each write SHALL replace both stored values:

| Write | Stored hunk keys after it | Stored file key after it |
|---|---|---|
| mark the file | every current hunk's key when its hunks are known, else $$S_h$$ unchanged | $$k$$ |
| unmark the file | none | none |
| mark hunk $$h$$ | $$V \cup \{\text{key}(h)\}$$ | $$k$$ when that covers every hunk of $$H$$, else $$S_f$$ unchanged |
| unmark hunk $$h$$ | $$V \setminus \{\text{key}(h)\}$$ | none |

After any write to a file whose hunks are known, no stored hunk key of that file matches none of its current hunks.

**States.** A file's state SHALL be derived from its keys, its inclusion and the skip patterns alone:

$$\text{state}(f) = \begin{cases} \text{viewed} & S_f = k \;\lor\; \bigl(H \text{ known} \wedge \forall h \in H:\ \text{viewed}(h)\bigr) \\ \text{changed since viewed} & \text{else when } S_f \text{ is stored} \\ \text{partly viewed} & \text{else when } \bigl(H \text{ known} \wedge \exists h \in H:\ \text{viewed}(h)\bigr) \lor \bigl(H \text{ not known} \wedge S_h \ne \emptyset\bigr) \\ \text{skipped} & \text{else when } \text{match}(f) \ne \bot \wedge \text{path}(f) \notin I \\ \text{unviewed} & \text{otherwise} \end{cases}$$

where $$I$$ is the entry's included paths and $$\text{match}(f)$$ is the first skip pattern that matches the file, or $$\bot$$ when none does (see *Review Skip Patterns*). The reader's marks therefore always come first: a file with any stored mark keeps the state those marks give it, whether or not a pattern matches it. A skipped file has nothing stored, so a push or a retarget never makes it changed since viewed. Each file's progress SHALL carry the pattern that matches it, whether or not the file is skipped, and whether its path is included.

The view's header SHALL show "n of m files viewed", how many files are skipped when any are, and how many files are changed since viewed, counted from these states. Skipped files SHALL be left out of the files the header counts, never counted as viewed:

$$m = |F| - s \qquad s = \bigl|\{\, f \in F : \text{state}(f) = \text{skipped} \,\}\bigr|$$

where $$F$$ is the cached detail's files. A partly viewed file counts in neither viewed nor changed since viewed. `lastMarkedHead` SHALL advance only when a file or a hunk is marked, and SHALL only date that count ("since you last marked, at `abc1234`").

**Reading.** `get_review_progress(reference)` SHALL answer for that one pull request, never for the whole store, and only while its provider is enabled, with the states of the cached detail's files under the skip patterns stored when it answers, each file's hunk states in order (none when its hunks are not known), and the cached detail's head and base commits, which those hunk states belong to. While the provider is disabled it SHALL refuse without content.

**Pruning.** An entry untouched for 90 days whose pull request its own provider no longer lists SHALL be pruned, but only while that provider is enabled and its list is complete, once every enabled provider has completed a successful, non-stale refresh in this run, and never at load, when no snapshot exists yet. A disabled provider's empty list proves nothing, so its entries SHALL be kept. Neither does an incomplete one, so while a provider's list is incomplete its entries SHALL be kept too. A GitHub list is incomplete when it reports results withheld (an organisation blocked by single sign-on), and a BitBucket list when it reports skipped workspaces:

$$\text{prune}(e) \iff \text{now} - \text{touchedAt}(e) \ge 90\ \text{days} \;\wedge\; p(e) \in \text{enabled} \;\wedge\; \text{complete}(S_{p(e)}) \;\wedge\; e \notin S_{p(e)} \;\wedge\; \forall p \in \text{enabled}:\ \text{refreshedThisRun}(p)$$

where $$p(e)$$ is the entry's provider.

**Notifying.** Each stored mark or unmark, of a file or of a hunk, and each stored inclusion or exclusion, SHALL raise a `review-progress-changed` notice carrying the reference, on a broadcast the application service owns. The desktop application SHALL forward it to every window, and the web transport's event stream SHALL carry it to every tab. It SHALL never be emitted directly by a command, and SHALL NOT be a variant of the cache-event stream. Every view of one pull request in one service, whether in the center pane, in a pull-request window or in a tab served by the desktop's embedded server, therefore stays in step.

**Transports.** `get_review_progress`, `set_file_viewed`, `set_hunk_viewed` and `set_file_included` SHALL be served on both transports, since progress is local state of the person using SpecForge rather than a write to either host.

**Two writers.** A standalone `specforge-serve` running beside the desktop application is a second writer of the same file, as it is of the activity log. Each process SHALL re-read the file before each write. This narrows lost marks without closing the race, and neither process sees the other's marks until it re-reads. A version of SpecForge from before hunk marks rewrites any entry it marks or unmarks without its `hunks`. Its write therefore drops that pull request's hunk marks and keeps its file marks. A version from before skip patterns likewise rewrites the entry without its `included`, so the files of that pull request the reader included are skipped again unless a mark of theirs holds.

#### Scenario: A mark survives a restart and never leaves the machine

- **WHEN** the user marks a file viewed, restarts SpecForge and reopens the pull request
- **THEN** once its detail has been read again, the file is still viewed
- **AND** no request carrying the mark was sent to either host

#### Scenario: A push that changes a file flags it

- **WHEN** a file marked viewed through its box has the lines of one of its four hunks changed in a later push
- **THEN** the file is changed since viewed, with one hunk to review
- **AND** the header's changed-since-viewed count includes it
- **AND** its other three hunks are still viewed

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

- **WHEN** a pull request has ten files, none of which a skip pattern matches, of which four are viewed and one is changed since viewed
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

#### Scenario: An incomplete list prunes nothing of its provider

- **WHEN** GitHub's successful refresh reports results withheld from an organisation that requires single sign-on, and a GitHub entry untouched for 120 days is not in the list
- **THEN** the entry is kept until a refresh that withholds nothing leaves it out

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

#### Scenario: A rebase that only moves a file's hunks keeps it viewed

- **WHEN** a viewed file's pull request is rebased onto a base that added lines above the file's hunks, so that every hunk's line ranges change and none of its lines do
- **THEN** the file is still viewed

#### Scenario: Marking a hunk leaves the others alone

- **WHEN** the reader marks the second of a file's three hunks viewed
- **THEN** the second hunk is viewed and the first and third are not
- **AND** the file is partly viewed

#### Scenario: Marking a file marks every hunk

- **WHEN** the reader marks a file of four hunks viewed
- **THEN** all four hunks are viewed
- **AND** the entry stores the file's key and its four hunk keys

#### Scenario: Marking the last hunk marks the file

- **WHEN** three of a file's four hunks are viewed and the reader marks the fourth
- **THEN** the file is viewed
- **AND** the entry stores the file's key

#### Scenario: Unmarking a hunk keeps the others viewed

- **WHEN** a file of four hunks was marked viewed through its box before hunk marks existed, so its entry stores its key and no hunk keys, and the reader unmarks one of its hunks
- **THEN** the file is partly viewed, with the other three hunks still viewed
- **AND** the entry stores those three hunk keys and no key for the file

#### Scenario: Unmarking a file clears its hunks

- **WHEN** a file has two viewed hunks and the reader unmarks the file
- **THEN** the file is unviewed
- **AND** the entry stores neither its key nor any of its hunk keys

#### Scenario: Identical hunks are marked apart

- **WHEN** a file has two hunks whose bodies are byte-identical and the reader marks the first viewed
- **THEN** the first is viewed and the second is not

#### Scenario: A hunk is keyed by the bytes the provider sent

- **WHEN** a hunk of a BitBucket pull request holding the Latin-1 byte `0xE9` was marked viewed, and a later push replaces that byte with `0xE8`, which decodes to the same replacement character
- **THEN** that hunk is unviewed

#### Scenario: Stored hunk keys follow the file's hunks

- **WHEN** a later push changed one of a file's viewed hunks, and the reader then marks another hunk of that file
- **THEN** the entry stores no key for the changed hunk's earlier body

#### Scenario: A hunk mark needs the file's hunks

- **WHEN** `set_hunk_viewed` names a file GitHub sent without its patch that has not been loaded
- **THEN** the mark is refused and nothing is stored

#### Scenario: A hunk past the last is refused

- **WHEN** `set_hunk_viewed` names hunk 4 of a file with four hunks
- **THEN** the mark is refused and nothing is stored

#### Scenario: A loaded file's hunks become known

- **WHEN** a file GitHub sent without its patch has been loaded
- **THEN** `get_review_progress` gives its hunk states
- **AND** `set_hunk_viewed` marks its hunks

#### Scenario: A file without hunks keeps a file mark only

- **WHEN** a binary file is marked viewed
- **THEN** it is viewed
- **AND** its progress carries no hunk states

#### Scenario: An unloaded file with hunk marks is partly viewed

- **WHEN** one hunk of a file GitHub sent without its patch was marked viewed, and after a restart the detail has been read again but the file has not been loaded
- **THEN** the file is partly viewed, with no hunk states

#### Scenario: Hunk states name their detail

- **WHEN** `get_review_progress` answers for a pull request whose cached detail has the head commit `abc1234` and the base commit `0f1e2d3`
- **THEN** the answer names those two commits

#### Scenario: A hunk mark reaches every view

- **WHEN** the user marks a hunk viewed in a pull-request window while the center pane shows the same pull request
- **THEN** the center pane receives `review-progress-changed` and shows the hunk viewed

#### Scenario: The browser skin marks hunks

- **WHEN** the browser skin sends `set_hunk_viewed` for a hunk of a file of a pull request whose detail is cached
- **THEN** the web transport dispatches it and the mark is stored

#### Scenario: An entry from before hunk marks reads unchanged

- **WHEN** `review-progress.json` holds an entry with file keys and no `hunks`
- **THEN** each of its files is in the state it was in before hunk marks existed

#### Scenario: A matching file with nothing stored is skipped

- **WHEN** the skip patterns hold `**/tests/**` and a pull request changes `crates/core/tests/parse.rs`, of which nothing is stored
- **THEN** `get_review_progress` gives that file the state skipped, the pattern `**/tests/**`, and no inclusion
- **AND** no entry is created

#### Scenario: Skipped files are counted apart

- **WHEN** a pull request has ten files, of which three are skipped, four are viewed and none is changed since viewed
- **THEN** the header shows "4 of 7 files viewed" and three files skipped

#### Scenario: A push never brings a skipped file back

- **WHEN** a skipped file's patch changes in a later push
- **THEN** the file is still skipped
- **AND** the header's changed-since-viewed count does not include it

#### Scenario: Including a skipped file

- **WHEN** the reader includes a skipped file through `set_file_included`
- **THEN** the file is unviewed, still carries its matching pattern, and is included
- **AND** `lastMarkedHead` is unchanged
- **AND** every view of the pull request receives `review-progress-changed`

#### Scenario: Inclusion survives a push

- **WHEN** an included file's patch changes in a later push
- **THEN** the file is still included and unviewed

#### Scenario: Excluding creates nothing

- **WHEN** `set_file_included` removes a path of a pull request that has no entry
- **THEN** no entry is created

#### Scenario: Including is refused as a mark is

- **WHEN** `set_file_included` names a head commit that differs from the cached detail's
- **THEN** it is refused and nothing is stored

#### Scenario: Marking a matching file includes it

- **WHEN** the reader marks a skipped file viewed and later unmarks it
- **THEN** the file is viewed after the mark and unviewed after the unmark, and included after both

#### Scenario: An older entry's marks outrank the patterns

- **WHEN** an entry written before skip patterns stores the key of `src/app.test.ts` and no `included`, and the skip patterns hold `*.test.{js,jsx,ts,tsx,mjs,cjs}`
- **THEN** the file is viewed while its key holds
- **AND** it is changed since viewed, not skipped, after a push changes it

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
