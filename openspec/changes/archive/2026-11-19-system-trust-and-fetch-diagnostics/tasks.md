## 1. Trust roots (openspec-app)

- [x] 1.1 Add `rustls-native-certs` to `crates/openspec-app/Cargo.toml`, plus `webpki-root-certs` (pinned to the `webpki-roots` version ureq locks) and a direct edge to `rustls` at the version already in `Cargo.lock` (no new feature). Give each one a comment in the style of the existing manifest notes explaining why it is there and that the musl static build still holds.
- [x] 1.2 Create `crates/openspec-app/src/trust_roots.rs`: a `LazyLock` root set = Mozilla (`webpki-root-certs`) ∪ `rustls_native_certs::load_native_certs()` (unparseable entries skipped, total failure → Mozilla only), exposed as `root_certs() -> ureq::tls::RootCerts::Specific(..)`. Write the one-time stderr count line when it is built. Inject the native loader and the line sink so unit tests cover the union, skip, fallback and print-once paths (`outbound-https-trust`: *HTTPS Clients Trust Bundled and System Roots*, *The Root Set Is Reported Once*).
- [x] 1.3 Wire it into `usage_http::agent` (`crates/openspec-app/src/usage_http.rs`) via `TlsConfig::builder().root_certs(trust_roots::root_certs())`. Extend the existing agent-config test to assert the roots are `Specific` and the proxy is still `None`. Update the module docs, which currently describe the webpki-only behaviour.
- [x] 1.4 Wire the same roots into `pinned_agent` in `crates/openspec-app/src/pull_request_content_images.rs`, and into its test twin near line 1345, keeping `disable_verification(false)`.
- [x] 1.5 Add a real-TLS test: a local rustls server with a throwaway CA. The client refuses it with the default roots, and accepts it when the CA is supplied through the injected native loader (`outbound-https-trust`: *An untrusted chain is still refused*, *A corporate root in the system store is trusted*).

## 2. Failure classification (openspec-app)

- [x] 2.1 Add `UnavailableReason` (`#[serde(tag = "kind", rename_all = "camelCase")]`) and `classify(&ureq::Error) -> UnavailableReason` in a shared spot (e.g. `usage_http.rs`). It checks `Error::Rustls` and `Error::Io` with an inner `rustls::Error`, `HostNotFound`, `ConnectionFailed`, `Timeout` and the io kinds. Also give it a `describe()` that returns the stderr wording plus the error's `Display`.
- [x] 2.2 Using 1.5's server, assert that an untrusted self-signed certificate classifies as `CertificateUntrusted`, not `Connect`. Assert a closed local port classifies as `Connect`, and an unresolvable `.invalid` host as `Dns`.
- [x] 2.3 In `crates/openspec-app/src/bitbucket.rs`, have `send` return the classified failure instead of `None` (`Result<Reply, Failure>`, where the failure holds the reason, the path template and the message). Carry the reason through `FetchResult::Transient` / `Unavailable` / `RateLimited`, and map non-success statuses that aren't unauthenticated or 429 to `HttpStatus { status }`. Update every `fetch_snapshot_with` test double.
- [x] 2.4 Add `reason: Option<UnavailableReason>` to `BitbucketPullRequestsState`. Set it in `next_state`, `degrade_to_stale` (stale keeps rows and gains the reason) and `publish_held_unavailable` (`RateLimited`). Make sure `is_news` treats a reason change as news. Cover each scenario of `bitbucket-pull-requests`: *Unavailable and Stale Snapshots Carry a Reason* in the module's tests, and update `crates/openspec-app/tests/wire_shape.rs`.
- [x] 2.5 Hand-mirror `UnavailableReason` and the optional `reason` field in `src/types.ts`, next to `BitbucketPullRequestsState`.

## 3. stderr reporting (openspec-app)

- [x] 3.1 In the poll loop in `bitbucket.rs`, keep the last reported reason. Print `specforge: bitbucket: GET <template> failed: <describe>` when it changes, and `specforge: bitbucket: recovered` when an ok refresh follows a failure. Route the output through an injectable sink so tests assert once-per-change, recovery and silence on repeats (`bitbucket-pull-requests`: *Failures Are Reported on Standard Error*).
- [x] 3.2 Build path templates from the request kind, never by redacting the URL: `/2.0/user`, `/2.0/user/workspaces`, `/2.0/workspaces/{workspace}/pullrequests/{user}`. Add a test that the line for a failing `acme` listing contains neither `acme`, the username, the token, nor reply text.
- [x] 3.3 In `crates/openspec-app/src/bitbucket_detail.rs`, replace both `.call().ok()?` sites with the classifier, and print one line per failed detail read using `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}` (or the matching sub-path template). Leave the read's own result semantics unchanged.
- [x] 3.4 Confirm `specforge-tui` writes none of these lines: it runs no poller and never calls `trust_roots` from a code path that prints. If the TUI can reach `usage_http` (quota pollers), have the root-set line's sink stay silent there.

## 4. Panel (src/)

- [x] 4.1 In `src/components/PullRequestPanel.tsx`, make the BitBucket unavailable line state the reason ("BitBucket is unavailable: this machine does not trust the server's certificate. Add your organisation's root CA to the system store or SSL_CERT_FILE." etc.). Fall back to today's line when there is no reason, and leave GitHub's copy unchanged. Show a stale snapshot's reason in the header tooltip.
- [x] 4.2 Extend `src/components/PullRequestPanel.test.ts` with each reason's copy, the reasonless fallback, and the stale tooltip.

## 5. Verification

- [x] 5.1 `bun run build`, `bun run lint`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`.
- [x] 5.2 Mutation-test the diff (`cargo mutants --in-diff` against `origin/master`) and close any survivors in `trust_roots.rs`, `classify` and `next_state`.
- [x] 5.3 Manual smoke: run a debug `specforge-serve` with `SSL_CERT_FILE` pointing at a throwaway CA, with `api.bitbucket.org` routed to a local TLS server signed by a different CA. Confirm the panel names `certificateUntrusted` and stderr has exactly one failure line. Then switch to the right CA, confirm recovery and the recovered line, and confirm the startup root-count line appears once.
  - Waived by the user on 2026-11-19: `API_BASE` cannot be redirected without /etc/hosts or DNS edits. Covered instead by `bitbucket::tests::a_poller_behind_an_untrusted_root_says_so_once_and_recovers` (a loopback TLS stand-in: untrusted chain → `certificateUntrusted` with one stderr line, then a trusted chain → ok with a `recovered` line) and `trust_roots` `the_set_is_built_and_reported_once`. The real confirmation is the operator check on the affected WSL machine.
