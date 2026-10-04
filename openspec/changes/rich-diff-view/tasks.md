## 1. Core: the diff model and its parsers (`openspec-core`)

- [ ] 1.1 Create `crates/openspec-core/src/diff.rs` with design D1's model: `DiffFile`, `FileStatus`, `DiffContent`, `Hunk`, `Line` and `LineKind`. Declare `pub mod diff;` in `crates/openspec-core/src/lib.rs` and re-export the model there, and later the parsers and the budget as they land. Give the structs `#[serde(rename_all = "camelCase")]`. Give `FileStatus` and `DiffContent` `#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]`, as `ArchiveScope` has, because on an enum `rename_all` renames only the variants (`crates/CLAUDE.md`). Serialize `LineKind` as `context`, `added` and `removed`, and omit `Line::no_newline` when it is false (`#[serde(default, skip_serializing_if = …)]`). In the same step:
  - hand-mirror the six types in `src/types.ts`: `FileStatus` and `DiffContent` as unions discriminated on `kind`, and `noNewline?: true`. The step is additive, and `CommitFile` stays until its last reader goes in 11.1;
  - extend `crates/openspec-app/tests/wire_shape.rs` with a `Vec<DiffFile>` fixture that populates every status variant, every content variant and every field, run through `assert_camel_case`;
  - assert each union's discriminant by exact value through `["kind"]`, as `workspace_view_discriminants_match_the_declared_union` does: `added`, `modified`, `deleted`, `renamed`, `copied`, `modeChanged` and `typeChanged`, then `hunks`, `withheld`, `tooLarge` and `binary`;
  - assert each `LineKind` with `assert_wire_value`;
  - assert by identity the keys `src/types.ts` reads (`oldPath`, `newPath`, `oldMode`, `newMode`, `oldStart`, `oldLines`, `newStart`, `newLines`, `section`, `oldNo`, `newNo`, `similarity`), as `github_row_and_snapshot_keys_match_the_declared_mirror` does;
  - assert that `noNewline` is `true` on the flagged line and absent from every unflagged one.

  (`diff-view`: *Diff Model*, scenario *The model crosses the wire with exact discriminants*)
- [ ] 1.2 In `diff.rs`, implement `pub fn parse_hunks(patch: &[u8]) -> Vec<Hunk>` for a header-less per-file patch that starts at its first `@@`; GitHub's `patch` field passes its bytes.
  - Read each `@@ -a[,b] +c[,d] @@ heading` header. An omitted count is 1, and the heading becomes `section`, or none when it is empty.
  - Number every line from the ranges: a context line gets an old and a new number, a removed line an old one, and an added line a new one.
  - Fold each `\ No newline at end of file` into the line before it as `no_newline`, whether that line is removed, added or context. The marker never becomes a line of its own.
  - Decode each line on its own with `String::from_utf8_lossy`. Split on `\n` alone, so a CRLF line keeps its `\r` and a copy stays faithful.
  - Keep the hunk reader shared, because `parse_diff` (1.4) reads its hunks through it.

  (`diff-view`: *Diff Model*)
- [ ] 1.3 Create the test target `crates/openspec-core/tests/diff.rs` and cover `parse_hunks` with fixtures.
  - The spec's scenarios: *Line numbers follow the hunk ranges* (`@@ -10,3 +10,4 @@ fn main()`), *A header-less patch parses to hunks*, *The no-newline marker qualifies the line before it*, *Adding only the final newline flags the old line alone* and *A context line can carry the flag*.
  - Also: markers on both sides of one change block, omitted counts (`@@ -1 +1 @@`), an added file's `@@ -0,0 +1,3 @@`, an empty context line, several hunks, a cut-off hunk that keeps the lines it read, and a one-line change to a very long line.
  - Hold each fixture as a byte-string literal, or as a file whose exact bytes the test pins, so an editor that trims trailing whitespace or re-encodes cannot change it.
- [ ] 1.4 In `diff.rs`, implement `pub fn parse_diff(text: &[u8]) -> Vec<DiffFile>` for a full unified diff, as git writes it and as BitBucket serves it.
  - Split it into sections at `diff --git`.
  - Read the extended headers: `old mode` and `new mode`, `new file mode`, `deleted file mode`, `similarity index`, `rename from` and `rename to`, `copy from` and `copy to`, `index`, and `Binary files … differ`.
  - Read hunks through 1.2's reader. A submodule's `Subproject commit` lines are ordinary hunk lines of a `160000` file.
  - Derive each file's status: added, deleted, renamed or copied with its similarity, mode-changed for a change to the mode alone, and otherwise modified, carrying both modes.
  - Derive its counts from its parsed lines; a binary file has none.
  - Derive its content: `Hunks`, possibly empty, or `Binary`.

  (`diff-view`: *Diff Model*)
- [ ] 1.5 Name each `parse_diff` file by the provider-text rule, never by guessing.
  - First from `rename from` and `rename to`, or `copy from` and `copy to`.
  - Else from the `---` and `+++` lines. Drop the `a/` or `b/` prefix, read `/dev/null` as absent, and drop the tab git appends after a name that contains a space. Unquote git's C-quoted form: its backslash escapes and three-digit octal escapes become bytes, which are then decoded lossily.
  - Only for a section with neither (a mode-only change, an empty added or deleted file, a binary file), from its `diff --git` line, split where its two halves name the same path.

  (`diff-view`: *Diff Model*)
