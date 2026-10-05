//! A pull request as the viewer names it, the snapshot lookup every detail
//! read goes through, and the detail a read builds (`pull-request-viewer`:
//! *Detail Reads Are Scoped to the Snapshot*, *Pull-Request View*;
//! `view-routing`: *Pull-Request Addresses*).
//!
//! The reference — provider, owner, repository, number — is all a frontend
//! sends. It is never a URL, and none of it reaches a provider request: the
//! service looks it up in that provider's current snapshot and reads through
//! the matched row's own values, so a caller of either transport can spend
//! the host's credential only on pull requests the host's account already
//! lists (design D2, D5).
//!
//! The detail is one model whichever provider it came from, so the view
//! renders it the same in the center pane and in the pull-request window. A
//! provider's recipe (`crate::github_detail`, `crate::bitbucket_detail`)
//! reads [`ReadParts`]; [`assemble`] applies the line and byte budgets to its
//! files and keeps beside each, never on the wire, what a withheld file's
//! load and a review key need.

use openspec_core::{eager_files, DiffContent, DiffFile, Hunk, PatchSize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bitbucket::BitbucketPullRequestsState;
use crate::events::PullRequestProvider;
use crate::github::GithubPullRequestsState;
use crate::pull_requests::PullRequestSummary;
use crate::window_title::is_default_ignorable;

// ---- the reference ----

/// A pull request as every pull-request command and address names it: its
/// provider, its owner (a GitHub owner or a BitBucket workspace), its
/// repository and its number. Hand-mirrored in `src/types.ts`.
///
/// Two references are equal when their providers and numbers are equal and
/// their owners and repositories are equal ignoring ASCII case, as
/// `pull_request_links.rs` compares repositories, so an address spelt
/// `ACME/Api` names the row listed as `acme/api`. That equality is why the
/// type is not `Hash`: a map is keyed by [`PullRequestReference::key`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReference {
    pub provider: PullRequestProvider,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl PartialEq for PullRequestReference {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.number == other.number
            && self.owner.eq_ignore_ascii_case(&other.owner)
            && self.repo.eq_ignore_ascii_case(&other.repo)
    }
}

impl Eq for PullRequestReference {}

/// A reference's canonical key: the provider, the owner and repository in
/// lowercase, and the number. Equal references have equal keys, so the detail
/// cache and the review-progress store key by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PullRequestKey {
    pub provider: PullRequestProvider,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl PullRequestReference {
    /// The canonical key, whatever case the reference was spelt in.
    pub fn key(&self) -> PullRequestKey {
        PullRequestKey {
            provider: self.provider,
            owner: self.owner.to_ascii_lowercase(),
            repo: self.repo.to_ascii_lowercase(),
            number: self.number,
        }
    }

    /// Whether `row` is the pull request this reference names: its
    /// `repo_full_name` splits into an equal owner and repository, its id is
    /// the number, and its URL is not empty. A row without a URL on its
    /// provider's own site cannot be opened from its panel, and a hand-made
    /// address must not open it either. Matching the provider is the caller's
    /// part, by the snapshot it looks in.
    pub fn names(&self, row: &PullRequestSummary) -> bool {
        let Some((owner, repo)) = row.repo_full_name.split_once('/') else {
            return false;
        };
        row.id == self.number
            && !row.url.is_empty()
            && owner.eq_ignore_ascii_case(&self.owner)
            && repo.eq_ignore_ascii_case(&self.repo)
    }

    /// The reference `row` is listed under in `provider`'s snapshot, spelt as
    /// the row spells it. `None` for a row whose repository has no owner
    /// half, which no reference names.
    pub(crate) fn of_row(provider: PullRequestProvider, row: &PullRequestSummary) -> Option<Self> {
        let (owner, repo) = row.repo_full_name.split_once('/')?;
        Some(Self {
            provider,
            owner: owner.to_string(),
            repo: repo.to_string(),
            number: row.id,
        })
    }
}

/// The row `reference` names in its own provider's current snapshot, fresh or
/// stale, and in either GitHub list. `None` when it is not listed there, in
/// which case a detail read sends nothing.
pub fn listed_row(
    reference: &PullRequestReference,
    bitbucket: &BitbucketPullRequestsState,
    github: &GithubPullRequestsState,
) -> Option<PullRequestSummary> {
    let row = match reference.provider {
        PullRequestProvider::Bitbucket => bitbucket
            .pull_requests
            .iter()
            .find(|row| reference.names(row)),
        PullRequestProvider::Github => github.rows().find(|row| reference.names(row)),
    };
    row.cloned()
}

