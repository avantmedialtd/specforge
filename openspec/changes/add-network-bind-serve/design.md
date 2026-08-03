# Design

## Context

The `web-ui` capability was built loopback-first and says so in three separate requirements (`web-ui/spec.md:11`, `:119`, `:148`). This change puts one deliberate hole in that, for one binary, behind one flag. The value of writing the design down is not the flag — it is recording *exactly* what the hole admits, so the next person to touch the guard knows which invariants are still load-bearing and which were traded away on purpose.

## Decision 1: The standalone binary, not the TUI

The request arrived as "make the TUI binary serve the UI on a port." The TUI is the natural target because it is the only SpecForge binary CI ships, so on a remote box it is the only one you have. But the gap it exposes is a *distribution* gap, not a missing capability: `specforge-serve` already does the job.

There is a real argument for the TUI hosting the server — one process means one `AppService`, one watcher, and one writer, avoiding the two-writer `activity.json` contention that `specforge-web/src/main.rs:10-13` documents for running the standalone server beside the desktop app. That argument applies to running it beside a TUI too.

**Rejected** in favour of shipping `specforge-serve`, because:

- it costs no new code paths in `specforge-tui`, which stays a terminal frontend with no HTTP surface and no reason to link axum;
- `specforge-tui` would otherwise embed the 5.7 MB `dist/` bundle via rust-embed in release builds, for a feature most TUI users never touch;
- the contention it avoids is only a problem if you want the terminal UI *and* the browser UI on the same machine at the same time, which is the less common case for a headless box;
- `release-pipeline` already has a five-requirement template for shipping a standalone binary per platform, so the distribution fix is mechanical.

The consequence to accept: running `specforge-serve` and `specforge-tui` against the same config dir means two `AppService` instances and the documented `activity.json` contention. That is unchanged by this proposal; it is simply no longer hypothetical now that both binaries ship. If it bites, hosting the server inside the TUI is the follow-up.

## Decision 2: `--bind` must move three gates, not one

The guard in `lib.rs` is three independent gates, and only the first is about which interface is bound:

```
  GATE 1  BIND          127.0.0.1                 who can open a socket
  GATE 2  HOST/ORIGIN   {localhost,127.0.0.1,::1} which page may drive the API
                        (+ tailnet name)          — the DNS-rebinding defense
  GATE 3  LOGIN         Tailscale-User-Login      which tailnet user, trusted
                        (non-loopback Host only)  only because GATE 1 is loopback
```

A `--bind` that moves only gate 1 produces a server that accepts TCP connections from the LAN and answers every one of them with `403 forbidden: host not allowed` (`lib.rs:157`), because a request to `http://192.168.1.5:4317/` carries `Host: 192.168.1.5:4317`, which is not in the allowlist. That reads as a bug and is worse than not having the flag. So gate 2 has to move with gate 1, and gate 3's premise has to be dealt with. All three are in scope.

## Decision 3: an explicit network bind disables the authority allowlist

When `--bind` names a non-loopback address, `Host` and `Origin` are accepted unconditionally.

This is the operator's declared position — they have asked to publish the UI on a network they trust, and any narrower rule turns "reach it at `http://devbox:4317`" into a support question about which spelling of the host name is on an allowlist. The flag should do the obvious thing.

It is also a genuine reversal of `web-ui/spec.md:119` ("never a wildcard and never 'any non-loopback host'"), so the delta rewrites that sentence rather than quietly outgrowing it. **What the reversal admits, precisely:**

The allowlist is not bookkeeping about reachability. It is the DNS-rebinding defense, and rebinding is the one attack that walks around CORS:

- An ordinary cross-origin request from a malicious page to `http://192.168.1.5:4317/api/invoke` never lands. `Content-Type: application/json` is not CORS-safelisted, so the browser preflights; the router has only `post(invoke_handler)` on that path, so `OPTIONS` returns 405 and the real request is never sent. The `Host` check is not what saves you here.
- A rebinding request does land. The attacker points a name they control at the server's address, and the page fetches *its own origin* — same-origin, so no preflight, no CORS, and the response body is readable by the page. The request carries `Host: evil.example:4317`. Gate 2 is the only thing that can distinguish it from a legitimate request, and with the allowlist disabled it cannot.

So on a `--bind 0.0.0.0` server the reachable audience is not "hosts on the subnet" but "hosts on the subnet, plus any website visited by any browser on the subnet." What that audience can read is the whole `/api/invoke` surface: every `.md` under every registered workspace (`read_workspace_file`, `list_markdown_files`, `read_artifact`), the commit graph and full commit diffs (`get_commit_graph`, `get_commit_detail`, `get_commit_diff`), and the workspace list — plus the `set_*` arms, which can rewrite settings, and `register_workspace`, which can widen what is readable to any path on the host containing an `openspec/` directory.

What it still cannot do is act on the host: `open_artifact_link` is refused by name (`dispatch.rs:112`), `read_workspace_file` is `.md`-only, `..`-free, canonicalised and confined to a registered root (`service.rs:758-786`), and there is no write path into workspace files at all (`web-ui/spec.md:209`). The exposure is disclosure and reconfiguration, not execution.

**Mitigations that are load-bearing and must stay:** off by default; absent from the embedded server; not reachable from any UI, settings file, or environment default that a user could arrive at without typing the flag; and announced at startup. The startup banner is part of the security posture, not decoration — it is the only moment the operator is told what they just published.

