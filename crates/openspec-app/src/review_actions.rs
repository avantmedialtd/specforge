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
//!
//! The module also carries the current-state authorisation gate (below):
//! non-serialisable vocabulary the service assembles and four boundary
//! entry points every draft and operation boundary runs through. Neither
//! the gate nor a wire type returns a credential, token or fingerprint
//! beyond the one-way digest an action stamp records.

use crate::credential_store::{CredentialSource, ResolvedCredentialContext};
use crate::pull_request_detail::{file_path, DiffSide, PullRequestDetail, PullRequestReference};
use crate::review_action_store::ActionAuthorisationStamp;
use openspec_core::{DiffContent, DiffFile};
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

// ------------------------------------------------------- operation order

/// The canonical write order of one frozen draft: its placed items in
/// frozen order, the summary when it is non-empty, and the decision when
/// the provider writes one as a step of its own. GitHub submits every
/// decision — Comment included — as its review's event, so
/// `comment_is_write` is true there; BitBucket's Comment has no endpoint of
/// its own, so it is false and its content writes are the operation.
pub(crate) fn operation_steps(draft: &ReviewDraft, comment_is_write: bool) -> Vec<LedgerStep> {
    let mut steps = Vec::new();
    for item in &draft.items {
        match item {
            DraftItem::PlacedInline { id, .. } => {
                steps.push(LedgerStep::InlineComment { item: id.clone() })
            }
            DraftItem::PlacedReply { id, .. } => steps.push(LedgerStep::Reply { item: id.clone() }),
            // Freezing refuses an unplaced item; one persisted by an older
            // build names no write this order may invent.
            DraftItem::UnplacedInline { .. } | DraftItem::UnplacedReply { .. } => {}
        }
    }
    if !draft.summary.trim().is_empty() {
        steps.push(LedgerStep::Summary);
    }
    if comment_is_write || draft.decision != ReviewDecision::Comment {
        steps.push(LedgerStep::Decision);
    }
    steps
}

/// The ledger's settled and unsettled writes, in the order they were
/// journalled. A non-submitted operation reports all three: what the
/// provider confirmed, what no read has decided, and what failed.
pub(crate) fn partial_result(ledger: &[LedgerEntry]) -> PartialResult {
    let mut result = PartialResult {
        succeeded: Vec::new(),
        unresolved: Vec::new(),
        failed: Vec::new(),
    };
    for entry in ledger {
        match entry.outcome {
            LedgerOutcome::Confirmed { .. } => result.succeeded.push(entry.clone()),
            LedgerOutcome::Uncertain => result.unresolved.push(entry.clone()),
            LedgerOutcome::Failed { .. } => result.failed.push(entry.clone()),
        }
    }
    result
}

/// Where an explicit Resume starts: the first canonical step without a
/// confirmed entry, provided the attempt a previous run left is eligible
/// for another one. `None` means every step is confirmed. A step carrying
/// an uncertain outcome must first be reconciled to confirmation or a
/// proved absence, and a failed step is never retried under the same
/// credential; both refuse with the reviewer's sentence.
pub(crate) fn resume_step(
    steps: &[LedgerStep],
    ledger: &[LedgerEntry],
) -> Result<Option<LedgerStep>, String> {
    for step in steps {
        match ledger.iter().find(|entry| entry.step == *step) {
            Some(LedgerEntry {
                outcome: LedgerOutcome::Confirmed { .. },
                ..
            }) => continue,
            Some(LedgerEntry {
                outcome: LedgerOutcome::Uncertain,
                ..
            }) => return Err(
                "the first unresolved step has not been reconciled; read the action state again"
                    .to_string(),
            ),
            Some(LedgerEntry {
                outcome: LedgerOutcome::Failed { reason },
                ..
            }) => {
                return Err(format!(
                    "the first unresolved step failed and is not retried: {reason}"
                ))
            }
            None => return Ok(Some(step.clone())),
        }
    }
    Ok(None)
}

/// The five actions current state offers, each with its availability and,
/// when unavailable, the gate's own sentence. The gate is asked exactly as
/// a creation boundary would ask it, so a capability shown here is one an
/// edit may act on; a draft's own fenced stamp may still refuse its edits.
pub(crate) fn current_capabilities(state: &CurrentState) -> Vec<ReviewCapability> {
    const NAMES: [ReviewCapabilityName; 5] = [
        ReviewCapabilityName::Comment,
        ReviewCapabilityName::Approve,
        ReviewCapabilityName::RequestChanges,
        ReviewCapabilityName::InlineComment,
        ReviewCapabilityName::Reply,
    ];
    NAMES
        .iter()
        .map(|&name| {
            let wanted = [name];
            match gate(
                state,
                None,
                CommitRequirement::Record,
                CapabilityRequirement::Check(&wanted),
            ) {
                Ok(_) => ReviewCapability {
                    name,
                    available: true,
                    reason: None,
                },
                Err(reason) => ReviewCapability {
                    name,
                    available: false,
                    reason: Some(worded(reason)),
                },
            }
        })
        .collect()
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

// ------------------------------------------------------- the authorisation gate
//
// The service assembles [`CurrentState`] and passes it to the boundary
// entry points below; the gate is deliberately a pure fn of that input,
// because every draft or operation boundary runs through one call of it
// (`pull-request-actions`: *Current State Authorises Every Draft and
// Submission Boundary*). None of it serialises: no wire type below carries
// serde derives, and a resolved credential shows up only as its source and
// the fingerprint's one-way digest — exactly the facts an
// [`ActionAuthorisationStamp`] records. The provider-facing facts enter as
// neutral vocabulary: both providers' snapshot handles assemble
// [`SnapshotContext`] from their own private identity contexts, so the gate
// stays free of provider names.

/// One freshly resolved credential's action-relevant facts: where it came
/// from and the digest spelling of its stable fingerprint. The service
/// resolves it durably — transaction recovery first, under the settings
/// lock — and never from a process-local shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCredential {
    /// Where the credential resolved from. A resolution always carries one
    /// of [`CredentialSource::Environment`], [`CredentialSource::Secure`]
    /// or [`CredentialSource::Legacy`].
    pub source: CredentialSource,
    /// The credential fingerprint's digest, as 64 lowercase hex digits —
    /// the fingerprint spelling a stamp records.
    pub fingerprint: String,
}

impl ResolvedCredential {
    /// The crate's resolved credential context as these two facts. The
    /// context type is crate-internal, so this is the only way current
    /// state is assembled: no other spelling of a resolution enters the
    /// gate.
    pub(crate) fn from_context(context: &ResolvedCredentialContext) -> Self {
        Self {
            source: context.source(),
            fingerprint: context.fingerprint().action_digest(),
        }
    }
}

/// Everything one authorisation-boundary call sees of current state, which
/// the service assembles and holds no lock while the gate reads it:
/// freshly resolved credential, the provider's enabled flag and snapshot's
/// identity context, and the cached detail's commits under that
/// fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentState {
    /// The resolution this boundary starts from.
    pub resolved: Option<ResolvedCredential>,
    /// Whether the provider is enabled at this instant. Disabled providers
    /// keep no current snapshot, so both answers deny.
    pub enabled: bool,
    /// The provider snapshot's identity context, with the row the canonical
    /// reference names. `None` when the snapshot is stale or read under no
    /// credential: no current state lists anything.
    pub snapshot: Option<SnapshotContext>,
    /// The cached detail's head and base commits, when the cache still
    /// holds a detail read under the current credential generation and
    /// fingerprint. `detailRequired` blocks every boundary this gap
    /// reaches.
    pub commits: Option<CommitBasis>,
}

/// One provider snapshot's identity context, assembled by the provider
/// module so the gate derives every capability from it provider-neutrally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotContext {
    /// The fingerprint digest the current rows were read under. One read
    /// under an earlier credential can never combine with the replacement
    /// credential's actor: the gate refuses it as `credentialChanged`.
    pub fingerprint: String,
    /// The snapshot's stable authenticated actor id — GitHub's viewer node
    /// id, BitBucket's account uuid or account id. `None` fails closed,
    /// per provider: no row of this snapshot is actionable.
    pub actor: Option<String>,
    /// The row the canonical reference names, as the snapshot lists it:
    /// `None` when nothing lists the pull request, and [`SnapshotRow`]
    /// when it is listed — trusted or not.
    pub row: Option<SnapshotRow>,
}

