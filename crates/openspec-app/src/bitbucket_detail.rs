//! The pull-request viewer's BitBucket detail read (`pull-request-viewer`:
//! *BitBucket Detail Reads*; `bitbucket-pull-requests`: *Privacy and Safety*;
//! design D7), ported from artifex's client as the panel's recipe was.
//!
//! A read sends only GETs, each to `api.bitbucket.org`, none following a
//! redirect, each carrying the credential only in its `Authorization` header:
//!
//! 1. the pull request, `/2.0/repositories/{workspace}/{repo}/pullrequests/{id}`,
//!    for its description, author, branches and commits, and its `links.diff`
//!    and `links.diffstat`;
//! 2. its diffstat, at `links.diffstat`, for counts, and for the status and
//!    rename of every file the diff does not reach, up to
//!    [`DETAIL_MAX_PAGES`] pages;
//! 3. its diff, at `links.diff`, read up to [`DIFF_LIMIT`];
//! 4. its comments, `…/comments?pagelen=100`, up to [`DETAIL_MAX_PAGES`]
//!    pages;
//! 5. its build statuses, `…/statuses?pagelen=100`, one page.
//!
//! It never requests the pull-request-scoped `/diff` or `/diffstat`, which
//! answer with a redirect: the payload's own links name their destinations.
//! A link taken from a payload, a page's `next` included, is followed only
//! when [`followable`] says so, since the credential goes with it.
//!
//! The recipe runs over an injected transport and asks, before each request,
//! whether it may still send, as `crate::github_detail` does. No outcome
//! carries the text of a reply it failed on.

use std::collections::HashMap;
use std::io::Read as _;

use openspec_core::{parse_diff_with_spans, DiffContent, DiffFile, FileStatus, SpannedFile};
use serde_json::Value;

use crate::bitbucket::{encode, BitbucketLimits, API_BASE, DETAIL_MAX_PAGES, USER_AGENT};
use crate::github_detail::{is_commit, VERSION_READ_LIMIT};
use crate::pull_request_cache::ImageFetch;
use crate::pull_request_detail::{
    file_path, hunk_digests, ConversationEntry, DiffSide, PatchDigest, PullRequestCheck,
    PullRequestCheckState, PullRequestComment, PullRequestReference, ReadEnd, ReadFile, ReadParts,
    ReviewThread, Versions,
};
use crate::pull_request_limits::Deadlines;
use crate::pull_requests::saturating_u32;
use crate::quota::parse_rfc3339_to_unix;
use crate::usage_http::{self, Auth, Verdict};

/// How much of a pull request's diff a read takes. Every file past it is too
/// large to preview, with its diffstat's status, rename and counts.
pub const DIFF_LIMIT: usize = 8 * 1024 * 1024;
/// How much of a JSON page's body a read takes: ureq's own limit for a body
/// read whole.
const JSON_LIMIT: usize = 10 * 1024 * 1024;
/// Comments and statuses asked for per page: BitBucket's largest.
const PAGE_LEN: u32 = 100;
/// Where a link from a payload may lead: the API itself, over `https`, with
/// no user information and no explicit port, beneath `/2.0/`.
const FOLLOWABLE_ROOT: &str = "https://api.bitbucket.org/2.0/";
/// Where a comment's web page may live. Only a link on BitBucket's own site
/// survives into the model, as only one does into a row.
const WEB_URL_PREFIX: &str = "https://bitbucket.org/";

/// What one GET reads: a JSON page, the diff's text, or a file's raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    Json,
    Diff,
    Raw,
}

impl Body {
    /// The `Accept` header artifex sends for it.
    fn accept(self) -> &'static str {
        match self {
            Body::Json => "application/json",
            Body::Diff => "text/plain, */*",
            Body::Raw => "*/*",
        }
    }

    /// How much of it is read: a diff to its ceiling, and a file's version to
    /// one byte past the per-file ceiling, so a longer one is known to be too
    /// large without reading it all.
    fn limit(self) -> usize {
        match self {
            Body::Json => JSON_LIMIT,
            Body::Diff => DIFF_LIMIT,
            Body::Raw => VERSION_READ_LIMIT,
        }
    }
}

/// One reply, reduced to what the read reads off it: its status, its
/// `Retry-After`, and for a 2xx its body, as received, up to its limit. A
/// transport error is no reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) retry_after: Option<String>,
    pub(crate) body: Vec<u8>,
}

impl Reply {
    /// Reads a response into a reply, its body only for a 2xx and only up to
    /// `body`'s limit. `None` when the body cannot be read: a transport error.
    pub(crate) fn read(mut response: ureq::http::Response<ureq::Body>, body: Body) -> Option<Self> {
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut bytes = Vec::new();
        if (200..=299).contains(&status) {
            response
                .body_mut()
                .as_reader()
                .take(body.limit() as u64)
                .read_to_end(&mut bytes)
                .ok()?;
        }
        Some(Self {
            status,
            retry_after,
            body: bytes,
        })
    }
}

/// One authenticated GET. Nothing about it — the URL, the error, the reply —
/// is logged, and the credential is only ever inside the header.
pub(crate) fn send_get(url: &str, username: &str, token: &str, body: Body) -> Option<Reply> {
    let response = usage_http::get_without_redirects(url, Auth::Basic { username, token })
        .header("Accept", body.accept())
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    Reply::read(response, body)
}

/// The pull request's own resource, beneath which its comments and statuses
/// lie.
fn pull_request_url(pull_request: &PullRequestReference) -> String {
    format!(
        "{API_BASE}/repositories/{}/{}/pullrequests/{}",
        encode(&pull_request.owner),
        encode(&pull_request.repo),
        pull_request.number,
    )
}

/// Whether a link taken from a payload may be requested, the credential with
/// it: an `https` URL whose host is exactly `api.bitbucket.org`, with no user
/// information and no explicit port, and a path under `/2.0/` that no dot
/// segment, plain or percent-encoded, leads out of. A backslash, which some
/// parsers read as a slash, is refused too.
fn followable(link: &str) -> bool {
    let Some(rest) = link.strip_prefix(FOLLOWABLE_ROOT) else {
        return false;
    };
    let path = rest.split(['?', '#']).next().unwrap_or_default();
    !path.contains('\\') && !path.split('/').any(is_dot_segment)
}

/// `.` or `..`, with any of its dots written `%2e`.
fn is_dot_segment(segment: &str) -> bool {
    let decoded = segment.to_ascii_lowercase().replace("%2e", ".");
    decoded == "." || decoded == ".."
}

/// The link at `pointer`, when it may be followed.
fn followable_link(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|link| followable(link))
        .map(str::to_string)
}

