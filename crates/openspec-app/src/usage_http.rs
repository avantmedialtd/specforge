//! The HTTP shape shared by the two usage-quota pollers (`crate::quota` and
//! `crate::chatgpt_quota`).
//!
//! Both pollers make the same request — one blocking GET to a vendor's
//! undocumented usage endpoint, with a bearer token — and read the same four
//! outcomes off the reply. Only the URL, the extra headers, and the body parser
//! differ, so the request posture and the status mapping live here once.
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

use ureq::typestate::WithoutBody;
use ureq::RequestBuilder;

/// Network timeout for a single usage request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// What one usage reply amounts to, before a poller maps it onto its own
/// `FetchResult`. Deliberately vendor-agnostic: the two pollers hold different
/// state types, but they agree on these four outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// 2xx — read and parse the body.
    Read,
    /// The endpoint rejected the token (401).
    Unauthenticated,
    /// Rate-limited (429); back off for the hinted (or default) delay.
    RateLimited { retry_after: Option<u64> },
    /// Anything else. Transient from a gauge's point of view: keep showing the
    /// last known snapshot rather than blanking it.
    Transient,
}

/// A GET builder carrying the posture both pollers want.
///
/// `http_status_as_error(false)` is what keeps a 429's `Retry-After` reachable
/// — see the module note.
///
/// `proxy(None)` pins ureq 2's behaviour. ureq 3's `Config::default()` calls
/// `Proxy::try_from_env()`, whereas ureq 2 only honoured `HTTPS_PROXY`/
/// `ALL_PROXY` under its non-default `proxy-from-env` feature, which this crate
/// never enabled. Leaving the new default in place would silently start routing
/// these requests — and the `Authorization: Bearer` header they carry — through
/// whatever proxy a user happens to have exported. Adopting env-proxy support
/// may well be worth doing, but it is a deliberate decision about where a
/// credential travels, not a side effect of a version bump.
pub(crate) fn get(url: &str) -> RequestBuilder<WithoutBody> {
    ureq::get(url)
        .config()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .http_status_as_error(false)
        .proxy(None)
        .build()
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
        // authentication state.
        assert_eq!(classify(403, None), Verdict::Transient);
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
        for status in [301, 302, 400, 404, 500, 502, 503] {
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
