//! The pull-request viewer's GitHub detail read (`pull-request-viewer`:
//! *GitHub Detail Reads*; `github-pull-requests`: *GitHub Privacy and
//! Safety*; design D6).
//!
//! A read is one POST of [`DETAIL_QUERY`] to `https://api.github.com/graphql`,
//! then the REST pages of the pull request's changed files, at most
//! [`DETAIL_MAX_FILES_PAGES`] of [`FILES_PER_PAGE`]. GraphQL's changed files
//! carry no patch text, and the one-call diff media type fails on exactly the
//! large pull requests a reviewer most needs help with.
//!
//! The query is fixed at build time and is never a mutation or subscription.
//! The owner, name and number it reads come from the snapshot row the service
//! matched, never from a caller, and travel in GraphQL's `variables`, so its
//! text stays byte-identical whichever pull request it reads. Every request
//! follows no redirect and carries the token only in `Authorization`, and
//! nothing here carries the token's text into a result.
//!
//! The recipe runs over an injected transport and asks, before each request,
//! whether it may still send: the provider enabled under the credential the
//! read started with, no deadline holding, and the request counted against
//! the hourly budget (`crate::pull_request_read`).

use std::io::Read;

use openspec_core::diff::REQUESTED_FILE_BYTES_LIMIT;
use openspec_core::{
    diff_versions_with_bodies, parse_hunks_with_bodies, DiffContent, DiffFile, FileStatus,
};
use serde_json::{json, Value};

use crate::bitbucket::encode;
use crate::github::{
    rate_limited_reply, GithubLimits, GithubRequest, RateHeaders, RateLimit, Reply, API_URL,
    DETAIL_MAX_FILES_PAGES, USER_AGENT,
};
use crate::pull_request_cache::FileFetch;
use crate::pull_request_detail::{
    hunk_digests, ConversationEntry, DiffSide, FetchPaths, Fetched, PatchDigest, PullRequestCheck,
    PullRequestCheckState, PullRequestComment, PullRequestReference, ReadEnd, ReadFile, ReadParts,
    ReviewState, ReviewThread,
};
use crate::pull_request_limits::Deadlines;
use crate::pull_requests::saturating_u32;
use crate::quota::parse_rfc3339_to_unix;
use crate::usage_http::{self, Auth};

/// Where every files request goes, beneath the matched pull request.
const REPOS_URL: &str = "https://api.github.com/repos";
/// Files asked for per page. GitHub is reported to omit `patch` past the
/// 70th file of a 100-file page, so a page stays well under that.
pub const FILES_PER_PAGE: usize = 50;
/// Where a comment's web page may live. Only a link on GitHub's own site
/// survives into the model, as only one does into a row.
const WEB_URL_PREFIX: &str = "https://github.com/";

/// The one query a detail read sends, fixed at build time (design D6).
///
/// The `states` filter leaves out the account's own pending review, which
/// nobody else can see; its inline comments still come back in
/// `reviewThreads` for their author, so each thread comment's `state` is read
/// and every `PENDING` one dropped. Thread and comment ids, and each thread's
/// sides, are read now so a later change can reply and anchor without
/// changing this text.
pub(crate) const DETAIL_QUERY: &str = r#"query SpecForgePullRequestDetail($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      body author { login }
      baseRefName headRefName baseRefOid headRefOid
      reviews(first: 50, states: [APPROVED, CHANGES_REQUESTED, COMMENTED, DISMISSED]) {
        nodes { id author { login } state body submittedAt url isMinimized minimizedReason }
      }
      commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 100) { nodes {
        ... on CheckRun { name status conclusion detailsUrl }
        ... on StatusContext { context state targetUrl }
      } } } } } }
      comments(first: 100) { nodes { id author { login } body createdAt url isMinimized minimizedReason } }
      reviewThreads(first: 100) { nodes {
        id isResolved isOutdated path line originalLine startLine originalStartLine diffSide startDiffSide
        comments(first: 50) { nodes { id author { login } body createdAt url state isMinimized minimizedReason } }
      } }
      files(first: 100) { totalCount }
    }
  }
}
"#;

/// The query's request body: the constant text, and the matched row's owner,
/// name and number as its only variables.
pub(crate) fn query_body(owner: &str, name: &str, number: u64) -> String {
    json!({
        "query": DETAIL_QUERY,
        "variables": { "owner": owner, "name": name, "number": number },
    })
    .to_string()
}

/// Page `page` of the matched pull request's changed files.
pub(crate) fn files_url(owner: &str, name: &str, number: u64, page: usize) -> String {
    format!(
        "{REPOS_URL}/{}/{}/pulls/{number}/files?per_page={FILES_PER_PAGE}&page={page}",
        encode(owner),
        encode(name),
    )
}

/// The comparison of the cached detail's `base` and `head`, read by a file
/// read for `merge_base_commit.sha`: the commit GitHub diffs a pull request
/// against. `per_page=1` keeps its list of commits to one.
pub(crate) fn compare_url(owner: &str, name: &str, base: &str, head: &str) -> String {
    format!(
        "{REPOS_URL}/{}/{}/compare/{}...{}?per_page=1",
        encode(owner),
        encode(name),
        encode(base),
        encode(head),
    )
}

/// One version of a file: `path` at `commit`, each of the path's segments
/// percent-encoded on its own, so a `+` or a space in a name reaches GitHub
/// as itself.
pub(crate) fn contents_url(owner: &str, name: &str, path: &str, commit: &str) -> String {
    let path: Vec<String> = path.split('/').map(encode).collect();
    format!(
        "{REPOS_URL}/{}/{}/contents/{}?ref={}",
        encode(owner),
        encode(name),
        path.join("/"),
        encode(commit),
    )
}

/// The most of a version a contents GET reads: one byte past the per-file
/// ceiling, so a longer version is known to be too large without reading it
/// all.
pub(crate) const VERSION_READ_LIMIT: usize = REQUESTED_FILE_BYTES_LIMIT + 1;

/// A contents GET's reply: its status, its rate-limit headers, and its body's
/// bytes, up to [`VERSION_READ_LIMIT`], for a 2xx, 403 or 429. A transport
/// error, a cut-off body included, is no reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawReply {
    pub(crate) status: u16,
    pub(crate) headers: RateHeaders,
    pub(crate) body: Option<Vec<u8>>,
}

