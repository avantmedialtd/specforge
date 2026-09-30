//! Opt-in GitHub pull-request tracking.
//!
//! Lists the open pull requests the token's account authored, and the open pull
//! requests awaiting its review, for the desktop app and the browser skin to
//! render as a side-pane panel (the `github-pull-requests` capability). A
//! structural twin of `crate::bitbucket` — its own recipe, snapshot, handle and
//! poll loop — sharing only the row types in `crate::pull_requests`, as
//! `crate::chatgpt_quota` twins `crate::quota` (`github-pull-requests-panel`
//! design D1).
//!
//! One refresh is one request: a POST of [`QUERY`], a GraphQL query fixed at
//! build time, to `https://api.github.com/graphql` (design D2). Nothing supplied
//! at runtime is interpolated into it, and it is a `query`, never a mutation.
//!
//! Everything is gated behind the `github.enabled` setting: with the feature
//! off, no token is read and no request is made. The token travels only to
//! [`API_URL`], only in the `Authorization` header that `usage_http::Auth`
//! builds, never through an ambient proxy, never across a redirect
//! (`usage_http::post` pins both), and is never formatted into any other string
//! — no outcome or state in this module carries text at all.
//!
//! The pure parts — the request body, token resolution, the rate-limit reading,
//! the verdict, the parser and row mapping, the state transitions and the
//! refresh schedule — are separate functions with their own tests, so the
//! mutation gate has assertions to catch. The loop itself is thin, and runs on
//! a plain `std::thread` like the other pollers so the app layer stays
//! runtime-agnostic.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openspec_core::{CacheEvent, WatcherManager};
use serde::Serialize;
use serde_json::Value;

use crate::pull_requests::{
    merge_newest_first, saturating_u32, ChecksState, PullRequestSummary, PullRequestsStatus,
    ReviewSummary,
};
use crate::quota::parse_rfc3339_to_unix;
use crate::settings::SettingsStore;
use crate::usage_http::{self, Auth, Verdict};

/// The GraphQL endpoint. The only destination the token is ever sent to.
const API_URL: &str = "https://api.github.com/graphql";
/// GitHub rejects requests without a `User-Agent`; identify SpecForge as the
/// other pollers do.
const USER_AGENT: &str = concat!("SpecForge/", env!("CARGO_PKG_VERSION"));
/// Poller wake cadence — see `quota.rs`'s identical constant.
const TICK: Duration = Duration::from_secs(2);
/// Floor for the configurable refresh interval, as for BitBucket.
const MIN_REFRESH_SECS: u64 = 60;
/// Fallback backoff when a rate-limited reply carries neither `Retry-After`
/// nor `x-ratelimit-reset`. Above GitHub's "wait at least one minute" for a
/// secondary limit without hints.
const DEFAULT_BACKOFF_SECS: u64 = 300;
/// Ceiling on any rate-limit backoff. GitHub's primary window is an hour, so
/// no honest `Retry-After` or reset lies further out; the cap is what keeps a
/// hostile or corrupt header (`Retry-After: 18446744073709551615`) from
/// overflowing `Instant + Duration` and killing the poller thread.
const MAX_BACKOFF_SECS: u64 = 3_600;
/// Where a row's web page may live. The desktop opener hands a row's URL to the
/// OS, so only a link on GitHub's own site survives into a row.
const WEB_URL_PREFIX: &str = "https://github.com/";
/// The environment variables that override the stored token, in the GitHub
/// CLI's own order of precedence, so one shell profile serves both tools.
const ENV_GH_TOKEN: &str = "GH_TOKEN";
const ENV_GITHUB_TOKEN: &str = "GITHUB_TOKEN";

/// The one request a refresh sends, fixed at build time (design D2).
///
/// Authored rows come from `viewer.pullRequests`, which has no search-index lag
/// and no archived qualifier (archived repositories are filtered in the
/// parser); review requests come from search, where `review-requested:@me`
/// includes requests to the viewer's teams and `archived:false` filters
/// server-side. It costs 4 of the 5,000 hourly GraphQL points.
pub(crate) const QUERY: &str = r#"fragment Row on PullRequest {
  number title url isDraft updatedAt
  author { login }
  repository { nameWithOwner isArchived }
  headRepository { nameWithOwner }
  headRefName baseRefName mergeable
  reviewRequests(first: 20) { totalCount }
  latestOpinionatedReviews(first: 20) { nodes { state author { login } } }
  reviewThreads(first: 100) { nodes { isResolved } }
  commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
}
query SpecForgePullRequests {
  viewer {
    pullRequests(states: OPEN, first: 50, orderBy: { field: UPDATED_AT, direction: DESC }) { nodes { ...Row } }
  }
  reviewRequested: search(query: "is:pr is:open archived:false review-requested:@me sort:updated-desc", type: ISSUE, first: 50) {
    nodes { ...Row }
  }
}
"#;

/// The request body: the constant query and nothing else — no `variables`, so
/// nothing at runtime can steer it.
fn request_body() -> String {
    serde_json::json!({ "query": QUERY }).to_string()
}

// ---- the snapshot ----

/// The snapshot every frontend renders (`github-pull-requests`: *GitHub
/// Pull-Request Panel*). `stale` marks rows kept from an earlier refresh after a
/// transient failure or a rate limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubPullRequestsState {
    pub status: PullRequestsStatus,
    pub stale: bool,
    /// When the lists were fetched, as Unix epoch seconds — kept through a
    /// stale period. `None` when there are no lists at all.
    pub fetched_at_unix: Option<u64>,
    /// The account's own open pull requests, newest-updated first.
    pub authored: Vec<PullRequestSummary>,
    /// Open pull requests awaiting the account's review (directly or through a
    /// team), newest-updated first, never repeating an authored row.
    pub review_requested: Vec<PullRequestSummary>,
    /// Entries GitHub withheld (returned as `null`) — typically an organisation
    /// enforcing single sign-on the token is not authorised for. Informational,
    /// never an error.
    pub withheld: u32,
}

impl GithubPullRequestsState {
    /// The initial / disabled snapshot: nothing to show.
    pub fn disabled() -> Self {
        Self::status_only(PullRequestsStatus::Disabled)
    }

    /// A snapshot carrying only a status (no rows).
    fn status_only(status: PullRequestsStatus) -> Self {
        Self {
            status,
            stale: false,
            fetched_at_unix: None,
            authored: Vec::new(),
            review_requested: Vec::new(),
            withheld: 0,
        }
    }

    /// Every row in the snapshot, authored first.
    pub fn rows(&self) -> impl Iterator<Item = &PullRequestSummary> {
        self.authored.iter().chain(self.review_requested.iter())
    }
}

