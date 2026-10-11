## 1. Shared agents in openspec-app

- [x] 1.1 Confirm that the workspace toolchain supports `std::sync::LazyLock` (Rust 1.80 or later). If it does not, use `OnceLock::get_or_init` behind a private getter instead; nothing else in the design changes (design D1).
- [x] 1.2 Add the private `new_agent` constructor to `crates/openspec-app/src/usage_http.rs`. It takes a redirect choice: an enum `Redirects::{Follow, Never}`, or `Option<u32>`. Build the agent with `ureq::Agent::config_builder()` and set, at agent level, `timeout_global(Some(REQUEST_TIMEOUT))`, `http_status_as_error(false)` and `proxy(None)`. Also set `max_idle_age(IDLE_CONNECTION_AGE)` with a new `IDLE_CONNECTION_AGE = 15 s` constant, and `max_idle_connections(10)` and `max_idle_connections_per_host(3)`. Add `max_redirects(0)` only for `Never`. Keep the logic minimal, so that every branch is visible to the posture test (design D2, D4). This implements `github-pull-requests`: *GitHub Connection Reuse* and `bitbucket-pull-requests`: *Connection Reuse*.
- [x] 1.3 Add the two process-wide statics in `usage_http.rs`: `FOLLOWING`, built with `Redirects::Follow`, and `NO_REDIRECT`, built with `Redirects::Never` (design D1, D3).
- [x] 1.4 Rewrite `get` as `FOLLOWING.get(url)`, `get_without_redirects` as `NO_REDIRECT.get(url)` and `post` as `NO_REDIRECT.post(url)`. Each still adds `.header("Authorization", auth.header_value())`, and none calls `.config()` any more. Signatures stay the same, so no caller in `github.rs`, `github_detail.rs`, `bitbucket.rs`, `bitbucket_detail.rs`, `quota.rs` or `chatgpt_quota.rs` changes.
- [x] 1.5 Update the doc comments in `usage_http.rs`:
  - Move the posture rationale (the `proxy(None)` history and the redirect reasoning) from the builders onto `new_agent` and the statics.
  - Add a short paragraph to the module note explaining why the posture is agent-level: request-level config bypasses rustls' `ClientConfig` cache (`tls/rustls.rs` in ureq 3.4.0) and every request read the proxy environment.
  - State why there are two agents rather than one.
  - Keep the cross-references to the `github-pull-requests` and `bitbucket-pull-requests` spec requirements.

## 2. Tests in usage_http.rs (mutation-gated)

- [x] 2.1 Add `the_agents_carry_the_posture`. For both `FOLLOWING` and `NO_REDIRECT`, assert on `config()`:
  - `proxy().is_none()`;
  - `!http_status_as_error()`;
  - `timeouts().global == Some(REQUEST_TIMEOUT)`;
  - `max_idle_age() == IDLE_CONNECTION_AGE`;
  - `max_idle_connections() == 10` and `max_idle_connections_per_host() == 3`;
  - `max_redirects() == 0` for `NO_REDIRECT` and `max_redirects() > 0` for `FOLLOWING`.

  This test is what checks the proxy contract and the idle limit. Do not set `HTTPS_PROXY` in-process, because that races the other tests, and do not wait out the idle age, because that is a timing assertion (`github-pull-requests`: *GitHub Connection Reuse*, scenarios *An exported proxy is ignored* and *A connection idle past the limit is not reused*; `bitbucket-pull-requests`: *Connection Reuse*, the same two scenarios).