/// One GET of a version, as raw bytes (`application/vnd.github.raw`). Nothing
/// about it — the URL, the error, the reply — is logged, and the token is
/// only ever inside the header.
pub(crate) fn send_get_raw(url: &str, token: &str) -> Option<RawReply> {
    let mut response = usage_http::get_without_redirects(url, Auth::Bearer(token))
        .header("Accept", "application/vnd.github.raw")
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let headers = RateHeaders::of_response(&response);
    let status = response.status().as_u16();
    let body = if matches!(status, 200..=299 | 403 | 429) {
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(VERSION_READ_LIMIT as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    } else {
        None
    };
    Some(RawReply {
        status,
        headers,
        body,
    })
}

/// One GET of a files page. Nothing about it — the URL, the error, the reply
/// — is logged, and the token is only ever inside the header.
pub(crate) fn send_get(url: &str, token: &str) -> Option<Reply> {
    let response = usage_http::get_without_redirects(url, Auth::Bearer(token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    Some(Reply::read(response))
}

/// One read of `pull_request`, spelt as its matched row spells it, over an
/// injected transport: the query through `post`, then up to
/// [`DETAIL_MAX_FILES_PAGES`] files pages through `get`, stopping at a short
/// page. `clear` is asked before every request and ends the read when it may
/// not be sent.
pub(crate) fn read_with(
    pull_request: &PullRequestReference,
    mut post: impl FnMut(&str, String) -> Option<Reply>,
    mut get: impl FnMut(&str) -> Option<Reply>,
    clear: impl Fn() -> Result<(), ReadEnd>,
    limits: &GithubLimits,
    now: impl Fn() -> u64,
) -> Result<ReadParts, ReadEnd> {
    let PullRequestReference {
        owner,
        repo: name,
        number,
        ..
    } = pull_request;
    clear()?;
    let query = post(API_URL, query_body(owner, name, *number));
    let read = query_verdict(query, limits, now())?;
    let mut files = Vec::new();
    for page in 1..=DETAIL_MAX_FILES_PAGES {
        clear()?;
        let entries = files_verdict(get(&files_url(owner, name, *number, page)), limits, now())?;
        files.extend(entries.iter().filter_map(read_file));
        if entries.len() < FILES_PER_PAGE {
            break;
        }
    }
    Ok(parts(&read, files))
}

/// One file read of a file of `pull_request` that GitHub sent without its
/// patch (`pull-request-viewer`: *GitHub Detail Reads*), spelt as its matched
/// row spells it, over an injected transport: the compare through `get` when
/// `fetch` carries no merge base yet, then each version the file has through
/// `get_raw`, its old path at the merge base and its new path at the head.
/// `clear` is asked before every request and ends the read when it may not
/// be sent. Returns the content the two versions diff to with its hunks'
/// body digests, and the merge base they were read by.
pub(crate) fn read_file_with(
    pull_request: &PullRequestReference,
    fetch: &FileFetch,
    mut get: impl FnMut(&str) -> Option<Reply>,
    mut get_raw: impl FnMut(&str) -> Option<RawReply>,
    clear: impl Fn() -> Result<(), ReadEnd>,
    limits: &GithubLimits,
    now: impl Fn() -> u64,
) -> Result<(Fetched, String), ReadEnd> {
    let PullRequestReference {
        owner, repo: name, ..
    } = pull_request;
    let merge_base = match &fetch.merge_base {
        Some(merge_base) => merge_base.clone(),
        None => {
            clear()?;
            let url = compare_url(owner, name, &fetch.base, &fetch.head);
            compare_verdict(get(&url), limits, now())?
        }
    };
    let mut version = |path: Option<&String>, commit: &str| -> Result<Option<Vec<u8>>, ReadEnd> {
        let Some(path) = path else {
            return Ok(None);
        };
        clear()?;
        let url = contents_url(owner, name, path, commit);
        contents_verdict(get_raw(&url), limits, now()).map(Some)
    };
    let old = version(fetch.paths.old.as_ref(), &merge_base)?;
    let new = version(fetch.paths.new.as_ref(), &fetch.head)?;
    let diff = diff_versions_with_bodies(old.as_deref(), new.as_deref());
    let hunks = matches!(diff.content, DiffContent::Hunks { .. }).then(|| {
        hunk_digests(
            diff.hunk_bodies
                .iter()
                .map(|body| &diff.patch[body.clone()]),
        )
    });
    let fetched = Fetched {
        content: diff.content,
        hunks,
    };
    Ok((fetched, merge_base))
}

// ---- replies ----

/// Sets the deadlines a rate-limited reply to `request` calls for (design
/// D8), and defers the read until the later of the two, since a detail read
/// checks both.
fn deferred(limits: &GithubLimits, request: GithubRequest, limit: RateLimit, now: u64) -> ReadEnd {
    limits.rate_limited(request, limit, now);
    ReadEnd::Deferred {
        until: limits.deadlines().held_until(),
    }
}

/// The query's reply, by the status half of the poller's verdict and its
/// GraphQL rules: the pull request, or how the read ends. A redirect, a 404
/// or any other status is transient on the query.
fn query_verdict(reply: Option<Reply>, limits: &GithubLimits, now: u64) -> Result<Value, ReadEnd> {
    let reply = reply.ok_or(ReadEnd::Transient)?;
    let body = reply.body.as_deref();
    if let Some(limit) = rate_limited_reply(reply.status, &reply.headers, body, now) {
        return Err(deferred(limits, GithubRequest::Query, limit, now));
    }
    match reply.status {
        200..=299 => read_query(body, &reply.headers, limits, now),
        // A 403 the rate-limit reading passed over: the token may not read it.
        401 | 403 => Err(ReadEnd::Unauthenticated),
        _ => Err(ReadEnd::Transient),
    }
}

/// A 2xx query reply's body. A `RATE_LIMITED` error is a rate limit, and no
/// data with an `INSUFFICIENT_SCOPES` error a credential problem, both before
/// anything else is read; otherwise `data.repository.pullRequest` is read
/// (see [`pull_request_in`]). Unlike the poller's, a body that is not JSON at
/// all is transient: "Any other reply, a transport error, a redirect on the
/// query or any other non-success status, SHALL be transient"
/// (`pull-request-viewer`: *GitHub Detail Reads*).
fn read_query(
    body: Option<&str>,
    headers: &RateHeaders,
    limits: &GithubLimits,
    now: u64,
) -> Result<Value, ReadEnd> {
    let json = body
        .and_then(|body| serde_json::from_str::<Value>(body).ok())
        .ok_or(ReadEnd::Transient)?;
    let has_error = |kind: &str| {
        json.get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| {
                errors
                    .iter()
                    .any(|error| error.get("type").and_then(Value::as_str) == Some(kind))
            })
    };
    if has_error("RATE_LIMITED") {
        let limit = RateLimit::read(headers, body, now);
        return Err(deferred(limits, GithubRequest::Query, limit, now));
    }
    let data = json.get("data").filter(|data| data.is_object());
    if data.is_none() && has_error("INSUFFICIENT_SCOPES") {
        return Err(ReadEnd::Unauthenticated);
    }
    pull_request_in(&json)
}

/// `data.repository.pullRequest` of a query reply: the pull request when it
/// is an object. "While `data` is present, a null `repository` or a null
/// `pullRequest` SHALL be unavailable": GitHub could not resolve one or the
/// other, so a retry would not either. "A null or absent `data` is GitHub's
/// answer to an execution failure such as a timeout, so it SHALL be
/// transient", as any other value on the way, or none, is.
fn pull_request_in(json: &Value) -> Result<Value, ReadEnd> {
    let Some(mut at) = json.get("data").filter(|data| !data.is_null()) else {
        return Err(ReadEnd::Transient);
    };
    for field in ["repository", "pullRequest"] {
        match at.get(field) {
            Some(Value::Null) => return Err(ReadEnd::Unavailable),
            Some(value) => at = value,
            None => return Err(ReadEnd::Transient),
        }
    }
    match at {
        Value::Object(_) => Ok(at.clone()),
        _ => Err(ReadEnd::Transient),
    }
}

/// A files page's reply: its entries, or how the read ends. Unlike the
/// query's, a redirect or a 404 is unavailable for the pull request, so a
/// moved or deleted repository is reported rather than retried. A 2xx page
/// that is not a JSON array is any other reply, and transient.
fn files_verdict(
    reply: Option<Reply>,
    limits: &GithubLimits,
    now: u64,
) -> Result<Vec<Value>, ReadEnd> {
    let reply = reply.ok_or(ReadEnd::Transient)?;
    let body = reply.body.as_deref();
    rest_status(reply.status, &reply.headers, body, limits, now)?;
    match body.and_then(|body| serde_json::from_str::<Value>(body).ok()) {
        Some(Value::Array(entries)) => Ok(entries),
        _ => Err(ReadEnd::Transient),
    }
}

/// A REST GET's status, by the rules a files page follows: a rate limit sets
/// the deadlines a files GET's would and defers the read; a redirect or a 404
/// is unavailable; a 401, or a 403 without a rate-limit signal,
/// unauthenticated; any other status but a 2xx transient.
fn rest_status(
    status: u16,
    headers: &RateHeaders,
    body: Option<&str>,
    limits: &GithubLimits,
    now: u64,
) -> Result<(), ReadEnd> {
    if let Some(limit) = rate_limited_reply(status, headers, body, now) {
        return Err(deferred(limits, GithubRequest::Files, limit, now));
    }
    match status {
        200..=299 => Ok(()),
        300..=399 | 404 => Err(ReadEnd::Unavailable),
        401 | 403 => Err(ReadEnd::Unauthenticated),
        _ => Err(ReadEnd::Transient),
    }
}

/// A compare reply: the merge base, or how the file read ends. Its status is
/// read as a files page's is, and a 2xx body must be a JSON object whose
/// `merge_base_commit.sha` is a 40-character hexadecimal commit; anything
/// else is transient.
fn compare_verdict(
    reply: Option<Reply>,
    limits: &GithubLimits,
    now: u64,
) -> Result<String, ReadEnd> {
    let reply = reply.ok_or(ReadEnd::Transient)?;
    let body = reply.body.as_deref();
    rest_status(reply.status, &reply.headers, body, limits, now)?;
    body.and_then(|body| serde_json::from_str::<Value>(body).ok())
        .as_ref()
        .and_then(|json| json.pointer("/merge_base_commit/sha"))
        .and_then(Value::as_str)
        .filter(|sha| is_commit(sha))
        .map(str::to_string)
        .ok_or(ReadEnd::Transient)
}

/// Whether `sha` names a commit as GitHub writes one: 40 hexadecimal digits.
fn is_commit(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A contents reply: the version's bytes, or how the file read ends. Its
/// status is read as a files page's is; only a 403's or 429's body is read
/// as text, for its rate-limit signal.
fn contents_verdict(
    reply: Option<RawReply>,
    limits: &GithubLimits,
    now: u64,
) -> Result<Vec<u8>, ReadEnd> {
    let reply = reply.ok_or(ReadEnd::Transient)?;
    let text = matches!(reply.status, 403 | 429)
        .then(|| reply.body.as_deref().map(String::from_utf8_lossy))
        .flatten();
    rest_status(reply.status, &reply.headers, text.as_deref(), limits, now)?;
    reply.body.ok_or(ReadEnd::Transient)
}

// ---- the pull request ----

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The login at `author`; `None` for a deleted account.
fn login(value: &Value) -> Option<String> {
    value
        .pointer("/author/login")
        .and_then(Value::as_str)
        .filter(|login| !login.is_empty())
        .map(str::to_string)
}

/// The objects of the list at `pointer`. GitHub returns a null for an entry
/// the token may not see, which is skipped.
fn nodes<'a>(value: &'a Value, pointer: &str) -> impl Iterator<Item = &'a Value> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|node| node.is_object())
}

/// A line number, when it fits one.
fn line(value: &Value, field: &str) -> Option<u32> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|line| u32::try_from(line).ok())
}

/// Everything the query read, beside the files the pages listed.
fn parts(pull_request: &Value, files: Vec<ReadFile>) -> ReadParts {
    let total = pull_request
        .pointer("/files/totalCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ReadParts {
        head_branch: text(pull_request, "/headRefName"),
        base_branch: text(pull_request, "/baseRefName"),
        head_commit: text(pull_request, "/headRefOid"),
        base_commit: text(pull_request, "/baseRefOid"),
        author: login(pull_request),
        description: text(pull_request, "/body"),
        conversation: conversation(pull_request),
        checks: checks(pull_request),
        threads: threads(pull_request),
        // Past the thousandth file, the files are counted, not listed.
        unlisted_files: saturating_u32(total.saturating_sub(files.len() as u64)),
        files,
    }
}

