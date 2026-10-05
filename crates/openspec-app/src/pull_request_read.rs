//! A detail read end to end (`pull-request-viewer`: *Detail Reads Are Scoped
//! to the Snapshot*, *Shared Backoff and Detail Budget*; design D5, D8): the
//! I/O it goes through, and the read itself, which runs on the blocking pool.
//!
//! Everything a read touches outside the service — the clock, the process
//! environment a credential may come from, and the network — goes through
//! [`DetailIo`]. `AppService::pull_request_detail` passes [`LiveIo`]; tests
//! pass their own, so every decision is made over an injected transport and
//! clock.
//!
//! A read resolves its credential as its provider's poller does, then is
//! admitted by its provider's limits (`crate::pull_request_limits`), waiting
//! for a slot while two reads are in flight. Before each request it checks
//! that the provider is still enabled under the credential generation it
//! started with, and asks its permit, which holds it back while a deadline
//! holds and counts the request against the hourly budget. A read that finds
//! the provider disabled or its credential saved sends nothing more, and its
//! result is neither cached nor returned: the cache stores a result only
//! under the same check.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bitbucket::{self, BitbucketLimits};
use crate::bitbucket_detail::{self, Body};
use crate::events::PullRequestProvider;
use crate::github::{self, GithubLimits};
use crate::github_detail;
use crate::pull_request_cache::{PullRequestDetails, ReadDetail, RowSignature};
use crate::pull_request_detail::{
    assemble, PullRequestDetailOutcome, PullRequestReference, ReadEnd, ReadParts,
};
use crate::pull_request_limits::{Admission, Deadlines, DetailPermit};
use crate::pull_requests::PullRequestSummary;
use crate::settings::SettingsStore;

/// Everything a detail read touches outside the service.
pub(crate) trait DetailIo: Send + Sync {
    /// The clock, as Unix epoch seconds.
    fn now(&self) -> u64;
    /// The token a GitHub read authenticates with, resolved as the GitHub
    /// poller resolves it.
    fn github_token(&self, settings: &SettingsStore) -> Option<String>;
    /// The credential pair a BitBucket read authenticates with, resolved as
    /// the BitBucket poller resolves it.
    fn bitbucket_credentials(&self, settings: &SettingsStore) -> Option<(String, String)>;
    /// One POST of GitHub's detail query.
    fn github_post(&self, token: &str, url: &str, body: String) -> Option<github::Reply>;
    /// One GET of a page of GitHub's changed files.
    fn github_get(&self, token: &str, url: &str) -> Option<github::Reply>;
    /// One BitBucket GET, its body read as `body` says.
    fn bitbucket_get(
        &self,
        username: &str,
        token: &str,
        url: &str,
        body: Body,
    ) -> Option<bitbucket_detail::Reply>;
}

/// The real I/O: the wall clock, the process environment, and HTTPS.
pub(crate) struct LiveIo;

impl DetailIo for LiveIo {
    fn now(&self) -> u64 {
        now_unix()
    }

    fn github_token(&self, settings: &SettingsStore) -> Option<String> {
        github::resolve_token(|name| std::env::var(name).ok(), settings.github_token())
    }

    fn bitbucket_credentials(&self, settings: &SettingsStore) -> Option<(String, String)> {
        bitbucket::resolve_credentials(
            |name| std::env::var(name).ok(),
            settings.bitbucket_credentials(),
        )
    }

    fn github_post(&self, token: &str, url: &str, body: String) -> Option<github::Reply> {
        github::send(url, body, token)
    }

    fn github_get(&self, token: &str, url: &str) -> Option<github::Reply> {
        github_detail::send_get(url, token)
    }

    fn bitbucket_get(
        &self,
        username: &str,
        token: &str,
        url: &str,
        body: Body,
    ) -> Option<bitbucket_detail::Reply> {
        bitbucket_detail::send_get(url, username, token, body)
    }
}