/// A listed row's trust in the snapshot that listed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotRow {
    /// The row carries every trusted remote identifier its provider needs
    /// to act on it.
    Trusted {
        provenance: RowProvenance,
        /// The row author's stable id, when the provider carries one.
        author: Option<String>,
    },
    /// The row is listed, but its provider's trusted remote identifiers are
    /// not established for it: readable, and actionable by nothing.
    Untrusted,
}

/// Whether a listed row is open to review by the actor, or is the actor's
/// own authorship — the fold-in of the decision rules' self-review limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowProvenance {
    /// Open for review; the row's author is known and comparable.
    ReviewRequested,
    /// Authored by the credential's own account: comment only.
    Authored,
}

/// What a granted boundary carries: the single stamp of authorisation
/// current state supplied, and the commit basis the boundary operates
/// under — the required one for a fenced boundary, the freshly recorded one
/// for a creation's or recovery's revision. A reconciliation read asserts
/// nothing about commits and carries none here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorisationGranted {
    /// The stamp: the recorded one for every boundary but creation and
    /// Start over, whose fresh stamp derives it.
    pub stamp: ActionAuthorisationStamp,
    /// The commit basis this boundary's revision is written with.
    pub basis: Option<CommitBasis>,
}

/// A draft's creation or Start over's boundary: everything must be current,
/// and the wanted actions must be the current capabilities. No stamp is
/// consulted — Start over exists to discard a fenced draft's stamp and
/// re-derive a new one; the granted basis is the current one.
pub fn authorise_creation(
    state: &CurrentState,
    wanted: &[ReviewCapabilityName],
) -> Result<AuthorisationGranted, BlockedReason> {
    gate(
        state,
        None,
        CommitRequirement::Record,
        CapabilityRequirement::Check(wanted),
    )
}

/// A boundary already fenced by a stamp — an ordinary edit, freeze,
/// initial submission, resume or provider cleanup: everything must be
/// current AND the stamp must still match the fingerprint, the actor and
/// the recorded commits exactly.
pub fn authorise_bound(
    state: &CurrentState,
    stamp: &ActionAuthorisationStamp,
    required: &CommitBasis,
    wanted: &[ReviewCapabilityName],
) -> Result<AuthorisationGranted, BlockedReason> {
    gate(
        state,
        Some(stamp),
        CommitRequirement::Equal(required),
        CapabilityRequirement::Check(wanted),
    )
}

/// Explicit stale recovery: identity only. The stamp's fingerprint and
/// actor must still match, but the commits may have moved on — the gate
/// hands back the current basis the recovered revision will record, with
/// the wanted capabilities checked like every other acting boundary's.
pub fn authorise_stale_recovery(
    state: &CurrentState,
    stamp: &ActionAuthorisationStamp,
    wanted: &[ReviewCapabilityName],
) -> Result<AuthorisationGranted, BlockedReason> {
    gate(
        state,
        Some(stamp),
        CommitRequirement::Record,
        CapabilityRequirement::Check(wanted),
    )
}

/// One provider request's immediate clearance: identity only. The run's
/// acting gate already asserted commits and capabilities at its start, and
/// the provider protocol checks commits itself wherever it writes; this is
/// what every request in a run repeats — the bare forbiddances that must
/// never slip past: a fresh environment/secure resolution, the enabled
/// flag, the current snapshot read under that fingerprint, a listed trusted
/// row, and the operation stamp's fingerprint and actor.
pub fn authorise_request(
    state: &CurrentState,
    stamp: &ActionAuthorisationStamp,
) -> Result<AuthorisationGranted, BlockedReason> {
    gate(
        state,
        Some(stamp),
        CommitRequirement::None,
        CapabilityRequirement::Unchecked,
    )
}

/// A read-only reconciliation boundary: the provider must still be enabled
/// and the pull request listed under the stamped credential and actor, but
/// neither commits nor capabilities are asserted — reconciliation exists to
/// classify a write that raced either change, and its reads never move the
/// operation into `running`.
pub fn authorise_reconciliation(
    state: &CurrentState,
    stamp: &ActionAuthorisationStamp,
) -> Result<AuthorisationGranted, BlockedReason> {
    authorise_request(state, stamp)
}

/// What a boundary asserts about commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitRequirement<'a> {
    /// The recorded basis must still be the current one.
    Equal(&'a CommitBasis),
    /// No equality assertion, and the current basis is handed back.
    Record,
    /// A read-only boundary asserts nothing, and hands nothing back.
    None,
}

/// What a boundary asserts about capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityRequirement<'a> {
    /// The wanted actions are the current capabilities.
    Check(&'a [ReviewCapabilityName]),
    /// A reconciliation read asserts nothing about them.
    Unchecked,
}

fn gate(
    state: &CurrentState,
    pinned: Option<&ActionAuthorisationStamp>,
    commits: CommitRequirement<'_>,
    capabilities: CapabilityRequirement<'_>,
) -> Result<AuthorisationGranted, BlockedReason> {
    // 1. A fresh resolution must yield an environment or secure-store
    //    credential. A legacy plaintext credential keeps reads working but
    //    authorises no review write, and nothing resolved is nothing to
    //    authorise with; this outranks every other failure, whatever other
    //    state has gone stale meanwhile.
    let resolved = state
        .resolved
        .as_ref()
        .filter(|resolved| {
            matches!(
                resolved.source,
                CredentialSource::Environment | CredentialSource::Secure
            )
        })
        .ok_or(BlockedReason::LegacyCredential)?;
    let fingerprint = resolved.fingerprint.clone();

    // 2. The provider must be enabled, with a snapshot whose current rows
    //    were read under this same fingerprint. A snapshot read under an
    //    earlier credential is not current and never combines with the
    //    replacement credential's actor; `None` — stale, or read under no
    //    credential — lists nothing.
    let snapshot = state
        .snapshot
        .as_ref()
        .filter(|_| state.enabled)
        .ok_or(BlockedReason::NotListed)?;
    if snapshot.fingerprint != fingerprint {
        return Err(BlockedReason::CredentialChanged);
    }

    // 3. The pull request must still be listed in that snapshot.
    let row = snapshot.row.as_ref().ok_or(BlockedReason::NotListed)?;

    // 4. Identity: a fenced boundary's stamp must still be the current
    //    one — its fingerprint matching the fresh resolution (a saved or
    //    replaced credential changes it under either snapshot) and its
    //    actor matching the stable actor the current snapshot still names,
    //    which `None` matches no stamp of.
    if let Some(recorded) = pinned {
        if recorded.fingerprint != fingerprint {
            return Err(BlockedReason::CredentialChanged);
        }
        if snapshot.actor.as_deref() != Some(recorded.actor.as_str()) {
            return Err(BlockedReason::ActorChanged);
        }
    }

    // 5. Commits, exactly as this boundary asserts them.
    let basis = match commits {
        CommitRequirement::None => None,
        CommitRequirement::Record => {
            Some(state.commits.clone().ok_or(BlockedReason::DetailRequired)?)
        }
        CommitRequirement::Equal(required) => {
            let current = state
                .commits
                .as_ref()
                .ok_or(BlockedReason::DetailRequired)?;
            if *current != *required {
                return Err(BlockedReason::StaleCommits);
            }
            Some(current.clone())
        }
    };

    // The stamp every granted boundary carries: a created or Start-over
    // revision derives it from this gate's resolution and the snapshot's
    // stable actor; a fenced boundary carries its recorded stamp again.
    let stamp = match pinned {
        Some(recorded) => recorded.clone(),
        None => ActionAuthorisationStamp {
            fingerprint,
            actor: snapshot
                .actor
                .clone()
                .ok_or(BlockedReason::DecisionUnavailable)?,
        },
    };

    // 6. Capabilities, exactly as this boundary asks for them.
    if let CapabilityRequirement::Check(wanted) = capabilities {
        let Some((provenance, author)) = (match row {
            SnapshotRow::Trusted { provenance, author } => Some((*provenance, author.clone())),
            SnapshotRow::Untrusted => None,
        }) else {
            return Err(BlockedReason::DecisionUnavailable);
        };
        // Whether this row is the actor's own authorship: an authored row
        // always is, and a review-requested row whose author cannot be
        // proved to be someone else runs the same comment-only rules rather
        // than letting an approval slip past unverified.
        let own = match provenance {
            RowProvenance::Authored => true,
            RowProvenance::ReviewRequested => match author.as_deref() {
                None => true,
                Some(author) => author == stamp.actor.as_str(),
            },
        };
        for name in wanted {
            match name {
                ReviewCapabilityName::Approve | ReviewCapabilityName::RequestChanges => {
                    if own {
                        return Err(BlockedReason::DecisionUnavailable);
                    }
                }
                // Commentary and the two content kinds record no decision:
                // an own-authorship row offers them all the same.
                ReviewCapabilityName::Comment
                | ReviewCapabilityName::InlineComment
                | ReviewCapabilityName::Reply => {}
            }
        }
    }

    Ok(AuthorisationGranted { stamp, basis })
}

