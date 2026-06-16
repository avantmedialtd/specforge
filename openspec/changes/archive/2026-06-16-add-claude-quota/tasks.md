## 1. Backend foundation (settings + dependency)

- [x] 1.1 Add `ureq` (blocking HTTP, rustls) to `openspec-app`'s Cargo.toml
- [x] 1.2 Add `claude_quota_enabled: bool` (default `false`) and `claude_quota_refresh_secs: u64` (default `300`) to `AppSettings` with `#[serde(default)]` + a default helper; update the `Default` impl
- [x] 1.3 Add `claude_quota_enabled()`/`set_claude_quota_enabled()` and `claude_quota_refresh_secs()` to `SettingsStore`, persisting via the existing save path

## 2. Credential resolution

- [x] 2.1 Create a `quota` module in `openspec-app`; read the active account's `~/.claude/.credentials.json` (honoring `CLAUDE_CONFIG_DIR`) and parse `claudeAiOauth.accessToken`, checking `expiresAt`
- [x] 2.2 Add a macOS-only Keychain fallback shelling out to `security find-generic-password` read-only, using Claude Code's service-name scheme
- [x] 2.3 Return a typed "no usable token" result (missing/expired) without mutating any store; never log the token value

## 3. Usage poller, state, and event

- [x] 3.1 Define a `ClaudeQuotaState` snapshot (5-hour + weekly utilization, reset times, status: ok/unauthenticated/unavailable/stale) and a defensive parser tolerant of missing windows
- [x] 3.2 Implement the blocking `ureq` GET to the usage endpoint with the bearer token + `anthropic-beta` header; map 401 → unauthenticated, 429 → backoff, parse failure → unavailable
- [x] 3.3 Add response caching + single-in-flight + 429 `Retry-After` backoff, mirroring claude-quota's cache semantics
- [x] 3.4 Add a `CacheEvent::QuotaUpdated` variant in `openspec-core` and emit it after each refresh via `WatcherManager::emit` (no other core changes)

## 4. AppService wiring

- [x] 4.1 Hold the latest `ClaudeQuotaState` on `AppService` (Arc) and add a `claude_quota()` accessor
- [x] 4.2 Add `AppService::spawn_quota_poller()` — a `std::thread` loop (like `spawn_backfill`) honoring the enabled flag + refresh interval; polls, updates state, emits `QuotaUpdated`; issues no requests while disabled
- [x] 4.3 Re-evaluate the poller when the enabled setting toggles (begin on enable, idle on disable)

## 5. Tauri shell (thin)

- [x] 5.1 Add `get_claude_quota` (delegates to `AppService::claude_quota()`) plus `get/set_claude_quota_enabled` commands and register them in the `invoke_handler`
- [x] 5.2 Add a `quota-updated` event + payload in `events.rs` and forward `CacheEvent::QuotaUpdated` through `spawn_event_forwarder`
- [x] 5.3 Add camelCase TS mirrors (quota snapshot type, command + event signatures) to `src/types.ts` and `src/api.ts`

## 6. Desktop frontend

- [x] 6.1 Add a "Claude quota" toggle row to `SettingsView` beside Gamification (optimistic toggle → `set_claude_quota_enabled`)
- [x] 6.2 Build the sidebar-footer quota pill: 5-hour + weekly bars reusing the `task-progress` meter styling, threshold colors, and reset countdown; render only when enabled and a snapshot exists
- [x] 6.3 Subscribe to `quota-updated` (re-fetch `get_claude_quota`) and gate the pill on enabled + snapshot status (hidden / unauthenticated / stale / unavailable)

## 7. Terminal UI

- [x] 7.1 Add a `Msg::Quota(...)` variant and a `quota: Option<ClaudeQuotaState>` field to the TUI `Model`
- [x] 7.2 Drive the quota update from the TUI (subscribe to `QuotaUpdated` and/or the existing 250ms tick) and post `Msg::Quota` back through `tx`
- [x] 7.3 Render a compact 5-hour + weekly gauge in `title_bar` reusing `progress_bar` + the theme color/ASCII ladder; handle disabled/unauthenticated/stale states tersely

## 8. Verification & docs

- [x] 8.1 Unit-test the response parser (ok / missing windows / unparseable → unavailable) and the utilization → color mapping
- [x] 8.2 Manually verify both frontends enabled and disabled: gauge appears/updates/hides, no network while disabled, token absent from logs
- [x] 8.3 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (every new field/helper wired — `specforge-tui` is a binary, so `pub` does not exempt `dead_code`), `cargo test`, and frontend typecheck + build all green
- [x] 8.4 Document the opt-in quota feature and its credential/endpoint behavior in the relevant README(s)