/// The wall clock, as Unix epoch seconds.
pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether `provider` is enabled, as its setting says now.
pub(crate) fn provider_enabled(settings: &SettingsStore, provider: PullRequestProvider) -> bool {
    match provider {
        PullRequestProvider::Github => settings.github_enabled(),
        PullRequestProvider::Bitbucket => settings.bitbucket_enabled(),
    }
}

/// What a read needs of the service: cheap clones of its handles.
#[derive(Clone)]
pub(crate) struct ReadContext {
    pub(crate) settings: Arc<SettingsStore>,
    pub(crate) github_limits: GithubLimits,
    pub(crate) bitbucket_limits: BitbucketLimits,
    pub(crate) details: PullRequestDetails,
}

impl ReadContext {
    fn enabled(&self, provider: PullRequestProvider) -> bool {
        provider_enabled(&self.settings, provider)
    }
}

/// The admitted read's permit, or how the read ends without one: deferred
/// while a deadline or the budget holds, and abandoned when its provider
/// was disabled while it waited for a slot.
fn admitted<D: Deadlines>(admission: Admission<D>) -> Result<DetailPermit<D>, ReadEnd> {
    match admission {
        Admission::Admitted(permit) => Ok(permit),
        Admission::Deferred { until } => Err(ReadEnd::Deferred { until }),
        Admission::Refused => Err(ReadEnd::Abandoned),
    }
}

/// Asked before each of a read's requests: the provider still enabled under
/// the credential generation the read started with, as `current` says, and
/// then the permit, which holds the request back while a deadline holds and
/// otherwise counts it against the hourly budget.
fn clear_to_send<D: Deadlines>(
    current: &dyn Fn() -> bool,
    permit: &DetailPermit<D>,
    now: u64,
) -> Result<(), ReadEnd> {
    if !current() {
        return Err(ReadEnd::Abandoned);
    }
    permit
        .request(now)
        .map_err(|until| ReadEnd::Deferred { until })
}

/// One GitHub read, and when it was admitted.
fn read_github(
    context: &ReadContext,
    pull_request: &PullRequestReference,
    current: &dyn Fn() -> bool,
    io: &dyn DetailIo,
) -> Result<(ReadParts, u64), ReadEnd> {
    let token = io
        .github_token(&context.settings)
        .ok_or(ReadEnd::Unauthenticated)?;
    let limits = &context.github_limits;
    let enabled = || context.enabled(PullRequestProvider::Github);
    let permit = admitted(limits.admit(enabled, || io.now()))?;
    let read_at = io.now();
    let parts = github_detail::read_with(
        pull_request,
        |url, body| io.github_post(&token, url, body),
        |url| io.github_get(&token, url),
        || clear_to_send(current, &permit, io.now()),
        limits,
        || io.now(),
    )?;
    Ok((parts, read_at))
}

/// One BitBucket read, and when it was admitted.
fn read_bitbucket(
    context: &ReadContext,
    pull_request: &PullRequestReference,
    current: &dyn Fn() -> bool,
    io: &dyn DetailIo,
) -> Result<(ReadParts, u64), ReadEnd> {
    let (username, token) = io
        .bitbucket_credentials(&context.settings)
        .ok_or(ReadEnd::Unauthenticated)?;
    let limits = &context.bitbucket_limits;
    let enabled = || context.enabled(PullRequestProvider::Bitbucket);
    let permit = admitted(limits.admit(enabled, || io.now()))?;
    let read_at = io.now();
    let parts = bitbucket_detail::read_with(
        pull_request,
        |url, body| io.bitbucket_get(&username, &token, url, body),
        || clear_to_send(current, &permit, io.now()),
        limits,
        || io.now(),
    )?;
    Ok((parts, read_at))
}