#[cfg(test)]
mod authorisation_tests {
    use super::*;
    use crate::credential_store::{BitbucketCredential, GithubCredential};

    /// A stable author id different from the actor every fixture acts as.
    const OTHER_AUTHOR: &str = "U_author9";

    fn github_resolved(token: &str) -> ResolvedCredential {
        ResolvedCredential::from_context(
            &GithubCredential::new(token.to_string(), CredentialSource::Secure)
                .context()
                .clone(),
        )
    }

    fn github_resolved_legacy() -> ResolvedCredential {
        ResolvedCredential::from_context(
            &GithubCredential::new(String::from("tok-plain"), CredentialSource::Legacy)
                .context()
                .clone(),
        )
    }

    fn bitbucket_resolved(username: &str, token: &str) -> ResolvedCredential {
        ResolvedCredential::from_context(
            &BitbucketCredential::new(
                username.to_string(),
                token.to_string(),
                CredentialSource::Secure,
            )
            .context()
            .clone(),
        )
    }

    fn basis() -> CommitBasis {
        CommitBasis {
            head: "c".repeat(40),
            base: "b".repeat(40),
        }
    }

    fn other_basis() -> CommitBasis {
        CommitBasis {
            head: "9".repeat(40),
            base: "b".repeat(40),
        }
    }

    fn state(
        resolved: Option<ResolvedCredential>,
        snapshot: Option<SnapshotContext>,
        commits: Option<CommitBasis>,
    ) -> CurrentState {
        CurrentState {
            resolved,
            enabled: true,
            snapshot,
            commits,
        }
    }

    /// A current snapshot freshly read under `resolved`: one stable actor,
    /// and `row` as listed for the canonical reference.
    fn current_snapshot(
        resolved: &ResolvedCredential,
        actor: Option<&str>,
        row: Option<SnapshotRow>,
    ) -> SnapshotContext {
        SnapshotContext {
            fingerprint: resolved.fingerprint.clone(),
            actor: actor.map(str::to_string),
            row,
        }
    }

    fn trusted_review_requested(author: Option<&str>) -> SnapshotRow {
        SnapshotRow::Trusted {
            provenance: RowProvenance::ReviewRequested,
            author: author.map(str::to_string),
        }
    }

    fn stamp_of(resolved: &ResolvedCredential, actor: &str) -> ActionAuthorisationStamp {
        ActionAuthorisationStamp {
            fingerprint: resolved.fingerprint.clone(),
            actor: actor.to_string(),
        }
    }

    /// Everything current: a secure credential, an enabled provider, a
    /// snapshot read under it with a trusted review-requested row by
    /// another author, and cached commits.
    fn all_current(
        token: &str,
        actor: &str,
        author: Option<&str>,
    ) -> (ResolvedCredential, CurrentState) {
        let resolved = github_resolved(token);
        let snapshot = current_snapshot(
            &resolved,
            Some(actor),
            Some(trusted_review_requested(author)),
        );
        (
            resolved.clone(),
            state(Some(resolved), Some(snapshot), Some(basis())),
        )
    }

    const COMMENT: [ReviewCapabilityName; 1] = [ReviewCapabilityName::Comment];
    const CONTENT: [ReviewCapabilityName; 3] = [
        ReviewCapabilityName::Comment,
        ReviewCapabilityName::InlineComment,
        ReviewCapabilityName::Reply,
    ];
    const APPROVE: [ReviewCapabilityName; 1] = [ReviewCapabilityName::Approve];

