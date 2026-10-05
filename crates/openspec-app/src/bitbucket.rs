//! Opt-in BitBucket pull-request tracking.
//!
//! Lists the open pull requests the configured account authored across every
//! workspace it belongs to, for the desktop app and the browser skin to render
//! as a side-pane panel (the `bitbucket-pull-requests` capability). The read
//! recipe is a port of the artifex CLI's `af bb pr mine` (design D3); the poll
//! loop is a twin of `crate::quota`'s (design D1).
//!
//! Everything is gated behind the `bitbucket.enabled` setting: with the feature
//! off, no credential is read and no request is made. The credential travels
//! only to `https://api.bitbucket.org`, only in the `Authorization` header that
//! `usage_http::Auth` builds, never through an ambient proxy (`usage_http::get`
//! pins `proxy(None)`), and is never formatted into any other string — no
//! outcome or state in this module carries text about a failure at all.
//!
//! The pure parts — URL building, credential resolution, the account
//! identifier, the parsers, the review summary, the merge, the state
//! transitions and the refresh schedule — are separate functions with their
//! own tests, so the mutation gate has assertions to catch. The loop itself is
//! thin, and runs on a plain `std::thread` like the quota pollers so the app
//! layer stays runtime-agnostic.
//!
//! The rate-limit deadline is the provider's, not the loop's: the poller
//! shares [`BitbucketLimits`] with the pull-request viewer's detail reads, so
//! a 429 either side meets holds the other back too
//! (`crate::pull_request_limits`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openspec_core::{CacheEvent, WatcherManager};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::Serialize;
use serde_json::Value;

use crate::pull_request_limits::{Deadlines, ProviderLimits};
use crate::pull_requests::{
    merge_newest_first, saturating_u32, PullRequestSummary, PullRequestsStatus, ReviewSummary,
};
use crate::quota::parse_rfc3339_to_unix;
use crate::settings::SettingsStore;
use crate::usage_http::{self, Auth, Verdict};

/// The API root. The only destination the credential is ever sent to.
pub(crate) const API_BASE: &str = "https://api.bitbucket.org/2.0";
/// Identifies SpecForge to the API, as the ChatGPT poller does to its endpoint.
pub(crate) const USER_AGENT: &str = concat!("SpecForge/", env!("CARGO_PKG_VERSION"));
/// Poller wake cadence: how often the loop re-checks the enabled flag and
/// whether a refresh is due — see `quota.rs`'s identical constant.
const TICK: Duration = Duration::from_secs(2);
/// Floor for the configurable refresh interval. Higher than the quota pollers'
/// because one refresh here is `2 + W` requests, not one.
const MIN_REFRESH_SECS: u64 = 60;
/// Fallback backoff when a 429 carries no `Retry-After`.
const DEFAULT_BACKOFF_SECS: u64 = 300;
/// Ceiling on a 429 backoff, so the deadline never lies more than an hour
/// ahead and a hostile or corrupt `Retry-After` (`18446744073709551615`)
/// cannot park the provider forever — the fix `crate::github` needed, applied
/// to its twin.
const MAX_BACKOFF_SECS: u64 = 3_600;
/// The most pages of a diffstat, or of comments, one detail read follows
/// (`pull-request-viewer`: *BitBucket Detail Reads*).
pub const DETAIL_MAX_PAGES: usize = 10;
/// The most requests one detail read sends: the pull request, its diffstat
/// pages, the diff, its comment pages and the statuses.
pub const DETAIL_MAX_REQUESTS: usize = 1 + DETAIL_MAX_PAGES + 1 + DETAIL_MAX_PAGES + 1;
/// The workspace count [`DETAIL_BUDGET`] leaves the poller room for.
pub const BUDGET_WORKSPACES: usize = 7;
/// BitBucket's hourly budget of detail requests (design D8 leaves the number
/// open). BitBucket allows 1,000 requests an hour to `/2.0/repositories/*`,
/// which the shared deadline assumes the poller draws on too, so the budget
/// leaves room within it for two reads admitted just under it, which send at
/// most `2 × 23 − 1` past it, and for the poller's `2 + W` requests a refresh
/// at its 60-second floor, for an account in up to [`BUDGET_WORKSPACES`]
/// workspaces: `400 + 45 + 60 × 9 = 985`.
pub const DETAIL_BUDGET: usize = 400;
/// Page length of each workspace's list. Only the first page is fetched: a
/// panel holding 50 open authored pull requests per workspace is already past
/// what it is for (design D3).
const PAGE_LEN: u32 = 50;
/// Page length for `GET /2.0/user/workspaces`, whose default page holds only
/// ten workspaces. The listing is followed through `next` up to
/// [`MAX_WORKSPACE_PAGES`] pages, so an account in many workspaces is not
/// silently truncated to its first ten.
const WORKSPACES_PAGE_LEN: u32 = 100;
/// Upper bound on workspace-listing pages followed in one refresh, so a
/// malformed `next` chain can never loop a refresh forever.
const MAX_WORKSPACE_PAGES: usize = 5;
/// The partial-response `fields` value that adds `participants` and
/// `reviewers` back to pull-request list values — the list endpoints omit both
/// by default. Ported verbatim from artifex's `REVIEW_FIELDS`. Every `+` must
/// reach the wire as `%2B`: an unencoded one decodes as a space, matches
/// nothing, and every review cell would silently read "unknown".
const REVIEW_FIELDS: &str = "+values.participants,+values.reviewers";
/// The environment pair that overrides the stored credentials — the artifex
/// names, so one shell profile configures both tools.
const ENV_USERNAME: &str = "BITBUCKET_USERNAME";
const ENV_API_TOKEN: &str = "BITBUCKET_API_TOKEN";

/// What `encodeURIComponent` leaves unescaped. artifex builds its path segments
/// with it, and its `fields` value through `URLSearchParams`, which agrees with
/// it on every character `REVIEW_FIELDS` holds — so this reproduces its URLs
/// byte for byte.
const URI_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

// ---- the snapshot ----

/// The snapshot every frontend renders. `stale` marks rows kept from an earlier
/// refresh after a transient failure, so the panel can de-emphasise them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BitbucketPullRequestsState {
    pub status: PullRequestsStatus,
    pub stale: bool,
    /// When the list was fetched, as Unix epoch seconds — kept through a stale
    /// period, since it dates the rows still shown. `None` when there is no
    /// list at all (disabled, unauthenticated, unavailable).
    pub fetched_at_unix: Option<u64>,
    /// Newest-updated first, across every workspace.
    pub pull_requests: Vec<PullRequestSummary>,
    /// Slugs of the workspaces that answered 403 or 404 and were skipped —
    /// informational, never an error.
    pub skipped_workspaces: Vec<String>,
}

impl BitbucketPullRequestsState {
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
            pull_requests: Vec::new(),
            skipped_workspaces: Vec::new(),
        }
    }
}

/// Cheaply-cloneable handle to the latest snapshot, shared between the poller
/// (writer) and the frontends (readers) — the `QuotaHandle` model.
#[derive(Clone)]
pub struct BitbucketPullRequestsHandle(Arc<Mutex<BitbucketPullRequestsState>>);

impl BitbucketPullRequestsHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(BitbucketPullRequestsState::disabled())))
    }

    /// The current snapshot.
    pub fn get(&self) -> BitbucketPullRequestsState {
        self.0.lock().unwrap().clone()
    }

    pub(crate) fn set(&self, state: BitbucketPullRequestsState) {
        *self.0.lock().unwrap() = state;
    }
}

impl Default for BitbucketPullRequestsHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---- credentials ----

/// The credential pair a refresh authenticates with: `BITBUCKET_USERNAME` and
/// `BITBUCKET_API_TOKEN` from the environment when **both** are set, else the
/// stored pair, else none (→ unauthenticated). A partial environment never
/// overrides — half a pair from each source would authenticate as nobody.
///
/// `env` is the environment lookup, injected so the rule is testable without
/// mutating the process environment. The detail reads resolve their
/// credential by the same rule.
pub(crate) fn resolve_credentials(
    env: impl Fn(&str) -> Option<String>,
    stored: Option<(String, String)>,
) -> Option<(String, String)> {
    let var = |name: &str| env(name).filter(|value| !value.is_empty());
    match (var(ENV_USERNAME), var(ENV_API_TOKEN)) {
        (Some(username), Some(token)) => Some((username, token)),
        _ => stored,
    }
}

// ---- the recipe: URLs and parsers ----

/// `component` as one path segment, encoded as `encodeURIComponent` encodes
/// it. The detail reads, GitHub's included, encode their segments with it.
pub(crate) fn encode(component: &str) -> String {
    utf8_percent_encode(component, URI_COMPONENT).to_string()
}