// ---- the detail ----
//
// Every type below is camelCase on the wire and hand-mirrored in
// `src/types.ts`; `tests/wire_shape.rs` pins the keys, each outcome's `kind`
// and each string-valued enum's values.

/// Which side of a diff a review thread is anchored on (`pull-request-viewer`:
/// *Changed Files in the Pull-Request View*). GitHub's `diffSide` gives it,
/// `LEFT` old and `RIGHT` new, and BitBucket's anchor gives it, `inline.from`
/// old and `inline.to` new. A bare line number could be in either column of
/// a side-by-side diff, so a thread always names its side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffSide {
    Old,
    New,
}

/// A submitted review's state, as GitHub reports it. The account's own
/// pending review never reaches the model: the query asks for these four
/// states only, and the parser drops any other should one appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

/// One check's state, mapped explicitly from every value its provider
/// reports: a GitHub check run's `status` and `conclusion`, a GitHub status
/// context's `state`, and a BitBucket build status's `state`. A value this
/// version does not know is `Unknown`, never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestCheckState {
    Passing,
    Failing,
    Pending,
    Neutral,
    Skipped,
    Cancelled,
    Unknown,
}

/// One comment, in the conversation or in a review thread. Its body is
/// untrusted markdown, which the view renders in its pull-request mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestComment {
    /// GitHub's node id, or BitBucket's comment id as text, kept so a later
    /// change can reply to it.
    pub id: String,
    /// The author's GitHub login or BitBucket display name; `None` for a
    /// deleted account.
    pub author: Option<String>,
    pub body: String,
    /// When the comment was posted, or the review submitted, as Unix epoch
    /// seconds; `0` when unreadable.
    pub posted_at_unix: u64,
    /// Its web page, kept only when it is on its provider's own site.
    pub url: Option<String>,
    /// `Some` when GitHub reports it minimised: GitHub's stated reason, empty
    /// when it states none. The view renders it collapsed behind the reason.
    pub minimized_reason: Option<String>,
    /// BitBucket reports it deleted.
    pub deleted: bool,
}

/// One entry of the conversation: the pull request's own comment, or a
/// submitted review's summary, which names the review's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEntry {
    /// The review's state; `None` for a comment.
    pub review: Option<ReviewState>,
    pub comment: PullRequestComment,
}

/// One check of the head commit: a GitHub check run or status context, or a
/// BitBucket build status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCheck {
    pub name: String,
    pub state: PullRequestCheckState,
    /// Its details page as its provider gives it, whatever the scheme: the
    /// view links it only when it is an absolute `http` or `https` URL with a
    /// host.
    pub url: Option<String>,
}

/// One review thread: a GitHub review thread, or a BitBucket inline comment
/// with its replies. Its anchor is kept whole, side included, so a later
/// change can place it in the diff and reply to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewThread {
    pub id: String,
    pub path: String,
    /// The side its line is on; the new side when the provider names none,
    /// as GitHub's `RIGHT` is for a context line.
    pub side: DiffSide,
    /// Its line on that side; `None` for a comment on the whole file, or a
    /// GitHub thread whose line no longer exists.
    pub line: Option<u32>,
    /// The first line's side and number, for a comment on a range.
    pub start_side: Option<DiffSide>,
    pub start_line: Option<u32>,
    /// GitHub's lines when the thread was started, which an outdated
    /// thread's label shows. BitBucket gives none.
    pub original_line: Option<u32>,
    pub original_start_line: Option<u32>,
    pub resolved: bool,
    /// GitHub reports the thread outdated by a later push. BitBucket gives no
    /// such flag.
    pub outdated: bool,
    /// Never empty: a thread left with no comment is not shown.
    pub comments: Vec<PullRequestComment>,
}

/// One pull request as the view renders it, from whichever provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestDetail {
    /// The pull request, spelt as the row it was read through spells it.
    pub reference: PullRequestReference,
    /// The row it was read through, whose signals the header shows once the
    /// live row is gone.
    pub row: PullRequestSummary,
    pub head_branch: String,
    pub base_branch: String,
    /// GitHub's head and base commit ids, or BitBucket's source and
    /// destination commits: a withheld file's load and a viewed mark name
    /// the ones the view rendered.
    pub head_commit: String,
    pub base_commit: String,
    pub author: Option<String>,
    /// Untrusted markdown.
    pub description: String,
    /// Comments and review summaries, as one list in submission order.
    pub conversation: Vec<ConversationEntry>,
    pub checks: Vec<PullRequestCheck>,
    pub threads: Vec<ReviewThread>,
    /// The changed files under the line and byte budgets. A withheld file
    /// loads from the cache, through `get_pull_request_file`.
    pub files: Vec<DiffFile>,
    /// Files the pull request changes that are not listed: on GitHub, those
    /// past the thousandth.
    pub unlisted_files: u32,
    /// When it was read, as Unix epoch seconds.
    pub read_at_unix: u64,
    /// The pull request has left its provider's list since this, its last
    /// detail, was read.
    pub no_longer_listed: bool,
}