    #[test]
    fn creation_is_authorised_with_current_state_and_derives_a_new_stamp() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let granted = authorise_creation(&current, &CONTENT).unwrap();
        assert_eq!(granted.stamp.actor, "U_viewer1".to_string());
        assert_eq!(
            granted.stamp.fingerprint,
            github_resolved("tok-1").fingerprint
        );
        assert_eq!(granted.stamp.fingerprint.len(), 64);
        assert!(
            granted.stamp.fingerprint.chars().all(|character| {
                character.is_ascii_digit() || ('a'..='f').contains(&character)
            }),
            "the digest spelling a stamp persists is 64 lowercase hex digits"
        );
        assert_eq!(granted.basis, Some(basis()));
    }

    #[test]
    fn a_legacy_credential_reads_but_never_authorises() {
        let legacy = github_resolved_legacy();
        let current = state(
            Some(legacy),
            Some(SnapshotContext {
                fingerprint: "a".repeat(64),
                actor: Some("U_viewer1".to_string()),
                row: Some(trusted_review_requested(Some(OTHER_AUTHOR))),
            }),
            Some(basis()),
        );
        assert_eq!(
            authorise_creation(&current, &COMMENT),
            Err(BlockedReason::LegacyCredential)
        );
    }

    #[test]
    fn no_resolved_credential_cannot_authorise_either() {
        let current = state(None, None, None);
        assert_eq!(
            authorise_bound(
                &current,
                &stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                &basis(),
                &COMMENT
            ),
            Err(BlockedReason::LegacyCredential)
        );
    }

    #[test]
    fn a_disabled_provider_or_missing_snapshot_lists_nothing() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let mut disabled = current.clone();
        disabled.enabled = false;
        assert_eq!(
            authorise_creation(&disabled, &COMMENT),
            Err(BlockedReason::NotListed)
        );
        let mut snapshot_gone = current.clone();
        snapshot_gone.snapshot = None;
        assert_eq!(
            authorise_creation(&snapshot_gone, &COMMENT),
            Err(BlockedReason::NotListed)
        );
    }

    #[test]
    fn a_snapshot_read_under_a_former_credential_is_not_fresh_state() {
        let resolved = github_resolved("tok-2");
        let mut former = current_snapshot(
            &resolved,
            Some("U_viewer1"),
            Some(trusted_review_requested(Some(OTHER_AUTHOR))),
        );
        former.fingerprint = github_resolved("tok-1").fingerprint;
        let current = state(Some(resolved), Some(former), Some(basis()));
        // Neither a creation...
        assert_eq!(
            authorise_creation(&current, &COMMENT),
            Err(BlockedReason::CredentialChanged)
        );
        // ...nor a fenced boundary: the old snapshot cannot stamp a new
        // credential in.
        assert_eq!(
            authorise_reconciliation(&current, &stamp_of(&github_resolved("tok-1"), "U_viewer1")),
            Err(BlockedReason::CredentialChanged)
        );
    }

    #[test]
    fn a_pull_request_off_the_current_snapshot_lists_nothing() {
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(&resolved, Some("U_viewer1"), None);
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_creation(&current, &COMMENT),
            Err(BlockedReason::NotListed)
        );
    }

    #[test]
    fn a_listed_row_without_trusted_ids_has_no_capability() {
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(&resolved, Some("U_viewer1"), Some(SnapshotRow::Untrusted));
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_creation(&current, &COMMENT),
            Err(BlockedReason::DecisionUnavailable)
        );
    }

    #[test]
    fn no_current_detail_establishes_no_commits() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let mut commitless = current;
        commitless.commits = None;
        assert_eq!(
            authorise_creation(&commitless, &COMMENT),
            Err(BlockedReason::DetailRequired)
        );
    }

    #[test]
    fn an_ordinary_edit_of_a_moved_commits_basis_is_stale() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let stamp = stamp_of(&github_resolved("tok-1"), "U_viewer1");
        assert_eq!(
            authorise_bound(&current, &stamp, &other_basis(), &CONTENT),
            Err(BlockedReason::StaleCommits)
        );
        assert_eq!(
            authorise_bound(&current, &stamp, &basis(), &CONTENT),
            Ok(AuthorisationGranted {
                stamp: stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                basis: Some(basis()),
            })
        );
    }

    #[test]
    fn a_saved_credential_fences_the_bound_draft() {
        // The current state is fresh under tok-2; the draft recorded tok-1.
        // Even a same-looking actor fences: the fingerprint differs.
        let resolved = github_resolved("tok-2");
        let snapshot = current_snapshot(
            &resolved,
            Some("U_viewer1"),
            Some(trusted_review_requested(Some(OTHER_AUTHOR))),
        );
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_bound(
                &current,
                &stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                &basis(),
                &COMMENT
            ),
            Err(BlockedReason::CredentialChanged)
        );
    }

    #[test]
    fn a_different_actor_cannot_act_under_a_recorded_stamp() {
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(
            &resolved,
            Some("U_viewer2"),
            Some(trusted_review_requested(Some(OTHER_AUTHOR))),
        );
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_bound(
                &current,
                &stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                &basis(),
                &COMMENT
            ),
            Err(BlockedReason::ActorChanged)
        );
    }

    #[test]
    fn a_snapshot_without_a_stable_actor_fences_the_stamp_and_offers_nothing() {
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(
            &resolved,
            None,
            Some(trusted_review_requested(Some(OTHER_AUTHOR))),
        );
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        // Fenced: the snapshot names no actor the recorded stamp could
        // verify.
        assert_eq!(
            authorise_bound(
                &current,
                &stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                &basis(),
                &COMMENT
            ),
            Err(BlockedReason::ActorChanged)
        );
        // Creation: nothing derives a stamp from.
        assert_eq!(
            authorise_creation(&current, &COMMENT),
            Err(BlockedReason::DecisionUnavailable)
        );
    }

    #[test]
    fn own_authorship_never_offers_approve_or_request_changes() {
        for row in [
            SnapshotRow::Trusted {
                provenance: RowProvenance::Authored,
                author: None,
            },
            SnapshotRow::Untrusted,
            trusted_review_requested(Some("U_viewer1")),
        ] {
            let resolved = github_resolved("tok-1");
            let snapshot = current_snapshot(&resolved, Some("U_viewer1"), Some(row));
            let current = state(Some(resolved), Some(snapshot), Some(basis()));
            assert_eq!(
                authorise_creation(&current, &APPROVE),
                Err(BlockedReason::DecisionUnavailable),
                "an own or unverifiable row cannot be approved"
            );
        }
    }

    #[test]
    fn another_authors_review_requested_row_offers_every_decision() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        for wanted in [&COMMENT[..], &APPROVE[..], &CONTENT[..]] {
            let granted = authorise_creation(&current, wanted).unwrap();
            assert_eq!(granted.stamp.actor, "U_viewer1".to_string());
        }
    }

    /// Action state lists all five actions, each exactly as the gate answers
    /// it: what an own pull request cannot do carries the gate's sentence,
    /// and what it can carries none.
    #[test]
    fn current_capabilities_list_every_action_as_the_gate_answers_it() {
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(
            &resolved,
            Some("U_viewer1"),
            Some(trusted_review_requested(Some("U_viewer1"))),
        );
        let own = state(Some(resolved), Some(snapshot), Some(basis()));
        let unavailable = Some(worded(BlockedReason::DecisionUnavailable));
        let offered = |name| ReviewCapability {
            name,
            available: true,
            reason: None,
        };
        assert_eq!(
            current_capabilities(&own),
            vec![
                offered(ReviewCapabilityName::Comment),
                ReviewCapability {
                    name: ReviewCapabilityName::Approve,
                    available: false,
                    reason: unavailable.clone(),
                },
                ReviewCapability {
                    name: ReviewCapabilityName::RequestChanges,
                    available: false,
                    reason: unavailable,
                },
                offered(ReviewCapabilityName::InlineComment),
                offered(ReviewCapabilityName::Reply),
            ]
        );
    }

    #[test]
    fn an_unprovably_foreign_author_runs_comment_only() {
        let (_, current) = all_current("tok-1", "U_viewer1", None);
        assert_eq!(
            authorise_creation(&current, &COMMENT).unwrap().basis,
            Some(basis())
        );
        assert_eq!(
            authorise_creation(&current, &APPROVE),
            Err(BlockedReason::DecisionUnavailable)
        );
    }

    #[test]
    fn bitbucket_identifies_the_same_neutral_capacity() {
        // BitBucket lists the actor's own authored rows: comment only, never
        // an approval — the neutral gate derives it from the assembled
        // provenance, not from a provider name.
        let resolved = bitbucket_resolved("ada", "tok-bb-1");
        let snapshot = current_snapshot(
            &resolved,
            Some("bb-uuid-1"),
            Some(SnapshotRow::Trusted {
                provenance: RowProvenance::Authored,
                author: None,
            }),
        );
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_creation(&current, &COMMENT).unwrap().basis,
            Some(basis())
        );
        assert_eq!(
            authorise_creation(&current, &APPROVE),
            Err(BlockedReason::DecisionUnavailable)
        );
        assert_eq!(
            authorise_creation(&current, &[ReviewCapabilityName::RequestChanges]),
            Err(BlockedReason::DecisionUnavailable)
        );
    }

    #[test]
    fn start_over_runs_a_fresh_gate_and_copies_no_fenced_stamp() {
        // A draft recorded tok-1; the credential was replaced with tok-2 and
        // the snapshot re-read under it. Start over compares neither
        // fingerprint nor actor: creation derives the new stamp.
        let resolved = github_resolved("tok-2");
        let snapshot = current_snapshot(
            &resolved,
            Some("U_viewer1"),
            Some(trusted_review_requested(Some(OTHER_AUTHOR))),
        );
        let current = state(Some(resolved), Some(snapshot), Some(basis()));
        let old_stamp = stamp_of(&github_resolved("tok-1"), "U_viewer1");
        let granted = authorise_creation(&current, &COMMENT).unwrap();
        assert_ne!(granted.stamp.fingerprint, old_stamp.fingerprint);
        assert_eq!(
            granted.stamp.fingerprint,
            github_resolved("tok-2").fingerprint
        );
    }

    #[test]
    fn stale_recovery_requires_identity_and_records_the_current_basis() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let stamp = stamp_of(&github_resolved("tok-1"), "U_viewer1");
        // Identity holds; the current basis is handed back for the recovered
        // revision to record, however it moved since the recorded one.
        let recovered = authorise_stale_recovery(&current, &stamp, &COMMENT).unwrap();
        assert_eq!(
            recovered.stamp,
            stamp_of(&github_resolved("tok-1"), "U_viewer1")
        );
        assert_eq!(recovered.basis, Some(basis()), "the current basis");
        // Identity no longer holds on either axis: fenced.
        assert_eq!(
            authorise_stale_recovery(
                &current,
                &stamp_of(&github_resolved("tok-2"), "U_viewer1"),
                &COMMENT
            ),
            Err(BlockedReason::CredentialChanged)
        );
        assert_eq!(
            authorise_stale_recovery(
                &current,
                &stamp_of(&github_resolved("tok-1"), "U_viewer2"),
                &COMMENT
            ),
            Err(BlockedReason::ActorChanged)
        );
    }

    #[test]
    fn a_reconciliation_read_needs_only_listed_identity() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let stamp = stamp_of(&github_resolved("tok-1"), "U_viewer1");
        assert_eq!(
            authorise_reconciliation(&current, &stamp).unwrap(),
            AuthorisationGranted { stamp, basis: None }
        );

        // It asserts nothing about commits — a raced push still classifies —
        // and nothing about capabilities: an untrusted row still lets the
        // stamped actor reconcile what its operation put there.
        let mut raced = current;
        raced.commits = None;
        assert_eq!(
            authorise_reconciliation(&raced, &stamp_of(&github_resolved("tok-1"), "U_viewer1"))
                .unwrap(),
            AuthorisationGranted {
                stamp: stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                basis: None,
            }
        );
        let untrusted = {
            let resolved = github_resolved("tok-1");
            let snapshot =
                current_snapshot(&resolved, Some("U_viewer1"), Some(SnapshotRow::Untrusted));
            state(Some(resolved.clone()), Some(snapshot), Some(basis()))
        };
        assert_eq!(
            authorise_reconciliation(
                &untrusted,
                &stamp_of(&github_resolved("tok-1"), "U_viewer1")
            )
            .unwrap(),
            AuthorisationGranted {
                stamp: stamp_of(&github_resolved("tok-1"), "U_viewer1"),
                basis: None,
            }
        );
    }

    #[test]
    fn reconciliation_still_asserts_listed_identity() {
        let (_, current) = all_current("tok-1", "U_viewer1", Some(OTHER_AUTHOR));
        let stamp = stamp_of(&github_resolved("tok-1"), "U_viewer1");
        // Row absent: not listed, whatever else is current.
        let resolved = github_resolved("tok-1");
        let snapshot = current_snapshot(&resolved, Some("U_viewer1"), None);
        let unlisted = state(Some(resolved.clone()), Some(snapshot), Some(basis()));
        assert_eq!(
            authorise_reconciliation(&unlisted, &stamp),
            Err(BlockedReason::NotListed)
        );
        // Actor changed: fenced.
        let mut changed = current.clone();
        changed
            .snapshot
            .as_mut()
            .expect("current carries a snapshot")
            .actor = Some("U_viewer2".to_string());
        assert_eq!(
            authorise_reconciliation(&changed, &stamp),
            Err(BlockedReason::ActorChanged)
        );
        // Disabled: nothing is listed, whatever the rows were.
        let mut disabled = current.clone();
        disabled.enabled = false;
        assert_eq!(
            authorise_reconciliation(&disabled, &stamp),
            Err(BlockedReason::NotListed)
        );
        // And nothing resolved authorises nothing.
        assert_eq!(
            authorise_reconciliation(&state(None, None, None), &stamp),
            Err(BlockedReason::LegacyCredential)
        );
    }

    #[test]
    fn legacy_refusal_outranks_every_lesser_block() {
        let legacy = github_resolved_legacy();
        let mut disabled = state(
            Some(legacy),
            Some(SnapshotContext {
                fingerprint: "a".repeat(64),
                actor: Some("U_viewer1".to_string()),
                row: Some(trusted_review_requested(Some(OTHER_AUTHOR))),
            }),
            Some(basis()),
        );
        disabled.enabled = false;
        assert_eq!(
            authorise_creation(&disabled, &COMMENT),
            Err(BlockedReason::LegacyCredential)
        );
    }
}

