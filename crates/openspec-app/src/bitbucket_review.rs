//! The BitBucket review protocol, over the pull request's own comment and
//! approval endpoints (`pull-request-actions`: *BitBucket Submission Is
//! Sequential and Durably Partial*; design D6): the snapshot-derived paths,
//! the one-at-a-time content writes, the commit guard every write stands
//! behind, and the bounded comments read a lost write reconciles through.
//!
//! Every path is built from the trusted snapshot row's canonical
//! reference — never from a reply's link — and points only beneath
//! BitBucket's API, as 3.1's transport itself re-checks. A write carries
//! no redirect and no ambient proxy, and the credential rides in its
//! header alone.
//!
//! BitBucket has no pending review and no expected-head condition: a
//! content write is immediate. The module therefore refuses to send one
//! without reading the pull request immediately before it and proving
//! both remote commits are the frozen basis, and it reads them again
//! after every successful or ambiguous write before the caller may
//! continue. A before-read that proves another commit sends nothing; an
//! after-read that proves one reports the write as a confirmed or
//! uncertain partial effect of a stale basis; an after-read that proves
//! nothing leaves it unverified. In no case does this module repeat a
//! write by itself (`pull-request-actions`: *Interrupted Operations
//! Reconcile Without Automatic Mutation*).
//!
//! A body carries the same invisible operation/item markers the GitHub
//! protocol writes — BitBucket preserves raw markdown — so a write whose
//! answer was lost is reconciled by finding its one marker in a bounded
//! comments read: at most ten 100-object pages, each body within 3.1's
//! 10 MiB read bound. One marker is success, a duplicate or a reached
//! bound is paused, and absence counts only after the whole connection
//! was read within those bounds. A body's bytes travel no further than
//! the parser that reads them.

use serde_json::{json, Value};

use crate::bitbucket::{encode, BitbucketLimits, API_BASE};
use crate::github_review::{Page, PendingComment};
use crate::pull_request_detail::{DiffSide, PullRequestReference};
use crate::review_action_io::{
    page_objects, ActionFlow, ActionIo, ActionVerdict, BitbucketAuthority, BitbucketTransport,
    CredentialRefused,
};
use crate::review_actions::{BlockedReason, CommitBasis};

/// BitBucket's largest comment page, as every read asks it.
pub const COMMENT_PAGE_LEN: u32 = 100;

// ---- the paths ----

/// The pull request's own resource beneath BitBucket's API, from the
/// canonical reference the trusted snapshot row named: owner and
/// repository as path segments, the number as the last. Nothing else
/// spells a destination.
pub fn pull_request_url(reference: &PullRequestReference) -> String {
    format!(
        "{API_BASE}/repositories/{}/{}/pullrequests/{}",
        encode(&reference.owner),
        encode(&reference.repo),
        reference.number,
    )
}

/// The pull request's comments, one 100-object page at a time: the
/// write's own destination and the reconciliation read's first page.
pub fn comments_url(reference: &PullRequestReference) -> String {
    format!(
        "{}/comments?pagelen={COMMENT_PAGE_LEN}",
        pull_request_url(reference)
    )
}

// ---- the commit guard ----

/// What one read of the pull request's commits answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitRead {
    /// Both remote commits, as the resource named them.
    Read {
        /// The source branch's head commit.
        head: String,
        /// The destination branch's commit.
        base: String,
    },
    RateLimited {
        /// A delay the reply asked for; `None` leaves the provider's own
        /// existing default.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// The provider refused the credential: the run stops.
    Permission(CredentialRefused),
    /// The clearance refused: nothing was sent.
    Refused(BlockedReason),
    /// Anything else the reply or its body named: no proof of the
    /// commits.
    Unresolved,
}

impl CommitRead {
    /// Whether the read proves both remote commits are exactly `basis`.
    pub fn matches(&self, basis: &CommitBasis) -> bool {
        matches!(
            self,
            Self::Read { head, base } if head == &basis.head && base == &basis.base
        )
    }
}