/// The blocking read of the pull request `row` lists in `provider`'s
/// snapshot, through the row's own values. A manual refresh dates the cache's
/// manual bound once its read is stored. Its detail is cached and returned
/// only when the provider is still enabled, under the generation the read
/// started with, as it ends; otherwise a disabled provider refuses and a
/// saved credential leaves the read transient. A deferred read carries any
/// cached detail.
pub(crate) fn read_pull_request(
    context: &ReadContext,
    provider: PullRequestProvider,
    row: PullRequestSummary,
    manual: bool,
    io: &dyn DetailIo,
) -> PullRequestDetailOutcome {
    let Some(reference) = PullRequestReference::of_row(provider, &row) else {
        return PullRequestDetailOutcome::Unavailable;
    };
    let key = reference.key();
    let generation = context.details.generation(provider);
    let current =
        || context.enabled(provider) && context.details.generation(provider) == generation;
    let read = match provider {
        PullRequestProvider::Github => read_github(context, &reference, &current, io),
        PullRequestProvider::Bitbucket => read_bitbucket(context, &reference, &current, io),
    };
    let end = match read {
        Ok((parts, read_at)) => {
            let signature = RowSignature::of(&row);
            let (detail, files) = assemble(reference, row, parts, read_at);
            let read = ReadDetail {
                key: key.clone(),
                detail: detail.clone(),
                files,
                signature,
                generation,
                manual_at: manual.then_some(read_at),
            };
            if context.details.store(read, || context.enabled(provider)) {
                return PullRequestDetailOutcome::Detail {
                    detail: Box::new(detail),
                };
            }
            ReadEnd::Abandoned
        }
        Err(end) => end,
    };
    match end {
        ReadEnd::Abandoned if !context.enabled(provider) => PullRequestDetailOutcome::Refused,
        ReadEnd::Abandoned | ReadEnd::Transient => PullRequestDetailOutcome::Transient,
        ReadEnd::Unauthenticated => PullRequestDetailOutcome::Unauthenticated,
        ReadEnd::Unavailable => PullRequestDetailOutcome::Unavailable,
        ReadEnd::Deferred { until } => PullRequestDetailOutcome::Deferred {
            until_unix: until,
            detail: context.details.cached(&key, false).map(Box::new),
        },
    }
}