/// The first page of `workspace`'s open pull requests authored by `account`,
/// newest-updated first, with the review fields added back
/// (`GET /2.0/workspaces/{workspace}/pullrequests/{account}`). The removed
/// cross-workspace `GET /2.0/pullrequests/{user}` is never built.
fn build_pull_requests_url(workspace: &str, account: &str) -> String {
    format!(
        "{API_BASE}/workspaces/{}/pullrequests/{}?state=OPEN&sort=-updated_on&pagelen={PAGE_LEN}&fields={}",
        encode(workspace),
        encode(account),
        encode(REVIEW_FIELDS),
    )
}

/// The account a workspace's list is requested for: its `uuid` when present,
/// else its `account_id` — artifex's resolution. Re-read on every refresh, so a
/// changed credential takes effect at the next one without a restart.
fn account_identifier(user: &Value) -> Option<String> {
    let field = |key: &str| {
        user.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    field("uuid")
        .or_else(|| field("account_id"))
        .map(str::to_string)
}

/// One page of `GET /2.0/user/workspaces`: its workspace slugs in response
/// order, and the page's `next` link when there is one. `None` when the body
/// is not a page at all (→ unavailable).
///
/// A `next` link is kept only when it points back at [`API_BASE`]: it is
/// followed with the credential attached, so a link anywhere else — however
/// it got into the body — must never be requested.
fn parse_workspace_slugs(body: &str) -> Option<(Vec<String>, Option<String>)> {
    let json: Value = serde_json::from_str(body).ok()?;
    let values = json.get("values")?.as_array()?;
    let slugs = values
        .iter()
        .filter_map(|access| access.pointer("/workspace/slug")?.as_str())
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
        .collect();
    let next = json
        .get("next")
        .and_then(Value::as_str)
        .filter(|next| next.starts_with(API_BASE))
        .map(str::to_string);
    Some((slugs, next))
}

/// The first page of the workspace listing.
fn build_workspaces_url() -> String {
    format!("{API_BASE}/user/workspaces?pagelen={WORKSPACES_PAGE_LEN}")
}

/// One page of a workspace's pull-request list, as rows. `None` when the body
/// is not a page at all (→ unavailable). Tolerant of a missing `participants`,
/// `task_count`, `draft` or `links.html` on any entry; an entry without an `id`
/// is skipped rather than failing the page.
fn parse_pull_requests(body: &str) -> Option<Vec<PullRequestSummary>> {
    let json: Value = serde_json::from_str(body).ok()?;
    let values = json.get("values")?.as_array()?;
    Some(values.iter().filter_map(parse_pull_request).collect())
}

fn parse_pull_request(pr: &Value) -> Option<PullRequestSummary> {
    let text = |pointer: &str| {
        pr.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(PullRequestSummary {
        id: pr.get("id")?.as_u64()?,
        title: text("/title"),
        repo_full_name: text("/destination/repository/full_name"),
        source_repo_full_name: text("/source/repository/full_name"),
        source_branch: text("/source/branch/name"),
        destination_branch: text("/destination/branch/name"),
        url: web_url(pr),
        draft: pr.get("draft").and_then(Value::as_bool).unwrap_or(false),
        updated_at_unix: pr
            .get("updated_on")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_to_unix)
            .unwrap_or(0),
        review: summarize_review(pr),
        open_tasks: pr
            .get("task_count")
            .and_then(Value::as_u64)
            .map_or(0, saturating_u32),
        // GitHub-only signals: BitBucket's build statuses and conflicts cost
        // extra requests per pull request and are not fetched, and the panel
        // shows only the requester's author on GitHub's "To review" rows.
        author: None,
        checks: None,
        conflicting: false,
        unresolved_threads: 0,
    })
}

/// Where a row's web page may live. The desktop opener hands a row's URL to
/// the OS, so only a link on BitBucket's own site survives into a row.
const WEB_URL_PREFIX: &str = "https://bitbucket.org/";

/// The pull request's web page — `links.html.href`, kept only when it is an
/// `https://bitbucket.org/` URL. A response that smuggled in a `file:`,
/// custom-scheme or off-site link (a redirect's body, say) must not survive
/// into a row; such a row simply cannot be opened.
fn web_url(pr: &Value) -> String {
    pr.pointer("/links/html/href")
        .and_then(Value::as_str)
        .filter(|href| href.starts_with(WEB_URL_PREFIX))
        .unwrap_or_default()
        .to_string()
}

// ---- the review summary (a port of artifex's `summarizeReview`) ----

/// Whether two users are the same account: by `account_id` when both carry
/// one, else by `uuid`.
fn same_account(a: Option<&Value>, b: Option<&Value>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let field = |user: &Value, key: &str| {
        user.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    if let (Some(x), Some(y)) = (field(a, "account_id"), field(b, "account_id")) {
        return x == y;
    }
    matches!((field(a, "uuid"), field(b, "uuid")), (Some(x), Some(y)) if x == y)
}

fn has_approved(participant: &Value) -> bool {
    participant.get("approved").and_then(Value::as_bool) == Some(true)
}

fn has_requested_changes(participant: &Value) -> bool {
    participant.get("state").and_then(Value::as_str) == Some("changes_requested")
}

/// Summarise a pull request's review state, or `None` when the response carried
/// no `participants`, so "unknown" stays distinct from "none".
///
/// The author's own approval does not count toward BitBucket's
/// minimum-approvals check, so the author is excluded from approvals and change
/// requests. Pending is counted from `reviewers`, so participants who only
/// commented never count.
fn summarize_review(pr: &Value) -> Option<ReviewSummary> {
    let participants = pr.get("participants")?.as_array()?;
    let author = pr.get("author");
    let others: Vec<&Value> = participants
        .iter()
        .filter(|p| !same_account(p.get("user"), author))
        .collect();
    let responded: Vec<&Value> = participants
        .iter()
        .filter(|p| has_approved(p) || has_requested_changes(p))
        .collect();
    let pending = pr
        .get("reviewers")
        .and_then(Value::as_array)
        .map_or(0, |reviewers| {
            reviewers
                .iter()
                .filter(|reviewer| {
                    !responded
                        .iter()
                        .any(|p| same_account(p.get("user"), Some(reviewer)))
                })
                .count()
        });
    Some(ReviewSummary {
        approvals: others.iter().filter(|p| has_approved(p)).count() as u32,
        changes_requested: others.iter().filter(|p| has_requested_changes(p)).count() as u32,
        pending: pending as u32,
    })
}

// ---- the fetch ----

/// Outcome of one refresh — the whole request chain.
#[derive(Debug, PartialEq, Eq)]
enum FetchResult {
    /// Every workspace answered or was skipped: the merged rows.
    Ok {
        pull_requests: Vec<PullRequestSummary>,
        skipped_workspaces: Vec<String>,
    },
    /// No credential, a request answered 401, or the account resources
    /// (`/user`, `/user/workspaces`) answered 403 — the token exists but lacks
    /// the account or workspace-membership read scope, which is the common
    /// misconfiguration and is fixed in Settings, not by waiting.
    Unauthenticated,
    /// A 2xx body could not be parsed into the expected shape.
    Unavailable,
    /// Rate-limited (429); back off for the hinted (or default) delay.
    RateLimited { retry_after: Option<u64> },
    /// A transport error or any other non-success status — keep the last rows.
    Transient,
}

/// One reply, reduced to what the recipe reads off it: the verdict
/// `usage_http::classify` gave its status, and its body when that verdict is
/// `Read`. A transport error (offline, timeout, TLS) is no reply at all.
struct Reply {
    verdict: Verdict,
    body: Option<String>,
}

/// One refresh's request chain over `get`, asking `may_send` before every
/// request, so switching the feature off, or a deadline a detail read sets
/// meanwhile, stops the chain at its next request instead of letting it run
/// to the end. `None` when it was stopped short: its result is then dropped,
/// so a detail read's 429 never marks the snapshot stale
/// (`bitbucket-pull-requests`: *A detail read's failure leaves the snapshot
/// unchanged*).
fn refresh(
    mut get: impl FnMut(&str) -> Option<Reply>,
    may_send: impl Fn() -> bool,
) -> Option<FetchResult> {
    let mut stopped = false;
    let result = fetch_snapshot_with(|url| {
        if may_send() {
            get(url)
        } else {
            stopped = true;
            None
        }
    });
    (!stopped).then_some(result)
}

/// One authenticated GET. Nothing about it — the URL, the error, the reply —
/// is logged, and the credential is only ever inside the header.
fn send(url: &str, username: &str, token: &str) -> Option<Reply> {
    let mut response = usage_http::get(url, Auth::Basic { username, token })
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let retry_after = response
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let verdict = usage_http::classify(response.status().as_u16(), retry_after.as_deref());
    let body = match verdict {
        Verdict::Read => response.body_mut().read_to_string().ok(),
        _ => None,
    };
    Some(Reply { verdict, body })
}

/// The chain itself, over an injected `get`, so every branch is testable
/// without a network: the account, its workspaces (one page for up to a
/// hundred of them), then each workspace's authored pull requests —
/// sequentially, `2 + W` requests in all.
fn fetch_snapshot_with(mut get: impl FnMut(&str) -> Option<Reply>) -> FetchResult {
    let user = match body_of(get(&format!("{API_BASE}/user"))) {
        Ok(body) => body,
        Err(stop) => return stop,
    };
    let Some(account) = serde_json::from_str::<Value>(&user)
        .ok()
        .as_ref()
        .and_then(account_identifier)
    else {
        return FetchResult::Unavailable;
    };

    let mut slugs = Vec::new();
    let mut page_url = Some(build_workspaces_url());
    // A counted `for`, not a `while` with a counter: the bound is then a
    // property of the loop itself, and no arithmetic on it can be mutated
    // into an endless chain.
    for _ in 0..MAX_WORKSPACE_PAGES {
        let Some(url) = page_url.take() else {
            break;
        };
        let workspaces = match body_of(get(&url)) {
            Ok(body) => body,
            Err(stop) => return stop,
        };
        let Some((page_slugs, next)) = parse_workspace_slugs(&workspaces) else {
            return FetchResult::Unavailable;
        };
        slugs.extend(page_slugs);
        page_url = next;
    }

    let mut per_workspace = Vec::with_capacity(slugs.len());
    let mut skipped_workspaces = Vec::new();
    for slug in slugs {
        let reply = get(&build_pull_requests_url(&slug, &account));
        // A workspace the token cannot read is skipped and named; the others
        // still list.
        if let Some(Reply {
            verdict: Verdict::Forbidden | Verdict::NotFound,
            ..
        }) = reply
        {
            skipped_workspaces.push(slug);
            continue;
        }
        let page = match body_of(reply) {
            Ok(body) => body,
            Err(stop) => return stop,
        };
        let Some(rows) = parse_pull_requests(&page) else {
            return FetchResult::Unavailable;
        };
        per_workspace.push(rows);
    }

    FetchResult::Ok {
        pull_requests: merge_newest_first(per_workspace),
        skipped_workspaces,
    }
}

/// A reply's body, or the result the whole refresh stops with.
fn body_of(reply: Option<Reply>) -> Result<String, FetchResult> {
    let Some(reply) = reply else {
        return Err(FetchResult::Transient);
    };
    match reply.verdict {
        Verdict::Read => reply.body.ok_or(FetchResult::Unavailable),
        Verdict::Unauthenticated => Err(FetchResult::Unauthenticated),
        Verdict::RateLimited { retry_after } => Err(FetchResult::RateLimited { retry_after }),
        // Outside the per-workspace step there is nothing to skip. A 403 on
        // `/user` or `/user/workspaces` means the token lacks a read scope the
        // recipe needs — a credential problem the user fixes in Settings, so
        // it reports as unauthenticated rather than as an outage. A 404 there
        // is as transient as any other status.
        Verdict::Forbidden => Err(FetchResult::Unauthenticated),
        Verdict::NotFound | Verdict::Transient => Err(FetchResult::Transient),
    }
}

// ---- the provider's limits ----

/// BitBucket's one rate-limit deadline, the Unix second it ends at, zero when
/// never set. Its poller and its detail reads share it, since BitBucket's
/// hourly limit is assumed to cover both (design D8).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BitbucketDeadline {
    pub until: u64,
}

impl Deadlines for BitbucketDeadline {
    fn held_until(&self) -> u64 {
        self.until
    }
}

/// BitBucket's limits: its deadline, its hourly detail budget and its detail
/// reads in flight, held on `AppService` and handed to the poller.
pub type BitbucketLimits = ProviderLimits<BitbucketDeadline>;

impl BitbucketLimits {
    pub fn new() -> Self {
        Self::with_budget(DETAIL_BUDGET)
    }

    /// Sets the deadline a 429 calls for, whether the poller or a detail read
    /// met it: its `Retry-After`, else 300 seconds, never more than an hour
    /// ahead. It only ever moves later, so a short delay never cuts short a
    /// longer one still holding.
    pub(crate) fn rate_limited(&self, retry_after: Option<u64>, now_unix: u64) {
        let until = now_unix.saturating_add(backoff(retry_after).as_secs());
        self.update_deadlines(|deadline| deadline.until = deadline.until.max(until));
    }

    /// Whether the poller must wait at `now_unix`, whichever request set the
    /// deadline. Never the budget or the reads in flight.
    pub(crate) fn holds_poller(&self, now_unix: u64) -> bool {
        now_unix < self.deadlines().until
    }
}

impl Default for BitbucketLimits {
    fn default() -> Self {
        Self::new()
    }
}

/// The tick rule for a provider enabled inside its deadline
/// (`bitbucket-pull-requests`: *Polling With Caching and Backoff*): while the
/// deadline holds, a snapshot still reading `disabled` is published as
/// `unavailable` and announced at once, as a 429 with no previous rows reads,
/// so the panel and any pull-request address say BitBucket is unavailable
/// rather than loading until the deadline passes. The caller has checked that
/// BitBucket is enabled. Returns whether it published.
pub(crate) fn publish_held_unavailable(
    handle: &BitbucketPullRequestsHandle,
    watcher: &WatcherManager,
    limits: &BitbucketLimits,
    now_unix: u64,
) -> bool {
    if !limits.holds_poller(now_unix) || handle.get().status != PullRequestsStatus::Disabled {
        return false;
    }
    handle.set(BitbucketPullRequestsState::status_only(
        PullRequestsStatus::Unavailable,
    ));
    watcher.emit(CacheEvent::BitbucketPullRequestsUpdated);
    true
}

// ---- the poll loop ----

/// The snapshot a refresh leaves behind, given the one before it.
fn next_state(
    prev: &BitbucketPullRequestsState,
    result: FetchResult,
    now_unix: u64,
) -> BitbucketPullRequestsState {
    match result {
        FetchResult::Ok {
            pull_requests,
            skipped_workspaces,
        } => BitbucketPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(now_unix),
            pull_requests,
            skipped_workspaces,
        },
        FetchResult::Unauthenticated => {
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unauthenticated)
        }
        FetchResult::Unavailable => {
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        }
        FetchResult::RateLimited { .. } | FetchResult::Transient => degrade_to_stale(prev),
    }
}