- [x] 2.2 Add `consecutive_gets_share_one_connection`. A loopback listener accepts exactly one connection and serves two keep-alive `HTTP/1.1 200` replies on it, each with a `Content-Length` body and no `Connection: close`, capturing each request head. The client sends `get` with `Auth::Bearer("first")` and then with `Auth::Bearer("second")`, and reads each body to the end. Assert that:
  - both heads arrived on the one stream;
  - the first head's `Authorization` is `Bearer first`;
  - the second head's `Authorization` is `Bearer second` and does not contain `first`;
  - after the server thread joins, a non-blocking `accept()` on the listener reports no second connection.

  Use no sleeps and no timing assertions. A client that opened a second connection would time out at `REQUEST_TIMEOUT`, so a regression makes the test fail, never pass by chance (scenarios *A poll cycle's requests share one connection*, *A replaced token is sent on a reused connection* and *Replaced credentials are sent on a reused connection*).
- [x] 2.3 Add `a_detail_get_and_a_post_share_one_connection`. This is the same harness, sending `get_without_redirects` followed by `post` with a body, and asserting that both requests arrived on one connection. That proves the two no-redirect builders share `NO_REDIRECT` (scenario *A detail read's requests share one connection*).
- [x] 2.4 Add `a_reused_connection_follows_no_redirect`. On one keep-alive connection, the server answers a `get_without_redirects` with a 200, then a second `get_without_redirects` with a `302` whose `Location` names a second, non-blocking listener. Assert that the second reply is the 302, that both requests arrived on one connection, and that the second listener's `accept()` reports nothing (scenarios *A reused connection follows no redirect* and *A reused connection follows no redirect for a detail read*).
- [x] 2.5 Keep `a_post_never_follows_a_redirect`, `a_detail_get_never_follows_a_redirect`, `the_request_carries_the_authorization_header` and `the_post_request_carries_the_bearer_header` unchanged, and confirm that they still pass. Their `Connection: close` servers keep them independent of the pool.
- [x] 2.6 If 2.2 to 2.4 share a loopback keep-alive helper, put it in the `#[cfg(test)]` module next to `test_response`. The helper must read each request head up to `\r\n\r\n` plus any `Content-Length` body before it replies, so that the POST body never ends up in the next head. Give the server's accepted stream a read timeout (for example `REQUEST_TIMEOUT` plus a margin) and return the listener from the server thread, so a regression that opens a second connection fails the test through the client's timeout and a joined thread, never by leaving the server thread blocked on a read.

## 3. Verification

- [x] 3.1 `cargo fmt --all -- --check`
- [x] 3.2 `cargo clippy --workspace --all-targets -- -D warnings`. Any new suppression must be `#[expect(…, reason = "…")]`, never `#[allow]`.
- [x] 3.3 `bun install && bun run build` once in a fresh worktree so that `dist/` exists, then `cargo test`, which must pass workspace-wide. Re-run `cargo test -p openspec-app usage_http` a few times to confirm that the new loopback tests are stable when run in parallel.
- [x] 3.4 `bun run lint`. There are no frontend changes, so this should be a clean run with no output.
- [x] 3.5 `bun run build`
- [x] 3.6 `git fetch origin master && git diff $(git merge-base origin/master HEAD) HEAD > /tmp/sf.diff && cargo mutants --in-diff /tmp/sf.diff`. Every mutant on the changed lines of `usage_http.rs` is caught, or is listed as unviable because `ureq::Agent` has no `Default`. Never use `--baseline=skip`. If a survivor appears, add an assertion to the posture test rather than an exclusion.
- [x] 3.7 Live smoke, if a GitHub token and BitBucket credentials are configured on the machine. Start `specforge-serve`, or `bun run wt:dev`, with both providers enabled, and open a GitHub pull request with more than 50 changed files and a BitBucket pull request. While each detail read runs, check `lsof -nP -iTCP -a -p <pid>`: it should show one established connection to `api.github.com` and one to `api.bitbucket.org`, not one per request. Confirm that both panels, the detail views and the quota gauges render as before. If no credentials are available, record that this check was skipped.
  - Credentialed check skipped: the implementation ran in an isolated background session with no live provider credentials. The hermetic loopback tests (2.2 to 2.4) cover plain-HTTP reuse. An unauthenticated live check during code review confirmed TLS reuse to `api.github.com` and `api.bitbucket.org`, with one handshake per host.
