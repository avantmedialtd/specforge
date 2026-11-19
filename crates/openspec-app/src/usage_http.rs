//! The HTTP shape shared by the app's network callers: the two usage-quota
//! pollers (`crate::quota` and `crate::chatgpt_quota`), the BitBucket
//! pull-request poller (`crate::bitbucket`), the GitHub pull-request poller
//! (`crate::github`), and the pull-request viewer's detail reads.
//!
//! Every caller makes the same kind of request — a blocking, authenticated
//! call: a GET for three pollers, a POST carrying one constant GraphQL query
//! for GitHub's poller and its detail query, and a GET that follows no
//! redirect for the other detail reads — and reads the same outcomes off the
//! reply. Only the URL, the method, the credential scheme, the extra headers,
//! and the body parser differ, so the request posture, the `Authorization`
//! header and the status mapping live here once.
//! The credential is formatted into exactly one place — the header value built
//! by [`Auth`] — and never into anything a caller could log.
//!
//! ureq 3 is the reason this module exists rather than the mapping sitting
//! inline in each poller. In ureq 2 a non-2xx reply arrived as
//! `Error::Status(code, response)`, so the 429 branch could read `Retry-After`
//! straight off the error. ureq 3's nearest variant is `Error::StatusCode(u16)`,
//! which carries the code but drops the response — the header would be
//! unreachable. Keeping non-2xx on the `Ok` side (`http_status_as_error(false)`)
//! restores it, at the cost of moving the status branch into our own code. That
//! branch is [`classify`], which is pure and unit-tested, so the mutation gate
//! has assertions to catch it.
//!
//! The posture is agent-level, not per request. Every builder takes its
//! request from one of two process-wide agents, [`FOLLOWING`] and
//! [`NO_REDIRECT`], whose own `Config` carries the timeout, the status
//! handling, `proxy(None)` and the idle-pool limits, so a request inherits all
//! of it and no builder can forget a piece. Two costs went away with that:
//! `ureq::get`/`ureq::post` built a use-once agent per call, so every request
//! opened its own TCP and TLS connection and re-read the proxy environment;
//! and a per-request `.config()` makes a request *request-level*, which in
//! ureq 3.4.0 never fills the rustls connector's `ClientConfig` cache
//! (`tls/rustls.rs`), so every new connection rebuilt it, trust store and
//! all. Now the requests of one burst (a detail read, a file or image read, a
//! BitBucket poll cycle) share a pooled connection, and each agent builds its
//! TLS config once (`github-pull-requests`: *GitHub Connection Reuse*;
//! `bitbucket-pull-requests`: *Connection Reuse*).
//!
//! That TLS config trusts [`crate::trust_roots`]' one root set: the bundled
//! Mozilla roots together with the operating system's, `SSL_CERT_FILE` and
//! `SSL_CERT_DIR` included (`outbound-https-trust`: *HTTPS Clients Trust
//! Bundled and System Roots*). ureq's own default is the Mozilla roots alone,
//! which a TLS-inspecting network's corporate root is never in. Verification
//! itself is never relaxed.
//!
//! A request that produces no reply is classified by [`classify_error`] into
//! the closed set of reasons a snapshot carries, [`UnavailableReason`]. The
//! error's own message goes no further than a standard-error line
//! ([`RequestFailure::line`]), which names the request by its path template,
//! never by its URL.
//!
//! Reuse holds only when a reply's body is read to its end: ureq returns a
//! connection to the pool only then. A body left unread (a 401, a 404, a 3xx)
//! or cut short by a `take(limit)` cap closes its connection instead, so the
//! next request reconnects; it never inherits a half-read stream.
//!
//! There are two agents rather than one because the callers need two redirect
//! behaviours, and expressing either as a per-request override would make that
//! side request-level again: the pollers' [`get`] follows redirects, and every
//! detail, file and image read and every POST follows none.

use std::io;
use std::sync::LazyLock;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Serialize;
use ureq::config::RedirectAuthHeaders;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::typestate::{WithBody, WithoutBody};
use ureq::{Agent, RequestBuilder};

use crate::trust_roots;

/// Network timeout for a single request. `timeout_global` bounds one call,
/// not the agent's lifetime, so every request gets its own 15 s.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a pooled connection may sit idle and still be reused. ureq
/// 3.4.0's own default, pinned so a future default cannot silently change it.
///
/// Long enough for a burst's requests, which follow one another within a
/// fraction of a second; short enough that a poll cycle 30 s or 60 s later
/// opens a fresh connection. A longer age would race the providers' load
/// balancers closing idle keep-alives, and ureq never retries a request that
/// fails on a pooled connection the server closed meanwhile.
const IDLE_CONNECTION_AGE: Duration = Duration::from_secs(15);

/// Idle connections one agent keeps across all hosts (ureq 3.4.0's default,
/// pinned): at most 20 idle sockets for the process, none carrying a
/// credential.
const MAX_IDLE_CONNECTIONS: usize = 10;

/// Idle connections one agent keeps per host (ureq 3.4.0's default, pinned).
const MAX_IDLE_CONNECTIONS_PER_HOST: usize = 3;

/// Whether an agent follows redirects.
#[derive(Clone, Copy)]
enum Redirects {
    /// ureq's own redirect handling, with `redirect_auth_headers` pinned to
    /// `Never` so `Authorization` is never forwarded across a redirect.
    Follow,
    /// `max_redirects(0)`: a 3xx comes back as an ordinary reply.
    Never,
}

/// Build an agent carrying the posture every request wants.
///
/// - `timeout_global(REQUEST_TIMEOUT)`, per request.
/// - `http_status_as_error(false)` is what keeps a 429's `Retry-After`
///   reachable — see the module note.
/// - `proxy(None)` pins ureq 2's behaviour. ureq 3's `Config::default()` calls
///   `Proxy::try_from_env()`, whereas ureq 2 only honoured `HTTPS_PROXY`/
///   `ALL_PROXY` under its non-default `proxy-from-env` feature, which this
///   crate never enabled. Leaving the new default in place would silently
///   start routing these requests — and the `Authorization` header they
///   carry — through whatever proxy a user happens to have exported. Adopting
///   env-proxy support may well be worth doing, but it is a deliberate
///   decision about where a credential travels, not a side effect of a
///   version bump. Set on the agent, it covers every request, including one
///   from a builder added later.
/// - `redirect_auth_headers(Never)`, ureq 3.4.0's default, pinned: it is the
///   only thing keeping a poller's credential off the target of a redirect
///   [`FOLLOWING`] follows, so a future default cannot silently loosen it.
/// - The idle-pool limits, pinned at ureq 3.4.0's defaults
///   ([`IDLE_CONNECTION_AGE`]).
/// - `roots`, the root set certificates and hostnames are verified against:
///   the process's one set ([`trust_roots::root_certs`]) for both agents, or
///   a set a test builds. Verification stays on.
///
/// No cookie jar exists to carry state between requests (ureq's `cookies`
/// feature is off), and `Authorization` is a header on each request, never
/// agent state. Keep this free of logic the posture test cannot observe.
fn new_agent(redirects: Redirects, roots: RootCerts) -> Agent {
    let config = Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .proxy(None)
        .redirect_auth_headers(RedirectAuthHeaders::Never)
        .max_idle_connections(MAX_IDLE_CONNECTIONS)
        .max_idle_connections_per_host(MAX_IDLE_CONNECTIONS_PER_HOST)
        .max_idle_age(IDLE_CONNECTION_AGE)
        .tls_config(
            TlsConfig::builder()
                .root_certs(roots)
                .disable_verification(false)
                .build(),
        );
    match redirects {
        Redirects::Follow => config,
        Redirects::Never => config.max_redirects(0),
    }
    .build()
    .new_agent()
}

