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
use crate::pull_request_detail::{DiffSide, PullRequestReference};
use crate::review_action_store::ActionAuthorisationStamp;
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

/// A read-only reconciliation boundary: the provider must still be enabled
/// and the pull request listed under the stamped credential and actor, but
/// neither commits nor capabilities are asserted — reconciliation exists to
/// classify a write that raced either change, and its reads never move the
/// operation into `running`.
pub fn authorise_reconciliation(
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
