//! The transport every provider review request goes through
//! (`pull-request-actions`: *Review Operations Respect Provider Failure
//! Boundaries*; design D9): the I/O each step's request touches, and the
//! discipline every request is admitted under.
//!
//! Which request goes where is the provider protocols' own business: the
//! GitHub review capability reads and writes GraphQL documents fixed at
//! build time, and the BitBucket one walks collections and writes beneath
//! the listed pull request. What they all share lives here, so no protocol
//! can be written loose. One of its requests is
//!
//! 1. **host-checked** — a GitHub request has no URL to mistype: its one
//!    GraphQL endpoint is fixed in this module, and a BitBucket request's
//!    URL is refused unless it parses, as a payload link must, to an `https`
//!    path under `https://api.bitbucket.org/2.0/`. Nothing reachable through
//!    a dot segment, a backslash or any other host is ever requested.
//! 2. **cleared** — through [`GithubAuthority::clear_to_send`] or
//!    [`BitbucketAuthority::clear_to_send`], which the operation runner
//!    provides: fresh durable credential resolution, the enabled flag and
//!    credential generation, the current snapshot's row, trusted identifiers
//!    and actor, fingerprint and actor against the operation's stamp —
//!    asked again before every request, so a credential another process
//!    saved or cleared is fenced at the next request no matter what this
//!    process's own memory said a moment ago
//!    (`pull-request-actions`: *Current State Authorises Every Draft and
//!    Submission Boundary*). A refused clearance sends nothing, and is
//!    answered with the gate's structured reason.
//! 3. **admitted** — by its provider's limits
//!    ([`crate::pull_request_limits`]), which the poller's and a detail
//!    read's deadlines, slots and line it shares, holding a slot until it
//!    is done; and *re-checked* against the same deadlines just before the
//!    send, so a deadline set while it was being admitted holds it back.
//!    The hourly detail budget is asked nothing: every one of its counted
//!    requests belongs to a detail read.
//! 4. **sent** — through `usage_http`'s agents, which follow no redirect and
//!    configure no ambient proxy, so the credential goes to exactly the one
//!    URL the call was given. A reply's body is read only as its status and
//!    the reconciliation byte bound make sense of it.
//!
//! The reply reduces to an [`ActionVerdict`], and what the run journals is
//! what that types, never a raw body: the whole of one rides
//! [`ActionVerdict::Read`] to the protocol parser that asked for it, and
//! never past it into a journal, an event or a log line.
//!
//! The real I/O is [`LiveActionIo`]; every test passes its own, so every
//! boundary decision is made over an injected transport and clock.

use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bitbucket::BitbucketLimits;
use crate::github::{self, GithubLimits};
use crate::pull_request_limits::ActionAdmission;
use crate::review_actions::BlockedReason;
use crate::usage_http::{self, Auth};

/// The pages one reconciliation pass reads, in total: a bound reached is
/// uncertainty, never proof of what a further page might have held.
pub const MAX_RECONCILIATION_PAGES: usize = 10;

/// The objects one page of a reconciliation pass carries: how many a page's
/// own reply is trusted to say it holds.
pub const RECONCILIATION_PAGE_OBJECTS: usize = 100;

/// The bytes one reconciliation response body may be read whole; a body that
/// reaches past it is uncertain, and is read no further.
pub const MAX_RECONCILIATION_BYTES: usize = 10 * 1024 * 1024;

/// The one endpoint a GitHub review request may go to. It is fixed here so
/// the token in its header can only ever pass over `https`, to GitHub's API
/// host, on the path its GraphQL documents are answered on.
pub const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";

// ---- the I/O ----

/// Everything a review request touches outside the service: the clock and
/// the network. The credential travels no further than the one header
/// value.
pub trait ActionIo: Send + Sync {
    /// The clock, as Unix epoch seconds — the shared deadlines', every
    /// clearance's and every bound's sense of now.
    fn now(&self) -> u64;
    /// One POST of GitHub's fixed GraphQL endpoint. A transport error is no
    /// reply.
    fn github_post(&self, token: &str, body: String) -> Option<ActionReply>;
    /// One GET of the BitBucket URL given, already vetted to the official
    /// host and path. A transport error is no reply.
    fn bitbucket_get(&self, username: &str, token: &str, url: &str) -> Option<ActionReply>;
    /// One POST carrying the JSON a write built, to the BitBucket URL
    /// given, already vetted to the official host and path. A transport
    /// error is no reply.
    fn bitbucket_post(
        &self,
        username: &str,
        token: &str,
        url: &str,
        json: String,
    ) -> Option<ActionReply>;
}

/// The real I/O: the wall clock and HTTPS, over the agents no redirect and
/// no ambient proxy belong to.
pub struct LiveActionIo;

impl ActionIo for LiveActionIo {
    fn now(&self) -> u64 {
        now_unix()
    }

