## MODIFIED Requirements

### Requirement: Commit Detail View

The commit-detail view rendered in the center pane SHALL show the commit's metadata (abbreviated and full hash, author, date, and full message), the list of files the commit changed with each file's status and per-file added/removed line counts, and the textual diff of the change. The metadata SHALL include the commit's git trailers — the `Key: value` lines of the message's last paragraph as recognized by git's own trailer parser — rendered as a list of key/value pairs in git's emitted order, with every value shown when a key appears more than once. Trailers SHALL be presented as neutral commit metadata: the `OpenSpec-Id` trailer SHALL receive no styling, link, or marker that distinguishes it from any other trailer, and a commit whose message carries no trailers SHALL render no trailer section. A breadcrumb SHALL indicate the commit context and that selecting an artifact returns to the artifact view.

**Rendering.** The changed files and their diff SHALL be rendered through the `diff-view` capability, in the layout the reader chose there — unified or side by side — as that capability puts the choice into effect. The commit-detail view SHALL contribute only its own header (the metadata, trailers and breadcrumb above) and SHALL NOT render or parse a diff of its own. It SHALL name the diff's two sides for the diff view: the old side by the first parent's abbreviated hash, or `empty tree` for a root commit, and the new side by the commit's own abbreviated hash. The layout SHALL NOT reach the application service: which files arrive with their hunks, what crosses IPC, and how a file loads on request SHALL be the same in either layout, and switching between the layouts SHALL read nothing again.

**Reading the commit.** Opening a commit's detail SHALL cost no more than a fixed number of `git` processes and IPC calls, whatever the number $$N$$ of files the commit changed. One `git` invocation SHALL list every changed file with its status, its old and new paths, its old and new modes, and its added/removed counts, and one further invocation, read as a stream, SHALL supply the patch text. Which files arrive with their hunks SHALL be decided by the budgets of the `diff-view` capability: its line budget from the file list, before any patch is read, and its byte limits as the patch streams, which only ever withhold more. Every other file with a patch SHALL arrive withheld, with its counts and without its hunks, and a binary file SHALL be listed without a diff. The application SHALL stop reading the stream after the last file that arrives with its hunks, and SHALL abandon it once 8 MiB of patch text has been read in all, withholding every file not yet reached; a `git` process the application stops deliberately SHALL count as a successful read, not as a failure. Every patch line and every file-list record SHALL be decoded on its own, with bytes that are not valid UTF-8 replaced, so a file in another encoding shows replacement characters in its own lines or path and no other file is affected; paths SHALL be read verbatim from the file list, never C-quoted.

**A truthful file list.**

- Renames SHALL be detected at git's default similarity, the heuristic `git show` and `git log -M` apply: a renamed file is one file, shown as its old path → its new path, with its hunks when its content also changed — never a deletion plus an addition.
- A change of file type, between a regular file, a symbolic link and a submodule, SHALL be one type-changed file, never a deletion plus an addition.
- A root commit SHALL be diffed against the empty tree, so it lists every file it added rather than reporting that it changed no files.
- A merge commit SHALL be diffed against its first parent as a two-tree diff, never as a combined diff, so it lists every file that differs from that parent and not only the files the merge resolved by hand. The view SHALL label that diff `Changes against first parent` followed by the parent's abbreviated hash. The application service SHALL read the first parent as a hexadecimal object id rather than accept it from the caller, and SHALL pass it to `git` after the end-of-options marker, as it does every commit reference (see the *Commit References Are Injection-Safe Arguments* requirement).

The diff SHALL be computed by git's plumbing, which honours git's core diff settings, `diff.renameLimit` among them, but not its display settings: hunks follow git's default algorithm and carry three lines of context even where the user has set `diff.algorithm` or `diff.context`.

**Loading a withheld file.** A withheld file SHALL load on request, alone, from the same commit and against the same base as the rest of its diff — the first parent for a merge, the empty tree for a root commit. The read SHALL pass the file's path, and for a renamed file its old path too, as literal pathspecs, so a path such as `pages/[id].tsx` reads only that file and a renamed file loads as a rename. A file whose diff text exceeds 8 MiB SHALL load as too large to preview, reading `too large to preview`, with no hunks.

**Commands.** `get_commit_detail` SHALL return the commit's files under the budgets, and `get_commit_diff` the one file read on request, with its hunks or as too large to preview, both in the diff model the `diff-view` capability defines. Both SHALL keep their names and existing arguments; `get_commit_diff` SHALL additionally accept the file's old path as an optional argument. Both SHALL be served on the desktop command surface and the web command endpoint alike, and both remain commit-reading operations under the *Commit References Are Injection-Safe Arguments* and *Commit Reading Is Restricted to Registered Repositories* requirements.

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
