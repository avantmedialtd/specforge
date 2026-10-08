## MODIFIED Requirements

### Requirement: Commit Detail View

The commit-detail view rendered in the center pane SHALL show the commit's metadata (abbreviated and full hash, author, date, and full message), the list of files the commit changed with each file's status and per-file added/removed line counts, and the textual diff of the change. The metadata SHALL include the commit's git trailers — the `Key: value` lines of the message's last paragraph as recognized by git's own trailer parser — rendered as a list of key/value pairs in git's emitted order, with every value shown when a key appears more than once. Trailers SHALL be presented as neutral commit metadata: the `OpenSpec-Id` trailer SHALL receive no styling, link, or marker that distinguishes it from any other trailer, and a commit whose message carries no trailers SHALL render no trailer section. A breadcrumb SHALL indicate the commit context and that selecting an artifact returns to the artifact view.

**Rendering.** The changed files and their diff SHALL be rendered through the `diff-view` capability, in the layout the reader chose there — unified or side by side — as that capability puts the choice into effect. The commit-detail view SHALL render its own header (the metadata, trailers and breadcrumb above, and a merge's first-parent label) and SHALL NOT render or parse a diff of its own. It SHALL name the diff's two sides for the diff view: the old side by the first parent's abbreviated hash, or `empty tree` for a root commit, and the new side by the commit's own abbreviated hash. The layout SHALL NOT reach the application service: which files arrive with their hunks, what crosses IPC, and how a file loads on request SHALL be the same in either layout, and switching between the layouts SHALL read nothing again.

**Reading the commit.** Opening a commit's detail SHALL cost no more than a fixed number of `git` processes and IPC calls, whatever the number $$N$$ of files the commit changed. One `git` invocation SHALL list every changed file with its status, its old and new paths, its old and new modes, and its added/removed counts, and one further invocation, read as a stream, SHALL supply the patch text. Which files arrive with their hunks SHALL be decided by the budgets of the `diff-view` capability: its line budget from the file list, before any patch is read, and its byte limits as the patch streams, which only ever withhold more. Every other file with a patch SHALL arrive withheld, with its counts and without its hunks, and a binary file SHALL be listed without a diff; an image file then shows its versions, as **Images** below says. Opening a commit's detail SHALL read no image. The application SHALL stop reading the stream after the last file that arrives with its hunks, and SHALL abandon it once 8 MiB of patch text has been read in all, withholding every patched file not yet reached; a `git` process the application stops deliberately SHALL count as a successful read, not as a failure. Every patch line and every file-list record SHALL be decoded on its own, with bytes that are not valid UTF-8 replaced, so a file in another encoding shows replacement characters in its own lines or path and no other file is affected; paths SHALL be read verbatim from the file list, never C-quoted.

**A truthful file list.**

- Renames SHALL be detected at git's default similarity, the heuristic `git show` and `git log -M` apply: a renamed file is one file, shown as its old path → its new path, with its hunks when its content also changed — never a deletion plus an addition.
- A change of file type, between a regular file, a symbolic link and a submodule, SHALL be one type-changed file, never a deletion plus an addition.
- A root commit SHALL be diffed against the empty tree, so it lists every file it added rather than reporting that it changed no files.
- A merge commit SHALL be diffed against its first parent as a two-tree diff, never as a combined diff, so it lists every file that differs from that parent and not only the files the merge resolved by hand. The view SHALL label that diff `Changes against first parent` followed by the parent's abbreviated hash. The application service SHALL read the first parent as a hexadecimal object id rather than accept it from the caller, and SHALL pass it to `git` after the end-of-options marker, as it does every commit reference (see the *Commit References Are Injection-Safe Arguments* requirement).

The diff SHALL be computed by git's plumbing, which honours git's core diff settings, `diff.renameLimit` among them, but not its display settings: hunks follow git's default algorithm and carry three lines of context even where the user has set `diff.algorithm` or `diff.context`.

**Loading a withheld file.** A withheld file SHALL load on request, alone, from the same commit and against the same base as the rest of its diff — the first parent for a merge, the empty tree for a root commit. The read SHALL pass the file's path, and for a renamed file its old path too, as literal pathspecs, so a path such as `pages/[id].tsx` reads only that file and a renamed file loads as a rename. A file whose diff text exceeds 8 MiB SHALL load as too large to preview, reading `too large to preview`, with no hunks.

**Images.** Commit detail SHALL be a host whose image reader reads as a section nears the view (see the *Image Comparison* and *Diff View Hosts* requirements in the `diff-view` capability). An image file's read SHALL take the file's two versions from the same commit and against the same base as its diff: the first parent for a merge, and for a root commit the empty tree, which leaves the old side absent.
- **Identifying the versions.** The read SHALL identify them by the object ids `git` reports for the file's path, and for a renamed file its old path too, passed as literal pathspecs.
- **Sizes before bytes.** It SHALL read each side's size before any of its bytes, and SHALL read no bytes of a side past 8 MiB.
- **What reaches `git`.** It SHALL pass `git` nothing beyond the commit, its first parent, those literal pathspecs and the object ids `git` itself reported, and SHALL pass the object ids on standard input.
- **Fixed cost.** Each image file's read SHALL cost at most four `git` processes, whatever the file's size and whatever the commit holds: one for the commit's parents, one for the two object ids, one for their sizes and one for the bytes of the sides within 8 MiB, which SHALL NOT run when no side is.

$$\text{git processes per image file} \le 4$$

**Commands.** `get_commit_detail` SHALL return the commit's files under the budgets, and `get_commit_diff` the one file read on request, with its hunks or as too large to preview, both in the diff model the `diff-view` capability defines. Both SHALL keep their names and existing arguments; `get_commit_diff` SHALL additionally accept the file's old path as an optional argument. `get_commit_file_image(repoId, sha, path, oldPath)` SHALL return an image file's old and new sides, as the *Image Comparison* requirement in the `diff-view` capability defines them. All three SHALL be served on the desktop command surface and the web command endpoint alike, and all three are commit-reading operations under the *Commit References Are Injection-Safe Arguments* and *Commit Reading Is Restricted to Registered Repositories* requirements.

#### Scenario: Detail view lists changed files and diff

- **WHEN** the commit-detail view renders for a commit
- **THEN** it shows the commit's metadata, the changed-files list with each file's status and added/removed counts, and the diff
- **AND** the files and their diff are rendered through the `diff-view` capability, in the layout the reader chose

#### Scenario: Commit trailers are listed

- **WHEN** the commit-detail view renders for a commit whose message carries git trailers (e.g. `OpenSpec-Id` and `Co-Authored-By`)
- **THEN** each trailer is shown as a key/value pair in git's emitted order

#### Scenario: Repeated trailer keys are all shown

- **WHEN** a commit carries the same trailer key more than once (e.g. two `Co-Authored-By` lines)
- **THEN** every occurrence is listed and not collapsed to a single entry

#### Scenario: Body prose is not shown as a trailer

- **WHEN** a commit's message has a multi-paragraph body and only its last paragraph contains trailers
- **THEN** only the recognized trailers are listed and the body prose is not mistaken for a trailer

#### Scenario: OpenSpec-Id is rendered as a neutral trailer

- **WHEN** a commit carries an `OpenSpec-Id` trailer
- **THEN** it is displayed identically to any other trailer, with no link, tint, or marker distinguishing it

#### Scenario: A commit with no trailers shows no trailer section

- **WHEN** the commit-detail view renders for a commit whose message carries no trailers
- **THEN** no trailer list or empty trailer affordance is shown

#### Scenario: Breadcrumb indicates how to return

- **WHEN** the commit-detail view is shown
- **THEN** a breadcrumb identifies the commit and indicates that selecting an artifact returns to the artifact view

#### Scenario: A commit is read in a fixed number of git processes

- **WHEN** the commit-detail view renders for an ordinary commit that changed one text file, and then for an ordinary commit that changed two hundred text files
- **THEN** the two-hundred-file commit is read with no more `git` processes and no more IPC calls than the one-file commit
- **AND** no `git` process or IPC call is made per file
- **AND** where the budgets withhold the files after the last one that arrives with its hunks, reading stops there and the detail renders without an error

#### Scenario: A renamed file is one file

- **WHEN** the commit-detail view renders for a commit that renamed the forty-line file `src/old.ts` to `src/new.ts` and changed two of its lines
- **THEN** the changed-files list shows one renamed file, `src/old.ts → src/new.ts`
- **AND** its hunks show the two changed lines
- **AND** neither a deletion of `src/old.ts` nor an addition of `src/new.ts` is listed

#### Scenario: A type change is one file

- **WHEN** the commit-detail view renders for a commit that replaced the regular file `config` with a symbolic link of the same name
- **THEN** the changed-files list shows one type-changed file, `config`, with its old and new modes
- **AND** neither a deletion nor an addition of `config` is listed

#### Scenario: A root commit lists the files it added

- **WHEN** the commit-detail view renders for a repository's root commit, which added the two short files `README.md` and `src/main.rs`
- **THEN** both files are listed as added, with every line of each shown as an added line
- **AND** the view does not report that the commit changed no files
- **AND** while side by side is in effect, the old side is named `empty tree`

#### Scenario: A merge shows its changes against its first parent

- **WHEN** the commit-detail view renders for a merge commit whose tree differs from its first parent's in fifty files, two of which the merge resolved by hand
- **THEN** all fifty files are listed as changes against the first parent, and not only the two resolved by hand
- **AND** the diff is labelled `Changes against first parent` followed by the first parent's abbreviated hash
- **AND** a file of the merge loaded on request is diffed against that same parent

#### Scenario: A withheld file loads on request

- **WHEN** the budgets withheld a commit's renamed file `lib/a.rs → lib/b.rs`
- **AND** the reader activates that file's `Load diff` control
- **THEN** that file alone is read from the commit, by its new path and its old path
- **AND** it renders as one renamed file with its hunks, in the layout in effect
- **AND** no other file of the commit is read again

#### Scenario: A path with pattern characters loads only itself

- **WHEN** a commit changed both `pages/[id].tsx` and `pages/i.tsx`, and the budgets withheld `pages/[id].tsx`
- **AND** the reader loads `pages/[id].tsx`
- **THEN** only the changes to `pages/[id].tsx` are read and shown
- **AND** `pages/i.tsx`, which `pages/[id].tsx` would match as a pattern, is not read with it

#### Scenario: A file past the per-file ceiling is too large to preview

- **WHEN** the reader loads a withheld file whose diff text exceeds 8 MiB
- **THEN** the file reads `too large to preview` and shows no hunks
- **AND** the rest of the commit's detail is unchanged

#### Scenario: A file in another encoding does not blank the commit

- **WHEN** the commit-detail view renders for a commit that changed `docs/café.md`, then a file with one line in Latin-1, then another UTF-8 file
- **THEN** `docs/café.md` is listed under that name, not as a C-quoted escape
- **AND** the Latin-1 line shows replacement characters for its undecodable bytes, in its own file only
- **AND** both UTF-8 files show their hunks intact

#### Scenario: Another commit opens in the chosen layout

- **WHEN** the reader switches a commit's detail to side by side, in a pane wide enough for two columns
- **AND** then selects another commit in the rail
- **THEN** that commit's detail opens side by side
- **AND** the old side is named by its first parent's abbreviated hash and the new side by its own abbreviated hash

#### Scenario: Switching layout reads nothing again

- **WHEN** the reader has loaded one withheld file in a commit's detail
- **AND** switches the detail between unified and side by side
- **THEN** no `git` process runs and no IPC call is made
- **AND** the loaded file keeps its hunks, and every other file keeps its hunks or stays withheld exactly as before the switch

#### Scenario: Opening a commit of images reads no image

- **WHEN** the commit-detail view opens a commit that regenerated twenty icons
- **THEN** the detail is read with the same `git` processes and IPC calls as a commit of twenty text files
- **AND** no image is read until a section nears the view

#### Scenario: An image file reads its versions near the view

- **WHEN** the reader scrolls a modified image file's section near the view
- **THEN** that file's two versions are read from the commit and its first parent in exactly four `git` processes
- **AND** they render as the `diff-view` capability's *Image Comparison* requirement says

#### Scenario: An oversized image is refused unread

- **WHEN** a commit adds `assets/huge.png` of 50 MiB
- **THEN** its size is read and its new side is refused as too large
- **AND** none of its bytes are read

#### Scenario: A root commit's image has no old side

- **WHEN** the commit-detail view shows a root commit that added `icon.png`
- **THEN** its read answers an absent old side and the image as its new side

#### Scenario: A renamed image is read by both paths

- **WHEN** a commit renamed `img/a.png` to `img/b.png` and changed its pixels
- **THEN** the old side is read from `img/a.png` in the first parent and the new side from `img/b.png` in the commit

### Requirement: Commit References Are Injection-Safe Arguments

Any commit reference supplied to a commit-reading operation (the commit-detail file list, the per-file diff and the per-file images) SHALL be treated as untrusted data rather than as a command-line argument to the underlying git invocation, such that no reference value can cause git to write, delete, or otherwise mutate any file or the working tree, nor invoke an external program — it can only cause git to read the named commit. To achieve this the application SHALL both (a) reject a reference that is not a plausible git object id (a hexadecimal string of 4 to 64 characters) before it is used, and (b) pass the reference to git in a position that git cannot interpret as an option (after an end-of-options marker). Guarantee (b) SHALL hold at the point where the git command is constructed, so that it protects every frontend and transport that can reach these operations — the desktop command surface and the optional web command endpoint alike — independent of any per-frontend validation. This strengthens the *Read-Only Operation* requirement: that one ensures the UI offers no mutating action; this one ensures the argument-passing path cannot be coerced into a mutating action either.

#### Scenario: A reference shaped like an option cannot write a file

- **WHEN** a commit-reading operation is invoked with a reference value that resembles a git option that would write to a path (for example, a value requesting diff output be written to a file)
- **THEN** no file is created, truncated, or modified as a result
- **AND** the operation returns an error or an empty result rather than executing the option

#### Scenario: A malformed reference is rejected

- **WHEN** a commit-reading operation is invoked with a reference that is not a hexadecimal object id (for example, an empty string, a branch name, or a leading-dash string)
- **THEN** the operation is refused with an error indicating an invalid reference
- **AND** git is not asked to act on that value

#### Scenario: A legitimate commit hash still resolves

- **WHEN** a commit-reading operation is invoked with a valid commit hash from the graph
- **THEN** the operation returns that commit's file list, diff or images as before

#### Scenario: The guarantee holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same reference-safety guarantees apply, because they are enforced where the git command is constructed rather than in a single frontend

### Requirement: Commit Reading Is Restricted to Registered Repositories

A commit-reading operation (the graph, the commit-detail file list, the per-file diff, and the per-file images) SHALL act only on a repository that belongs to a registered workspace, and SHALL refuse a caller-supplied repository identifier that is not the git repository of any registered workspace rather than reading it. Authorization SHALL be decided by comparing the canonical form of the supplied identifier against the canonical git directories of the registered workspaces, using the same path-canonicalization the registry uses to key its entries, so that an equivalent but differently spelled path is neither wrongly refused nor able to evade the check. This authorization SHALL be enforced at the shared application boundary so that it holds identically for every frontend and transport — the desktop command surface and the optional web command endpoint alike — and not only for whichever transport happens to route through that boundary today. This complements *Graceful Degradation Without Git*: a registered-but-unreadable repository still degrades to an empty rail, whereas an unregistered repository is refused as unauthorized.

#### Scenario: An unregistered repository is refused

- **WHEN** a commit-reading operation is invoked with a repository identifier that is not the git repository of any registered workspace
- **THEN** the operation is refused and no commit history, file list, diff, or image of that repository is returned
- **AND** no `git` command is run against that repository

#### Scenario: A registered repository is read normally

- **WHEN** a commit-reading operation is invoked with the repository of a registered workspace
- **THEN** the operation returns that repository's graph, file list, diff, or images as before

#### Scenario: The restriction holds across transports

- **WHEN** a commit-reading operation is reached through the optional web command endpoint rather than the desktop command surface
- **THEN** the same registration check applies, because it is enforced at the shared application boundary both transports use

#### Scenario: Path spelling does not defeat or trip the check

- **WHEN** a registered repository is identified by an equivalent but differently spelled path (for example with a trailing separator, a `..` segment, a symlink, or a platform verbatim prefix)
- **THEN** it is recognized as the same registered repository and read normally