    fn github_post(&self, token: &str, body: String) -> Option<ActionReply> {
        let response = usage_http::post(GITHUB_GRAPHQL_URL, Auth::Bearer(token))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", github::USER_AGENT)
            .send(body)
            .ok()?;
        reply_of(response)
    }

    fn bitbucket_get(&self, username: &str, token: &str, url: &str) -> Option<ActionReply> {
        let response = usage_http::get_without_redirects(url, Auth::Basic { username, token })
            .header("Accept", "application/json")
            .header("User-Agent", crate::bitbucket::USER_AGENT)
            .call()
            .ok()?;
        reply_of(response)
    }

    fn bitbucket_post(
        &self,
        username: &str,
        token: &str,
        url: &str,
        json: String,
    ) -> Option<ActionReply> {
        let response = usage_http::post(url, Auth::Basic { username, token })
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", crate::bitbucket::USER_AGENT)
            .send(json)
            .ok()?;
        reply_of(response)
    }
}

/// The clock, as Unix epoch seconds, of [`LiveActionIo`]. `now_unix` is
/// `pull_request_read`'s own; this keeps that module without an export only
/// this one wants.
pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// One action reply, reduced to what the classification and one protocol
/// parser may read: the status, the reply's rate-limit hints as their
/// headers carry them, and whatever of the body the status and the byte
/// bound make sense of. A transport error is no reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReply {
    /// The HTTP status, as the classification reads it.
    pub status: u16,
    /// The reply's `Retry-After`, as sent, when it gave one.
    pub retry_after: Option<String>,
    /// The reply's `x-ratelimit-remaining`, as sent, when it gave one.
    pub remaining: Option<String>,
    /// The reply's `x-ratelimit-reset`, as sent, when it gave one.
    pub reset: Option<String>,
    /// Whatever of the body the reply's own status makes a difference to.
    pub body: ActionBody,
}

/// One reply's body, as its bound reads it. Nothing here is persisted: the
/// whole of one rides to the protocol parser that asked for it, and a
/// bound-struck one proves nothing at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionBody {
    /// Its status carries no body the classification would read one of.
    Unread,
    /// The body, whole and within its bound.
    Complete(Vec<u8>),
    /// The body filled the reconciliation byte bound: read no further, and
    /// trusted with nothing.
    Filled,
}

impl ActionBody {
    /// A body read whole, as its bound answers it: within the byte bound it
    /// is what arrived, past it, it is overrun and stands for nothing.
    pub fn read(bytes: Vec<u8>) -> Self {
        if bytes.len() > MAX_RECONCILIATION_BYTES {
            Self::Filled
        } else {
            Self::Complete(bytes)
        }
    }

    /// The whole body as text, for the one protocol that was handed it.
    /// A bound-struck body answers none.
    pub fn text(&self) -> Option<&str> {
        match self {
            ActionBody::Complete(bytes) => std::str::from_utf8(bytes)
                .ok()
                .filter(|text| !text.is_empty()),
            _ => None,
        }
    }
}

/// Reads one response into an action reply: its status, the raw rate-limit
/// hints, and — only for a body the classification needs, a 2xx, 403 or
/// 429 — its bytes up to one past the byte bound. A transport error, a
/// cut-off body included, is no reply.
fn reply_of(mut response: ureq::http::Response<ureq::Body>) -> Option<ActionReply> {
    let status = response.status().as_u16();
    let raw = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let retry_after = raw("Retry-After");
    let remaining = raw("x-ratelimit-remaining");
    let reset = raw("x-ratelimit-reset");
    let body = if matches!(status, 200..=299 | 403 | 429) {
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_RECONCILIATION_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        ActionBody::read(bytes)
    } else {
        ActionBody::Unread
    };
    Some(ActionReply {
        status,
        retry_after,
        remaining,
        reset,
        body,
    })
}

// ---- the clearances ----

/// The clearance one GitHub request goes through
/// (`pull-request-actions`: *Current State Authorises Every Draft and
/// Submission Boundary*). The operation runner implements it over the
/// service's fresh resolution and gate for one request's credential;
/// tests give their own.
pub trait GithubAuthority: Send + Sync {
    /// Asks everything a request's send holds — fresh durable credential
    /// resolution, the enabled flag and credential generation, the current
    /// snapshot's row, trusted identifiers, fingerprint and actor against
    /// the operation's stamp — and answers the token of whichever
    /// credential this fresh resolution resolved. It exists no longer than
    /// the request does, and the only place it can go is the one header.
    /// `Err` is the gate's structured reason, and no request is asked of
    /// the admission gate at all.
    fn clear_to_send(&self) -> Result<String, BlockedReason>;
}

/// The clearance of one BitBucket request, as a [`GithubAuthority`] is of
/// one of its own provider's.
pub trait BitbucketAuthority: Send + Sync {
    /// Asks the same for a BitBucket request, and answers the username and
    /// API token of its fresh resolution.
    fn clear_to_send(&self) -> Result<(String, String), BlockedReason>;
}

// ---- the verdicts and the flows ----

