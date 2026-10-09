## 1. Skip rule (openspec-app)

- [x] 1.1 Add `globset` to `crates/openspec-app/Cargo.toml` (and `regex` as a direct dependency; it is already in `Cargo.lock` through other crates).
- [x] 1.2 Create `crates/openspec-app/src/review_skip.rs`, with no default list (remove `DEFAULT_PATTERNS`):
  - `SkipRule::compile(&[String]) -> (SkipRule, Vec<PatternError>)`, where a `/…/` pattern of at least three characters is a regex, a glob containing `/` matches the whole path, and a glob without `/` matches the basename, using `literal_separator(true)`;
  - `SkipRule::first_match(&[&str]) -> Option<&str>`, returning the pattern as written;
  - `validate(&[String]) -> Result<Vec<String>, Vec<PatternError>>`, which trims and applies the 64-pattern cap, the 1–256-byte cap and the no-leading-`/` rule for globs.

  Register the module in `lib.rs`. (`pull-request-viewer`: *Review Skip Patterns*)
- [x] 1.3 Unit-test `review_skip.rs` table-driven at every boundary:
  - basename vs. whole path;
  - `*` not crossing `/`;
  - `**` matching zero segments;
  - case sensitivity;
  - regex detection at 2 and 3 characters;
  - first match in list order;
  - 64 and 65 patterns, 256 and 257 bytes;
  - whitespace trimming;
  - a bad pattern skipped by `compile` but refused by `validate`;
  - the spec's matching scenarios verbatim.
- [x] 1.4 Add `review_skip_patterns: Vec<String>` (with `#[serde(default)]`, absent meaning empty) to `AppSettings` in `crates/openspec-app/src/settings.rs`, plus `SettingsStore::review_skip_patterns()` (the stored list and its stored compile errors) and `set_review_skip_patterns(Vec<String>)`, which validates before writing and returns the refused patterns. Test: a file without the field reads as the empty list and skips nothing, an empty list is stored, and a refused list leaves the file untouched. (`pull-request-viewer`: *Review Skip Patterns*)
- [x] 1.5 Add `EVENT_REVIEW_SKIP_PATTERNS_CHANGED = "review-skip-patterns-changed"` and its payload type (the stored list only, no `defaults` flag) to `crates/openspec-app/src/events.rs`, with the name-literal test the other events have, and make sure it is not a cache event. Mirror the name and payload in `src/types.ts`.

## 2. Skipped state and inclusion (openspec-app)

- [x] 2.1 In `crates/openspec-app/src/review_progress.rs`:
  - add `included` (a set of paths, skipped on serialisation while empty, read as empty when absent) to the stored entry;
  - add `FileReviewState::Skipped`;
  - add `matched: Option<String>` and `included: bool` to `FileReviewProgress`, and `skipped: usize` to `ReviewProgress`;
  - mirror all of these in `src/types.ts`. (`pull-request-viewer`: *Review Progress*)
