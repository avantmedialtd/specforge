## 1. Web: caching and borrowed bytes (`crates/specforge-web`)

- [x] 1.1 In `crates/specforge-web/src/assets.rs`, add `body_of(Cow<'static, [u8]>) -> Body`: `Borrowed` becomes `Bytes::from_static`, `Owned` becomes `Bytes::from(vec)`. Replace the three `content.data.into_owned()` calls in `static_handler` with it (design Decision 2).
- [x] 1.2 In `assets.rs`, add a pure `if_none_match_matches(header, etag) -> bool` with weak comparison (comma-separated lists, `*`, `W/` on either side) and a weak-ETag formatter over `EmbeddedFile::metadata.sha256_hash()`. Unit-test matching, non-matching, list, wildcard and malformed headers in the module's `tests` (`web-ui`: *HTTP Caching of the Served Bundle*).
- [x] 1.3 Make `static_handler` take the request's `HeaderMap`. Apply the policy table from design Decision 1 based on the path that was served. Serve `assets/*` hits as `public, max-age=31536000, immutable` with an `ETag`. Serve the shell (root, `/index.html`, SPA fallback) and the stable root files (`WELL_KNOWN_ASSETS`, `is_root_icon`) as `no-cache` with an `ETag`. Answer a matching `If-None-Match` with a bodiless `304`. Set `Vary: Accept-Encoding` on the `200` and the `304` of every compressible type, so the two agree (design Decision 1). Send namespace-miss 404s and the build hint as `no-store` (`web-ui`: *HTTP Caching of the Served Bundle*).
- [x] 1.4 Change `shell_response` to take the status, an optional body and the ETag, so the `200` and the `304` both get `Content-Security-Policy: frame-ancestors 'none'`, `X-Frame-Options: DENY` and `X-DNS-Prefetch-Control: off` from one place (`web-ui`: *Localhost Trust Boundary*). Update the module doc comment.
- [x] 1.5 In `crates/specforge-web/tests/server.rs`, add integration tests for these cases:
  - an `assets/` hit carries the immutable policy and its own content type;
  - the root, `/index.html` and a deep address each carry `no-cache` and an `ETag`;
  - `favicon.svg`, a declared `icon-*.png` and `manifest.webmanifest` carry `no-cache`, not `immutable`;
  - a namespace miss is `no-store`;
  - a matching `If-None-Match` on the shell returns `304` with an empty body, the same `ETag` and `Vary: Accept-Encoding` exactly once;
  - a stale tag returns `200` with the full shell.
- [x] 1.6 Extend `assert_the_shell_carries_its_headers` so it also sends a matching `If-None-Match` and asserts the three shell headers on the `304`. It runs under the loopback bind, the network bind and Tailscale support (`the_shell_carries_its_headers_*`).

## 2. Web: response compression (`crates/specforge-web`)

- [x] 2.1 Add `tower-http = { version = "0.6", features = ["compression-br", "compression-gzip"] }` to `crates/specforge-web/Cargo.toml`, with a comment giving the reason. Confirm that `Cargo.lock` resolves it to the 0.6.x already present, and review the new transitive crates the two features add (`async-compression` and its codecs). Record the release `specforge-serve` binary size before and after in the PR.
- [x] 2.2 In `crates/specforge-web/src/lib.rs` `router_with_bind`, add a `CompressionLayer` outside the `authority_guard` layer, using `DefaultPredicate` and an explicit Brotli/gzip level (design Decision 3). Comment that the predicate is what keeps `/api/events` unbuffered (`web-ui`: *Compressed Responses Except the Event Stream*).
- [x] 2.3 Add tests in `tests/server.rs`:
  - an `assets/*.js` request with `Accept-Encoding: br` gets `Content-Encoding: br` and `Vary: Accept-Encoding`, and the decoded body is byte-identical to the file in `dist/`;
  - the same request with no `Accept-Encoding` has no `Content-Encoding`;
  - `/api/invoke` `get_workspace_views` (or another result above 32 bytes) with `Accept-Encoding: gzip` decodes to the same JSON as the uncompressed response;
  - an unknown command with `Accept-Encoding: gzip` still decodes to the `{ "error": ... }` envelope with status 422.