/// Which credential refusal a reply named, for the one sentence a report
/// words it with: no body rode it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialRefused {
    /// The provider does not accept the credential (401): it may be stale,
    /// or belong to an account that has since lost access.
    CredentialRefused,
    /// The provider accepted the credential but it may not write this
    /// review (403 without a rate-limit hint): it lacks the permission, or
    /// its account lost it.
    NotPermitted,
}

/// The sentence one permission refusal is worded with. Fixed, so nothing a
/// reply body carried ever reaches a report.
pub fn permission_reason(refusal: CredentialRefused) -> &'static str {
    match refusal {
        CredentialRefused::CredentialRefused => "the review credential was refused by the provider",
        CredentialRefused::NotPermitted => {
            "the review credential may not perform this review action"
        }
    }
}

/// What one request came to once it was classified, as far as its status
/// and headers can name it. Nothing a journal or an event consumes can be
/// built from a raw body, for a body is only carried — whole — to the
/// protocol parser that asked to read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionVerdict {
    /// A 2xx whose whole body arrived: the protocol reads the ids and
    /// errors of what it returned.
    Read {
        /// The body, whole.
        body: Vec<u8>,
    },
    /// The provider rate-limited the request, as a poller's or a detail
    /// read's is: a shared deadline sets the run back, never across it.
    RateLimited {
        /// The delay the reply asks for; `None` leaves the provider's own
        /// default backoff.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// The credential does not write this review: the run stops, and
    /// repeating with the stamped credential is unavailable.
    Permission {
        /// Which refusal it was, one sentence says.
        insufficient: CredentialRefused,
    },
    /// A 422: only a stale or invalid anchor meets an action request
    /// there, and a stale anchor is never retried against another line.
    Stale,
    /// Nothing a status could decide — a transport failure any other status
    /// a provider gives an action request, and a 2xx whose body filled the
    /// byte bound: the provider's reconciliation read decides, and until
    /// then this step is unresolved.
    Unresolved,
}

/// Classifies one reply by its status and headers, provider-neutrally
/// (`pull-request-actions`: *Review Operations Respect Provider Failure
/// Boundaries*, the status half): a 429, and a 403 that signals a rate
/// limit, are the one a deadline answers; a 401 and a signal-less 403 are
/// the permission refusal; a 422 is the stale anchor; and a 2xx whose body
/// filled the bound settles nothing. The provider protocol reads the rest
/// out of a whole 2xx body, including the rate-limit error a GraphQL reply
/// carries within one.
pub fn classify(reply: Option<ActionReply>, now_unix: u64) -> ActionVerdict {
    let Some(reply) = reply else {
        return ActionVerdict::Unresolved;
    };
    let rate = github::RateHeaders::from_raw(
        reply.retry_after.as_deref(),
        reply.remaining.as_deref(),
        reply.reset.as_deref(),
    );
    if let Some(limit) =
        github::rate_limited_reply(reply.status, &rate, reply.body.text(), now_unix)
    {
        return ActionVerdict::RateLimited {
            delay_secs: limit.delay,
            secondary: limit.secondary,
        };
    }
    match reply.status {
        200..=299 => match reply.body {
            ActionBody::Complete(bytes) => ActionVerdict::Read { body: bytes },
            _ => ActionVerdict::Unresolved,
        },
        422 => ActionVerdict::Stale,
        401 => ActionVerdict::Permission {
            insufficient: CredentialRefused::CredentialRefused,
        },
        403 => ActionVerdict::Permission {
            insufficient: CredentialRefused::NotPermitted,
        },
        _ => ActionVerdict::Unresolved,
    }
}

// ---- the bounds ----

/// One reconciliation pass's pages, as its budget counts them: at most
/// [`MAX_RECONCILIATION_PAGES`] per pass, whatever the pass is looking
/// for. Any bound struck ends the pass uncertain rather than prove what a
/// further page might have held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageBudget(usize);

impl PageBudget {
    /// A fresh pass, within all its pages.
    pub fn full() -> Self {
        Self(MAX_RECONCILIATION_PAGES)
    }

    /// The pages still to be read.
    pub fn remaining(&self) -> usize {
        self.0
    }

    /// One page for one read: `false` once none are left, the pass then
    /// ending uncertain rather than prove a marker absent.
    pub fn spend(&mut self) -> bool {
        if self.0 == 0 {
            return false;
        }
        self.0 -= 1;
        true
    }
}

/// The objects one page's reply lists, as the per-page bound answers them:
/// `None` when it named more than a page's [`RECONCILIATION_PAGE_OBJECTS`],
/// a page that answered beyond what its pass asked for saying nothing the
/// pass may trust.
pub fn page_objects(listed: usize) -> Option<usize> {
    (listed <= RECONCILIATION_PAGE_OBJECTS).then_some(listed)
}

// ---- the transports ----