// ---------------------------------------------------------------- draft content
//
// The boundaries that create, edit and start a draft over, and the checks
// every one of them runs. A refusal is not an authorisation gate's: a gate
// failure and a content fault both cow the caller into re-reading current
// state, and neither ever changes persisted content.

/// The 30 KiB (30 × 1024 bytes) UTF-8 bound on a summary and on an item
/// body, excluding SpecForge's temporary reconciliation markers
/// (`pull-request-actions`: *Draft Items and Decisions Are Explicit and
/// Valid*).
const MAX_BODY_BYTES: usize = 30 * 1024;

/// The bound on one draft's comment/reply items in total.
const MAX_DRAFT_ITEMS: usize = 100;

/// One item of a draft edit. An item the caller already read from the
/// current draft is carried whole — its backend UUID and all, changes
/// included — so an accepted edit keeps every item's identity and order;
/// anything absent from the edit's list is dropped. A new item carries no
/// id: the service generates the item's backend UUID on acceptance, in the
/// caller's list order. No range, no start line and no structured
/// suggestion exists in this vocabulary, and a body is opaque markdown
/// stored exactly as the reviewer entered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DraftItemInput {
    /// An item as the current draft stores it, carried back whole: an
    /// unplaced one keeps its former anchor and current basis untouched,
    /// a placed one is re-validated against the current detail exactly as
    /// a new one is.
    Carried { item: DraftItem },
    /// A new single-line inline comment on one current line of one current
    /// diff-file identity — its new path, or its old path when deleted —
    /// in the draft's basis.
    NewInline {
        body: String,
        path: String,
        side: DiffSide,
        line: u32,
    },
    /// A new reply to one thread the current detail reports replyable, by
    /// its stable provider target.
    NewReply { body: String, target: String },
}

/// Why a draft mutation — create, edit or Start over — refused. Every
/// variant leaves the persisted draft exactly as it was; the caller decides
/// again from the refusal. `storeCorrupt` is the damaged-journal boundary
/// the same as it reports through action state; `blocked` is the
/// authorisation gate's structured reason; the rest are the mutation's own
/// checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DraftMutationRefusal {
    /// `pull-request-actions.json` is damaged and preserved in place;
    /// review actions report `actionStoreCorrupt`. The store's reason
    /// rides along worded, and never carries a credential-derived value.
    StoreCorrupt { reason: String },
    /// The store could not be read, locked, written or durably flushed.
    /// Nothing was changed; the caller's own I/O story rides along worded.
    StoreFailure { reason: String },
    /// The authorisation gate refused, with its exact structured reason.
    Blocked { reason: BlockedReason },
    /// No current draft exists for the pull request, so nothing is edited
    /// or started over; the caller creates one first.
    NoDraft,
    /// An unfinished review operation still fences this pull request, so
    /// Start over may not replace its draft; it is abandoned first, or the
    /// operation finishes.
    OperationUnfinished,
    /// The pull request has no operation to resume or abandon.
    NoOperation,
    /// Another process — or this one's run — still holds the operation's
    /// execution lock, so its live phase is not crash residue and nothing
    /// may normalise, reconcile, resume or abandon it here.
    OperationLive,
    /// The operation already submitted or was abandoned; its receipt stays
    /// readable through action state.
    OperationFinished,
    /// Resume's first unresolved step is not eligible for another attempt:
    /// an uncertain write no read has decided, or a failed one that is
    /// never retried under the same credential. The reason is the
    /// service's sentence for the reviewer.
    OperationNotEligible { reason: String },
    /// The revision the caller read has moved on. Nothing changed, and the
    /// current draft rides along — enough non-secret state to re-read.
    StaleRevision { current: Box<ReviewDraft> },
    /// A body or the summary exceeds 30 KiB as UTF-8, or the draft would
    /// carry a 101st comment/reply item. No oversized value is persisted.
    ContentTooLarge,
    /// The item at this zero-based position anchors nothing current: not a
    /// diff-file identity, side and line the cached detail under the
    /// draft's basis shows, and not a thread it reports replyable.
    StaleAnchor { at: u32 },
    /// A body, or the supplied summary, is empty after whitespace is
    /// ignored — whitespace is not review content. `at` is the item's
    /// zero-based position, or `None` for the summary.
    EmptyContent { at: Option<u32> },
    /// The frozen revision this freeze proposes was already confirmed into
    /// an operation: one draft revision confirms into at most one. Nothing
    /// changed, and the current draft rides along to re-read from.
    RevisionClaimed { current: Box<ReviewDraft> },
    /// The decision's content rule is not met: a Comment or a
    /// Request-changes freeze needs at least one non-empty summary or item,
    /// while whitespace is not review content (`pull-request-actions`:
    /// *Draft Items and Decisions Are Explicit and Valid*). An empty
    /// approval is valid and never refused this way.
    NoContent,
}

/// A fresh empty draft: backend-generated UUID, revision 1, `basis`, the
/// decision as it starts, no summary and no items. The creation boundary
/// records exactly this, and Start over replaces its draft with exactly
/// this — the discarded draft's content and identity are copied to nothing.
pub(crate) fn fresh_draft(reference: PullRequestReference, basis: CommitBasis) -> ReviewDraft {
    ReviewDraft {
        id: crate::review_action_store::new_record_id(),
        reference,
        revision: 1,
        basis,
        decision: ReviewDecision::Comment,
        summary: String::new(),
        items: Vec::new(),
    }
}

/// The stored item one edit input becomes: a carried item comes back whole
/// with its backend UUID, a new one is placed with a fresh backend UUID in
/// the caller's list order.
pub(crate) fn stored_item(input: DraftItemInput, fresh_id: String) -> DraftItem {
    match input {
        DraftItemInput::Carried { item } => item,
        DraftItemInput::NewInline {
            body,
            path,
            side,
            line,
        } => DraftItem::PlacedInline {
            id: fresh_id,
            body,
            path,
            side,
            line,
        },
        DraftItemInput::NewReply { body, target } => DraftItem::PlacedReply {
            id: fresh_id,
            body,
            target,
        },
    }
}

/// Whether `path` names `file`'s current identity: its new path, or its old
/// path when deleted. A renamed file's old path is not its identity — only
/// the new one is (`pull-request-viewer`: *Changed Files in the
/// Pull-Request View*, keyed the same way).
fn file_identity(file: &DiffFile, path: &str) -> bool {
    file_path(file) == Some(path)
}

/// Whether `file`'s current content shows `side` line `line` to anchor on,
/// as its hunks number them. A file without hunks — withheld, binary,
/// oversized — shows nothing to anchor.
fn line_shown(file: &DiffFile, side: DiffSide, line: u32) -> bool {
    let DiffContent::Hunks { hunks } = &file.content else {
        return false;
    };
    hunks.iter().any(|hunk| {
        hunk.lines
            .iter()
            .any(|shown| match side {
                DiffSide::Old => shown.old_no,
                DiffSide::New => shown.new_no,
            } == Some(line))
    })
}

