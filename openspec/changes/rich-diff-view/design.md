## Context

Commit detail (archived as part of `2026-05-29-add-commit-graph-rail`) was deliberately shipped as stage 2 of three: "clicking a commit swaps the center pane to `--stat` + a raw unified diff". Stage 3, "file-tree navigation, per-file collapse, syntax highlighting, +/- gutters, as in the reference screenshot", was recorded as a deferred fast-follow. What exists today:

- **Data.**
  - `commit_files` (`crates/openspec-core/src/git.rs`) runs `git diff-tree --name-status` and `--numstat` for one commit and joins them by path.
  - It does no rename detection, so a rename arrives as a deletion plus an addition.
  - It passes neither `--root` nor a merge option, so a root commit and every merge return no files.
  - `commit_diff` runs `git show --format= <sha> -- <path>` once per file.
  - Both pass the reference after the end-of-options marker, and the service refuses any reference that is not a 4–64 character hex id before calling them (`is_object_id`; `commit-graph`: *Commit References Are Injection-Safe Arguments*).
  - Both are confined to registered repositories (*Commit Reading Is Restricted to Registered Repositories*).
- **Transport.** `CommitDetailView` asks for the file list, then fetches every file's diff in parallel: one IPC call and one `git` process per file, with no size cap. The workspace-file browser caps its reads at 5 MiB (`MAX_WORKSPACE_FILE_BYTES`, "file is too large to preview"), but diffs have no equivalent.
- **Rendering.** `DiffBlock` splits the raw text on newlines and colours each line by its prefix (hunk, file header, added, removed), one DOM node per line, with no gutters, highlighting, collapse or navigation.

The read-only pull-request viewer (change `pull-request-viewer`) needs to render a pull request's changed files. Its diffs arrive as provider text:
- BitBucket sends a full unified diff.
- GitHub sends per-file `patch` fields that start at `@@`, with no file headers, while status, paths and counts arrive as JSON.

Both are parsed in `openspec-app`, which can hand a frontend a model but not a component. Rendering commit detail and pull requests from one model is what keeps the two from drifting.

`highlight.js` already ships in the bundle, reached through `rehype-highlight` 7 → `lowlight` 3, and `App.css` styles its `hljs-*` classes for fenced code. Those colours are guaranteed 4.5:1 only against the code well (`visual-identity`: *Syntax Highlight Palette*).

## Goals / Non-Goals

**Goals:**

- One diff model, produced in Rust from either git's output or provider text, and rendered by one component for commit detail now and the pull-request viewer next.
- Bounded work per commit: a fixed number of `git` processes and IPC calls whatever the file count, and a rendering budget that keeps the page responsive on a large commit.
- A truthful file list: renames, type changes, root commits and merges.
- Highlighting that matches markdown code blocks and keeps the contrast floor on tinted lines.
- Author-controlled text that cannot hide what it contains.
- Pure, separately tested parser and budget logic, so the mutation gate on `openspec-core` has assertions to catch.

**Non-Goals:**

- A split view, word-level emphasis, an ignore-whitespace toggle, virtualised rendering, or expanding a hunk's context to the full file.
- Making commit detail addressable, or any change to the graph or rail.
- The full-message fix in the commit header.
- Any terminal surface.

## Decisions

### D1. Parse in `openspec-core`; the frontend renders a model

```rust
pub struct DiffFile {
    pub old_path: Option<String>,   // None when added
    pub new_path: Option<String>,   // None when deleted
    pub status: FileStatus,
    pub additions: Option<u32>,     // None when the source gives no count
    pub deletions: Option<u32>,
    pub content: DiffContent,
}
pub enum FileStatus { Added, Modified, Deleted, Renamed { similarity: Option<u8> },
                      Copied { similarity: Option<u8> }, ModeChanged, TypeChanged }
pub enum DiffContent { Hunks { hunks: Vec<Hunk> }, // empty: no textual change
                       Withheld,                   // held back by the budget; loadable
                       TooLarge,                   // past a ceiling, or a provider withheld it for size
                       Binary }
pub struct Hunk { pub old_start: u32, pub old_lines: u32, pub new_start: u32, pub new_lines: u32,
                  pub section: Option<String>, pub lines: Vec<Line> }
pub struct Line { pub kind: LineKind,   // Context | Added | Removed | NoNewline
                  pub old_no: Option<u32>, pub new_no: Option<u32>, pub text: String }
```