/// What `get_pull_request_detail` answers (`pull-request-viewer`: *Detail
/// Reads Are Scoped to the Snapshot*). Only a read can carry a fresh detail;
/// every other answer sent nothing, or stopped, and carries no content of the
/// provider's beyond what was already cached.
///
/// The detail is boxed: it dwarfs every other variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PullRequestDetailOutcome {
    /// The detail: read now, or answered from the cache, then marked no
    /// longer listed when its pull request has left its list.
    Detail { detail: Box<PullRequestDetail> },
    /// Not in its provider's list, and nothing cached: nothing was sent.
    NotListed,
    /// A cache-only call, and nothing cached.
    NotCached,
    /// The provider is disabled: no content at all.
    Refused,
    /// No credential, or the provider rejected it: Settings fixes it.
    Unauthenticated,
    /// The pull request cannot be read: moved, deleted, or redirected.
    Unavailable,
    /// A rate-limit deadline or the hourly budget holds: nothing more is
    /// sent, and a read becomes possible at `until_unix`. Any cached detail
    /// rides along, so the view keeps painting it.
    Deferred {
        until_unix: u64,
        detail: Option<Box<PullRequestDetail>>,
    },
    /// A transport error or any other failure: a later read may succeed.
    Transient,
}

// ---- what a read builds ----

/// The path a file goes by, as the view keys it: its new path, or its old one
/// once deleted.
pub(crate) fn file_path(file: &DiffFile) -> Option<&str> {
    file.new_path.as_deref().or(file.old_path.as_deref())
}

/// Why a read ended without a detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadEnd {
    /// The provider was disabled, or its credential saved, since the read
    /// started: it sends nothing more, and its result is neither cached nor
    /// returned.
    Abandoned,
    Unauthenticated,
    Unavailable,
    /// A deadline holds until then, set by this read's own rate-limited reply
    /// or by another request meanwhile.
    Deferred {
        until: u64,
    },
    Transient,
}

/// What is kept of a file's patch text, beside the file in the cache and
/// never on the wire: its length, which the byte limits measure, and its
/// SHA-256, which keys a review mark (`pull-request-viewer`: *Review
/// Progress*). Both are taken from the bytes exactly as the provider sent
/// them — GitHub's `patch` field, or the bytes of BitBucket's diff within the
/// file's spans, both sections of a folded type change included — while the
/// read holds them, so no cache entry keeps the text beside the hunks parsed
/// from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PatchDigest {
    /// Its length in bytes.
    pub(crate) len: usize,
    /// The SHA-256 of its bytes, in order.
    pub(crate) sha256: [u8; 32],
}

impl PatchDigest {
    /// The digest of the patch text made of `parts`, in order: one stream,
    /// however the bytes are split.
    pub(crate) fn of<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut hasher = Sha256::new();
        let mut len = 0;
        for part in parts {
            hasher.update(part);
            len += part.len();
        }
        Self {
            len,
            sha256: hasher.finalize().into(),
        }
    }
}

/// One changed file as a recipe read it: the file with every hunk it has, and
/// what its review key is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadFile {
    pub(crate) file: DiffFile,
    /// Its patch text's digest; `None` for a file that has no patch text: a
    /// binary file, one too large or past a ceiling, or a GitHub entry
    /// without a `patch`.
    pub(crate) patch: Option<PatchDigest>,
    /// GitHub's blob `sha` for the file, when GitHub gives one.
    pub(crate) blob_sha: Option<String>,
}

/// Everything a provider's recipe read of one pull request, before the
/// budgets apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadParts {
    pub(crate) head_branch: String,
    pub(crate) base_branch: String,
    pub(crate) head_commit: String,
    pub(crate) base_commit: String,
    pub(crate) author: Option<String>,
    pub(crate) description: String,
    pub(crate) conversation: Vec<ConversationEntry>,
    pub(crate) checks: Vec<PullRequestCheck>,
    pub(crate) threads: Vec<ReviewThread>,
    pub(crate) files: Vec<ReadFile>,
    pub(crate) unlisted_files: u32,
}