/// Whether `target` names one thread the current detail reports replyable.
fn replyable_thread(detail: &PullRequestDetail, target: &str) -> bool {
    detail
        .threads
        .iter()
        .any(|thread| thread.id == *target && thread.replyable)
}

/// An item's body, whatever kind it is.
fn item_body(item: &DraftItem) -> &str {
    match item {
        DraftItem::PlacedInline { body, .. }
        | DraftItem::UnplacedInline { body, .. }
        | DraftItem::PlacedReply { body, .. }
        | DraftItem::UnplacedReply { body, .. } => body,
    }
}

impl DraftItem {
    /// The item's backend-generated UUID, which a carried item presents
    /// again and which the service verifies against the current draft.
    pub(crate) fn id(&self) -> &str {
        match self {
            DraftItem::PlacedInline { id, .. }
            | DraftItem::UnplacedInline { id, .. }
            | DraftItem::PlacedReply { id, .. }
            | DraftItem::UnplacedReply { id, .. } => id,
        }
    }
}

/// `current` with what a completed submission of `frozen` posted taken out:
/// every item the submission carried, still held exactly as it was frozen,
/// the summary when it is still the frozen one, and the decision, back to
/// Comment, when it is still the frozen one. Whatever the reviewer added or
/// edited while the frozen operation ran stays, so it can be submitted
/// next. A draft that holds none of it is returned as it was; any other
/// takes the next revision, once.
pub(crate) fn without_submitted(current: &ReviewDraft, frozen: &ReviewDraft) -> ReviewDraft {
    let items: Vec<DraftItem> = current
        .items
        .iter()
        .filter(|item| !frozen.items.contains(item))
        .cloned()
        .collect();
    let summary = if current.summary == frozen.summary {
        String::new()
    } else {
        current.summary.clone()
    };
    let decision = if current.decision == frozen.decision {
        ReviewDecision::Comment
    } else {
        current.decision
    };
    if items == current.items && summary == current.summary && decision == current.decision {
        return current.clone();
    }
    ReviewDraft {
        revision: current.revision + 1,
        decision,
        summary,
        items,
        ..current.clone()
    }
}

/// The items one stale-draft recovery carries into the recovered revision
/// (`pull-request-actions`: *Stale Commit Recovery Never Remaps an Inline
/// Comment*), read against the detail the gate authorised under:
///
/// - every inline item unplaces, retaining its body, former path, side and
///   line and the commits that placement was made under — an already
///   unplaced one keeps the older basis it recorded,
/// - a reply stays placed only when its exact stable provider target is
///   still one thread the current detail reports replyable, and otherwise
///   retains its body and former target as an unplaced item;
///
/// and nothing else: no automatic placement happens even where the same
/// path, side, line number or text exists in the new diff. `former_basis`
/// is the draft's basis the placements were made under.
pub(crate) fn recovered_items(
    items: &[DraftItem],
    former_basis: &CommitBasis,
    detail: &PullRequestDetail,
) -> Vec<DraftItem> {
    items
        .iter()
        .map(|item| match item {
            DraftItem::PlacedInline {
                id,
                body,
                path,
                side,
                line,
            } => DraftItem::UnplacedInline {
                id: id.clone(),
                body: body.clone(),
                path: path.clone(),
                side: *side,
                line: *line,
                basis: former_basis.clone(),
            },
            DraftItem::PlacedReply { id, body, target } => {
                if replyable_thread(detail, target) {
                    item.clone()
                } else {
                    DraftItem::UnplacedReply {
                        id: id.clone(),
                        body: body.clone(),
                        target: target.clone(),
                    }
                }
            }
            DraftItem::UnplacedInline { .. } | DraftItem::UnplacedReply { .. } => item.clone(),
        })
        .collect()
}

/// Whether `item`'s placement is something the cached detail shows: a placed
/// item's exact anchor — file identity, side and line for an inline comment,
/// one replyable thread for a reply — and trivially so for an unplaced one,
/// which asserts nothing until the reviewer explicitly places it again.
fn anchored(item: &DraftItem, detail: &PullRequestDetail) -> bool {
    match item {
        DraftItem::PlacedInline {
            path, side, line, ..
        } => detail
            .files
            .iter()
            .find(|file| file_identity(file, path))
            .is_some_and(|file| line_shown(file, *side, *line)),
        DraftItem::PlacedReply { target, .. } => replyable_thread(detail, target),
        DraftItem::UnplacedInline { .. } | DraftItem::UnplacedReply { .. } => true,
    }
}

/// Validates one draft edit's summary and items exactly as the spec bounds
/// them (`pull-request-actions`: *Draft Items and Decisions Are Explicit and
/// Valid*), against the cached detail the authorisation gate authorised
/// under: a present summary or body is rejected when it is empty after
/// whitespace is ignored or beyond 30 KiB as UTF-8; the edit is rejected
/// when it would carry the draft a 101st comment/reply item; and every
/// anchor is current — each new or carried placed inline item names one
/// current file identity, side and line, each new or carried placed reply
/// one replyable thread, while a carried unplaced item asserts nothing. The
/// first fault wins.
pub(crate) fn validate_draft_content(
    summary: &Option<String>,
    items: &[DraftItemInput],
    detail: &PullRequestDetail,
) -> Result<(), DraftMutationRefusal> {
    if let Some(summary) = summary {
        if summary.trim().is_empty() {
            return Err(DraftMutationRefusal::EmptyContent { at: None });
        }
        if summary.len() > MAX_BODY_BYTES {
            return Err(DraftMutationRefusal::ContentTooLarge);
        }
    }
    if items.len() > MAX_DRAFT_ITEMS {
        return Err(DraftMutationRefusal::ContentTooLarge);
    }
    for (index, input) in items.iter().enumerate() {
        let at = u32::try_from(index).unwrap_or(u32::MAX);
        let (body, current) = match input {
            DraftItemInput::Carried { item } => (item_body(item), anchored(item, detail)),
            DraftItemInput::NewInline {
                body,
                path,
                side,
                line,
            } => (
                body.as_str(),
                detail
                    .files
                    .iter()
                    .find(|file| file_identity(file, path))
                    .is_some_and(|file| line_shown(file, *side, *line)),
            ),
            DraftItemInput::NewReply { body, target } => {
                (body.as_str(), replyable_thread(detail, target))
            }
        };
        if body.trim().is_empty() {
            return Err(DraftMutationRefusal::EmptyContent { at: Some(at) });
        }
        if body.len() > MAX_BODY_BYTES {
            return Err(DraftMutationRefusal::ContentTooLarge);
        }
        if !current {
            return Err(DraftMutationRefusal::StaleAnchor { at });
        }
    }
    Ok(())
}

// ------------------------------------------------------------ action notices
//
// The review-action invalidation broadcast, deliberately not a
// `ServiceNotice` variant and never an event of `events.rs`
// (`pull-request-actions`: *Review Action State Synchronises Only Native
// Windows*): windows of the desktop re-read action state when one arrives,
// while the web server neither subscribes to it nor exposes it, there
// being no action command on any served surface at all.

/// One review-action invalidation: that some action state of the canonical
/// pull request moved on. It carries only the canonical reference plus the
/// draft's non-secret revision when a draft changed, or the operation's
/// phase when an operation's state moved — and never draft prose, a
/// receipt body, credential-derived values, or the claimed/attempted
/// provider effects, none of which the receiver needs to know that its
/// view is stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActionInvalidation {
    /// A draft — created, edited, recovered or started over — moved to
    /// this revision.
    Draft {
        reference: PullRequestReference,
        revision: u64,
    },
    /// An operation's state moved to this phase.
    Operation {
        reference: PullRequestReference,
        phase: OperationPhase,
    },
}

// --------------------------------------------------------- native confirmation
//
// The freeze between the composer and the provider
// (`pull-request-actions`: *Native Confirmation Freezes One Exact Draft
// Revision*, design D4): an operating-system confirmation freezes one exact
// draft revision as a private native value before anything else may write.

