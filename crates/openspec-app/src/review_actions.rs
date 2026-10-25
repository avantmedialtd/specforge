//! Provider-neutral review-action wire types for desktop pull-request
//! reviews.
//!
//! Drafts, their items, operations, ledgers, receipts and the actor-neutral
//! vocabulary the composer renders live here, hand-mirrored in `src/types.ts`
//! and pinned in `crates/openspec-app/tests/wire_shape.rs`. The types carry
//! the persisted review content and no credential-derived value: an
//! authorisation stamp's fingerprint is kept beside these records by the
//! action store and never enters a wire type (see
//! `pull-request-actions`' *Persisted action state contains no credential*).

use crate::pull_request_detail::{DiffSide, PullRequestReference};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- capabilities

/// The five reviewer actions current state can offer, provider-neutrally:
/// decisions, and the two content kinds an item can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewCapabilityName {
    /// Submit the draft as a plain comment review, or only its content when
    /// no decision is permitted.
    Comment,
    /// Submit the draft as an approval.
    Approve,
    /// Submit the draft as requesting changes.
    RequestChanges,
    /// Place inline comments on current lines.
    InlineComment,
    /// Add replies to current replyable threads.
    Reply,
}

/// One action as the current state offers it: available, or unavailable with
/// the service's reason. The reason is `None` exactly when `available` is
/// true; the composer renders availability from this one record and never
/// re-derives provider rules from a provider name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCapability {
    pub name: ReviewCapabilityName,
    pub available: bool,
    /// Why the action is unavailable, in the service's own words; `None`
    /// while `available`.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------- drafts

/// The decision a draft would submit as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewDecision {
    Comment,
    Approve,
    RequestChanges,
}

/// The head and base commits a draft's placements are made under: the
/// authorisation gate compares a draft's basis against currently cached
/// commits, so a push makes it stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitBasis {
    pub head: String,
    pub base: String,
}

/// One ordered item of a draft. An item is either placed — under the draft's
/// current basis — or unplaced, carrying the anchor it formerly placed at
/// with the basis its placement was made under, until the reviewer
/// explicitly places it again. A reply names the stable provider
/// thread/comment target; nothing here accepts a range or a start line, and
/// every body is opaque markdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DraftItem {
    /// A single-line inline comment, placed on one line of both the
    /// diff-file's identity and its current line, in the draft's basis.
    PlacedInline {
        id: String,
        body: String,
        path: String,
        side: DiffSide,
        line: u32,
    },
    /// An inline comment that stale recovery turned unplaced: its body, the
    /// path, side and line it was placed at, and the commits that placement
    /// was made under. It cannot be submitted until the reviewer explicitly
    /// chooses one current line.
    UnplacedInline {
        id: String,
        body: String,
        path: String,
        side: DiffSide,
        line: u32,
        /// The commits the placement was made under, which stay with the
        /// item while the draft's basis moves on.
        basis: CommitBasis,
    },
    /// A reply to one current thread, by its stable provider target.
    PlacedReply {
        id: String,
        body: String,
        target: String,
    },
    /// A reply whose former target no longer exists, or is no longer
    /// replyable: its body and former target are retained, and it cannot be
    /// submitted until the reviewer explicitly chooses one current replyable
    /// thread.
    UnplacedReply {
        id: String,
        body: String,
        target: String,
    },
}

/// One review draft, the persisted content and what the composer edits.
/// Backend-generated UUID, monotonic revision, and the commit basis the
/// service authorised it under; the credential fingerprint and actor it was
/// authorised with stay with the action store alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDraft {
    /// Backend-generated UUID; at most one current draft exists for a
    /// canonical pull request.
    pub id: String,
    /// The pull request as the application spells it canonically.
    pub reference: PullRequestReference,
    /// Monotonically increasing with every accepted mutation; every
    /// read-modify-write names the revision its caller read.
    pub revision: u64,
    /// The head/base commits placements are made under.
    pub basis: CommitBasis,
    /// The decision a submission would carry.
    pub decision: ReviewDecision,
    /// Untrusted markdown, stored exactly as the reviewer entered it.
    pub summary: String,
    /// In creation order; the order a provider writes content in.
    pub items: Vec<DraftItem>,
}

// ---------------------------------------------------------------- blocked reasons

/// Why a review-action boundary refused, structured for the caller to word
/// and for tests to pin. `actionStoreCorrupt` blocks every draft or
/// operation mutation until the damaged file is repaired or explicitly
/// removed outside the action API; the rest name the authorisation gate's
/// failed checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockedReason {
    /// `pull-request-actions.json` is damaged and preserved; nothing starts
    /// empty and no provider mutation sends.
    ActionStoreCorrupt,
    /// The pull request left its provider's current snapshot.
    NotListed,
    /// No cached detail read under the current credential generation and
    /// fingerprint establishes the pull request's current commits.
    DetailRequired,
    /// A cached detail or the provider's commits changed against the basis
    /// the caller authorised under; only explicit recovery may move to them.
    StaleCommits,
    /// The only resolved credential is a legacy plaintext one, which reads
    /// but never authorises a review write.
    LegacyCredential,
    /// The resolved credential's fingerprint or generation differs from the
    /// recorded one; the draft or operation is fenced.
    CredentialChanged,
    /// The resolved credential's stable actor differs from the recorded one.
    ActorChanged,
    /// The requested decision or item type is not currently a capability.
    DecisionUnavailable,
}