/// After a transient failure or a 429: keep the previous rows, marked stale.
/// With no previous rows to keep, the snapshot is unavailable.
fn degrade_to_stale(prev: &BitbucketPullRequestsState) -> BitbucketPullRequestsState {
    if prev.status == PullRequestsStatus::Ok && !prev.pull_requests.is_empty() {
        BitbucketPullRequestsState {
            stale: true,
            ..prev.clone()
        }
    } else {
        BitbucketPullRequestsState::status_only(PullRequestsStatus::Unavailable)
    }
}

/// Whether a new snapshot is worth announcing. The fetch time alone is not:
/// every successful refresh stamps a new one, so counting it would make every
/// refresh an event — and a refresh that changes nothing must be silent.
fn is_news(prev: &BitbucketPullRequestsState, next: &BitbucketPullRequestsState) -> bool {
    let without_time = |state: &BitbucketPullRequestsState| BitbucketPullRequestsState {
        fetched_at_unix: None,
        ..state.clone()
    };
    without_time(prev) != without_time(next)
}

/// The wait between refreshes: the setting, floored.
fn refresh_interval(setting_secs: u64) -> Duration {
    Duration::from_secs(setting_secs.max(MIN_REFRESH_SECS))
}

/// How far ahead a 429 sets the deadline: the hint, else the default, never
/// more than an hour.
fn backoff(retry_after: Option<u64>) -> Duration {
    Duration::from_secs(
        retry_after
            .unwrap_or(DEFAULT_BACKOFF_SECS)
            .min(MAX_BACKOFF_SECS),
    )
}