/// A comment or a review, with its time read from `posted`.
fn comment(node: &Value, posted: &str) -> PullRequestComment {
    let minimized = node.get("isMinimized").and_then(Value::as_bool) == Some(true);
    PullRequestComment {
        id: text(node, "/id"),
        author: login(node),
        body: text(node, "/body"),
        posted_at_unix: node
            .get(posted)
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_to_unix)
            .unwrap_or(0),
        url: node
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| url.starts_with(WEB_URL_PREFIX))
            .map(str::to_string),
        minimized_reason: minimized.then(|| text(node, "/minimizedReason")),
        deleted: false,
    }
}

/// A submitted review's state. A pending review, should one appear despite
/// the query's filter, and any state this version does not know, add nothing
/// to the conversation.
fn review_state(state: &str) -> Option<ReviewState> {
    match state {
        "APPROVED" => Some(ReviewState::Approved),
        "CHANGES_REQUESTED" => Some(ReviewState::ChangesRequested),
        "COMMENTED" => Some(ReviewState::Commented),
        "DISMISSED" => Some(ReviewState::Dismissed),
        _ => None,
    }
}

/// The pull request's comments and its submitted reviews with a body, as one
/// list in submission order. The sort is stable, and the comments go in
/// first, so entries posted in the same second keep GitHub's own order, a
/// comment before a review.
fn conversation(pull_request: &Value) -> Vec<ConversationEntry> {
    let comments = nodes(pull_request, "/comments/nodes").map(|node| ConversationEntry {
        review: None,
        comment: comment(node, "createdAt"),
    });
    let reviews = nodes(pull_request, "/reviews/nodes").filter_map(|node| {
        let review = review_state(node.get("state")?.as_str()?)?;
        let comment = comment(node, "submittedAt");
        // A review without a body, such as a bare approval, adds nothing.
        (!comment.body.trim().is_empty()).then_some(ConversationEntry {
            review: Some(review),
            comment,
        })
    });
    let mut entries: Vec<ConversationEntry> = comments.chain(reviews).collect();
    entries.sort_by_key(|entry| entry.comment.posted_at_unix);
    entries
}

/// `diffSide` or `startDiffSide`: `LEFT` is the old side, `RIGHT` the new.
fn side(value: Option<&Value>) -> Option<DiffSide> {
    match value?.as_str()? {
        "LEFT" => Some(DiffSide::Old),
        "RIGHT" => Some(DiffSide::New),
        _ => None,
    }
}

/// The review threads, each without its `PENDING` comments: the account's
/// own unsubmitted review, which nobody else can see. A thread left with no
/// comment is dropped.
fn threads(pull_request: &Value) -> Vec<ReviewThread> {
    nodes(pull_request, "/reviewThreads/nodes")
        .filter_map(|node| {
            let comments: Vec<PullRequestComment> = nodes(node, "/comments/nodes")
                .filter(|comment| comment.get("state").and_then(Value::as_str) != Some("PENDING"))
                .map(|node| comment(node, "createdAt"))
                .collect();
            if comments.is_empty() {
                return None;
            }
            let flag = |field: &str| node.get(field).and_then(Value::as_bool) == Some(true);
            Some(ReviewThread {
                id: text(node, "/id"),
                path: text(node, "/path"),
                side: side(node.get("diffSide")).unwrap_or(DiffSide::New),
                line: line(node, "line"),
                start_side: side(node.get("startDiffSide")),
                start_line: line(node, "startLine"),
                original_line: line(node, "originalLine"),
                original_start_line: line(node, "originalStartLine"),
                resolved: flag("isResolved"),
                outdated: flag("isOutdated"),
                comments,
            })
        })
        .collect()
}

/// A check run's state, from its `status` and, once it has completed, its
/// `conclusion`.
fn check_run_state(status: Option<&str>, conclusion: Option<&str>) -> PullRequestCheckState {
    match status {
        Some("COMPLETED") => match conclusion {
            Some("SUCCESS") => PullRequestCheckState::Passing,
            Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED") => {
                PullRequestCheckState::Failing
            }
            Some("NEUTRAL" | "STALE") => PullRequestCheckState::Neutral,
            Some("SKIPPED") => PullRequestCheckState::Skipped,
            Some("CANCELLED") => PullRequestCheckState::Cancelled,
            _ => PullRequestCheckState::Unknown,
        },
        Some("QUEUED" | "IN_PROGRESS" | "WAITING" | "PENDING" | "REQUESTED") => {
            PullRequestCheckState::Pending
        }
        _ => PullRequestCheckState::Unknown,
    }
}

/// A status context's state.
fn status_context_state(state: Option<&str>) -> PullRequestCheckState {
    match state {
        Some("SUCCESS") => PullRequestCheckState::Passing,
        Some("FAILURE" | "ERROR") => PullRequestCheckState::Failing,
        Some("PENDING" | "EXPECTED") => PullRequestCheckState::Pending,
        _ => PullRequestCheckState::Unknown,
    }
}

/// The head commit's check runs and status contexts, as its rollup lists
/// them. A status context is told from a check run by its `context`.
fn checks(pull_request: &Value) -> Vec<PullRequestCheck> {
    let field =
        |node: &Value, name: &str| node.get(name).and_then(Value::as_str).map(str::to_string);
    nodes(
        pull_request,
        "/commits/nodes/0/commit/statusCheckRollup/contexts/nodes",
    )
    .filter_map(|node| match field(node, "context") {
        Some(context) => Some(PullRequestCheck {
            name: context,
            state: status_context_state(node.get("state").and_then(Value::as_str)),
            url: field(node, "targetUrl"),
        }),
        None => Some(PullRequestCheck {
            name: field(node, "name")?,
            state: check_run_state(
                node.get("status").and_then(Value::as_str),
                node.get("conclusion").and_then(Value::as_str),
            ),
            url: field(node, "detailsUrl"),
        }),
    })
    .collect()
}

// ---- the files ----

/// A files entry's status, by the explicit table of design D6: `added` is
/// added, `removed` deleted, `renamed` and `copied` carry no similarity, and
/// `changed` is a type change (git's `T`). `modified`, `unchanged` (modified
/// with no textual change) and any status this version does not know are
/// modified.
fn file_status(status: &str) -> FileStatus {
    match status {
        "added" => FileStatus::Added,
        "removed" => FileStatus::Deleted,
        "renamed" => FileStatus::Renamed { similarity: None },
        "copied" => FileStatus::Copied { similarity: None },
        "changed" => FileStatus::TypeChanged,
        _ => FileStatus::Modified,
    }
}