/// The agent behind the pollers' [`get`]: redirects followed, without the
/// `Authorization` header.
static FOLLOWING: LazyLock<Agent> =
    LazyLock::new(|| new_agent(Redirects::Follow, trust_roots::root_certs()));

/// The agent behind [`get_without_redirects`] and [`post`]: no redirect is
/// ever followed, so a request, its body and its credential only ever go to
/// the one URL the caller passed (`github-pull-requests`: *GitHub Privacy and
/// Safety*; `bitbucket-pull-requests`: *Privacy and Safety*). A GitHub detail
/// read's POST and its files GETs share it, and so its connections.
static NO_REDIRECT: LazyLock<Agent> =
    LazyLock::new(|| new_agent(Redirects::Never, trust_roots::root_certs()));

/// What one reply amounts to, before a poller maps it onto its own
/// `FetchResult`. Deliberately vendor-agnostic: the pollers hold different
/// state types, but they agree on these outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// 2xx — read and parse the body.
    Read,
    /// The endpoint rejected the credential (401).
    Unauthenticated,
    /// The credential is valid but may not read this resource (403). Only the
    /// BitBucket poller tells this apart — a workspace its token is not scoped
    /// to is skipped, not fatal. The quota pollers treat it as [`Transient`].
    ///
    /// [`Transient`]: Verdict::Transient
    Forbidden,
    /// The resource does not exist, or is hidden from this credential (404).
    /// The same split as [`Forbidden`](Verdict::Forbidden).
    NotFound,
    /// Rate-limited (429); back off for the hinted (or default) delay.
    RateLimited { retry_after: Option<u64> },
    /// Anything else. Transient from a gauge's point of view: keep showing the
    /// last known snapshot rather than blanking it.
    Transient,
}

/// The credential a request carries in its `Authorization` header.
///
/// Deliberately not `Debug`: a value holds a live secret, and the only thing
/// ever done with it is to turn it into the one header value below.
#[derive(Clone, Copy)]
pub(crate) enum Auth<'a> {
    /// `Authorization: Bearer <token>` — the two usage-quota pollers.
    Bearer(&'a str),
    /// `Authorization: Basic base64(username:token)` — the BitBucket poller
    /// (an API token paired with its account's username or email).
    Basic { username: &'a str, token: &'a str },
}

impl Auth<'_> {
    /// The `Authorization` header value. Handed straight to the request and
    /// never formatted into anything else.
    fn header_value(&self) -> String {
        match self {
            Auth::Bearer(token) => format!("Bearer {token}"),
            Auth::Basic { username, token } => {
                format!("Basic {}", STANDARD.encode(format!("{username}:{token}")))
            }
        }
    }
}

/// A GET builder carrying the posture every poller wants, authenticated with
/// `auth`, from the [`FOLLOWING`] agent.
///
/// It calls no `.config()`: a per-request override would make the request
/// request-level and bypass the agent's cached TLS config — see the module
/// note.
pub(crate) fn get(url: &str, auth: Auth<'_>) -> RequestBuilder<WithoutBody> {
    FOLLOWING
        .get(url)
        .header("Authorization", auth.header_value())
}

/// A GET builder with [`get`]'s posture plus redirects turned off, from the
/// [`NO_REDIRECT`] agent, for the pull-request viewer's detail reads
/// (`pull-request-viewer`: *GitHub Detail Reads*, *BitBucket Detail Reads*).
///
/// A followed redirect would take the read somewhere its caller never named,
/// and on to whatever lies beyond. With `max_redirects(0)` a 3xx comes back as
/// an ordinary reply, which the read reports as unavailable for that pull
/// request, so its credential only ever goes to the one URL the caller passed
/// (`bitbucket-pull-requests`: *Privacy and Safety*). The pollers keep [`get`].
pub(crate) fn get_without_redirects(url: &str, auth: Auth<'_>) -> RequestBuilder<WithoutBody> {
    NO_REDIRECT
        .get(url)
        .header("Authorization", auth.header_value())
}

/// A POST builder with the same posture as [`get`], plus redirects turned off,
/// from the [`NO_REDIRECT`] agent.
///
/// ureq 3 already drops the `Authorization` header when it follows a redirect,
/// but a followed POST would still re-send its body somewhere the caller never
/// named. With `max_redirects(0)` a 3xx comes back as an ordinary reply (the
/// posture keeps non-2xx on the `Ok` side), which [`classify`] reports as
/// transient — so the request, its body and its credential only ever go to the
/// one URL the caller passed (`github-pull-requests`: *GitHub Privacy and
/// Safety*).
pub(crate) fn post(url: &str, auth: Auth<'_>) -> RequestBuilder<WithBody> {
    NO_REDIRECT
        .post(url)
        .header("Authorization", auth.header_value())
}

/// Map a reply's status (and its `Retry-After` header, when present) onto a
/// [`Verdict`].
///
/// `retry_after` is the raw header value; a missing, non-numeric or negative
/// one degrades to `None` so the caller falls back to its own default backoff
/// rather than treating the reply as unusable.
pub(crate) fn classify(status: u16, retry_after: Option<&str>) -> Verdict {
    match status {
        200..=299 => Verdict::Read,
        401 => Verdict::Unauthenticated,
        403 => Verdict::Forbidden,
        404 => Verdict::NotFound,
        429 => Verdict::RateLimited {
            retry_after: retry_after.and_then(|h| h.trim().parse::<u64>().ok()),
        },
        _ => Verdict::Transient,
    }
}

