## Context

`commit_log` in `crates/openspec-core/src/git.rs` runs one `git log --all --date-order -n <limit>` with a unit-separator (`\x1f`) field format and **newline-delimited records**, parsed by `for line in raw.lines()`. The format comment explicitly notes this is safe only because `%s` (subject) and `%D` (decorations) never contain newlines. Git *trailers* live in the commit body and are inherently multi-line, so they cannot be appended to the current format without breaking record splitting.

Trailers are also not naïvely parseable: git defines them as the `Key: value` lines of the message's **last paragraph**, with folding (continuation lines) and possibly repeated keys. An "any line with a colon" scan would misfire — e.g. the `Bump GitHub Actions…` commit has a bulleted body and a blank line before its `Co-Authored-By` trailer. Git's own `%(trailers)` pretty-format placeholder applies the real parser.

The commit graph is fetched on demand (`get_commit_graph` → `commit_log` → `layout`); nothing is cached. Trailers therefore need no watcher, cache, or event work — they are additive fields on the existing payload.

## Goals / Non-Goals

**Goals:**

- Capture each commit's git trailers using git's own parser, robustly, in a single `git log` pass.
- Carry them additively through `RawCommit` → `LaidOutCommit` → IPC → `src/types.ts`.
- Render them as a neutral key/value list in the commit-detail view.

**Non-Goals:**

- Any commit↔OpenSpec-change linking, navigation, or filtering (explicitly deferred; would collide with the rail's "no OpenSpec semantics" rule).
- Row chips in the rail — the 26px row stays exactly as it is.
- Parsing `Co-Authored-By` into name/email, deduping authors, or rendering avatars.
- Showing the non-trailer body prose of the message; only the recognized trailer block is surfaced.

## Decisions

**1. Use git's trailer parser via `%(trailers)`, never a hand-rolled scan.** Append `%(trailers:only,unfold,key_value_separator=%x1d,separator=%x1e)` as the final field of the `commit_log` format. `only` drops non-trailer lines from the last paragraph; `unfold` flattens folded continuations; the explicit separators make the field unambiguously machine-splittable. _Alternative — split the raw body on `\n` and match `^\w[\w-]*:`:_ rejected; it duplicates git's last-paragraph + folding logic and misfires on prose containing colons.

**2. Switch this `git log` to NUL-delimited records (`-z`).** The trailers field can contain bytes the current newline-record split would choke on. `-z` makes records NUL-separated; parsing changes from `raw.lines()` to `raw.split('\0')` (dropping the trailing empty chunk). Fields stay `\x1f`-separated. _Alternative — keep newline records and strip newlines from the trailers field:_ rejected as fragile; NUL records are the canonical git answer for "a field may contain anything."

**3. A three-level separator scheme, all C0 control bytes.** Records `\0`, fields `\x1f` (unchanged), trailers within the trailers field `\x1e`, key/value within one trailer `\x1d`. Each separator is a distinct control byte that cannot appear in a hash, author name, ISO date, subject, decoration, or trailer text — so splitting is unambiguous at every level and no escaping is needed. Parse: split the record on `\x1f`; split the trailers field on `\x1e`; split each trailer once on `\x1d` into `{ key, value }`, both trimmed.

**4. `Trailer { key, value }` as a structured, ordered pair.** A `Vec<Trailer>` (not a map) preserves git's emitted order and supports repeated keys (two `Co-Authored-By`). Mirrored to `src/types.ts` by hand as `interface Trailer { key: string; value: string }`; `#[serde(rename_all = "camelCase")]` on the Rust side (the fields are single words, so the wire form is `key`/`value`).

**5. Neutral rendering, in the detail view only.** The view lists trailers as `key`→`value` rows below the parents block. The `OpenSpec-Id` trailer is rendered identically to every other trailer — no link, tint, or marker — keeping the feature consistent with the standing "the graph SHALL carry no OpenSpec semantics" rule even though the detail view is technically outside that requirement's scope. A commit with zero trailers renders no section.

## Risks / Trade-offs

- **The `-z` switch touches working code.** `commit_log`'s record loop changes. → Mitigated by the existing `commit_log` tests (history order, decorations, merge parents, empty-outside-repo) plus new trailer tests; all must stay green.
- **`key_value_separator` requires a reasonably modern git.** The option is long-established; this app already depends on a capable `git` on PATH and `commit_log` returns `Vec::new()` on any error, so an ancient or missing git fails safe (empty graph) rather than corrupting output.
- **Long trailer values** (`Co-Authored-By: … <noreply@anthropic.com>`). The detail-view header is roomy, but a value can be long. → Rendered in a wrapping/truncating value cell with the full value available (e.g. `title`); no layout breakage.
- **Body prose is intentionally dropped.** Only git-recognized trailers are shown, not the full message body. → Accepted; the requirement's "full message" intent is satisfied for the structured trailer block, and a full-body renderer is a separate concern.