- [ ] 1.6 Fold a type change inside `parse_diff`. It applies to a section deleting a path that is followed directly by one creating the same path, when their `deleted file mode` and `new file mode` differ in file type: regular (`100644`, `100755`), symlink (`120000`) or gitlink (`160000`). The two sections become one `TypeChanged` file carrying both modes, both sections' hunks in order and their summed counts. Two sections of the same file type stay unfolded (`diff-view`: *Diff Model*, scenario *A type change is one file*).
- [ ] 1.7 Cover `parse_diff` in `crates/openspec-core/tests/diff.rs`.
  - The spec's scenarios: *A rename is one file* (92%), *A type change is one file* (`bin/tool` becoming a symlink), *A mode-only change has its own status*, *An edited file whose mode changed is modified*, *A C-quoted path is unquoted* (`café.md`), *A name containing a space keeps its exact path* (`my notes.md`), *An ambiguous diff line is split on its matching halves* (`x b/y`), and *One file in another encoding affects only itself* (a Latin-1 line between two UTF-8 files).
  - Also: a submodule change, a binary marker, an empty added file, a deleted file, a copy and an empty diff.
  - Also: a diff cut mid-section (the earlier files intact, and no panic), many files with long lines, and a type change followed by a modified file, whose section the fold must not swallow.

## 2. Core: the line and byte budgets (`openspec-core`)

- [ ] 2.1 In `diff.rs`, implement the budget decision of design D2 as pure functions over per-file inputs taken in the model's order. Each input is whether the file has a patch, its changed lines and, for the byte limits, its patch text's length. The decision depends neither on git nor on how the text was read, so `openspec-app` can apply it unchanged to a pull request's in-memory detail in `pull-request-viewer`.
  - The limits are `pub const`s and none of them is a setting: 500 changed lines per file, 3,000 eager lines in all, 64 KiB of patch text per file, 1 MiB of eager patch text, 8 MiB read by a streamed read in all, and the 8 MiB per-file ceiling of a read on request.
  - The line rule is decided before any patch text is read. Only patched files take part: a `Binary` or `TooLarge` file keeps its state, adds nothing to the total and is never withheld. Context lines are not counted. A file is eager when its own changed lines are at most 500 and, added to the eager files before it, at most 3,000.
  - The byte limits are applied after the line rule, as one incremental decision that the streamed reader (3.4) and an in-memory caller drive the same way. A file whose own patch text passes 64 KiB is withheld, but its lines stay in the line total. Once the eager files' text reaches 1 MiB, every remaining file is withheld, and a reader stops.
  - A helper turns a model and its decision into the payload: every patched file that is not eager becomes `Withheld`.

  (`diff-view`: *Line and Byte Budgets With On-Request Loading*)
- [ ] 2.2 Cover the budget in `crates/openspec-core/tests/diff.rs`.
  - The spec's scenarios: *Files are eager in order while the total allows*, *Context lines are not counted*, *Binary and too-large files keep their own state*, *A byte limit only shrinks the eager set* and *The eager files' text stops at 1 MiB*.
  - Hand-written boundaries: 500 and 501 changed lines, a running total of exactly 3,000 and of 3,001, exactly 64 KiB of patch text and one byte more, and an eager total reaching exactly 1 MiB. The mutation gate cannot be trusted to mutate a comparator, so these cases are what pins `<=` against `<`.
  - One case driven through an in-memory model, as `openspec-app` will drive it for a pull request.

## 3. Core: reading a commit (`crates/openspec-core/src/git.rs`)

- [ ] 3.1 Add a parents read: one invocation through `git_command`, for example `rev-list --parents -n 1 --end-of-options <sha>`, that returns the commit's parents, each checked with `is_object_id`. The reads below take their base from it. A root commit uses `--root <sha>`, and every other commit the two-tree form `<first parent> <sha>`, always after `--end-of-options`. The parent is read here and never accepted from a caller (`commit-graph`: *Commit Detail View*, *Commit References Are Injection-Safe Arguments*).
- [ ] 3.2 Add the file-list read: one `git diff-tree -r -M --root --no-commit-id -z --raw --numstat --end-of-options <base>` invocation through `git_command`, whose bytes a pure function in `git.rs` parses.
  - All the raw records come first: `:<old mode> <new mode> <old id> <new id> <status>`, then one path, or two for a rename. The numstat records follow in the same order: `<added>\t<deleted>\t<path>`, or an empty path and then the old and new paths for a rename. Pair them by position.
  - Decode each path on its own with `from_utf8_lossy`, verbatim, since `-z` never C-quotes.
  - Read `000000` as an absent mode, and `-` counts as a binary file.
  - Map `A`, `M`, `D`, `T` and `R<score>` to statuses. An `M` whose two object ids are equal changed only its mode, so it is mode-changed.

  (`commit-graph`: *Commit Detail View*)
- [ ] 3.3 Cover the list parser with byte fixtures in `git.rs`'s test module, as the porcelain v2 parser is covered: renames with their similarity, a type change, a mode-only change, an edit with a mode change, a binary file and a submodule; names containing spaces, tabs and newlines; `docs/café.md` read verbatim; and the spec's *A non-UTF-8 path affects only itself*.
- [ ] 3.4 Add the streamed, budgeted patch read, in two parts so its logic is testable without git.
  - A consumer generic over `impl BufRead` walks git's patch output section by section. It pairs each section with its list record in order, after the type-change fold, so a `T` record takes both of its sections. It parses the eager sections with `parse_diff`, keeping their content and leaving their names to the record, and discards withheld sections as it passes them. It applies 2.1's byte limits, and it gives up once it has read 8 MiB in all, withholding every file it has not reached. It returns as soon as the last eager file's section is complete.
  - The process glue spawns `git diff-tree -r -M --root --no-commit-id --patch --end-of-options <base>` through `git_command`, with stdout piped and stdin and stderr null (an unread stderr pipe could stall git). It feeds stdout to the consumer, then drops the pipe, kills the child and waits for it. A child stopped deliberately counts as a successful read, not as the failed exit the `.output()` pattern would report. With no eager file, it spawns nothing.

  (`commit-graph`: *Commit Detail View*; `diff-view`: *Line and Byte Budgets With On-Request Loading*)