/// One files entry as a file of the model: its paths from `filename` and
/// `previous_filename`, its counts from its own fields, no modes, and its
/// hunks from its `patch`, a per-file patch without a file header. An entry
/// without a `patch` that has changed lines is withheld, with the paths a
/// file read fetches its two versions by (`pull-request-viewer`: *GitHub
/// Detail Reads*). One without either is shown by its status alone, with no
/// hunks: GitHub does
/// not say whether it is a rename or a type change with no content change, a
/// mode-only change, or a binary or empty file, so it is never called too
/// large or binary. Its `patch`'s digest and its blob `sha` stay beside it,
/// for the byte limits and the review keys; the text itself goes once its
/// hunks are parsed.
fn read_file(entry: &Value) -> Option<ReadFile> {
    let filename = entry.get("filename")?.as_str()?.to_string();
    let status = file_status(
        entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let count = |field: &str| entry.get(field).and_then(Value::as_u64).map(saturating_u32);
    let (additions, deletions) = (count("additions"), count("deletions"));
    let patch = entry.get("patch").and_then(Value::as_str);
    let mut digests = None;
    let content = match patch {
        Some(patch) => {
            let (hunks, bodies): (Vec<_>, Vec<_>) = parse_hunks_with_bodies(patch.as_bytes())
                .into_iter()
                .unzip();
            digests = Some(hunk_digests(
                bodies.into_iter().map(|body| &patch.as_bytes()[body]),
            ));
            DiffContent::Hunks { hunks }
        }
        None if additions.unwrap_or(0) > 0 || deletions.unwrap_or(0) > 0 => DiffContent::Withheld,
        None => DiffContent::Hunks { hunks: Vec::new() },
    };
    let fetch_versions = content == DiffContent::Withheld;
    let previous = entry
        .get("previous_filename")
        .and_then(Value::as_str)
        .map(str::to_string);
    let (old_path, new_path) = match status {
        FileStatus::Added => (None, Some(filename)),
        FileStatus::Deleted => (Some(filename), None),
        FileStatus::Renamed { .. } | FileStatus::Copied { .. } => (
            Some(previous.unwrap_or_else(|| filename.clone())),
            Some(filename),
        ),
        _ => (Some(filename.clone()), Some(filename)),
    };
    let fetch = fetch_versions.then(|| FetchPaths {
        old: old_path.clone(),
        new: new_path.clone(),
    });
    Some(ReadFile {
        file: DiffFile {
            old_path,
            new_path,
            old_mode: None,
            new_mode: None,
            status,
            additions,
            deletions,
            content,
        },
        patch: patch.map(|patch| PatchDigest::of([patch.as_bytes()])),
        hunks: digests,
        blob_sha: entry
            .get("sha")
            .and_then(Value::as_str)
            .filter(|sha| !sha.is_empty())
            .map(str::to_string),
        fetch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GithubDeadlines;
    use openspec_core::{diff_versions, parse_hunks};
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;

    const NOW: u64 = 1_800_000_000;

    // ------------------------------------------------------------ fixtures

    fn ok(body: Value) -> Option<Reply> {
        reply(200, RateHeaders::default(), Some(body.to_string()))
    }

    fn reply(status: u16, headers: RateHeaders, body: Option<String>) -> Option<Reply> {
        Some(Reply {
            status,
            headers,
            body,
        })
    }

    fn status(code: u16) -> Option<Reply> {
        reply(code, RateHeaders::default(), None)
    }

    /// A pull request as the query returns it, with `extra` laid over a
    /// minimal one.
    fn pull_request(extra: Value) -> Value {
        let mut pull_request = json!({
            "body": "Adds rate limits.",
            "author": { "login": "ada" },
            "baseRefName": "main",
            "headRefName": "feature/limits",
            "baseRefOid": "b".repeat(40),
            "headRefOid": "a".repeat(40),
            "reviews": { "nodes": [] },
            "commits": { "nodes": [] },
            "comments": { "nodes": [] },
            "reviewThreads": { "nodes": [] },
            "files": { "totalCount": 0 },
        });
        for (key, value) in extra.as_object().unwrap() {
            pull_request[key] = value.clone();
        }
        pull_request
    }

    fn query_reply(pull_request: Value) -> Option<Reply> {
        ok(json!({ "data": { "repository": { "pullRequest": pull_request } } }))
    }

    fn entry(filename: &str, status: &str, patch: Option<&str>, added: u64, removed: u64) -> Value {
        let mut entry = json!({
            "sha": format!("sha-{filename}"),
            "filename": filename,
            "status": status,
            "additions": added,
            "deletions": removed,
            "changes": added + removed,
        });
        if let Some(patch) = patch {
            entry["patch"] = json!(patch);
        }
        entry
    }

    /// `count` files, as `per_page` pages of the files endpoint serve them.
    fn page_of(count: usize, page: usize) -> Option<Reply> {
        let first = (page - 1) * FILES_PER_PAGE;
        let entries: Vec<Value> = (first..count.min(first + FILES_PER_PAGE))
            .map(|n| {
                entry(
                    &format!("src/file{n}.rs"),
                    "modified",
                    Some("@@ -1 +1 @@\n-a\n+b"),
                    1,
                    1,
                )
            })
            .collect();
        ok(Value::Array(entries))
    }

    /// The page number a files URL asks for.
    fn page_number(url: &str) -> usize {
        url.rsplit("&page=").next().unwrap().parse().unwrap()
    }

    /// What one read sent and how it ended.
    struct Read {
        outcome: Result<ReadParts, ReadEnd>,
        posts: Vec<(String, String)>,
        gets: Vec<String>,
    }

    /// A GitHub pull request, spelt as its row spells it.
    fn pr(owner: &str, repo: &str, number: u64) -> PullRequestReference {
        PullRequestReference {
            provider: crate::events::PullRequestProvider::Github,
            owner: owner.to_string(),
            repo: repo.to_string(),
            number,
        }
    }

    /// One read of `acme/api#42`, answering the query with `query` and each
    /// files page with `page`, with nothing in the read's way.
    fn read(query: Option<Reply>, page: impl Fn(usize) -> Option<Reply>) -> Read {
        read_as(&pr("acme", "api", 42), &GithubLimits::new(), query, page)
    }

    fn read_as(
        pull_request: &PullRequestReference,
        limits: &GithubLimits,
        query: Option<Reply>,
        page: impl Fn(usize) -> Option<Reply>,
    ) -> Read {
        let (posts, gets) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
        let outcome = read_with(
            pull_request,
            |url, body| {
                posts.borrow_mut().push((url.to_string(), body));
                query.clone()
            },
            |url| {
                gets.borrow_mut().push(url.to_string());
                page(page_number(url))
            },
            || Ok(()),
            limits,
            || NOW,
        );
        Read {
            outcome,
            posts: posts.into_inner(),
            gets: gets.into_inner(),
        }
    }

    fn parts_of(read: Read) -> ReadParts {
        read.outcome.expect("the read succeeds")
    }

    fn files(count: usize, total: u64) -> Read {
        read(
            query_reply(pull_request(json!({ "files": { "totalCount": total } }))),
            move |page| page_of(count, page),
        )
    }

    // ------------------------------------------------------------ the query

    #[test]
    fn the_detail_query_is_a_read_only_constant() {
        let lowered = DETAIL_QUERY.to_ascii_lowercase();
        assert!(!lowered.contains("mutation"), "no mutation");
        assert!(!lowered.contains("subscription"), "no subscription");
        assert!(DETAIL_QUERY.starts_with("query SpecForgePullRequestDetail("));
        for field in [
            "repository(owner: $owner, name: $name)",
            "pullRequest(number: $number)",
            "states: [APPROVED, CHANGES_REQUESTED, COMMENTED, DISMISSED]",
            "reviewThreads(first: 100)",
            "id isResolved isOutdated path line originalLine startLine originalStartLine diffSide startDiffSide",
            "url state isMinimized minimizedReason",
            "isMinimized minimizedReason",
            "... on CheckRun { name status conclusion detailsUrl }",
            "... on StatusContext { context state targetUrl }",
            "files(first: 100) { totalCount }",
            "baseRefName headRefName baseRefOid headRefOid",
        ] {
            assert!(DETAIL_QUERY.contains(field), "{field}");
        }
    }

    /// `github-pull-requests`: *The detail query cannot write and does not
    /// vary*. Two pull requests' queries are the same text, and differ only
    /// in their variables, which carry each row's own spelling.
    #[test]
    fn two_pull_requests_send_byte_identical_query_text() {
        let limits = GithubLimits::new();
        let first = read_as(
            &pr("acme", "api", 42),
            &limits,
            query_reply(pull_request(json!({}))),
            |page| page_of(0, page),
        );
        let second = read_as(
            &pr("Acme", "Web", 7),
            &limits,
            query_reply(pull_request(json!({}))),
            |page| page_of(0, page),
        );
        let body = |read: &Read| -> Value { serde_json::from_str(&read.posts[0].1).unwrap() };
        let (first, second) = (body(&first), body(&second));
        assert_eq!(first["query"], DETAIL_QUERY);
        assert_eq!(first["query"], second["query"]);
        assert_eq!(
            first["variables"],
            json!({ "owner": "acme", "name": "api", "number": 42 })
        );
        assert_eq!(
            second["variables"],
            json!({ "owner": "Acme", "name": "Web", "number": 7 })
        );
        assert_eq!(
            first.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["query", "variables"]
        );
    }

    /// Every request goes to the GraphQL endpoint or a files page of the
    /// matched pull request, with each path segment encoded.
    #[test]
    fn every_request_goes_to_the_graphql_endpoint_or_a_files_page() {
        let read = files(120, 120);
        assert_eq!(read.posts.len(), 1);
        assert_eq!(read.posts[0].0, "https://api.github.com/graphql");
        assert_eq!(
            read.gets,
            (1..=3)
                .map(|page| format!(
                    "https://api.github.com/repos/acme/api/pulls/42/files?per_page=50&page={page}"
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            files_url("my org", "a/b", 7, 1),
            "https://api.github.com/repos/my%20org/a%2Fb/pulls/7/files?per_page=50&page=1"
        );
    }

    // ------------------------------------------------------------ the pages

    /// `pull-request-viewer`: *File pages stop at a thousand files*.
    #[test]
    fn twelve_hundred_files_request_twenty_pages_and_no_twenty_first() {
        let read = files(1_200, 1_200);
        assert_eq!(read.gets.len(), 20);
        assert_eq!(DETAIL_MAX_FILES_PAGES, 20);
        let parts = parts_of(read);
        assert_eq!(parts.files.len(), 1_000);
        assert_eq!(parts.unlisted_files, 200);
    }

    /// A full page asks for the next; a short one is the last.
    #[test]
    fn a_full_page_asks_for_the_next_and_a_short_one_ends_the_list() {
        let read = files(100, 100);
        assert_eq!(read.gets.len(), 3, "two full pages, then an empty one");
        assert_eq!(parts_of(read).unlisted_files, 0);
        let read = files(99, 99);
        assert_eq!(read.gets.len(), 2);
        assert_eq!(parts_of(read).files.len(), 99);
        let read = files(1_000, 1_001);
        assert_eq!(read.gets.len(), 20);
        assert_eq!(parts_of(read).unlisted_files, 1);
    }

    // ------------------------------------------------------------ the files

    fn one_file(entry: Value) -> ReadFile {
        let page = Value::Array(vec![entry]);
        let mut parts = parts_of(read(query_reply(pull_request(json!({}))), move |_| {
            ok(page.clone())
        }));
        parts.files.remove(0)
    }

    /// `pull-request-viewer`: *Statuses map explicitly*, with every status of
    /// the table.
    #[test]
    fn every_status_maps_as_the_table_says() {
        for (status, expected) in [
            ("added", FileStatus::Added),
            ("removed", FileStatus::Deleted),
            ("modified", FileStatus::Modified),
            ("renamed", FileStatus::Renamed { similarity: None }),
            ("copied", FileStatus::Copied { similarity: None }),
            ("changed", FileStatus::TypeChanged),
            ("unchanged", FileStatus::Modified),
            ("something-new", FileStatus::Modified),
        ] {
            assert_eq!(file_status(status), expected, "{status}");
        }
        let unchanged = one_file(entry("README.md", "unchanged", None, 0, 0)).file;
        assert_eq!(unchanged.status, FileStatus::Modified);
        assert_eq!(unchanged.content, DiffContent::Hunks { hunks: Vec::new() });
        assert_eq!(
            one_file(entry("link", "changed", None, 0, 0)).file.status,
            FileStatus::TypeChanged
        );
    }

    /// `pull-request-viewer`: *A patchless file is never mislabelled*.
    #[test]
    fn a_patchless_entry_without_lines_is_neither_too_large_nor_binary() {
        let file = one_file(entry("logo.png", "modified", None, 0, 0)).file;
        assert_eq!(file.content, DiffContent::Hunks { hunks: Vec::new() });
        assert_eq!((file.additions, file.deletions), (Some(0), Some(0)));
    }

    /// `pull-request-viewer`: *A patchless file with lines is too large* only
    /// once read: by its added lines or its removed ones, it arrives withheld
    /// with its counts, and with the paths a file read fetches — none on the
    /// side it does not have, and a rename's old path on its old side.
    #[test]
    fn a_patchless_entry_with_lines_is_withheld_with_the_paths_to_fetch() {
        let read = one_file(entry("data.json", "added", None, 4_000, 0));
        assert_eq!(read.file.content, DiffContent::Withheld);
        assert_eq!(
            (read.file.additions, read.file.deletions),
            (Some(4_000), Some(0))
        );
        let paths = |old: Option<&str>, new: Option<&str>| {
            Some(FetchPaths {
                old: old.map(str::to_string),
                new: new.map(str::to_string),
            })
        };
        assert_eq!(read.fetch, paths(None, Some("data.json")));
        let read = one_file(entry("data.json", "removed", None, 0, 1));
        assert_eq!(read.file.content, DiffContent::Withheld);
        assert_eq!(read.fetch, paths(Some("data.json"), None));
        let mut renamed = entry("b.json", "renamed", None, 3, 3);
        renamed["previous_filename"] = json!("a.json");
        assert_eq!(
            one_file(renamed).fetch,
            paths(Some("a.json"), Some("b.json"))
        );
        // A patchless entry without lines is shown by its status alone, and
        // nothing is fetched for it.
        let read = one_file(entry("mode.sh", "modified", None, 0, 0));
        assert_eq!(read.file.content, DiffContent::Hunks { hunks: Vec::new() });
        assert_eq!(read.fetch, None);
    }

    /// A version is read to one byte past the per-file ceiling, no more and no
    /// less: one byte fewer would read a version just past the ceiling as one
    /// within it, cut short, and `diff_versions` would diff a truncated file.
    #[test]
    fn a_version_is_read_to_one_byte_past_the_per_file_ceiling() {
        assert_eq!(VERSION_READ_LIMIT, REQUESTED_FILE_BYTES_LIMIT + 1);
        let past = vec![b'x'; VERSION_READ_LIMIT];
        assert_eq!(diff_versions(None, Some(&past)), DiffContent::TooLarge);
    }

    /// The file read's two URLs: each path segment encoded on its own, so a
    /// `+` and a space reach GitHub as themselves, and the commits as given.
    #[test]
    fn the_file_read_urls_encode_each_path_segment() {
        assert_eq!(
            contents_url("my org", "a/b", "apps/uk/+Page.tsx", "abc"),
            "https://api.github.com/repos/my%20org/a%2Fb/contents/apps/uk/%2BPage.tsx?ref=abc"
        );
        assert_eq!(
            compare_url("acme", "api", "base1", "head2"),
            "https://api.github.com/repos/acme/api/compare/base1...head2?per_page=1"
        );
    }

    fn body_reply(status: u16, body: &str) -> Option<Reply> {
        Some(Reply {
            status,
            headers: RateHeaders::default(),
            body: Some(body.to_string()),
        })
    }

    fn raw(status: u16, body: &[u8]) -> Option<RawReply> {
        Some(RawReply {
            status,
            headers: RateHeaders::default(),
            body: Some(body.to_vec()),
        })
    }

    /// A compare answers its merge base only as a 40-digit hexadecimal commit;
    /// a missing, short or foreign one is transient, and its status is read
    /// as a files page's is.
    #[test]
    fn a_compare_reply_answers_a_commit_or_how_the_read_ends() {
        let limits = GithubLimits::new();
        let sha = "0123456789abcdef0123456789ABCDEF01234567";
        let body = json!({ "merge_base_commit": { "sha": sha } }).to_string();
        assert_eq!(
            compare_verdict(body_reply(200, &body), &limits, NOW),
            Ok(sha.to_string())
        );
        for bad in [
            json!({}),
            json!({ "merge_base_commit": { "sha": "0123456789abcdef" } }),
            json!({ "merge_base_commit": { "sha": "g123456789abcdef0123456789abcdef01234567" } }),
            json!({ "merge_base_commit": { "sha": format!("{sha}0") } }),
        ] {
            assert_eq!(
                compare_verdict(body_reply(200, &bad.to_string()), &limits, NOW),
                Err(ReadEnd::Transient),
                "{bad}"
            );
        }
        assert_eq!(
            compare_verdict(body_reply(404, ""), &limits, NOW),
            Err(ReadEnd::Unavailable)
        );
        assert_eq!(
            compare_verdict(body_reply(301, ""), &limits, NOW),
            Err(ReadEnd::Unavailable)
        );
        assert_eq!(
            compare_verdict(body_reply(401, ""), &limits, NOW),
            Err(ReadEnd::Unauthenticated)
        );
        assert_eq!(
            compare_verdict(body_reply(500, ""), &limits, NOW),
            Err(ReadEnd::Transient)
        );
        assert_eq!(compare_verdict(None, &limits, NOW), Err(ReadEnd::Transient));
    }

    /// A contents reply answers its bytes as they came; a 429 defers the read
    /// and sets the REST deadline, a 404 is unavailable and a 403 without a
    /// rate-limit signal a credential problem.
    #[test]
    fn a_contents_reply_answers_its_bytes_or_how_the_read_ends() {
        let limits = GithubLimits::new();
        assert_eq!(
            contents_verdict(raw(200, b"\xff\0bytes"), &limits, NOW),
            Ok(b"\xff\0bytes".to_vec())
        );
        assert_eq!(
            contents_verdict(raw(404, b""), &limits, NOW),
            Err(ReadEnd::Unavailable)
        );
        assert_eq!(
            contents_verdict(raw(403, b""), &limits, NOW),
            Err(ReadEnd::Unauthenticated)
        );
        assert_eq!(
            contents_verdict(raw(502, b""), &limits, NOW),
            Err(ReadEnd::Transient)
        );
        assert_eq!(
            contents_verdict(None, &limits, NOW),
            Err(ReadEnd::Transient)
        );
        let Err(ReadEnd::Deferred { until }) = contents_verdict(raw(429, b""), &limits, NOW) else {
            panic!("a 429 defers the read");
        };
        assert!(until > NOW);
        assert_eq!(limits.deadlines().rest, until);
        assert_eq!(
            limits.deadlines().graphql,
            0,
            "a REST limit holds only REST"
        );
    }

    /// Each entry's paths, counts, hunks and modes, and the patch and blob
    /// `sha` the review keys need, kept beside it.
    #[test]
    fn an_entry_maps_its_paths_counts_and_hunks_and_keeps_its_patch_and_sha() {
        let patch = "@@ -1,2 +1,2 @@ fn main()\n-let a = 1;\n+let a = 2;\n let b = 3;";
        let mut renamed = entry("src/new.rs", "renamed", Some(patch), 1, 1);
        renamed["previous_filename"] = json!("src/old.rs");
        let read = one_file(renamed);
        assert_eq!(read.file.old_path.as_deref(), Some("src/old.rs"));
        assert_eq!(read.file.new_path.as_deref(), Some("src/new.rs"));
        assert_eq!((read.file.old_mode, read.file.new_mode), (None, None));
        assert_eq!(
            (read.file.additions, read.file.deletions),
            (Some(1), Some(1))
        );
        assert_eq!(
            read.file.content,
            DiffContent::Hunks {
                hunks: parse_hunks(patch.as_bytes())
            }
        );
        assert_eq!(read.patch, Some(PatchDigest::of([patch.as_bytes()])));
        // Its one hunk is keyed by its body: the bytes past its header line.
        let body: &[u8] = b"-let a = 1;\n+let a = 2;\n let b = 3;";
        assert_eq!(read.hunks, Some(vec![Sha256::digest(body).into()]));
        assert_eq!(read.blob_sha.as_deref(), Some("sha-src/new.rs"));

        let added = one_file(entry("a.rs", "added", Some("@@ -0,0 +1 @@\n+a"), 1, 0)).file;
        assert_eq!(
            (added.old_path, added.new_path.as_deref()),
            (None, Some("a.rs"))
        );
        let removed = one_file(entry("b.rs", "removed", Some("@@ -1 +0,0 @@\n-b"), 0, 1)).file;
        assert_eq!(
            (removed.old_path.as_deref(), removed.new_path),
            (Some("b.rs"), None)
        );
        let modified = one_file(entry("c.rs", "modified", None, 0, 0));
        // Without patch text there is nothing to key a hunk by.
        assert_eq!(modified.hunks, None);
        let modified = modified.file;
        assert_eq!(modified.old_path.as_deref(), Some("c.rs"));
        assert_eq!(modified.new_path.as_deref(), Some("c.rs"));
        // A copy without its source named keeps its own name on both sides.
        let copied = one_file(entry("d.rs", "copied", None, 0, 0)).file;
        assert_eq!(copied.old_path.as_deref(), Some("d.rs"));
        // An empty `sha` is no `sha`; an entry without a name is no file.
        let mut no_sha = entry("e.rs", "modified", None, 0, 0);
        no_sha["sha"] = json!("");
        assert_eq!(one_file(no_sha).blob_sha, None);
        assert_eq!(read_file(&json!({ "status": "added" })), None);
    }

    // ------------------------------------------------------------ the replies

    /// `pull-request-viewer`: *A moved or deleted repository is reported,
    /// not retried*.
    #[test]
    fn a_files_404_or_redirect_is_unavailable() {
        for code in [301, 302, 307, 308, 404] {
            let read = read(query_reply(pull_request(json!({}))), |_| status(code));
            assert_eq!(read.outcome, Err(ReadEnd::Unavailable), "status {code}");
            assert_eq!(read.gets.len(), 1, "nothing follows it");
        }
    }

    /// "A files page SHALL be read as a JSON array", and a 2xx page that is
    /// not one is any other reply: transient, unlike the poller's verdict on
    /// a body it cannot read.
    #[test]
    fn a_files_page_that_is_not_a_list_is_transient() {
        for body in [
            Some("{}".to_string()),
            Some("not json".to_string()),
            Some("null".to_string()),
            None,
        ] {
            let read = read(query_reply(pull_request(json!({}))), |_| {
                reply(200, RateHeaders::default(), body.clone())
            });
            assert_eq!(read.outcome, Err(ReadEnd::Transient), "{body:?}");
            assert_eq!(read.gets.len(), 1, "nothing more is asked");
        }
    }

    #[test]
    fn a_files_401_or_plain_403_is_a_credential_problem() {
        for code in [401, 403] {
            let read = read(query_reply(pull_request(json!({}))), |_| status(code));
            assert_eq!(read.outcome, Err(ReadEnd::Unauthenticated), "status {code}");
        }
    }

    #[test]
    fn a_files_transport_error_or_server_error_is_transient() {
        let offline = read(query_reply(pull_request(json!({}))), |_| None);
        assert_eq!(offline.outcome, Err(ReadEnd::Transient));
        let failing = read(query_reply(pull_request(json!({}))), |_| status(502));
        assert_eq!(failing.outcome, Err(ReadEnd::Transient));
    }

    /// `pull-request-viewer`: *A missing pull request is unavailable*: while
    /// `data` is present, a null `pullRequest`, or a null `repository` GitHub
    /// could not resolve, is unavailable.
    #[test]
    fn a_null_pull_request_or_repository_is_unavailable() {
        for body in [
            json!({ "data": { "repository": { "pullRequest": null } } }),
            json!({ "data": { "repository": { "pullRequest": null } }, "errors": [{ "type": "NOT_FOUND" }] }),
            json!({ "data": { "repository": null }, "errors": [{ "type": "NOT_FOUND" }] }),
        ] {
            let read = read(ok(body.clone()), |_| None);
            assert_eq!(read.outcome, Err(ReadEnd::Unavailable), "{body}");
            assert!(read.gets.is_empty(), "no files are asked for");
        }
    }

    /// "A null or absent `data` is GitHub's answer to an execution failure
    /// such as a timeout, so it SHALL be transient." The rules that come
    /// first still do: `RATE_LIMITED` defers, and `INSUFFICIENT_SCOPES` with
    /// no data is a credential problem.
    #[test]
    fn a_null_or_absent_data_is_transient() {
        let timeout = json!([{ "message": "Something went wrong while executing your query. This may be the result of a timeout, or it could be a GitHub bug." }]);
        for body in [
            json!({ "data": null }),
            json!({ "data": null, "errors": timeout }),
            json!({ "errors": timeout }),
            json!({ "errors": [{ "type": "NOT_FOUND" }] }),
        ] {
            let read = read(ok(body.clone()), |_| None);
            assert_eq!(read.outcome, Err(ReadEnd::Transient), "{body}");
            assert!(read.gets.is_empty(), "no files are asked for");
        }

        let scopes = json!({ "data": null, "errors": [{ "type": "INSUFFICIENT_SCOPES" }] });
        assert_eq!(
            read(ok(scopes), |_| None).outcome,
            Err(ReadEnd::Unauthenticated)
        );
        let limits = GithubLimits::new();
        let limited = json!({ "data": null, "errors": [{ "type": "RATE_LIMITED" }] });
        let deferred = read_as(&pr("acme", "api", 42), &limits, ok(limited), |_| None);
        assert!(
            matches!(deferred.outcome, Err(ReadEnd::Deferred { .. })),
            "{:?}",
            deferred.outcome
        );
    }

    /// "Any other reply … SHALL be transient": a 2xx body that is not JSON,
    /// none at all, or JSON holding neither a pull request nor a null for
    /// one. Unlike the poller's verdict, none of these reads as unavailable.
    #[test]
    fn a_query_body_holding_no_pull_request_and_no_null_is_transient() {
        for body in [
            Some("<html>".to_string()),
            None,
            Some("[]".to_string()),
            Some(json!({}).to_string()),
            Some(json!({ "errors": [{ "type": "SOMETHING_NEW" }] }).to_string()),
            Some(json!({ "data": {} }).to_string()),
            Some(json!({ "data": { "repository": {} } }).to_string()),
            Some(json!({ "data": { "repository": { "pullRequest": 42 } } }).to_string()),
            Some(json!({ "data": [] }).to_string()),
        ] {
            let read = read(reply(200, RateHeaders::default(), body.clone()), |_| None);
            assert_eq!(read.outcome, Err(ReadEnd::Transient), "{body:?}");
            assert!(read.gets.is_empty(), "no files are asked for");
        }
    }

    /// `pull-request-viewer`: *A missing scope is a credential problem*. With
    /// data beside it, the data is read.
    #[test]
    fn insufficient_scopes_with_no_data_is_unauthenticated() {
        let scopes = json!([{ "type": "INSUFFICIENT_SCOPES", "message": "needs read:org" }]);
        let read_body = |body: Value| read(ok(body), |page| page_of(0, page)).outcome;
        assert_eq!(
            read_body(json!({ "data": null, "errors": scopes })),
            Err(ReadEnd::Unauthenticated)
        );
        assert_eq!(
            read_body(json!({ "errors": scopes })),
            Err(ReadEnd::Unauthenticated)
        );
        let with_data = json!({
            "data": { "repository": { "pullRequest": pull_request(json!({})) } },
            "errors": scopes,
        });
        assert!(read_body(with_data).is_ok());
    }

    #[test]
    fn a_query_401_or_plain_403_is_a_credential_problem() {
        for code in [401, 403] {
            assert_eq!(
                read(status(code), |_| None).outcome,
                Err(ReadEnd::Unauthenticated)
            );
        }
    }

    /// Unlike a files page's, a redirect or a 404 on the query is transient,
    /// as is a transport error or any other status.
    #[test]
    fn a_redirect_or_404_on_the_query_is_transient() {
        for code in [302, 404, 500, 502] {
            assert_eq!(
                read(status(code), |_| None).outcome,
                Err(ReadEnd::Transient),
                "status {code}"
            );
        }
        assert_eq!(read(None, |_| None).outcome, Err(ReadEnd::Transient));
    }

    /// A rate-limited reply sets the deadlines of design D8 and defers the
    /// read until the later of the two.
    #[test]
    fn a_rate_limited_reply_sets_its_deadlines_and_defers_the_read() {
        let deadlines = |graphql, rest| GithubDeadlines { graphql, rest };
        let retry = |secs: &str| RateHeaders::from_raw(Some(secs), None, None);
        let acme = pr("acme", "api", 42);

        let limits = GithubLimits::new();
        let read = read_as(&acme, &limits, reply(429, retry("600"), None), |_| None);
        assert_eq!(read.outcome, Err(ReadEnd::Deferred { until: NOW + 600 }));
        assert_eq!(limits.deadlines(), deadlines(NOW + 600, 0));

        // GraphQL's `RATE_LIMITED`, in a 200, by the headers beside it.
        let limits = GithubLimits::new();
        let spent = RateHeaders::from_raw(None, Some("0"), Some(&(NOW + 900).to_string()));
        let body = json!({ "errors": [{ "type": "RATE_LIMITED" }] }).to_string();
        let read = read_as(&acme, &limits, reply(200, spent, Some(body)), |_| None);
        assert_eq!(read.outcome, Err(ReadEnd::Deferred { until: NOW + 900 }));
        assert_eq!(limits.deadlines(), deadlines(NOW + 900, 0));

        // A files page's spent REST quota sets the REST deadline alone.
        let limits = GithubLimits::new();
        let read = read_as(&acme, &limits, query_reply(pull_request(json!({}))), |_| {
            reply(403, spent, Some("{}".to_string()))
        });
        assert_eq!(read.outcome, Err(ReadEnd::Deferred { until: NOW + 900 }));
        assert_eq!(limits.deadlines(), deadlines(0, NOW + 900));

        // A secondary limit on a files page sets both, and the read waits on
        // the later one.
        let limits = GithubLimits::new();
        let earlier = RateLimit {
            delay: Some(1_200),
            secondary: false,
        };
        limits.rate_limited(GithubRequest::Query, earlier, NOW);
        let secondary = r#"{"message":"You have exceeded a secondary rate limit."}"#.to_string();
        let read = read_as(&acme, &limits, query_reply(pull_request(json!({}))), |_| {
            reply(403, RateHeaders::default(), Some(secondary.clone()))
        });
        assert_eq!(read.outcome, Err(ReadEnd::Deferred { until: NOW + 1_200 }));
        assert_eq!(limits.deadlines(), deadlines(NOW + 1_200, NOW + 300));
    }

    // ------------------------------------------------------------ the conversation

    fn review(id: &str, state: &str, body: &str, at: &str) -> Value {
        json!({
            "id": id, "author": { "login": "grace" }, "state": state, "body": body,
            "submittedAt": at, "url": format!("https://github.com/acme/api/pull/42#pullrequestreview-{id}"),
            "isMinimized": false, "minimizedReason": null,
        })
    }

    fn issue_comment(id: &str, body: &str, at: &str) -> Value {
        json!({
            "id": id, "author": { "login": "ada" }, "body": body, "createdAt": at,
            "url": format!("https://github.com/acme/api/pull/42#issuecomment-{id}"),
            "isMinimized": false, "minimizedReason": null,
        })
    }

    fn conversation_of(comments: Vec<Value>, reviews: Vec<Value>) -> Vec<ConversationEntry> {
        conversation(&pull_request(json!({
            "comments": { "nodes": comments },
            "reviews": { "nodes": reviews },
        })))
    }

    fn ids(entries: &[ConversationEntry]) -> Vec<&str> {
        entries
            .iter()
            .map(|entry| entry.comment.id.as_str())
            .collect()
    }

    /// `pull-request-viewer`: *The conversation follows submission order*.
    #[test]
    fn the_conversation_follows_submission_order() {
        let entries = conversation_of(
            vec![
                issue_comment("c2", "Done.", "2026-09-01T10:10:00Z"),
                issue_comment("c1", "First pass.", "2026-09-01T10:00:00Z"),
            ],
            vec![review(
                "r1",
                "COMMENTED",
                "Looks close",
                "2026-09-01T10:05:00Z",
            )],
        );
        assert_eq!(ids(&entries), ["c1", "r1", "c2"]);
        assert_eq!(entries[1].review, Some(ReviewState::Commented));
        assert_eq!(entries[0].review, None);
        assert_eq!(
            entries[0].comment,
            PullRequestComment {
                id: "c1".to_string(),
                author: Some("ada".to_string()),
                body: "First pass.".to_string(),
                posted_at_unix: 1_788_256_800,
                url: Some("https://github.com/acme/api/pull/42#issuecomment-c1".to_string()),
                minimized_reason: None,
                deleted: false,
            }
        );
    }

    /// A comment and a review posted in the same second keep the documented
    /// tie-break: GitHub's own order, the comment first. The mutation gate
    /// cannot see a sort's stability, so this pins it.
    #[test]
    fn a_comment_and_a_review_in_the_same_second_keep_the_comment_first() {
        let at = "2026-09-01T10:00:00Z";
        let entries = conversation_of(
            vec![
                issue_comment("c1", "One", at),
                issue_comment("c2", "Two", at),
            ],
            vec![
                review("r1", "APPROVED", "Ship it", at),
                review("r2", "COMMENTED", "Nit", at),
            ],
        );
        assert_eq!(ids(&entries), ["c1", "c2", "r1", "r2"]);
    }

    /// `pull-request-viewer`: *A review without a body adds nothing to the
    /// conversation*; nor does a pending review, nor a state this version
    /// does not know.
    #[test]
    fn a_body_less_or_pending_review_adds_nothing() {
        let at = "2026-09-01T10:00:00Z";
        let entries = conversation_of(
            Vec::new(),
            vec![
                review("bare", "APPROVED", "", at),
                review("blank", "APPROVED", "  \n", at),
                review("pending", "PENDING", "Draft thoughts", at),
                review("new", "SOMETHING_NEW", "Body", at),
                review("kept", "CHANGES_REQUESTED", "Fix the tests", at),
            ],
        );
        assert_eq!(ids(&entries), ["kept"]);
    }

    #[test]
    fn every_review_state_maps_to_its_own() {
        for (state, expected) in [
            ("APPROVED", Some(ReviewState::Approved)),
            ("CHANGES_REQUESTED", Some(ReviewState::ChangesRequested)),
            ("COMMENTED", Some(ReviewState::Commented)),
            ("DISMISSED", Some(ReviewState::Dismissed)),
            ("PENDING", None),
        ] {
            assert_eq!(review_state(state), expected, "{state}");
        }
    }

    /// `pull-request-viewer`: *A minimised comment stays collapsed*: the
    /// reason as GitHub states it, empty when it states none.
    #[test]
    fn a_minimised_comment_carries_its_reason() {
        let mut node = issue_comment("c1", "spam", "2026-09-01T10:00:00Z");
        assert_eq!(comment(&node, "createdAt").minimized_reason, None);
        node["isMinimized"] = json!(true);
        node["minimizedReason"] = json!("outdated");
        assert_eq!(
            comment(&node, "createdAt").minimized_reason.as_deref(),
            Some("outdated")
        );
        node["minimizedReason"] = Value::Null;
        assert_eq!(
            comment(&node, "createdAt").minimized_reason.as_deref(),
            Some("")
        );
    }

    /// Only a link on GitHub's own site survives; a deleted account and an
    /// unreadable time read as absent.
    #[test]
    fn a_comment_keeps_only_a_github_url_and_tolerates_absence() {
        let mut node = issue_comment("c1", "Hi", "not a time");
        node["url"] = json!("https://evil.example/x");
        node["author"] = Value::Null;
        let read = comment(&node, "createdAt");
        assert_eq!(read.url, None);
        assert_eq!(read.author, None);
        assert_eq!(read.posted_at_unix, 0);
    }

    // ------------------------------------------------------------ threads

    fn thread_comment(id: &str, state: &str) -> Value {
        json!({
            "id": id, "author": { "login": "grace" }, "body": format!("{state} comment"),
            "createdAt": "2026-09-01T10:00:00Z", "url": "https://github.com/acme/api/pull/42#discussion_r1",
            "state": state, "isMinimized": false, "minimizedReason": null,
        })
    }

    fn thread(id: &str, comments: Vec<Value>) -> Value {
        json!({
            "id": id, "isResolved": false, "isOutdated": false, "path": "src/api.ts",
            "line": 12, "originalLine": 10, "startLine": null, "originalStartLine": null,
            "diffSide": "LEFT", "startDiffSide": null,
            "comments": { "nodes": comments },
        })
    }

    fn threads_of(nodes: Vec<Value>) -> Vec<ReviewThread> {
        threads(&pull_request(
            json!({ "reviewThreads": { "nodes": nodes } }),
        ))
    }

    /// `pull-request-viewer`: *The account's pending review is never shown*.
    #[test]
    fn a_pending_thread_comment_is_dropped_and_a_thread_left_empty_with_it() {
        let threads = threads_of(vec![
            thread("pending-only", vec![thread_comment("p1", "PENDING")]),
            thread(
                "mixed",
                vec![
                    thread_comment("p2", "PENDING"),
                    thread_comment("s1", "SUBMITTED"),
                ],
            ),
            thread("empty", Vec::new()),
        ]);
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, "mixed");
        let kept: Vec<&str> = threads[0].comments.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(kept, ["s1"]);
    }

    /// `pull-request-viewer`: *A thread names its file, side and line*.
    #[test]
    fn a_thread_names_its_file_side_and_lines() {
        let mut ranged = thread("t2", vec![thread_comment("s2", "SUBMITTED")]);
        ranged["diffSide"] = json!("RIGHT");
        ranged["startDiffSide"] = json!("LEFT");
        ranged["startLine"] = json!(8);
        ranged["originalStartLine"] = json!(6);
        ranged["isResolved"] = json!(true);
        ranged["isOutdated"] = json!(true);
        let mut sideless = thread("t3", vec![thread_comment("s3", "SUBMITTED")]);
        sideless["diffSide"] = Value::Null;
        sideless["line"] = Value::Null;
        let threads = threads_of(vec![
            thread("t1", vec![thread_comment("s1", "SUBMITTED")]),
            ranged,
            sideless,
        ]);
        fn anchor(
            thread: &ReviewThread,
        ) -> (&str, DiffSide, Option<u32>, Option<DiffSide>, Option<u32>) {
            (
                thread.path.as_str(),
                thread.side,
                thread.line,
                thread.start_side,
                thread.start_line,
            )
        }
        assert_eq!(
            anchor(&threads[0]),
            ("src/api.ts", DiffSide::Old, Some(12), None, None)
        );
        assert_eq!(
            (
                threads[0].original_line,
                threads[0].resolved,
                threads[0].outdated
            ),
            (Some(10), false, false)
        );
        assert_eq!(
            anchor(&threads[1]),
            (
                "src/api.ts",
                DiffSide::New,
                Some(12),
                Some(DiffSide::Old),
                Some(8)
            )
        );
        assert_eq!(
            (
                threads[1].original_start_line,
                threads[1].resolved,
                threads[1].outdated
            ),
            (Some(6), true, true)
        );
        assert_eq!(
            anchor(&threads[2]),
            ("src/api.ts", DiffSide::New, None, None, None)
        );
    }

    /// Each side by its own name, the start's included: a range starting on
    /// the new side names it, and no side is guessed from a value GitHub
    /// does not send.
    #[test]
    fn each_diff_side_is_read_by_its_name() {
        assert_eq!(side(Some(&json!("LEFT"))), Some(DiffSide::Old));
        assert_eq!(side(Some(&json!("RIGHT"))), Some(DiffSide::New));
        assert_eq!(side(Some(&json!("MIDDLE"))), None);
        assert_eq!(side(Some(&Value::Null)), None);
        assert_eq!(side(None), None);
        let mut ranged = thread("t4", vec![thread_comment("s4", "SUBMITTED")]);
        ranged["startDiffSide"] = json!("RIGHT");
        ranged["startLine"] = json!(3);
        let threads = threads_of(vec![ranged]);
        assert_eq!(
            (
                threads[0].side,
                threads[0].start_side,
                threads[0].start_line
            ),
            (DiffSide::Old, Some(DiffSide::New), Some(3))
        );
    }

    // ------------------------------------------------------------ checks

    #[test]
    fn every_check_run_value_maps_explicitly() {
        use PullRequestCheckState::*;
        for (conclusion, expected) in [
            ("SUCCESS", Passing),
            ("FAILURE", Failing),
            ("TIMED_OUT", Failing),
            ("STARTUP_FAILURE", Failing),
            ("ACTION_REQUIRED", Failing),
            ("NEUTRAL", Neutral),
            ("STALE", Neutral),
            ("SKIPPED", Skipped),
            ("CANCELLED", Cancelled),
            ("SOMETHING_NEW", Unknown),
        ] {
            assert_eq!(
                check_run_state(Some("COMPLETED"), Some(conclusion)),
                expected,
                "{conclusion}"
            );
        }
        assert_eq!(check_run_state(Some("COMPLETED"), None), Unknown);
        for status in ["QUEUED", "IN_PROGRESS", "WAITING", "PENDING", "REQUESTED"] {
            assert_eq!(check_run_state(Some(status), None), Pending, "{status}");
            assert_eq!(
                check_run_state(Some(status), Some("SUCCESS")),
                Pending,
                "{status}"
            );
        }
        assert_eq!(
            check_run_state(Some("SOMETHING_NEW"), Some("SUCCESS")),
            Unknown
        );
        assert_eq!(check_run_state(None, Some("SUCCESS")), Unknown);
    }

    #[test]
    fn every_status_context_value_maps_explicitly() {
        use PullRequestCheckState::*;
        for (state, expected) in [
            ("SUCCESS", Passing),
            ("FAILURE", Failing),
            ("ERROR", Failing),
            ("PENDING", Pending),
            ("EXPECTED", Pending),
            ("SOMETHING_NEW", Unknown),
        ] {
            assert_eq!(status_context_state(Some(state)), expected, "{state}");
        }
        assert_eq!(status_context_state(None), Unknown);
    }

    /// `pull-request-viewer`: *Checks link out*: each check by its name and
    /// state with its link as given, the view deciding what to link.
    #[test]
    fn the_head_commits_checks_are_read_from_the_rollup() {
        let checks = checks(&pull_request(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "contexts": { "nodes": [
                { "name": "build", "status": "COMPLETED", "conclusion": "FAILURE",
                  "detailsUrl": "https://ci.example/run/7" },
                { "context": "deploy/preview", "state": "PENDING", "targetUrl": "file:///etc/passwd" },
                { "status": "COMPLETED" },
                null,
            ] } } } }] },
        })));
        assert_eq!(
            checks,
            [
                PullRequestCheck {
                    name: "build".to_string(),
                    state: PullRequestCheckState::Failing,
                    url: Some("https://ci.example/run/7".to_string()),
                },
                PullRequestCheck {
                    name: "deploy/preview".to_string(),
                    state: PullRequestCheckState::Pending,
                    url: Some("file:///etc/passwd".to_string()),
                },
            ]
        );
        let none = pull_request(json!({
            "commits": { "nodes": [{ "commit": { "statusCheckRollup": null } }] },
        }));
        assert!(super::checks(&none).is_empty(), "no checks ran");
    }

    // ------------------------------------------------------------ the detail

    #[test]
    fn the_pull_requests_own_fields_are_read() {
        let parts = parts_of(read(
            query_reply(pull_request(json!({ "files": { "totalCount": 1 } }))),
            |page| page_of(1, page),
        ));
        assert_eq!(parts.head_branch, "feature/limits");
        assert_eq!(parts.base_branch, "main");
        assert_eq!(parts.head_commit, "a".repeat(40));
        assert_eq!(parts.base_commit, "b".repeat(40));
        assert_eq!(parts.author.as_deref(), Some("ada"));
        assert_eq!(parts.description, "Adds rate limits.");
        assert_eq!(parts.files.len(), 1);
        assert_eq!(parts.unlisted_files, 0);
        let ghost = parts_of(read(
            query_reply(pull_request(json!({ "author": null }))),
            |page| page_of(0, page),
        ));
        assert_eq!(ghost.author, None);
    }

    // ------------------------------------------------------------ the gate

    /// `github-pull-requests`: *Disabling stops a detail read between
    /// requests*: once `clear` says no, nothing more is sent, and the read
    /// ends as it says.
    #[test]
    fn a_read_stopped_between_requests_sends_nothing_more() {
        for end in [ReadEnd::Abandoned, ReadEnd::Deferred { until: NOW + 60 }] {
            let sent = RefCell::new(0);
            let outcome = read_with(
                &pr("acme", "api", 42),
                |_, _| {
                    *sent.borrow_mut() += 1;
                    query_reply(pull_request(json!({})))
                },
                |_| {
                    *sent.borrow_mut() += 1;
                    page_of(0, 1)
                },
                || {
                    if *sent.borrow() == 0 {
                        Ok(())
                    } else {
                        Err(end)
                    }
                },
                &GithubLimits::new(),
                || NOW,
            );
            assert_eq!(outcome, Err(end));
            assert_eq!(*sent.borrow(), 1, "the query only");
        }
        // Stopped before its first request, it sends nothing at all.
        let outcome = read_with(
            &pr("acme", "api", 42),
            |_, _| panic!("nothing is sent"),
            |_| panic!("nothing is sent"),
            || Err(ReadEnd::Abandoned),
            &GithubLimits::new(),
            || NOW,
        );
        assert_eq!(outcome, Err(ReadEnd::Abandoned));
    }

    /// `github-pull-requests`: *Token never logged*. Every reply here echoes
    /// the token, in an error message, a rate-limit body or beside good data;
    /// none of it reaches a result, its `Debug` form or the assembled
    /// detail's wire form.
    #[test]
    fn no_outcome_carries_the_token() {
        let token = "ghp_super_secret_value";
        let echo = |status: u16, body: String| reply(status, RateHeaders::default(), Some(body));
        let with_errors = json!({
            "data": { "repository": { "pullRequest": pull_request(json!({})) } },
            "errors": [{ "type": "FORBIDDEN", "message": format!("token {token} is not authorised") }],
        })
        .to_string();
        let replies = [
            echo(200, with_errors),
            echo(200, json!({ "errors": [{ "type": "RATE_LIMITED", "message": token }] }).to_string()),
            echo(200, json!({ "data": null, "errors": [{ "type": "INSUFFICIENT_SCOPES", "message": token }] }).to_string()),
            echo(401, format!(r#"{{"message":"Bad credentials: {token}"}}"#)),
            echo(403, format!(r#"{{"message":"{token} lacks access"}}"#)),
            echo(429, format!("slow down, {token}")),
            echo(500, format!("oops {token}")),
        ];
        let pages = [
            echo(404, format!("no files for {token}")),
            ok(json!([entry(
                "a.rs",
                "modified",
                Some("@@ -1 +1 @@\n-a\n+b"),
                1,
                1
            )])),
        ];
        let mut detailed = 0;
        for query in replies {
            for page in &pages {
                let read = read(query.clone(), |_| page.clone());
                let debug = format!("{:?}", read.outcome);
                assert!(!debug.contains(token), "{debug}");
                if let Ok(parts) = read.outcome {
                    let row = crate::pull_requests::PullRequestSummary {
                        id: 42,
                        title: String::new(),
                        repo_full_name: "acme/api".to_string(),
                        source_branch: String::new(),
                        destination_branch: String::new(),
                        url: String::new(),
                        draft: false,
                        updated_at_unix: 0,
                        review: None,
                        open_tasks: 0,
                        author: None,
                        checks: None,
                        conflicting: false,
                        unresolved_threads: 0,
                        source_repo_full_name: String::new(),
                    };
                    let (detail, _) = crate::pull_request_detail::assemble(
                        pr("acme", "api", 42),
                        row,
                        parts,
                        NOW,
                    );
                    assert!(!serde_json::to_string(&detail).unwrap().contains(token));
                    detailed += 1;
                }
            }
        }
        assert_eq!(detailed, 1, "data beside an error is read, and checked");
    }
}
