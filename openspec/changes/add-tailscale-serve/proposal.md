# Minimal Tailscale Serve Support

## Why

The local web UI (`specforge-web`) binds `127.0.0.1` only, and its trust-boundary
guard rejects any request whose `Host`/`Origin` is not loopback. That makes it
unreachable from another device — which is the whole point of wanting it on a
phone or second laptop.

`tailscale serve` is the natural fit: it runs a reverse proxy inside the local
`tailscaled`, terminates HTTPS with a real MagicDNS cert, and forwards tailnet
requests to a local port — so the app can stay bound to loopback and let
Tailscale carry the encryption, device authentication, and even per-user
identity. The **only** thing blocking it today is the guard: `tailscale serve`
preserves the original `Host` header (verified in `ipn/ipnlocal/serve.go`), so a
proxied request arrives with `Host`/`Origin` = `machine.tailnet.ts.net` and is
refused with a 403.

This change opens exactly that one path, safely: trust the node's **own** tailnet
name (and nothing else) in the guard, keeping the loopback bind and the strict
cross-origin protection intact. It deliberately does **not** open raw `0.0.0.0`
binding or Tailscale Funnel — those have a different, weaker safety story and are
out of scope.

Two verified facts make this both small and safe:

- **SSE streams cleanly through `serve`** (Go's `ReverseProxy` flushes
  `text/event-stream` immediately), so the existing `/api/events` live-update
  stream works over Tailscale with no transport change.
- **`serve` injects a trustworthy `Tailscale-User-Login` header** (it strips any
  client-supplied copy first) that is safe to trust *only* when the backend is
  reachable solely via `serve` — i.e. bound to loopback, which we keep. So
  optional per-user authorization is a cheap header check, not a project, and it
  makes a *shared* tailnet safe, not just a personal one.

## What Changes

- The web server's trust boundary becomes **configurable** instead of
  loopback-only: it accepts loopback always, plus — when Tailscale support is
  enabled — the node's own tailnet (MagicDNS) name on both `Host` and `Origin`.
  Everything else is still rejected; no wildcard.
- The node's tailnet name is discovered from the local Tailscale status (with a
  manual override), so the user does not have to hand-configure it.
- Optional per-user authorization: when an allow-list of Tailscale logins is
  configured, a proxied request is accepted only if it carries an allowed
  `Tailscale-User-Login` identity. Empty list = trust the whole tailnet.
- The app **still binds `127.0.0.1` only** — reachability beyond the machine is
  provided exclusively by the external `tailscale serve` proxy. Off by default.

## Capabilities

- **Modified:** `web-ui` — the *Localhost Trust Boundary* requirement
  generalizes to a configurable allowlist, and a new *Tailscale Serve Access*
  requirement is added.

## Impact

- `crates/openspec-app` — new Tailscale settings on the web config.
- `crates/specforge-web` — the guard generalizes from loopback-only to an
  allowlist; a small Tailscale-name discovery helper.
- `openspec/specs/web-ui/spec.md` — modified + added requirements.
- No change to the bind address, the transport (relative URLs + SSE already
  proxy cleanly), or TLS (Tailscale terminates it).

## Out of Scope

- Binding `0.0.0.0` or any non-loopback interface directly (different, weaker
  trust model — the `Tailscale-User-Login` header is only trustworthy under a
  loopback bind).
- Tailscale **Funnel** (public-internet exposure; carries no identity headers).
- Embedding a Tailscale node (`tsnet`) in the app, or a standalone token-auth
  scheme (a possible separate, complementary change).