- [ ] 3.5 Cover the consumer over in-memory text in `git.rs`'s test module, through a reader that counts the bytes it hands out.
  - It keeps eager sections and discards withheld ones.
  - It stops after the last eager file and reads nothing further.
  - A file passing 64 KiB is withheld mid-stream, and the stream goes on.
  - *The eager files' text stops at 1 MiB* and *A read gives up at 8 MiB*: each withholds every remaining file and reads no further.
  - After a type change, every later file stays paired with its own record.
- [ ] 3.6 Add the one-file read: the same `diff-tree --patch` against the same base, limited by `-- :(literal)<old path> :(literal)<path>`. The old path is passed only when given, and after `--` no path can act as an option. Read it under the 8 MiB ceiling; past it, stop and reap the child and return the file as `TooLarge` with no hunks. Parse it with `parse_diff`, so the type-change fold and the provider-text path rule apply. Return an error when the pathspecs match nothing, as they will for a path that was not valid UTF-8 and was decoded lossily (`commit-graph`: *Commit Detail View*).
- [ ] 3.7 Cover the reads against real repositories in `git.rs`'s test module with `init_repo`, `commit_file` and `git`. Keep the repositories few and small, because every mutant reruns them.
  - A forty-line file renamed with two lines changed is one renamed record with its similarity and its hunks. This replaces `commit_files_shows_rename_as_delete_plus_add`.
  - A regular file replaced by a symlink is one type-changed file, from the list and from the one-file read.
  - A root commit lists its added files.
  - A merge's list holds every file that differs from its first parent, not only the one resolved by hand, and its one-file read diffs against that parent.
  - A Latin-1 line shows replacement characters in its own file only.
  - `pages/[id].tsx` loads alone beside `pages/i.tsx`, and a renamed file loads by both of its paths as one renamed file.
  - With `diff.context = 10` and `diff.algorithm = patience` in the repository's config, hunks keep three lines of context.
  - Each new read stays inert for an option-shaped reference and resolves an abbreviated sha. Port `commit_files_option_shaped_sha_writes_no_file_and_is_inert`, `commit_diff_option_shaped_sha_writes_no_file_and_is_inert` and `commit_diff_and_files_resolve_full_and_abbreviated_sha` for this.
  - A read stopped after its last eager file reports success.

## 4. App: commit detail over the model (`openspec-app`)

- [ ] 4.1 In `crates/openspec-app/src/service.rs`, add the blocking reads behind both commands, leaving the public methods unchanged for now.
  - The detail read takes the parents (3.1), then the file list (3.2), the line rule (2.1) and the streamed read for the eager set (3.4). It assembles each file from its list record (status, paths, modes and counts) and the content the read returned (`Hunks`, `Withheld` or `Binary`).
  - The one-file read takes the parents and then calls 3.6, refusing an empty path.

  (`commit-graph`: *Commit Detail View*)
- [ ] 4.2 Cover them over real repositories in `service.rs`'s test module.
  - A one-file commit and a two-hundred-file commit cost the same number of `git` invocations, and none is made per file. Count them with `openspec_core::git::invocation_log` (`enable`, `mark` and `recorded_since`), filtered by the test's own repository path (`commit-graph`: *Commit Detail View*, scenario *A commit is read in a fixed number of git processes*).
  - A commit whose budgets withhold the files after the last eager one stops reading there and still returns its model, with those files `Withheld` and carrying their counts.
- [ ] 4.3 Switch `AppService::commit_detail` to return `Vec<DiffFile>`, and `AppService::commit_diff` to take `old_path: Option<String>` and return `DiffFile`. Both keep `is_object_id` first and `ensure_registered_repo` second.
  - In the same edit, update the two forwarders so the workspace keeps compiling: the handler signatures in `crates/specforge/src/commands.rs`, and `CommitDiffArg` and its arm in `crates/specforge-web/src/dispatch.rs`.
  - Move `commit_detail_and_diff_refuse_non_object_id_ref` and `commit_reads_require_a_registered_repository` to the new signatures. Both still assert the same refusals.

  (`commit-graph`: *Commit References Are Injection-Safe Arguments*, *Commit Reading Is Restricted to Registered Repositories*)
- [ ] 4.4 Retire what the switch left unused:
  - in `git.rs`, `commit_files`, `diff_tree_lines`, `parse_stat` (if nothing else reads it), the strict `commit_diff` and `CommitFile`, with their six tests;
  - their re-exports in `crates/openspec-core/src/lib.rs`, and their imports in `service.rs` and `commands.rs`;
  - the `CommitFile` fixture in `crates/openspec-app/tests/wire_shape.rs`.

  Then repoint the comment in `task_completion_history` that names `commit_diff` and `diff_tree_lines` as the reads that take a caller's reference.

## 5. Shells: both transports (`specforge`, `specforge-web`)