fn parse_commits(flow: ActionFlow) -> CommitRead {
    match flow {
        ActionFlow::Sent(ActionVerdict::Read { body }) => parse_commits_data(&body),
        ActionFlow::Sent(ActionVerdict::RateLimited {
            delay_secs,
            secondary,
        }) => CommitRead::RateLimited {
            delay_secs,
            secondary,
        },
        ActionFlow::Sent(ActionVerdict::Permission { insufficient }) => {
            CommitRead::Permission(insufficient)
        }
        ActionFlow::Refused { reason } => CommitRead::Refused(reason),
        _ => CommitRead::Unresolved,
    }
}

fn parse_commits_data(body: &[u8]) -> CommitRead {
    let Ok(text) = std::str::from_utf8(body) else {
        return CommitRead::Unresolved;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return CommitRead::Unresolved;
    };
    let head = value.pointer("/source/commit/hash").and_then(Value::as_str);
    let base = value
        .pointer("/destination/commit/hash")
        .and_then(Value::as_str);
    match (head, base) {
        (Some(head), Some(base)) => CommitRead::Read {
            head: head.to_string(),
            base: base.to_string(),
        },
        _ => CommitRead::Unresolved,
    }
}

// ---- the comments read ----

/// What one page read of the pull request's comments answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentsRead {
    /// One 100-object page: each comment's stable id and body, and the
    /// next page's API link when the connection named one.
    Page(Page<PendingComment>),
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    Refused(BlockedReason),
    /// No whole page this read can structure: over its object bound, a
    /// comment without its id or with a body typed otherwise, or a
    /// continuation the credential may not follow.
    Unresolved,
}

fn parse_comments(flow: ActionFlow) -> CommentsRead {
    match flow {
        ActionFlow::Sent(ActionVerdict::Read { body }) => parse_comments_data(&body),
        ActionFlow::Sent(ActionVerdict::RateLimited {
            delay_secs,
            secondary,
        }) => CommentsRead::RateLimited {
            delay_secs,
            secondary,
        },
        ActionFlow::Sent(ActionVerdict::Permission { insufficient }) => {
            CommentsRead::Permission(insufficient)
        }
        ActionFlow::Refused { reason } => CommentsRead::Refused(reason),
        _ => CommentsRead::Unresolved,
    }
}

fn parse_comments_data(body: &[u8]) -> CommentsRead {
    let Ok(text) = std::str::from_utf8(body) else {
        return CommentsRead::Unresolved;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return CommentsRead::Unresolved;
    };
    let Some(values) = value.get("values").and_then(Value::as_array) else {
        return CommentsRead::Unresolved;
    };
    let Some(count) = page_objects(values.len()) else {
        return CommentsRead::Unresolved;
    };
    let mut nodes = Vec::with_capacity(count);
    for comment in values {
        let Some(id) = comment.get("id").and_then(Value::as_u64) else {
            return CommentsRead::Unresolved;
        };
        let body = match comment.pointer("/content/raw") {
            Some(Value::String(raw)) => raw.clone(),
            // A deleted comment may carry no content at all; no marker
            // can hide in what is not there.
            Some(Value::Null) | None => String::new(),
            _ => return CommentsRead::Unresolved,
        };
        nodes.push(PendingComment {
            id: id.to_string(),
            body,
        });
    }
    let next = match value.get("next") {
        Some(Value::Null) | None => None,
        // A continuation is followed only where the credential may go;
        // anything else makes the connection inconclusive, never absent.
        Some(Value::String(link)) if crate::bitbucket_detail::followable(link) => {
            Some(link.clone())
        }
        _ => return CommentsRead::Unresolved,
    };
    CommentsRead::Page(Page { nodes, next })
}

// ---- the content writes ----

/// One inline comment's write: the frozen path, one line of one side, and
/// the marker-bearing body. BitBucket anchors the new side with `to` and
/// the old with `from`, and a single line carries exactly one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineVariables {
    /// The file path, as the frozen draft named it.
    pub path: String,
    /// The one line the comment rides.
    pub line: u32,
    /// Which side of the diff the line is on.
    pub side: DiffSide,
    /// The provisional body the write carries.
    pub body: String,
}

