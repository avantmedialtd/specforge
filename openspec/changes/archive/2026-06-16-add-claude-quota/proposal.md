# Add a Claude Code Quota Status Line

## Why

Developers run SpecForge next to Claude Code all day but have no in-app sense of how much of their Claude usage budget is left — they hit the 5-hour or weekly cap only when Claude Code starts refusing work mid-session. The same OAuth token already on disk can answer "how much room is left?" against Anthropic's usage endpoint, so SpecForge can surface a small, opt-in quota gauge right where the user already watches their work.

## What Changes

- Add an **opt-in** "Claude quota" feature (default **off**). When enabled it periodically reads the local Claude Code OAuth token (read-only), queries Anthropic's usage endpoint, and exposes the current 5-hour and weekly utilization to both frontends.
- New **status-line gauge** in both frontends:
  - **Desktop**: a compact pill in the sidebar footer — 5-hour and weekly utilization bars, green / orange (≥70%) / red (≥90%), switching to a reset countdown when a window is spent.
  - **TUI**: the same gauge appended to the title bar, reusing the existing `progress_bar` + theme/color-ladder primitives.
- New **settings**: `claude_quota_enabled` (bool, default `false`) and `claude_quota_refresh_secs` (default `60`), surfaced as a toggle row in the desktop Settings panel beside Gamification.
- New **background poller** in the headless app layer: a single `std::thread` loop (mirroring `spawn_backfill`, so the app layer stays runtime-agnostic) with response caching and HTTP 429 backoff, publishing a `QuotaUpdated` event on the broadcast channel both frontends already consume. **Active account only** (current `CLAUDE_CONFIG_DIR` / `~/.claude`).
- **Cross-platform credential resolver**: read `~/.claude/.credentials.json` where present, fall back to the macOS Keychain; degrade quietly when no usable token is available.
- First **outbound network** in the project, via a lightweight HTTP client (`ureq`), gated entirely behind the opt-in.

Nothing is **BREAKING** — the feature is purely additive and ships disabled.

## Capabilities

### New Capabilities
- `claude-quota`: resolving the local Claude Code OAuth token, polling Anthropic's usage endpoint on an interval with caching and backoff, and rendering the 5-hour + weekly utilization as an opt-in status-line gauge in both the desktop and terminal frontends — with quiet degradation when disabled, unauthenticated, or offline.

### Modified Capabilities
<!-- None. The gauge lives in app chrome (sidebar footer / title bar), not in the dashboard or spec-browser requirements; settings persistence is implementation detail, not a spec-level capability. -->

## Impact

- **`openspec-app` (net-new module)**: credential resolver, usage poller (HTTP + cache + 429 backoff), a `ClaudeQuotaState` snapshot, `AppService::spawn_quota_poller()`, and a `claude_quota()` accessor. Two new `#[serde(default)]` fields on `AppSettings` (no migration).
- **`openspec-core`**: one new `CacheEvent::QuotaUpdated` variant on the existing broadcast channel. No watchers or parsers added (layering rule preserved).
- **`specforge` (Tauri)**: one thin `get_claude_quota` command, a `quota-updated` event forwarded through the existing forwarder, a sidebar-footer pill, a Settings toggle row, and matching camelCase mirrors in `src/types.ts`.
- **`specforge-tui`**: a `Msg::Quota` variant, a poll spawn, and a title-bar gauge.
- **Dependencies**: adds `ureq` (HTTP, rustls) — the project's first network dependency. Mitigations: opt-in only, official Anthropic endpoint only, token never logged, token accessed read-only (never refreshed or rewritten, so it cannot log the user out).
- **Risk**: the `/api/oauth/usage` endpoint is internal to Claude Code and undocumented; the feature treats its response shape as best-effort and degrades to a quiet "unavailable" rather than failing loudly.