- [ ] 5.1 Walk the two commands through the four places a command lives (`src/CLAUDE.md`). This change adds no command, and both commands keep their names and existing arguments.
  1. `crates/specforge/src/commands.rs`: `get_commit_detail` returns `Vec<DiffFile>`, and `get_commit_diff` takes `old_path: Option<String>` and returns `DiffFile`. Both only forward to `AppService`, and their doc comments describe the model.
  2. `crates/specforge/src/lib.rs`: both stay in `tauri::generate_handler![…]`.
  3. `crates/specforge-web/src/dispatch.rs`: both arms stay, and `CommitDiffArg` reads an absent `oldPath` as none.
  4. `src/api.ts`: the wrappers move together with their only reader in 11.1, which keeps `tsc` green until then.

  (`commit-graph`: *Commit Detail View*)
- [ ] 5.2 In `dispatch.rs`'s test module, add a routing test beside `list_workspace_file_rows_is_routed_and_parses_the_frontends_json`. Send `get_commit_detail`, and `get_commit_diff` once with `oldPath` and once without, as the literal JSON `src/api.ts` will send, with a valid hex sha and an unregistered repository. Assert the `unregistered repository` refusal, which is reached only once the arm has routed the command and parsed its arguments. Every command this change touches stays on the web transport, so there is no unknown-command pin to add.

## 6. Frontend: the layouts' pure decisions (`src/diffLayout.ts`)

- [ ] 6.1 Create `src/diffLayout.ts`, opening with the reason it exists, as `src/commitHistory.ts` does: there are no component tests, and a `src/`-only diff skips the mutation gate. Add `splitRows(hunk)`.
  - A context line fills both halves of one row.
  - A change block is a maximal run of removed and added lines that no context line interrupts. Walk it with one open slot, the earliest left-only row not yet given a partner: a removed line opens a left-only row, and an added line fills the open slot's right half, or opens a right-only row when no slot is open.
  - Rows refer to the hunk's lines by index, so they can be memoised per hunk.

  (`diff-view`: *Unified and Side-by-Side Layouts*)
- [ ] 6.2 Test `splitRows` in `src/diffLayout.test.ts`.
  - The spec's scenarios: *A git change block pairs by position* (a context line, three removed, two added and a context line), *Interleaved provider text pairs each added line with the open slot* (−a +b −c +d) and *An added line never pairs with a later removed line* (+a −b).
  - The rows of `visual-identity`'s *The filler cell has its own background*: three removed lines and one blank added line.
  - A block of k removed and m added lines taking max(k, m) rows, for several values of k and m.
- [ ] 6.3 Add the one-column decision. A file whose every line is on one side, with no context line at all, renders in one column headed by that side, `old` or `new`; any context line keeps both columns. Test an added file, a deleted file, a file emptied, a file filled from empty, a file whose only hunk adds lines between context lines, and a file that adds and removes with no context, which keeps two columns (`diff-view`: *Unified and Side-by-Side Layouts*, scenarios *An added file uses one column* and *A file with context keeps both columns*).
- [ ] 6.4 Add the width decisions, and export their thresholds of 104 and 96 `ch`.
  - The layout in effect takes the chosen layout, the sections column's width in `ch` of the code font, and the layout in effect until then. Side by side is in effect only while it is chosen, and then when the width is at least 104 ch, or when it was already in effect and the width is at least 96 ch.
  - The navigator folds for side by side while side by side is chosen and the view's width less the navigator's, in the same `ch`, is under 104.
  - Test *Side by side holds between the thresholds* step by step (110, 100, 95, 100, then 104), *The fallback says why and keeps the choice* (80 ch), both sides of each threshold, unified chosen at any width, *Widening never turns side by side off* and *Unified chosen keeps the navigator beside the sections*.

  (`diff-view`: *Narrow Views Fall Back to Unified*)
- [ ] 6.5 Add the stored choice behind an injectable store, following `src/commitHistory.ts`.
  - The key is `specforge.diffLayout`.
  - A read takes exactly `split` as side by side. Anything else is unified: no value, any other value, or a store that throws.
  - A write is best-effort and reports whether it succeeded, so the view can keep a refused choice for itself alone.
  - Test an absent value, `split`, `Split`, `split ` and `side-by-side`, a read that throws, a write that throws, and a round trip.

  (`diff-view`: *The Layout Choice Is Per Surface*)
- [ ] 6.6 Add the selection decisions.
  - Side naming is a pure transition. A pointer-down in a side-by-side code cell names that cell's side. A pointer-down outside a code cell clears it, as does a pointer-up that leaves the selection collapsed or outside the view. The collapsed selection a pointer-down leaves before a drag does not clear it.
  - The clipboard text is built from the model. Side by side, it is the named side's selected lines; in unified, the selected lines in the order shown. It is code text only, joined by newlines, with partial first and last lines honoured.
  - Test *A click ends the named side*, *Copying one side yields that side's code* (from the middle of old line 10 to the middle of old line 12), *Copying in unified yields the lines as shown*, a selection that crosses a hunk boundary (it yields the lines alone), and *Escaped characters copy as themselves* (a zero-width space comes back as itself).

  (`diff-view`: *Selection and Copying*)

## 7. Frontend: hidden characters, highlighting and the file list

- [ ] 7.1 Create `src/hiddenChars.ts`, the escapes every host reuses; `pull-request-viewer` applies them to titles and branch names. It decides which `\p{Default_Ignorable_Code_Point}` characters render as a visible, marked escape, and which escapes raise the file's warning. It decides from the text's own characters and, for a diff line, from the line's own kind and numbers, never from the layout or a facing cell. It returns segments that keep each escaped character's real value and its offset in the source text, for copying.
  - On context lines, paths, side names, titles and branch names, only these are exempt: a U+200D between two emoji; one U+FE0E or U+FE0F directly after an emoji; the tag characters of the three RGI subdivision flags (U+1F3F4, then `gbeng`, `gbsct` or `gbwls`, then U+E007F); and a U+FEFF at the very start of a context line that is old line 1 and new line 1. Every other variation selector or tag character is escaped.
  - On an added or removed line, nothing is exempt. A character escaped only because its line changed raises no warning: one the exemptions would pass on a context line, counting a U+FEFF at the very start of a line that is line 1 of its side.
  - The spec leaves "emoji" undefined. Take it as `\p{Extended_Pictographic}`, so a joiner or selector after an ASCII digit, `#` or `*` (each of them `\p{Emoji}`) stays escaped. On its left, a joiner between two emoji may follow an emoji modifier or one U+FE0F.

  (`diff-view`: *Hidden Characters Are Shown*)