/// One file of a cached detail, beside the file the detail carries. Never on
/// the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedFile {
    /// Its hunks when the budgets withheld it: a load serves them, with no
    /// request.
    pub(crate) withheld: Option<Vec<Hunk>>,
    /// Its patch text's digest, withheld or not.
    pub(crate) patch: Option<PatchDigest>,
    /// GitHub's blob `sha` for the file, when GitHub gives one.
    pub(crate) blob_sha: Option<String>,
}

/// A file's size as the budgets measure it, or `None` when it has no hunks to
/// show: a binary or too-large file, or one with no textual change, which the
/// budgets never withhold (`diff-view`: *Line and Byte Budgets With
/// On-Request Loading*). Its changed lines are its counts, as a commit's are
/// its numstat, and its bytes are its patch text's.
fn patch_size(read: &ReadFile) -> Option<PatchSize> {
    let DiffContent::Hunks { hunks } = &read.file.content else {
        return None;
    };
    let lines = |count: Option<u32>| count.unwrap_or(0);
    (!hunks.is_empty()).then(|| PatchSize {
        changed_lines: lines(read.file.additions).saturating_add(lines(read.file.deletions)),
        bytes: read.patch.map_or(0, |patch| patch.len),
    })
}

/// One file as the detail carries it and as the cache keeps it beside: a file
/// with hunks that the budgets did not make eager is withheld, and its hunks
/// stay in the cache.
fn keep(read: ReadFile, eager: bool) -> (DiffFile, CachedFile) {
    let ReadFile {
        mut file,
        patch,
        blob_sha,
    } = read;
    let withheld = match file.content {
        DiffContent::Hunks { ref mut hunks } if !eager && !hunks.is_empty() => {
            let hunks = std::mem::take(hunks);
            file.content = DiffContent::Withheld;
            Some(hunks)
        }
        _ => None,
    };
    let cached = CachedFile {
        withheld,
        patch,
        blob_sha,
    };
    (file, cached)
}

/// The detail of `parts`, read through `row` at `read_at`, with its files
/// under the line and byte budgets, and beside each file what the cache keeps
/// of it: a withheld file's hunks, and its review-key material.
pub(crate) fn assemble(
    reference: PullRequestReference,
    row: PullRequestSummary,
    parts: ReadParts,
    read_at: u64,
) -> (PullRequestDetail, Vec<CachedFile>) {
    let eager = eager_files(parts.files.iter().map(patch_size));
    let (files, cached): (Vec<DiffFile>, Vec<CachedFile>) = parts
        .files
        .into_iter()
        .zip(eager)
        .map(|(read, eager)| keep(read, eager))
        .unzip();
    let detail = PullRequestDetail {
        reference,
        row,
        head_branch: parts.head_branch,
        base_branch: parts.base_branch,
        head_commit: parts.head_commit,
        base_commit: parts.base_commit,
        author: parts.author,
        description: parts.description,
        conversation: parts.conversation,
        checks: parts.checks,
        threads: parts.threads,
        files,
        unlisted_files: parts.unlisted_files,
        read_at_unix: read_at,
        no_longer_listed: false,
    };
    (detail, cached)
}

// ---- links out ----

