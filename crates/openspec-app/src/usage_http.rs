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
//! (`tls/rustls.rs`), so every new connection rebuilt it with the webpki
//! trust store. Now the requests of one burst (a detail read, a file or image
//! read, a BitBucket poll cycle) share a pooled connection, and each agent
//! builds its TLS config once (`github-pull-requests`: *GitHub Connection
//! Reuse*; `bitbucket-pull-requests`: *Connection Reuse*).
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

use std::sync::LazyLock;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use ureq::config::RedirectAuthHeaders;
use ureq::typestate::{WithBody, WithoutBody};
use ureq::{Agent, RequestBuilder};

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
///
/// No cookie jar exists to carry state between requests (ureq's `cookies`
/// feature is off), and `Authorization` is a header on each request, never
/// agent state. Keep this free of logic the posture test cannot observe.
fn new_agent(redirects: Redirects) -> Agent {
    let config = Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .proxy(None)
        .redirect_auth_headers(RedirectAuthHeaders::Never)
        .max_idle_connections(MAX_IDLE_CONNECTIONS)
        .max_idle_connections_per_host(MAX_IDLE_CONNECTIONS_PER_HOST)
        .max_idle_age(IDLE_CONNECTION_AGE);
    match redirects {
        Redirects::Follow => config,
        Redirects::Never => config.max_redirects(0),
    }
    .build()
    .new_agent()
}

/// The agent behind the pollers' [`get`]: redirects followed, without the
/// `Authorization` header.
static FOLLOWING: LazyLock<Agent> = LazyLock::new(|| new_agent(Redirects::Follow));

/// The agent behind [`get_without_redirects`] and [`post`]: no redirect is
/// ever followed, so a request, its body and its credential only ever go to
/// the one URL the caller passed (`github-pull-requests`: *GitHub Privacy and
/// Safety*; `bitbucket-pull-requests`: *Privacy and Safety*). A GitHub detail
/// read's POST and its files GETs share it, and so its connections.
static NO_REDIRECT: LazyLock<Agent> = LazyLock::new(|| new_agent(Redirects::Never));

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