- [ ] 7.2 Test it in `src/hiddenChars.test.ts` with every scenario of *Hidden Characters Are Shown*:
  - a bidirectional control escaped and warned of, and a Hangul filler on a context line;
  - family and heart emoji passing on a context line, and the same emoji escaped without a warning on an added line;
  - a change of only a variation selector, which reads differently, and a second U+FE0F, which is escaped;
  - `gbsct` passing as a flag, while any other tag run escapes every one of its tags;
  - the byte-order mark at line 1 of both sides, on added new line 1, and on a context line at old 1 and new 3;
  - an escaped path and an escaped side name;
  - the same decisions whichever layout asks.

  Add a joiner between two digits, which stays escaped and warns.
- [ ] 7.3 Make `lowlight` a direct dependency in `package.json`, pinned to the version `bun.lock` already resolves for `rehype-highlight` (3.3.0 today). Confirm that `bun.lock` still holds a single `lowlight` entry. From then on, keep the two aligned, as `katex` is kept aligned with `rehype-katex`.
- [ ] 7.4 Create `src/diffHighlight.ts`.
  - Build one `lowlight` instance from `common`, the grammar set `rehype-highlight` uses by default.
  - Resolve a file's language with `registered()`, from its name's extension, else from its whole name (`Makefile`), else none, which renders plain.
  - Highlight each hunk twice, each side as one contiguous text: its old side (context and removed lines) and its new side (context and added lines). Split each result back into per-line token lists; a span that crosses a newline continues on the next line with its classes.
  - Memoise per hunk object in a `WeakMap`, so both layouts read the same token lines and a switch re-tokenises nothing.

  (`diff-view`: *Syntax Highlighting*)
- [ ] 7.5 Test it in `src/diffHighlight.test.ts`: *A line inside a block comment is highlighted as a comment*; *Unified context lines read as the code now reads*, where the context line `x = 1;` between added `/*` and `*/` is code on the old side and a comment on the new; *An extensionless file is highlighted by its name* (`Makefile`); *An unknown language renders plain* (`notes.qqq`); and a second request for a hunk returning the memoised lines.
- [ ] 7.6 Create `src/diffFiles.ts` with:
  - the file key, `newPath ?? oldPath`;
  - the header path, `old → new` for a rename or a copy;
  - the navigator tree by directory, single-child directories compacted into one row and files in section order;
  - the count of files not shown in full, meaning those withheld or too large.

  Test it in `src/diffFiles.test.ts`: *Files are grouped by directory* (`src/components/diff` holding two files beside `README.md`), a deleted file placed by its old path, a renamed file by its new path, and *The view counts the files it does not show in full* (`diff-view`: *File Navigator*, *Line and Byte Budgets With On-Request Loading*).

## 8. Frontend: the layout control (`src/components/ChoiceGroup.tsx`)

- [ ] 8.1 Create `src/components/ChoiceGroup.tsx`, a `role="radiogroup"` of `role="radio"` buttons styled with the existing `.settings-choice-row` and `.settings-choice` classes. It adds the arrow-key contract of the workspace tint palette (`handlePaletteKeyDown` in `src/components/settings/WorkspacesGroup.tsx`).
  - The checked option is the group's single Tab stop.
  - ArrowRight and ArrowDown move focus to the next option and select it, and ArrowLeft and ArrowUp do the same with the previous one, wrapping at either end.
  - It takes text labels, a label for the group and an optional `aria-describedby`.

  Settings' own choice rows keep their markup and keyboard behaviour (`diff-view`: *Keyboard and Accessibility*).
- [ ] 8.2 Export the decision from a key to the next option from `ChoiceGroup.tsx`, and test it in `src/components/ChoiceGroup.test.ts`, as `PullRequestPanel.test.ts` tests its component's pure decisions. Cover both arrows in both directions, wrapping at either end of a two-option and a three-option group, and other keys ignored (*The layout control is one Tab stop with wrapping arrows*).

## 9. Frontend: the `DiffView` component (`src/components/DiffView.tsx`)