/// Whether `href` may go to the platform opener (`pull-request-viewer`:
/// *Desktop Link Opener*; design D10): an absolute `http` or `https` URL with
/// a host, as a URI parser reads it, holding no whitespace, control or
/// default-ignorable character anywhere — its fragment included, which the
/// parser leaves unread — so it reads the same to every parser on its way to
/// the browser. A `file:`, `javascript:`, `data:` or `mailto:` href fails, as
/// does every custom scheme and a relative link. Its form is all this reads:
/// it never fetches the href, nor looks for it in the pull request's content.
pub(crate) fn openable_link(href: &str) -> bool {
    if href
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || is_default_ignorable(c))
    {
        return false;
    }
    // `ureq`'s re-export of the `http` crate's parser, which knows `http` and
    // `https` in any case and lowercases them.
    let Ok(uri) = href.parse::<ureq::http::Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("http" | "https"))
        && uri.host().is_some_and(|host| !host.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_requests::PullRequestsStatus;
    use serde_json::json;

    fn reference(provider: PullRequestProvider, owner: &str, repo: &str) -> PullRequestReference {
        PullRequestReference {
            provider,
            owner: owner.to_string(),
            repo: repo.to_string(),
            number: 42,
        }
    }

    fn github(owner: &str, repo: &str) -> PullRequestReference {
        reference(PullRequestProvider::Github, owner, repo)
    }

    fn row(repo_full_name: &str, id: u64, url: &str) -> PullRequestSummary {
        PullRequestSummary {
            id,
            title: format!("PR {id}"),
            repo_full_name: repo_full_name.to_string(),
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            url: url.to_string(),
            draft: false,
            updated_at_unix: 1_700_000_000,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: repo_full_name.to_string(),
        }
    }

    fn listed(repo_full_name: &str) -> PullRequestSummary {
        row(
            repo_full_name,
            42,
            &format!("https://github.com/{repo_full_name}/pull/42"),
        )
    }

    fn github_snapshot(
        authored: Vec<PullRequestSummary>,
        review_requested: Vec<PullRequestSummary>,
        stale: bool,
    ) -> GithubPullRequestsState {
        GithubPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale,
            fetched_at_unix: Some(1_700_000_000),
            authored,
            review_requested,
            withheld: 0,
        }
    }

    fn bitbucket_snapshot(pull_requests: Vec<PullRequestSummary>) -> BitbucketPullRequestsState {
        BitbucketPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: false,
            fetched_at_unix: Some(1_700_000_000),
            pull_requests,
            skipped_workspaces: Vec::new(),
        }
    }

    // ------------------------------------------------------------ equality

    #[test]
    fn references_are_equal_ignoring_ascii_case_in_owner_and_repository() {
        assert_eq!(github("ACME", "Api"), github("acme", "api"));
        assert_eq!(github("acme", "api"), github("acme", "api"));
    }

    /// Each part on its own tells two references apart.
    #[test]
    fn a_different_provider_number_owner_or_repository_is_another_reference() {
        let base = github("acme", "api");
        let other_provider = reference(PullRequestProvider::Bitbucket, "acme", "api");
        let other_number = PullRequestReference {
            number: 43,
            ..base.clone()
        };
        for other in [
            other_provider,
            other_number,
            github("acme-corp", "api"),
            github("acme", "api-v2"),
        ] {
            assert_ne!(base, other, "{other:?}");
        }
    }

    /// Only ASCII folds: `é` and `É` are different characters to GitHub and
    /// BitBucket alike.
    #[test]
    fn only_ascii_case_is_ignored() {
        assert_ne!(github("caf\u{e9}", "api"), github("CAF\u{c9}", "api"));
    }

    #[test]
    fn equal_references_share_one_lowercase_key() {
        let key = github("ACME", "Api").key();
        assert_eq!(key, github("acme", "api").key());
        assert_eq!(
            key,
            PullRequestKey {
                provider: PullRequestProvider::Github,
                owner: "acme".to_string(),
                repo: "api".to_string(),
                number: 42,
            }
        );
        assert_ne!(
            key,
            reference(PullRequestProvider::Bitbucket, "acme", "api").key()
        );
        assert_ne!(
            key,
            PullRequestReference {
                number: 7,
                ..github("acme", "api")
            }
            .key()
        );
    }

    // ------------------------------------------------------------ the wire

    /// The JSON `src/types.ts`'s `PullRequestReference` describes, in both
    /// directions: the frontend sends it as a command argument and reads it
    /// back in `review-progress-changed`.
    #[test]
    fn the_reference_crosses_the_wire_as_the_frontend_spells_it() {
        let wire = json!({ "provider": "github", "owner": "acme", "repo": "api", "number": 42 });
        assert_eq!(serde_json::to_value(github("acme", "api")).unwrap(), wire);
        let sent: PullRequestReference = serde_json::from_value(json!({
            "provider": "bitbucket", "owner": "acme", "repo": "api", "number": 7
        }))
        .unwrap();
        assert_eq!(sent.provider, PullRequestProvider::Bitbucket);
        assert_eq!((sent.owner.as_str(), sent.repo.as_str()), ("acme", "api"));
        assert_eq!(sent.number, 7);
    }

    /// No other provider, and no number that is not a non-negative integer,
    /// deserializes at all.
    #[test]
    fn a_malformed_reference_does_not_deserialize() {
        for wire in [
            json!({ "provider": "gitlab", "owner": "acme", "repo": "api", "number": 42 }),
            json!({ "provider": "github", "owner": "acme", "repo": "api", "number": -1 }),
            json!({ "provider": "github", "owner": "acme", "repo": "api", "number": 4.2 }),
            json!({ "provider": "github", "owner": "acme", "repo": "api" }),
        ] {
            assert!(
                serde_json::from_value::<PullRequestReference>(wire.clone()).is_err(),
                "{wire}"
            );
        }
    }

    // ------------------------------------------------------------ the lookup

    #[test]
    fn a_reference_names_the_row_of_its_repository_and_number() {
        let reference = github("ACME", "Api");
        assert!(reference.names(&listed("acme/api")));
        assert!(!reference.names(&row("acme/api", 43, "https://github.com/acme/api/pull/43")));
        assert!(!reference.names(&listed("acme/web")));
        assert!(!reference.names(&listed("other/api")));
        // The repository splits at its first slash only.
        assert!(!reference.names(&listed("acme/api/extra")));
        // A name with no owner half never splits into one.
        assert!(!reference.names(&row("acme-api", 42, "https://github.com/acme-api/pull/42")));
    }

    #[test]
    fn a_row_with_an_empty_url_never_matches() {
        let rows = vec![row("acme/api", 42, "")];
        assert!(!github("acme", "api").names(&rows[0]));
        assert_eq!(
            listed_row(
                &github("acme", "api"),
                &BitbucketPullRequestsState::disabled(),
                &github_snapshot(rows, Vec::new(), false),
            ),
            None
        );
    }

    #[test]
    fn a_stale_snapshot_still_matches() {
        let snapshot = github_snapshot(vec![listed("acme/api")], Vec::new(), true);
        assert_eq!(
            listed_row(
                &github("ACME", "Api"),
                &BitbucketPullRequestsState::disabled(),
                &snapshot,
            ),
            Some(listed("acme/api"))
        );
    }

    #[test]
    fn a_row_in_to_review_matches_like_one_in_yours() {
        let bitbucket = BitbucketPullRequestsState::disabled();
        let yours = github_snapshot(vec![listed("acme/api")], Vec::new(), false);
        let to_review = github_snapshot(Vec::new(), vec![listed("acme/api")], false);
        let reference = github("acme", "api");
        assert_eq!(
            listed_row(&reference, &bitbucket, &yours),
            Some(listed("acme/api"))
        );
        assert_eq!(
            listed_row(&reference, &bitbucket, &to_review),
            Some(listed("acme/api"))
        );
    }

    /// A reference is looked up in its own provider's snapshot only: the same
    /// repository and number listed by the other provider is not its row.
    #[test]
    fn each_provider_is_looked_up_in_its_own_snapshot() {
        let bitbucket_row = row(
            "acme/api",
            42,
            "https://bitbucket.org/acme/api/pull-requests/42",
        );
        let bitbucket = bitbucket_snapshot(vec![bitbucket_row.clone()]);
        let github_only = github_snapshot(vec![listed("acme/api")], Vec::new(), false);
        let none = GithubPullRequestsState::disabled();

        let on_bitbucket = reference(PullRequestProvider::Bitbucket, "acme", "api");
        assert_eq!(
            listed_row(&on_bitbucket, &bitbucket, &none),
            Some(bitbucket_row)
        );
        assert_eq!(
            listed_row(
                &on_bitbucket,
                &BitbucketPullRequestsState::disabled(),
                &github_only
            ),
            None
        );
        assert_eq!(listed_row(&github("acme", "api"), &bitbucket, &none), None);
    }

    #[test]
    fn a_row_is_listed_under_its_own_spelling() {
        let row = listed("Acme/Api");
        let reference = PullRequestReference::of_row(PullRequestProvider::Github, &row).unwrap();
        assert_eq!(
            (
                reference.owner.as_str(),
                reference.repo.as_str(),
                reference.number
            ),
            ("Acme", "Api", 42)
        );
        assert_eq!(reference.provider, PullRequestProvider::Github);
        assert!(reference.names(&row));
        assert_eq!(
            PullRequestReference::of_row(PullRequestProvider::Bitbucket, &listed("acme-api")),
            None
        );
    }

    // ------------------------------------------------------------ assembly

    use openspec_core::{FileStatus, Line, LineKind};

    const NOW: u64 = 1_800_000_000;

    fn added_lines(count: usize) -> Vec<Hunk> {
        vec![Hunk {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: count as u32,
            section: None,
            lines: (1..=count)
                .map(|n| Line {
                    kind: LineKind::Added,
                    old_no: None,
                    new_no: Some(n as u32),
                    text: format!("line {n}"),
                    no_newline: false,
                })
                .collect(),
        }]
    }

    /// A file as a recipe reads it: `added` and `removed` lines by its
    /// counts, its content, and `patch_bytes` of patch text.
    fn read_file(
        path: &str,
        added: u32,
        removed: u32,
        content: DiffContent,
        patch_bytes: usize,
    ) -> ReadFile {
        ReadFile {
            file: DiffFile {
                old_path: Some(path.to_string()),
                new_path: Some(path.to_string()),
                old_mode: None,
                new_mode: None,
                status: FileStatus::Modified,
                additions: Some(added),
                deletions: Some(removed),
                content,
            },
            patch: Some(PatchDigest::of(["+".repeat(patch_bytes).as_bytes()])),
            blob_sha: Some(format!("sha-{path}")),
        }
    }

    /// A patched file with `added` added lines and as many bytes.
    fn patched(path: &str, added: u32) -> ReadFile {
        read_file(
            path,
            added,
            0,
            DiffContent::Hunks {
                hunks: added_lines(added as usize),
            },
            added as usize,
        )
    }

    fn parts_with(files: Vec<ReadFile>) -> ReadParts {
        ReadParts {
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            head_commit: "head".to_string(),
            base_commit: "base".to_string(),
            author: Some("ada".to_string()),
            description: "Adds rate limits.".to_string(),
            conversation: Vec::new(),
            checks: Vec::new(),
            threads: Vec::new(),
            files,
            unlisted_files: 3,
        }
    }

    fn assembled(files: Vec<ReadFile>) -> (PullRequestDetail, Vec<CachedFile>) {
        assemble(
            github("acme", "api"),
            listed("acme/api"),
            parts_with(files),
            NOW,
        )
    }

    fn is_withheld(file: &DiffFile) -> bool {
        file.content == DiffContent::Withheld
    }

    /// The line rule withholds a file over 500 changed lines, counting its
    /// removed lines with its added ones; the file stays withheld in the
    /// detail and its hunks stay in the cache.
    #[test]
    fn a_file_the_budgets_hold_back_is_withheld_and_its_hunks_cached() {
        let (detail, cached) = assembled(vec![
            read_file(
                "even.rs",
                300,
                200,
                DiffContent::Hunks {
                    hunks: added_lines(1),
                },
                10,
            ),
            read_file(
                "over.rs",
                300,
                201,
                DiffContent::Hunks {
                    hunks: added_lines(2),
                },
                10,
            ),
        ]);
        assert!(!is_withheld(&detail.files[0]), "500 changed lines");
        assert_eq!(cached[0].withheld, None);
        assert!(is_withheld(&detail.files[1]), "501 changed lines");
        assert_eq!(cached[1].withheld, Some(added_lines(2)));
        assert_eq!(
            (detail.files[1].additions, detail.files[1].deletions),
            (Some(300), Some(201)),
            "a withheld file keeps its counts"
        );
    }

    /// The byte limit measures a file's patch text as received: past 64 KiB
    /// it is withheld, whatever its lines.
    #[test]
    fn a_file_whose_patch_text_passes_the_byte_limit_is_withheld() {
        let limit = openspec_core::diff::FILE_PATCH_BYTES_LIMIT;
        let hunks = || DiffContent::Hunks {
            hunks: added_lines(1),
        };
        let (detail, _) = assembled(vec![
            read_file("at.rs", 1, 0, hunks(), limit),
            read_file("past.rs", 1, 0, hunks(), limit + 1),
        ]);
        assert!(!is_withheld(&detail.files[0]));
        assert!(is_withheld(&detail.files[1]));
    }

    /// Only a file with hunks to show is ever withheld: a binary file, one
    /// too large, and one with no textual change keep their state past a
    /// spent line total, and add nothing to it.
    #[test]
    fn a_file_without_hunks_to_show_is_never_withheld() {
        let mut files: Vec<ReadFile> = (0..6)
            .map(|n| patched(&format!("full{n}.rs"), 500))
            .collect();
        files.push(read_file(
            "empty.rs",
            0,
            0,
            DiffContent::Hunks { hunks: Vec::new() },
            40,
        ));
        files.push(read_file("logo.png", 0, 0, DiffContent::Binary, 0));
        files.push(read_file("data.json", 4_000, 0, DiffContent::TooLarge, 0));
        files.push(patched("one-more.rs", 1));
        let (detail, cached) = assembled(files);
        assert!(
            detail.files[..6].iter().all(|file| !is_withheld(file)),
            "3,000 lines"
        );
        assert_eq!(
            detail.files[6].content,
            DiffContent::Hunks { hunks: Vec::new() }
        );
        assert_eq!(detail.files[7].content, DiffContent::Binary);
        assert_eq!(detail.files[8].content, DiffContent::TooLarge);
        assert!(is_withheld(&detail.files[9]), "the 3,001st line");
        assert!(cached[6..9].iter().all(|file| file.withheld.is_none()));
    }

    /// The detail carries the read's own fields, read through the row at its
    /// read time and listed; the cache keeps every file's patch and blob
    /// `sha` beside it, in order.
    #[test]
    fn the_detail_is_the_read_through_its_row_and_the_cache_keeps_the_key_material() {
        let (detail, cached) = assembled(vec![patched("a.rs", 2), patched("b.rs", 3)]);
        assert_eq!(detail.reference, github("acme", "api"));
        assert_eq!(detail.row, listed("acme/api"));
        assert_eq!(
            (detail.head_branch.as_str(), detail.base_branch.as_str()),
            ("feature", "main")
        );
        assert_eq!(
            (detail.head_commit.as_str(), detail.base_commit.as_str()),
            ("head", "base")
        );
        assert_eq!(detail.author.as_deref(), Some("ada"));
        assert_eq!(detail.description, "Adds rate limits.");
        assert_eq!(detail.unlisted_files, 3);
        assert_eq!(detail.read_at_unix, NOW);
        assert!(!detail.no_longer_listed);
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[1].patch, Some(PatchDigest::of([b"+++".as_slice()])));
        assert_eq!(cached[1].blob_sha.as_deref(), Some("sha-b.rs"));
    }

    /// A patch's digest is its bytes' length and SHA-256, taken in order as
    /// one stream: BitBucket's two sections of a folded type change digest as
    /// their concatenation, and in the other order as something else.
    #[test]
    fn a_patch_digest_is_its_bytes_length_and_hash_in_order() {
        let whole = PatchDigest::of([b"@@ -1 +1 @@\n-a\n+b\n".as_slice()]);
        let split = PatchDigest::of([b"@@ -1 +1 @@\n".as_slice(), b"-a\n+b\n"]);
        assert_eq!(split, whole);
        assert_eq!(whole.len, 18);
        let reordered = PatchDigest::of([b"-a\n+b\n".as_slice(), b"@@ -1 +1 @@\n"]);
        assert_eq!(reordered.len, 18);
        assert_ne!(reordered.sha256, whole.sha256);
        let empty = PatchDigest::of([]);
        assert_eq!(empty.len, 0);
        assert_ne!(empty.sha256, whole.sha256);
    }

    #[test]
    fn a_file_goes_by_its_new_path_or_once_deleted_its_old_one() {
        let mut file = patched("src/new.rs", 1).file;
        file.old_path = Some("src/old.rs".to_string());
        assert_eq!(file_path(&file), Some("src/new.rs"));
        file.new_path = None;
        assert_eq!(file_path(&file), Some("src/old.rs"));
        file.old_path = None;
        assert_eq!(file_path(&file), None);
    }

    // ------------------------------------------------------------ links out

    /// `pull-request-viewer`: *An external link opens in the system browser*
    /// and *Other schemes are refused*: an absolute `http` or `https` URL
    /// with a host passes, in any case and with any port, user, query or
    /// fragment; `javascript:`, `file:`, `data:`, `mailto:` and every custom
    /// scheme fail.
    #[test]
    fn only_an_absolute_http_or_https_link_with_a_host_opens() {
        for href in [
            "https://example.com/docs#setup",
            "http://example.com",
            "HTTPS://Example.com/a?b=c",
            "https://ada@example.com:8443/path",
            "https://[::1]/",
        ] {
            assert!(openable_link(href), "{href}");
        }
        for href in [
            "javascript:alert(1)",
            "JavaScript://example.com/%0Aalert(1)",
            "file:///etc/passwd",
            "file://host/share",
            "data:text/html,<script>alert(1)</script>",
            "mailto:ada@example.com",
            "vscode://file/etc/passwd",
            "slack://open",
            "ftp://example.com/",
            "http+unix://socket/",
        ] {
            assert!(!openable_link(href), "{href}");
        }
    }

    /// A hostless href fails, as does a relative one (*A relative link does
    /// not navigate*).
    #[test]
    fn a_hostless_or_relative_link_does_not_open() {
        for href in [
            "https://",
            "https:///path",
            "https:example.com",
            "https://:443/",
            "//example.com/x",
            "docs/setup.md",
            "/docs",
            "#section",
            "",
        ] {
            assert!(!openable_link(href), "{href:?}");
        }
    }

    /// One whitespace, control or default-ignorable character anywhere fails
    /// the link. Each of these the URI parser alone would pass, in a fragment
    /// it leaves unread or a path that may hold UTF-8, so each check is the
    /// one that stops its own.
    #[test]
    fn a_hidden_or_blank_character_anywhere_fails_the_link() {
        for href in [
            // Whitespace.
            "https://example.com/#a b",
            "https://example.com/\u{a0}",
            // Control characters.
            "https://example.com/#a\u{0}",
            "https://example.com/#\u{7f}",
            "https://example.com/\u{9b}x",
            // Default-ignorable characters.
            "https://example.com/\u{202e}gpj.exe",
            "https://example.com/#\u{200b}",
        ] {
            assert!(!openable_link(href), "{href:?}");
        }
        assert!(openable_link("https://example.com/caf\u{e9}#\u{e9}t\u{e9}"));
    }
}