/// How one request ended, as the operation's journal can consume it:
/// whatever was sent, or why nothing was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionFlow {
    /// One verdict of the request's send.
    Sent(ActionVerdict),
    /// The clearance refused: no request, and the gate's structured reason
    /// the run stops before.
    Refused { reason: BlockedReason },
    /// A shared deadline held at admission or just before the send: no
    /// request, and `until` names when one may be cleared again, which an
    /// explicit Resume never crosses.
    Deferred { until: u64 },
    /// The destination was not the provider's official endpoint: nothing
    /// was sent, as no credential may go wherever a call was pointed that
    /// is not the row's own provider. Unreachable over a value the snapshot
    /// supplied; the guard stands so no bug's slip becomes a leak.
    OffHost,
}

/// Everything one GitHub action request goes through: the injected I/O, its
/// provider's shared limits, the enabled flag the admission gate reads
/// fresh each time, and the clearance asked before every request.
pub struct GithubTransport<'a> {
    limits: &'a GithubLimits,
    io: &'a dyn ActionIo,
    enabled: &'a dyn Fn() -> bool,
    authority: &'a dyn GithubAuthority,
}

impl<'a> GithubTransport<'a> {
    /// The transport over `limits` and `io`, whose every request is cleared
    /// by `authority`, admitted while `enabled` holds, and re-checked
    /// against the shared deadlines before its send.
    pub fn new(
        limits: &'a GithubLimits,
        io: &'a dyn ActionIo,
        enabled: &'a dyn Fn() -> bool,
        authority: &'a dyn GithubAuthority,
    ) -> Self {
        Self {
            limits,
            io,
            enabled,
            authority,
        }
    }

    /// One POST of the fixed GraphQL endpoint, carrying `body` — a fixed
    /// document with its runtime values as variables, as the review
    /// capability requires. Cleared first, then admitted, then re-checked,
    /// then sent; never a bundle a redirect or a spent budget may bend.
    pub fn graphql(&self, body: String) -> ActionFlow {
        // No URL a caller could hand: the endpoint is fixed, so the host
        // guard needs no check to pass.
        let token = match self.authority.clear_to_send() {
            Err(reason) => return ActionFlow::Refused { reason },
            Ok(token) => token,
        };
        match self.limits.admit_action(self.enabled, || self.io.now()) {
            ActionAdmission::Refused => ActionFlow::Refused {
                reason: BlockedReason::NotListed,
            },
            ActionAdmission::Deferred { until } => ActionFlow::Deferred { until },
            ActionAdmission::Admitted(permit) => {
                if let Err(until) = permit.request(self.io.now()) {
                    return ActionFlow::Deferred { until };
                }
                let reply = self.io.github_post(&token, body);
                ActionFlow::Sent(classify(reply, self.io.now()))
            }
        }
    }
}

/// Everything one BitBucket action request goes through, as a
/// [`GithubTransport`] is of its own provider's.
pub struct BitbucketTransport<'a> {
    limits: &'a BitbucketLimits,
    io: &'a dyn ActionIo,
    enabled: &'a dyn Fn() -> bool,
    authority: &'a dyn BitbucketAuthority,
}

impl<'a> BitbucketTransport<'a> {
    /// The transport over `limits` and `io`, whose every request is
    /// cleared by `authority`, admitted while `enabled` holds, re-checked
    /// against the shared deadlines before its send, and refused whole
    /// wherever its URL leaves the API's own `https` path.
    pub fn new(
        limits: &'a BitbucketLimits,
        io: &'a dyn ActionIo,
        enabled: &'a dyn Fn() -> bool,
        authority: &'a dyn BitbucketAuthority,
    ) -> Self {
        Self {
            limits,
            io,
            enabled,
            authority,
        }
    }

    /// One GET of `url` — a reconciliation pass's page — that nothing may
    /// bend a redirect nor a spent budget on.
    pub fn get(&self, url: &str) -> ActionFlow {
        self.send(url, |username, token, url| {
            self.io.bitbucket_get(username, token, url)
        })
    }

    /// One POST of `url` carrying `json`, as one write's request.
    pub fn post(&self, url: &str, json: String) -> ActionFlow {
        self.send(url, |username, token, url| {
            self.io.bitbucket_post(username, token, url, json)
        })
    }