/// Whether a refresh is due at `now`: the interval has elapsed since the last
/// one (or there was none), and the deadline does not hold.
fn refresh_due(last_poll: Option<Instant>, held: bool, interval: Duration, now: Instant) -> bool {
    let due = last_poll.is_none_or(|t| now.duration_since(t) >= interval);
    due && !held
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Run the poll loop on the calling thread. Honours the enabled flag, the
/// refresh interval and the deadline, caches the latest snapshot, and emits
/// `CacheEvent::BitbucketPullRequestsUpdated` whenever the snapshot changes. Never reads
/// a credential or issues a request while disabled.
fn run_poller(
    settings: Arc<SettingsStore>,
    watcher: WatcherManager,
    handle: BitbucketPullRequestsHandle,
    limits: BitbucketLimits,
) {
    let mut last_poll: Option<Instant> = None;

    loop {
        if !settings.bitbucket_enabled() {
            // Idle: collapse to Disabled once (announcing, so the panel goes),
            // then keep sleeping without touching a credential or the network.
            // The deadline stays: it is the provider's, and switching it off
            // and on must not reset it (design D8).
            if handle.get().status != PullRequestsStatus::Disabled {
                handle.set(BitbucketPullRequestsState::disabled());
                watcher.emit(CacheEvent::BitbucketPullRequestsUpdated);
            }
            last_poll = None;
            std::thread::sleep(TICK);
            continue;
        }

        publish_held_unavailable(&handle, &watcher, &limits, now_unix());
        let now = Instant::now();
        let interval = refresh_interval(settings.bitbucket_refresh_secs());
        if refresh_due(last_poll, limits.holds_poller(now_unix()), interval, now) {
            let credentials = resolve_credentials(
                |name| std::env::var(name).ok(),
                settings.bitbucket_credentials(),
            );
            let result = match &credentials {
                None => Some(FetchResult::Unauthenticated),
                Some((username, token)) => refresh(
                    |url| send(url, username, token),
                    || settings.bitbucket_enabled() && !limits.holds_poller(now_unix()),
                ),
            };
            last_poll = Some(now);
            if let Some(FetchResult::RateLimited { retry_after }) = &result {
                limits.rate_limited(*retry_after, now_unix());
            }
            // Stopped short, or switched off once the chain ran: drop the
            // result, and let the next pass collapse the snapshot to Disabled
            // or wait the deadline out.
            if let Some(result) = result.filter(|_| settings.bitbucket_enabled()) {
                let prev = handle.get();
                let next = next_state(&prev, result, now_unix());
                if next != prev {
                    let news = is_news(&prev, &next);
                    handle.set(next);
                    if news {
                        watcher.emit(CacheEvent::BitbucketPullRequestsUpdated);
                    }
                }
            }
        }

        std::thread::sleep(TICK);
    }
}

/// Spawn the poll loop on a background thread (mirroring
/// `quota::spawn_poller`). The thread lives for the process; while the feature
/// is disabled it only re-checks the flag and never reaches the network.
/// `limits` are the provider's, shared with the detail reads.
pub fn spawn_poller(
    settings: Arc<SettingsStore>,
    watcher: WatcherManager,
    handle: BitbucketPullRequestsHandle,
    limits: BitbucketLimits,
) {
    std::thread::spawn(move || run_poller(settings, watcher, handle, limits));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_request_limits::{admit_or_fail, wait_until, Admission, DetailPermit};
    use crate::usage_http::classify;
    use serde_json::json;
    use std::collections::HashMap;

    // ------------------------------------------------------------ fixtures

    fn row(id: u64, updated_at_unix: u64) -> PullRequestSummary {
        PullRequestSummary {
            id,
            title: format!("PR {id}"),
            repo_full_name: "ws/repo".to_string(),
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            url: format!("https://bitbucket.org/ws/repo/pull-requests/{id}"),
            draft: false,
            updated_at_unix,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: "ws/repo".to_string(),
        }
    }

    fn ok_state(rows: Vec<PullRequestSummary>) -> BitbucketPullRequestsState {
        BitbucketPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(1_000),
            pull_requests: rows,
            skipped_workspaces: vec!["locked-ws".to_string()],
        }
    }

    fn user(account_id: &str, uuid: &str) -> Value {
        json!({ "account_id": account_id, "uuid": uuid, "display_name": account_id })
    }

    fn participant(who: Value, approved: bool, state: Option<&str>) -> Value {
        json!({ "user": who, "role": "REVIEWER", "approved": approved, "state": state })
    }

    // ------------------------------------------------------------ the URL

    #[test]
    fn the_pull_requests_url_is_the_recipe_with_the_review_fields_encoded() {
        let url = build_pull_requests_url("ws", "{me-uuid}");
        assert_eq!(
            url,
            "https://api.bitbucket.org/2.0/workspaces/ws/pullrequests/%7Bme-uuid%7D\
             ?state=OPEN&sort=-updated_on&pagelen=50\
             &fields=%2Bvalues.participants%2C%2Bvalues.reviewers"
        );
        assert!(url.contains("state=OPEN"));
        assert!(url.contains("sort=-updated_on"));
        assert!(url.contains("pagelen=50"));
        assert!(url.contains("%2Bvalues.participants"));
        assert!(url.contains("%2Bvalues.reviewers"));
        assert!(!url.contains('+'), "a raw + decodes as a space: {url}");
    }

    #[test]
    fn the_pull_requests_url_escapes_its_path_segments() {
        let url = build_pull_requests_url("my ws", "557058:abc/def");
        assert!(
            url.starts_with(
                "https://api.bitbucket.org/2.0/workspaces/my%20ws/pullrequests/557058%3Aabc%2Fdef?"
            ),
            "{url}"
        );
    }

    // ------------------------------------------------------------ credentials

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    fn stored() -> Option<(String, String)> {
        Some(("stored-user".to_string(), "stored-token".to_string()))
    }

    #[test]
    fn a_complete_environment_pair_overrides_the_stored_one() {
        let env = env_of(&[
            ("BITBUCKET_USERNAME", "env-user"),
            ("BITBUCKET_API_TOKEN", "env-token"),
        ]);
        assert_eq!(
            resolve_credentials(env, stored()),
            Some(("env-user".to_string(), "env-token".to_string()))
        );
    }

    #[test]
    fn a_partial_environment_does_not_override() {
        for pairs in [
            [("BITBUCKET_USERNAME", "env-user")],
            [("BITBUCKET_API_TOKEN", "env-token")],
        ] {
            assert_eq!(resolve_credentials(env_of(&pairs), stored()), stored());
        }
        // An empty value is as good as unset.
        let env = env_of(&[
            ("BITBUCKET_USERNAME", "env-user"),
            ("BITBUCKET_API_TOKEN", ""),
        ]);
        assert_eq!(resolve_credentials(env, stored()), stored());
    }

    #[test]
    fn with_neither_source_there_is_no_credential() {
        assert_eq!(resolve_credentials(env_of(&[]), stored()), stored());
        assert_eq!(resolve_credentials(env_of(&[]), None), None);
    }

    // ------------------------------------------------------------ the account

    #[test]
    fn the_account_is_its_uuid_when_present_else_its_account_id() {
        assert_eq!(
            account_identifier(&json!({ "uuid": "{u-1}", "account_id": "a-1" })).as_deref(),
            Some("{u-1}")
        );
        assert_eq!(
            account_identifier(&json!({ "account_id": "a-1" })).as_deref(),
            Some("a-1")
        );
        assert_eq!(
            account_identifier(&json!({ "uuid": "", "account_id": "a-1" })).as_deref(),
            Some("a-1"),
            "an empty uuid is no uuid"
        );
        assert_eq!(account_identifier(&json!({ "display_name": "Ada" })), None);
    }

    #[test]
    fn workspace_slugs_are_read_in_order_and_a_non_page_is_none() {
        let body = json!({
            "values": [
                { "workspace": { "slug": "w1", "uuid": "{1}" } },
                { "workspace": { "uuid": "{no-slug}" } },
                { "workspace": { "slug": "w2" } }
            ],
            "pagelen": 10
        })
        .to_string();
        assert_eq!(
            parse_workspace_slugs(&body),
            Some((vec!["w1".to_string(), "w2".to_string()], None))
        );
        assert_eq!(parse_workspace_slugs("not json"), None);
        assert_eq!(parse_workspace_slugs(r#"{"type": "error"}"#), None);
    }

    #[test]
    fn a_workspace_page_keeps_only_a_next_link_back_to_the_api() {
        let on_api = json!({
            "values": [{ "workspace": { "slug": "w1" } }],
            "next": format!("{API_BASE}/user/workspaces?page=2&pagelen=100")
        })
        .to_string();
        assert_eq!(
            parse_workspace_slugs(&on_api).and_then(|(_, next)| next),
            Some(format!("{API_BASE}/user/workspaces?page=2&pagelen=100"))
        );
        // The credential travels with every followed link, so a `next`
        // pointing anywhere else is dropped rather than requested.
        let elsewhere = json!({
            "values": [],
            "next": "https://evil.example/user/workspaces?page=2"
        })
        .to_string();
        assert_eq!(parse_workspace_slugs(&elsewhere), Some((vec![], None)));
        let not_a_string = json!({ "values": [], "next": 2 }).to_string();
        assert_eq!(parse_workspace_slugs(&not_a_string), Some((vec![], None)));
    }

    // ------------------------------------------------------------ the parser

    fn full_pull_request() -> Value {
        let author = user("me", "{me}");
        json!({
            "id": 42,
            "title": "Add the panel",
            "state": "OPEN",
            "draft": true,
            "author": author,
            "source": {
                "branch": { "name": "feature/panel" },
                "repository": { "full_name": "ada/specforge" }
            },
            "destination": {
                "branch": { "name": "main" },
                "repository": { "full_name": "acme/specforge" }
            },
            "participants": [
                participant(user("rev-1", "{r1}"), true, Some("approved")),
            ],
            "reviewers": [user("rev-1", "{r1}"), user("rev-2", "{r2}")],
            "task_count": 3,
            "updated_on": "2026-09-01T12:00:00.123456+00:00",
            "links": { "html": { "href": "https://bitbucket.org/acme/specforge/pull-requests/42" } }
        })
    }

    #[test]
    fn a_full_entry_parses_into_every_field() {
        let body = json!({ "values": [full_pull_request()] }).to_string();
        let rows = parse_pull_requests(&body).expect("a page");
        assert_eq!(
            rows,
            vec![PullRequestSummary {
                id: 42,
                title: "Add the panel".to_string(),
                repo_full_name: "acme/specforge".to_string(),
                source_branch: "feature/panel".to_string(),
                destination_branch: "main".to_string(),
                url: "https://bitbucket.org/acme/specforge/pull-requests/42".to_string(),
                draft: true,
                updated_at_unix: 1_788_264_000,
                review: Some(ReviewSummary {
                    approvals: 1,
                    changes_requested: 0,
                    pending: 1,
                }),
                open_tasks: 3,
                // The GitHub-only signals are never filled from BitBucket.
                author: None,
                checks: None,
                conflicting: false,
                unresolved_threads: 0,
                source_repo_full_name: "ada/specforge".to_string(),
            }]
        );
    }

    #[test]
    fn missing_optional_fields_default_rather_than_fail() {
        // No participants, task_count, draft or links — the list shape without
        // the review fields, or a trimmed-down future response.
        let body = json!({
            "values": [{
                "id": 7,
                "title": "Bare",
                "source": { "branch": { "name": "b" } },
                "destination": { "branch": { "name": "main" } },
                "updated_on": "2026-09-01T12:00:00+00:00"
            }]
        })
        .to_string();
        let rows = parse_pull_requests(&body).expect("a page");
        assert_eq!(rows.len(), 1);
        let pr = &rows[0];
        assert_eq!(pr.id, 7);
        assert_eq!(pr.review, None, "no participants means unknown, not zero");
        assert_eq!(pr.open_tasks, 0);
        assert!(!pr.draft);
        assert_eq!(pr.url, "");
        assert_eq!(pr.repo_full_name, "");
        assert_eq!(pr.updated_at_unix, 1_788_264_000);
    }

    #[test]
    fn only_an_https_web_link_becomes_the_row_url() {
        for href in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "http://bitbucket.org/x",
            "https://evil.example/bitbucket.org/x",
            "https://bitbucket.org.evil.example/x",
        ] {
            let body = json!({ "values": [{ "id": 1, "links": { "html": { "href": href } } }] })
                .to_string();
            assert_eq!(parse_pull_requests(&body).unwrap()[0].url, "", "{href}");
        }
    }

    #[test]
    fn an_entry_without_an_id_is_skipped_and_a_non_page_is_none() {
        let body = json!({ "values": [{ "title": "no id" }, { "id": 9 }] }).to_string();
        let rows = parse_pull_requests(&body).expect("a page");
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![9]);

        assert_eq!(parse_pull_requests("<html>"), None);
        assert_eq!(parse_pull_requests(r#"{"values": {}}"#), None);
        assert_eq!(
            parse_pull_requests(r#"{"values": []}"#),
            Some(Vec::new()),
            "an empty page is a page"
        );
    }

    #[test]
    fn a_huge_task_count_saturates_instead_of_wrapping() {
        let body = json!({ "values": [{ "id": 1, "task_count": 5_000_000_000u64 }] }).to_string();
        assert_eq!(parse_pull_requests(&body).unwrap()[0].open_tasks, u32::MAX);
    }

    // ------------------------------------------------------------ the review

    #[test]
    fn the_authors_own_approval_is_excluded() {
        let me = user("me", "{me}");
        let pr = json!({
            "author": me,
            "participants": [
                participant(user("me", "{me}"), true, Some("approved")),
                participant(user("other", "{o}"), true, Some("approved")),
            ],
        });
        let review = summarize_review(&pr).expect("participants present");
        assert_eq!(review.approvals, 1);
        assert_eq!(review.changes_requested, 0);
    }

    #[test]
    fn the_authors_own_change_request_is_excluded() {
        let pr = json!({
            "author": user("me", "{me}"),
            "participants": [participant(user("me", "{me}"), false, Some("changes_requested"))],
        });
        assert_eq!(summarize_review(&pr).unwrap().changes_requested, 0);
    }

    #[test]
    fn pending_reviewers_are_counted_from_the_reviewers_list() {
        let (a, b, c) = (user("a", "{a}"), user("b", "{b}"), user("c", "{c}"));
        let pr = json!({
            "author": user("me", "{me}"),
            "participants": [
                participant(a.clone(), true, Some("approved")),
                participant(b.clone(), false, Some("changes_requested")),
                // Commented only: a participant, but no response.
                participant(c.clone(), false, None),
            ],
            "reviewers": [a, b, c],
        });
        assert_eq!(
            summarize_review(&pr),
            Some(ReviewSummary {
                approvals: 1,
                changes_requested: 1,
                pending: 1,
            })
        );
    }

    #[test]
    fn a_response_without_participants_has_no_summary() {
        let pr = json!({ "author": user("me", "{me}"), "reviewers": [user("a", "{a}")] });
        assert_eq!(summarize_review(&pr), None);
        let pr = json!({ "participants": null });
        assert_eq!(summarize_review(&pr), None);
    }

    #[test]
    fn participants_without_reviewers_have_nothing_pending() {
        let pr = json!({
            "author": user("me", "{me}"),
            "participants": [participant(user("a", "{a}"), true, Some("approved"))],
        });
        assert_eq!(
            summarize_review(&pr),
            Some(ReviewSummary {
                approvals: 1,
                changes_requested: 0,
                pending: 0,
            })
        );
    }

    #[test]
    fn accounts_match_by_account_id_first_then_by_uuid() {
        // Both carry an account_id: it decides, even when the uuids agree.
        assert!(same_account(
            Some(&json!({ "account_id": "x", "uuid": "{1}" })),
            Some(&json!({ "account_id": "x", "uuid": "{2}" }))
        ));
        assert!(!same_account(
            Some(&json!({ "account_id": "x", "uuid": "{1}" })),
            Some(&json!({ "account_id": "y", "uuid": "{1}" }))
        ));
        // Either lacks one: the uuid decides.
        assert!(same_account(
            Some(&json!({ "uuid": "{1}" })),
            Some(&json!({ "account_id": "y", "uuid": "{1}" }))
        ));
        assert!(!same_account(
            Some(&json!({ "uuid": "{1}" })),
            Some(&json!({ "uuid": "{2}" }))
        ));
        // Nothing to match on is never a match.
        assert!(!same_account(Some(&json!({})), Some(&json!({}))));
        assert!(!same_account(None, Some(&json!({ "uuid": "{1}" }))));
    }

    #[test]
    fn a_reviewer_matched_by_uuid_alone_counts_as_responded() {
        // The participant entry carries only a uuid, the reviewer entry both —
        // the fallback is what keeps this reviewer from reading as pending.
        let pr = json!({
            "author": user("me", "{me}"),
            "participants": [participant(json!({ "uuid": "{a}" }), true, Some("approved"))],
            "reviewers": [user("a", "{a}")],
        });
        assert_eq!(summarize_review(&pr).unwrap().pending, 0);
    }

    // ------------------------------------------------------------ the chain

    /// A scripted BitBucket: each URL prefix maps to a `(status, body)` reply,
    /// or to a transport error; every URL requested is recorded.
    struct FakeApi {
        routes: Vec<(String, Option<(u16, String)>)>,
        requested: Vec<String>,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                routes: Vec::new(),
                requested: Vec::new(),
            }
        }

        fn route(mut self, prefix: &str, status: u16, body: Value) -> Self {
            self.routes.push((
                format!("{API_BASE}{prefix}"),
                Some((status, body.to_string())),
            ));
            self
        }

        fn route_raw(mut self, prefix: &str, status: u16, body: &str) -> Self {
            self.routes.push((
                format!("{API_BASE}{prefix}"),
                Some((status, body.to_string())),
            ));
            self
        }

        fn offline(mut self, prefix: &str) -> Self {
            self.routes.push((format!("{API_BASE}{prefix}"), None));
            self
        }

        fn fetch(&mut self) -> FetchResult {
            let routes = &self.routes;
            let requested = &mut self.requested;
            fetch_snapshot_with(|url| {
                requested.push(url.to_string());
                let (_, reply) = routes
                    .iter()
                    .find(|(prefix, _)| url.starts_with(prefix.as_str()))
                    .unwrap_or_else(|| panic!("unscripted request: {url}"));
                let (status, body) = reply.clone()?;
                let verdict = classify(status, None);
                let body = (verdict == Verdict::Read).then_some(body);
                Some(Reply { verdict, body })
            })
        }
    }

    fn a_page(prs: Vec<Value>) -> Value {
        json!({ "values": prs, "pagelen": 50 })
    }

    fn a_pr(id: u64, updated_on: &str) -> Value {
        json!({
            "id": id,
            "title": format!("PR {id}"),
            "author": user("me", "{me}"),
            "participants": [],
            "reviewers": [],
            "updated_on": updated_on,
            "links": { "html": { "href": format!("https://bitbucket.org/x/y/pull-requests/{id}") } }
        })
    }

    fn two_workspace_api() -> FakeApi {
        FakeApi::new()
            .route(
                "/user/workspaces",
                200,
                json!({
                    "values": [
                        { "workspace": { "slug": "w1" } },
                        { "workspace": { "slug": "w2" } }
                    ]
                }),
            )
            .route("/user", 200, json!({ "uuid": "{me}", "account_id": "me" }))
            .route(
                "/workspaces/w1/pullrequests/",
                200,
                a_page(vec![a_pr(1, "2026-09-01T10:00:00+00:00")]),
            )
            .route(
                "/workspaces/w2/pullrequests/",
                200,
                a_page(vec![a_pr(2, "2026-09-02T10:00:00+00:00")]),
            )
    }

    #[test]
    fn pull_requests_are_listed_from_every_workspace_in_two_plus_w_requests() {
        let mut api = two_workspace_api();

        let (pull_requests, skipped_workspaces) = match api.fetch() {
            FetchResult::Ok {
                pull_requests,
                skipped_workspaces,
            } => (pull_requests, skipped_workspaces),
            other => panic!("expected rows, got {other:?}"),
        };
        // Newest first, regardless of workspace.
        assert_eq!(
            pull_requests.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert!(skipped_workspaces.is_empty());
        assert_eq!(api.requested.len(), 2 + 2, "2 + W requests");
        assert_eq!(api.requested[0], format!("{API_BASE}/user"));
        assert_eq!(api.requested[1], build_workspaces_url());
        assert!(api.requested[1].ends_with("?pagelen=100"));
        assert_eq!(api.requested[2], build_pull_requests_url("w1", "{me}"));
        assert_eq!(api.requested[3], build_pull_requests_url("w2", "{me}"));
    }

    #[test]
    fn every_request_goes_to_the_official_api_and_never_to_the_removed_endpoint() {
        let mut api = two_workspace_api();
        api.fetch();
        for url in &api.requested {
            assert!(url.starts_with("https://api.bitbucket.org/2.0/"), "{url}");
            assert!(
                !url.starts_with("https://api.bitbucket.org/2.0/pullrequests/"),
                "the removed cross-workspace endpoint: {url}"
            );
        }
    }

    #[test]
    fn an_inaccessible_workspace_is_skipped_and_named() {
        for status in [403, 404] {
            let mut api = FakeApi::new()
                .route(
                    "/user/workspaces",
                    200,
                    json!({
                        "values": [
                            { "workspace": { "slug": "w1" } },
                            { "workspace": { "slug": "w2" } }
                        ]
                    }),
                )
                .route("/user", 200, json!({ "uuid": "{me}" }))
                .route(
                    "/workspaces/w1/pullrequests/",
                    200,
                    a_page(vec![a_pr(1, "2026-09-01T10:00:00+00:00")]),
                )
                .route(
                    "/workspaces/w2/pullrequests/",
                    status,
                    json!({ "type": "error" }),
                );

            assert_eq!(
                api.fetch(),
                FetchResult::Ok {
                    pull_requests: parse_pull_requests(
                        &a_page(vec![a_pr(1, "2026-09-01T10:00:00+00:00")]).to_string()
                    )
                    .unwrap(),
                    skipped_workspaces: vec!["w2".to_string()],
                },
                "status {status}"
            );
        }
    }

    #[test]
    fn a_401_anywhere_is_unauthenticated() {
        let mut api = FakeApi::new().route("/user", 401, json!({}));
        assert_eq!(api.fetch(), FetchResult::Unauthenticated);
        assert_eq!(api.requested.len(), 1, "the chain stops at the rejection");

        let mut api = two_workspace_api();
        api.routes.insert(
            0,
            (
                format!("{API_BASE}/workspaces/w2/pullrequests/"),
                Some((401, "{}".to_string())),
            ),
        );
        assert_eq!(api.fetch(), FetchResult::Unauthenticated);
    }

    #[test]
    fn a_429_carries_its_hint_back() {
        let fetched = fetch_snapshot_with(|url| {
            if url.ends_with("/user") {
                Some(Reply {
                    verdict: Verdict::Read,
                    body: Some(json!({ "uuid": "{me}" }).to_string()),
                })
            } else {
                Some(Reply {
                    verdict: classify(429, Some("30")),
                    body: None,
                })
            }
        });
        assert_eq!(
            fetched,
            FetchResult::RateLimited {
                retry_after: Some(30)
            }
        );
    }

    #[test]
    fn transport_errors_and_other_statuses_are_transient() {
        let mut api = FakeApi::new().offline("/user");
        assert_eq!(api.fetch(), FetchResult::Transient);

        let mut api = FakeApi::new().route("/user", 500, json!({}));
        assert_eq!(api.fetch(), FetchResult::Transient);

        // A 404 on the account itself skips nothing — it fails the refresh.
        let mut api = FakeApi::new().route("/user", 404, json!({}));
        assert_eq!(api.fetch(), FetchResult::Transient);

        let mut api = two_workspace_api();
        api.routes
            .insert(0, (format!("{API_BASE}/workspaces/w1/pullrequests/"), None));
        assert_eq!(api.fetch(), FetchResult::Transient);
    }

    #[test]
    fn a_forbidden_account_resource_is_a_credential_problem() {
        // The token exists but lacks the account read scope: Settings, not
        // patience, fixes this, so it must not read as an outage.
        let mut api = FakeApi::new().route("/user", 403, json!({}));
        assert_eq!(api.fetch(), FetchResult::Unauthenticated);

        let mut api = FakeApi::new()
            .route("/user/workspaces", 403, json!({}))
            .route("/user", 200, json!({ "uuid": "{me}" }));
        assert_eq!(api.fetch(), FetchResult::Unauthenticated);
    }

    #[test]
    fn the_workspace_listing_follows_next_until_it_ends() {
        let page2 = format!("{API_BASE}/user/workspaces?page=2&pagelen=100");
        let mut api = FakeApi::new()
            .route(
                "/user/workspaces?page=2",
                200,
                json!({ "values": [{ "workspace": { "slug": "w2" } }] }),
            )
            .route(
                "/user/workspaces?pagelen",
                200,
                json!({ "values": [{ "workspace": { "slug": "w1" } }], "next": page2 }),
            )
            .route("/user", 200, json!({ "uuid": "{me}" }))
            .route(
                "/workspaces/w1/pullrequests/",
                200,
                a_page(vec![a_pr(1, "2026-09-01T10:00:00+00:00")]),
            )
            .route(
                "/workspaces/w2/pullrequests/",
                200,
                a_page(vec![a_pr(2, "2026-09-02T10:00:00+00:00")]),
            );

        let rows = match api.fetch() {
            FetchResult::Ok { pull_requests, .. } => pull_requests,
            other => panic!("expected rows, got {other:?}"),
        };
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 1]);
        // 1 (account) + 2 (workspace pages) + 2 (workspaces).
        assert_eq!(api.requested.len(), 5);
        assert_eq!(api.requested[1], build_workspaces_url());
        assert_eq!(api.requested[2], page2);
    }

    #[test]
    fn the_workspace_listing_stops_at_the_page_bound() {
        // Every page points at itself: without the bound this would never end.
        let looping = build_workspaces_url();
        let mut api = FakeApi::new()
            .route(
                "/user/workspaces?pagelen",
                200,
                json!({ "values": [{ "workspace": { "slug": "w1" } }], "next": looping }),
            )
            .route("/user", 200, json!({ "uuid": "{me}" }))
            .route("/workspaces/w1/pullrequests/", 200, a_page(vec![]));
        assert!(matches!(api.fetch(), FetchResult::Ok { .. }));
        let pages = api
            .requested
            .iter()
            .filter(|u| u.contains("/user/workspaces"))
            .count();
        assert_eq!(pages, MAX_WORKSPACE_PAGES);
    }

    #[test]
    fn an_unparseable_body_is_unavailable() {
        let mut api = FakeApi::new().route_raw("/user", 200, "<html>maintenance</html>");
        assert_eq!(api.fetch(), FetchResult::Unavailable);

        // A well-formed account with nothing to identify it by.
        let mut api = FakeApi::new().route("/user", 200, json!({ "display_name": "Ada" }));
        assert_eq!(api.fetch(), FetchResult::Unavailable);

        let mut api = FakeApi::new()
            .route("/user/workspaces", 200, json!({ "unexpected": true }))
            .route("/user", 200, json!({ "uuid": "{me}" }));
        assert_eq!(api.fetch(), FetchResult::Unavailable);

        let mut api = two_workspace_api();
        api.routes.insert(
            0,
            (
                format!("{API_BASE}/workspaces/w2/pullrequests/"),
                Some((200, "not json".to_string())),
            ),
        );
        assert_eq!(api.fetch(), FetchResult::Unavailable);
    }

    #[test]
    fn a_read_verdict_without_a_body_is_unavailable() {
        assert_eq!(
            body_of(Some(Reply {
                verdict: Verdict::Read,
                body: None,
            })),
            Err(FetchResult::Unavailable)
        );
    }

    #[test]
    fn an_account_in_no_workspace_lists_nothing() {
        let mut api = FakeApi::new()
            .route("/user/workspaces", 200, json!({ "values": [] }))
            .route("/user", 200, json!({ "uuid": "{me}" }));
        assert_eq!(
            api.fetch(),
            FetchResult::Ok {
                pull_requests: Vec::new(),
                skipped_workspaces: Vec::new(),
            }
        );
        assert_eq!(api.requested.len(), 2);
    }

    // ------------------------------------------------------------ the loop's pure parts

    #[test]
    fn degrade_to_stale_keeps_the_rows_and_flags_them() {
        let ok = ok_state(vec![row(1, 100), row(2, 50)]);
        let staled = degrade_to_stale(&ok);
        assert_eq!(staled.status, PullRequestsStatus::Ok);
        assert!(staled.stale);
        assert_eq!(staled.pull_requests, ok.pull_requests);
        assert_eq!(
            staled.fetched_at_unix, ok.fetched_at_unix,
            "rows keep their fetch time"
        );
        assert_eq!(staled.skipped_workspaces, ok.skipped_workspaces);
    }

    #[test]
    fn degrade_to_stale_without_previous_rows_is_unavailable() {
        for prev in [
            ok_state(Vec::new()),
            BitbucketPullRequestsState::disabled(),
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unauthenticated),
        ] {
            let next = degrade_to_stale(&prev);
            assert_eq!(next.status, PullRequestsStatus::Unavailable, "{prev:?}");
            assert!(!next.stale);
            assert!(next.pull_requests.is_empty());
        }
    }

    #[test]
    fn next_state_maps_each_outcome() {
        let prev = ok_state(vec![row(1, 100)]);

        let ok = next_state(
            &prev,
            FetchResult::Ok {
                pull_requests: vec![row(2, 200)],
                skipped_workspaces: vec!["w9".to_string()],
            },
            5_000,
        );
        assert_eq!(
            ok,
            BitbucketPullRequestsState {
                status: PullRequestsStatus::Ok,
                stale: false,
                fetched_at_unix: Some(5_000),
                pull_requests: vec![row(2, 200)],
                skipped_workspaces: vec!["w9".to_string()],
            }
        );

        // A rejected credential drops the rows: they belonged to it.
        assert_eq!(
            next_state(&prev, FetchResult::Unauthenticated, 5_000),
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unauthenticated)
        );
        assert_eq!(
            next_state(&prev, FetchResult::Unavailable, 5_000),
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        );
        for failure in [
            FetchResult::Transient,
            FetchResult::RateLimited { retry_after: None },
        ] {
            let next = next_state(&prev, failure, 5_000);
            assert!(next.stale);
            assert_eq!(next.pull_requests, prev.pull_requests);
        }
    }

    #[test]
    fn a_new_fetch_time_alone_is_not_news() {
        let prev = ok_state(vec![row(1, 100)]);
        let refetched = BitbucketPullRequestsState {
            fetched_at_unix: Some(9_999),
            ..prev.clone()
        };
        assert!(
            !is_news(&prev, &refetched),
            "a refresh that changes nothing is silent"
        );

        // Each field that is news, on its own.
        let changed_rows = BitbucketPullRequestsState {
            pull_requests: vec![row(1, 101)],
            ..prev.clone()
        };
        let changed_stale = BitbucketPullRequestsState {
            stale: true,
            ..prev.clone()
        };
        let changed_status = BitbucketPullRequestsState {
            status: PullRequestsStatus::Unavailable,
            ..prev.clone()
        };
        let changed_skips = BitbucketPullRequestsState {
            skipped_workspaces: Vec::new(),
            ..prev.clone()
        };
        for next in [changed_rows, changed_stale, changed_status, changed_skips] {
            assert!(is_news(&prev, &next), "{next:?}");
        }
    }

    #[test]
    fn a_tiny_interval_is_floored_at_sixty_seconds() {
        assert_eq!(refresh_interval(0), Duration::from_secs(60));
        assert_eq!(refresh_interval(5), Duration::from_secs(60));
        assert_eq!(refresh_interval(60), Duration::from_secs(60));
        assert_eq!(refresh_interval(120), Duration::from_secs(120));
    }

    #[test]
    fn a_429_backs_off_by_its_hint_or_five_minutes() {
        assert_eq!(backoff(Some(42)), Duration::from_secs(42));
        assert_eq!(backoff(None), Duration::from_secs(300));
    }

    /// A hostile `Retry-After` is capped at an hour, so the loop's
    /// `now + backoff(..)` can never overflow and panic the thread.
    #[test]
    fn a_backoff_is_capped_at_an_hour() {
        assert_eq!(backoff(Some(3_600)), Duration::from_secs(3_600));
        assert_eq!(backoff(Some(3_601)), Duration::from_secs(3_600));
        assert_eq!(backoff(Some(u64::MAX)), Duration::from_secs(3_600));
        assert!(Instant::now()
            .checked_add(backoff(Some(u64::MAX)))
            .is_some());
    }

    #[test]
    fn a_refresh_is_due_after_the_interval_and_outside_a_deadline() {
        let t0 = Instant::now();
        let interval = Duration::from_secs(60);
        let at = |secs| t0 + Duration::from_secs(secs);

        assert!(refresh_due(None, false, interval, t0), "never polled");
        assert!(!refresh_due(Some(t0), false, interval, at(59)));
        assert!(refresh_due(Some(t0), false, interval, at(60)));
        // Held by the deadline: not even once the interval has long passed.
        assert!(!refresh_due(Some(t0), true, interval, at(120)));
        assert!(!refresh_due(None, true, interval, at(1)));
    }

    // ------------------------------------------------------------ the limits

    const NOW: u64 = 1_800_000_000;

    /// Whether the poller refreshes `elapsed` seconds after a reply at `NOW`,
    /// its last refresh having run then, at the 120-second default interval.
    fn poller_due(limits: &BitbucketLimits, elapsed: u64) -> bool {
        let then = Instant::now();
        refresh_due(
            Some(then),
            limits.holds_poller(NOW + elapsed),
            refresh_interval(120),
            then + Duration::from_secs(elapsed),
        )
    }

    /// What a detail GET's reply leaves on the provider: a 429's deadline,
    /// and nothing else.
    fn detail_reply(limits: &BitbucketLimits, status: u16, retry_after: Option<&str>) {
        if let Verdict::RateLimited { retry_after } = classify(status, retry_after) {
            limits.rate_limited(retry_after, NOW);
        }
    }

    fn deadline(until: u64) -> BitbucketDeadline {
        BitbucketDeadline { until }
    }

    fn deferred_until(admission: Admission<BitbucketDeadline>) -> Option<u64> {
        match admission {
            Admission::Deferred { until } => Some(until),
            _ => None,
        }
    }

    fn permit(admission: Admission<BitbucketDeadline>) -> DetailPermit<BitbucketDeadline> {
        match admission {
            Admission::Admitted(permit) => permit,
            other => panic!("expected the read to be admitted, got {other:?}"),
        }
    }

    /// `bitbucket-pull-requests`: *A detail read's rate limit holds the
    /// poller*.
    #[test]
    fn a_detail_reads_429_holds_the_poller() {
        let limits = BitbucketLimits::new();
        detail_reply(&limits, 429, Some("900"));
        assert_eq!(limits.deadlines(), deadline(NOW + 900));
        for elapsed in [120, 240, 899] {
            assert!(!poller_due(&limits, elapsed), "{elapsed} s after the reply");
        }
        assert!(poller_due(&limits, 900));
    }

    /// `bitbucket-pull-requests`: *The poller's rate limit holds the detail
    /// reads*.
    #[test]
    fn the_pollers_bare_429_holds_detail_reads_for_five_minutes() {
        let limits = BitbucketLimits::new();
        let mut api = FakeApi::new()
            .route("/user/workspaces", 429, json!({}))
            .route("/user", 200, json!({ "uuid": "{me}" }));
        let FetchResult::RateLimited { retry_after } = api.fetch() else {
            panic!("the poller's 429 is a rate limit");
        };
        limits.rate_limited(retry_after, NOW);

        assert_eq!(limits.deadlines(), deadline(NOW + 300));
        assert_eq!(
            deferred_until(admit_or_fail(&limits, true, NOW + 299)),
            Some(NOW + 300)
        );
        drop(permit(admit_or_fail(&limits, true, NOW + 300)));
    }

    #[test]
    fn the_deadline_is_never_more_than_an_hour_ahead() {
        for (retry_after, ahead) in [("3600", 3_600), ("3601", 3_600), ("7200", 3_600)] {
            let limits = BitbucketLimits::new();
            detail_reply(&limits, 429, Some(retry_after));
            assert_eq!(
                limits.deadlines(),
                deadline(NOW + ahead),
                "Retry-After: {retry_after}"
            );
        }
    }

    /// A shorter delay never cuts short a longer deadline still holding.
    #[test]
    fn the_deadline_only_ever_moves_later() {
        let limits = BitbucketLimits::new();
        detail_reply(&limits, 429, Some("900"));
        detail_reply(&limits, 429, Some("60"));
        assert_eq!(limits.deadlines(), deadline(NOW + 900));
        detail_reply(&limits, 429, Some("1200"));
        assert_eq!(limits.deadlines(), deadline(NOW + 1_200));
    }

    #[test]
    fn the_poller_waits_until_exactly_the_deadline() {
        let limits = BitbucketLimits::new();
        assert!(!limits.holds_poller(NOW), "no deadline set");
        detail_reply(&limits, 429, Some("300"));
        assert!(limits.holds_poller(NOW + 299));
        assert!(!limits.holds_poller(NOW + 300));
    }

    /// `bitbucket-pull-requests`: *Polling With Caching and Backoff*, the
    /// tick rule for a provider enabled inside its deadline.
    #[test]
    fn an_enabled_tick_inside_the_deadline_publishes_unavailable() {
        let watcher = WatcherManager::new(Duration::from_millis(50));
        let mut events = watcher.subscribe();
        let handle = BitbucketPullRequestsHandle::new();
        let limits = BitbucketLimits::new();
        let announced = |events: &mut tokio::sync::broadcast::Receiver<CacheEvent>| {
            matches!(
                events.try_recv(),
                Ok(CacheEvent::BitbucketPullRequestsUpdated)
            )
        };

        assert!(!publish_held_unavailable(&handle, &watcher, &limits, NOW));
        assert_eq!(handle.get(), BitbucketPullRequestsState::disabled());
        assert!(!announced(&mut events));

        detail_reply(&limits, 429, Some("1200"));
        assert!(publish_held_unavailable(
            &handle,
            &watcher,
            &limits,
            NOW + 1
        ));
        assert_eq!(
            handle.get(),
            BitbucketPullRequestsState::status_only(PullRequestsStatus::Unavailable)
        );
        assert!(announced(&mut events));
        // Published once: the next tick inside the deadline changes nothing.
        assert!(!publish_held_unavailable(
            &handle,
            &watcher,
            &limits,
            NOW + 2
        ));
        assert!(!announced(&mut events));
        // Rows already shown are never replaced by it.
        handle.set(ok_state(vec![row(1, 100)]));
        assert!(!publish_held_unavailable(
            &handle,
            &watcher,
            &limits,
            NOW + 3
        ));
        assert_eq!(handle.get(), ok_state(vec![row(1, 100)]));
        // At the deadline it lets go, and the first refresh reads.
        handle.set(BitbucketPullRequestsState::disabled());
        assert!(!publish_held_unavailable(
            &handle,
            &watcher,
            &limits,
            NOW + 1_200
        ));
        assert!(!announced(&mut events));
    }

    /// `bitbucket-pull-requests`: *A detail read's failure leaves the
    /// snapshot unchanged*. A detail read's 401 is its own outcome and its
    /// 429 only sets the deadline; and when that 429 lands while the
    /// poller's chain is running, the chain stops at its next request and
    /// its result is dropped, so the snapshot is not marked stale either.
    #[test]
    fn a_detail_reads_401_or_429_leaves_the_snapshot_unchanged() {
        let handle = BitbucketPullRequestsHandle::new();
        let fresh = ok_state(vec![row(1, 100)]);
        handle.set(fresh.clone());
        let limits = BitbucketLimits::new();

        detail_reply(&limits, 401, None);
        assert_eq!(limits.deadlines(), BitbucketDeadline::default());
        // The poller's chain: a detail read's 429 lands after its first
        // request.
        let mut requested = Vec::new();
        let refreshed = refresh(
            |url| {
                requested.push(url.to_string());
                detail_reply(&limits, 429, None);
                Some(Reply {
                    verdict: Verdict::Read,
                    body: Some(json!({ "uuid": "{me}" }).to_string()),
                })
            },
            || !limits.holds_poller(NOW),
        );
        assert_eq!(refreshed, None, "stopped short, the result is dropped");
        assert_eq!(requested, [format!("{API_BASE}/user")]);
        assert_eq!(limits.deadlines(), deadline(NOW + 300));
        assert_eq!(handle.get(), fresh);
    }

    /// A chain that runs to its end keeps its result, whatever it is.
    #[test]
    fn a_refresh_that_is_never_stopped_keeps_its_result() {
        let mut api = two_workspace_api();
        let expected = api.fetch();
        let routes = &api.routes;
        let refreshed = refresh(
            |url| {
                let (_, reply) = routes
                    .iter()
                    .find(|(prefix, _)| url.starts_with(prefix.as_str()))
                    .unwrap();
                let (status, body) = reply.clone()?;
                let verdict = classify(status, None);
                let body = (verdict == Verdict::Read).then_some(body);
                Some(Reply { verdict, body })
            },
            || true,
        );
        assert_eq!(refreshed, Some(expected));
        // Switched off before its first request: nothing is sent.
        assert_eq!(refresh(|_| panic!("nothing is sent"), || false), None);
    }

    /// `pull-request-viewer`: *At most two detail reads per provider* — the
    /// third waits while two are in flight, and is sent when one ends.
    #[test]
    fn a_third_bitbucket_read_waits_while_two_are_in_flight() {
        let limits = BitbucketLimits::new();
        let first = permit(admit_or_fail(&limits, true, NOW));
        let _second = permit(admit_or_fail(&limits, true, NOW));
        let (answered, answer) = std::sync::mpsc::channel();
        let waiting = limits.clone();
        std::thread::spawn(move || {
            let _ = answered.send(waiting.admit(|| true, || NOW));
        });
        wait_until(|| limits.waiting() == 1);
        assert_eq!(limits.in_flight(), 2, "not sent while both are in flight");
        drop(first);
        let third = answer
            .recv_timeout(Duration::from_secs(10))
            .expect("sent when one ends, with no further ask");
        assert!(matches!(third, Admission::Admitted(_)));
    }

    /// `pull-request-viewer`: *Shared Backoff and Detail Budget*: the
    /// budget, two reads admitted just under it at their largest, and the
    /// poller at its 60-second floor for the documented workspace count fit
    /// BitBucket's 1,000 requests an hour.
    #[test]
    fn the_budget_leaves_room_for_two_reads_past_it_and_the_poller() {
        assert_eq!(DETAIL_MAX_REQUESTS, 23);
        let overshoot = 2 * DETAIL_MAX_REQUESTS - 1;
        let refreshes_an_hour = (3_600 / MIN_REFRESH_SECS) as usize;
        let poller = refreshes_an_hour * (2 + BUDGET_WORKSPACES);
        assert_eq!((overshoot, poller), (45, 540));
        assert!(DETAIL_BUDGET + overshoot + poller <= 1_000);
        assert_eq!(DETAIL_BUDGET, 400);
    }

    /// The overshoot the budget leaves room for: two reads admitted just
    /// under it, each sending the most requests a read sends, pass it by
    /// `2 × 23 − 1` at most.
    #[test]
    fn two_reads_admitted_just_under_the_budget_pass_it_by_at_most_45() {
        let limits = BitbucketLimits::new();
        let filler = permit(admit_or_fail(&limits, true, NOW));
        for _ in 0..DETAIL_BUDGET - 1 {
            filler.request(NOW).unwrap();
        }
        drop(filler);
        let first = permit(admit_or_fail(&limits, true, NOW));
        let second = permit(admit_or_fail(&limits, true, NOW));
        for read in [&first, &second] {
            for _ in 0..DETAIL_MAX_REQUESTS {
                assert_eq!(read.request(NOW), Ok(()));
            }
        }
        assert_eq!(
            limits.spent(NOW) - DETAIL_BUDGET,
            2 * DETAIL_MAX_REQUESTS - 1
        );
        drop((first, second));
        assert_eq!(
            deferred_until(admit_or_fail(&limits, true, NOW)),
            Some(NOW + 3_600)
        );
    }

    #[test]
    fn the_handle_starts_disabled_and_serves_what_was_set() {
        let handle = BitbucketPullRequestsHandle::default();
        assert_eq!(handle.get(), BitbucketPullRequestsState::disabled());
        let state = ok_state(vec![row(1, 100)]);
        handle.set(state.clone());
        assert_eq!(handle.clone().get(), state, "clones share the snapshot");
    }
}
