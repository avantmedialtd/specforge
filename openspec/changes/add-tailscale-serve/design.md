# Design

## Context

`specforge-web` binds `127.0.0.1` and runs a `loopback_guard` middleware that
rejects any request whose `Host` or `Origin` is not in `{localhost, 127.0.0.1,
::1}`. That guard does two jobs that are fused today but pull apart the moment
you leave localhost:

1. **Reachability** — who can open a socket (controlled by the bind address).
2. **Browser-origin defense** — which web page may drive the API (the `Origin`
   allowlist, anti-CSRF; the `Host` allowlist, anti-DNS-rebinding).

Exposing the UI to a tailnet must relax **reachability only**. The origin defense
must stay strict, or a malicious page in a tailnet device's browser could
`fetch()` the tailnet URL.

The mechanism is `tailscale serve`: `tailscaled` reverse-proxies an incoming
tailnet HTTPS request to a local target, terminating TLS with a MagicDNS cert.
Because `tailscaled` connects *from* localhost, the app needs no bind change —
the request already reaches `127.0.0.1`. The following behaviours were verified
against Tailscale's source/docs and shape the design:

- **`Host` is preserved.** For a loopback TCP backend, `serve` forwards
  `r.Out.Host = r.In.Host`, so the backend sees `machine.tailnet.ts.net`, not
  `127.0.0.1`. It also adds `X-Forwarded-For` (caller's tailnet IP) and
  `X-Forwarded-Proto: https`. → the allowlist must include the tailnet name on
  **both** `Host` and `Origin`.
- **Identity headers are injected and spoof-stripped.** `serve` sets
  `Tailscale-User-Login` / `Tailscale-User-Name` (user-owned devices only; not
  Funnel) and removes any client-supplied copies before forwarding. They are
  trustworthy **iff the backend is reachable only via `serve`** — i.e. bound to
  loopback. We keep the loopback bind, so they are trustworthy here.
- **SSE streams without buffering.** `serve` uses an unconfigured Go
  `ReverseProxy`, which flushes `text/event-stream` immediately. `/api/events`
  works live over Tailscale with no change. (WebSocket through `serve` has known
  bugs; we use SSE, so this is moot — and a retroactive vindication of that
  earlier choice.)
- **`serve` is tailnet-only; Funnel is public.** `serve` never faces the
  internet and is governed by tailnet ACLs; Funnel exposes publicly and carries
  no identity. We target `serve` exclusively.

## Goals

- Make `tailscale serve` reach the web UI with the smallest safe change.
- Keep the `127.0.0.1` bind and the strict cross-origin/DNS-rebind defense.
- Off by default; explicit opt-in.
- Optional per-user authorization that is safe on a *shared* tailnet.
- No new transport, no TLS in the app, no second event channel.

## Non-Goals

- Binding `0.0.0.0` or any non-loopback interface directly.
- Tailscale Funnel (public exposure).
- Embedding `tsnet` or a standalone token-auth scheme (possible later, separate).
- Mutating-command authorization changes — the read-only/no-mutation parity of
  the web UI is unchanged.

## Decisions

### 1. Keep the loopback bind; trust the tailnet name in the guard

The app does not bind a non-loopback interface. `tailscale serve` (external)
provides reachability; the app only learns to *accept* the proxied request.

- **Alternative — bind the Tailscale IP / `0.0.0.0`:** rejected. It enlarges the
  socket-level surface and, crucially, **breaks the identity-header trust
  guarantee** (which holds only when the service is reachable solely via
  `serve`). Keeping loopback is both smaller and strictly safer.

### 2. Allowlist the node's own name on Host AND Origin — never a wildcard

`is_loopback_authority` generalizes to `is_allowed_authority(value)` = `value ∈
{loopback} ∪ {configured/discovered tailnet names}`. Applied to both `Host`
(anti-rebind; `serve` preserves it) and `Origin` (anti-CSRF; the browser always
stamps it). A page on `evil.com` still sends `Origin: https://evil.com` → 403.

- **Alternative — accept any `*.ts.net`:** rejected. Trusts other tailnets'
  machines and is a broader rebind target. Trust the *specific* name only.

### 3. Discover the name from local Tailscale state, with a manual override

On enable, shell `tailscale status --json` and read `.Self.DNSName` — an FQDN
*with a trailing dot* (`machine.tailnet.ts.net.`), which we strip. Fall back to a
user-supplied name if Tailscale isn't installed/running or MagicDNS is off
(then `.Self.TailscaleIPs` or manual entry). Discovery is best-effort; a missing
name simply means no tailnet authority is trusted (fails closed).

- **Alternative — manual config only:** more friction; most users would have to
  copy/paste their MagicDNS name. Keep manual as the fallback, not the default.
- **Alternative — query the LocalAPI socket directly (no shelling out):** cleaner
  long-term but adds a dependency/parsing surface; shelling `tailscale status
  --json` matches how `git` is already invoked and is enough for v1.

### 4. Optional per-user authorization via the identity header

A `allowed_logins: Vec<String>` (default empty). When non-empty, a request whose
`Host` is the tailnet name is allowed only if it carries `Tailscale-User-Login ∈
allowed_logins`. When empty, the tailnet itself is the trust boundary (personal
tailnet). Loopback requests never require a login (desktop / SSH-tunnel use).

The header is trustworthy because the app binds loopback and `serve` strips
client copies — the only forger is a local process, which is already inside the
loopback trust domain. This is documented as the load-bearing invariant.

- **Alternative — a bearer token in the opened URL:** complementary, not
  conflicting; deferred. The identity header is free here and ties access to
  Tailscale's own auth.

### 5. No transport or TLS change

Relative URLs (`/api/invoke`, `/api/events`) proxy through `serve` unchanged;
TLS is terminated by Tailscale; SSE streams (verified). Nothing in the frontend
or the dispatch/SSE layers changes.

### 6. Off by default; `serve`, not Funnel

A new `web.tailscale` config block, disabled by default. Enabling only widens the
guard's allowlist — it does not start, configure, or require `tailscale serve`
(the user runs that themselves). Funnel is explicitly unsupported.

## Risks

- **Stale/incorrect discovered name** → the guard would 403 legitimate `serve`
  traffic. Mitigation: manual override; surface the resolved name in settings;
  fail closed (no name trusted) rather than open.
- **MagicDNS / HTTPS not enabled in the tailnet** → `serve` can't get a cert.
  This is a user-side prerequisite (the `serve` CLI prompts to enable it);
  documented, not an app concern.
- **Local process forging `Host` + identity header on a direct loopback hit** →
  no privilege escalation: a local process is already trusted as "you" under the
  loopback model. The guarantee rests on "only `serve` and local-you can reach
  `127.0.0.1`," which the loopback bind preserves.
- **Tailscale version/serve-config drift** → discovery parses `.Self.DNSName`
  (a stable documented FQDN invariant), not the less-stable serve-config shape.
- **Scope creep toward `0.0.0.0`** → kept explicitly out; the safety argument
  depends on the loopback bind, so the two must not be conflated.