**Shape in code:** a two-variant trust mode on `GuardConfig` (`Allowlist(Vec<String>)` vs `AnyAuthority`) rather than pushing a `"*"` string into the existing `Vec`. A sentinel in the allowlist would make `is_allowed_authority` silently permissive and would be one typo away from matching a real host named `*`; an enum makes the bypass a branch you have to read.

## Decision 4: `--bind` is a CLI flag, never a setting

`WebServerConfig` (`settings.rs:82`) is shared: the desktop app's embedded toggle reads the same struct. A persisted `bindAddress` field would therefore be read by the desktop app, and a hand-edited settings file — or a future settings UI control — could push the *desktop* server off loopback. That would silently break `web-ui/spec.md:11` for a path nobody intended to change, on machines whose owners never asked for network exposure.

Keeping the widening exclusively on the standalone binary's argv draws the line where the intent is:

```
  embedded (desktop toggle)  ──►  loopback, always, no override
  standalone specforge-serve ──►  loopback by default
                                  --bind is the only way to widen
```

It also keeps the spec delta narrow: the existing requirement stays literally true for the embedded server and gains a sibling for the standalone one, instead of being rewritten into something conditional that both paths have to be read against.

`SPECFORGE_WEB_BIND` is an environment *fallback*, mirroring the existing `SPECFORGE_WEB_PORT`, and is equally explicit — it has to be set for this process. It is not a persisted setting and nothing writes it.

## Decision 5: fail loud when the Tailscale login gate is voided

Gate 3's trustworthiness is asserted on one basis, in the spec (`:152`) and again at the check site (`lib.rs:141-144`): the `Tailscale-User-Login` header can be trusted because the server binds loopback, so only `tailscale serve` can deliver a non-loopback request and it strips any client-supplied copy. Bind a network interface and that stops being true — anyone who can reach the port can send the header themselves and satisfy the allow-list.

Two ways to handle it: ignore the login gate under a network bind, or refuse to start. **Refuse to start**, with a message naming both inputs. A user who configured `allowedLogins` did so to restrict access; silently downgrading that to "no restriction" because of an unrelated flag is the failure mode where someone believes they are protected and is not. An error they have to resolve — drop the flag, or clear the allow-list — makes the trade explicit.

The check is narrow on purpose: only a *non-empty* `allowedLogins` blocks startup. `tailscale.enabled` alone does not, because it only widens the allowlist, which a network bind is already doing wholesale.

## Decision 6: extend the hand-rolled parser; add `--help`

`resolve_port()` (`main.rs:45`) is a hand-rolled scan over `std::env::args()`, matching the `--status` / `--line` style of `specforge-tui`. The flag surface after this change is three (`--bind`, `--port`, `--help`/`--version`), which does not justify pulling `clap` into a workspace that has so far avoided it.

`--help` is worth adding even though it is manual: this binary is about to land on remote machines as a downloaded tarball, where `--help` is the only documentation within reach. It should name the default bind and say what a non-loopback bind means. Both `--bind 0.0.0.0` (bare address, combined with `--port`) and the `--flag=value` spelling the existing parser already accepts are supported.

An unparseable `--bind` value must exit non-zero rather than falling back to loopback. Silently ignoring a malformed security-relevant flag is how someone ends up believing the server is reachable when it is not — or, in the reverse case for a future default change, the opposite.

## Decision 7: CI is a mechanical mirror of the TUI binary

Each release job already runs `bun install` and a Tauri build, and `bun tauri build` runs `bun run build` first. `dist/` therefore exists before any `cargo build` in all three jobs, which satisfies rust-embed's compile-time `#[folder = "../../dist"]` requirement (`assets.rs:19`) with no new step. The shared `openspec-app` / `openspec-core` graph is already compiled by that Tauri build, so the incremental cost is the `specforge-web` crate plus axum.

Per job: one build step, one package step, one line in the upload glob — the same shape as the existing TUI steps, including the macOS two-arch build plus `lipo`, and `cargo xwin build` on the Windows cross job. No new jobs, runners, toolchains, or caches, which keeps the existing `release-pipeline` requirements about not adding runners true for the new binary too.

## Alternatives considered

| Alternative | Why not |
|---|---|
| Host the server inside `specforge-tui` (`--serve`) | The stronger long-term answer for terminal + browser on one box (one `AppService`, no `activity.json` contention), but it links axum and embeds 5.7 MB of `dist/` into a binary whose users mostly want a TUI. The immediate gap is distribution, and shipping `specforge-serve` closes it without touching the terminal frontend. Left as a follow-up. |
| Restrict the widened allowlist to IP literals (plus named `--allow-host` values) | Would preserve the rebinding defense at no configured cost — a rebind needs a *name*, and a request whose `Host` is a bare IP is cross-origin again and dies on CORS preflight. Considered and explicitly declined: it makes reaching the server by hostname a configuration step, and the operator's position is that the network is trusted. Recorded here because it is the natural hardening if that position ever changes. |
| A token or basic auth on non-loopback binds | Real authentication, and the honest answer for an untrusted network. Out of scope for this change and cleanly layerable on top of `--bind` later, since it would gate requests rather than change what is bound. |
| Degrade to read-only when bound non-loopback (drop `set_*` and `register_workspace`) | Cheap, and would stop a remote peer widening its own read surface. Deferred rather than rejected: it is an independent axis from the bind flag and belongs behind its own switch. |
| `--bind` as a `WebServerConfig` field | Rejected — see Decision 4. The struct is shared with the embedded desktop server. |
| Document `ssh -L` / `tailscale serve` harder instead | Both already work and remain the recommended path on untrusted networks, but neither is available on a box without Tailscale or an SSH client on the viewing device, and neither addresses the fact that the server binary is not downloadable at all. |