    /// The clearance, admission, deadline re-check, send and reduce one
    /// BitBucket call shares, and nothing but exactly the official host's
    /// `https` path under `/2.0/` may leave it.
    fn send<C>(&self, url: &str, call: C) -> ActionFlow
    where
        C: FnOnce(&str, &str, &str) -> Option<ActionReply>,
    {
        // The host guard stands first: no credential may reach a URL that
        // a call passed in without the row's provider naming it, whatever
        // the state of the clearance.
        if !crate::bitbucket_detail::followable(url) {
            return ActionFlow::OffHost;
        }
        let (username, token) = match self.authority.clear_to_send() {
            Err(reason) => return ActionFlow::Refused { reason },
            Ok((username, token)) => (username, token),
        };
        match self.limits.admit_action(self.enabled, || self.io.now()) {
            ActionAdmission::Refused => ActionFlow::Refused {
                reason: BlockedReason::NotListed,
            },
            ActionAdmission::Deferred { until } => ActionFlow::Deferred { until },
            ActionAdmission::Admitted(permit) => {
                if let Err(until) = permit.request(self.io.now()) {
                    return ActionFlow::Deferred { until };
                }
                let reply = call(&username, &token, url);
                ActionFlow::Sent(classify(reply, self.io.now()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitbucket_detail;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    const NOW: u64 = 1_800_000_000;

    /// One request's send, as the test transport records it: where it went
    /// and what it carried. Never a token — a test fake that records the
    /// credential value writes the very log line the feature must never
    /// produce.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct OneSent {
        url: String,
        body: String,
    }

    /// The io one test's transport transports with: the replies it gives
    /// per call, in the order asked, and every send recorded with where it
    /// went. Its clock is a test's to move.
    #[derive(Default)]
    struct TestIo {
        now: Mutex<u64>,
        github: Mutex<VecDeque<Option<ActionReply>>>,
        bitbucket: Mutex<VecDeque<Option<ActionReply>>>,
        sent: Mutex<Vec<OneSent>>,
    }

    impl TestIo {
        fn new(now: u64) -> Self {
            Self {
                now: Mutex::new(now),
                ..Self::default()
            }
        }

        fn hand_github(&self, reply: Option<ActionReply>) {
            self.github.lock().unwrap().push_back(reply);
        }

        fn hand_bitbucket(&self, reply: Option<ActionReply>) {
            self.bitbucket.lock().unwrap().push_back(reply);
        }

        fn sents(&self) -> Vec<OneSent> {
            self.sent.lock().unwrap().clone()
        }

        fn take_now(&self, to: u64) {
            *self.now.lock().unwrap() = to;
        }
    }

    impl ActionIo for TestIo {
        fn now(&self) -> u64 {
            *self.now.lock().unwrap()
        }

        fn github_post(&self, _token: &str, body: String) -> Option<ActionReply> {
            self.sent.lock().unwrap().push(OneSent {
                url: GITHUB_GRAPHQL_URL.to_string(),
                body,
            });
            self.github.lock().unwrap().pop_front().flatten()
        }

        fn bitbucket_get(&self, _username: &str, _token: &str, url: &str) -> Option<ActionReply> {
            self.sent.lock().unwrap().push(OneSent {
                url: url.to_string(),
                body: String::new(),
            });
            self.bitbucket.lock().unwrap().pop_front().flatten()
        }

        fn bitbucket_post(
            &self,
            _username: &str,
            _token: &str,
            url: &str,
            json: String,
        ) -> Option<ActionReply> {
            self.sent.lock().unwrap().push(OneSent {
                url: url.to_string(),
                body: json,
            });
            self.bitbucket.lock().unwrap().pop_front().flatten()
        }
    }

    /// A clearance a test's transport clears with: either a token it will
    /// repeat per ask, or the reason it refuses, in either order a test's
    /// runner may meet.
    struct TestGithubAuthority {
        cleared: Mutex<Result<String, BlockedReason>>,
        asked: Mutex<usize>,
    }

    impl TestGithubAuthority {
        fn grant(token: &str) -> Self {
            Self {
                cleared: Mutex::new(Ok(token.to_string())),
                asked: Mutex::new(0),
            }
        }

        /// The gate refuses from now on.
        fn refusals(&self, reason: BlockedReason) {
            *self.cleared.lock().unwrap() = Err(reason);
        }

        fn asked(&self) -> usize {
            *self.asked.lock().unwrap()
        }
    }

    impl GithubAuthority for TestGithubAuthority {
        fn clear_to_send(&self) -> Result<String, BlockedReason> {
            *self.asked.lock().unwrap() += 1;
            self.cleared.lock().unwrap().clone()
        }
    }

    /// The clearance of the BitBucket transport's tests, as its GitHub
    /// sibling is.
    struct TestBitbucketAuthority {
        cleared: Mutex<Result<(String, String), BlockedReason>>,
        asked: Mutex<usize>,
    }

    impl TestBitbucketAuthority {
        fn grant(username: &str, token: &str) -> Self {
            Self {
                cleared: Mutex::new(Ok((username.to_string(), token.to_string()))),
                asked: Mutex::new(0),
            }
        }

        fn asked(&self) -> usize {
            *self.asked.lock().unwrap()
        }
    }

    impl BitbucketAuthority for TestBitbucketAuthority {
        fn clear_to_send(&self) -> Result<(String, String), BlockedReason> {
            *self.asked.lock().unwrap() += 1;
            self.cleared.lock().unwrap().clone()
        }
    }

    fn github_transport<'a>(
        io: &'a TestIo,
        limits: &'a GithubLimits,
        authority: &'a TestGithubAuthority,
    ) -> GithubTransport<'a> {
        GithubTransport::new(limits, io, &|| true, authority)
    }

    fn bitbucket_transport<'a>(
        io: &'a TestIo,
        limits: &'a BitbucketLimits,
        authority: &'a TestBitbucketAuthority,
    ) -> BitbucketTransport<'a> {
        BitbucketTransport::new(limits, io, &|| true, authority)
    }

