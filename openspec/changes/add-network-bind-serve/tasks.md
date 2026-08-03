# Tasks

## 1. CLI surface (`specforge-web/src/main.rs`)

- [x] 1.1 Add `resolve_bind() -> Result<IpAddr, String>`: scan argv for `--bind <addr>` / `--bind=<addr>`, fall back to `SPECFORGE_WEB_BIND`, then to `127.0.0.1`. Mirror the existing `resolve_port` scan style — no new argument-parsing dependency
- [x] 1.2 An unparseable `--bind` value (or env value) exits non-zero naming the offending string. Never silently fall back to loopback
- [x] 1.3 Compose `SocketAddr::new(bind, port)` and pass it to `specforge_web::serve`; drop the hard-coded `[127, 0, 0, 1]`
- [x] 1.4 `--help` (usage, both flags, both env vars, the default bind, and one line on what a non-loopback bind publishes) and `--version` (from `CARGO_PKG_VERSION`). Exit 0 for both
- [x] 1.5 Reject unknown `-`-prefixed flags with a usage hint and exit 2, matching `specforge-tui`'s existing behaviour
- [x] 1.6 Startup banner: loopback prints today's `SpecForge web UI on http://<addr>`; a non-loopback bind additionally prints that the UI is reachable from the network and unauthenticated. Print before `serve` awaits, so it is visible even if the bind then fails
- [x] 1.7 Update the module doc comment — it currently states "Binds the loopback interface only"

## 2. Trust boundary (`specforge-web/src/lib.rs`)

- [x] 2.1 Give `GuardConfig` an explicit trust mode — `Allowlist(Arc<Vec<String>>)` vs `AnyAuthority` — rather than a `"*"` sentinel in the existing `Vec`, so the bypass is a branch a reader has to see (see `design.md` Decision 3)
- [x] 2.2 `build_guard_config` takes the resolved bind address; a non-loopback bind yields `AnyAuthority`, loopback yields today's allowlist unchanged
- [x] 2.3 `authority_guard` skips both the `Host` and the `Origin` check under `AnyAuthority`, and is unchanged otherwise
- [x] 2.4 Under `AnyAuthority` the `Tailscale-User-Login` gate is unreachable by construction (startup refuses that combination — §3), but assert the invariant at the check site with a comment recording *why* the header is no longer trustworthy off loopback
- [x] 2.5 `serve()`'s doc comment currently says the address "must be loopback" — correct it
- [x] 2.6 Keep `router(svc)` working for tests without a bind address (loopback/allowlist semantics by default); add the bind-aware constructor beside it rather than breaking the existing signature

## 3. Fail loud when the login gate is voided (`specforge-web`)

- [x] 3.1 At startup, if the resolved bind is non-loopback **and** `settings.web_config().tailscale.allowed_logins` is non-empty, exit non-zero with a message naming both the requested bind and the configured allow-list
- [x] 3.2 `tailscale.enabled` with an *empty* allow-list must NOT block startup (it only widens an allowlist that `AnyAuthority` already sets aside)
- [x] 3.3 Factor the predicate so it is unit-testable without binding a socket or constructing an `AppService`

## 4. Tests (`specforge-web`)

- [x] 4.1 Unit: `--bind` parsing — both spellings, IPv4, IPv6, `0.0.0.0`, env fallback, precedence (flag beats env beats default), malformed value is an error not a default
- [x] 4.2 Unit: `is_loopback` classification of the resolved bind (`127.0.0.1`, `::1` loopback; `0.0.0.0`, `192.168.1.5`, `::` not)
- [x] 4.3 Guard: under `AnyAuthority`, a request with an arbitrary `Host` **and** an arbitrary `Origin` is dispatched (drive the router via `oneshot`, as `tests/server.rs` already does)
- [x] 4.4 Guard: under the default allowlist, the same request is still `403` — the existing loopback behaviour is untouched
- [x] 4.5 Guard: `AnyAuthority` does not weaken anything else — `open_artifact_link` and `launch_on_login` are still refused, and an unknown command is still refused
- [x] 4.6 Startup predicate: non-loopback bind + non-empty `allowed_logins` → refuse; non-loopback + empty → allow; loopback + non-empty → allow
- [x] 4.7 Note: the mutation gate is **vacuous for this change** — `.cargo/mutants.toml` excludes `crates/specforge-web/**/*.rs` (the crate cannot build in a mutants scratch tree; it needs the gitignored `dist/`), so `cargo mutants --in-diff` will find no mutants in this diff and pass without measuring anything. All the rigour has to come from §4.1–4.6 being genuinely exhaustive. Do not read a green mutants job as coverage here

## 5. Release pipeline (`.github/workflows/release.yml`)

- [x] 5.1 Linux job: `cargo build --release -p specforge-web --bin specforge-serve`, package `specforge-serve_${VERSION}_linux-x64.tar.gz`, add to the upload glob
- [x] 5.2 Windows job: `cargo xwin build --release -p specforge-web --bin specforge-serve --target x86_64-pc-windows-msvc`, zip as `specforge-serve_${VERSION}_windows-x64.zip`, add to the upload glob
- [x] 5.3 macOS job: build both `x86_64-apple-darwin` and `aarch64-apple-darwin`, `lipo -create`, assert both slices are present (mirroring the TUI step), package `specforge-serve_${VERSION}_macos-universal.tar.gz`, add to the upload glob
- [x] 5.4 Place each build step *after* that job's `bun tauri build`, which is what produces the `dist/` rust-embed compiles in
- [x] 5.5 Verify the embedded bundle is non-empty rather than trusting the compile — assert `dist/index.html` exists before the cargo build, so an empty embed fails the job instead of shipping a binary that answers with the build hint
- [x] 5.6 Confirm `if-no-files-found: error` still covers the new archives (it is job-level, but the glob must actually match them)

## 6. Documentation

- [x] 6.1 Release-notes template/checklist: the macOS quarantine line for `specforge-serve`, and the network-bind posture (loopback by default; a non-loopback bind is unauthenticated, trusted-network only)
- [x] 6.2 `README` download/run section (if it enumerates release assets) gains `specforge-serve` beside `specforge-tui`
- [x] 6.3 `CLAUDE.md`: note that `specforge-web` now ships a released binary and that `--bind` is CLI-only by design (never a `WebServerConfig` field), so a future settings control is not added by reflex

## 7. Spec sync + verification

- [x] 7.1 Sync the deltas into `openspec/specs/web-ui/spec.md` (MODIFIED *Local Self-Served Web Server*, *Localhost Trust Boundary*, *Tailscale Serve Access*) and `openspec/specs/release-pipeline/spec.md` (seven ADDED requirements)
- [x] 7.2 `cargo test` + `cargo fmt --check` + `cargo clippy -- -D warnings` + `bun run build` green
- [ ] 7.3 Manual smoke: `specforge-serve --bind 0.0.0.0 --port 4317` on one machine, load `http://<lan-ip>:4317` from another, confirm the dashboard renders **and live-updates over SSE** (the guard change touches every request, including the event stream). Confirm the default invocation is still refused from the LAN
- [x] 7.4 Manual smoke: `--help`, `--version`, a malformed `--bind`, and the fail-loud path (non-loopback bind with `allowedLogins` set) each behave as specified