- [x] 2.4 Add a test that opens `/api/events` with `Accept-Encoding: br, gzip` and asserts there is no `Content-Encoding` and the content type is `text/event-stream`. Add a test that a `.png` icon request with `Accept-Encoding: gzip` has no `Content-Encoding`.
- [x] 2.5 Add any decompression helper the tests need (for example `flate2`/`brotli`, or `tower-http`'s `decompression-*` features) under `[dev-dependencies]` only.

## 3. TUI: coalesced watcher refresh (`crates/specforge-tui`)

- [x] 3.1 In `crates/specforge-tui/src/main.rs`, extract `drain_cache(first, &mut rx) -> (CacheBurst, closed)` (design Decision 4). It classifies the event that woke the loop, drains `try_recv()` until `Empty` (bounded, so a flood still returns to `select!`), treats `Lagged` as `refresh`, maps `QuotaUpdated` to `quota`, ignores the two PR-snapshot variants, and reports `Closed`.
- [x] 3.2 Rewrite the `select!` cache branch to call `drain_cache` and apply at most one `Msg::Cache` and one `Msg::Quota` per loop turn. Guard the branch on `!cache_closed` so a closed channel cannot spin (`terminal-ui`: *Watcher Bursts Coalesce Into One Refresh*).
- [x] 3.3 Unit-test `drain_cache` against a real `tokio::sync::broadcast` channel, with no timing involved:
  - five mixed events yield one `refresh`;
  - a PR-only burst yields neither flag;
  - `QuotaUpdated` plus `Updated` yields both flags;
  - overflowing a small-capacity channel yields `refresh` through `Lagged`;
  - dropping the sender reports `closed`.
- [x] 3.4 In `render_tests.rs` (or `app.rs` tests), assert that one `Msg::Cache` after several on-disk edits re-reads the tree, the Settings mirror and the open artifact's body exactly as before. The existing *Live Updates From the Watcher* tests (scroll clamp, kept body on failure, `pending_trigger` upgrade) must stay green unchanged.

## 4. TUI: single-flight, generation-checked graph reads (`crates/specforge-tui/src/app.rs`)

- [x] 4.1 Change `Msg::Graph` to `Graph { gen: u64, result: Result<Box<CommitGraph>, String> }`. Make `reload_graph` always send a reply, `Err` included.
- [x] 4.2 Add `graph_gen`, `graph_in_flight` and `graph_stale` to `Model`. Add a `Model::set_graph` that replaces `graph`, bumps the rail render revision and clamps `graph_selected`. Split reloads into user-initiated (`load_graph`, the `m` key: always bump the generation and spawn, clear `graph_stale`) and watcher-initiated (`apply_cache_refresh`: spawn only when idle, otherwise set `graph_stale`) (design Decision 5; `terminal-ui`: *Commit Graph Reloads Are Single-Flight and Current*).
- [x] 4.3 In `update`'s `Msg::Graph` arm, discard replies whose `gen` is not current. On a current reply, clear `graph_in_flight`; apply `Ok` through `set_graph` and keep the graph on `Err`. If `graph_stale` is set, clear it and, when History is still open with a `graph_repo`, issue one more watcher reload.
- [x] 4.4 Unit-test this by driving `update` with synthetic `Msg::Graph` replies, deterministically:
  - a reply with an old generation after a repository switch is ignored;
  - a smaller-window reply after `m` is ignored;
  - three watcher refreshes while a read is in flight leave exactly one more read pending after the reply (assert on `graph_gen` and `graph_in_flight`);
  - an `Err` reply clears `graph_in_flight` and keeps the previous graph.

## 5. TUI: reuse rendering across frames (`crates/specforge-tui`)

- [x] 5.1 Add a `RefCell<Option<MarkdownCache>>` to `Model`, keyed on `(detail_md, active scheme)`. The other `Theme` fields are fixed for the process (design Decision 6). In `crates/specforge-tui/src/ui.rs` `detail`, re-render through `markdown::render` only when the key differs (design Decision 6; `terminal-ui`: *Rendering Work Is Reused Across Frames*, *Artifact Markdown Rendering*).
- [x] 5.2 In `crates/specforge-tui/src/graph.rs`, split `commit_rail` into `rail_lines(graph)`, which builds unselected lines from edges indexed by `band`, plus a draw-time highlight. Keep `commit_rail(graph, selected)` as their composition. Stop allocating a `String` per cell.
- [x] 5.3 Add a `RefCell<Option<RailCache>>` to `Model`, keyed on `(graph render revision, active scheme)`. In `ui::history`, clone only the visible window of the cached lines and apply `REVERSED` to the selected row.
- [x] 5.4 Add tests:
  - rendering the detail pane twice with an unchanged body reuses the cache (assert the cache key and line count or pointer stay the same), and changing the body or the scheme invalidates it. Test scheme invalidation through a cache helper that takes the scheme as an argument, not by calling the global `theme::set_scheme`: tests run in parallel and share that global, so mutating it can flake other render tests;
  - a `TestBackend` buffer drawn through the cached History path equals one drawn from `commit_rail(graph, selected)` for every selected row of a multi-lane fixture;
  - a fixture with several hundred commits and many edges, including diagonals that cross the same cells in one band, renders the same rail as the pre-change algorithm (keep the old function as a test oracle, or snapshot its output). Compare the rendered cells and colours, not the span count, because merging same-colour runs changes the spans.
- [x] 5.5 In `main.rs`, draw only when `needs_draw` is set (after any applied `Msg`, including `Resize` and `Tick`). Change the tick to 1 s with `MissedTickBehavior::Delay` (design Decision 7). Update any comment that refers to the 250 ms cadence.

## 6. Verification

- [x] 6.1 `cargo fmt --all -- --check`
- [x] 6.2 `cargo clippy --workspace --all-targets -- -D warnings` (any suppression uses `#[expect(..., reason = "…")]`).
- [x] 6.3 `bun install && bun run build` (the web tests need `dist/`), then `cargo test`, including `cargo test -p specforge-web` and `cargo test -p specforge-tui`.
- [x] 6.4 `bun run lint` and `bun run build` (no `src/` changes are expected; this confirms that).
- [x] 6.5 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Only the ungated frontend crates change, so this is expected to short-circuit. Green here means "not run", not "covered"; the coverage comes from the tests in groups 1 to 5.
- [ ] 6.6 Web smoke test (the implementer runs it): start a debug `specforge-serve` against isolated state and load it in Chrome with devtools Network open.
  - *Not run in the autonomous drain: needs an interactive browser. A curl smoke against a debug `specforge-serve` with isolated state was run instead (encodings, `304` with the shell headers, an unencoded SSE stream, a rebuilt `dist/` picked up without a restart).*
  - The first load shows `br` or `gzip` encodings on the JS chunks.
  - A reload shows the shell revalidated (`304`) and the `assets/` chunks served from the memory or disk cache.
  - `/api/events` stays uncompressed and keeps delivering events.
  - After `bun run build` the next reload picks up the new bundle without a server restart.
- [ ] 6.7 TUI smoke test (the implementer runs it): run `specforge-tui` against a registered repository.
  - *Not run in the autonomous drain: needs an interactive terminal. An idle run in a pty with isolated state was done instead (about 57 ms of CPU over 9 s, a clean quit on `q`).*
  - Edit a change's `tasks.md` several times quickly while the artifact is open: the body updates and the scroll position holds.
  - Open History, press `m`, and switch repositories quickly: the shown graph always matches the selected repository and window.
  - Switch colour scheme in Settings: the detail pane and the rail recolour immediately.
  - While idle, CPU stays near zero (check with `top`).