`diff.rs` exposes two pure functions plus the budget decision of D2:
- `parse_diff(text) -> Vec<DiffFile>` reads git's and BitBucket's full unified diffs;
- `parse_hunks(patch) -> Vec<Hunk>` reads a header-less per-file patch such as GitHub's `patch` field. The caller builds that file's `DiffFile` from the provider's own status, paths and counts.

The parser understands:
- git's extended headers (`rename from/to`, `similarity index`, `new/deleted file mode`, `old/new mode`, `Binary files … differ`, `Subproject commit`);
- the `\ No newline at end of file` marker;
- git's C-quoted paths.

It assigns line numbers from the hunk ranges.

Paths are never guessed from ambiguous headers. `-z` does not apply to patch output, git C-quotes unusual names, and it leaves spaces unquoted in the `diff --git` line. So:
- **For a commit**, each patch section is paired with its `-z` file-list record (D2) by git's output order, and paths come from that record. A type change, which git prints as a deletion section and then a creation section for one path, folds into one `TypeChanged` file.
- **For provider text**, paths come from `rename from/to`, else from `---`/`+++`, dropping the tab git appends after a name that contains a space.
- **Only for a section with neither** (a mode-only change, an empty added or deleted file, a binary file) is the `diff --git` line read, split where its two halves name the same path.

On the wire:
- Structs cross IPC with `rename_all = "camelCase"`.
- `FileStatus` and `DiffContent` are tagged unions, `#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]`, as `ArchiveScope` is.
- `LineKind` is a camelCase string enum.
- `wire_shape.rs` populates every variant. It asserts each `LineKind` value with `assert_wire_value` (`noNewline`, …), and each union's discriminant by exact value through `["kind"]` (`modeChanged`, `typeChanged`, `tooLarge`, …), as `workspace_view_discriminants_match_the_declared_union` does.

*Rejected — keep parsing in TypeScript, as `DiffBlock` does by line prefix.* The frontend is a pure consumer by convention, and parsers belong where `cargo test` and the mutation gate reach them. The pull-request viewer also parses provider diffs in `openspec-app`, which cannot call a TypeScript parser.