/// One read of `pull_request`, spelt as its matched row spells it, over an
/// injected transport: `get` sends one GET and reads its body as asked.
/// `clear` is asked before every request and ends the read when it may not
/// be sent.
pub(crate) fn read_with(
    pull_request: &PullRequestReference,
    mut get: impl FnMut(&str, Body) -> Option<Reply>,
    clear: impl Fn() -> Result<(), ReadEnd>,
    limits: &BitbucketLimits,
    now: impl Fn() -> u64,
) -> Result<ReadParts, ReadEnd> {
    let mut fetch = |url: &str, body: Body| {
        clear()?;
        verdict(get(url, body), limits, now())
    };
    let base = pull_request_url(pull_request);
    let read = json(&fetch(&base, Body::Json)?)?;
    // A payload that lacks either link, or names one `followable` refuses,
    // is unavailable: the payload parsed, and it says this pull request
    // cannot be read the way the requirement allows.
    let (Some(diffstat), Some(diff)) = (
        followable_link(&read, "/links/diffstat/href"),
        followable_link(&read, "/links/diff/href"),
    ) else {
        return Err(ReadEnd::Unavailable);
    };
    let (diffstat, size) = pages(&mut fetch, diffstat)?;
    let diff = fetch(&diff, Body::Diff)?;
    let (comments, _) = pages(&mut fetch, format!("{base}/comments?pagelen={PAGE_LEN}"))?;
    let statuses = json(&fetch(
        &format!("{base}/statuses?pagelen={PAGE_LEN}"),
        Body::Json,
    )?)?;

    let head_commit = text(&read, "/source/commit/hash");
    let (files, unlisted_files) = files(&diffstat, size, &diff);
    let (conversation, threads) = conversation_and_threads(&comments);
    Ok(ReadParts {
        head_branch: text(&read, "/source/branch/name"),
        base_branch: text(&read, "/destination/branch/name"),
        checks: checks(&statuses, &head_commit),
        head_commit,
        base_commit: text(&read, "/destination/commit/hash"),
        author: read
            .pointer("/author/display_name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        description: read
            .get("description")
            .and_then(Value::as_str)
            .or_else(|| read.pointer("/summary/raw").and_then(Value::as_str))
            .unwrap_or_default()
            .to_string(),
        conversation,
        threads,
        files,
        unlisted_files,
    })
}

/// The merge base of the cached detail's `head` and `base`, which an image
/// read reads a file's old version at (`pull-request-viewer`: *Pull-Request
/// Image Reads*).
pub(crate) fn merge_base_url(
    pull_request: &PullRequestReference,
    head: &str,
    base: &str,
) -> String {
    format!(
        "{API_BASE}/repositories/{}/{}/merge-base/{}..{}",
        encode(&pull_request.owner),
        encode(&pull_request.repo),
        encode(head),
        encode(base),
    )
}

/// One version of a file: `path` at `commit`, each of the path's segments
/// percent-encoded on its own, as a GitHub contents URL's are.
pub(crate) fn src_url(pull_request: &PullRequestReference, commit: &str, path: &str) -> String {
    let path: Vec<String> = path.split('/').map(encode).collect();
    format!(
        "{API_BASE}/repositories/{}/{}/src/{}/{}",
        encode(&pull_request.owner),
        encode(&pull_request.repo),
        encode(commit),
        path.join("/"),
    )
}

/// One image read of an image file of `pull_request`, spelt as its matched
/// row spells it, over an injected transport (`pull-request-viewer`:
/// *Pull-Request Image Reads*; `bitbucket-pull-requests`: *Privacy and
/// Safety*): the `merge-base` GET when `fetch` carries no merge base yet,
/// then a `src` GET of each version the file has, its old path at the merge
/// base and its new path at the head. Every URL is built from the row and the
/// cached detail, never from a payload. `clear` is asked before every request
/// and ends the read when it may not be sent. Returns each version's bytes
/// and the merge base they were read by.
pub(crate) fn read_images_with(
    pull_request: &PullRequestReference,
    fetch: &ImageFetch,
    mut get: impl FnMut(&str, Body) -> Option<Reply>,
    clear: impl Fn() -> Result<(), ReadEnd>,
    limits: &BitbucketLimits,
    now: impl Fn() -> u64,
) -> Result<Versions, ReadEnd> {
    let mut fetch_one = |url: &str, body: Body| {
        clear()?;
        image_verdict(get(url, body), limits, now())
    };
    let merge_base = match &fetch.merge_base {
        Some(merge_base) => merge_base.clone(),
        None => {
            let url = merge_base_url(pull_request, &fetch.head, &fetch.base);
            merge_base_in(&fetch_one(&url, Body::Json)?)?
        }
    };
    let mut version = |path: Option<&String>, commit: &str| -> Result<Option<Vec<u8>>, ReadEnd> {
        let Some(path) = path else {
            return Ok(None);
        };
        fetch_one(&src_url(pull_request, commit, path), Body::Raw).map(Some)
    };
    let old = version(fetch.paths.old.as_ref(), &merge_base)?;
    let new = version(fetch.paths.new.as_ref(), &fetch.head)?;
    Ok(Versions {
        old,
        new,
        merge_base,
    })
}

// ---- replies ----

/// An image read's reply: as a detail read's (see [`verdict`]), except that a
/// redirect is redirected, never followed, so the view can say so and link to
/// the file's diff on BitBucket.
fn image_verdict(
    reply: Option<Reply>,
    limits: &BitbucketLimits,
    now: u64,
) -> Result<Vec<u8>, ReadEnd> {
    match reply {
        Some(reply) if (300..=399).contains(&reply.status) => Err(ReadEnd::Redirected),
        reply => verdict(reply, limits, now),
    }
}

/// A `merge-base` reply's commit: a JSON object whose `hash` is a
/// 40-character hexadecimal commit. Anything else is transient.
fn merge_base_in(body: &[u8]) -> Result<String, ReadEnd> {
    json(body)?
        .get("hash")
        .and_then(Value::as_str)
        .filter(|hash| is_commit(hash))
        .map(str::to_string)
        .ok_or(ReadEnd::Transient)
}

/// A reply's body, or how the read ends. A 401, or a 403 on any detail GET,
/// is a credential problem, since a 403 means the token lacks a scope the
/// read needs; a 429 sets the shared deadline by the poller's rule; unlike
/// the poller, a redirect or a 404 is unavailable for the pull request; a
/// transport error or any other status is transient (`pull-request-viewer`:
/// *BitBucket Detail Reads*).
fn verdict(reply: Option<Reply>, limits: &BitbucketLimits, now: u64) -> Result<Vec<u8>, ReadEnd> {
    let reply = reply.ok_or(ReadEnd::Transient)?;
    match usage_http::classify(reply.status, reply.retry_after.as_deref()) {
        Verdict::Read => Ok(reply.body),
        Verdict::Unauthenticated | Verdict::Forbidden => Err(ReadEnd::Unauthenticated),
        Verdict::RateLimited { retry_after } => {
            limits.rate_limited(retry_after, now);
            Err(ReadEnd::Deferred {
                until: limits.deadlines().held_until(),
            })
        }
        Verdict::NotFound => Err(ReadEnd::Unavailable),
        Verdict::Transient if (300..=399).contains(&reply.status) => Err(ReadEnd::Unavailable),
        Verdict::Transient => Err(ReadEnd::Transient),
    }
}

/// A 2xx page's JSON object. "A successful reply whose body cannot be read
/// as the JSON its request expects SHALL be transient, as on GitHub": a page
/// that does not parse says nothing about the pull request itself, so a
/// later read may succeed. The poller calls such a body unavailable; the
/// detail read does not.
fn json(body: &[u8]) -> Result<Value, ReadEnd> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .filter(Value::is_object)
        .ok_or(ReadEnd::Transient)
}

/// Up to [`DETAIL_MAX_PAGES`] pages of a listing from `first`, following each
/// page's `next` while it may be followed: their `values` in order, and the
/// listing's `size` when a page gives it. A page without a `values` list is
/// transient, as a page that is not JSON is.
fn pages(
    fetch: &mut impl FnMut(&str, Body) -> Result<Vec<u8>, ReadEnd>,
    first: String,
) -> Result<(Vec<Value>, Option<u64>), ReadEnd> {
    let mut values = Vec::new();
    let mut size = None;
    let mut next = Some(first);
    // A counted `for`, as the poller's workspace listing is: the bound is a
    // property of the loop, and no arithmetic can stretch it.
    for _ in 0..DETAIL_MAX_PAGES {
        let Some(url) = next.take() else {
            break;
        };
        let page = json(&fetch(&url, Body::Json)?)?;
        let listed = page
            .get("values")
            .and_then(Value::as_array)
            .ok_or(ReadEnd::Transient)?;
        values.extend(listed.iter().cloned());
        size = size.or_else(|| page.get("size").and_then(Value::as_u64));
        next = followable_link(&page, "/next");
    }
    Ok((values, size))
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

// ---- the files ----

/// One diffstat entry: the counts of a file of the diff text, and the whole
/// of a file the diff did not reach.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Diffstat {
    status: FileStatus,
    old_path: Option<String>,
    new_path: Option<String>,
    added: u32,
    removed: u32,
}