/// The complete frozen draft revision, held as a private native value
/// between freezing and acceptance. Deliberately **not**
/// `Serialize`/`Deserialize`: it never crosses IPC and never reaches any
/// wire, so the only things a window ever sees of it are the confirmation
/// [`NativeConfirmation`]'s counts, and dropping it without acceptance
/// writes nothing anywhere. Acceptance copies the freeze's whole draft and
/// stamp into the operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewFreeze {
    /// The exact revision frozen, whole: UUID, reference, revision, basis,
    /// decision, summary and every item with its body.
    pub(crate) draft: ReviewDraft,
    /// The authorisation stamp acceptance must still re-authorise under.
    pub(crate) stamp: ActionAuthorisationStamp,
}

/// What the operating-system confirmation names: the provider, repository,
/// pull-request number, exact decision, whether a summary is present, the
/// number of inline comments and replies, and the frozen revision. The
/// frozen bodies never ride it — the reviewer confirms what the composer
/// shows, and the store keeps the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeConfirmation {
    /// The canonical pull request the frozen revision belongs to.
    pub reference: PullRequestReference,
    /// The exact decision the operation would submit.
    pub decision: ReviewDecision,
    /// Whether the frozen draft carries a non-empty summary. Its prose never
    /// leaves the action store.
    pub summary_present: bool,
    /// How many placed inline comments the frozen draft carries.
    pub inline_comments: usize,
    /// How many placed replies the frozen draft carries.
    pub replies: usize,
    /// The exact draft revision frozen, which acceptance must still find.
    pub frozen_revision: u64,
}

/// Validates one draft as freezable, exactly the spec bounds submission
/// inherits (`pull-request-actions`: *Draft Items and Decisions Are
/// Explicit and Valid*, *Native Confirmation Freezes One Exact Draft
/// Revision*): every item must be placed — an unplaced one is ineligible
/// for submission and keeps refusing until the reviewer explicitly
/// converts it — and the decision's content rule must be met. A Comment or
/// a Request-changes draft needs at least one non-empty summary or item;
/// an empty approval is valid. Size bounds and per-item prose checks were
/// already enforced at every edit, and a persisted draft cannot hold
/// values they would have refused; the first fault wins.
pub(crate) fn validate_freeze_content(draft: &ReviewDraft) -> Result<(), DraftMutationRefusal> {
    let summary_present = !draft.summary.trim().is_empty();
    let mut has_item = false;
    for (index, item) in draft.items.iter().enumerate() {
        let at = u32::try_from(index).map_err(|_| DraftMutationRefusal::ContentTooLarge)?;
        match item {
            DraftItem::PlacedInline { .. } | DraftItem::PlacedReply { .. } => {
                has_item = true;
            }
            DraftItem::UnplacedInline { .. } | DraftItem::UnplacedReply { .. } => {
                return Err(DraftMutationRefusal::StaleAnchor { at });
            }
        }
    }
    if matches!(
        draft.decision,
        ReviewDecision::Comment | ReviewDecision::RequestChanges
    ) && !summary_present
        && !has_item
    {
        return Err(DraftMutationRefusal::NoContent);
    }
    Ok(())
}

/// Freezes one exact draft revision: the private native value acceptance
/// copies whole, and the confirmation summary it names to the reviewer.
pub(crate) fn freeze(
    draft: ReviewDraft,
    stamp: ActionAuthorisationStamp,
) -> (ReviewFreeze, NativeConfirmation) {
    let confirmation = NativeConfirmation {
        reference: draft.reference.clone(),
        decision: draft.decision,
        summary_present: !draft.summary.trim().is_empty(),
        inline_comments: draft
            .items
            .iter()
            .filter(|item| matches!(item, DraftItem::PlacedInline { .. }))
            .count(),
        replies: draft
            .items
            .iter()
            .filter(|item| matches!(item, DraftItem::PlacedReply { .. }))
            .count(),
        frozen_revision: draft.revision,
    };
    (ReviewFreeze { draft, stamp }, confirmation)
}

/// A blocked reason worded for the reviewer, one sentence a status view can
/// show as the operation's `last_error` without a Debug spelling.
pub(crate) fn worded(reason: BlockedReason) -> String {
    match reason {
        BlockedReason::ActionStoreCorrupt => {
            "pull-request-actions.json is damaged; repair or remove it before this review can proceed".to_string()
        }
        BlockedReason::NotListed => {
            "the pull request is no longer listed by its provider".to_string()
        }
        BlockedReason::DetailRequired => {
            "the pull request's detail has not been read under the current credential yet"
                .to_string()
        }
        BlockedReason::StaleCommits => {
            "the pull request's commits changed against the reviewed basis".to_string()
        }
        BlockedReason::LegacyCredential => {
            "only a legacy plaintext credential was found, which cannot authorise a review".to_string()
        }
        BlockedReason::CredentialChanged => {
            "the credential changed since the review was authorised".to_string()
        }
        BlockedReason::ActorChanged => {
            "the authenticated actor changed since the review was authorised".to_string()
        }
        BlockedReason::DecisionUnavailable => {
            "this decision is not offered by the provider's current capability".to_string()
        }
    }
}

#[cfg(test)]
mod submitted_draft_tests {
    use super::*;
    use crate::events::PullRequestProvider;

    fn reply(id: &str, body: &str) -> DraftItem {
        DraftItem::PlacedReply {
            id: id.to_string(),
            body: body.to_string(),
            target: "PRRT_1".to_string(),
        }
    }

    fn draft(
        revision: u64,
        decision: ReviewDecision,
        summary: &str,
        items: Vec<DraftItem>,
    ) -> ReviewDraft {
        ReviewDraft {
            id: "draft-1".to_string(),
            reference: PullRequestReference {
                provider: PullRequestProvider::Github,
                owner: "acme".to_string(),
                repo: "api".to_string(),
                number: 7,
            },
            revision,
            basis: CommitBasis {
                head: "head".to_string(),
                base: "base".to_string(),
            },
            decision,
            summary: summary.to_string(),
            items,
        }
    }

    /// The draft submitted unchanged empties: no item, no summary, the
    /// decision back to Comment, on the next revision of the same draft.
    #[test]
    fn a_draft_submitted_as_it_stands_empties() {
        let frozen = draft(
            2,
            ReviewDecision::Approve,
            "Looks good.",
            vec![reply("a", "Agreed.")],
        );
        assert_eq!(
            without_submitted(&frozen, &frozen),
            draft(3, ReviewDecision::Comment, "", Vec::new()),
            "only the revision moves, once"
        );
    }

    /// Content added or edited while the frozen operation ran stays for
    /// the next submission; what was posted as frozen goes.
    #[test]
    fn edits_made_after_the_freeze_stay() {
        let frozen = draft(
            2,
            ReviewDecision::Comment,
            "First.",
            vec![reply("a", "Agreed."), reply("b", "Typo here.")],
        );
        let current = draft(
            4,
            ReviewDecision::RequestChanges,
            "Second thoughts.",
            vec![
                reply("a", "Agreed."),
                reply("b", "Typo here, and here."),
                reply("c", "New."),
            ],
        );
        assert_eq!(
            without_submitted(&current, &frozen),
            draft(
                5,
                ReviewDecision::RequestChanges,
                "Second thoughts.",
                vec![reply("b", "Typo here, and here."), reply("c", "New.")],
            )
        );
    }

    /// Each part is weighed on its own: an unchanged summary goes while a
    /// changed decision stays, and the reverse.
    #[test]
    fn summary_and_decision_are_each_cleared_only_when_unchanged() {
        let frozen = draft(2, ReviewDecision::Approve, "Ship it.", Vec::new());
        assert_eq!(
            without_submitted(
                &draft(3, ReviewDecision::RequestChanges, "Ship it.", Vec::new()),
                &frozen
            ),
            draft(4, ReviewDecision::RequestChanges, "", Vec::new())
        );
        assert_eq!(
            without_submitted(
                &draft(3, ReviewDecision::Approve, "Wait.", Vec::new()),
                &frozen
            ),
            draft(4, ReviewDecision::Comment, "Wait.", Vec::new())
        );
    }

    /// A draft holding nothing the submission posted is returned as it was,
    /// revision included, so nothing is written or announced.
    #[test]
    fn a_draft_holding_nothing_submitted_is_untouched() {
        let frozen = draft(2, ReviewDecision::Comment, "", vec![reply("a", "Agreed.")]);
        let current = draft(5, ReviewDecision::Comment, "", vec![reply("z", "Later.")]);
        assert_eq!(without_submitted(&current, &frozen), current);
        let empty = draft(5, ReviewDecision::Comment, "", Vec::new());
        assert_eq!(without_submitted(&empty, &frozen), empty);
    }
}