    /// A reply, as a fixture: one status, hint headers, one body.
    fn reply(
        status: u16,
        retry_after: Option<&str>,
        remaining: Option<&str>,
        reset: Option<&str>,
        body: ActionBody,
    ) -> Option<ActionReply> {
        Some(ActionReply {
            status,
            retry_after: retry_after.map(str::to_owned),
            remaining: remaining.map(str::to_owned),
            reset: reset.map(str::to_owned),
            body,
        })
    }

    fn body(text: &str) -> ActionBody {
        ActionBody::Complete(text.as_bytes().to_vec())
    }

    // ------------------------------------------------------ classification

    /// The outcomes, status by status, as the failure boundary names them:
    /// rate limited by a 429 and by a signalling 403, permission refused by
    /// a 401 and a signal-less 403, stale by a 422, unresolved by
    /// transports and statuses and bodies beyond their bound, and read by
    /// a 2xx whose body came whole.
    #[test]
    fn classify_maps_the_outcomes_the_boundary_names() {
        // A whole 2xx body is the protocol's to read.
        assert_eq!(
            classify(reply(200, None, None, None, body("{\"ok\":true}")), NOW),
            ActionVerdict::Read {
                body: b"{\"ok\":true}".to_vec()
            }
        );
        assert_eq!(
            classify(reply(429, Some("10"), None, None, ActionBody::Unread), NOW),
            ActionVerdict::RateLimited {
                delay_secs: Some(10),
                secondary: false
            }
        );
        // A primary limit on a 403, named by the exhausted window.
        assert_eq!(
            classify(
                reply(
                    403,
                    None,
                    Some("0"),
                    Some(&(NOW + 90).to_string()),
                    ActionBody::Unread
                ),
                NOW
            ),
            ActionVerdict::RateLimited {
                delay_secs: Some(90),
                secondary: false
            }
        );
        // A secondary limit on a 403, named by its body.
        assert_eq!(
            classify(
                reply(
                    403,
                    Some("60"),
                    Some("4999"),
                    None,
                    body("You have exceeded a secondary rate limit")
                ),
                NOW
            ),
            ActionVerdict::RateLimited {
                delay_secs: Some(60),
                secondary: true
            }
        );
        assert_eq!(
            classify(reply(401, None, None, None, ActionBody::Unread), NOW),
            ActionVerdict::Permission {
                insufficient: CredentialRefused::CredentialRefused
            }
        );
        assert_eq!(
            classify(
                reply(403, None, Some("4999"), None, ActionBody::Unread),
                NOW
            ),
            ActionVerdict::Permission {
                insufficient: CredentialRefused::NotPermitted
            }
        );
        assert_eq!(
            classify(reply(422, None, None, None, ActionBody::Unread), NOW),
            ActionVerdict::Stale
        );
        // A transport error, and whatever else no status names.
        assert_eq!(classify(None, NOW), ActionVerdict::Unresolved);
        for status in [301, 307, 404, 500] {
            assert_eq!(
                classify(reply(status, None, None, None, ActionBody::Unread), NOW),
                ActionVerdict::Unresolved,
                "status {status} is unresolved until its reconciliation read"
            );
        }
        // A 2xx whose body filled the bound is unresolved: a body beyond
        // the bound is trusted with nothing.
        assert_eq!(
            classify(reply(200, None, None, None, ActionBody::Filled), NOW),
            ActionVerdict::Unresolved
        );
        // And so is a 2xx whose bound-struck body is the last rate-limit
        // hint it could be read for.
        assert_eq!(
            classify(reply(403, None, None, None, ActionBody::Filled), NOW),
            ActionVerdict::Permission {
                insufficient: CredentialRefused::NotPermitted
            },
            "a bound-struck 403 is still a 403: no rate signal is read from the body"
        );
    }

    /// Nothing but [`ActionVerdict::Read`] carries what a body carried, so
    /// nothing a journal consumes may hold one: the same statuses with very
    /// different bodies answer identical verdicts.
    #[test]
    fn no_unread_verdict_causes_a_body_other_than_read() {
        let with_body = reply(
            401,
            None,
            None,
            None,
            body("{\"secret\":\"value\",\"message\":\"x\"}"),
        )
        .unwrap();
        let without_body = reply(401, None, None, None, ActionBody::Unread).unwrap();
        assert_eq!(
            classify(Some(with_body), NOW).clone(),
            classify(Some(without_body), NOW).clone()
        );
        let wordy_permission = permission_reason(CredentialRefused::CredentialRefused);
        assert!(wordy_permission.is_ascii());
        assert!(!wordy_permission.contains("value"));
        assert!(!wordy_permission.contains("secret"));
        assert_eq!(
            permission_reason(CredentialRefused::NotPermitted),
            "the review credential may not perform this review action"
        );
    }