- [ ] 9.1 Create `src/components/DiffView.tsx` with the host contract of design D5. Its props:
  - `files: DiffFile[]`;
  - `sideNames: { old: string; new: string }`;
  - `loadFile(file): Promise<DiffFile>`, offered only for a withheld file;
  - the optional slots `renderFileHeaderExtra(file)`, inside the sticky header, and `renderFilePreamble(file)`, between the header and the first hunk and across the section's full width. Neither slot gets a column, and both render the same in either layout.

  Keep the view state per file key and never persist it: collapse, loaded content (dropped when the host passes a new `files` array) and the marked section. Read the stored layout once, when the view mounts. A host that re-renders with new files then keeps the layout, and one that remounts the view for another commit or pull request starts from the stored choice (`diff-view`: *Diff View Hosts*; `diff-view`: *The Layout Choice Is Per Surface*, scenarios *An open view keeps its layout* and *A re-read keeps the open view's layout*).
- [ ] 9.2 Build the toolbar.
  - The `ChoiceGroup` offers "Unified" and "Side by side", always visible with its text labels.
  - A switch applies at once and is stored through 6.5. When the store refuses the write, the view keeps the choice for itself.
  - While the fallback holds, "Too narrow — showing unified" is visible text, and it is the control's `aria-describedby`.
  - The two side names, with 7.1's escapes, show while side by side is in effect, and neither shows in unified.
  - The count of files not shown in full.

  Add no keyboard shortcut, menu item or Settings row (`diff-view`: *The Layout Choice Is Per Surface*, *Narrow Views Fall Back to Unified*, *Diff View Hosts*).
- [ ] 9.3 Measure before the rows paint.
  - In a layout effect, read the sections column's width, and the width of `ch` from a hidden probe set in the code font. Keep both current with one `ResizeObserver`, as `FigureLightbox.tsx` does.
  - Render the rows only once the width is known, so a diff never paints in one layout and then flips to the other.
  - Feed both widths to 6.4's decisions, and set the attribute the stylesheet folds the navigator on while side by side is chosen.

  (`diff-view`: *Narrow Views Fall Back to Unified*, scenarios *No flash of the wrong layout* and *Zoom moves the threshold*)
- [ ] 9.4 Build the navigator from 7.6's tree.
  - Each file row shows its status and counts, and its path with its escapes.
  - It is one Tab stop with a roving current row, as `WorkspaceTree.tsx` keeps one. The arrows move between the visible rows and open and close directories, and Enter or Space on a file activates it exactly as a click does.
  - Activating a file scrolls its section into view and marks it, and scrolling moves the mark to the section being read.
  - It folds into a list above the sections through a container query on the view, never a media query, and through 9.3's attribute.

  (`diff-view`: *File Navigator*)
- [ ] 9.5 Build each file section.
  - The sticky header is the section's own child, above its rows. It shows the path or `old → new`, the status, both modes whenever they differ, the counts, the hidden-character warning, the header-extra slot, and a collapse toggle that is always visible.
  - The preamble slot follows the header, and the content follows it in the layout in effect.
  - A withheld file renders collapsed: its header, and one full-width state row with its counts and "Load diff". The control calls the host's loader and replaces that file's content alone; a failure stays in that row.
  - A too-large file reads "too large to preview". A binary or hunk-less file names its state. None of these offers a control.
  - Every file keeps its navigator row and its section header, whatever its state.

  (`diff-view`: *File Sections*, *Line and Byte Budgets With On-Request Loading*)
- [ ] 9.6 Render unified.
  - One column of rows in the model's order, each tinted by its kind, with its old and new numbers, its marker (`+`, `−`, or blank for context) and its code.
  - Each hunk header, `@@ -a,b +c,d @@` and its heading, is one full-width row.
  - The no-newline flag is a small badge in the line's cell.
  - Lines never wrap. They scroll sideways inside a scroller that holds the lines alone, and the sticky header sits outside it.
  - Tabs render at one width.

  (`diff-view`: *Unified and Side-by-Side Layouts*, *File Sections*)
- [ ] 9.7 Render side by side.
  - Per file, one grid of four columns in two fixed, equal halves (old number, old code, new number, new code), built from 6.1's memoised rows, each cell tinted by its line's kind. Where 6.3 says so, the file is one full-width column headed by its side instead.
  - Fillers carry no number, marker or text, sit on their own background, and are `aria-hidden` and unselectable.
  - Code wraps inside its cell, breaking anywhere, with no number on the continuation, and each row is as tall as its taller cell.
  - Hunk headers and state rows span both halves.
  - The no-newline badge shows in every cell that shows the flagged line, so a flagged context line carries it on both sides.
  - The sticky header sits outside the grid.
  - A withheld file loaded while side by side is in effect renders side by side.

  (`diff-view`: *Unified and Side-by-Side Layouts*)
- [ ] 9.8 Draw tokens and escapes in both layouts from the same computations.
  - Tokens come from 7.4's memoised token lines. Side by side, the left column draws the old side's tokens and the right column the new side's. In unified, a removed line draws the old side's tokens, and an added or context line the new side's.
  - 7.1's escapes are decided from the whole line, so no decision depends on where a token ends, and drawn inside the token spans.
  - The header warns when any line holds a character escaped for itself.

  (`diff-view`: *Syntax Highlighting*, *Hidden Characters Are Shown*)
- [ ] 9.9 Give every rendered line its side-qualified identity in both layouts, as data attributes a later anchor can target: old line n for a removed line, new line n for an added line, and both for a context line.
  - On a switch, record the identity and offset of the topmost visible line in the scrolling ancestor, and restore them in a layout effect before the new layout paints.
  - Collapse, loaded files and the mark persist per file.
  - A switch invokes no command and tokenises no hunk again.

  (`diff-view`: *Side-Qualified Line Identity and Switching*)
- [ ] 9.10 Wire selection and copying.
  - Pointer handlers drive 6.6's side-naming rule and set the named side on the view's root. The stylesheet then turns off selection in the other column of every file's grid.
  - A `copy` handler writes through `clipboardData.setData` inside the copy event, which every origin permits.
  - When both ends of the selection lie in code cells of one file, the handler puts 6.6's model text on the clipboard. It maps DOM offsets back to the model through 7.1's segment offsets.
  - Otherwise it builds document-order text from the selected range, one line per rendered line, skipping line numbers, markers, fillers and badges. While a side is named it also skips the other column's cells, since old WebKit copies text that `user-select: none` only hides. Every escape becomes its real character.

  (`diff-view`: *Selection and Copying*)
- [ ] 9.11 Make both layouts accessible.
  - Each two-column grid is exposed as a table, with visually hidden (`.sr-only`) column headers: old line, old, new line and new.
  - Every line-number cell is named by its side, number and kind ("old line 12, removed"), and its visible digits are hidden from assistive technology.
  - Fillers are hidden.
  - Reading order follows visual order: unified reads line by line, and side by side row by row, the left half first.
  - Unified carries every line's numbers, marker, badge and escapes, and every slot, so it stays fully equivalent for linear reading.

  (`diff-view`: *Keyboard and Accessibility*)

## 10. Frontend: styles and the syntax palette (`src/App.css`)

- [ ] 10.1 Style the diff view in `src/App.css` with design tokens:
  - the toolbar, and the navigator beside the sections or folded above them, through a `container-type: inline-size` query on the view as Settings has, plus 9.3's attribute;
  - the sticky headers, gutters and markers, and the unified scroller;
  - the side-by-side grid with its wrapping cells, the fillers and the badges;
  - the marked escape and the warning;
  - `tab-size`, and the `user-select: none` rule keyed on the named side.

  (`diff-view`: *File Navigator*, *Unified and Side-by-Side Layouts*)
- [ ] 10.2 Add the line backgrounds as tokens on `:root`, with their dark values in the `@media (prefers-color-scheme: dark)` block: the context, added and removed line tints, and the filler's own neutral background. In each scheme, the filler's background differs from all three line backgrounds (`visual-identity`: *Syntax Highlight Palette*, scenario *The filler cell has its own background*).
- [ ] 10.3 Measure every class the palette colours against the context, added and removed backgrounds in both schemes. That covers the literal classes and the token-driven ones (comments at `--text-faint`, titles at `--accent`) alike.
  - Wherever a class falls under 4.5:1, give it a per-scheme, per-background value under the diff view. Leave the tokens themselves unchanged.
  - Record the measurements in a table beside the existing syntax-palette table, which records the code well's.
  - Apply the same selectors to unified lines and side-by-side cells, so the floor holds in both layouts.

  (`visual-identity`: *Syntax Highlight Palette*)

## 11. Frontend: commit detail as the first host

- [ ] 11.1 In `src/api.ts`, the fourth place of 5.1, `getCommitDetail(repoId, sha)` resolves to `DiffFile[]`, and `getCommitDiff(repoId, sha, path, oldPath?)` sends `oldPath` and resolves to `DiffFile`. Remove `CommitFile` from `src/types.ts` and from `api.ts`'s imports.
- [ ] 11.2 Rewrite `src/components/CommitDetailView.tsx` as its header over a `DiffView`.
  - Keep the breadcrumb, subject, meta, parents and trailers exactly as they are. The full-message fix stays out of this change.
  - Read the commit with one `getCommitDetail` call.
  - Render `<DiffView key={commit.id}>`. Its side names are the first parent's abbreviated id, or `empty tree`, and the commit's own abbreviated id.
  - Label a merge's diff `Changes against first parent`, followed by the first parent's abbreviated id.
  - Its loader calls `getCommitDiff` with the file's key path and, for a renamed file, its old path.
  - Say "This commit changed no files." only when the model is empty.
  - Delete `DiffBlock` and `diffLineKind`.

  (`commit-graph`: *Commit Detail View*)
- [ ] 11.3 Delete the rules in `src/App.css` that only the old file list and `DiffBlock` used: `.commit-detail-filelist`, `.commit-detail-fileitem`, `.commit-file-*`, `.stat-add`, `.stat-del`, `.commit-diff*`, `.diff-block*` and `.diff-line*`. Grep `src/` first, to confirm that nothing else reads them.

## 12. Notes

- [ ] 12.1 Record the change where the next reader looks.
  - In `crates/CLAUDE.md`, add a `diff.rs` entry: the model, `parse_diff`, `parse_hunks` and the budgets. Under `git.rs`, note that the commit reads stop and reap their child deliberately.
  - In `src/CLAUDE.md`, add a paragraph: `DiffView` is the one renderer every host uses, through its loader, its side names and its two slots; `specforge.diffLayout` is per-surface view state, never a setting; and pure modules hold its decisions.
- [ ] 12.2 Carry the break for external `/api/invoke` scripts toward the release notes. Mark the change's commit message BREAKING, and name `get_commit_detail` and `get_commit_diff`, their new return shapes and the optional `oldPath`.

## 13. Verification

- [ ] 13.1 Run `bun install && bun run build` once in this worktree, before any `cargo` command. `dist/` is gitignored, and both `generate_context!` and `specforge-web`'s `RustEmbed` need it. A stale `dist/` makes the debug web app serve the pre-change UI.
- [ ] 13.2 Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`; both gate CI. `git.rs` now spawns and kills a child, so also run `cargo check --target x86_64-pc-windows-msvc -p openspec-core --all-targets`, and the same for `x86_64-unknown-linux-musl`, as `crates/CLAUDE.md` advises; no CI job compiles the `cfg(windows)` paths.
- [ ] 13.3 Run `cargo test --workspace`, and get it green before 13.6, because a red test poisons the mutation baseline.
- [ ] 13.4 Run `bun test`. Discovery grows by the new test files (`diffLayout`, `hiddenChars`, `diffHighlight`, `diffFiles` and `ChoiceGroup`). That growth is expected, and it is not the `bunfig.toml` trap.
- [ ] 13.5 Run `bun run build`: strict `tsc` with `noUnusedLocals` and `noUnusedParameters`, then the bundle.
- [ ] 13.6 Run the mutation gate on the change's committed state. The command diffs `HEAD`, so uncommitted work and untracked files are not measured: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`.
  - Kill each survivor in `diff.rs`, `git.rs` and `service.rs` with an assertion, or exclude it in `.cargo/mutants.toml` with a written reason. The likely candidates are 3.4's stop-and-reap glue and any equivalent `o.status.success()` guard.
  - Never pass `--baseline=skip`.
- [ ] 13.7 Build a scratch repository outside the worktree, with an `openspec/` directory so it can be registered. Its history holds:
  - a root commit adding `README.md` and `src/main.rs`;
  - the forty-line `src/old.ts` renamed to `src/new.ts` with two lines changed, and a long `lib/a.rs` renamed to `lib/b.rs` with over 500 lines changed, so that it is withheld while staying similar enough to be detected as a rename;
  - `config` replaced by a symlink, and `run.sh` made executable alone and then again with an edit;
  - a merge whose tree differs from its first parent's in fifty files, two of them resolved by hand;
  - a commit of two hundred files, and one of nine hundred large enough that most of its files are withheld;
  - a file whose diff passes 8 MiB, a binary file, and a submodule pointer change (`git update-index --cacheinfo 160000,…`);
  - `docs/café.md`, then a file with a Latin-1 line, then another UTF-8 file, all in one commit;
  - a path holding U+200B, and a non-UTF-8 path added through `git update-index --cacheinfo`, because the filesystem may refuse the name;
  - `pages/[id].tsx`, with over 500 lines changed, beside `pages/i.tsx`;
  - every no-newline case, a 400-character line paired with a short one, a file emptied and one filled from empty;
  - a removed line inside a block comment, `/*` and `*/` added around the context line `x = 1;`, a `Makefile` and a `notes.qqq`;
  - each case of *Hidden Characters Are Shown*;
  - `diff.context = 10` in the repository's config.

  Register it in an isolated-state instance. If you also register it in the desktop app, whose state the main checkout shares, unregister it when you are done.
- [ ] 13.8 Smoke the desktop app yourself with `bun run wt:dev`; never ask the user to run it. Walk:
  - every scenario of `commit-graph`'s *Commit Detail View* on the scratch repository: the rename, the type change, the root commit and its `empty tree`, the merge with its label and its file loaded against the first parent, the withheld rename loading by both paths, `pages/[id].tsx`, the file past the ceiling, `café` and the Latin-1 line, and another commit opening in the chosen layout;
  - the budgets: the nine-hundred-file commit keeps every navigator row and header and states how many files are not shown in full, and only a withheld file offers "Load diff";
  - the layouts: unified by default; a switch that applies at once and survives a relaunch; side names in the toolbar only while side by side is in effect; the narrow fallback and its message as the side panes are dragged; widening with the navigator folded; long sections keeping their header in view; long lines, fillers, one-column files, badges and full-width rows;
  - selection and copying in the desktop WebView: a drag that stays in its column and carries into the next file, both copy paths, a click that ends the named side, select-all from the keyboard, and escaped characters;
  - the keyboard: the layout control's single Tab stop and wrapping arrows, and the navigator.
- [ ] 13.9 Smoke the browser skin: a debug `specforge-serve` with isolated state, serving the rebuilt `dist/`. Walk:
  - the network log: one `get_commit_detail` for each commit opened, nothing per file, nothing at all on a switch, and `get_commit_diff` carrying `oldPath` on "Load diff";
  - per-surface state: through the desktop app's own served instance, so both surfaces share one settings file, choose unified in a browser tab while the desktop app holds side by side, and the desktop's next diff still opens side by side while the settings file stays unchanged. Also: two tabs, where a switch in one leaves the other's open diff alone; a stored `Split`, `split ` or `side-by-side` reading as unified; a stubbed `localStorage.setItem` that throws; collapse writing nothing to settings or storage; an unchanged URL; and no layout row in Settings;
  - zoom moving the threshold, and, under device emulation with no hover and a phone's width, the control visible with its labels and the fallback with its message;
  - the accessibility tree: the radiogroup and its description, the line-number names, the hidden column headers, the missing fillers, and unified's linear order;
  - the highlighting and hidden-character scenarios, and copying in Chrome.

  Check scroll-marking, the resize-driven fallback and place-keeping across a switch in a visible window, because a hidden automation tab stalls `IntersectionObserver`, `ResizeObserver` and `requestAnimationFrame`. Some scenarios need provider text: the header-less patch, interleaved and added-first pairing, a re-read keeping the layout, and escaped side names. Until `pull-request-viewer` lands, 1.3, 6.2 and 7.2 cover them and 9.1 builds the re-read rule; record them as such rather than skipping them silently.
- [ ] 13.10 Make a throwaway, uncommitted edit that fills both slots from `CommitDetailView`: a mark in the header extra and a note in the preamble. Confirm *Slots render the same in both layouts* and that both slots stay in place across a switch, then revert the edit.
- [ ] 13.11 In both schemes, read the computed colour of every token class on context, added and removed lines, in both layouts, and confirm 10.3's table. Confirm that the filler's background differs from all three line backgrounds, and that the code well's existing table still holds (`visual-identity`: *Syntax Highlight Palette*).
- [ ] 13.12 Measure, as the design's Risks ask, the largest commit and the commit with the most files in this repository, found with `git log --numstat`. Take both layouts, and measure the rendered cells, the time to the diff's first paint and the cost of a switch.
  - Record the numbers in `design.md`'s Risks.
  - If the cost is visible, apply the design's mitigations: highlight a section when it is first expanded, and render sections past the first 1,000 files when the navigator reaches them.
  - If a budget constant has to move, move the spec delta, the constants and the fixtures together.