impl From<&InlineVariables> for Value {
    fn from(variables: &InlineVariables) -> Self {
        let mut inline = serde_json::Map::new();
        inline.insert("path".to_string(), json!(variables.path));
        match variables.side {
            DiffSide::Old => {
                inline.insert("from".to_string(), json!(variables.line));
            }
            DiffSide::New => {
                inline.insert("to".to_string(), json!(variables.line));
            }
        }
        json!({ "content": { "raw": variables.body }, "inline": inline })
    }
}

/// One reply's write: the thread's root comment — a BitBucket comment id,
/// which its `parent` object takes as the number it is — and the
/// marker-bearing body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyVariables {
    /// The thread's root comment id, as the trusted detail named it.
    pub in_reply_to: u64,
    /// The provisional body the write carries.
    pub body: String,
}

impl From<&ReplyVariables> for Value {
    fn from(variables: &ReplyVariables) -> Self {
        json!({
            "content": { "raw": variables.body },
            "parent": { "id": variables.in_reply_to },
        })
    }
}

/// One summary's write: the review's only general comment, its
/// marker-bearing body and nothing else. The operation posts it only when
/// the frozen summary is non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryVariables {
    /// The provisional body the write carries.
    pub body: String,
}

impl From<&SummaryVariables> for Value {
    fn from(variables: &SummaryVariables) -> Self {
        json!({ "content": { "raw": variables.body } })
    }
}

/// What one content write's own answer named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The provider confirmed the comment and named its stable id.
    Written {
        /// The comment's stable id, the ledger records.
        comment_id: String,
    },
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    /// The provider refused the write's anchor as unprocessable.
    Stale,
    Refused(BlockedReason),
    /// The answer named no stable id: the write may have landed, and only
    /// a marker read may decide it.
    Unresolved,
}

impl WriteOutcome {
    /// Whether the write's own answer is one that may have landed, which
    /// the after-read must be made for.
    fn may_have_landed(&self) -> bool {
        matches!(self, Self::Written { .. } | Self::Unresolved)
    }
}

fn parse_written(flow: ActionFlow) -> WriteOutcome {
    match flow {
        ActionFlow::Sent(ActionVerdict::Read { body }) => parse_written_data(&body),
        ActionFlow::Sent(ActionVerdict::RateLimited {
            delay_secs,
            secondary,
        }) => WriteOutcome::RateLimited {
            delay_secs,
            secondary,
        },
        ActionFlow::Sent(ActionVerdict::Permission { insufficient }) => {
            WriteOutcome::Permission(insufficient)
        }
        ActionFlow::Sent(ActionVerdict::Stale) => WriteOutcome::Stale,
        ActionFlow::Refused { reason } => WriteOutcome::Refused(reason),
        _ => WriteOutcome::Unresolved,
    }
}

fn parse_written_data(body: &[u8]) -> WriteOutcome {
    let Ok(text) = std::str::from_utf8(body) else {
        return WriteOutcome::Unresolved;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return WriteOutcome::Unresolved;
    };
    match value.get("id").and_then(Value::as_u64) {
        Some(id) => WriteOutcome::Written {
            comment_id: id.to_string(),
        },
        None => WriteOutcome::Unresolved,
    }
}

/// What one guarded content write came to: the commit readings stand
/// around the write, and every write that may have landed carries the
/// reading its caller must weigh before anything follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteRun {
    /// The before-read proved the frozen basis, and the write — or the
    /// after-read of one that may have landed — proved it again. `outcome`
    /// is what the write's own answer named.
    Sent(WriteOutcome),
    /// Nothing was sent: the before-read did not prove the frozen basis,
    /// and [`CommitRead`] carries exactly what it answered.
    NotSent(CommitRead),
    /// The write may have landed, and the after-read proved a commit
    /// other than the frozen basis: a partial effect of a stale basis,
    /// confirmed or uncertain as `outcome` says.
    BasisMoved {
        outcome: WriteOutcome,
        /// The after-read, proved different.
        after: CommitRead,
    },
    /// The write may have landed, and the after-read could not prove the
    /// commits either way: the effect stands unverified, and nothing
    /// further may follow this run.
    Unverified {
        outcome: WriteOutcome,
        /// The after-read, proved nothing.
        after: CommitRead,
    },
}