/// Why a refresh left a snapshot unavailable or stale (`bitbucket-pull-requests`:
/// *Unavailable and Stale Snapshots Carry a Reason*). A closed set, and never
/// text taken from a reply, an error or a request, so the wire shape does not
/// depend on a library's wording and the panel owns its sentence. Mirrored in
/// `src/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UnavailableReason {
    /// The server's certificate chain was rejected: an unknown issuer, a bad
    /// signature, the wrong name, expired or not yet valid.
    CertificateUntrusted,
    /// Any other TLS failure.
    Tls,
    /// The host name did not resolve.
    Dns,
    /// The connection was refused, reset or unreachable.
    Connect,
    /// The request exceeded its deadline.
    Timeout,
    /// A non-success reply the poller maps neither to the unauthenticated
    /// state nor to a rate limit.
    HttpStatus { status: u16 },
    /// A reply that could not be read as the request expects.
    UnexpectedResponse,
    /// An HTTP 429, or the shared deadline holding the poller.
    RateLimited,
}

impl UnavailableReason {
    /// The reason in the words a standard-error line uses.
    pub(crate) fn words(self) -> String {
        match self {
            UnavailableReason::CertificateUntrusted => "certificate not trusted".to_string(),
            UnavailableReason::Tls => "TLS failure".to_string(),
            UnavailableReason::Dns => "host name did not resolve".to_string(),
            UnavailableReason::Connect => "connection failed".to_string(),
            UnavailableReason::Timeout => "timeout".to_string(),
            UnavailableReason::HttpStatus { status } => format!("HTTP {status}"),
            UnavailableReason::UnexpectedResponse => "unexpected response".to_string(),
            UnavailableReason::RateLimited => "rate limited".to_string(),
        }
    }
}

/// std's message for a failed host lookup on every Unix, where the error's
/// kind is the unnameable `Uncategorized`: the resolver's own words follow.
const LOOKUP_FAILED: &str = "failed to lookup address information";

/// Windows' host-lookup failures, which std reports as these raw codes
/// (`WSAHOST_NOT_FOUND` to `WSANO_DATA`). No Unix error number comes near
/// them, so they are read on every platform.
const WINDOWS_LOOKUP_FAILED: std::ops::RangeInclusive<i32> = 11001..=11004;

/// The reason a request that produced no reply failed, read off ureq's error
/// (`bitbucket-pull-requests`: *Unavailable and Stale Snapshots Carry a
/// Reason*). A rejected certificate can arrive two ways: as `Error::Rustls`,
/// or, since ureq 3.4.0 runs the handshake inside the stream, as `Error::Io`
/// wrapping the rustls error. Both are read, so neither is mistaken for a
/// refused connection.
pub(crate) fn classify_error(error: &ureq::Error) -> UnavailableReason {
    match error {
        ureq::Error::Rustls(error) => tls_reason(error),
        ureq::Error::Io(error) => io_reason(error),
        ureq::Error::Timeout(_) => UnavailableReason::Timeout,
        ureq::Error::HostNotFound => UnavailableReason::Dns,
        ureq::Error::StatusCode(status) => UnavailableReason::HttpStatus { status: *status },
        ureq::Error::Tls(_) | ureq::Error::Pem(_) | ureq::Error::TlsRequired => {
            UnavailableReason::Tls
        }
        // What the server sent could not be taken as a reply, or a link it
        // sent could not be requested.
        ureq::Error::Protocol(_)
        | ureq::Error::LargeResponseHeader(..)
        | ureq::Error::Decompress(..)
        | ureq::Error::BodyExceedsLimit(_)
        | ureq::Error::TooManyRedirects
        | ureq::Error::RedirectFailed
        | ureq::Error::BadUri(_)
        | ureq::Error::Http(_) => UnavailableReason::UnexpectedResponse,
        // `ConnectionFailed`, and every other way of ending up with no
        // connection that could carry the request: no proxy is ever
        // configured, so none of them can say anything more specific.
        _ => UnavailableReason::Connect,
    }
}

/// A TLS failure's reason: a rejected certificate chain, or anything else.
fn tls_reason(error: &rustls::Error) -> UnavailableReason {
    if matches!(error, rustls::Error::InvalidCertificate(_)) {
        UnavailableReason::CertificateUntrusted
    } else {
        UnavailableReason::Tls
    }
}

/// An I/O failure's reason: the TLS failure it wraps, when it wraps one,
/// else what its kind, or a failed host lookup's message, says.
fn io_reason(error: &io::Error) -> UnavailableReason {
    if let Some(tls) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
    {
        return tls_reason(tls);
    }
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => UnavailableReason::Timeout,
        io::ErrorKind::NotFound => UnavailableReason::Dns,
        _ if lookup_failed(error) => UnavailableReason::Dns,
        _ => UnavailableReason::Connect,
    }
}

/// Whether `error` is a failed host lookup as std reports one: by its message
/// on Unix, by its code on Windows.
fn lookup_failed(error: &io::Error) -> bool {
    error.to_string().starts_with(LOOKUP_FAILED)
        || error
            .raw_os_error()
            .is_some_and(|code| WINDOWS_LOOKUP_FAILED.contains(&code))
}

/// The error's own message, for the variants whose text carries nothing of
/// the request or the reply: a TLS or I/O failure's, a timeout's, a failed
/// lookup's or connection's. `None` for every other, since `BadUri` would
/// echo the URL and a protocol error may quote what the server sent.
fn transport_message(error: &ureq::Error) -> Option<String> {
    match error {
        ureq::Error::Rustls(error) => Some(error.to_string()),
        ureq::Error::Io(error) => Some(error.to_string()),
        ureq::Error::Timeout(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::Tls(_) => Some(error.to_string()),
        _ => None,
    }
}

/// A request that produced no reply: why, and the TLS or I/O error's own
/// message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransportFailure {
    pub(crate) reason: UnavailableReason,
    pub(crate) message: Option<String>,
}

impl TransportFailure {
    /// The failure ureq's `error` describes.
    pub(crate) fn of(error: &ureq::Error) -> Self {
        Self {
            reason: classify_error(error),
            message: transport_message(error),
        }
    }
}

/// One failed request, as its standard-error line reports it
/// (`bitbucket-pull-requests`: *Failures Are Reported on Standard Error*).
/// It holds no URL, header, body or query string: the request is named by its
/// path template, which the caller picks from the request's kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestFailure {
    pub(crate) reason: UnavailableReason,
    /// The request's path, with `{workspace}`, `{repo}`, `{user}` and `{id}`
    /// in place of the values.
    pub(crate) template: &'static str,
    /// For a transport failure, the TLS or I/O error's own message.
    pub(crate) message: Option<String>,
}

impl RequestFailure {
    /// A request to `template` that produced no reply.
    pub(crate) fn transport(template: &'static str, failure: TransportFailure) -> Self {
        Self {
            reason: failure.reason,
            template,
            message: failure.message,
        }
    }

    /// A request to `template` that failed for `reason`, with no error of its
    /// own: a reply's status, or a body that could not be read.
    pub(crate) fn because(template: &'static str, reason: UnavailableReason) -> Self {
        Self {
            reason,
            template,
            message: None,
        }
    }