    /// The body bound answers as the reconciliation bounds require: a body
    /// right at it arrives whole, one past it is overrun, and nothing read
    /// past the bound is trusted with text.
    #[test]
    fn a_body_beyond_its_bound_is_overrun() {
        let at = vec![b'x'; MAX_RECONCILIATION_BYTES];
        assert_eq!(
            ActionBody::read(at.clone()),
            ActionBody::Complete(at),
            "{MAX_RECONCILIATION_BYTES} bytes, whole"
        );
        let beyond = vec![b'x'; MAX_RECONCILIATION_BYTES + 1];
        assert_eq!(
            ActionBody::read(beyond),
            ActionBody::Filled,
            "one byte past the bound is overrun, read no further"
        );
        assert_eq!(ActionBody::Filled.text(), None);
        assert_eq!(
            ActionBody::read(Vec::new()).text(),
            None,
            "an empty body is not a body to read"
        );
    }

    /// The reconciliation bounds, demanded everywhere the pass might go:
    /// ten pages, one hundred objects a page.
    #[test]
    fn the_page_bounds_are_exactly_ten_pages_of_a_hundred_objects() {
        assert_eq!(MAX_RECONCILIATION_PAGES, 10);
        assert_eq!(RECONCILIATION_PAGE_OBJECTS, 100);
        assert_eq!(MAX_RECONCILIATION_BYTES, 10 * 1024 * 1024);
        let mut budget = PageBudget::full();
        assert_eq!(budget.remaining(), 10);
        for spent in 1..=10 {
            assert!(budget.spend(), "page {spent} is within the bound");
            assert_eq!(budget.remaining(), 10 - spent);
        }
        assert!(!budget.spend(), "an eleventh page is uncertain, not absent");
        assert_eq!(page_objects(100), Some(100));
        assert_eq!(page_objects(0), Some(0));
        assert_eq!(
            page_objects(101),
            None,
            "a page beyond the bound says nothing"
        );
    }

    // ------------------------------------------------------- the transports

    /// The GitHub transport sends the fixed GraphQL endpoint nothing but
    /// what it was given, and only after the clearance, the admission gate
    /// and the pre-send deadline check all passed — in that order, with no
    /// send whatever the first of them refuses.
    #[test]
    fn a_github_request_is_cleared_admitted_and_sent_to_the_graphql_endpoint() {
        let io = TestIo::new(NOW);
        let limits = GithubLimits::new();
        let authority = TestGithubAuthority::grant("tok");
        io.hand_github(reply(200, None, None, None, body("mark")));

        let flow =
            github_transport(&io, &limits, &authority).graphql("{\"query\":\"x\"}".to_string());
        assert_eq!(
            flow,
            ActionFlow::Sent(ActionVerdict::Read {
                body: b"mark".to_vec()
            }),
            "the whole mocked reply rides its verdict"
        );
        assert_eq!(
            io.sents(),
            [OneSent {
                url: GITHUB_GRAPHQL_URL.to_string(),
                body: "{\"query\":\"x\"}".to_string()
            }],
            "one send, to the one fixed endpoint"
        );
        assert_eq!(authority.asked(), 1);
    }

    /// A refused clearance sends nothing, and the structured reason is the
    /// flow's, and every later request asks again — a fenced credential at
    /// one request does not fence the whole run's questions.
    #[test]
    fn a_refused_clearance_sends_nothing_and_the_next_request_asks_fresh() {
        let io = TestIo::new(NOW);
        let limits = GithubLimits::new();
        let authority = TestGithubAuthority::grant("tok");

        // First request clears and sends; the second is fenced.
        let _ = github_transport(&io, &limits, &authority).graphql("first".to_string());
        authority.refusals(BlockedReason::CredentialChanged);
        let flow = github_transport(&io, &limits, &authority).graphql("second".to_string());
        assert_eq!(
            flow,
            ActionFlow::Refused {
                reason: BlockedReason::CredentialChanged
            },
            "the gate's structured reason stops the run"
        );
        assert_eq!(io.sents().len(), 1, "the fenced request sent nothing");
        assert_eq!(authority.asked(), 2, "each request is asked fresh");
    }

    /// A shared deadline holds a request back from admission, sends
    /// nothing, and names when it may be asked again — and once past it,
    /// with nothing having crossed, the request is sent.
    #[test]
    fn a_shared_deadline_defers_a_request_until_it_passes() {
        let io = TestIo::new(NOW);
        let limits = GithubLimits::new();
        let authority = TestGithubAuthority::grant("tok");
        limits.update_deadlines(|deadlines| {
            *deadlines = github::GithubDeadlines {
                graphql: NOW + 600,
                ..github::GithubDeadlines::default()
            }
        });
        let flow = github_transport(&io, &limits, &authority).graphql("body".to_string());
        assert_eq!(flow, ActionFlow::Deferred { until: NOW + 600 });
        assert!(io.sents().is_empty());
        io.take_now(NOW + 600);
        let flow = github_transport(&io, &limits, &authority).graphql("body".to_string());
        assert!(
            matches!(flow, ActionFlow::Sent(_)),
            "at exactly the deadline a request is admitted again"
        );
    }

