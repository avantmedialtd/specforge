# Tasks

## 1. Date-tolerant archive detection

- [x] 1.1 Add a small helper that strips a leading `YYYY-MM-DD-` prefix from an archive directory name when (and only when) those leading characters form a valid date, returning the bare logical id; non-dated names pass through unchanged (watcher-local, or shared in `parser.rs` next to `list_archived_changes`)
- [x] 1.2 In `crates/openspec-core/src/watcher.rs` `handle_events`, replace the per-id `archive_path.is_dir()` check (line 511) with a set built once per batch: read `openspec/changes/archive/`, map each entry through the strip helper, and classify a removed `<id>` as archived iff it is a member
- [x] 1.3 Keep the existing downstream behaviour on a positive match — emit `CacheEvent::ChangeArchived` and record the `ChangeArchived` achievement; keep a removal with no matching archive entry silent (deletion)

## 2. Tests

- [x] 2.1 Add an `openspec-core` watcher test: archiving an active change into `openspec/changes/archive/<YYYY-MM-DD>-<id>/` produces a `ChangeArchived` event for `<id>` and records the archival achievement
- [x] 2.2 Add a regression test: removing an active change with no archive directory produces no `ChangeArchived` event and no archival achievement (deletion stays silent)
- [x] 2.3 Add a guard test for the matcher: an unrelated archive entry like `2026-06-04-x-foo` does not match id `foo`

## 3. Verify

- [x] 3.1 `cargo test -p openspec-core` (watcher + activity_log) passes
- [x] 3.2 `openspec validate detect-dated-archive-dirs --strict` passes
