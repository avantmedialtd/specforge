# Tasks

## 1. Config (`openspec-app`)

- [x] 1.1 Extend `WebServerConfig` with a `tailscale` block: `TailscaleConfig { enabled: bool (default false), name: Option<String> (manual override), allowed_logins: Vec<String> (default empty) }`, all `#[serde(default)]` so existing settings files load unchanged
- [x] 1.2 `SettingsStore` accessors/setters: `web_config()` already returns the block; add `set_web_tailscale_enabled`, `set_web_tailscale_name`, `set_web_tailscale_allowed_logins`
- [x] 1.3 Export the new type; keep `src/types.ts` mirror in sync

## 2. Tailscale name discovery (`specforge-web`)

- [x] 2.1 `tailscale::resolve_name() -> Option<String>`: shell `tailscale status --json`, parse `.Self.DNSName`, strip the trailing dot; return `None` if Tailscale is absent/stopped or MagicDNS is off (no panic, no error surfaced to the request path)
- [x] 2.2 Resolution precedence: manual `config.tailscale.name` (override) → discovered name → `None` (fail closed)
- [x] 2.3 Unit-test the trailing-dot strip and the `None` fallbacks (feed a sample `status --json` blob)

## 3. Trust-boundary generalization (`specforge-web`)

- [x] 3.1 `AppState` carries the resolved allowlist: `allowed_authorities: Vec<String>` (always includes the loopback set; plus the Tailscale name when enabled+resolved) and `allowed_logins: Vec<String>`
- [x] 3.2 Replace `is_loopback_authority` with `is_allowed_authority(value, &allowed)`; apply to both `Host` and `Origin`. Never wildcard
- [x] 3.3 Build the allowlist in `router()`/`serve()` from `config.tailscale` + discovery (resolve once at startup)
- [x] 3.4 Identity gate: when the matched authority is the (non-loopback) Tailscale name AND `allowed_logins` is non-empty, require `Tailscale-User-Login ∈ allowed_logins`; loopback requests never require a login. Document the "trustworthy only because we bind loopback" invariant at the check site

## 4. Tests (`specforge-web`)

- [x] 4.1 Tailscale disabled → a request with a tailnet `Host`/`Origin` is `403` (default behaviour unchanged)
- [x] 4.2 Tailscale enabled with name `m.tailnet.ts.net` → a request with that `Host`+`Origin` passes; a loopback request still passes
- [x] 4.3 Cross-origin (`Origin: https://evil.com`) is still `403` with Tailscale enabled
- [x] 4.4 Another `.ts.net` name (not the configured one) is `403`
- [x] 4.5 `allowed_logins` set: request with an allowed `Tailscale-User-Login` passes; with a non-listed login `403`; loopback request (no login header) still passes

## 5. Frontend (`src/`)

- [x] 5.1 Commands + `api.ts` wrappers for the new Tailscale config (get is covered by `get_web_config`; add `set_web_tailscale_enabled` / `set_web_tailscale_allowed_logins` and an optional name override)
- [x] 5.2 Settings → Web UI: a Tailscale subsection (desktop-only) — enable toggle, the **resolved tailnet name** shown read-only (so a stale/missing name is diagnosable), and an optional logins allow-list field
- [x] 5.3 Extend the existing "reach it from another device" hint with the `tailscale serve --bg <port>` one-liner (now that direct serve is supported), alongside the existing SSH-tunnel hints

## 6. Spec + verification

- [x] 6.1 Sync the delta into `openspec/specs/web-ui/spec.md` (MODIFIED *Localhost Trust Boundary*, ADDED *Tailscale Serve Access*)
- [x] 6.2 `cargo test` + `cargo fmt --check` + `cargo clippy -- -D warnings` + `bun run build` green
- [ ] 6.3 Manual verify on a real tailnet (requires a tailnet + a second device — not runnable in CI/headless): enable, run `tailscale serve --bg <port>`, load `https://<machine>.tailnet.ts.net` from another tailnet device, confirm the dashboard renders and **live-updates over SSE**, and that an empty vs populated `allowed_logins` gates access as specified. (The guard/login behaviour it would exercise is covered by the automated tests in §4; what remains genuinely manual is the real `tailscale serve` transport + SSE-over-serve.)