- [x] 2.2 Pass the compiled `SkipRule` into `progress()`, and apply the precedence viewed → changed since viewed → partly viewed → skipped → unviewed, matching each file by its new and old paths. Count `skipped` and keep `viewed` and `changed_since_viewed` as before. Test each precedence step, including an older entry's key outranking a pattern and a push leaving a skipped file skipped.
- [x] 2.3 Add an inclusion write (`include_write`) and make `file_write` and `hunk_write` add the path to `included` when the rule matches the file, never creating an entry on an unmark or an exclusion and never advancing `lastMarkedHead` on inclusion. Test each, including "mark then unmark leaves the file unviewed and included". (`pull-request-viewer`: *Review Progress*)
- [x] 2.4 In `crates/openspec-app/src/service.rs`:
  - add `AppService::set_file_included(reference, path, included, head, base)` through `write_review_marks`, so it shares that function's refusals and raises `review-progress-changed`;
  - compile the rule from `SettingsStore` in `review_progress` and in every mark write;
  - add `AppService::set_review_skip_patterns`, which stores the list and raises `review-skip-patterns-changed` with the stored list (emitted directly by the setting command on both transports, per the spec's *Notifying*).

  Add service tests for the refusals (no cached detail, foreign path, head mismatch, provider disabled) and for both notices.
- [x] 2.5 Extend `crates/openspec-app/tests/wire_shape.rs` to pin `matched`, `included`, `skipped`, the `skipped` state value, the stored `included` key and the new event's payload.

## 3. Transports (specforge, specforge-web)

- [x] 3.1 `crates/specforge/src/commands.rs` + `lib.rs`: add `get_review_skip_patterns`, `set_review_skip_patterns` (taking a list, never null) and `set_file_included` to `generate_handler!`, thin over `AppService`/`SettingsStore`. Emit `review-skip-patterns-changed` to every window from the setting command.
- [x] 3.2 `crates/specforge-web/src/dispatch.rs`: add arms for the same three commands, and carry the new event on `sse.rs`'s stream. Add dispatch tests: `set_file_included` routes and refuses as `set_file_viewed` does, `set_review_skip_patterns` refuses a bad list and emits on a good one, and `get_review_skip_patterns` answers the empty list on a fresh settings file.
- [x] 3.3 `src/api.ts`: add wrappers for the three commands and a listener for `review-skip-patterns-changed` (`listenLogged`), and cover the wrappers in `src/api.test.ts`.

## 4. Pull-request view (src/)

- [x] 4.1 `src/pullRequestView.ts`:
  - add a pure `newlySkipped(previous, next)` helper, compared by path and ignoring the detail;
  - add the header-count helper (`n of m files viewed · s skipped`, where $$m = \text{total} - s$$);
  - test both in `src/pullRequestView.test.ts`. (`pull-request-viewer`: *Changed Files in the Pull-Request View*)
- [x] 4.2 `src/components/PullRequestView.tsx`:
  - a skipped file's header extra shows the unchecked box, "skipped · matches `<pattern>`" and **Review**;
  - an included matching file shows "matches `<pattern>`" and **Skip**;
  - the view asks the diff view to collapse each newly skipped file once;
  - it re-reads progress on `review-skip-patterns-changed`;
  - the header shows the skipped count.

  Style in `src/App.css` alongside the existing viewed-mark styles.

## 5. Settings (src/)

- [x] 5.1 Add the "Skip in review" settings row to `src/components/settings/IntegrationsGroup.tsx`, after the pull-request cards and only while a pull-request integration is on:
  - one `CommittedField` per pattern, with a remove control, then an empty add field, whose placeholder suggests `**/tests/**`; while the list is empty, the add field alone;
  - no "Restore defaults" control;
  - a refusal reported on the field that caused it, and a stored compile error reported on its field;
  - live update on `review-skip-patterns-changed`.

  Extract any pure list-editing logic into a tested helper next to `src/settingsFields.ts`. (`pull-request-viewer`: *Review Skip Patterns*; `settings-view`: *Settings Are Organised Into Groups*)

## 6. Verification

- [x] 6.1 `bun install && bun run build` (fresh worktree; also refreshes `dist/`), then `cargo test`, `cargo fmt --check` and `cargo clippy --workspace -- -D warnings`.
- [x] 6.2 `bun test` for the frontend helpers.
- [x] 6.3 Mutation-test the diff: `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Add an assertion or a reasoned exclusion for any survivor in `review_skip.rs`, `review_progress.rs`, `settings.rs` or `service.rs`.
- [x] 6.4 Run the app yourself (`specforge-serve` debug build + `bun run dev`, or `bun run wt:dev` for the window case) against a real pull request that changes test files. Walk the scenarios:
  - a fresh settings file shows the row with only the suggesting add field, and no file is skipped;
  - a skipped file opens collapsed and names its pattern;
  - Review includes it without expanding;
  - Skip collapses it again;
  - an expanded skipped section survives a re-read;
  - marking then unmarking a test file leaves it unviewed;
  - the header reads "n of m · s skipped";
  - adding `docs/**` in Settings updates an open pull-request window;
  - a bad regex is reported on its field;
  - removing every pattern skips nothing again;
  - the row is absent while both integrations are off.