impl Diffstat {
    fn read(value: &Value) -> Option<Self> {
        let path = |side: &str| {
            value
                .get(side)
                .and_then(|entry| entry.get("path"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let (old_path, new_path) = (path("old"), path("new"));
        if old_path.is_none() && new_path.is_none() {
            return None;
        }
        let count = |field: &str| {
            value
                .get(field)
                .and_then(Value::as_u64)
                .map_or(0, saturating_u32)
        };
        Some(Self {
            status: diffstat_status(
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
            old_path,
            new_path,
            added: count("lines_added"),
            removed: count("lines_removed"),
        })
    }

    fn path(&self) -> Option<&str> {
        self.new_path.as_deref().or(self.old_path.as_deref())
    }

    /// The file it describes, too large to preview: its diff text lies past
    /// the ceiling, or was never reached.
    fn too_large(self) -> ReadFile {
        ReadFile {
            file: DiffFile {
                old_path: self.old_path,
                new_path: self.new_path,
                old_mode: None,
                new_mode: None,
                status: self.status,
                additions: Some(self.added),
                deletions: Some(self.removed),
                content: DiffContent::TooLarge,
            },
            patch: None,
            hunks: None,
            blob_sha: None,
            fetch: None,
        }
    }
}

/// A diffstat entry's status: `added`, `removed` and `renamed` as named, with
/// no similarity, and `modified`, the conflict states a pull request's
/// diffstat may report, and any status this version does not know, modified.
/// The diffstat names no mode-only change and no type change.
fn diffstat_status(status: &str) -> FileStatus {
    match status {
        "added" => FileStatus::Added,
        "removed" => FileStatus::Deleted,
        "renamed" => FileStatus::Renamed { similarity: None },
        _ => FileStatus::Modified,
    }
}

/// Whether a diff was read to its ceiling, in which case its last section
/// may have been cut short.
fn reached_ceiling(diff: &[u8]) -> bool {
    diff.len() >= DIFF_LIMIT
}

/// Takes every diffstat entry for `path`, summing their counts; `None` when
/// none is. A type change the diffstat lists as a removal and an addition of
/// one path is one file of the diff text.
fn take_counts(diffstat: &mut [Option<Diffstat>], path: &str) -> Option<(u32, u32)> {
    let mut counts: Option<(u32, u32)> = None;
    for slot in diffstat.iter_mut() {
        if let Some(entry) = slot.take_if(|entry| entry.path() == Some(path)) {
            let (added, removed) = counts.unwrap_or((0, 0));
            counts = Some((
                added.saturating_add(entry.added),
                removed.saturating_add(entry.removed),
            ));
        }
    }
    counts
}

/// A file of the diff text, keeping the status, paths and modes its text
/// gives, with the diffstat's counts for its path when it has any. A binary
/// file keeps no counts. A file with text is digested now from its patch
/// text as received, the bytes of `diff` within its spans, for the byte
/// limits and its review key, and each of its hunks from the bytes of its
/// body, for its hunks' review keys, so the diff itself need not outlive the
/// read.
fn text_file(spanned: SpannedFile, counts: Option<(u32, u32)>, diff: &[u8]) -> ReadFile {
    let SpannedFile {
        mut file,
        spans,
        hunk_bodies,
    } = spanned;
    let has_text = matches!(file.content, DiffContent::Hunks { .. });
    if has_text {
        if let Some((added, removed)) = counts {
            file.additions = Some(added);
            file.deletions = Some(removed);
        }
    }
    ReadFile {
        file,
        patch: has_text.then(|| PatchDigest::of(spans.iter().map(|span| &diff[span.clone()]))),
        hunks: has_text.then(|| hunk_digests(hunk_bodies.iter().map(|body| &diff[body.clone()]))),
        blob_sha: None,
        fetch: None,
    }
}

/// The pull request's files: each file of the diff text, joined by path to
/// the diffstat's counts, then every file the diffstat lists that the diff
/// did not reach, too large to preview. A diff read to its ceiling may cut
/// its last section short, so that file is read from the diffstat too, in
/// its place in the diffstat's order. A file of the text with no path at all
/// cannot be keyed, and is left to its diffstat entry.
fn files(diffstat: &[Value], size: Option<u64>, diff: &[u8]) -> (Vec<ReadFile>, u32) {
    let mut entries: Vec<Option<Diffstat>> = diffstat.iter().map(Diffstat::read).collect();
    let mut parsed = parse_diff_with_spans(diff);
    if reached_ceiling(diff) {
        parsed.pop();
    }
    let mut files: Vec<ReadFile> = parsed
        .into_iter()
        .filter_map(|spanned| {
            let path = file_path(&spanned.file)?.to_string();
            let counts = take_counts(&mut entries, &path);
            Some(text_file(spanned, counts, diff))
        })
        .collect();
    files.extend(entries.into_iter().flatten().map(Diffstat::too_large));
    let unlisted = size.unwrap_or(0).saturating_sub(files.len() as u64);
    (files, saturating_u32(unlisted))
}

// ---- comments ----

/// One comment as read, before it is placed in the conversation or a thread.
struct Posted {
    id: u64,
    parent: Option<u64>,
    /// Its inline anchor, when it has one on a path.
    inline: Option<Value>,
    resolved: bool,
    comment: PullRequestComment,
}

impl Posted {
    fn read(value: &Value) -> Option<Self> {
        let id = value.get("id")?.as_u64()?;
        let inline = value
            .get("inline")
            .filter(|inline| {
                inline
                    .get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|path| !path.is_empty())
            })
            .cloned();
        Some(Self {
            id,
            parent: value.pointer("/parent/id").and_then(Value::as_u64),
            inline,
            resolved: value
                .get("resolution")
                .is_some_and(|resolution| !resolution.is_null()),
            comment: PullRequestComment {
                id: id.to_string(),
                author: value
                    .pointer("/user/display_name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string),
                body: text(value, "/content/raw"),
                posted_at_unix: value
                    .get("created_on")
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339_to_unix)
                    .unwrap_or(0),
                url: value
                    .pointer("/links/html/href")
                    .and_then(Value::as_str)
                    .filter(|url| url.starts_with(WEB_URL_PREFIX))
                    .map(str::to_string),
                minimized_reason: None,
                deleted: value.get("deleted").and_then(Value::as_bool) == Some(true),
            },
        })
    }
}

/// A side and line from a pair of anchor fields: the new side's line when
/// there is one, else the old side's.
fn anchor(inline: &Value, new: &str, old: &str) -> Option<(DiffSide, u32)> {
    let line = |field: &str| {
        inline
            .get(field)
            .and_then(Value::as_u64)
            .and_then(|line| u32::try_from(line).ok())
    };
    match (line(new), line(old)) {
        (Some(line), _) => Some((DiffSide::New, line)),
        (None, Some(line)) => Some((DiffSide::Old, line)),
        (None, None) => None,
    }
}

/// The review thread an inline comment starts: `inline.to` names the new
/// side, `inline.from` the old, and `start_to` or `start_from` the first line
/// of a range. A comment on the whole file has no line, and is on the new
/// side. BitBucket reports no original lines and no outdated flag.
fn thread(root: &Posted, inline: &Value) -> ReviewThread {
    let at = anchor(inline, "to", "from");
    let start = anchor(inline, "start_to", "start_from");
    ReviewThread {
        id: root.id.to_string(),
        path: text(inline, "/path"),
        side: at.map_or(DiffSide::New, |(side, _)| side),
        line: at.map(|(_, line)| line),
        start_side: start.map(|(side, _)| side),
        start_line: start.map(|(_, line)| line),
        original_line: None,
        original_start_line: None,
        resolved: root.resolved,
        outdated: false,
        comments: Vec::new(),
    }
}

/// The comments as the conversation and the review threads. Each comment
/// belongs where its root does, the comment its chain of parents leads to: a
/// root with an inline anchor starts a review thread holding every comment
/// rooted in it, its replies included, and every other comment is the
/// conversation's. Both are in submission order, a tie broken by id.
fn conversation_and_threads(comments: &[Value]) -> (Vec<ConversationEntry>, Vec<ReviewThread>) {
    let mut posted: Vec<Posted> = comments.iter().filter_map(Posted::read).collect();
    posted.sort_by_key(|posted| (posted.comment.posted_at_unix, posted.id));
    let index: HashMap<u64, usize> = posted
        .iter()
        .enumerate()
        .map(|(at, posted)| (posted.id, at))
        .collect();
    let root = |mut at: usize| {
        // Bounded by the number of comments, so a cycle of parents ends.
        for _ in 0..posted.len() {
            match posted[at].parent.and_then(|parent| index.get(&parent)) {
                Some(&parent) => at = parent,
                None => break,
            }
        }
        at
    };
    let mut conversation = Vec::new();
    let mut threads: Vec<ReviewThread> = Vec::new();
    let mut thread_of: HashMap<usize, usize> = HashMap::new();
    for (at, comment) in posted.iter().enumerate() {
        let root_at = root(at);
        let root = &posted[root_at];
        match &root.inline {
            Some(inline) => {
                let held = *thread_of.entry(root_at).or_insert_with(|| {
                    threads.push(thread(root, inline));
                    threads.len() - 1
                });
                threads[held].comments.push(comment.comment.clone());
            }
            None => conversation.push(ConversationEntry {
                review: None,
                comment: comment.comment.clone(),
            }),
        }
    }
    (conversation, threads)
}

// ---- checks ----

/// A build status's state.
fn status_state(state: Option<&str>) -> PullRequestCheckState {
    match state {
        Some("SUCCESSFUL") => PullRequestCheckState::Passing,
        Some("FAILED") => PullRequestCheckState::Failing,
        Some("INPROGRESS") => PullRequestCheckState::Pending,
        Some("STOPPED") => PullRequestCheckState::Cancelled,
        _ => PullRequestCheckState::Unknown,
    }
}

/// artifex's `sameCommit`: two hashes name one commit when the shorter is a
/// prefix of the longer, since a list carries a short hash and a status the
/// full one.
fn same_commit(a: &str, b: &str) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// The build statuses of the source head commit, as artifex's
/// `summarizeBuilds` keeps them: one whose commit prefix-matches `head`, or
/// that names no commit. `…/statuses` covers every commit of the pull
/// request, and a build of an earlier one says nothing of the head. With the
/// head unknown, every status is kept, as an empty hash prefixes any.
fn checks(statuses: &Value, head: &str) -> Vec<PullRequestCheck> {
    let field = |status: &Value, name: &str| {
        status
            .get(name)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    statuses
        .get("values")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|status| {
            status
                .pointer("/commit/hash")
                .and_then(Value::as_str)
                .is_none_or(|hash| same_commit(hash, head))
        })
        .map(|status| PullRequestCheck {
            name: field(status, "name")
                .or_else(|| field(status, "key"))
                .unwrap_or_default(),
            state: status_state(status.get("state").and_then(Value::as_str)),
            url: field(status, "url"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitbucket::BitbucketDeadline;
    use crate::events::PullRequestProvider;
    use crate::usage_http::test_response;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::cell::RefCell;

    const NOW: u64 = 1_800_000_000;
    const BASE: &str = "https://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7";
    const DIFFSTAT: &str = "https://api.bitbucket.org/2.0/repositories/acme/api/diffstat/acme/api:abc123def456%0D0123456789ab?from_pullrequest_id=7&topic=true";
    const DIFF: &str = "https://api.bitbucket.org/2.0/repositories/acme/api/diff/acme/api:abc123def456%0D0123456789ab?from_pullrequest_id=7&topic=true";

    fn acme() -> PullRequestReference {
        PullRequestReference {
            provider: PullRequestProvider::Bitbucket,
            owner: "acme".to_string(),
            repo: "api".to_string(),
            number: 7,
        }
    }

    fn ok(body: impl Into<Vec<u8>>) -> Option<Reply> {
        Some(Reply {
            status: 200,
            retry_after: None,
            body: body.into(),
        })
    }

    fn status(code: u16) -> Option<Reply> {
        Some(Reply {
            status: code,
            retry_after: None,
            body: Vec::new(),
        })
    }

    fn payload() -> Value {
        json!({
            "id": 7,
            "title": "Add rate limits",
            "description": "Adds **rate limits**.",
            "author": { "display_name": "Ada Lovelace", "uuid": "{ada}" },
            "source": {
                "branch": { "name": "feature/limits" },
                "commit": { "hash": "abc123def456" },
                "repository": { "full_name": "acme/api" }
            },
            "destination": {
                "branch": { "name": "main" },
                "commit": { "hash": "0123456789ab" },
                "repository": { "full_name": "acme/api" }
            },
            "links": {
                "diff": { "href": DIFF },
                "diffstat": { "href": DIFFSTAT },
                "html": { "href": "https://bitbucket.org/acme/api/pull-requests/7" }
            }
        })
    }

    fn diffstat_entry(
        status: &str,
        old: Option<&str>,
        new: Option<&str>,
        added: u64,
        removed: u64,
    ) -> Value {
        let side = |path: Option<&str>| path.map_or(Value::Null, |path| json!({ "path": path }));
        json!({
            "type": "diffstat", "status": status,
            "lines_added": added, "lines_removed": removed,
            "old": side(old), "new": side(new),
        })
    }

    /// A scripted BitBucket: the pull request, its diffstat and comment
    /// pages, its diff and its statuses, each overridable by URL prefix;
    /// every request recorded with the body it asked for.
    struct Api {
        payload: Value,
        diffstat: Vec<Vec<Value>>,
        diff: Vec<u8>,
        comments: Vec<Vec<Value>>,
        statuses: Vec<Value>,
        replies: Vec<(String, Option<Reply>)>,
        requested: RefCell<Vec<(String, Body)>>,
    }

    impl Api {
        fn new() -> Self {
            Self {
                payload: payload(),
                diffstat: vec![vec![diffstat_entry("modified", Some("README.md"), Some("README.md"), 1, 1)]],
                diff: b"diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n".to_vec(),
                comments: vec![Vec::new()],
                statuses: Vec::new(),
                replies: Vec::new(),
                requested: RefCell::new(Vec::new()),
            }
        }

        /// Answers every URL starting with `prefix` with `reply`.
        fn reply(mut self, prefix: &str, reply: Option<Reply>) -> Self {
            self.replies.push((prefix.to_string(), reply));
            self
        }

        /// Page `page` of a listing of `pages`, with a `next` link to the
        /// following one.
        fn page(
            pages: &[Vec<Value>],
            page: usize,
            next: impl Fn(usize) -> String,
        ) -> Option<Reply> {
            let values = pages.get(page - 1).cloned().unwrap_or_default();
            let mut body = json!({ "values": values, "pagelen": 100, "page": page });
            if page < pages.len() {
                body["next"] = json!(next(page + 1));
            }
            ok(body.to_string())
        }

        fn get(&self, url: &str, body: Body) -> Option<Reply> {
            self.requested.borrow_mut().push((url.to_string(), body));
            if let Some((_, reply)) = self
                .replies
                .iter()
                .find(|(prefix, _)| url.starts_with(prefix.as_str()))
            {
                return reply.clone();
            }
            let page = url
                .split_once("page=")
                .and_then(|(_, rest)| rest.split('&').next()?.parse().ok())
                .unwrap_or(1);
            if url == BASE {
                ok(self.payload.to_string())
            } else if url.starts_with(DIFFSTAT) {
                Self::page(&self.diffstat, page, |next| {
                    format!("{DIFFSTAT}&page={next}")
                })
            } else if url == DIFF {
                // The transport reads a diff to its ceiling, and no further.
                ok(self.diff[..self.diff.len().min(Body::Diff.limit())].to_vec())
            } else if url.starts_with(&format!("{BASE}/comments")) {
                Self::page(&self.comments, page, |next| {
                    format!("{BASE}/comments?pagelen=100&page={next}")
                })
            } else if url.starts_with(&format!("{BASE}/statuses")) {
                ok(json!({ "values": self.statuses }).to_string())
            } else {
                panic!("unscripted request: {url}")
            }
        }

        fn read(&self) -> Result<ReadParts, ReadEnd> {
            self.read_under(&BitbucketLimits::new(), || Ok(()))
        }

        fn read_under(
            &self,
            limits: &BitbucketLimits,
            clear: impl Fn() -> Result<(), ReadEnd>,
        ) -> Result<ReadParts, ReadEnd> {
            read_with(
                &acme(),
                |url, body| self.get(url, body),
                clear,
                limits,
                || NOW,
            )
        }

        fn urls(&self) -> Vec<String> {
            self.requested
                .borrow()
                .iter()
                .map(|(url, _)| url.clone())
                .collect()
        }
    }

    fn parts_of(api: &Api) -> ReadParts {
        api.read().expect("the read succeeds")
    }

    // ------------------------------------------------------------ requests

    /// `pull-request-viewer`: *A read sends its five GETs*.
    #[test]
    fn a_read_whose_listings_fit_one_page_sends_five_gets_to_the_api() {
        let api = Api::new();
        parts_of(&api);
        assert_eq!(
            *api.requested.borrow(),
            [
                (BASE.to_string(), Body::Json),
                (DIFFSTAT.to_string(), Body::Json),
                (DIFF.to_string(), Body::Diff),
                (format!("{BASE}/comments?pagelen=100"), Body::Json),
                (format!("{BASE}/statuses?pagelen=100"), Body::Json),
            ]
        );
        for url in api.urls() {
            assert!(url.starts_with("https://api.bitbucket.org/2.0/"), "{url}");
            assert!(
                !url.starts_with(&format!("{BASE}/diff")),
                "the pull-request-scoped diff: {url}"
            );
        }
    }

    #[test]
    fn the_pull_requests_segments_are_encoded() {
        let pull_request = PullRequestReference {
            owner: "my ws".to_string(),
            repo: "a/b".to_string(),
            ..acme()
        };
        assert_eq!(
            pull_request_url(&pull_request),
            "https://api.bitbucket.org/2.0/repositories/my%20ws/a%2Fb/pullrequests/7"
        );
    }

    /// `bitbucket-pull-requests`: *A payload link off the API is not
    /// followed*: a foreign `links.diff` or `links.diffstat` is never
    /// requested, and the read is unavailable.
    #[test]
    fn a_foreign_payload_link_is_never_requested() {
        for link in [
            "https://bitbucket.org/acme/api/diff/x",
            "http://api.bitbucket.org/2.0/repositories/acme/api/diff/x",
            "https://user@api.bitbucket.org/2.0/repositories/acme/api/diff/x",
            "https://user:pass@api.bitbucket.org/2.0/repositories/acme/api/diff/x",
            "https://api.bitbucket.org:443/2.0/repositories/acme/api/diff/x",
            "https://api.bitbucket.org/1.0/repositories/acme/api/diff/x",
            "https://api.bitbucket.org/internal/diff/x",
            "https://api.bitbucket.org.evil.example/2.0/diff/x",
            "https://api.bitbucket.org/2.0/../internal/diff/x",
            "https://api.bitbucket.org/2.0/repositories/../../internal",
            "https://api.bitbucket.org/2.0/%2e%2e/internal",
            "https://api.bitbucket.org/2.0/.%2E/internal",
            "https://api.bitbucket.org/2.0/%2E./internal",
            "https://api.bitbucket.org/2.0/./x",
            "https://api.bitbucket.org/2.0/x\\..\\..\\internal",
        ] {
            for field in ["diff", "diffstat"] {
                let mut api = Api::new();
                api.payload["links"][field]["href"] = json!(link);
                assert_eq!(api.read(), Err(ReadEnd::Unavailable), "{field}: {link}");
                assert_eq!(api.urls(), [BASE], "{field}: {link}");
            }
        }
        // A payload with no link at all is no better.
        let mut api = Api::new();
        api.payload["links"] = json!({});
        assert_eq!(api.read(), Err(ReadEnd::Unavailable));
    }

    /// "A payload that lacks its `links.diff` or `links.diffstat`, or names
    /// one this read would not follow, SHALL be unavailable": either link
    /// missing on its own, or not a string, ends the read after the pull
    /// request's own GET.
    #[test]
    fn a_payload_lacking_either_link_is_unavailable() {
        for field in ["diff", "diffstat"] {
            for lacking in [json!(null), json!({}), json!({ "href": 7 })] {
                let mut api = Api::new();
                api.payload["links"][field] = lacking.clone();
                assert_eq!(api.read(), Err(ReadEnd::Unavailable), "{field}: {lacking}");
                assert_eq!(api.urls(), [BASE], "{field}: {lacking}");
            }
        }
    }

    /// The links BitBucket itself writes are followed: a revision range
    /// joined by an encoded carriage return, a query, and a dot-dot in the
    /// query rather than the path.
    #[test]
    fn the_api_s_own_links_are_followed() {
        for link in [
            DIFF,
            DIFFSTAT,
            "https://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7/comments?page=2",
            "https://api.bitbucket.org/2.0/x?next=../..#..",
            "https://api.bitbucket.org/2.0/a.b/..c/c../.%2e2",
        ] {
            assert!(followable(link), "{link}");
        }
        assert!(is_dot_segment(".") && is_dot_segment("..") && is_dot_segment("%2E%2e"));
        assert!(!is_dot_segment("...") && !is_dot_segment("a..") && !is_dot_segment(""));
    }

    /// A page's `next` off the API is not followed either: the listing ends
    /// there.
    #[test]
    fn a_foreign_next_link_ends_the_listing() {
        let api = Api::new().reply(
            &format!("{BASE}/comments"),
            ok(
                json!({ "values": [], "next": "https://evil.example/2.0/comments?page=2" })
                    .to_string(),
            ),
        );
        parts_of(&api);
        assert_eq!(api.urls().len(), 5);
        assert!(api.urls().iter().all(|url| !url.contains("evil.example")));
    }

    /// `pull-request-viewer`: *Pagination is capped*: fourteen comment pages
    /// stop at ten, as do eleven diffstat pages.
    #[test]
    fn fourteen_comment_pages_stop_at_ten() {
        let mut api = Api::new();
        api.comments = (0..14).map(|_| Vec::new()).collect();
        api.diffstat = (0..11).map(|_| Vec::new()).collect();
        parts_of(&api);
        let count = |part: &str| api.urls().iter().filter(|url| url.contains(part)).count();
        assert_eq!(count("/comments"), 10);
        assert_eq!(count("/diffstat/"), 10);
        assert_eq!(DETAIL_MAX_PAGES, 10);
    }

    // ------------------------------------------------------------ the diff

    /// A new file's section of `lines` added lines, a kibibyte each.
    fn section(name: &str, lines: usize) -> Vec<u8> {
        let mut text = format!(
            "diff --git a/{name} b/{name}\nnew file mode 100644\n--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,{lines} @@\n"
        )
        .into_bytes();
        for _ in 0..lines {
            text.push(b'+');
            text.extend(std::iter::repeat_n(b'x', 1_022));
            text.push(b'\n');
        }
        text
    }

    /// `pull-request-viewer`: *A huge diff stops at its ceiling*. A 9 MiB
    /// diff is read to 8 MiB: the files it holds whole keep their hunks, and
    /// the one the ceiling cuts and every one past it are too large, with
    /// their diffstat's status, paths and counts, a rename's included.
    #[test]
    fn a_nine_mebibyte_diff_stops_at_eight_and_its_later_files_are_too_large() {
        let mut api = Api::new();
        api.diff = (1..=9)
            .flat_map(|n| section(&format!("f{n}.txt"), 1_024))
            .collect();
        assert!(api.diff.len() > 9 * 1024 * 1024);
        let mut entries: Vec<Value> = (1..=8)
            .map(|n| diffstat_entry("added", None, Some(&format!("f{n}.txt")), 1_024, 0))
            .collect();
        entries.push(diffstat_entry(
            "renamed",
            Some("old/f9.txt"),
            Some("f9.txt"),
            1_024,
            0,
        ));
        api.diffstat = vec![entries];
        let parts = parts_of(&api);
        assert_eq!(parts.files.len(), 9);
        for (n, read) in parts.files.iter().enumerate() {
            let file = &read.file;
            let name = format!("f{}.txt", n + 1);
            assert_eq!(file.new_path.as_deref(), Some(name.as_str()));
            assert_eq!(
                (file.additions, file.deletions),
                (Some(1_024), Some(0)),
                "{name}"
            );
            if n < 7 {
                assert!(
                    matches!(&file.content, DiffContent::Hunks { hunks } if hunks.len() == 1),
                    "{name}"
                );
                assert_eq!(
                    (file.status, file.new_mode.as_deref()),
                    (FileStatus::Added, Some("100644"))
                );
            } else {
                assert_eq!(file.content, DiffContent::TooLarge, "{name}");
                assert_eq!(read.patch, None);
            }
        }
        // The cut file and the rename past the ceiling, from the diffstat.
        assert_eq!(parts.files[7].file.status, FileStatus::Added);
        let renamed = &parts.files[8].file;
        assert_eq!(renamed.status, FileStatus::Renamed { similarity: None });
        assert_eq!(renamed.old_path.as_deref(), Some("old/f9.txt"));
    }

    /// Exactly the ceiling may hide a cut; a byte under it is the whole diff.
    #[test]
    fn a_diff_read_to_exactly_its_ceiling_is_taken_as_cut() {
        assert!(reached_ceiling(&vec![b'x'; DIFF_LIMIT]));
        assert!(!reached_ceiling(&vec![b'x'; DIFF_LIMIT - 1]));
        assert_eq!(DIFF_LIMIT, 8 * 1024 * 1024);
    }

    /// A mode-only change keeps the status and modes its text gives, which
    /// the diffstat cannot, and takes its counts from the diffstat.
    #[test]
    fn a_mode_only_change_stays_mode_changed_with_both_modes() {
        let mut api = Api::new();
        api.diff = b"diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\n".to_vec();
        api.diffstat = vec![vec![diffstat_entry(
            "modified",
            Some("run.sh"),
            Some("run.sh"),
            0,
            0,
        )]];
        let read = parts_of(&api).files.remove(0);
        assert_eq!(read.file.status, FileStatus::ModeChanged);
        assert_eq!(read.file.old_mode.as_deref(), Some("100644"));
        assert_eq!(read.file.new_mode.as_deref(), Some("100755"));
        assert_eq!(
            (read.file.additions, read.file.deletions),
            (Some(0), Some(0))
        );
        assert_eq!(read.file.content, DiffContent::Hunks { hunks: Vec::new() });
    }

    /// A file of the text takes the diffstat's counts for its path, and is
    /// digested from the bytes of its own section; a binary file keeps no
    /// counts and no digest.
    #[test]
    fn a_text_file_takes_its_counts_and_its_sections_digest_and_a_binary_file_neither() {
        let mut api = Api::new();
        let readme = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n";
        let image = "diff --git a/logo.png b/logo.png\nindex 1111111..2222222 100644\nBinary files a/logo.png and b/logo.png differ\n";
        api.diff = format!("{readme}{image}").into_bytes();
        api.diffstat = vec![vec![
            diffstat_entry("modified", Some("README.md"), Some("README.md"), 7, 3),
            diffstat_entry("modified", Some("logo.png"), Some("logo.png"), 0, 0),
        ]];
        let files = parts_of(&api).files;
        assert_eq!(
            (files[0].file.additions, files[0].file.deletions),
            (Some(7), Some(3))
        );
        assert_eq!(files[0].patch, Some(PatchDigest::of([readme.as_bytes()])));
        assert_eq!(files[1].file.content, DiffContent::Binary);
        assert_eq!(
            (files[1].file.additions, files[1].file.deletions),
            (None, None)
        );
        assert_eq!(files[1].patch, None);
        assert_eq!(files.len(), 2, "each diffstat entry joined, none left over");
    }

    /// A file of the text the diffstat does not list keeps its own counts.
    #[test]
    fn a_text_file_without_a_diffstat_entry_keeps_its_own_counts() {
        let mut api = Api::new();
        api.diffstat = vec![Vec::new()];
        let file = parts_of(&api).files.remove(0).file;
        assert_eq!((file.additions, file.deletions), (Some(1), Some(1)));
    }

    /// A type change the diffstat lists as two entries of one path is one
    /// file, with both entries' counts.
    #[test]
    fn a_type_change_listed_twice_is_one_file_with_both_counts() {
        let mut api = Api::new();
        api.diff = b"diff --git a/link b/link\ndeleted file mode 100644\n--- a/link\n+++ /dev/null\n@@ -1 +0,0 @@\n-text\ndiff --git a/link b/link\nnew file mode 120000\n--- /dev/null\n+++ b/link\n@@ -0,0 +1 @@\n+target\n".to_vec();
        api.diffstat = vec![vec![
            diffstat_entry("removed", Some("link"), None, 0, 1),
            diffstat_entry("added", None, Some("link"), 1, 0),
        ]];
        let files = parts_of(&api).files;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file.status, FileStatus::TypeChanged);
        assert_eq!(
            (files[0].file.additions, files[0].file.deletions),
            (Some(1), Some(1))
        );
        assert_eq!(
            files[0].patch,
            Some(PatchDigest::of([api.diff.as_slice()])),
            "both sections, in order, are its patch text"
        );
    }

    /// The digest is taken from the bytes as received, never from text
    /// decoded from them: `é` (0xE9) and `è` (0xE8) in Latin-1 are each
    /// invalid UTF-8 and parse to the same line, yet digest apart.
    #[test]
    fn one_latin1_byte_parses_alike_and_digests_apart() {
        let latin1 = |byte: u8| {
            let mut diff =
                b"diff --git a/menu.txt b/menu.txt\n--- a/menu.txt\n+++ b/menu.txt\n@@ -1 +1 @@\n-cafe\n+caf"
                    .to_vec();
            diff.extend([byte, b'\n']);
            diff
        };
        let read = |byte: u8| {
            let mut api = Api::new();
            api.diff = latin1(byte);
            api.diffstat = vec![Vec::new()];
            parts_of(&api).files.remove(0)
        };
        let (acute, grave) = (read(0xe9), read(0xe8));
        assert_eq!(acute.file, grave.file, "one line, decoded alike");
        assert_eq!(
            acute.patch,
            Some(PatchDigest::of([latin1(0xe9).as_slice()]))
        );
        assert_ne!(acute.patch, grave.patch);
        // Its one hunk too, from its body's bytes as received
        // (`pull-request-viewer`: *A hunk is keyed by the bytes the provider
        // sent*).
        let body: &[u8] = b"-cafe\n+caf\xe9\n";
        assert_eq!(acute.hunks, Some(vec![Sha256::digest(body).into()]));
        assert_ne!(acute.hunks, grave.hunks);
    }

    /// A file the diff never reached takes its status and rename from the
    /// diffstat, as does a deleted one its one path; the diffstat's other
    /// statuses map as named.
    #[test]
    fn a_file_the_diff_never_reached_takes_its_status_and_rename_from_the_diffstat() {
        let mut api = Api::new();
        api.diffstat = vec![vec![
            diffstat_entry("modified", Some("README.md"), Some("README.md"), 1, 1),
            diffstat_entry("renamed", Some("src/old.rs"), Some("src/new.rs"), 4, 2),
            diffstat_entry("removed", Some("gone.rs"), None, 0, 9),
            diffstat_entry("merge conflict", Some("x.rs"), Some("x.rs"), 1, 1),
        ]];
        let files = parts_of(&api).files;
        let renamed = &files[1].file;
        assert_eq!(renamed.status, FileStatus::Renamed { similarity: None });
        assert_eq!(renamed.old_path.as_deref(), Some("src/old.rs"));
        assert_eq!(renamed.new_path.as_deref(), Some("src/new.rs"));
        assert_eq!((renamed.additions, renamed.deletions), (Some(4), Some(2)));
        assert_eq!(renamed.content, DiffContent::TooLarge);
        assert_eq!(
            (files[2].file.status, files[2].file.new_path.as_deref()),
            (FileStatus::Deleted, None)
        );
        assert_eq!(files[3].file.status, FileStatus::Modified);
        assert_eq!(diffstat_status("added"), FileStatus::Added);
        // An entry with no path at all is no file.
        assert_eq!(
            Diffstat::read(&json!({ "status": "added", "old": null, "new": null })),
            None
        );
        let added = Diffstat::read(&diffstat_entry("added", None, Some("a.rs"), 1, 0)).unwrap();
        assert_eq!(added.path(), Some("a.rs"));
    }

    /// The diffstat's `size` counts the files the read could not list.
    #[test]
    fn files_the_diffstat_counts_but_no_page_lists_are_unlisted() {
        let entries = vec![diffstat_entry(
            "modified",
            Some("README.md"),
            Some("README.md"),
            1,
            1,
        )];
        let page = ok(json!({ "values": entries, "size": 5_001 }).to_string());
        let api = Api::new().reply(DIFFSTAT, page);
        assert_eq!(parts_of(&api).unlisted_files, 5_000);
        assert_eq!(
            parts_of(&Api::new()).unlisted_files,
            0,
            "no size, none counted"
        );
    }

    // ------------------------------------------------------------ the pull request

    #[test]
    fn the_pull_requests_own_fields_are_read() {
        let parts = parts_of(&Api::new());
        assert_eq!(parts.head_branch, "feature/limits");
        assert_eq!(parts.base_branch, "main");
        assert_eq!(parts.head_commit, "abc123def456");
        assert_eq!(parts.base_commit, "0123456789ab");
        assert_eq!(parts.author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(parts.description, "Adds **rate limits**.");

        let mut api = Api::new();
        api.payload.as_object_mut().unwrap().remove("description");
        api.payload["summary"] = json!({ "raw": "From the summary." });
        api.payload["author"] = json!({ "display_name": "" });
        let parts = parts_of(&api);
        assert_eq!(parts.description, "From the summary.");
        assert_eq!(parts.author, None);
    }

    // ------------------------------------------------------------ comments

    fn comment(id: u64, at: &str, parent: Option<u64>, inline: Option<Value>) -> Value {
        let mut comment = json!({
            "id": id,
            "content": { "raw": format!("comment {id}") },
            "user": { "display_name": "Grace" },
            "created_on": at,
            "deleted": false,
            "links": { "html": { "href": format!("https://bitbucket.org/acme/api/pull-requests/7/_/diff#comment-{id}") } },
        });
        if let Some(parent) = parent {
            comment["parent"] = json!({ "id": parent });
        }
        if let Some(inline) = inline {
            comment["inline"] = inline;
        }
        comment
    }

    fn placed_of(comments: Vec<Value>) -> ReadParts {
        let mut api = Api::new();
        api.comments = vec![comments];
        parts_of(&api)
    }

    fn ids(comments: &[PullRequestComment]) -> Vec<&str> {
        comments.iter().map(|comment| comment.id.as_str()).collect()
    }

    /// General comments, their replies included, are the conversation in
    /// submission order, a tie broken by id.
    #[test]
    fn general_comments_are_the_conversation_in_submission_order() {
        let parts = placed_of(vec![
            comment(3, "2026-09-01T10:10:00+00:00", None, None),
            comment(2, "2026-09-01T10:00:00+00:00", Some(1), None),
            comment(1, "2026-09-01T10:00:00+00:00", None, None),
        ]);
        let conversation: Vec<PullRequestComment> = parts
            .conversation
            .iter()
            .map(|entry| entry.comment.clone())
            .collect();
        assert_eq!(ids(&conversation), ["1", "2", "3"]);
        assert!(parts
            .conversation
            .iter()
            .all(|entry| entry.review.is_none()));
        assert_eq!(
            conversation[0],
            PullRequestComment {
                id: "1".to_string(),
                author: Some("Grace".to_string()),
                body: "comment 1".to_string(),
                posted_at_unix: 1_788_256_800,
                url: Some(
                    "https://bitbucket.org/acme/api/pull-requests/7/_/diff#comment-1".to_string()
                ),
                minimized_reason: None,
                deleted: false,
            }
        );
        assert!(parts.threads.is_empty());
    }

    /// `pull-request-viewer`: *A BitBucket inline comment takes its side from
    /// its anchor*, with its replies, its resolution and its deleted flag.
    #[test]
    fn an_inline_comment_and_its_replies_are_one_thread() {
        let mut root = comment(
            10,
            "2026-09-01T10:00:00+00:00",
            None,
            Some(json!({ "path": "README.md", "to": 7, "from": null })),
        );
        root["resolution"] =
            json!({ "type": "comment_resolution", "user": { "display_name": "Ada" } });
        let mut reply = comment(
            11,
            "2026-09-01T10:05:00+00:00",
            Some(10),
            Some(json!({ "path": "README.md", "to": 7 })),
        );
        reply["deleted"] = json!(true);
        let nested = comment(12, "2026-09-01T10:06:00+00:00", Some(11), None);
        let parts = placed_of(vec![nested, reply, root]);
        assert!(parts.conversation.is_empty());
        assert_eq!(parts.threads.len(), 1);
        let thread = &parts.threads[0];
        assert_eq!(
            (
                thread.id.as_str(),
                thread.path.as_str(),
                thread.side,
                thread.line
            ),
            ("10", "README.md", DiffSide::New, Some(7))
        );
        assert_eq!((thread.start_side, thread.start_line), (None, None));
        assert!(thread.resolved);
        assert!(!thread.outdated);
        assert_eq!(ids(&thread.comments), ["10", "11", "12"]);
        assert_eq!(
            thread
                .comments
                .iter()
                .map(|comment| comment.deleted)
                .collect::<Vec<_>>(),
            [false, true, false]
        );
    }

    /// `pull-request-viewer`: *Inline comments carry their side*: `from`
    /// alone names the old side, a range its first line by `start_to` or
    /// `start_from`, and a comment on the whole file no line.
    #[test]
    fn an_inline_anchor_names_its_side_line_and_range() {
        let at = "2026-09-01T10:00:00+00:00";
        let parts = placed_of(vec![
            comment(1, at, None, Some(json!({ "path": "a.rs", "from": 30 }))),
            comment(
                2,
                at,
                None,
                Some(json!({ "path": "b.rs", "to": 12, "start_to": 9 })),
            ),
            comment(
                3,
                at,
                None,
                Some(json!({ "path": "c.rs", "from": 5, "start_from": 2 })),
            ),
            comment(4, at, None, Some(json!({ "path": "d.rs" }))),
            comment(5, at, None, Some(json!({ "path": "", "to": 1 }))),
        ]);
        let anchors: Vec<_> = parts
            .threads
            .iter()
            .map(|thread| {
                (
                    thread.path.as_str(),
                    thread.side,
                    thread.line,
                    thread.start_side,
                    thread.start_line,
                )
            })
            .collect();
        assert_eq!(
            anchors,
            [
                ("a.rs", DiffSide::Old, Some(30), None, None),
                (
                    "b.rs",
                    DiffSide::New,
                    Some(12),
                    Some(DiffSide::New),
                    Some(9)
                ),
                ("c.rs", DiffSide::Old, Some(5), Some(DiffSide::Old), Some(2)),
                ("d.rs", DiffSide::New, None, None, None),
            ]
        );
        assert_eq!(parts.conversation[0].comment.id, "5", "no path, no thread");
        assert!(parts.threads.iter().all(|thread| !thread.resolved));
    }

    /// A reply whose parent was not read, past the last page, stands on its
    /// own; a cycle of parents ends.
    #[test]
    fn an_orphan_reply_stands_alone_and_a_cycle_of_parents_ends() {
        let at = "2026-09-01T10:00:00+00:00";
        let parts = placed_of(vec![
            comment(5, at, Some(999), Some(json!({ "path": "a.rs", "to": 1 }))),
            comment(6, at, Some(7), None),
            comment(7, at, Some(6), None),
        ]);
        assert_eq!(parts.threads.len(), 1);
        assert_eq!(ids(&parts.threads[0].comments), ["5"]);
        assert_eq!(parts.conversation.len(), 2);
    }

    #[test]
    fn a_comment_without_an_id_is_skipped_and_a_foreign_link_dropped() {
        let mut foreign = comment(1, "not a time", None, None);
        foreign["links"]["html"]["href"] = json!("https://evil.example/x");
        foreign["user"] = Value::Null;
        let parts = placed_of(vec![foreign, json!({ "content": { "raw": "no id" } })]);
        assert_eq!(parts.conversation.len(), 1);
        let read = &parts.conversation[0].comment;
        assert_eq!(
            (
                read.url.as_deref(),
                read.author.as_deref(),
                read.posted_at_unix
            ),
            (None, None, 0)
        );
    }

    // ------------------------------------------------------------ checks

    fn build(state: &str, hash: Option<&str>, name: &str) -> Value {
        let mut status = json!({ "key": format!("key-{name}"), "name": name, "state": state, "url": format!("https://ci.example/{name}") });
        if let Some(hash) = hash {
            status["commit"] = json!({ "hash": hash });
        }
        status
    }

    /// A failed build on an earlier commit and a passing one on the head list
    /// only the passing one; a status that names no commit is kept.
    #[test]
    fn only_the_head_commits_builds_are_checks() {
        let mut api = Api::new();
        api.statuses = vec![
            build(
                "FAILED",
                Some("9999999999999999999999999999999999999999"),
                "old",
            ),
            build(
                "SUCCESSFUL",
                Some("abc123def4567890abc123def4567890abc12345"),
                "head",
            ),
            build("INPROGRESS", None, "unanchored"),
        ];
        let checks = parts_of(&api).checks;
        assert_eq!(
            checks,
            [
                PullRequestCheck {
                    name: "head".to_string(),
                    state: PullRequestCheckState::Passing,
                    url: Some("https://ci.example/head".to_string()),
                },
                PullRequestCheck {
                    name: "unanchored".to_string(),
                    state: PullRequestCheckState::Pending,
                    url: Some("https://ci.example/unanchored".to_string()),
                },
            ]
        );
    }

    #[test]
    fn a_status_is_named_by_its_name_else_its_key() {
        let mut nameless = build("STOPPED", None, "");
        nameless["key"] = json!("deploy");
        nameless.as_object_mut().unwrap().remove("url");
        let checks = checks(&json!({ "values": [nameless] }), "abc");
        assert_eq!(checks[0].name, "deploy");
        assert_eq!(checks[0].url, None);
        assert_eq!(checks[0].state, PullRequestCheckState::Cancelled);
    }

    #[test]
    fn every_build_state_maps_explicitly() {
        use PullRequestCheckState::*;
        for (state, expected) in [
            ("SUCCESSFUL", Passing),
            ("FAILED", Failing),
            ("INPROGRESS", Pending),
            ("STOPPED", Cancelled),
            ("SOMETHING_NEW", Unknown),
        ] {
            assert_eq!(status_state(Some(state)), expected, "{state}");
        }
        assert_eq!(status_state(None), Unknown);
    }

    /// A short hash names the commit its full hash does, in either order; an
    /// empty head keeps every status.
    #[test]
    fn hashes_name_one_commit_when_one_prefixes_the_other() {
        assert!(same_commit("abc123", "abc123def456"));
        assert!(same_commit("abc123def456", "abc123"));
        assert!(!same_commit("abc124", "abc123def456"));
        assert!(same_commit("abc123", ""));
    }

    // ------------------------------------------------------------ replies

    /// `pull-request-viewer`: *A token without a needed scope is a credential
    /// problem*: a 401 or a 403 on any GET.
    #[test]
    fn a_diffstat_403_is_a_credential_problem() {
        let (comments, statuses) = (format!("{BASE}/comments"), format!("{BASE}/statuses"));
        for code in [401, 403] {
            for prefix in [BASE, DIFFSTAT, DIFF, comments.as_str(), statuses.as_str()] {
                let api = Api::new().reply(prefix, status(code));
                assert_eq!(
                    api.read(),
                    Err(ReadEnd::Unauthenticated),
                    "{code} on {prefix}"
                );
            }
        }
    }

    /// `pull-request-viewer`: *A redirect is reported rather than followed*:
    /// a redirect on any GET is unavailable, and its target is never asked
    /// for; so is a 404.
    #[test]
    fn a_redirect_on_any_get_is_unavailable_and_never_followed() {
        let (comments, statuses) = (format!("{BASE}/comments"), format!("{BASE}/statuses"));
        for prefix in [BASE, DIFFSTAT, DIFF, comments.as_str(), statuses.as_str()] {
            for code in [301, 302, 307, 308, 404] {
                let api = Api::new().reply(
                    prefix,
                    Some(Reply {
                        status: code,
                        retry_after: None,
                        body: b"https://elsewhere.example/".to_vec(),
                    }),
                );
                assert_eq!(api.read(), Err(ReadEnd::Unavailable), "{code} on {prefix}");
                let urls = api.urls();
                assert!(urls.iter().all(|url| !url.contains("elsewhere")));
                assert!(
                    urls.last().is_some_and(|url| url.starts_with(prefix)),
                    "{urls:?}"
                );
            }
        }
    }

    /// `pull-request-viewer`: *A rate limit sets the shared deadline*: a bare
    /// 429 sets it 300 s ahead, and the read waits it out.
    #[test]
    fn a_bare_429_sets_the_deadline_five_minutes_ahead() {
        let limits = BitbucketLimits::new();
        let api = Api::new().reply(DIFF, status(429));
        assert_eq!(
            api.read_under(&limits, || Ok(())),
            Err(ReadEnd::Deferred { until: NOW + 300 })
        );
        assert_eq!(limits.deadlines(), BitbucketDeadline { until: NOW + 300 });
        let limits = BitbucketLimits::new();
        let hinted = Api::new().reply(
            DIFFSTAT,
            Some(Reply {
                status: 429,
                retry_after: Some("900".to_string()),
                body: Vec::new(),
            }),
        );
        assert_eq!(
            hinted.read_under(&limits, || Ok(())),
            Err(ReadEnd::Deferred { until: NOW + 900 })
        );
    }

    /// "A transport error or any other status SHALL be transient", and "A
    /// successful reply whose body cannot be read as the JSON its request
    /// expects SHALL be transient, as on GitHub": a 2xx page that is not a
    /// JSON object, or a listing page without its `values` list, on any JSON
    /// GET. Nothing in it says the pull request is gone, and the read sends
    /// nothing more.
    #[test]
    fn a_transport_error_another_status_or_a_bad_body_is_transient() {
        assert_eq!(Api::new().reply(DIFF, None).read(), Err(ReadEnd::Transient));
        assert_eq!(
            Api::new().reply(BASE, status(500)).read(),
            Err(ReadEnd::Transient)
        );
        let (comments, statuses) = (format!("{BASE}/comments"), format!("{BASE}/statuses"));
        for prefix in [BASE, DIFFSTAT, comments.as_str(), statuses.as_str()] {
            for body in ["<html>", "[]", "null", ""] {
                let api = Api::new().reply(prefix, ok(body));
                assert_eq!(api.read(), Err(ReadEnd::Transient), "{body:?} on {prefix}");
                assert!(
                    api.urls().last().is_some_and(|url| url.starts_with(prefix)),
                    "nothing after it"
                );
            }
        }
        for prefix in [DIFFSTAT, comments.as_str()] {
            for page in [r#"{"values": {}}"#, r#"{"size": 3}"#] {
                let api = Api::new().reply(prefix, ok(page));
                assert_eq!(api.read(), Err(ReadEnd::Transient), "{page} on {prefix}");
            }
        }
    }

    /// `bitbucket-pull-requests`: *Disabling stops a detail read between
    /// requests*, as saving a credential does: once `clear` says no, nothing
    /// more is sent.
    #[test]
    fn a_read_stopped_between_requests_sends_nothing_more() {
        let api = Api::new();
        let outcome = api.read_under(&BitbucketLimits::new(), || {
            if api.requested.borrow().is_empty() {
                Ok(())
            } else {
                Err(ReadEnd::Abandoned)
            }
        });
        assert_eq!(outcome, Err(ReadEnd::Abandoned));
        assert_eq!(api.urls(), [BASE]);
    }

    /// `bitbucket-pull-requests`: *Token never logged*. Every failed reply
    /// echoes the credential; none of it reaches an outcome.
    #[test]
    fn no_outcome_carries_the_credential() {
        let token = "ATBB-super-secret";
        for code in [401, 403, 404, 302, 429, 500] {
            let api = Api::new().reply(
                DIFF,
                Some(Reply {
                    status: code,
                    retry_after: Some(token.to_string()),
                    body: format!("bad credential {token}").into_bytes(),
                }),
            );
            let debug = format!("{:?}", api.read());
            assert!(!debug.contains(token), "{code}: {debug}");
        }
    }

    // ------------------------------------------------------------ the transport

    /// A reply's body is read only for a 2xx, and only to its limit: a 9 MiB
    /// diff stops at 8 MiB.
    #[test]
    fn a_reply_reads_a_body_only_for_a_2xx_and_only_to_its_limit() {
        let huge = vec![b'+'; 9 * 1024 * 1024];
        let reply = Reply::read(test_response(200, &[], huge.clone()), Body::Diff).unwrap();
        assert_eq!(reply.body.len(), DIFF_LIMIT);
        let reply = Reply::read(test_response(200, &[], huge), Body::Json).unwrap();
        assert_eq!(
            reply.body.len(),
            9 * 1024 * 1024,
            "a JSON page reads to its own limit"
        );
        let reply = Reply::read(
            test_response(429, &[("Retry-After", "120")], "slow down"),
            Body::Json,
        )
        .unwrap();
        assert_eq!(
            reply,
            Reply {
                status: 429,
                retry_after: Some("120".to_string()),
                body: Vec::new(),
            }
        );
        for code in [302, 404] {
            assert_eq!(
                Reply::read(test_response(code, &[], "x"), Body::Diff)
                    .unwrap()
                    .body,
                b""
            );
        }
        assert_eq!(Body::Json.limit(), 10 * 1024 * 1024);
        assert_eq!(Body::Json.accept(), "application/json");
        assert_eq!(Body::Diff.accept(), "text/plain, */*");
        assert_eq!(Body::Raw.accept(), "*/*");
        assert_eq!(Body::Raw.limit(), 8 * 1024 * 1024 + 1);
    }

    // ------------------------------------------------------------ image reads

    const MERGE: &str = "4444444444444444444444444444444444444444";

    fn image_fetch(merge_base: Option<&str>) -> ImageFetch {
        ImageFetch {
            paths: crate::pull_request_detail::FetchPaths {
                old: Some("img/a b+c.png".to_string()),
                new: Some("img/a b+c.png".to_string()),
            },
            head: "abc123def456".to_string(),
            base: "0123456789ab".to_string(),
            merge_base: merge_base.map(str::to_string),
        }
    }

    /// `bitbucket-pull-requests`: *An image read stays on the API host*: each
    /// URL from the row and the cached detail, each path segment encoded on
    /// its own.
    #[test]
    fn image_urls_are_built_from_the_row_and_the_cached_detail() {
        let odd = PullRequestReference {
            owner: "acme co".to_string(),
            repo: "web/app".to_string(),
            ..acme()
        };
        assert_eq!(
            merge_base_url(&odd, "abc123def456", "0123456789ab"),
            "https://api.bitbucket.org/2.0/repositories/acme%20co/web%2Fapp/merge-base/abc123def456..0123456789ab"
        );
        assert_eq!(
            src_url(&acme(), MERGE, "img/a b+c.png"),
            format!(
                "https://api.bitbucket.org/2.0/repositories/acme/api/src/{MERGE}/img/a%20b%2Bc.png"
            )
        );
    }

    /// The merge base, then both versions; with the merge base known, only
    /// the versions; and none for a side the file does not have.
    #[test]
    fn an_image_read_sends_the_merge_base_once_and_each_version_it_has() {
        let limits = BitbucketLimits::new();
        let sent = RefCell::new(Vec::new());
        let get = |url: &str, body: Body| {
            sent.borrow_mut().push((url.to_string(), body));
            if url.contains("/merge-base/") {
                ok(json!({ "type": "commit", "hash": MERGE }).to_string())
            } else {
                ok(format!("bytes of {url}"))
            }
        };
        let read =
            read_images_with(&acme(), &image_fetch(None), get, || Ok(()), &limits, || NOW).unwrap();
        let api = "https://api.bitbucket.org/2.0/repositories/acme/api";
        let old = format!("{api}/src/{MERGE}/img/a%20b%2Bc.png");
        let new = format!("{api}/src/abc123def456/img/a%20b%2Bc.png");
        assert_eq!(
            sent.take(),
            [
                (
                    format!("{api}/merge-base/abc123def456..0123456789ab"),
                    Body::Json
                ),
                (old.clone(), Body::Raw),
                (new.clone(), Body::Raw),
            ]
        );
        assert_eq!(
            read,
            Versions {
                old: Some(format!("bytes of {old}").into_bytes()),
                new: Some(format!("bytes of {new}").into_bytes()),
                merge_base: MERGE.to_string(),
            }
        );

        let added = ImageFetch {
            paths: crate::pull_request_detail::FetchPaths {
                old: None,
                ..image_fetch(None).paths
            },
            ..image_fetch(Some(MERGE))
        };
        let read = read_images_with(&acme(), &added, get, || Ok(()), &limits, || NOW).unwrap();
        assert_eq!(sent.take(), [(new, Body::Raw)]);
        assert_eq!(read.old, None);
    }

    /// Every request is cleared first, and a read that may not send sends
    /// nothing more.
    #[test]
    fn an_image_read_asks_before_each_request() {
        let limits = BitbucketLimits::new();
        let asked = RefCell::new(0);
        let clear = || {
            *asked.borrow_mut() += 1;
            if *asked.borrow() > 1 {
                Err(ReadEnd::Abandoned)
            } else {
                Ok(())
            }
        };
        let sent = RefCell::new(0);
        let get = |_: &str, _: Body| {
            *sent.borrow_mut() += 1;
            ok(json!({ "hash": MERGE }).to_string())
        };
        assert_eq!(
            read_images_with(&acme(), &image_fetch(None), get, clear, &limits, || NOW),
            Err(ReadEnd::Abandoned)
        );
        assert_eq!((asked.take(), sent.take()), (2, 1));
    }

    /// `pull-request-viewer`: *Pull-Request Image Reads*: BitBucket's
    /// replies, classified. A redirect is redirected, never unavailable.
    #[test]
    fn an_image_reads_replies_are_classified() {
        let limits = BitbucketLimits::new();
        let verdict = |reply| image_verdict(reply, &limits, NOW);
        assert_eq!(verdict(ok("bytes")), Ok(b"bytes".to_vec()));
        for code in [301, 302, 307, 308] {
            assert_eq!(verdict(status(code)), Err(ReadEnd::Redirected), "{code}");
        }
        assert_eq!(verdict(status(401)), Err(ReadEnd::Unauthenticated));
        assert_eq!(verdict(status(403)), Err(ReadEnd::Unauthenticated));
        assert_eq!(verdict(status(404)), Err(ReadEnd::Unavailable));
        assert_eq!(verdict(status(500)), Err(ReadEnd::Transient));
        assert_eq!(verdict(None), Err(ReadEnd::Transient));
        assert_eq!(
            verdict(status(429)),
            Err(ReadEnd::Deferred {
                until: limits.deadlines().held_until()
            })
        );
        assert!(
            limits.deadlines().held_until() > NOW,
            "a 429 sets the shared deadline"
        );
    }

    #[test]
    fn a_merge_base_reply_names_a_full_commit() {
        assert_eq!(
            merge_base_in(
                json!({ "type": "commit", "hash": MERGE })
                    .to_string()
                    .as_bytes()
            ),
            Ok(MERGE.to_string())
        );
        for body in [
            json!({ "hash": "abc123def456" }).to_string(),
            json!({ "hash": format!("{}g", &MERGE[..39]) }).to_string(),
            json!({ "hash": 7 }).to_string(),
            json!({ "commit": MERGE }).to_string(),
            json!([MERGE]).to_string(),
            "not json".to_string(),
        ] {
            assert_eq!(
                merge_base_in(body.as_bytes()),
                Err(ReadEnd::Transient),
                "{body}"
            );
        }
    }
}