// ---------------------------------------------------------------- operations

/// An operation's persisted phase. `running`, `attempting` and `reconciling`
/// are live and require the operation's execution lock; `paused` is
/// unfinished and permits no provider mutation without an explicit, eligible
/// Resume or Abandon; `submitted` and `abandoned` are terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationPhase {
    Running,
    Attempting,
    Reconciling,
    Paused,
    Submitted,
    Abandoned,
}

impl OperationPhase {
    /// The phases that require the operation's execution lock: a persisted
    /// document in one of them is either a live run or the residue of a dead
    /// one, which the native actions distinguish only under that lock.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Running | Self::Attempting | Self::Reconciling)
    }

    /// The terminal phases: the operation is over and its receipt tells what
    /// the provider holds of it.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Submitted | Self::Abandoned)
    }
}

/// What one ledger entry's write was. The four content writes an operation
/// performs, named provider-neutrally and identified by the frozen draft
/// item's UUID where it has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LedgerStep {
    /// One frozen inline comment, by its item UUID.
    InlineComment { item: String },
    /// One frozen reply, by its item UUID.
    Reply { item: String },
    /// The summary: GitHub submits it as its review's body, BitBucket posts
    /// it as a general comment.
    Summary,
    /// The selected decision: GitHub's submit event, or BitBucket's approve
    /// or request-changes endpoint, always the operation's last write.
    Decision,
}

/// What one write came to at the provider. A settled outcome is one of
/// these; no outcome alone grants a retry — the caller re-authorises and
/// reads provider state first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LedgerOutcome {
    /// The provider confirmed the write, whose stable id it carries when it
    /// returned one — GitHub a node id, BitBucket a comment id.
    Confirmed {
        /// The provider's stable id for the confirmed object, when it
        /// returned one.
        provider_id: Option<String>,
    },
    /// Attempted without proving whether it landed; only a bounded provider
    /// reconciliation read may decide it, and a blind repeat is never sent.
    Uncertain,
    /// The provider refused it or the run stopped on it — a permission
    /// failure, a stale anchor or another non-retryable boundary — worded in
    /// the service's reason for the reviewer.
    Failed { reason: String },
}

/// One write of an operation's ledger: what was attempted and what came of
/// it, in the operation's write order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub step: LedgerStep,
    pub outcome: LedgerOutcome,
}

/// The partial result a non-submitted operation's ledger reports: which
/// writes the provider confirmed and that remain, which cannot be proven
/// either way, and which failed or were stopped. It is derived from the
/// ledger, never a phase, and is never a permission to retry — its point
/// is exactly what must not be repeated or silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialResult {
    /// Writes the provider confirmed, and that remain there.
    pub succeeded: Vec<LedgerEntry>,
    /// Writes without proof either way; reconciliation must decide them.
    pub unresolved: Vec<LedgerEntry>,
    /// Writes refused or stopped as stale; never retried under the same
    /// credential by their run.
    pub failed: Vec<LedgerEntry>,
}

/// A terminal operation's outcome, kept so the reviewer can inspect every
/// confirmed or uncertain provider effect before anything is re-authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReceipt {
    /// The provider's stable id for the submitted review, when it supplied
    /// one: GitHub does, BitBucket posts its content as individual comments.
    pub review_id: Option<String>,
    /// The decision the provider confirmed, when one was applied.
    pub decision: Option<ReviewDecision>,
    /// A pending review that may still remain at the provider after cleanup
    /// was unauthorised, failed or remained uncertain, which the reviewer
    /// handles manually there. `None` when nothing may remain.
    pub remaining_pending_review: Option<String>,
}

/// One review operation as the windows see it: its phase, the ledger of its
/// writes so far, the partial result its ledger reports while unfinished,
/// and its receipt once terminal. Its frozen content stays with the store
/// and the operation's id; the entries carry the frozen item UUIDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOperationState {
    /// Backend-generated UUID.
    pub id: String,
    /// The exact persisted phase.
    pub phase: OperationPhase,
    /// One record per write, in write order.
    pub ledger: Vec<LedgerEntry>,
    /// The partial result yet to be accounted, until either phase that
    /// closes it.
    pub partial: Option<PartialResult>,
    /// Kept for `submitted` and `abandoned`.
    pub receipt: Option<ReviewReceipt>,
}

/// What the windows read of one pull request's review actions: its draft,
/// what the current state permits, and the record of any operation there.
/// Its blocked reason comes from the exact vocabulary
/// [`BlockedReason`] names (`pull-request-actions`: *Current State
/// Authorises Every Draft and Submission Boundary*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewActionState {
    /// The canonical pull-request reference.
    pub reference: PullRequestReference,
    /// The current draft — its UUID, revision, basis, decision, summary and
    /// ordered items — when one exists.
    pub draft: Option<ReviewDraft>,
    /// The five provider-neutral capabilities current state reports.
    pub capabilities: Vec<ReviewCapability>,
    /// The structured reason every review-action boundary currently blocks
    /// for, or `None` while the gate permits and the store is sound.
    pub blocked_reason: Option<BlockedReason>,
    /// The unfinished or last operation, exposed as any process may read it.
    pub operation: Option<ReviewOperationState>,
    /// The draft's basis no longer matches the cached detail's commits: the
    /// draft is stale and only explicit recovery moves it, never automatic
    /// remapping.
    pub stale_basis: bool,
}