#[cfg(test)]
mod draft_content_tests {
    use super::*;
    use crate::events::PullRequestProvider;
    use crate::pull_request_detail::ReviewThread;
    use crate::pull_requests::PullRequestSummary;

    /// A detail with one replyable thread, `PRRT_1`, and no changed file.
    fn detail() -> PullRequestDetail {
        PullRequestDetail {
            reference: PullRequestReference {
                provider: PullRequestProvider::Github,
                owner: "acme".to_string(),
                repo: "api".to_string(),
                number: 7,
            },
            row: PullRequestSummary {
                id: 7,
                title: "PR 7".to_string(),
                repo_full_name: "acme/api".to_string(),
                source_branch: "feature".to_string(),
                destination_branch: "main".to_string(),
                url: "https://github.com/acme/api/pull/7".to_string(),
                draft: false,
                updated_at_unix: 1_700_000_000,
                review: None,
                open_tasks: 0,
                author: None,
                checks: None,
                conflicting: false,
                unresolved_threads: 1,
                source_repo_full_name: "acme/api".to_string(),
            },
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            head_commit: "head".to_string(),
            base_commit: "base".to_string(),
            author: None,
            description: String::new(),
            conversation: Vec::new(),
            checks: Vec::new(),
            threads: vec![ReviewThread {
                id: "PRRT_1".to_string(),
                path: "src/api.ts".to_string(),
                side: DiffSide::New,
                line: Some(3),
                start_side: None,
                start_line: None,
                original_line: None,
                original_start_line: None,
                resolved: false,
                outdated: false,
                replyable: true,
                comments: Vec::new(),
            }],
            files: Vec::new(),
            unlisted_files: 0,
            read_at_unix: 1_700_000_000,
            no_longer_listed: false,
        }
    }

    fn carried(item: DraftItem) -> DraftItemInput {
        DraftItemInput::Carried { item }
    }

    fn placed_reply(target: &str, body: &str) -> DraftItem {
        DraftItem::PlacedReply {
            id: "i1".to_string(),
            body: body.to_string(),
            target: target.to_string(),
        }
    }

    /// A carried placed item is re-validated against the current detail
    /// exactly as a new one is: a reply to a thread the detail no longer
    /// reports refuses as a stale anchor at its index.
    #[test]
    fn a_carried_placed_item_must_still_be_anchored() {
        let detail = detail();
        let current = carried(placed_reply("PRRT_1", "Agreed."));
        assert_eq!(
            validate_draft_content(&None, std::slice::from_ref(&current), &detail),
            Ok(())
        );
        assert_eq!(
            validate_draft_content(
                &None,
                &[current, carried(placed_reply("PRRT_gone", "Agreed."))],
                &detail
            ),
            Err(DraftMutationRefusal::StaleAnchor { at: 1 })
        );
    }

    /// A carried item's body is held to the same bounds as a new one's,
    /// whatever its kind: an unplaced item, which asserts no anchor, is
    /// still refused when its body is blank or oversized.
    #[test]
    fn a_carried_items_body_is_bounded_like_a_new_one() {
        let detail = detail();
        let unplaced = |body: &str| {
            carried(DraftItem::UnplacedReply {
                id: "i2".to_string(),
                body: body.to_string(),
                target: "PRRT_gone".to_string(),
            })
        };
        assert_eq!(
            validate_draft_content(&None, &[unplaced("Still true.")], &detail),
            Ok(())
        );
        assert_eq!(
            validate_draft_content(&None, &[unplaced(" \n\t")], &detail),
            Err(DraftMutationRefusal::EmptyContent { at: Some(0) })
        );
        assert_eq!(
            validate_draft_content(&None, &[unplaced(&"x".repeat(MAX_BODY_BYTES + 1))], &detail),
            Err(DraftMutationRefusal::ContentTooLarge)
        );
        assert_eq!(
            validate_draft_content(&None, &[carried(placed_reply("PRRT_1", ""))], &detail),
            Err(DraftMutationRefusal::EmptyContent { at: Some(0) })
        );
    }
}

#[cfg(test)]
mod operation_order_tests {
    use super::*;
    use crate::events::PullRequestProvider;

    fn draft(decision: ReviewDecision, summary: &str, items: Vec<DraftItem>) -> ReviewDraft {
        ReviewDraft {
            id: "draft-1".to_string(),
            reference: PullRequestReference {
                provider: PullRequestProvider::Github,
                owner: "acme".to_string(),
                repo: "api".to_string(),
                number: 7,
            },
            revision: 3,
            basis: CommitBasis {
                head: "head".to_string(),
                base: "base".to_string(),
            },
            decision,
            summary: summary.to_string(),
            items,
        }
    }

    fn items() -> Vec<DraftItem> {
        vec![
            DraftItem::PlacedInline {
                id: "i1".to_string(),
                body: "A line lies.".to_string(),
                path: "src/api.ts".to_string(),
                side: DiffSide::New,
                line: 27,
            },
            DraftItem::UnplacedReply {
                id: "i2".to_string(),
                body: "Left behind.".to_string(),
                target: "PRRT_9".to_string(),
            },
            DraftItem::PlacedReply {
                id: "i3".to_string(),
                body: "Agreed.".to_string(),
                target: "PRRT_1".to_string(),
            },
        ]
    }

    fn inline(item: &str) -> LedgerStep {
        LedgerStep::InlineComment {
            item: item.to_string(),
        }
    }

    fn reply(item: &str) -> LedgerStep {
        LedgerStep::Reply {
            item: item.to_string(),
        }
    }

    fn entry(step: LedgerStep, outcome: LedgerOutcome) -> LedgerEntry {
        LedgerEntry { step, outcome }
    }

    /// The canonical order is the placed items in frozen order, then the
    /// summary while it has prose, then the decision wherever the provider
    /// writes one: GitHub always, BitBucket only for a decision other than
    /// Comment. An unplaced item names no write.
    #[test]
    fn a_frozen_draft_writes_its_placed_items_then_summary_then_decision() {
        let full = draft(ReviewDecision::Comment, "About it.", items());
        assert_eq!(
            operation_steps(&full, true),
            vec![
                inline("i1"),
                reply("i3"),
                LedgerStep::Summary,
                LedgerStep::Decision
            ]
        );
        assert_eq!(
            operation_steps(&full, false),
            vec![inline("i1"), reply("i3"), LedgerStep::Summary]
        );
        let approval = draft(ReviewDecision::Approve, " \n", Vec::new());
        assert_eq!(
            operation_steps(&approval, false),
            vec![LedgerStep::Decision]
        );
    }

    /// Resume starts at the first step without a confirmed entry, answers
    /// none once every step is confirmed, and refuses — never retrying — at
    /// an uncertain or failed first unresolved step.
    #[test]
    fn resume_starts_at_the_first_unconfirmed_step_or_refuses() {
        let steps = [inline("i1"), reply("i3"), LedgerStep::Decision];
        let confirmed = LedgerOutcome::Confirmed { provider_id: None };
        assert_eq!(resume_step(&steps, &[]), Ok(Some(inline("i1"))));
        let first_done = [entry(inline("i1"), confirmed.clone())];
        assert_eq!(resume_step(&steps, &first_done), Ok(Some(reply("i3"))));

        let uncertain = [
            entry(inline("i1"), confirmed.clone()),
            entry(reply("i3"), LedgerOutcome::Uncertain),
        ];
        assert!(resume_step(&steps, &uncertain).is_err());
        let failed = [
            entry(inline("i1"), confirmed.clone()),
            entry(
                reply("i3"),
                LedgerOutcome::Failed {
                    reason: "thread deleted".to_string(),
                },
            ),
        ];
        let refusal = resume_step(&steps, &failed).unwrap_err();
        assert!(refusal.contains("thread deleted"), "{refusal}");

        let all_done = steps.clone().map(|step| entry(step, confirmed.clone()));
        assert_eq!(resume_step(&steps, &all_done), Ok(None));
    }

    /// Only the live phases need the execution lock, and only submitted and
    /// abandoned are over; paused is neither.
    #[test]
    fn phases_split_into_live_paused_and_terminal() {
        use OperationPhase::*;
        for (phase, live, terminal) in [
            (Running, true, false),
            (Attempting, true, false),
            (Reconciling, true, false),
            (Paused, false, false),
            (Submitted, false, true),
            (Abandoned, false, true),
        ] {
            assert_eq!(phase.is_live(), live, "{phase:?}");
            assert_eq!(phase.is_terminal(), terminal, "{phase:?}");
        }
    }
}