*Rejected — compute diffs with a library (`similar`, or `git2`'s diff).* SpecForge would then disagree with the user's own `git` about renames and hunks. `git2` would also add a native dependency, and it would bypass two things: `git_command`, the chokepoint that carries WSL routing and the test invocation log; and the call sites that place every reference after `--end-of-options`.

### D2. A fixed number of `git` processes per commit, and a line budget

```mermaid
sequenceDiagram
  participant V as CommitDetailView
  participant S as AppService
  participant G as git
  V->>S: get_commit_detail(repoId, sha)
  S->>G: diff-tree --raw --numstat -z (status, renames, modes, counts)
  S->>S: budget decides the eager files from the list
  S->>G: diff-tree --patch, streamed
  S->>G: stop after the last eager file (close, kill, wait)
  S-->>V: model: eager files with hunks, the rest withheld
  V->>S: get_commit_diff(repoId, sha, path, oldPath?) on "Load diff"
  S->>G: diff-tree --patch for that one file
  S-->>V: that file's hunks, or too large to preview
```

The file list comes from one invocation, `git diff-tree -r -M --root --no-commit-id -z --raw --numstat`, which replaces today's two. Its records arrive in the same order:
- the raw records give each file's status letter (A, M, D, T, or R with its similarity) and its old and new modes;
- the numstat records give the counts.

The budget is a pure function in `diff.rs` over that list. With $$c(f)$$ the added plus removed lines of file $$f$$, taken in git's output order, a file arrives with its hunks exactly when:

$$\text{eager}(f) \iff \neg\,\text{binary}(f) \;\wedge\; c(f) \le 500 \;\wedge\; c(f) + \sum_{g \prec f,\ \text{eager}(g)} c(g) \le 3000$$

Context lines are not counted, so a page renders at most about seven lines per counted line. The measurement under Risks is taken on rendered lines.

The 3,000-line total is SpecForge's own constant for page responsiveness, to be confirmed by that measurement. GitHub's own one-call diff stops much later, at 20,000 lines, 1 MB or 300 files. The 500-line single-file threshold sits close to GitHub's own automatic load of 400 lines or 20 KB per file. Both are constants, not settings. `openspec-app` applies the same decision to a pull request's detail in `pull-request-viewer`.

The patch comes from one invocation, `git diff-tree -r -M --root --no-commit-id --patch`, read as a stream. Because the eager set is known from the list before the patch is read:
- the service keeps eager files' hunks;
- it discards the patches of withheld files it passes over;
- it gives up, withholding every remaining file, once 8 MiB of patch text has been read;
- after the last eager file it closes the pipe, kills the child and waits for it.

A child it stopped deliberately counts as a success, not as the failed exit the existing `.output()` pattern would see. A withheld file never crosses IPC, although git may still diff one that precedes an eager file.

A withheld file renders collapsed, with its counts and a "Load diff" control. That control fetches the file alone through `get_commit_diff`, under a per-file ceiling of 8 MiB of diff text. Past the ceiling the file's content is `TooLarge` and reads "too large to preview", the wording the file browser already uses.

*Rejected — keep one IPC call and one `git` process per file.* An $$N$$-file commit costs $$N + 2$$ processes and $$N + 1$$ round trips, and there is no single place to apply a budget.

*Rejected — virtualised rendering instead of a budget.* Windowing breaks find-in-page, sticky headers and text selection across lines, and scroll-marking would need measured heights. A budget bounds the page without any of that, and the reader opens a withheld file in one click.

### D3. Renames, type changes, root commits and merges

- **Renames.** `-M` at git's default similarity, the same heuristic `git show` and `git log -M` use. A renamed file shows `old → new` and, when its content also changed, its hunks.
- **Type changes.** One `TypeChanged` file (D1), never a deletion plus an addition.
- **Root commits.** `--root`, so a root commit is diffed against the empty tree.
- **Merges.** A merge is diffed against its first parent as a two-tree diff, `--end-of-options <parent> <merge>`. The parent is read by the service as a hex id rather than accepted from the caller, and the view labels it "Changes against first parent `abc1234`".

*Rejected — a combined diff (`--cc`) for merges.* It shows only the paths the merge resolved by hand, so a merge that brought in fifty files would read as "changed two files".

*Rejected — leave merges and root commits empty.* "This commit changed no files" is false for both.

### D4. Highlight each hunk side with `lowlight`, and keep the contrast floor

Each hunk is highlighted twice:
- its old side, the context and removed lines, as one contiguous text;
- its new side, the context and added lines, as another.

The resulting token tree is split back into lines, so a comment or string that spans lines inside a hunk is highlighted correctly.

The language comes from the file's extension, or from its whole name for an extensionless file such as `Makefile`. It is resolved with `registered()` on a `lowlight` instance built from the same `common` grammar set `rehype-highlight` uses by default, and an unknown language renders plain. `lowlight` becomes a direct dependency pinned to the version `rehype-highlight` resolves.

Highlighting runs in the frontend, since it is presentation, which keeps the model text-only and the IPC payload small.

Once tokens own the text colour, added and removed lines are told apart by a background tint and their kept `+`/`−` markers. The palette's 4.5:1 floor, defined today only against the code well, is extended to the added, removed and context backgrounds in both schemes (the `visual-identity` delta). Where one literal cannot clear every background, it is defined per scheme or per line background.

*Rejected — tokenise each line on its own.* Every multi-line construct breaks: for example, a removed line inside a block comment is highlighted as code.

*Rejected — tokenise whole files.* That needs both full revisions of every file, which means more `git` reads and, for provider diffs, more API calls. It is a natural follow-up once full-file context exists.

*Rejected — Shiki.* TextMate grammars and a WASM engine are heavier and asynchronous, and diffs would stop matching markdown code blocks.

### D5. One `DiffView` component with two per-file slots

`DiffView` renders a navigator and the file sections from `DiffFile[]`.
- It accepts a loader, offered only for `Withheld` files. A `TooLarge` or `Binary` file shows its state and no control.
- Two optional slots carry everything a host adds:
  - `renderFileHeaderExtra(file)` sits inside the sticky header. The pull-request viewer puts its "viewed" mark and "changed since viewed" flag there.
  - `renderFilePreamble(file)` sits between the header and the first hunk. The viewer puts its review threads there.
- The navigator is a tree by directory, compacting single-child directories. It is keyboard-operable like the workspace tree and shows each file's status and counts.
- Activating a file scrolls its section into view, and the section being read is marked as the reader scrolls.
- Section collapse is view state held by the component and never persisted.
- Below the width at which the navigator fits beside the sections, it folds into a list above them. A container query on the view decides, as Settings does, never a media query.

`CommitDetailView` keeps only its header and passes the model through.

*Rejected — a second renderer for pull requests.* Two renderers drift. The two slots are the only differences the viewer needs.

### D6. Hidden characters are shown, not rendered

`DiffView` renders these characters in lines and paths as visible, marked escapes:
- bidirectional controls: U+202A–U+202E, U+2066–U+2069, U+200E, U+200F and U+061C;
- zero-width and format characters: U+200B–U+200D, U+2060 and U+FEFF.

A U+200D inside an emoji sequence and a U+FEFF at the very start of a file are exempt. A file containing any escaped character carries a warning in its header, as GitHub's diff does. The rule belongs to `diff-view`, so commit detail and pull requests share it, and the pull-request viewer reuses the same escapes for titles and branch names.

*Rejected — render them raw.* Code would read differently from what it compiles to (Trojan Source, CVE-2021-42574), and a path could pass for another.

*Rejected — strip them silently.* The view would hide what the file actually contains.

### D7. Same command names and arguments, new shapes, both transports

- `get_commit_detail(repoId, sha)` returns the budgeted `Vec<DiffFile>`.
- `get_commit_diff(repoId, sha, path, oldPath?)` returns one `DiffFile`, with `Hunks` or `TooLarge` content. The service passes `path` and `oldPath` as literal pathspecs (`:(literal)<path>`), so a file named `pages/[id].tsx` reads only itself, and the old path keeps rename detection intact.

The service keeps the hex-id check, and every `git` call site keeps the end-of-options marker. Both commands keep their dispatch arms on the web transport, and the break for external `/api/invoke` scripts is called out in the proposal.

*Rejected — new command names beside the old ones.* That leaves two shapes for one view. The bundled frontend moves with the change, which is the trade `get_my_pull_requests` → `get_bitbucket_pull_requests` already accepted.

## Risks / Trade-offs

- [The budget hides content a reader expected to see] → Every file stays in the navigator with its counts, a withheld file is one click from its diff, and the header states how many files are not shown in full.
- [Highlighting a commit at the budget blocks the main thread] → The budget bounds the work. Measure rendered lines on the largest commit in this repository and, if the cost is visible, highlight a section when it is first expanded rather than all at once.
- [A withheld file early in git's order is still diffed and piped] → The service discards it as it streams, and the 8 MiB read ceiling bounds the worst case.
- [Rename detection is slow on a commit that touches thousands of files] → git's own `diff.renameLimit` bounds it, and the user's git configuration applies as it does on the command line.
- [First-parent diffs surprise on a "merge master into feature" commit] → The label names the parent. A parent switcher is a follow-up if it proves wanted.
- [Token colours lose contrast on tinted lines] → The `visual-identity` delta makes the floor normative on every diff background in both schemes, and the existing contrast measurement extends to them.
- [`lowlight` drifts from the version `rehype-highlight` resolves] → Pin it to the same version and keep them aligned, as `katex` is kept aligned with `rehype-katex`.
- [Scripts calling the two commands over `/api/invoke` break] → **BREAKING** in the proposal and the release notes; the bundled frontend is updated in the same change.

## Open Questions

- Should word-level emphasis be computed in `openspec-core`, where it is testable, or in the frontend? The leaning is core, in its own change.
- Should a reader's collapsed sections survive switching commits? The proposal is no: collapse is view state.