    /// The reason in words, then the error's own message in brackets when
    /// there is one: `certificate not trusted (invalid peer certificate:
    /// UnknownIssuer)`.
    pub(crate) fn describe(&self) -> String {
        match &self.message {
            Some(message) => format!("{} ({message})", self.reason.words()),
            None => self.reason.words(),
        }
    }

    /// The whole line, for `provider`: `specforge: bitbucket: GET /2.0/user
    /// failed: timeout (timeout: global)`.
    pub(crate) fn line(&self, provider: &str) -> String {
        format!(
            "specforge: {provider}: GET {} failed: {}",
            self.template,
            self.describe()
        )
    }
}

/// A response as a transport hands it over, built with ureq's own test body,
/// so a reply reader is tested without a network.
#[cfg(test)]
pub(crate) fn test_response(
    status: u16,
    headers: &[(&str, &str)],
    body: impl Into<Vec<u8>>,
) -> ureq::http::Response<ureq::Body> {
    let mut response = ureq::http::Response::builder().status(status);
    for (name, value) in headers {
        response = response.header(*name, *value);
    }
    response.body(ureq::Body::builder().data(body)).unwrap()
}

/// An agent with [`get`]'s whole posture, redirects followed, trusting
/// `roots` in place of the process's set: the poller's transport, pointed at
/// a test's own root set.
#[cfg(test)]
pub(crate) fn following_agent_trusting(roots: RootCerts) -> Agent {
    new_agent(Redirects::Follow, roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn success_statuses_ask_for_the_body() {
        for status in [200, 201, 204, 299] {
            assert_eq!(classify(status, None), Verdict::Read, "status {status}");
        }
    }

    #[test]
    fn unauthorized_is_distinguished_from_other_client_errors() {
        assert_eq!(classify(401, None), Verdict::Unauthenticated);
        // 403 is *not* "your token is bad" — it must not blank the gauge's
        // authentication state. It gets its own verdict so the BitBucket
        // poller can skip one workspace; the quota pollers fold it back into
        // transient.
        assert_eq!(classify(403, None), Verdict::Forbidden);
    }

    #[test]
    fn forbidden_and_not_found_are_told_apart_from_their_neighbours() {
        assert_eq!(classify(403, None), Verdict::Forbidden);
        assert_eq!(classify(404, None), Verdict::NotFound);
        // Only those two codes: the statuses around them stay transient.
        for status in [400, 402, 405, 409, 410] {
            assert_eq!(
                classify(status, None),
                Verdict::Transient,
                "status {status}"
            );
        }
        // A `Retry-After` does not turn either into a backoff.
        assert_eq!(classify(403, Some("30")), Verdict::Forbidden);
        assert_eq!(classify(404, Some("30")), Verdict::NotFound);
    }

    /// RFC 7617's own example pair, so the expected value is not something
    /// this test computed with the code under test.
    #[test]
    fn basic_auth_encodes_username_colon_token() {
        let auth = Auth::Basic {
            username: "Aladdin",
            token: "open sesame",
        };
        assert_eq!(auth.header_value(), "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
    }

    #[test]
    fn bearer_auth_carries_the_token_verbatim() {
        assert_eq!(Auth::Bearer("tok-123").header_value(), "Bearer tok-123");
    }

    /// The builder carries the credential header itself, so no caller can
    /// forget it or spell the scheme differently.
    #[test]
    fn the_request_carries_the_authorization_header() {
        let request = get(
            "https://api.bitbucket.org/2.0/user",
            Auth::Basic {
                username: "Aladdin",
                token: "open sesame",
            },
        );
        let headers = request.headers_ref().expect("a well-formed request");
        assert_eq!(
            headers.get("Authorization").and_then(|v| v.to_str().ok()),
            Some("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
        );
    }

    /// The POST builder carries the credential the same way — the GitHub
    /// poller's Bearer token — so the header is built in one place for both
    /// methods.
    #[test]
    fn the_post_request_carries_the_bearer_header() {
        let request = post("https://api.github.com/graphql", Auth::Bearer("ghp_tok"));
        let headers = request.headers_ref().expect("a well-formed request");
        assert_eq!(
            headers.get("Authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer ghp_tok")
        );
    }

    /// The POST never follows a redirect: a 302 comes back as the reply, and
    /// the `Location` it names is never contacted — so neither the body nor
    /// the credential can travel anywhere but the URL the caller passed
    /// (`github-pull-requests`: *A redirect is not followed*). Hermetic: two
    /// loopback listeners, no network.
    #[test]
    fn a_post_never_follows_a_redirect() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let elsewhere = TcpListener::bind("127.0.0.1:0").unwrap();
        elsewhere.set_nonblocking(true).unwrap();
        let target = elsewhere.local_addr().unwrap();
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = origin.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = origin.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let reply = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{target}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(reply.as_bytes()).unwrap();
        });

        let reply = post(&format!("http://{addr}/graphql"), Auth::Bearer("tok"))
            .send("{}")
            .expect("a 302 is a reply, not an error");

        assert_eq!(reply.status().as_u16(), 302);
        server.join().unwrap();
        assert!(
            elsewhere.accept().is_err(),
            "the redirect target was never contacted"
        );
    }

    /// The detail reads' GET never follows a redirect either: a 302 comes
    /// back as the reply, the `Location` it names is never contacted, and the
    /// one request sent carried the credential (`bitbucket-pull-requests`:
    /// *A detail read follows no redirect*; `github-pull-requests`: *A detail
    /// read follows no redirect*). Hermetic: two loopback listeners per
    /// credential scheme, no network.
    #[test]
    fn a_detail_get_never_follows_a_redirect() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        for (auth, header) in [
            (
                Auth::Basic {
                    username: "Aladdin",
                    token: "open sesame",
                },
                "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==",
            ),
            (Auth::Bearer("ghp_tok"), "Bearer ghp_tok"),
        ] {
            let elsewhere = TcpListener::bind("127.0.0.1:0").unwrap();
            elsewhere.set_nonblocking(true).unwrap();
            let target = elsewhere.local_addr().unwrap();
            let origin = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = origin.local_addr().unwrap();
            // Answers one request with a redirect, and hands back its head.
            let server = std::thread::spawn(move || {
                let (mut stream, _) = origin.accept().unwrap();
                let mut head = Vec::new();
                let mut buf = [0u8; 4096];
                while !head.windows(4).any(|end| end == b"\r\n\r\n") {
                    let read = stream.read(&mut buf).unwrap();
                    if read == 0 {
                        break;
                    }
                    head.extend_from_slice(&buf[..read]);
                }
                let reply = format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{target}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                stream.write_all(reply.as_bytes()).unwrap();
                String::from_utf8(head).unwrap()
            });

            let reply = get_without_redirects(&format!("http://{addr}/2.0/diff"), auth)
                .call()
                .expect("a 302 is a reply, not an error");

            assert_eq!(reply.status().as_u16(), 302, "{header}");
            let head = server.join().unwrap();
            assert!(head.starts_with("GET /2.0/diff "), "{head}");
            let authorization = head.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim().to_string())
            });
            assert_eq!(authorization.as_deref(), Some(header));
            assert!(
                elsewhere.accept().is_err(),
                "the redirect target was never contacted"
            );
        }
    }

    /// Both agents carry the whole posture in their own `Config`, so every
    /// request inherits it. This is where the proxy contract and the idle
    /// limit are checked: setting `HTTPS_PROXY` in-process would race the
    /// other tests, and waiting out the idle age would be a timing assertion
    /// (`github-pull-requests`: *An exported proxy is ignored*, *A connection
    /// idle past the limit is not reused*; `bitbucket-pull-requests`: the same
    /// two scenarios). It also fails if a ureq upgrade moves a default this
    /// module relies on.
    #[test]
    fn the_agents_carry_the_posture() {
        for (name, agent) in [("FOLLOWING", &*FOLLOWING), ("NO_REDIRECT", &*NO_REDIRECT)] {
            let config = agent.config();
            assert!(config.proxy().is_none(), "{name}: no ambient proxy");
            assert!(!config.http_status_as_error(), "{name}: non-2xx is a reply");
            assert_eq!(config.timeouts().global, Some(REQUEST_TIMEOUT), "{name}");
            assert_eq!(config.max_idle_age(), IDLE_CONNECTION_AGE, "{name}");
            assert_eq!(config.max_idle_age(), Duration::from_secs(15), "{name}");
            assert_eq!(config.max_idle_connections(), 10, "{name}");
            assert_eq!(config.max_idle_connections_per_host(), 3, "{name}");
            // `outbound-https-trust`: both verify, against the process's one
            // root set of bundled and system roots, never ureq's default of
            // the Mozilla roots alone.
            let tls = config.tls_config();
            assert!(!tls.disable_verification(), "{name}: verification stays on");
            let (RootCerts::Specific(roots), RootCerts::Specific(shared)) =
                (tls.root_certs(), trust_roots::root_certs())
            else {
                panic!("{name}: specific roots");
            };
            assert!(Arc::ptr_eq(roots, &shared), "{name}: the one root set");
        }
        for agent in [&*FOLLOWING, &*NO_REDIRECT] {
            assert_eq!(
                agent.config().redirect_auth_headers(),
                RedirectAuthHeaders::Never,
                "a followed redirect never carries the credential"
            );
        }
        assert_eq!(NO_REDIRECT.config().max_redirects(), 0);
        assert!(FOLLOWING.config().max_redirects() > 0);
    }

    /// No builder carries a per-request `.config()`, which would make its
    /// request request-level and bypass the agent's cached TLS config
    /// (design D2). ureq stores a request-level override in the request's
    /// extensions as soon as any setter runs under `.config()`, so an
    /// agent-level request has none.
    #[test]
    fn the_builders_add_no_request_level_config() {
        let auth = Auth::Bearer("tok");
        let url = "https://api.github.com/graphql";
        assert_eq!(
            get(url, auth).extensions_ref().map(|e| e.is_empty()),
            Some(true),
            "get"
        );
        assert_eq!(
            get_without_redirects(url, auth)
                .extensions_ref()
                .map(|e| e.is_empty()),
            Some(true),
            "get_without_redirects"
        );
        assert_eq!(
            post(url, auth).extensions_ref().map(|e| e.is_empty()),
            Some(true),
            "post"
        );
    }

    /// One request as the keep-alive server read it off the stream.
    struct Received {
        head: String,
        body: Vec<u8>,
    }

    impl Received {
        fn authorization(&self) -> Option<&str> {
            self.head.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("authorization")
                    .then(|| value.trim())
            })
        }
    }

    /// Read one request — its head up to `\r\n\r\n`, then its `Content-Length`
    /// body — off `stream`, keeping any bytes past it in `pending`. `None`
    /// when the client closed the stream or the read timed out.
    fn read_request(stream: &mut std::net::TcpStream, pending: &mut Vec<u8>) -> Option<Received> {
        use std::io::Read;

        let mut buf = [0u8; 4096];
        let head_end = loop {
            if let Some(at) = pending.windows(4).position(|end| end == b"\r\n\r\n") {
                break at + 4;
            }
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => return None,
                Ok(read) => pending.extend_from_slice(&buf[..read]),
            }
        };
        let head = String::from_utf8(pending[..head_end].to_vec()).unwrap();
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        while pending.len() < head_end + length {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => return None,
                Ok(read) => pending.extend_from_slice(&buf[..read]),
            }
        }
        let body = pending[head_end..head_end + length].to_vec();
        pending.drain(..head_end + length);
        Some(Received { head, body })
    }

    /// A loopback server that accepts exactly one connection and answers one
    /// request per entry of `replies` on it, in order. Each reply should be a
    /// keep-alive one (`Content-Length` set, no `Connection: close`).
    ///
    /// The thread hands back what it read and the listener, so a test can
    /// check that no second connection was ever opened. The accepted stream
    /// has a read timeout past the client's own, so a client that opened a
    /// second connection fails through its timeout and a joined thread rather
    /// than leaving this one blocked.
    fn serve_keep_alive(
        replies: Vec<String>,
    ) -> (
        std::net::SocketAddr,
        std::thread::JoinHandle<(Vec<Received>, std::net::TcpListener)>,
    ) {
        use std::io::Write;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(REQUEST_TIMEOUT + Duration::from_secs(5)))
                .unwrap();
            let mut pending = Vec::new();
            let mut received = Vec::new();
            for reply in replies {
                let Some(request) = read_request(&mut stream, &mut pending) else {
                    break;
                };
                received.push(request);
                stream.write_all(reply.as_bytes()).unwrap();
            }
            (received, listener)
        });
        (addr, server)
    }

    const OK_KEEP_ALIVE: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";

    /// No second connection reached `listener` after the server's one.
    fn assert_no_second_connection(listener: &std::net::TcpListener) {
        listener.set_nonblocking(true).unwrap();
        let second = listener.accept().map(|_| ()).unwrap_err();
        assert_eq!(
            second.kind(),
            std::io::ErrorKind::WouldBlock,
            "a second connection was opened"
        );
    }

    /// Two `get`s in a row travel over one connection from the `FOLLOWING`
    /// pool, and each carries its own credential, nothing of the earlier one
    /// (`bitbucket-pull-requests`: *A poll cycle's requests share one
    /// connection*, *Replaced credentials are sent on a reused connection*;
    /// `github-pull-requests`: *A replaced token is sent on a reused
    /// connection*). A client that opened a second connection would never be
    /// answered and would time out, so a regression fails, never passes by
    /// chance.
    #[test]
    fn consecutive_gets_share_one_connection() {
        // The first reply sets a cookie: a shared agent must not carry it into
        // the next request (a jar would exist only if ureq's `cookies` feature
        // were ever unified on).
        let (addr, server) = serve_keep_alive(vec![
            "HTTP/1.1 200 OK\r\nSet-Cookie: session=first; Path=/\r\nContent-Length: 2\r\n\r\nok"
                .into(),
            OK_KEEP_ALIVE.into(),
        ]);

        for token in ["first", "second"] {
            let mut reply = get(&format!("http://{addr}/2.0/{token}"), Auth::Bearer(token))
                .call()
                .expect("a keep-alive reply");
            assert_eq!(reply.status().as_u16(), 200);
            assert_eq!(reply.body_mut().read_to_string().unwrap(), "ok");
        }

        let (received, listener) = server.join().unwrap();
        assert_eq!(received.len(), 2, "both requests on the one stream");
        assert!(received[0].head.starts_with("GET /2.0/first "));
        assert!(received[1].head.starts_with("GET /2.0/second "));
        assert_eq!(received[0].authorization(), Some("Bearer first"));
        assert_eq!(received[1].authorization(), Some("Bearer second"));
        assert!(!received[1].head.contains("first"), "{}", received[1].head);
        assert!(
            !received[1]
                .head
                .to_ascii_lowercase()
                .contains("\r\ncookie:"),
            "no cookie carried from the earlier reply: {}",
            received[1].head
        );
        assert_no_second_connection(&listener);
    }

    /// A detail read's GET and the GraphQL POST share the `NO_REDIRECT` pool,
    /// so they travel over one connection (`github-pull-requests`: *A detail
    /// read's requests share one connection*), and the POST's body arrives
    /// whole on it.
    #[test]
    fn a_detail_get_and_a_post_share_one_connection() {
        let (addr, server) = serve_keep_alive(vec![OK_KEEP_ALIVE.into(), OK_KEEP_ALIVE.into()]);

        let mut reply = get_without_redirects(&format!("http://{addr}/files"), Auth::Bearer("a"))
            .call()
            .expect("a keep-alive reply");
        assert_eq!(reply.body_mut().read_to_string().unwrap(), "ok");
        let mut reply = post(&format!("http://{addr}/graphql"), Auth::Bearer("b"))
            .send("{\"query\":\"q\"}")
            .expect("a keep-alive reply");
        assert_eq!(reply.body_mut().read_to_string().unwrap(), "ok");

        let (received, listener) = server.join().unwrap();
        assert_eq!(received.len(), 2, "both requests on the one stream");
        assert!(received[0].head.starts_with("GET /files "));
        assert_eq!(received[0].authorization(), Some("Bearer a"));
        assert!(received[1].head.starts_with("POST /graphql "));
        assert_eq!(received[1].authorization(), Some("Bearer b"));
        assert_eq!(received[1].body, b"{\"query\":\"q\"}");
        assert_no_second_connection(&listener);
    }

    /// Reuse does not relax the redirect rule: a detail read's 302 that
    /// arrives on a reused connection is the reply, and the `Location` it
    /// names is never contacted (`github-pull-requests`: *A reused connection
    /// follows no redirect*; `bitbucket-pull-requests`: *A reused connection
    /// follows no redirect for a detail read*).
    #[test]
    fn a_reused_connection_follows_no_redirect() {
        let elsewhere = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        elsewhere.set_nonblocking(true).unwrap();
        let target = elsewhere.local_addr().unwrap();
        let (addr, server) = serve_keep_alive(vec![
            OK_KEEP_ALIVE.into(),
            format!("HTTP/1.1 302 Found\r\nLocation: http://{target}/elsewhere\r\nContent-Length: 0\r\n\r\n"),
        ]);

        let mut reply = get_without_redirects(&format!("http://{addr}/pr"), Auth::Bearer("tok"))
            .call()
            .expect("a keep-alive reply");
        assert_eq!(reply.body_mut().read_to_string().unwrap(), "ok");
        let reply = get_without_redirects(&format!("http://{addr}/diff"), Auth::Bearer("tok"))
            .call()
            .expect("a 302 is a reply, not an error");
        assert_eq!(reply.status().as_u16(), 302);

        let (received, listener) = server.join().unwrap();
        assert_eq!(received.len(), 2, "both requests on the one stream");
        assert!(received[1].head.starts_with("GET /diff "));
        assert_no_second_connection(&listener);
        assert!(
            elsewhere.accept().is_err(),
            "the redirect target was never contacted"
        );
    }

    #[test]
    fn rate_limited_carries_the_hinted_delay() {
        assert_eq!(
            classify(429, Some("120")),
            Verdict::RateLimited {
                retry_after: Some(120)
            }
        );
    }

    #[test]
    fn rate_limited_tolerates_a_padded_retry_after() {
        assert_eq!(
            classify(429, Some("  90 ")),
            Verdict::RateLimited {
                retry_after: Some(90)
            }
        );
    }

    #[test]
    fn rate_limited_without_a_usable_hint_falls_back_to_none() {
        // Absent, non-numeric, and the HTTP-date form (which ureq hands over
        // verbatim) all mean "no integer delay" — the caller's own default
        // backoff applies.
        for header in [None, Some("soon"), Some(""), Some("-5")] {
            assert_eq!(
                classify(429, header),
                Verdict::RateLimited { retry_after: None },
                "header {header:?}"
            );
        }
        assert_eq!(
            classify(429, Some("Wed, 21 Oct 2026 07:28:00 GMT")),
            Verdict::RateLimited { retry_after: None }
        );
    }

    #[test]
    fn server_errors_and_redirects_are_transient() {
        for status in [301, 302, 400, 500, 502, 503] {
            assert_eq!(
                classify(status, None),
                Verdict::Transient,
                "status {status}"
            );
        }
    }

    #[test]
    fn a_retry_after_on_a_non_429_is_ignored() {
        // Only the 429 branch reads the header; a 503 carrying one is still
        // just transient, with no backoff hint smuggled through.
        assert_eq!(classify(503, Some("30")), Verdict::Transient);
    }

    // ---------------------------------------------------- failure reasons

    /// A rejected certificate inside an I/O error, as ureq 3.4.0 surfaces one.
    fn wrapped(error: rustls::Error) -> ureq::Error {
        ureq::Error::Io(io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn untrusted(error: rustls::CertificateError) -> rustls::Error {
        rustls::Error::InvalidCertificate(error)
    }

    /// Every error ureq can raise lands in the reason it names
    /// (`bitbucket-pull-requests`: *Unavailable and Stale Snapshots Carry a
    /// Reason*).
    #[test]
    fn each_error_classifies_into_its_reason() {
        use rustls::CertificateError::UnknownIssuer;
        use rustls::CertificateError::{BadSignature, Expired, NotValidForName, NotValidYet};
        use UnavailableReason::*;

        let kind = |kind: io::ErrorKind| ureq::Error::Io(io::Error::from(kind));
        let cases = [
            (
                ureq::Error::Rustls(untrusted(UnknownIssuer)),
                CertificateUntrusted,
            ),
            (
                ureq::Error::Rustls(untrusted(BadSignature)),
                CertificateUntrusted,
            ),
            (
                ureq::Error::Rustls(untrusted(NotValidForName)),
                CertificateUntrusted,
            ),
            (
                ureq::Error::Rustls(untrusted(Expired)),
                CertificateUntrusted,
            ),
            (
                ureq::Error::Rustls(untrusted(NotValidYet)),
                CertificateUntrusted,
            ),
            (
                ureq::Error::Rustls(rustls::Error::HandshakeNotComplete),
                Tls,
            ),
            (wrapped(untrusted(UnknownIssuer)), CertificateUntrusted),
            (
                wrapped(rustls::Error::AlertReceived(
                    rustls::AlertDescription::HandshakeFailure,
                )),
                Tls,
            ),
            (ureq::Error::Tls("Rustls invalid dns name error"), Tls),
            (
                ureq::Error::Pem(rustls::pki_types::pem::Error::NoItemsFound),
                Tls,
            ),
            (ureq::Error::TlsRequired, Tls),
            (ureq::Error::HostNotFound, Dns),
            (kind(io::ErrorKind::NotFound), Dns),
            (
                ureq::Error::Io(io::Error::other(
                    "failed to lookup address information: nodename nor servname provided, \
                     or not known",
                )),
                Dns,
            ),
            (ureq::Error::Io(io::Error::from_raw_os_error(11001)), Dns),
            (ureq::Error::Io(io::Error::from_raw_os_error(11004)), Dns),
            (ureq::Error::ConnectionFailed, Connect),
            (kind(io::ErrorKind::ConnectionRefused), Connect),
            (kind(io::ErrorKind::ConnectionReset), Connect),
            (kind(io::ErrorKind::UnexpectedEof), Connect),
            (ureq::Error::Io(io::Error::other("something else")), Connect),
            (
                ureq::Error::Io(io::Error::from_raw_os_error(11000)),
                Connect,
            ),
            (
                ureq::Error::Io(io::Error::from_raw_os_error(11005)),
                Connect,
            ),
            (
                ureq::Error::ConnectProxyFailed("refused".to_string()),
                Connect,
            ),
            (ureq::Error::InvalidProxyUrl, Connect),
            (ureq::Error::BodyStalled, Connect),
            (ureq::Error::Timeout(ureq::Timeout::Global), Timeout),
            (kind(io::ErrorKind::TimedOut), Timeout),
            (kind(io::ErrorKind::WouldBlock), Timeout),
            (ureq::Error::StatusCode(503), HttpStatus { status: 503 }),
            (
                ureq::Error::LargeResponseHeader(70_000, 65_536),
                UnexpectedResponse,
            ),
            (
                ureq::Error::Decompress("gzip", io::Error::other("corrupt")),
                UnexpectedResponse,
            ),
            (ureq::Error::BodyExceedsLimit(10), UnexpectedResponse),
            (ureq::Error::TooManyRedirects, UnexpectedResponse),
            (ureq::Error::RedirectFailed, UnexpectedResponse),
            (ureq::Error::BadUri("x".to_string()), UnexpectedResponse),
            (
                ureq::Error::Http(
                    ureq::http::Request::builder()
                        .uri("http://[")
                        .body(())
                        .unwrap_err(),
                ),
                UnexpectedResponse,
            ),
        ];
        for (error, reason) in cases {
            assert_eq!(classify_error(&error), reason, "{error:?}");
        }
    }

    /// A transport failure keeps the TLS or I/O error's own words, which name
    /// nothing of the request; an error whose text could quote the URL or the
    /// server keeps none.
    #[test]
    fn only_messages_that_carry_no_request_data_are_kept() {
        let message = |error: ureq::Error| transport_message(&error);
        assert_eq!(
            message(ureq::Error::Rustls(untrusted(
                rustls::CertificateError::UnknownIssuer
            )))
            .as_deref(),
            Some("invalid peer certificate: UnknownIssuer")
        );
        assert_eq!(
            message(wrapped(untrusted(rustls::CertificateError::UnknownIssuer))).as_deref(),
            Some("invalid peer certificate: UnknownIssuer"),
            "the I/O wrapper adds nothing"
        );
        assert_eq!(
            message(ureq::Error::Io(io::Error::from(
                io::ErrorKind::ConnectionRefused
            )))
            .as_deref(),
            Some("connection refused")
        );
        for (error, expected) in [
            (
                ureq::Error::Timeout(ureq::Timeout::Global),
                "timeout: global",
            ),
            (ureq::Error::HostNotFound, "host not found"),
            (ureq::Error::ConnectionFailed, "connection failed"),
            (
                ureq::Error::Tls("Rustls invalid dns name error"),
                "Rustls invalid dns name error",
            ),
        ] {
            assert_eq!(message(error).as_deref(), Some(expected));
        }
        for error in [
            ureq::Error::BadUri("https://api.bitbucket.org/2.0/workspaces/acme".to_string()),
            ureq::Error::RequireHttpsOnly("http://api.bitbucket.org/2.0/user".to_string()),
            ureq::Error::ConnectProxyFailed("acme".to_string()),
            ureq::Error::StatusCode(500),
            ureq::Error::TooManyRedirects,
        ] {
            assert_eq!(message(error), None);
        }
        let failure =
            TransportFailure::of(&wrapped(untrusted(rustls::CertificateError::UnknownIssuer)));
        assert_eq!(
            failure,
            TransportFailure {
                reason: UnavailableReason::CertificateUntrusted,
                message: Some("invalid peer certificate: UnknownIssuer".to_string()),
            }
        );
    }

    #[test]
    fn each_reason_has_its_words() {
        use UnavailableReason::*;
        for (reason, words) in [
            (CertificateUntrusted, "certificate not trusted"),
            (Tls, "TLS failure"),
            (Dns, "host name did not resolve"),
            (Connect, "connection failed"),
            (Timeout, "timeout"),
            (HttpStatus { status: 503 }, "HTTP 503"),
            (UnexpectedResponse, "unexpected response"),
            (RateLimited, "rate limited"),
        ] {
            assert_eq!(reason.words(), words);
        }
    }

    /// `bitbucket-pull-requests`: *Failures Are Reported on Standard Error* —
    /// the provider, the method, the template and the reason, then the
    /// error's own message for a transport failure.
    #[test]
    fn a_line_names_the_provider_the_method_the_template_and_the_reason() {
        let tls = RequestFailure::transport(
            "/2.0/user",
            TransportFailure {
                reason: UnavailableReason::CertificateUntrusted,
                message: Some("invalid peer certificate: UnknownIssuer".to_string()),
            },
        );
        assert_eq!(
            tls.describe(),
            "certificate not trusted (invalid peer certificate: UnknownIssuer)"
        );
        assert_eq!(
            tls.line("bitbucket"),
            "specforge: bitbucket: GET /2.0/user failed: certificate not trusted \
             (invalid peer certificate: UnknownIssuer)"
        );
        let status = RequestFailure::because(
            "/2.0/workspaces/{workspace}/pullrequests/{user}",
            UnavailableReason::HttpStatus { status: 500 },
        );
        assert_eq!(status.message, None);
        assert_eq!(
            status.line("bitbucket"),
            "specforge: bitbucket: GET /2.0/workspaces/{workspace}/pullrequests/{user} failed: \
             HTTP 500"
        );
    }

    #[test]
    fn a_reason_crosses_the_wire_as_its_kind() {
        assert_eq!(
            serde_json::to_value(UnavailableReason::HttpStatus { status: 503 }).unwrap(),
            serde_json::json!({ "kind": "httpStatus", "status": 503 })
        );
        assert_eq!(
            serde_json::to_value(UnavailableReason::CertificateUntrusted).unwrap(),
            serde_json::json!({ "kind": "certificateUntrusted" })
        );
    }

    // ------------------------------------------- real TLS, over loopback

    /// A loopback TLS server presenting `identity`, for one connection. The
    /// thread hands back the request head it read, or `None` when the client
    /// refused the handshake, so no request was ever sent over it.
    fn serve_tls(
        identity: trust_roots::fixtures::Identity,
    ) -> (
        std::net::SocketAddr,
        std::thread::JoinHandle<Option<String>>,
    ) {
        use std::io::{Read, Write};

        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(identity.chain, identity.key)
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(REQUEST_TIMEOUT + Duration::from_secs(5)))
                .unwrap();
            let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
            let mut tls = rustls::StreamOwned::new(connection, stream);
            let mut head = Vec::new();
            let mut buf = [0u8; 4096];
            while !head.windows(4).any(|end| end == b"\r\n\r\n") {
                match tls.read(&mut buf) {
                    Ok(0) | Err(_) => return None,
                    Ok(read) => head.extend_from_slice(&buf[..read]),
                }
            }
            tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            tls.conn.send_close_notify();
            let _ = tls.flush();
            Some(String::from_utf8(head).unwrap())
        });
        (addr, server)
    }

    /// One GET of `/2.0/user` on `addr`, verified against `roots`.
    fn get_over_tls(
        addr: std::net::SocketAddr,
        roots: RootCerts,
    ) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
        new_agent(Redirects::Never, roots)
            .get(&format!("https://{addr}/2.0/user"))
            .call()
    }

    /// `outbound-https-trust`: *An untrusted chain is still refused* and *A
    /// corporate root in the system store is trusted* — the same server,
    /// signed by a root only a system store could hold, is refused before any
    /// request without that root, and answers once the store holds it.
    #[test]
    fn a_root_in_the_system_store_is_trusted_and_refused_without_it() {
        use trust_roots::fixtures::{certificate, localhost, roots_with, CA};

        for roots in [roots_with(Vec::new()), trust_roots::root_certs()] {
            let (addr, server) = serve_tls(localhost());
            let error = get_over_tls(addr, roots).expect_err("an untrusted chain is refused");
            assert_eq!(
                classify_error(&error),
                UnavailableReason::CertificateUntrusted,
                "{error:?}"
            );
            assert_eq!(server.join().unwrap(), None, "no request was sent over it");
        }

        let (addr, server) = serve_tls(localhost());
        let mut reply = get_over_tls(addr, roots_with(vec![certificate(CA)]))
            .expect("a chain to a system root is trusted");
        assert_eq!(reply.status().as_u16(), 200);
        assert_eq!(reply.body_mut().read_to_string().unwrap(), "ok");
        let head = server.join().unwrap().expect("the request arrived");
        assert!(head.starts_with("GET /2.0/user "), "{head}");
    }

    /// An untrusted self-signed certificate is `certificateUntrusted`, never
    /// mistaken for a refused connection (design D2). ureq 3.4.0 runs the
    /// handshake inside the stream, so the rejection arrives as an I/O error
    /// wrapping rustls's — this pins that down.
    #[test]
    fn a_self_signed_certificate_is_untrusted_not_a_failed_connection() {
        let (addr, server) = serve_tls(trust_roots::fixtures::self_signed());
        let error = get_over_tls(addr, trust_roots::root_certs())
            .expect_err("a self-signed certificate is refused");
        assert!(matches!(error, ureq::Error::Io(_)), "{error:?}");
        assert_eq!(
            TransportFailure::of(&error),
            TransportFailure {
                reason: UnavailableReason::CertificateUntrusted,
                message: Some("invalid peer certificate: UnknownIssuer".to_string()),
            }
        );
        assert_eq!(server.join().unwrap(), None);
    }

    /// A closed local port is `connect`; a host that cannot resolve is `dns`.
    #[test]
    fn a_closed_port_is_connect_and_an_unresolvable_host_is_dns() {
        let roots = || trust_roots::fixtures::roots_with(Vec::new());
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let error = get_over_tls(closed, roots()).expect_err("nothing listens there");
        assert_eq!(
            classify_error(&error),
            UnavailableReason::Connect,
            "{error:?}"
        );

        let error = new_agent(Redirects::Never, roots())
            .get("https://specforge-test.invalid/2.0/user")
            .call()
            .expect_err("an .invalid host never resolves");
        assert_eq!(classify_error(&error), UnavailableReason::Dns, "{error:?}");
    }

    /// A reply that is not HTTP at all is an unexpected response, and its
    /// error keeps no message, since one could quote what the server sent.
    #[test]
    fn a_reply_that_is_not_http_is_an_unexpected_response() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            stream
                .write_all(b"SSH-2.0-OpenSSH_9.9 secret\r\n\r\n")
                .unwrap();
        });
        let error = new_agent(
            Redirects::Never,
            trust_roots::fixtures::roots_with(Vec::new()),
        )
        .get(&format!("http://{addr}/2.0/user"))
        .call()
        .expect_err("not an HTTP reply");
        server.join().unwrap();
        assert!(matches!(error, ureq::Error::Protocol(_)), "{error:?}");
        assert_eq!(
            TransportFailure::of(&error),
            TransportFailure {
                reason: UnavailableReason::UnexpectedResponse,
                message: None,
            }
        );
    }
}