    /// A disabled provider is refused by the gate: nothing is sent, and the
    /// NotListed reason the state answers with is the flow's.
    #[test]
    fn a_disabled_provider_refuses_whatever_the_clearance_says() {
        let io = TestIo::new(NOW);
        let limits = GithubLimits::new();
        let authority = TestGithubAuthority::grant("tok");
        let disabled = || false;
        let transport = GithubTransport::new(&limits, &io, &disabled, &authority);
        let flow = transport.graphql("body".to_string());
        assert_eq!(
            flow,
            ActionFlow::Refused {
                reason: BlockedReason::NotListed
            }
        );
        assert!(io.sents().is_empty());
    }

    /// The BitBucket transport sends the official host's own path, under
    /// the same guard a payload link obeys.
    #[test]
    fn a_bitbucket_request_sends_only_the_official_path() {
        let io = TestIo::new(NOW);
        let limits = BitbucketLimits::new();
        let authority = TestBitbucketAuthority::grant("user", "tok");
        io.hand_bitbucket(reply(200, None, None, None, body("{\"values\":[]}")));
        let url = "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests/7/comments";
        assert!(bitbucket_detail::followable(url));

        let flow = bitbucket_transport(&io, &limits, &authority).get(url);
        assert_eq!(
            flow,
            ActionFlow::Sent(ActionVerdict::Read {
                body: b"{\"values\":[]}".to_vec()
            }),
            "the official host's own path is followed"
        );
        assert_eq!(io.sents().len(), 1, "one request, exactly as asked");
        assert_eq!(authority.asked(), 1);
        // A write goes the same way, as a POST.
        io.hand_bitbucket(reply(200, None, None, None, body("{\"id\":9}")));
        let flow = bitbucket_transport(&io, &limits, &authority)
            .post(url, "{\"content\":\"c\"}".to_string());
        assert!(matches!(flow, ActionFlow::Sent(ActionVerdict::Read { .. })));
        assert_eq!(io.sents()[1].body, "{\"content\":\"c\"}");
    }

    /// A URL the guard refuses — another host, plain `http`, userinfo, an
    /// explicit port, a path outside `/2.0/`, or one a dot segment leads out
    /// of — is off-host: nothing is sent and no credential is even asked
    /// for, whatever the clearance would have said.
    #[test]
    fn the_official_path_refuses_its_neighbours() {
        let io = TestIo::new(NOW);
        let limits = BitbucketLimits::new();
        let authority = TestBitbucketAuthority::grant("user", "tok");
        for url in [
            "https://evil.example.com/2.0/repositories/ws/repo/pullrequests/7/comments",
            "http://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests/7/comments",
            "https://user@bitbucket.org/2.0/repositories/ws/repo/pullrequests/7/comments",
            "https://api.bitbucket.org:8443/2.0/repositories/ws/repo/pullrequests/7/comments",
            "https://api.bitbucket.org/1.0/repositories/ws/repo/pullrequests/7/comments",
            "https://api.bitbucket.org/2.0/repositories/ws/../pullrequests/7/comments",
            "https://api.bitbucket.org/2.0/repositories/ws/%2e%2e/pullrequests/7/comments",
        ] {
            let flow = bitbucket_transport(&io, &limits, &authority).get(url);
            assert_eq!(flow, ActionFlow::OffHost, "{url} is off the official host");
        }
        assert!(
            io.sents().is_empty(),
            "an off-host guard sends nothing at all"
        );
        assert_eq!(authority.asked(), 0, "an off-host guard asks no credential");
    }

    /// A held deadline defers a BitBucket request too: shared limits, as
    /// a detail read's.
    #[test]
    fn a_bitbucket_request_shares_its_deadlines_and_refuses_disabled() {
        let io = TestIo::new(NOW);
        let limits = BitbucketLimits::new();
        let authority = TestBitbucketAuthority::grant("user", "tok");
        limits.update_deadlines(|deadlines| {
            *deadlines = crate::bitbucket::BitbucketDeadline { until: NOW + 600 }
        });
        let url = "https://api.bitbucket.org/2.0/repositories/ws/repo/pullrequests/7/comments";
        let flow = bitbucket_transport(&io, &limits, &authority).get(url);
        assert_eq!(
            flow,
            ActionFlow::Deferred { until: NOW + 600 },
            "a deadline a detail read set holds the request back"
        );
        assert!(io.sents().is_empty());
        // And a disabled provider is refused the same way GitHub's is.
        let disabled = || false;
        let transport = BitbucketTransport::new(&limits, &io, &disabled, &authority);
        let flow = transport.get(url);
        assert_eq!(
            flow,
            ActionFlow::Refused {
                reason: BlockedReason::NotListed
            }
        );
        assert!(io.sents().is_empty());
        assert_eq!(authority.asked(), 2, "clearance precedes admission");
    }
}