// ---- the sender ----

/// One BitBucket review step's sender: 3.1's transport, whose every
/// request is cleared, admitted, re-checked and sent under the discipline
/// it owns, and nothing here may reconstruct (`crate::review_action_io`).
pub struct BitbucketReviewSender<'a> {
    sender: BitbucketTransport<'a>,
}

impl<'a> BitbucketReviewSender<'a> {
    /// One sender over `limits` and `io`, whose requests `authority`
    /// clears exactly as 3.1's transport does, while `enabled` holds.
    pub fn new(
        limits: &'a BitbucketLimits,
        io: &'a dyn ActionIo,
        enabled: &'a dyn Fn() -> bool,
        authority: &'a dyn BitbucketAuthority,
    ) -> Self {
        Self {
            sender: BitbucketTransport::new(limits, io, enabled, authority),
        }
    }

    /// One read of the pull request's two remote commits: the guard's own
    /// ask, made immediately before every write and again after every
    /// write that may have landed.
    pub fn commits(&self, reference: &PullRequestReference) -> CommitRead {
        parse_commits(self.sender.get(&pull_request_url(reference)))
    }

    /// One page of the pull request's comments: the first page from the
    /// reference's own path, a later one only from the page link the
    /// previous reply named.
    pub fn comment_page(
        &self,
        reference: &PullRequestReference,
        after: Option<&str>,
    ) -> CommentsRead {
        let url = match after {
            Some(link) => link.to_string(),
            None => comments_url(reference),
        };
        parse_comments(self.sender.get(&url))
    }

    /// One inline comment, behind the commit guard: nothing sends unless
    /// both remote commits are the frozen basis immediately before it,
    /// and the after-read stands over any answer that may have landed.
    pub fn write_inline(
        &self,
        reference: &PullRequestReference,
        basis: &CommitBasis,
        variables: &InlineVariables,
    ) -> WriteRun {
        self.guarded(reference, basis, || {
            self.sender
                .post(&comments_url(reference), Value::from(variables).to_string())
        })
    }

    /// One reply, behind the same guard.
    pub fn write_reply(
        &self,
        reference: &PullRequestReference,
        basis: &CommitBasis,
        variables: &ReplyVariables,
    ) -> WriteRun {
        self.guarded(reference, basis, || {
            self.sender
                .post(&comments_url(reference), Value::from(variables).to_string())
        })
    }

    /// The non-empty summary, behind the same guard: one general comment
    /// with no anchor and no parent.
    pub fn write_summary(
        &self,
        reference: &PullRequestReference,
        basis: &CommitBasis,
        variables: &SummaryVariables,
    ) -> WriteRun {
        self.guarded(reference, basis, || {
            self.sender
                .post(&comments_url(reference), Value::from(variables).to_string())
        })
    }

    /// The read-write-read discipline every content write shares.
    fn guarded<W>(
        &self,
        reference: &PullRequestReference,
        basis: &CommitBasis,
        write: W,
    ) -> WriteRun
    where
        W: FnOnce() -> ActionFlow,
    {
        let before = self.commits(reference);
        if !before.matches(basis) {
            // Another commit — or no proof of one — stops before the
            // write: nothing is sent under a basis no longer read.
            return WriteRun::NotSent(before);
        }
        let outcome = parse_written(write());
        if !outcome.may_have_landed() {
            return WriteRun::Sent(outcome);
        }
        let after = self.commits(reference);
        if after.matches(basis) {
            return WriteRun::Sent(outcome);
        }
        if matches!(after, CommitRead::Read { .. }) {
            WriteRun::BasisMoved { outcome, after }
        } else {
            WriteRun::Unverified { outcome, after }
        }
    }
}

#[cfg(test)]
mod tests;
