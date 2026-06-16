## Context

SpecForge is a gamified OpenSpec browser with a layered Rust backend (`openspec-core` → `openspec-app` → { `specforge` Tauri shell, `specforge-tui` }) and is, today, entirely local and offline. This change ports the approach of the `claude-quota` SwiftBar plugin — read the Claude Code OAuth token locally and query Anthropic's `/api/oauth/usage` endpoint for 5-hour and weekly utilization — into SpecForge as an opt-in feature surfaced as a status-line gauge in both frontends.

Constraints that shape the design:
- The Tauri crate must stay thin — no watchers or parsers (that logic lives in core/app).
- `openspec-app` is consumed by two different async runtimes (Tauri's and the TUI's), so it must not assume a runtime of its own.
- The workspace has no HTTP dependency today, and the desktop app has no global status bar.

## Goals / Non-Goals

**Goals:**
- A single shared poller in `openspec-app` that feeds both frontends through the existing `CacheEvent` broadcast channel.
- Opt-in and default-off: no token reads and no network until the user enables it.
- Cross-platform credential resolution — credentials file first, macOS Keychain fallback.
- A faithful, utilitarian gauge: 5-hour + weekly utilization, threshold colors, reset countdown.

**Non-Goals:**
- Multi-account gauges (the source's per-account bars) — active account only.
- Per-model windows, extra-usage credits, and the source's other dropdown detail — out of scope for v1.
- Managing the OAuth token in any way — strictly read-only.
- Native secret-store integration beyond the credentials file off macOS.

## Decisions

**1. Poller runs on a `std::thread` in `openspec-app`, not a tokio task.**
`openspec-app` is consumed by both runtimes; spawning tokio tasks from the app layer would couple it to one. `spawn_backfill` already uses a `std::thread` for exactly this reason. A blocking HTTP client on a dedicated thread mirrors that pattern and keeps the layer runtime-agnostic. *Alternative — async `reqwest` on a tokio task:* rejected for the runtime coupling and the heavier dependency.

**2. HTTP via `ureq` (blocking, rustls), not `reqwest`.**
`ureq` is small, blocking (fits the `std::thread` poller), and avoids dragging hyper/tokio-tls into a previously network-free build — which matters for the Tauri smoke build and binary size. The call is a single authenticated `GET` returning JSON. *Alternative — `reqwest`:* more transitive deps and an async surface we don't need.

**3. Publish via the existing `CacheEvent` broadcast channel (a new `QuotaUpdated` variant).**
Both frontends already subscribe to `CacheEvent`; the Tauri forwarder and the TUI `select!` loop each gain one arm. No new transport, no second state store. *Alternative — a dedicated channel/store:* rejected as redundant.

**4. Credential resolver: file-first, macOS Keychain fallback.**
Claude Code writes `~/.claude/.credentials.json` on Linux/Windows and uses the Keychain on macOS. Reading the file first covers the most platforms with the least platform-specific code; the macOS fallback matches the source's behavior on the dev platform. The macOS Keychain read shells out to `security` read-only, avoiding a `security-framework` dependency. *Alternative — the `keyring` crate:* rejected because it abstracts native keystores but won't read Claude Code's plaintext file on Linux, which is the common case there.

**5. Desktop placement: a sidebar-footer pill.**
The desktop app has no global status bar; the sidebar footer is the only always-visible chrome and matches the existing Archive/Settings strip and the `task-progress` meter styling. *Alternatives:* the Dashboard hero (visible on only one screen) and the titlebar drag region (conflicts with window dragging) — both rejected.

**6. Settings: two `#[serde(default)]` fields on `AppSettings`.**
This matches the existing gamification toggle exactly — read once, optimistic toggle, persist — and optional defaults mean existing `settings.json` files load with no migration.

## Risks / Trade-offs

- [Undocumented endpoint] `/api/oauth/usage` is internal to Claude Code and may change. → Parse defensively in one isolated module; degrade to "unavailable" rather than crashing; a shape change becomes a one-file fix.
- [First outbound network in a local-first tool] Some users value SpecForge's offline nature. → Default off; no token read or request until explicitly enabled; talk only to the official endpoint.
- [Token handling] Mishandling the OAuth token could log the user out or leak it. → Read-only access (never write/refresh/delete); never log the token; transmit only to the official endpoint.
- [Cross-platform credential drift] Claude Code's storage can differ by version/OS. → File-first covers the common case; macOS Keychain fallback; a quiet unauthenticated state with a "sign in with Claude Code" prompt when nothing resolves.
- [Stale data] Cached snapshots can lag real usage by up to the refresh interval. → Show a reset countdown when a window is spent; de-emphasize stale snapshots; the default 5-minute interval matches the source.

## Migration Plan

Additive and opt-in; no data migration. The new settings fields default (off / 300s) via serde defaults, so existing `settings.json` files load unchanged. Rollback: disabling the setting fully dormants the feature; removing the code removes the gauge with no residual state beyond two ignored settings fields.

## Open Questions

- Exact field names for the weekly/per-model windows across plan tiers — to be confirmed against a live usage response during implementation; the parser must tolerate missing windows.
- In the unauthenticated/unavailable states, whether the TUI shows a terse one-word marker or omits the gauge entirely (leaning: a terse marker).
- Whether the refresh interval needs a UI control in v1 or can remain a `settings.json` value with a sane default (leaning: `settings.json` only for v1).
