//! The HTTP shape shared by the app's pollers: the two usage-quota pollers
//! (`crate::quota` and `crate::chatgpt_quota`) and the BitBucket pull-request
//! poller (`crate::bitbucket`).
//!
//! Every poller makes the same kind of request — a blocking, authenticated GET
//! — and reads the same outcomes off the reply. Only the URL, the credential
//! scheme, the extra headers, and the body parser differ, so the request
//! posture, the `Authorization` header and the status mapping live here once.
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

use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use ureq::typestate::WithoutBody;
use ureq::RequestBuilder;

/// Network timeout for a single poller request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

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
/// `auth`.
///
/// `http_status_as_error(false)` is what keeps a 429's `Retry-After` reachable
/// — see the module note.
///
/// `proxy(None)` pins ureq 2's behaviour. ureq 3's `Config::default()` calls
/// `Proxy::try_from_env()`, whereas ureq 2 only honoured `HTTPS_PROXY`/
/// `ALL_PROXY` under its non-default `proxy-from-env` feature, which this crate
/// never enabled. Leaving the new default in place would silently start routing
/// these requests — and the `Authorization` header they carry — through
/// whatever proxy a user happens to have exported. Adopting env-proxy support
/// may well be worth doing, but it is a deliberate decision about where a
/// credential travels, not a side effect of a version bump.
pub(crate) fn get(url: &str, auth: Auth<'_>) -> RequestBuilder<WithoutBody> {
    ureq::get(url)
        .config()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .proxy(None)
        .build()
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