/// Cheaply-cloneable handle to the latest snapshot, shared between the poller
/// (writer) and the frontends (readers) — the `BitbucketPullRequestsHandle`
/// model.
#[derive(Clone)]
pub struct GithubPullRequestsHandle(Arc<Mutex<GithubPullRequestsState>>);

impl GithubPullRequestsHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(GithubPullRequestsState::disabled())))
    }

    /// The current snapshot.
    pub fn get(&self) -> GithubPullRequestsState {
        self.0.lock().unwrap().clone()
    }

    pub(crate) fn set(&self, state: GithubPullRequestsState) {
        *self.0.lock().unwrap() = state;
    }
}

impl Default for GithubPullRequestsHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---- the token ----

/// The token a refresh authenticates with: `GH_TOKEN`, else `GITHUB_TOKEN`,
/// else the stored token, skipping a variable that is absent or empty
/// (`github-pull-requests`: *The GitHub Token Is Stored Write-Only*). `None`
/// means unauthenticated.
///
/// `env` is the environment lookup, injected so the rule is testable without
/// mutating the process environment.
fn resolve_token(env: impl Fn(&str) -> Option<String>, stored: Option<String>) -> Option<String> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty());
    var(ENV_GH_TOKEN)
        .or_else(|| var(ENV_GITHUB_TOKEN))
        .or(stored)
}

// ---- rate limits and the verdict ----

/// The rate-limit headers of one reply, read off every reply whatever its
/// status: GitHub reports a GraphQL primary-limit hit as a 200 with the
/// exhausted headers beside the error (design D4).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RateHeaders {
    /// `Retry-After`, seconds.
    retry_after: Option<u64>,
    /// `x-ratelimit-remaining`.
    remaining: Option<u64>,
    /// `x-ratelimit-reset`, Unix epoch seconds.
    reset: Option<u64>,
}

impl RateHeaders {
    /// Parse the raw header values; a missing, non-numeric or negative one is
    /// `None`, as `usage_http::classify` treats `Retry-After`.
    fn from_raw(retry_after: Option<&str>, remaining: Option<&str>, reset: Option<&str>) -> Self {
        let number = |raw: Option<&str>| raw.and_then(|v| v.trim().parse::<u64>().ok());
        Self {
            retry_after: number(retry_after),
            remaining: number(remaining),
            reset: number(reset),
        }
    }
}

/// How long a rate-limited reply defers the next refresh: `Retry-After`, else —
/// only when `x-ratelimit-remaining` is `0` — the time until
/// `x-ratelimit-reset` (zero when it has passed), else `None` so the loop's
/// default applies (`github-pull-requests`: *GitHub Failure Classification*).
///
/// GitHub sends a reset on every reply, including a secondary-limit 403 whose
/// primary quota is nowhere near spent; that reset dates the primary window,
/// not the secondary limit, so honouring it would wait up to an hour for a
/// limit GitHub asks to be retried after a minute or so.
fn rate_limit_delay(headers: &RateHeaders, now_unix: u64) -> Option<u64> {
    headers.retry_after.or_else(|| {
        headers
            .reset
            .filter(|_| headers.remaining == Some(0))
            .map(|reset| reset.saturating_sub(now_unix))
    })
}

/// Whether a body says a rate limit was hit — GitHub's primary ("API rate
/// limit exceeded") and secondary ("exceeded a secondary rate limit") messages
/// both say so.
fn body_reports_rate_limit(body: &str) -> bool {
    body.to_ascii_lowercase().contains("rate limit")
}

/// Outcome of one refresh.
#[derive(Debug, PartialEq, Eq)]
enum FetchResult {
    /// The two lists, and how many entries GitHub withheld.
    Ok {
        authored: Vec<PullRequestSummary>,
        review_requested: Vec<PullRequestSummary>,
        withheld: u32,
    },
    /// No token, a 401, a 403 that is not a rate limit, or a missing scope.
    Unauthenticated,
    /// A 2xx whose body carries no usable data.
    Unavailable,
    /// Any form of rate limit; back off for the delay (or the default).
    RateLimited { retry_after: Option<u64> },
    /// A transport error, a redirect, or any other status — keep the last rows.
    Transient,
}

/// Classify one reply (design D4). `usage_http::classify` names the status; the
/// GitHub-specific part is telling a rate limit from a credential problem on a
/// 403, and reading a 2xx's body.
fn github_verdict(
    status: u16,
    headers: &RateHeaders,
    body: Option<&str>,
    now_unix: u64,
) -> FetchResult {
    let rate_limited = || FetchResult::RateLimited {
        retry_after: rate_limit_delay(headers, now_unix),
    };
    match usage_http::classify(status, None) {
        Verdict::Read => match body {
            Some(body) => parse_response(body, headers, now_unix),
            None => FetchResult::Unavailable,
        },
        Verdict::Unauthenticated => FetchResult::Unauthenticated,
        Verdict::RateLimited { .. } => rate_limited(),
        Verdict::Forbidden => {
            let signalled = headers.retry_after.is_some()
                || headers.remaining == Some(0)
                || body.is_some_and(body_reports_rate_limit);
            if signalled {
                rate_limited()
            } else {
                FetchResult::Unauthenticated
            }
        }
        Verdict::NotFound | Verdict::Transient => FetchResult::Transient,
    }
}

// ---- the parser ----

/// Read a 2xx body into a result (`github-pull-requests`: *One Constant
/// Read-Only Query*, *GitHub Failure Classification*).
fn parse_response(body: &str, headers: &RateHeaders, now_unix: u64) -> FetchResult {
    let Ok(json) = serde_json::from_str::<Value>(body) else {
        return FetchResult::Unavailable;
    };
    let has_error = |kind: &str| {
        json.get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| {
                errors
                    .iter()
                    .any(|e| e.get("type").and_then(Value::as_str) == Some(kind))
            })
    };
    if has_error("RATE_LIMITED") {
        return FetchResult::RateLimited {
            retry_after: rate_limit_delay(headers, now_unix),
        };
    }
    let no_data = || {
        if has_error("INSUFFICIENT_SCOPES") {
            FetchResult::Unauthenticated
        } else {
            FetchResult::Unavailable
        }
    };
    let Some(data) = json.get("data").filter(|d| d.is_object()) else {
        return no_data();
    };
    let (Some(authored_nodes), Some(review_nodes)) = (
        data.pointer("/viewer/pullRequests/nodes")
            .and_then(Value::as_array),
        data.pointer("/reviewRequested/nodes")
            .and_then(Value::as_array),
    ) else {
        return no_data();
    };

    let mut withheld: u32 = 0;
    let mut rows = |nodes: &Vec<Value>| -> Vec<PullRequestSummary> {
        nodes
            .iter()
            .filter_map(|node| {
                if node.is_null() {
                    withheld = withheld.saturating_add(1);
                    return None;
                }
                if is_archived(node) {
                    return None;
                }
                row_from_node(node)
            })
            .collect()
    };
    let authored = rows(authored_nodes);
    let mut review_requested = rows(review_nodes);
    // A pull request in both sets — a team the account belongs to requested on
    // its own pull request — is listed once, as authored, so a web URL is
    // unique within the snapshot.
    review_requested
        .retain(|row| row.url.is_empty() || !authored.iter().any(|mine| mine.url == row.url));

    FetchResult::Ok {
        authored: merge_newest_first(vec![authored]),
        review_requested: merge_newest_first(vec![review_requested]),
        withheld,
    }
}