/// A scripted provider behind the I/O seam, for the service's tests: a clock
/// the test sets, a credential it can withhold, and a pull request every
/// request reads, each request recorded. A hook may run before any request is
/// answered, which is how a test changes the world between two requests, and
/// a test may push: move the head and change the files later reads see.
#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use crate::github::RateHeaders;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    /// The head and base commits every scripted pull request is read at
    /// until a test pushes.
    pub(crate) const HEAD: &str = "1111111111111111111111111111111111111111";
    pub(crate) const BASE: &str = "2222222222222222222222222222222222222222";

    type Hook = Box<dyn FnMut(usize) + Send>;

    /// What every scripted pull request is read as, and what a push changes.
    #[derive(Debug, Clone)]
    pub(crate) struct Pushed {
        /// Its head commit: GitHub's `headRefOid`, BitBucket's source commit.
        pub(crate) head: String,
        /// GitHub's one page of changed files.
        pub(crate) github_files: Value,
        /// BitBucket's diff text, whose files the diffstat does not list
        /// unless they are `README.md`.
        pub(crate) bitbucket_diff: Vec<u8>,
    }

    impl Default for Pushed {
        /// On GitHub, `src/lib.rs`, small enough to arrive with its hunks,
        /// and `big.rs`, whose 600 lines the budgets withhold; on BitBucket,
        /// a one-line change to `README.md`.
        fn default() -> Self {
            Self {
                head: HEAD.to_string(),
                github_files: json!([
                    { "filename": "src/lib.rs", "status": "modified", "additions": 1, "deletions": 0,
                      "sha": "sha-lib", "patch": added(1) },
                    { "filename": "big.rs", "status": "added", "additions": 600, "deletions": 0,
                      "sha": "sha-big", "patch": added(600) },
                ]),
                bitbucket_diff: b"diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1,2 @@\n a\n+b\n".to_vec(),
            }
        }
    }

    pub(crate) struct FakeIo {
        clock: AtomicU64,
        credentialed: AtomicBool,
        requests: Mutex<Vec<String>>,
        hook: Mutex<Option<Hook>>,
        pushed: Mutex<Pushed>,
    }

    impl FakeIo {
        pub(crate) fn new(now: u64) -> Arc<Self> {
            Arc::new(Self {
                clock: AtomicU64::new(now),
                credentialed: AtomicBool::new(true),
                requests: Mutex::new(Vec::new()),
                hook: Mutex::new(None),
                pushed: Mutex::new(Pushed::default()),
            })
        }

        pub(crate) fn set_now(&self, now: u64) {
            self.clock.store(now, Ordering::SeqCst);
        }

        /// Changes what every later read sees, as a push would.
        pub(crate) fn push(&self, push: impl FnOnce(&mut Pushed)) {
            push(&mut self.pushed.lock().unwrap());
        }

        fn pushed(&self) -> Pushed {
            self.pushed.lock().unwrap().clone()
        }

        /// Resolves no credential from now on.
        pub(crate) fn withhold_credentials(&self) {
            self.credentialed.store(false, Ordering::SeqCst);
        }

        /// Every request sent so far, in order, each as its method and URL,
        /// and a POST's body.
        pub(crate) fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }

        /// Runs `hook` before each request is answered, with that request's
        /// number, counted from one.
        pub(crate) fn before_request(&self, hook: impl FnMut(usize) + Send + 'static) {
            *self.hook.lock().unwrap() = Some(Box::new(hook));
        }

        fn record(&self, request: String) {
            let number = {
                let mut requests = self.requests.lock().unwrap();
                requests.push(request);
                requests.len()
            };
            if let Some(hook) = self.hook.lock().unwrap().as_mut() {
                hook(number);
            }
        }
    }

    /// A patch adding `lines` lines.
    pub(crate) fn added(lines: usize) -> String {
        let mut patch = format!("@@ -0,0 +1,{lines} @@");
        for n in 0..lines {
            patch.push_str(&format!("\n+line {n}"));
        }
        patch
    }

    fn ok(body: Value) -> Option<github::Reply> {
        Some(github::Reply {
            status: 200,
            headers: RateHeaders::default(),
            body: Some(body.to_string()),
        })
    }

    fn bitbucket_ok(body: impl Into<Vec<u8>>) -> Option<bitbucket_detail::Reply> {
        Some(bitbucket_detail::Reply {
            status: 200,
            retry_after: None,
            body: body.into(),
        })
    }

    impl DetailIo for FakeIo {
        fn now(&self) -> u64 {
            self.clock.load(Ordering::SeqCst)
        }

        fn github_token(&self, _: &SettingsStore) -> Option<String> {
            self.credentialed
                .load(Ordering::SeqCst)
                .then(|| "ghp_scripted".to_string())
        }

        fn bitbucket_credentials(&self, _: &SettingsStore) -> Option<(String, String)> {
            self.credentialed
                .load(Ordering::SeqCst)
                .then(|| ("ada".to_string(), "ATBB-scripted".to_string()))
        }

        /// The detail query of any pull request, at the pushed head.
        fn github_post(&self, _: &str, url: &str, body: String) -> Option<github::Reply> {
            self.record(format!("POST {url} {body}"));
            let pushed = self.pushed();
            let total = pushed.github_files.as_array().map_or(0, Vec::len);
            ok(json!({ "data": { "repository": { "pullRequest": {
                "body": "Adds rate limits.",
                "author": { "login": "ada" },
                "baseRefName": "main", "headRefName": "feature",
                "baseRefOid": BASE, "headRefOid": pushed.head,
                "files": { "totalCount": total },
            } } } }))
        }

        /// The pushed files, on one page.
        fn github_get(&self, _: &str, url: &str) -> Option<github::Reply> {
            self.record(format!("GET {url}"));
            ok(self.pushed().github_files)
        }

        /// Any pull request, with the pushed diff, no comments and no
        /// statuses.
        fn bitbucket_get(
            &self,
            _: &str,
            _: &str,
            url: &str,
            _: Body,
        ) -> Option<bitbucket_detail::Reply> {
            self.record(format!("GET {url}"));
            let api = "https://api.bitbucket.org/2.0/repositories";
            let pushed = self.pushed();
            if url.contains("/diffstat/") {
                bitbucket_ok(
                    json!({ "values": [{
                    "status": "modified", "lines_added": 1, "lines_removed": 0,
                    "old": { "path": "README.md" }, "new": { "path": "README.md" },
                }] })
                    .to_string(),
                )
            } else if url.contains("/diff/") {
                bitbucket_ok(pushed.bitbucket_diff)
            } else if url.contains("/comments") || url.contains("/statuses") {
                bitbucket_ok(json!({ "values": [] }).to_string())
            } else {
                bitbucket_ok(json!({
                    "description": "Adds rate limits.",
                    "source": { "branch": { "name": "feature" }, "commit": { "hash": pushed.head } },
                    "destination": { "branch": { "name": "main" }, "commit": { "hash": BASE } },
                    "links": {
                        "diff": { "href": format!("{api}/acme/api/diff/acme/api:1%0D2") },
                        "diffstat": { "href": format!("{api}/acme/api/diffstat/acme/api:1%0D2") },
                    },
                }).to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitbucket::BitbucketDeadline;
    use crate::pull_request_limits::admit_or_fail;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn an_admission_becomes_a_permit_or_how_the_read_ends() {
        let deferred = Admission::<BitbucketDeadline>::Deferred { until: NOW };
        assert!(admitted(deferred).is_err_and(|end| end == ReadEnd::Deferred { until: NOW }));
        let refused = Admission::<BitbucketDeadline>::Refused;
        assert!(admitted(refused).is_err_and(|end| end == ReadEnd::Abandoned));
        let limits = BitbucketLimits::new();
        assert!(admitted(admit_or_fail(&limits, true, NOW)).is_ok());
    }

    /// A request is cleared only while the read is current, and then only
    /// outside a deadline, and each one cleared is counted.
    #[test]
    fn a_request_is_cleared_while_current_and_counted() {
        let limits = BitbucketLimits::new();
        let permit = match admit_or_fail(&limits, true, NOW) {
            Admission::Admitted(permit) => permit,
            other => panic!("admitted, got {other:?}"),
        };
        assert_eq!(
            clear_to_send(&|| false, &permit, NOW),
            Err(ReadEnd::Abandoned)
        );
        assert_eq!(limits.spent(NOW), 0, "an abandoned request is not counted");
        assert_eq!(clear_to_send(&|| true, &permit, NOW), Ok(()));
        assert_eq!(limits.spent(NOW), 1);
        limits.rate_limited(Some(60), NOW);
        assert_eq!(
            clear_to_send(&|| true, &permit, NOW + 59),
            Err(ReadEnd::Deferred { until: NOW + 60 })
        );
        assert_eq!(limits.spent(NOW + 59), 1, "nor is a deferred one");
    }

    #[test]
    fn a_provider_is_enabled_as_its_own_setting_says() {
        let cfg = tempfile::tempdir().unwrap();
        let settings = SettingsStore::load(cfg.path().join("settings.json"));
        settings.set_github_enabled(true).unwrap();
        assert!(provider_enabled(&settings, PullRequestProvider::Github));
        assert!(!provider_enabled(&settings, PullRequestProvider::Bitbucket));
        settings.set_bitbucket_enabled(true).unwrap();
        settings.set_github_enabled(false).unwrap();
        assert!(!provider_enabled(&settings, PullRequestProvider::Github));
        assert!(provider_enabled(&settings, PullRequestProvider::Bitbucket));
    }
}
