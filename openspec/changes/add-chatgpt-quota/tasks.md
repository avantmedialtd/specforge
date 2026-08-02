# Tasks: Add a ChatGPT Quota Status Line

## 1. Settings foundation

- [ ] 1.1 Add `chatgpt_quota_enabled: bool` (default `false`) and `chatgpt_quota_refresh_secs: u64` (default `60`) to `AppSettings` in `crates/openspec-app/src/settings.rs` with `#[serde(default)]` + a default helper; update the `Default` impl (`chatgpt-quota`: *Opt-in quota tracking*)
- [ ] 1.2 Add `chatgpt_quota_enabled()` / `set_chatgpt_quota_enabled()` / `chatgpt_quota_refresh_secs()` to `SettingsStore`, persisting via the existing save path

## 2. Credential resolution

- [ ] 2.1 Create `crates/openspec-app/src/chatgpt_quota.rs` (registered in the crate's `lib.rs`); resolve the Codex home (`CODEX_HOME` env var, else `~/.codex`) and read `auth.json` read-only, extracting the OAuth access token plus an **optional** account id (stored account id first, else the token's `chatgpt_account_id` claim) (`chatgpt-quota`: *Local credential resolution*)
- [ ] 2.2 Reject expired tokens by decoding the JWT payload segment locally (base64url, no signature verification) and comparing `exp` to now; treat an API-key-only or unreadable `auth.json` as "no usable credentials"; never log the token value (`chatgpt-quota`: *Quota privacy and safety*)

## 3. Usage fetch, state, and poller

- [ ] 3.1 Define `ChatGptQuotaState` (reusing `QuotaStatus` from `quota.rs`) with primary/secondary `ChatGptQuotaWindow`s carrying utilization, reset epoch seconds, and the server-reported window length, plus a cheaply-cloneable handle mirroring `QuotaHandle` — and hand-mirror the camelCase types in `src/types.ts` (paired with task 5.3)
- [ ] 3.2 Write the defensive parser: `rate_limit.primary_window` / `secondary_window` → windows (clamp `used_percent` to 0..=100, map `reset_at` epoch seconds, carry `limit_window_seconds`); require at least one window for `Ok`, else "unavailable" (`chatgpt-quota`: *Graceful degradation*)
- [ ] 3.3 Implement the blocking `ureq` GET to `https://chatgpt.com/backend-api/wham/usage` with `Authorization: Bearer`, a SpecForge `User-Agent`, and `ChatGPT-Account-Id` only when an account id resolved; map 401 → unauthenticated, 429 → backoff with `Retry-After`, transport errors → transient (`chatgpt-quota`: *Usage polling with caching and backoff*)
- [ ] 3.4 Port the poll loop from `quota.rs`: 2s tick re-checking the enabled flag, floored refresh interval, single request in flight, stale degradation on transient failure, emit the existing `CacheEvent::QuotaUpdated` only when the snapshot changes (no `openspec-core` changes)

## 4. AppService wiring

- [ ] 4.1 Hold the ChatGPT quota handle on `AppService` in `crates/openspec-app/src/service.rs`, with a `chatgpt_quota()` accessor and `spawn_chatgpt_quota_poller()` mirroring `spawn_quota_poller()`

## 5. Shells

- [ ] 5.1 Add `get_chatgpt_quota` / `get_chatgpt_quota_enabled` / `set_chatgpt_quota_enabled` commands to `crates/specforge/src/commands.rs`, register them in `lib.rs`'s `invoke_handler`, and call `spawn_chatgpt_quota_poller()` beside the existing quota spawn
- [ ] 5.2 Add the three matching dispatch arms to `crates/specforge-web/src/dispatch.rs` and the poller spawn to `crates/specforge-web/src/main.rs` (`chatgpt-quota`: *Quota status-line gauge* — web)
- [ ] 5.3 Add the `api.ts` wrappers (`getChatGptQuota`, `getChatGptQuotaEnabled`, `setChatGptQuotaEnabled`) reusing the existing `quota-updated` listener (no new event names)

## 6. Desktop/web frontend

- [ ] 6.1 Extract the provider-neutral meter row (`WindowRow`, `countdown`, `elapsedFraction`, `fillClass`) from `src/components/QuotaPill.tsx` into a shared module, leaving the Claude pill's behavior unchanged
- [ ] 6.2 Add `ChatGptQuotaPill`: a "ChatGPT" strip rendered beside the Claude pill in the sidebar footer, refetching on `quota-updated`, with unauthenticated copy naming the Codex CLI; derive each window's segment count and axis length from the reported window length (hours ≤ 24h, else days; 5h/7d fallbacks) (`chatgpt-quota`: *Quota status-line gauge*)
- [ ] 6.3 Add a "ChatGPT quota" toggle row to `src/components/SettingsView.tsx` beside the Claude quota row (optimistic toggle → `setChatGptQuotaEnabled`)

## 7. Terminal UI

- [ ] 7.1 Add the ChatGPT snapshot + enabled flag to the TUI `Model` in `crates/specforge-tui/src/app.rs`, initialized from the service and re-read on the quota message; bump `SETTINGS_TOGGLE_COUNT` from `2` to `3` (index 2 currently maps to the Appearance row, so the constant — not a hardcoded index — is what makes room), add the `2 =>` arm to `toggle_focused_setting`, and add the matching row to the `toggles` array in `ui.rs`
- [ ] 7.2 Rework the title-bar gauge in `crates/specforge-tui/src/ui.rs` to assemble an ordered list of provider groups (Claude, then ChatGPT) and drop whole trailing groups until the remainder satisfies the existing `area.width >= gauge_w + 16` guard — today's all-or-nothing guard would make enabling ChatGPT hide the Claude gauge on moderate widths (`chatgpt-quota`: *Quota status-line gauge* — narrow-terminal degradation)
- [ ] 7.3 Render the ChatGPT group itself: provider-prefixed label, terse "ChatGPT: sign in" / "ChatGPT: quota n/a" degraded states, and cells/axis derived from the reported window length with the 5h/7d fallbacks (`chatgpt-quota`: *Quota status-line gauge* — terminal)

## 8. Verification

- [ ] 8.1 Unit-test in `chatgpt_quota.rs`: parser (both windows / one window / none → unavailable / clamping / missing `limit_window_seconds`), JWT expiry rejection, API-key-only auth file → no credentials, missing account id still usable, and stale degradation — runnable via `cargo test -p openspec-app`
- [ ] 8.2 Unit-test the TUI group-degradation helper in `crates/specforge-tui`: both groups fit → both render; width fits only one → Claude renders and ChatGPT is dropped; width fits neither → no gauge
- [ ] 8.3 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`, and `bun run build` all green
- [ ] 8.4 Manual smoke via `bun run wt:dev` walking the spec scenarios: enable → "ChatGPT" strip appears beside Claude's; disable → it hides; both providers side by side; unauthenticated state when no `auth.json`; no network while disabled; TUI title bar shows/toggles the second gauge and narrows gracefully
- [ ] 8.5 Document the opt-in ChatGPT quota feature (credential source, endpoint, read-only guarantee) wherever the Claude quota feature is documented