fn is_archived(node: &Value) -> bool {
    node.pointer("/repository/isArchived")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// One node as a row, or `None` when it is not a pull request at all (a node
/// without a `number`). Every other field is tolerant of absence.
fn row_from_node(node: &Value) -> Option<PullRequestSummary> {
    let number = node.get("number")?.as_u64()?;
    let text = |pointer: &str| {
        node.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let author = node
        .pointer("/author/login")
        .and_then(Value::as_str)
        .filter(|login| !login.is_empty())
        .map(str::to_string);
    Some(PullRequestSummary {
        id: number,
        title: text("/title"),
        repo_full_name: text("/repository/nameWithOwner"),
        // Null for a deleted fork: an empty head repository links nothing.
        source_repo_full_name: text("/headRepository/nameWithOwner"),
        source_branch: text("/headRefName"),
        destination_branch: text("/baseRefName"),
        url: web_url(node),
        draft: node
            .get("isDraft")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        updated_at_unix: node
            .get("updatedAt")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_to_unix)
            .unwrap_or(0),
        review: summarize_review(node, author.as_deref()),
        // GitHub has no tasks; its conversations are `unresolved_threads`.
        open_tasks: 0,
        checks: checks_state(node),
        conflicting: node.get("mergeable").and_then(Value::as_str) == Some("CONFLICTING"),
        unresolved_threads: unresolved_threads(node),
        author,
    })
}

/// `url`, kept only when it is on `https://github.com/`; anything else leaves
/// the row unopenable rather than handing an arbitrary link to the OS.
fn web_url(node: &Value) -> String {
    node.get("url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with(WEB_URL_PREFIX))
        .unwrap_or_default()
        .to_string()
}

/// The review cell (design D5): each reviewer's latest opinionated review,
/// excluding the author's own, counted as approvals and change requests; the
/// outstanding review requests — users or teams — as pending. `None` when the
/// node lacks either input, so "unknown" stays distinct from "none".
fn summarize_review(node: &Value, author: Option<&str>) -> Option<ReviewSummary> {
    let reviews = node
        .pointer("/latestOpinionatedReviews/nodes")?
        .as_array()?;
    let pending = node.pointer("/reviewRequests/totalCount")?.as_u64()?;
    let by_author = |review: &Value| {
        matches!(
            (author, review.pointer("/author/login").and_then(Value::as_str)),
            (Some(a), Some(login)) if a == login
        )
    };
    let count = |state: &str| {
        let n = reviews
            .iter()
            .filter(|r| !by_author(r) && r.get("state").and_then(Value::as_str) == Some(state))
            .count();
        saturating_u32(n as u64)
    };
    Some(ReviewSummary {
        approvals: count("APPROVED"),
        changes_requested: count("CHANGES_REQUESTED"),
        pending: saturating_u32(pending),
    })
}

/// The latest commit's check rollup in the panel's three states; `None` when
/// no checks ran (a null rollup) or the state is unrecognised.
fn checks_state(node: &Value) -> Option<ChecksState> {
    match node
        .pointer("/commits/nodes/0/commit/statusCheckRollup/state")
        .and_then(Value::as_str)?
    {
        "SUCCESS" => Some(ChecksState::Passing),
        "FAILURE" | "ERROR" => Some(ChecksState::Failing),
        "PENDING" | "EXPECTED" => Some(ChecksState::Pending),
        _ => None,
    }
}

/// Review threads not marked resolved, among the first 100.
fn unresolved_threads(node: &Value) -> u32 {
    node.pointer("/reviewThreads/nodes")
        .and_then(Value::as_array)
        .map_or(0, |threads| {
            let open = threads
                .iter()
                .filter(|t| t.get("isResolved").and_then(Value::as_bool) == Some(false))
                .count();
            saturating_u32(open as u64)
        })
}

// ---- the fetch ----

/// One reply, reduced to what the verdict reads: its status, its rate-limit
/// headers, and its body for a 2xx, 403 or 429. A transport error is no reply.
struct Reply {
    status: u16,
    headers: RateHeaders,
    body: Option<String>,
}

/// The one authenticated POST. Nothing about it — the URL, the error, the
/// reply — is logged, and the token is only ever inside the header.
fn send(url: &str, body: String, token: &str) -> Option<Reply> {
    let mut response = usage_http::post(url, Auth::Bearer(token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .send(body)
        .ok()?;
    let headers = {
        let raw = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        RateHeaders::from_raw(
            raw("Retry-After").as_deref(),
            raw("x-ratelimit-remaining").as_deref(),
            raw("x-ratelimit-reset").as_deref(),
        )
    };
    let status = response.status().as_u16();
    let body = matches!(status, 200..=299 | 403 | 429)
        .then(|| response.body_mut().read_to_string().ok())
        .flatten();
    Some(Reply {
        status,
        headers,
        body,
    })
}

/// One refresh over an injected transport, so every branch is testable without
/// a network: exactly one request, to [`API_URL`], carrying [`request_body`].
fn fetch_snapshot_with(
    post: impl FnOnce(&str, String) -> Option<Reply>,
    now_unix: u64,
) -> FetchResult {
    let Some(reply) = post(API_URL, request_body()) else {
        return FetchResult::Transient;
    };
    github_verdict(
        reply.status,
        &reply.headers,
        reply.body.as_deref(),
        now_unix,
    )
}

// ---- the poll loop ----

/// The snapshot a refresh leaves behind, given the one before it.
fn next_state(
    prev: &GithubPullRequestsState,
    result: FetchResult,
    now_unix: u64,
) -> GithubPullRequestsState {
    match result {
        FetchResult::Ok {
            authored,
            review_requested,
            withheld,
        } => GithubPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(now_unix),
            authored,
            review_requested,
            withheld,
        },
        FetchResult::Unauthenticated => {
            GithubPullRequestsState::status_only(PullRequestsStatus::Unauthenticated)
        }
        FetchResult::Unavailable => {
            GithubPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        }
        FetchResult::RateLimited { .. } | FetchResult::Transient => degrade_to_stale(prev),
    }
}

/// After a transient failure or a rate limit: keep the previous lists and the
/// withheld count, marked stale. With no previous rows, the snapshot is
/// unavailable.
fn degrade_to_stale(prev: &GithubPullRequestsState) -> GithubPullRequestsState {
    if prev.status == PullRequestsStatus::Ok && prev.rows().next().is_some() {
        GithubPullRequestsState {
            stale: true,
            ..prev.clone()
        }
    } else {
        GithubPullRequestsState::status_only(PullRequestsStatus::Unavailable)
    }
}

/// Whether a new snapshot is worth announcing. The fetch time alone is not:
/// every successful refresh stamps a new one.
fn is_news(prev: &GithubPullRequestsState, next: &GithubPullRequestsState) -> bool {
    let without_time = |state: &GithubPullRequestsState| GithubPullRequestsState {
        fetched_at_unix: None,
        ..state.clone()
    };
    without_time(prev) != without_time(next)
}

/// The wait between refreshes: the setting, floored.
fn refresh_interval(setting_secs: u64) -> Duration {
    Duration::from_secs(setting_secs.max(MIN_REFRESH_SECS))
}

/// How long a rate limit defers the next refresh: the delay, else the default,
/// never more than an hour.
fn backoff(retry_after: Option<u64>) -> Duration {
    Duration::from_secs(
        retry_after
            .unwrap_or(DEFAULT_BACKOFF_SECS)
            .min(MAX_BACKOFF_SECS),
    )
}

/// Whether a refresh is due at `now`: the interval has elapsed since the last
/// one (or there was none), and no backoff is still running.
fn refresh_due(
    last_poll: Option<Instant>,
    backoff_until: Option<Instant>,
    interval: Duration,
    now: Instant,
) -> bool {
    let due = last_poll.is_none_or(|t| now.duration_since(t) >= interval);
    let backed_off = backoff_until.is_some_and(|t| now < t);
    due && !backed_off
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Run the poll loop on the calling thread. Honours the enabled flag and the
/// refresh interval, caches the latest snapshot, and emits
/// `CacheEvent::GithubPullRequestsUpdated` whenever the snapshot changes. Never
/// reads a token or issues a request while disabled.
fn run_poller(
    settings: Arc<SettingsStore>,
    watcher: WatcherManager,
    handle: GithubPullRequestsHandle,
) {
    let mut last_poll: Option<Instant> = None;
    let mut backoff_until: Option<Instant> = None;

    loop {
        if !settings.github_enabled() {
            // Idle: collapse to Disabled once (announcing, so the panel goes),
            // then keep sleeping without touching a token or the network.
            if handle.get().status != PullRequestsStatus::Disabled {
                handle.set(GithubPullRequestsState::disabled());
                watcher.emit(CacheEvent::GithubPullRequestsUpdated);
            }
            last_poll = None;
            backoff_until = None;
            std::thread::sleep(TICK);
            continue;
        }

        let now = Instant::now();
        let interval = refresh_interval(settings.github_refresh_secs());
        if refresh_due(last_poll, backoff_until, interval, now) {
            let token = resolve_token(|name| std::env::var(name).ok(), settings.github_token());
            let result = match &token {
                None => FetchResult::Unauthenticated,
                Some(token) => fetch_snapshot_with(|url, body| send(url, body, token), now_unix()),
            };
            last_poll = Some(now);
            if let FetchResult::RateLimited { retry_after } = &result {
                backoff_until = Some(now + backoff(*retry_after));
            }
            // Switched off while the request ran: drop the result, and let the
            // next pass collapse the snapshot to Disabled.
            if settings.github_enabled() {
                let prev = handle.get();
                let next = next_state(&prev, result, now_unix());
                if next != prev {
                    let news = is_news(&prev, &next);
                    handle.set(next);
                    if news {
                        watcher.emit(CacheEvent::GithubPullRequestsUpdated);
                    }
                }
            }
        }

        std::thread::sleep(TICK);
    }
}

/// Spawn the poll loop on a background thread (mirroring
/// `bitbucket::spawn_poller`). The thread lives for the process; while the
/// feature is disabled it only re-checks the flag and never reaches the
/// network.
pub fn spawn_poller(
    settings: Arc<SettingsStore>,
    watcher: WatcherManager,
    handle: GithubPullRequestsHandle,
) {
    std::thread::spawn(move || run_poller(settings, watcher, handle));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::HashMap;

    const NOW: u64 = 1_800_000_000;

    // ------------------------------------------------------------ fixtures

    /// A fully populated node, as the query returns it.
    fn full_node() -> Value {
        json!({
            "number": 42,
            "title": "Add the GitHub panel",
            "url": "https://github.com/acme/specforge/pull/42",
            "isDraft": true,
            "updatedAt": "2026-09-01T00:00:00Z",
            "author": { "login": "ada" },
            "repository": { "nameWithOwner": "acme/specforge", "isArchived": false },
            "headRepository": { "nameWithOwner": "ada/specforge" },
            "headRefName": "feature/github",
            "baseRefName": "main",
            "mergeable": "CONFLICTING",
            "reviewRequests": { "totalCount": 2 },
            "latestOpinionatedReviews": { "nodes": [
                { "state": "APPROVED", "author": { "login": "grace" } },
                { "state": "CHANGES_REQUESTED", "author": { "login": "linus" } },
                { "state": "COMMENTED", "author": { "login": "ken" } }
            ] },
            "reviewThreads": { "nodes": [
                { "isResolved": false },
                { "isResolved": true },
                { "isResolved": false }
            ] },
            "commits": { "nodes": [ { "commit": { "statusCheckRollup": { "state": "FAILURE" } } } ] }
        })
    }

    fn node(number: u64, updated_at: &str) -> Value {
        json!({
            "number": number,
            "title": format!("PR {number}"),
            "url": format!("https://github.com/acme/api/pull/{number}"),
            "updatedAt": updated_at,
            "repository": { "nameWithOwner": "acme/api", "isArchived": false },
            "reviewRequests": { "totalCount": 0 },
            "latestOpinionatedReviews": { "nodes": [] }
        })
    }

    fn response_body(authored: Vec<Value>, review_requested: Vec<Value>) -> String {
        json!({ "data": {
            "viewer": { "pullRequests": { "nodes": authored } },
            "reviewRequested": { "nodes": review_requested }
        } })
        .to_string()
    }

    fn row(id: u64) -> PullRequestSummary {
        row_from_node(&node(id, "2026-09-01T00:00:00Z")).unwrap()
    }

    fn ok_state(authored: Vec<PullRequestSummary>) -> GithubPullRequestsState {
        GithubPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(1_000),
            authored,
            review_requested: vec![row(9)],
            withheld: 2,
        }
    }

    fn no_headers() -> RateHeaders {
        RateHeaders::default()
    }

    fn ok_rows(result: FetchResult) -> (Vec<u64>, Vec<u64>, u32) {
        match result {
            FetchResult::Ok {
                authored,
                review_requested,
                withheld,
            } => (
                authored.iter().map(|r| r.id).collect(),
                review_requested.iter().map(|r| r.id).collect(),
                withheld,
            ),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    // ------------------------------------------------------------ the query

    #[test]
    fn the_query_is_a_read_only_constant() {
        let lowered = QUERY.to_ascii_lowercase();
        assert!(!lowered.contains("mutation"), "no mutation");
        assert!(!lowered.contains("subscription"), "no subscription");
        assert!(QUERY.contains("query SpecForgePullRequests"));
        assert!(QUERY.contains("review-requested:@me"));
        assert!(QUERY.contains("archived:false"));
        assert!(QUERY.contains("pullRequests(states: OPEN, first: 50"));
        assert!(QUERY.contains("orderBy: { field: UPDATED_AT, direction: DESC }"));
        assert!(QUERY.contains("is:pr is:open"));
        assert!(QUERY.contains("sort:updated-desc"));
        assert!(QUERY.contains("type: ISSUE, first: 50"));
        // Every row field the parser reads is requested.
        for field in [
            "number",
            "title",
            "url",
            "isDraft",
            "updatedAt",
            "author { login }",
            "nameWithOwner",
            "isArchived",
            "headRepository { nameWithOwner }",
            "headRefName",
            "baseRefName",
            "mergeable",
            "reviewRequests(first: 20) { totalCount }",
            "latestOpinionatedReviews(first: 20)",
            "reviewThreads(first: 100)",
            "statusCheckRollup { state }",
        ] {
            assert!(QUERY.contains(field), "{field}");
        }
    }

    #[test]
    fn the_request_body_is_the_query_alone_and_never_varies() {
        let first = request_body();
        assert_eq!(first, request_body());
        let parsed: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(parsed, json!({ "query": QUERY }));
    }

    // ------------------------------------------------------------ the token

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn gh_token_wins_over_everything() {
        let env = env_of(&[("GH_TOKEN", "gh"), ("GITHUB_TOKEN", "github")]);
        assert_eq!(
            resolve_token(env, Some("stored".into())).as_deref(),
            Some("gh")
        );
    }

    #[test]
    fn an_empty_gh_token_falls_to_github_token() {
        let env = env_of(&[("GH_TOKEN", ""), ("GITHUB_TOKEN", "github")]);
        assert_eq!(
            resolve_token(env, Some("stored".into())).as_deref(),
            Some("github")
        );
        let env = env_of(&[("GITHUB_TOKEN", "github")]);
        assert_eq!(resolve_token(env, None).as_deref(), Some("github"));
    }

    #[test]
    fn without_the_environment_the_stored_token_is_used() {
        let env = env_of(&[("GITHUB_TOKEN", "")]);
        assert_eq!(
            resolve_token(env, Some("stored".into())).as_deref(),
            Some("stored")
        );
    }

    #[test]
    fn with_no_source_there_is_no_token() {
        assert_eq!(resolve_token(env_of(&[]), None), None);
    }

    // ------------------------------------------------------------ rate limits

    #[test]
    fn rate_headers_parse_numbers_and_ignore_garbage() {
        assert_eq!(
            RateHeaders::from_raw(Some(" 30 "), Some("0"), Some("1800000600")),
            RateHeaders {
                retry_after: Some(30),
                remaining: Some(0),
                reset: Some(1_800_000_600),
            }
        );
        assert_eq!(
            RateHeaders::from_raw(Some("soon"), Some("-1"), None),
            RateHeaders::default()
        );
    }

    #[test]
    fn the_delay_prefers_retry_after_then_the_reset() {
        let both = RateHeaders {
            retry_after: Some(30),
            remaining: Some(0),
            reset: Some(NOW + 600),
        };
        assert_eq!(rate_limit_delay(&both, NOW), Some(30));
        let reset_only = RateHeaders {
            remaining: Some(0),
            reset: Some(NOW + 600),
            ..RateHeaders::default()
        };
        assert_eq!(rate_limit_delay(&reset_only, NOW), Some(600));
        let past = RateHeaders {
            remaining: Some(0),
            reset: Some(NOW - 5),
            ..RateHeaders::default()
        };
        assert_eq!(
            rate_limit_delay(&past, NOW),
            Some(0),
            "a passed reset is zero"
        );
        assert_eq!(rate_limit_delay(&RateHeaders::default(), NOW), None);
    }

    /// The reset dates the primary window: it only counts when that quota is
    /// spent. A secondary limit arrives with a reset too — often most of an
    /// hour away — and must fall back to the default instead.
    #[test]
    fn the_reset_counts_only_when_the_primary_quota_is_spent() {
        for remaining in [None, Some(1), Some(4_000)] {
            let headers = RateHeaders {
                remaining,
                reset: Some(NOW + 3_300),
                ..RateHeaders::default()
            };
            assert_eq!(
                rate_limit_delay(&headers, NOW),
                None,
                "remaining {remaining:?}"
            );
        }
        let secondary = RateHeaders {
            remaining: Some(4_000),
            reset: Some(NOW + 3_300),
            ..RateHeaders::default()
        };
        let body = r#"{"message":"You have exceeded a secondary rate limit."}"#;
        let result = github_verdict(403, &secondary, Some(body), NOW);
        assert_eq!(result, FetchResult::RateLimited { retry_after: None });
        let FetchResult::RateLimited { retry_after } = result else {
            unreachable!()
        };
        assert_eq!(
            backoff(retry_after),
            Duration::from_secs(300),
            "the default, not 55 minutes"
        );
    }

    // ------------------------------------------------------------ the verdict

    #[test]
    fn a_401_is_unauthenticated() {
        assert_eq!(
            github_verdict(401, &no_headers(), None, NOW),
            FetchResult::Unauthenticated
        );
    }

    #[test]
    fn a_plain_403_is_a_credential_problem() {
        let headers = RateHeaders {
            remaining: Some(4_000),
            ..RateHeaders::default()
        };
        assert_eq!(
            github_verdict(
                403,
                &headers,
                Some(r#"{"message":"Must have admin rights"}"#),
                NOW
            ),
            FetchResult::Unauthenticated
        );
        assert_eq!(
            github_verdict(403, &no_headers(), None, NOW),
            FetchResult::Unauthenticated
        );
    }

    #[test]
    fn an_exhausted_quota_on_a_403_backs_off_until_the_reset() {
        let headers = RateHeaders {
            remaining: Some(0),
            reset: Some(NOW + 600),
            ..RateHeaders::default()
        };
        assert_eq!(
            github_verdict(403, &headers, Some("{}"), NOW),
            FetchResult::RateLimited {
                retry_after: Some(600)
            }
        );
    }

    #[test]
    fn a_retry_after_on_a_403_or_429_is_a_rate_limit() {
        let headers = RateHeaders {
            retry_after: Some(45),
            remaining: Some(10),
            ..RateHeaders::default()
        };
        for status in [403, 429] {
            assert_eq!(
                github_verdict(status, &headers, None, NOW),
                FetchResult::RateLimited {
                    retry_after: Some(45)
                },
                "status {status}"
            );
        }
    }

    #[test]
    fn a_secondary_limit_named_only_in_the_body_is_a_rate_limit() {
        let headers = RateHeaders {
            remaining: Some(4_000),
            ..RateHeaders::default()
        };
        let body = r#"{"message":"You have exceeded a secondary rate limit. Please wait."}"#;
        assert_eq!(
            github_verdict(403, &headers, Some(body), NOW),
            FetchResult::RateLimited { retry_after: None }
        );
    }

    #[test]
    fn a_bare_429_is_rate_limited_without_a_hint() {
        assert_eq!(
            github_verdict(429, &no_headers(), None, NOW),
            FetchResult::RateLimited { retry_after: None }
        );
    }

    #[test]
    fn redirects_not_found_and_server_errors_are_transient() {
        for status in [301, 302, 304, 404, 500, 502, 503] {
            assert_eq!(
                github_verdict(status, &no_headers(), None, NOW),
                FetchResult::Transient,
                "status {status}"
            );
        }
    }

    #[test]
    fn a_2xx_without_a_body_is_unavailable() {
        assert_eq!(
            github_verdict(200, &no_headers(), None, NOW),
            FetchResult::Unavailable
        );
    }

    // ------------------------------------------------------------ the parser

    #[test]
    fn a_full_node_maps_every_field() {
        assert_eq!(
            row_from_node(&full_node()),
            Some(PullRequestSummary {
                id: 42,
                title: "Add the GitHub panel".to_string(),
                repo_full_name: "acme/specforge".to_string(),
                source_branch: "feature/github".to_string(),
                destination_branch: "main".to_string(),
                url: "https://github.com/acme/specforge/pull/42".to_string(),
                draft: true,
                updated_at_unix: 1_788_220_800,
                review: Some(ReviewSummary {
                    approvals: 1,
                    changes_requested: 1,
                    pending: 2,
                }),
                open_tasks: 0,
                author: Some("ada".to_string()),
                checks: Some(ChecksState::Failing),
                conflicting: true,
                unresolved_threads: 2,
                source_repo_full_name: "ada/specforge".to_string(),
            })
        );
    }

    #[test]
    fn the_authors_own_reviews_are_excluded() {
        let mut n = full_node();
        n["latestOpinionatedReviews"]["nodes"] = json!([
            { "state": "APPROVED", "author": { "login": "ada" } },
            { "state": "CHANGES_REQUESTED", "author": { "login": "ada" } },
            { "state": "APPROVED", "author": { "login": "grace" } }
        ]);
        let review = row_from_node(&n).unwrap().review.unwrap();
        assert_eq!(review.approvals, 1);
        assert_eq!(review.changes_requested, 0);
    }

    #[test]
    fn a_missing_review_input_gives_no_summary() {
        let mut without_requests = full_node();
        without_requests
            .as_object_mut()
            .unwrap()
            .remove("reviewRequests");
        assert_eq!(row_from_node(&without_requests).unwrap().review, None);
        let mut without_reviews = full_node();
        without_reviews
            .as_object_mut()
            .unwrap()
            .remove("latestOpinionatedReviews");
        assert_eq!(row_from_node(&without_reviews).unwrap().review, None);
    }

    #[test]
    fn every_rollup_maps_to_its_checks_state() {
        let with_rollup = |state: Value| {
            let mut n = full_node();
            n["commits"]["nodes"][0]["commit"]["statusCheckRollup"] = state;
            row_from_node(&n).unwrap().checks
        };
        assert_eq!(
            with_rollup(json!({ "state": "SUCCESS" })),
            Some(ChecksState::Passing)
        );
        assert_eq!(
            with_rollup(json!({ "state": "FAILURE" })),
            Some(ChecksState::Failing)
        );
        assert_eq!(
            with_rollup(json!({ "state": "ERROR" })),
            Some(ChecksState::Failing)
        );
        assert_eq!(
            with_rollup(json!({ "state": "PENDING" })),
            Some(ChecksState::Pending)
        );
        assert_eq!(
            with_rollup(json!({ "state": "EXPECTED" })),
            Some(ChecksState::Pending)
        );
        assert_eq!(with_rollup(Value::Null), None, "no checks ran");
        assert_eq!(with_rollup(json!({ "state": "SOMETHING_NEW" })), None);
    }

    #[test]
    fn only_a_known_conflict_is_conflicting() {
        let with_mergeable = |m: &str| {
            let mut n = full_node();
            n["mergeable"] = json!(m);
            row_from_node(&n).unwrap().conflicting
        };
        assert!(with_mergeable("CONFLICTING"));
        assert!(!with_mergeable("MERGEABLE"));
        assert!(
            !with_mergeable("UNKNOWN"),
            "an uncomputed mergeability is not a conflict"
        );
    }

    #[test]
    fn missing_optional_fields_default_rather_than_fail() {
        let bare = json!({ "number": 7 });
        let row = row_from_node(&bare).expect("a number is enough");
        assert_eq!(row.id, 7);
        assert_eq!(row.url, "");
        assert!(!row.draft);
        assert_eq!(row.updated_at_unix, 0);
        assert_eq!(row.review, None);
        assert_eq!(row.author, None);
        assert_eq!(row.checks, None);
        assert!(!row.conflicting);
        assert_eq!(row.unresolved_threads, 0);
        assert_eq!(row_from_node(&json!({ "title": "not a PR" })), None);
    }

    #[test]
    fn only_a_github_link_becomes_the_row_url() {
        for foreign in [
            "http://github.com/acme/api/pull/1",
            "https://evil.example/acme/api/pull/1",
            "file:///etc/passwd",
        ] {
            let mut n = full_node();
            n["url"] = json!(foreign);
            assert_eq!(row_from_node(&n).unwrap().url, "", "{foreign}");
        }
    }

    #[test]
    fn a_huge_count_saturates_instead_of_wrapping() {
        let mut n = full_node();
        n["reviewRequests"]["totalCount"] = json!(u64::from(u32::MAX) + 7);
        assert_eq!(row_from_node(&n).unwrap().review.unwrap().pending, u32::MAX);
    }

    #[test]
    fn both_lists_are_read_newest_first() {
        let result = parse_response(
            &response_body(
                vec![
                    node(1, "2026-09-01T00:00:00Z"),
                    node(2, "2026-09-03T00:00:00Z"),
                ],
                vec![node(3, "2026-09-02T00:00:00Z")],
            ),
            &no_headers(),
            NOW,
        );
        assert_eq!(ok_rows(result), (vec![2, 1], vec![3], 0));
    }

    #[test]
    fn archived_repositories_are_excluded_from_both_lists() {
        let mut archived = node(1, "2026-09-01T00:00:00Z");
        archived["repository"]["isArchived"] = json!(true);
        let mut archived_review = node(3, "2026-09-01T00:00:00Z");
        archived_review["repository"]["isArchived"] = json!(true);
        let result = parse_response(
            &response_body(
                vec![archived, node(2, "2026-09-01T00:00:00Z")],
                vec![archived_review],
            ),
            &no_headers(),
            NOW,
        );
        assert_eq!(ok_rows(result), (vec![2], vec![], 0));
    }

    #[test]
    fn a_review_request_on_an_authored_pull_request_is_listed_once() {
        let result = parse_response(
            &response_body(
                vec![node(1, "2026-09-01T00:00:00Z")],
                vec![
                    node(1, "2026-09-01T00:00:00Z"),
                    node(2, "2026-09-01T00:00:00Z"),
                ],
            ),
            &no_headers(),
            NOW,
        );
        assert_eq!(ok_rows(result), (vec![1], vec![2], 0));
    }

    #[test]
    fn null_entries_are_withheld_and_counted() {
        let result = parse_response(
            &response_body(
                vec![Value::Null, node(1, "2026-09-01T00:00:00Z")],
                vec![Value::Null, Value::Null, node(2, "2026-09-01T00:00:00Z")],
            ),
            &no_headers(),
            NOW,
        );
        assert_eq!(ok_rows(result), (vec![1], vec![2], 3));
        // Data alongside errors is still read.
        let with_errors = json!({
            "data": {
                "viewer": { "pullRequests": { "nodes": [null] } },
                "reviewRequested": { "nodes": [node(2, "2026-09-01T00:00:00Z")] }
            },
            "errors": [{ "type": "FORBIDDEN", "message": "Resource protected by organization SAML enforcement." }]
        })
        .to_string();
        assert_eq!(
            ok_rows(parse_response(&with_errors, &no_headers(), NOW)),
            (vec![], vec![2], 1)
        );
    }

    #[test]
    fn a_rate_limit_in_the_body_backs_off_by_the_headers() {
        let headers = RateHeaders {
            remaining: Some(0),
            reset: Some(NOW + 900),
            ..RateHeaders::default()
        };
        let body =
            json!({ "errors": [{ "type": "RATE_LIMITED", "message": "API rate limit exceeded" }] })
                .to_string();
        assert_eq!(
            parse_response(&body, &headers, NOW),
            FetchResult::RateLimited {
                retry_after: Some(900)
            }
        );
        // And through the verdict, as a 200.
        assert_eq!(
            github_verdict(200, &headers, Some(&body), NOW),
            FetchResult::RateLimited {
                retry_after: Some(900)
            }
        );
    }

    #[test]
    fn no_data_with_a_missing_scope_is_a_credential_problem() {
        let body =
            json!({ "data": null, "errors": [{ "type": "INSUFFICIENT_SCOPES" }] }).to_string();
        assert_eq!(
            parse_response(&body, &no_headers(), NOW),
            FetchResult::Unauthenticated
        );
    }

    #[test]
    fn no_usable_data_otherwise_is_unavailable() {
        for body in [
            "not json".to_string(),
            json!({ "data": null }).to_string(),
            json!({ "data": null, "errors": [{ "type": "SOMETHING" }] }).to_string(),
            json!({ "data": { "viewer": null, "reviewRequested": { "nodes": [] } } }).to_string(),
            json!({ "data": { "viewer": { "pullRequests": { "nodes": [] } } } }).to_string(),
        ] {
            assert_eq!(
                parse_response(&body, &no_headers(), NOW),
                FetchResult::Unavailable,
                "{body}"
            );
        }
    }

    // ------------------------------------------------------------ the fetch

    #[test]
    fn one_refresh_is_one_request_to_the_graphql_endpoint() {
        let calls = RefCell::new(Vec::new());
        let result = fetch_snapshot_with(
            |url, body| {
                calls.borrow_mut().push((url.to_string(), body));
                Some(Reply {
                    status: 200,
                    headers: RateHeaders::default(),
                    body: Some(response_body(vec![], vec![])),
                })
            },
            NOW,
        );
        assert_eq!(ok_rows(result), (vec![], vec![], 0));
        let calls = calls.into_inner();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "https://api.github.com/graphql");
        assert_eq!(calls[0].1, request_body());
    }

    #[test]
    fn a_transport_error_is_transient() {
        assert_eq!(
            fetch_snapshot_with(|_, _| None, NOW),
            FetchResult::Transient
        );
    }

    /// Every reply here echoes the token back — in an error message, a
    /// rate-limit body, a `RATE_LIMITED` error, alongside good data — the
    /// places a careless parser could copy text from into the state. None of
    /// it may surface in the snapshot, its `Debug` form, or its wire form
    /// (`github-pull-requests`: *GitHub Privacy and Safety*).
    #[test]
    fn no_outcome_carries_the_token() {
        let token = "ghp_super_secret_value";
        let with_errors = |data: Value| {
            json!({
                "data": data,
                "errors": [{ "type": "FORBIDDEN", "message": format!("token {token} is not authorised") }]
            })
            .to_string()
        };
        let replies: Vec<(u16, RateHeaders, Option<String>)> = vec![
            (
                200,
                RateHeaders::default(),
                Some(with_errors(json!({
                    "viewer": { "pullRequests": { "nodes": [full_node(), null] } },
                    "reviewRequested": { "nodes": [] }
                }))),
            ),
            (200, RateHeaders::default(), Some(with_errors(Value::Null))),
            (
                200,
                RateHeaders {
                    remaining: Some(0),
                    reset: Some(NOW + 60),
                    ..RateHeaders::default()
                },
                Some(
                    json!({ "errors": [{ "type": "RATE_LIMITED", "message": format!("limit for {token}") }] })
                        .to_string(),
                ),
            ),
            (
                401,
                RateHeaders::default(),
                Some(format!(r#"{{"message":"Bad credentials: {token}"}}"#)),
            ),
            (
                403,
                RateHeaders::default(),
                Some(format!(r#"{{"message":"{token} lacks access"}}"#)),
            ),
            (
                403,
                RateHeaders::default(),
                Some(format!(r#"{{"message":"secondary rate limit for {token}"}}"#)),
            ),
            (
                429,
                RateHeaders::default(),
                Some(format!("slow down, {token}")),
            ),
            (500, RateHeaders::default(), Some(format!("oops {token}"))),
        ];
        let mut prev = GithubPullRequestsState::disabled();
        for (status, headers, reply_body) in replies {
            let result = fetch_snapshot_with(
                |_, _| {
                    Some(Reply {
                        status,
                        headers,
                        body: reply_body.clone(),
                    })
                },
                NOW,
            );
            let debug_result = format!("{result:?}");
            let state = next_state(&prev, result, NOW);
            let debug = format!("{state:?}");
            let wire = serde_json::to_string(&state).unwrap();
            for text in [&debug_result, &debug, &wire] {
                assert!(!text.contains(token), "status {status}: {text}");
            }
            prev = state;
        }
    }

    // ------------------------------------------------------------ the loop

    #[test]
    fn next_state_maps_each_outcome() {
        let prev = ok_state(vec![row(1)]);

        let ok = next_state(
            &prev,
            FetchResult::Ok {
                authored: vec![row(2)],
                review_requested: vec![],
                withheld: 1,
            },
            5_000,
        );
        assert_eq!(ok.status, PullRequestsStatus::Ok);
        assert!(!ok.stale);
        assert_eq!(ok.fetched_at_unix, Some(5_000));
        assert_eq!(ok.authored, vec![row(2)]);
        assert!(ok.review_requested.is_empty());
        assert_eq!(ok.withheld, 1);

        let unauth = next_state(&prev, FetchResult::Unauthenticated, 5_000);
        assert_eq!(
            unauth,
            GithubPullRequestsState::status_only(PullRequestsStatus::Unauthenticated)
        );
        let unavailable = next_state(&prev, FetchResult::Unavailable, 5_000);
        assert_eq!(
            unavailable,
            GithubPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        );

        for failure in [
            FetchResult::Transient,
            FetchResult::RateLimited {
                retry_after: Some(60),
            },
        ] {
            let stale = next_state(&prev, failure, 5_000);
            assert!(stale.stale);
            assert_eq!(stale.authored, prev.authored);
            assert_eq!(stale.fetched_at_unix, prev.fetched_at_unix);
        }
    }

    #[test]
    fn degrade_to_stale_keeps_both_lists_and_the_withheld_count() {
        let prev = ok_state(vec![row(1)]);
        let stale = degrade_to_stale(&prev);
        assert_eq!(
            stale,
            GithubPullRequestsState {
                stale: true,
                ..prev
            }
        );
    }

    #[test]
    fn degrade_to_stale_keeps_a_review_only_list() {
        // No authored rows, but a review request: still rows to keep.
        let prev = ok_state(vec![]);
        assert!(degrade_to_stale(&prev).stale);
    }

    #[test]
    fn degrade_to_stale_without_previous_rows_is_unavailable() {
        let empty = GithubPullRequestsState {
            review_requested: vec![],
            ..ok_state(vec![])
        };
        assert_eq!(
            degrade_to_stale(&empty),
            GithubPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        );
        let unauth = GithubPullRequestsState::status_only(PullRequestsStatus::Unauthenticated);
        assert_eq!(
            degrade_to_stale(&unauth),
            GithubPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        );
    }

    #[test]
    fn a_new_fetch_time_alone_is_not_news() {
        let prev = ok_state(vec![row(1)]);
        let refetched = GithubPullRequestsState {
            fetched_at_unix: Some(9_999),
            ..prev.clone()
        };
        assert!(!is_news(&prev, &refetched));
        let changed = GithubPullRequestsState {
            withheld: 3,
            ..prev.clone()
        };
        assert!(is_news(&prev, &changed));
        let restaled = GithubPullRequestsState {
            stale: true,
            ..prev.clone()
        };
        assert!(is_news(&prev, &restaled));
    }

    #[test]
    fn a_tiny_interval_is_floored_at_sixty_seconds() {
        assert_eq!(refresh_interval(5), Duration::from_secs(60));
        assert_eq!(refresh_interval(60), Duration::from_secs(60));
        assert_eq!(refresh_interval(600), Duration::from_secs(600));
    }

    #[test]
    fn a_rate_limit_backs_off_by_its_delay_or_five_minutes() {
        assert_eq!(backoff(Some(600)), Duration::from_secs(600));
        assert_eq!(backoff(None), Duration::from_secs(300));
    }

    /// A hostile or corrupt header cannot push the backoff past an hour — so
    /// `now + backoff(..)` in the loop can never overflow and panic the thread.
    #[test]
    fn a_backoff_is_capped_at_an_hour() {
        assert_eq!(backoff(Some(3_600)), Duration::from_secs(3_600));
        assert_eq!(backoff(Some(3_601)), Duration::from_secs(3_600));
        assert_eq!(backoff(Some(u64::MAX)), Duration::from_secs(3_600));
        let hostile = RateHeaders::from_raw(Some("18446744073709551615"), None, None);
        let delay = rate_limit_delay(&hostile, NOW);
        assert!(Instant::now().checked_add(backoff(delay)).is_some());
    }

    #[test]
    fn a_refresh_is_due_after_the_interval_and_outside_a_backoff() {
        let t0 = Instant::now();
        let interval = Duration::from_secs(120);
        assert!(refresh_due(None, None, interval, t0), "the first refresh");
        assert!(!refresh_due(
            Some(t0),
            None,
            interval,
            t0 + Duration::from_secs(119)
        ));
        assert!(refresh_due(Some(t0), None, interval, t0 + interval));
        let until = t0 + Duration::from_secs(600);
        assert!(!refresh_due(Some(t0), Some(until), interval, t0 + interval));
        assert!(refresh_due(Some(t0), Some(until), interval, until));
    }

    #[test]
    fn the_handle_starts_disabled_and_serves_what_was_set() {
        let handle = GithubPullRequestsHandle::new();
        assert_eq!(handle.get(), GithubPullRequestsState::disabled());
        let state = ok_state(vec![row(1)]);
        handle.clone().set(state.clone());
        assert_eq!(handle.get(), state);
    }

    #[test]
    fn rows_iterates_authored_then_review_requested() {
        let state = ok_state(vec![row(1), row(2)]);
        assert_eq!(
            state.rows().map(|r| r.id).collect::<Vec<_>>(),
            vec![1, 2, 9]
        );
    }
}
