//! The durable store behind the desktop's pull-request review actions:
//! review drafts and review-operation journals in `pull-request-actions.json`,
//! beside the other application stores in the shared configuration directory
//! (`pull-request-actions`: *Durable Review Drafts Use Optimistic Revisions*,
//! *Interrupted Operations Reconcile Without Automatic Mutation*).
//!
//! Three rules shape everything here:
//!
//! - **The file is the idempotency journal, not a cache.** It is never
//!   returned wholesale, and a file that does not parse — or a persisted
//!   document this version cannot read with `deny_unknown_fields` and the
//!   pinned schema version — is preserved in place and fails every action
//!   closed (`actionStoreCorrupt`), never replaced with an empty model that
//!   could repeat an ambiguous provider write.
//! - **One short transaction per state change.** Every mutation takes the
//!   store's sidecar lock, re-reads the file anew, applies its checks and
//!   replaces the file atomically and durably, so two desktop processes
//!   cannot both act on the same stored state and this process's own
//!   writes never interleave. A declined transaction (a stale expected
//!   revision, an operation that moved on) changes nothing on disk.
//! - **One execution lock per operation.** Confirmation, read-only
//!   reconciliation, Resume and Abandon acquire the operation's lock
//!   non-blockingly and hold it across their provider requests; every
//!   store call they make is a transaction nested inside it, and the locks
//!   are never taken in the opposite order. Durable credential state is
//!   resolved under the settings store's own lock, which is released
//!   before any action-store transaction, so the two are never held
//!   simultaneously.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use openspec_core::config_file::{read_if_present, write_atomic};

use crate::file_lock::PathLock;
use crate::pull_request_detail::PullRequestReference;
use crate::review_actions::{
    LedgerEntry, LedgerOutcome, LedgerStep, OperationPhase, ReviewDecision, ReviewDraft,
    ReviewReceipt,
};

/// The schema version this store reads and writes. A document that names
/// another version — a future one ahead of this build, or a damaged one —
/// is preserved as corrupt input, so a rewrite never narrows meaning this
/// version does not understand.
const DOCUMENT_VERSION: u32 = 1;

// ---------------------------------------------------------------- records

/// The persisted, non-secret authorisation identity of one draft or
/// operation (`pull-request-actions`: *Durable Review Drafts Use Optimistic
/// Revisions*): the credential fingerprint's SHA-256 digest, hex-encoded,
/// and the provider's stable actor id. The digest is a one-way derivative,
/// not a credential or a token fragment; it stays in this file and the
/// service and never reaches IPC, an event or a log.
///
/// The authorisation gate derives stamps from freshly resolved credentials;
/// the store persists and compares them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionAuthorisationStamp {
    /// The resolved credential's fingerprint digest, as 64 lowercase hex
    /// characters.
    pub fingerprint: String,
    /// The provider's stable authenticated actor id: GitHub's account node
    /// id, or BitBucket's account id or UUID.
    pub actor: String,
}

/// One current draft as this file persists it: the wire-shaped draft — never
/// returned through IPC with the stamp — beside the authorisation identity it
/// was created or recovered under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionDraft {
    /// The draft itself: UUID, canonical reference, revision, basis,
    /// decision, summary and ordered items.
    pub draft: ReviewDraft,
    /// The authorisation identity editing and submitting must still match.
    pub stamp: ActionAuthorisationStamp,
}

/// One operation as this file persists it. Its phase, ledger, last error and
/// receipt are the operation's own state; the rest is the frozen draft this
/// slice confirmed, deliberately duplicated so a later draft edit, Start over
/// or cache eviction cannot change what was confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionOperation {
    /// The operation's backend-generated UUID, which its execution lock and
    /// its journal entries belong to.
    pub id: String,
    /// The draft UUID that was frozen into this operation. Start over
    /// replaces the current draft to a new UUID; the frozen one stays here.
    pub draft_id: String,
    /// The draft revision that confirmed into this operation; one draft
    /// revision confirms into at most one operation.
    pub frozen_revision: u64,
    /// The complete frozen draft content as confirmed.
    pub frozen_draft: ReviewDraft,
    /// The authorisation identity every re-check of this operation must
    /// still match.
    pub stamp: ActionAuthorisationStamp,
    /// The exact persisted phase.
    pub phase: OperationPhase,
    /// One entry per write, in the operation's write order.
    pub ledger: Vec<LedgerEntry>,
    /// The most recent local or provider failure, worded for the reviewer.
    pub last_error: Option<String>,
    /// The GitHub pending review this operation created or adopted, once a
    /// provider reply named it. It is the only review cleanup, cleanup
    /// reconciliation and submission may name; BitBucket has no pending
    /// review and leaves it `None`.
    #[serde(default)]
    pub pending_review_id: Option<String>,
    /// The terminal receipt, for `submitted` and `abandoned`.
    pub receipt: Option<ReviewReceipt>,
}

/// A canonical pull request's key in the file's maps: its provider, owner and
/// repository in lowercase and its number — the same grammar
/// `review_progress.rs` files its entries under, so one canonical pull
/// request is one key to every reader of the shared configuration directory.
pub fn reference_key(reference: &PullRequestReference) -> String {
    let key = reference.key();
    let provider = match key.provider {
        crate::events::PullRequestProvider::Github => "github",
        crate::events::PullRequestProvider::Bitbucket => "bitbucket",
    };
    format!("{provider}/{}/{}/{}", key.owner, key.repo, key.number)
}

/// The whole persisted document: which drafts and journal records exist, by
/// their keys. `deny_unknown_fields` makes a document a later version wrote
/// unparseable here, preserving it rather than letting a rewrite narrow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionDocuments {
    /// [`DOCUMENT_VERSION`], which this build reads and writes.
    #[serde(default)]
    pub version: u32,
    /// The current drafts, by canonical reference key.
    #[serde(default)]
    pub drafts: BTreeMap<String, ActionDraft>,
    /// The operations, by their UUIDs.
    #[serde(default)]
    pub operations: BTreeMap<String, ActionOperation>,
}

impl ActionDocuments {
    /// The operation ids whose phase is live and so may be crash residue:
    /// written by a process that then went away. `normalise_unfinished` asks
    /// each one's execution lock before touching it.
    fn live_operation_ids(&self) -> Vec<String> {
        self.operations
            .values()
            .filter(|operation| operation.phase.is_live())
            .map(|operation| operation.id.clone())
            .collect()
    }

    /// The operation of `reference` that is not terminal, when there is
    /// exactly one. At most one unfinished operation may exist for a
    /// canonical pull request; two mean an invariant this store never lets
    /// arise, and the caller gets a failure instead of an arbitrary choice.
    pub(crate) fn unfinished_operation(
        &self,
        reference: &PullRequestReference,
    ) -> Result<Option<&ActionOperation>, ActionStoreFailure> {
        let unfinished: Vec<&ActionOperation> = self
            .operations
            .values()
            .filter(|operation| {
                operation.frozen_draft.reference == *reference && !operation.phase.is_terminal()
            })
            .collect();
        match unfinished.len() {
            0 => Ok(None),
            1 => Ok(unfinished.into_iter().next()),
            _ => Err(ActionStoreFailure::Local(format!(
                "{} unfinished operations for one canonical pull request; refusing to choose",
                unfinished.len()
            ))),
        }
    }

    /// The operation of `reference` an action-state read exposes when none
    /// is unfinished: the latest frozen revision, its id breaking a tie no
    /// real journal can create. Receipts stay inspectable after submission
    /// or abandonment without guessing which operation a window meant.
    pub(crate) fn last_operation(
        &self,
        reference: &PullRequestReference,
    ) -> Option<&ActionOperation> {
        self.operations
            .values()
            .filter(|operation| operation.frozen_draft.reference == *reference)
            .max_by(|left, right| {
                left.frozen_revision
                    .cmp(&right.frozen_revision)
                    .then_with(|| left.id.cmp(&right.id))
            })
    }
}

// ---------------------------------------------------------------- failures

/// Why a review-action store call did not persist what was asked:
/// `Corrupt` is the fail-closed `actionStoreCorrupt` boundary for a damaged
/// or unreadable journal, and `Local` is a failure of this process's own
/// I/O or of an invariant its re-checks found broken. Either way no provider
/// request the failed step was meant to precede is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionStoreFailure {
    /// `pull-request-actions.json` is damaged, pinned to another schema
    /// version this build cannot read, or is otherwise unrecognisable, and
    /// is preserved in place. Every draft or operation mutation refuses
    /// until the file is repaired or explicitly removed outside the action
    /// API.
    Corrupt(String),
    /// A local read, lock, write, flush or invariant failure. The file is
    /// left exactly as it was found.
    Local(String),
}

impl ActionStoreFailure {
    /// Whether this is the corruption boundary, for callers word it as
    /// `actionStoreCorrupt`.
    pub fn is_corrupt(&self) -> bool {
        matches!(self, Self::Corrupt(_))
    }
}

impl fmt::Display for ActionStoreFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corrupt(reason) => write!(
                f,
                "pull-request-actions.json is damaged and is preserved; review actions are blocked: {reason}"
            ),
            Self::Local(reason) => write!(
                f,
                "the pull-request action store could not be updated and sent nothing: {reason}"
            ),
        }
    }
}

impl std::error::Error for ActionStoreFailure {}

impl From<io::Error> for ActionStoreFailure {
    fn from(error: io::Error) -> Self {
        Self::Local(error.to_string())
    }
}

// ---------------------------------------------------------------- the store

/// The sole durable owner of `pull-request-actions.json`. One per service;
/// constructed from the shared configuration directory with [`Self::open`],
/// which reads nothing and creates nothing: starting the browser server or
/// terminal frontend — which also constructs a service — neither opens nor
/// normalises this store, and native action initialisation is the boundary
/// that does ([`Self::normalise_unfinished`]).
pub struct ReviewActionStore {
    path: PathBuf,
    /// Serialises this process's own transactions, so a transaction's
    /// re-read, check and replace never interleave with this process's
    /// other ones. The sidecar lock below spans processes; both are held
    /// together across a whole transaction.
    writing: Mutex<()>,
    /// `<path>.lock`, held across every transaction's re-read, check and
    /// atomic replacement.
    file_lock: PathLock,
    /// Parks the next replace, so a test can hold a transaction inside its
    /// file I/O.
    #[cfg(test)]
    replace_hook: Mutex<Option<tests::ReplaceHook>>,
    /// Injects one replace failure after N successful replaces in unit
    /// tests, the only deterministic way to see a durability failure refuse
    /// a mutation and leave the journal intact.
    #[cfg(test)]
    replace_failure_after: Mutex<Option<usize>>,
}

impl ReviewActionStore {
    /// The store at `path`, its durable owner. Nothing is read and no file
    /// is created here.
    pub fn open(path: PathBuf) -> Self {
        Self {
            file_lock: PathLock::for_resource(&path),
            writing: Mutex::default(),
            path,
            #[cfg(test)]
            replace_hook: Mutex::default(),
            #[cfg(test)]
            replace_failure_after: Mutex::default(),
        }
    }

    /// The sidecar lock that serialises one operation's work — its pending
    /// provider requests and every journal transition around them — within
    /// this process and between processes, keyed by the operation's UUID.
    /// A caller acquires it non-blockingly before the activity that may
    /// touch the provider ([`PathLock::try_lock`]) and holds the guard
    /// around the transactions that carry that activity; nothing here ever
    /// acquires a store transaction inside it in the other order.
    pub fn execution_lock(&self, operation: &str) -> PathLock {
        let mut name = self.path.clone().into_os_string();
        name.push(format!(".{operation}.execution"));
        PathLock::new(PathBuf::from(name))
    }

    /// One short read-check-replace transaction: takes this process's
    /// transaction mutex and the store's inter-process sidecar lock, reads
    /// the file anew, hands `f` the read documents and — exactly when `f`
    /// reports the documents changed — persists them atomically and durably
    /// (the staged file's fsync and the parent directory's, so a transition
    /// that may precede a provider mutation is only complete once durable;
    /// a failed flush refuses and sends nothing).
    ///
    /// Callers that hold an operation's execution lock make their every
    /// store transaction here, nested inside it. A declined transaction —
    /// an expected revision that no longer matches, an operation that moved
    /// on — returns its result without writing, so the file stays exactly
    /// as it was read. A corrupt file refuses every transaction with
    /// [`ActionStoreFailure::Corrupt`] and is preserved, never replaced
    /// with an empty one.
    pub fn transact<R>(
        &self,
        f: impl FnOnce(&mut ActionDocuments) -> (R, bool),
    ) -> Result<R, ActionStoreFailure> {
        let writing = self
            .writing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let file_guard = self.file_lock.lock()?;
        let mut documents = self.read_documents()?;
        let (result, changed) = f(&mut documents);
        if changed {
            self.replace_documents(&mut documents)?;
        }
        drop(file_guard);
        drop(writing);
        Ok(result)
    }

    /// The native-initialisation pass: every persisted `running`,
    /// `attempting` or `reconciling` operation whose sidecar lock no longer
    /// holds a live runner becomes `paused` with its journal intact, and a
    /// persisted live phase whose lock is still held by another process is
    /// left exactly as it is — that process owns the run and it is not
    /// crash residue. The operating-system lock released by a dead
    /// process's exit is the whole proof, with no timeout and no persisted
    /// process id. This pass never sends a provider request and never
    /// touches a draft.
    pub fn normalise_unfinished(&self) -> Result<Vec<String>, ActionStoreFailure> {
        let candidates = self.read_documents()?.live_operation_ids();
        let mut normalised = Vec::new();
        for id in candidates {
            let lock = self.execution_lock(&id);
            let guard = match lock.try_lock() {
                Ok(Some(guard)) => guard,
                Ok(None) => continue,
                Err(error) => return Err(error.into()),
            };
            self.transact(|documents| {
                let paused = match documents.operations.get_mut(&id) {
                    Some(operation) if operation.phase.is_live() => {
                        operation.phase = OperationPhase::Paused;
                        true
                    }
                    _ => false,
                };
                if paused {
                    normalised.push(id.clone());
                }
                ((), paused)
            })?;
            drop(guard);
        }
        Ok(normalised)
    }

    /// The current draft of `reference`, read from the file now.
    pub fn draft(
        &self,
        reference: &PullRequestReference,
    ) -> Result<Option<ActionDraft>, ActionStoreFailure> {
        self.read_documents()?
            .drafts
            .get(&reference_key(reference))
            .cloned()
            .map(Ok)
            .transpose()
    }

    /// The unfinished operation of `reference`, read from the file now: the
    /// one whose phase is live or paused, if any. Terminal operations are
    /// kept for their receipts and are not returned.
    pub fn unfinished_operation(
        &self,
        reference: &PullRequestReference,
    ) -> Result<Option<ActionOperation>, ActionStoreFailure> {
        let documents = self.read_documents()?;
        let unfinished = documents.unfinished_operation(reference)?;
        Ok(unfinished.cloned())
    }

    /// One operation by id, read from the file now.
    pub fn operation(&self, id: &str) -> Result<Option<ActionOperation>, ActionStoreFailure> {
        Ok(self.read_documents()?.operations.get(id).cloned())
    }

    /// The unfinished operation of `reference` when one exists, else the
    /// latest terminal one, as action state exposes it.
    pub fn last_operation(
        &self,
        reference: &PullRequestReference,
    ) -> Result<Option<ActionOperation>, ActionStoreFailure> {
        let documents = self.read_documents()?;
        Ok(documents
            .unfinished_operation(reference)?
            .cloned()
            .or_else(|| documents.last_operation(reference).cloned()))
    }

    /// One short mutation of one operation, nested inside the operation's
    /// execution lock by every caller: the operation is re-read under the
    /// store's transaction and the change is durable before it returns.
    /// A missing or terminal operation refuses without writing.
    fn change_operation(
        &self,
        id: &str,
        change: impl FnOnce(&mut ActionOperation) -> Result<(), String>,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.transact(|documents| match documents.operations.get_mut(id) {
            None => (
                Err(ActionStoreFailure::Local(format!(
                    "operation {id} is not in the journal"
                ))),
                false,
            ),
            Some(operation) if operation.phase.is_terminal() => (
                Err(ActionStoreFailure::Local(format!(
                    "operation {id} is already terminal"
                ))),
                false,
            ),
            Some(operation) => match change(operation) {
                Ok(()) => (Ok(operation.clone()), true),
                Err(reason) => (Err(ActionStoreFailure::Local(reason)), false),
            },
        })
        .and_then(|changed| changed)
    }

    /// Durably records that `step` is about to be attempted: its ledger
    /// entry becomes `uncertain` — the durable attempting marker a crash or
    /// an ambiguous reply leaves behind — and the phase becomes `attempting`.
    /// A step already confirmed or failed refuses, so no write ever
    /// overwrites a settled outcome.
    pub fn journal_attempt(
        &self,
        id: &str,
        step: &LedgerStep,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            match operation.ledger.iter().find(|entry| entry.step == *step) {
                Some(entry) if entry.outcome != LedgerOutcome::Uncertain => {
                    return Err(format!(
                        "step {step:?} of operation {id} is already settled"
                    ));
                }
                Some(_) => {}
                None => operation.ledger.push(LedgerEntry {
                    step: step.clone(),
                    outcome: LedgerOutcome::Uncertain,
                }),
            }
            operation.phase = OperationPhase::Attempting;
            operation.last_error = None;
            Ok(())
        })
    }

    /// Durably records `step`'s settled or uncertain outcome and the phase
    /// the run is left in. An absent entry is appended at the ledger's end —
    /// the summary and decision are only ever recorded there.
    pub fn journal_outcome(
        &self,
        id: &str,
        step: &LedgerStep,
        outcome: LedgerOutcome,
        phase: OperationPhase,
        last_error: Option<String>,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            match operation
                .ledger
                .iter_mut()
                .find(|entry| entry.step == *step)
            {
                Some(entry) => entry.outcome = outcome,
                None => operation.ledger.push(LedgerEntry {
                    step: step.clone(),
                    outcome,
                }),
            }
            operation.phase = phase;
            operation.last_error = last_error;
            Ok(())
        })
    }

    /// A step proven absent by a complete reconciliation read: its durable
    /// attempting marker is removed, leaving the step eligible for a fresh
    /// explicit Resume, and the operation is paused. A settled outcome is
    /// never removed.
    pub fn journal_absent(
        &self,
        id: &str,
        step: &LedgerStep,
        last_error: Option<String>,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            match operation
                .ledger
                .iter()
                .position(|entry| entry.step == *step)
            {
                Some(at) if operation.ledger[at].outcome == LedgerOutcome::Uncertain => {
                    operation.ledger.remove(at);
                }
                Some(_) => {
                    return Err(format!(
                        "step {step:?} of operation {id} is already settled"
                    ))
                }
                None => {}
            }
            operation.phase = OperationPhase::Paused;
            operation.last_error = last_error;
            Ok(())
        })
    }

    /// The operation's phase and local failure sentence, recorded without
    /// touching the ledger.
    pub fn journal_phase(
        &self,
        id: &str,
        phase: OperationPhase,
        last_error: Option<String>,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            operation.phase = phase;
            operation.last_error = last_error;
            Ok(())
        })
    }

    /// The GitHub pending review this operation created or adopted, recorded
    /// before any content write may name it.
    pub fn journal_review_id(
        &self,
        id: &str,
        review_id: &str,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            operation.pending_review_id = Some(review_id.to_string());
            Ok(())
        })
    }

    /// A completed submission's one durable transition: the decision — and
    /// the summary the submit carried with it — stand confirmed, the phase
    /// is terminal, and the receipt names the provider's review id. One
    /// transaction, so no crash between the ledger and the phase can lose
    /// the submission or invent a second one.
    pub fn journal_submitted(
        &self,
        id: &str,
        review_id: Option<String>,
        decision: Option<ReviewDecision>,
        summary_present: bool,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        self.change_operation(id, |operation| {
            let settle = |ledger: &mut Vec<LedgerEntry>, step: LedgerStep, provider_id| match ledger
                .iter_mut()
                .find(|entry| entry.step == step)
            {
                Some(entry) => entry.outcome = LedgerOutcome::Confirmed { provider_id },
                None => ledger.push(LedgerEntry {
                    step,
                    outcome: LedgerOutcome::Confirmed { provider_id },
                }),
            };
            if decision.is_some() {
                settle(
                    &mut operation.ledger,
                    LedgerStep::Decision,
                    review_id.clone(),
                );
            }
            if summary_present {
                settle(&mut operation.ledger, LedgerStep::Summary, None);
            }
            operation.phase = OperationPhase::Submitted;
            operation.receipt = Some(ReviewReceipt {
                review_id,
                decision,
                remaining_pending_review: None,
            });
            operation.last_error = None;
            Ok(())
        })
    }

    /// The operation's terminal transition: its receipt is kept whole, and
    /// every confirmed or uncertain ledger entry stays with it.
    pub fn journal_finish(
        &self,
        id: &str,
        phase: OperationPhase,
        receipt: ReviewReceipt,
        last_error: Option<String>,
    ) -> Result<ActionOperation, ActionStoreFailure> {
        if !phase.is_terminal() {
            return Err(ActionStoreFailure::Local(format!(
                "operation {id} cannot finish in phase {phase:?}"
            )));
        }
        self.change_operation(id, |operation| {
            operation.phase = phase;
            operation.receipt = Some(receipt);
            operation.last_error = last_error;
            Ok(())
        })
    }

    /// The stored document, parsed fresh: an absent file is an empty
    /// document, and anything else must parse as this version's schema.
    fn read_documents(&self) -> Result<ActionDocuments, ActionStoreFailure> {
        match read_if_present(&self.path)? {
            None => Ok(ActionDocuments::default()),
            Some(raw) => {
                let documents: ActionDocuments = serde_json::from_slice(&raw)
                    .map_err(|error| ActionStoreFailure::Corrupt(error.to_string()))?;
                if documents.version != DOCUMENT_VERSION {
                    return Err(ActionStoreFailure::Corrupt(format!(
                        "document names version {}, and this build writes {}",
                        documents.version, DOCUMENT_VERSION
                    )));
                }
                Ok(documents)
            }
        }
    }

    /// Atomically replaces the file with `documents`, cross its durability
    /// barrier: the staged replacement's fsync and the parent directory's.
    /// The store pins the schema version itself, so no caller can forget it.
    fn replace_documents(&self, documents: &mut ActionDocuments) -> Result<(), ActionStoreFailure> {
        documents.version = DOCUMENT_VERSION;
        #[cfg(test)]
        {
            let hook = self
                .replace_hook
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            if let Some(hook) = hook {
                hook.park();
            }
            let mut failure = self
                .replace_failure_after
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(remaining) = failure.as_mut() {
                if *remaining == 0 {
                    *failure = None;
                    return Err(ActionStoreFailure::Local(
                        "injected replace failure".to_string(),
                    ));
                }
                *remaining -= 1;
            }
        }
        let raw = serde_json::to_vec_pretty(documents)
            .map_err(|error| ActionStoreFailure::Local(error.to_string()))?;
        write_atomic(&self.path, &raw)?;
        Ok(())
    }
}

/// A stable, backend-generated record identifier: the UUID v4 grammar the
/// store's records are created under, never an array index or a provider id.
pub fn new_record_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use crate::events::PullRequestProvider;
    use crate::pull_request_detail::DiffSide;
    use crate::review_actions::{
        CommitBasis, DraftItem, LedgerOutcome, LedgerStep, ReviewDecision, ReviewReceipt,
    };
    use crate::service::AppService;

    /// Generous: a correct build answers at once, so this bound is only ever
    /// reached by a broken one, which then fails rather than hangs.
    const GENEROUS: Duration = Duration::from_secs(30);
    /// How long a test waits to learn that a blocked peer has not completed.
    const WAITED: Duration = Duration::from_millis(200);

    /// Parks `store`'s next replacement inside its file I/O, so a test can
    /// hold one transaction at its sidecar lock (the settings store's hook
    /// shape).
    pub(super) struct ReplaceHook {
        entered: Sender<()>,
        release: Receiver<()>,
    }

    impl ReplaceHook {
        pub(super) fn park(self) {
            let _ = self.entered.send(());
            let _ = self.release.recv();
        }
    }

    /// Injects one replace failure after `successful` successful replaces,
    /// the only deterministic way to see a durability failure refuse a
    /// transaction and leave the journal intact.
    fn fail_replace_after(store: &ReviewActionStore, successful: usize) {
        *store
            .replace_failure_after
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(successful);
    }

    fn reference() -> PullRequestReference {
        PullRequestReference {
            provider: PullRequestProvider::Github,
            owner: "Acme".to_string(),
            repo: "API".to_string(),
            number: 42,
        }
    }

    fn basis() -> CommitBasis {
        CommitBasis {
            head: "c".repeat(40),
            base: "b".repeat(40),
        }
    }

    fn draft(id: &str, revision: u64) -> ReviewDraft {
        ReviewDraft {
            id: id.to_string(),
            reference: reference(),
            revision,
            basis: basis(),
            decision: ReviewDecision::Comment,
            summary: "About today's commits.".to_string(),
            items: vec![DraftItem::PlacedInline {
                id: "i1".to_string(),
                body: "A current line lies.".to_string(),
                path: "src/api.ts".to_string(),
                side: DiffSide::New,
                line: 27,
            }],
        }
    }

    /// The stamp a test uses: a 64-character hex digest and a stable actor id.
    fn stamp() -> ActionAuthorisationStamp {
        ActionAuthorisationStamp {
            fingerprint: "a3".repeat(32),
            actor: "gh-actor-1".to_string(),
        }
    }

    fn stamp_record() -> ActionAuthorisationStamp {
        ActionAuthorisationStamp {
            fingerprint: "b7".repeat(32),
            actor: "gh-actor-1".to_string(),
        }
    }

    /// An operation persisted by an earlier session: `running`, frozen at
    /// revision 3, with one confirmed and one ambiguous write.
    fn running_operation() -> ActionOperation {
        ActionOperation {
            id: "o-1".to_string(),
            draft_id: "d-7".to_string(),
            frozen_revision: 3,
            frozen_draft: draft("d-7", 3),
            stamp: stamp_record(),
            phase: OperationPhase::Running,
            ledger: vec![
                LedgerEntry {
                    step: LedgerStep::InlineComment {
                        item: "i1".to_string(),
                    },
                    outcome: LedgerOutcome::Confirmed {
                        provider_id: Some("1002".to_string()),
                    },
                },
                LedgerEntry {
                    step: LedgerStep::Reply {
                        item: "i2".to_string(),
                    },
                    outcome: LedgerOutcome::Uncertain,
                },
            ],
            last_error: None,
            pending_review_id: None,
            receipt: None,
        }
    }

    fn store_at(dir: &Path) -> (PathBuf, ReviewActionStore) {
        let path = dir.join("pull-request-actions.json");
        (path.clone(), ReviewActionStore::open(path))
    }

    fn file_bytes(path: &Path) -> String {
        fs::read_to_string(path).expect("the journal file exists")
    }

    fn store_draft(store: &ReviewActionStore, stored: &ActionDraft) {
        store
            .transact(|documents| {
                documents
                    .drafts
                    .insert(reference_key(&reference()), stored.clone());
                ("stored", true)
            })
            .unwrap();
    }

    fn store_operation(store: &ReviewActionStore, operation: &ActionOperation) {
        store
            .transact(|documents| {
                documents
                    .operations
                    .insert(operation.id.clone(), operation.clone());
                ("stored", true)
            })
            .unwrap();
    }

    /// Stores an accepted edit of the draft from `expected`' revision to
    /// `next`, and reports the current revision when the expectation no
    /// longer matches — the shape every draft mutation takes.
    fn apply_revision(
        store: &ReviewActionStore,
        expected: u64,
        next: u64,
    ) -> Result<String, ActionStoreFailure> {
        store.transact(|documents| {
            let key = reference_key(&reference());
            let current = documents.drafts.get(key.as_str()).unwrap().draft.revision;
            if current == expected {
                documents
                    .drafts
                    .get_mut(key.as_str())
                    .unwrap()
                    .draft
                    .revision = next;
                (String::from("applied"), true)
            } else {
                (current.to_string(), false)
            }
        })
    }

    /// The draft revision currently on disk, read off the file.
    fn revision_now(path: &Path) -> u64 {
        let wire: serde_json::Value = serde_json::from_str(&file_bytes(path)).unwrap();
        wire["drafts"][reference_key(&reference())]["draft"]["revision"]
            .as_u64()
            .unwrap()
    }

    fn sorted_keys(value: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    #[test]
    fn constructing_the_store_touches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        assert!(!path.exists(), "opening the store creates no file");

        // Reads of an absent file answer the empty document — and still
        // create nothing: starting the browser server or terminal frontend,
        // which also constructs a service, neither opens nor normalises
        // this store. Native action initialisation is the only pass that
        // does.
        assert_eq!(store.draft(&reference()).unwrap(), None);
        assert_eq!(store.unfinished_operation(&reference()).unwrap(), None);
        assert!(!path.exists(), "an absent store stays absent until a write");
    }

    #[test]
    fn a_draft_survives_a_restart_with_its_identity() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        let stored = ActionDraft {
            draft: draft("d-7", 4),
            stamp: stamp(),
        };
        store_draft(&store, &stored);

        // A second store instance on the same path — the next process —
        // reads the same UUID, revision and content; nothing a detail cache
        // does can reach this file.
        let reopened = ReviewActionStore::open(path);
        assert_eq!(reopened.draft(&reference()).unwrap(), Some(stored));
    }

    #[test]
    fn a_conflicting_expected_revision_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        store_draft(
            &store,
            &ActionDraft {
                draft: draft("d-7", 5),
                stamp: stamp(),
            },
        );
        let before = file_bytes(&path);

        // A window that read revision 4 asks for a change; the store's
        // revision is 5. The decline returns the current revision to re-read
        // and leaves the file byte-identical.
        let outcome = store
            .transact(|documents| {
                let key = reference_key(&reference());
                let current = documents.drafts.get(key.as_str()).unwrap().draft.revision;
                if current == 4 {
                    documents
                        .drafts
                        .get_mut(key.as_str())
                        .unwrap()
                        .draft
                        .revision = 12;
                    (String::from("applied"), true)
                } else {
                    (current.to_string(), false)
                }
            })
            .unwrap();
        assert_eq!(outcome, "5", "the decline returns the revision to re-read");
        assert_eq!(
            file_bytes(&path),
            before,
            "a declined transaction writes nothing"
        );
    }

    #[test]
    fn concurrent_transactions_commit_one_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        store_draft(
            &store,
            &ActionDraft {
                draft: draft("d-7", 5),
                stamp: stamp(),
            },
        );
        let store = Arc::new(store);

        // One process' transaction parks inside its own replace, holding
        // the journal's sidecar lock, and edits revision 5 to 6.
        let (entered, arrived) = mpsc::channel();
        let (release, released) = mpsc::channel();
        *store.replace_hook.lock().unwrap() = Some(ReplaceHook {
            entered,
            release: released,
        });
        let parked = {
            let store = store.clone();
            thread::spawn(move || apply_revision(&store, 5, 6))
        };
        arrived
            .recv_timeout(GENEROUS)
            .expect("the first transaction reaches its replace");

        // A second process' transaction waits for the sidecar lock and
        // cannot commit from the same on-disk revision.
        let waiting = Arc::new(ReviewActionStore::open(path.clone()));
        let (second_done, second) = mpsc::channel::<Result<String, ActionStoreFailure>>();
        thread::spawn({
            let waiting = waiting;
            move || {
                let _ = second_done.send(apply_revision(&waiting, 5, 9));
            }
        });
        assert!(
            second.recv_timeout(WAITED).is_err(),
            "the second transaction must not commit from the parked revision"
        );

        release.send(()).unwrap();
        assert_eq!(
            parked.join().unwrap().unwrap(),
            "applied",
            "the parked transaction, released, commits first"
        );
        assert_eq!(
            second.recv_timeout(GENEROUS).unwrap().unwrap(),
            "6",
            "the other re-reads the winner and does not apply from it"
        );
        assert_eq!(revision_now(&path), 6, "revision 9 never enters the file");
    }

    #[test]
    fn a_live_runner_is_not_mistaken_for_crash_residue() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        store_operation(&store, &running_operation());
        let store = Arc::new(store);

        // Another process holds the operation's execution lock: its run is
        // live, and native action initialisation leaves the phase exactly
        // as persisted, without reconciling, resuming or abandoning it.
        let peer = ReviewActionStore::open(path.clone());
        let lock = peer.execution_lock("o-1");
        let held = lock.lock().unwrap();
        assert_eq!(store.normalise_unfinished().unwrap(), Vec::<String>::new());
        assert_eq!(
            store
                .unfinished_operation(&reference())
                .unwrap()
                .as_ref()
                .map(|operation| operation.phase),
            Some(OperationPhase::Running)
        );

        // With the lock released, the following native pass normalises:
        // the operating-system lock release is the whole proof that no
        // live runner is left.
        drop(held);
        assert_eq!(
            store.normalise_unfinished().unwrap(),
            vec![String::from("o-1")]
        );
        assert_eq!(
            store
                .unfinished_operation(&reference())
                .unwrap()
                .as_ref()
                .map(|operation| operation.phase),
            Some(OperationPhase::Paused)
        );
    }

    #[test]
    fn restart_keeps_an_unfinished_operation_then_pauses_the_residue() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        let operation = running_operation();
        store_operation(&store, &operation);

        // The next session's store instance reads the same operation id,
        // frozen revision and ledger: unsubmitted work survives a restart.
        let reopened = ReviewActionStore::open(path.clone());
        let restarted = reopened
            .unfinished_operation(&reference())
            .unwrap()
            .unwrap();
        assert_eq!(restarted, operation);
        assert_eq!(restarted.phase, OperationPhase::Running);

        // Native initialisation normalises the residue to paused, taking
        // nothing away from the frozen content and the ledger.
        assert_eq!(
            reopened.normalise_unfinished().unwrap(),
            vec![String::from("o-1")]
        );
        let paused = reopened
            .unfinished_operation(&reference())
            .unwrap()
            .unwrap();
        assert_eq!(paused.phase, OperationPhase::Paused);
        assert_eq!(paused.ledger, operation.ledger);
        assert_eq!(paused.frozen_draft, operation.frozen_draft);
    }

    #[test]
    fn a_failed_flush_sends_nothing_and_leaves_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        store_draft(
            &store,
            &ActionDraft {
                draft: draft("d-7", 5),
                stamp: stamp(),
            },
        );
        let before = file_bytes(&path);

        // The replace's durability barrier fails: the transaction fails as
        // a local failure and the journal stays exactly as it was, so no
        // provider mutation this transition preceded is sent.
        fail_replace_after(&store, 0);
        let failed = apply_revision(&store, 5, 6).unwrap_err();
        assert!(matches!(failed, ActionStoreFailure::Local(_)));
        assert_eq!(file_bytes(&path), before);
        assert_eq!(revision_now(&path), 5);

        // The failure is local, not corruption: the next transaction
        // re-reads the intact journal and applies.
        let retried = apply_revision(&store, 5, 6).unwrap();
        assert_eq!(retried, "applied");
        assert_eq!(revision_now(&path), 6);
    }

    #[test]
    fn corrupt_input_is_preserved_and_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        fs::write(&path, "{ damaged").unwrap();
        let before = file_bytes(&path);

        assert!(store.draft(&reference()).unwrap_err().is_corrupt());
        assert!(store
            .unfinished_operation(&reference())
            .unwrap_err()
            .is_corrupt());
        let refused = store
            .transact(|documents| {
                documents.drafts.insert(
                    String::from("github/acme/api/42"),
                    ActionDraft {
                        draft: draft("d-1", 1),
                        stamp: stamp(),
                    },
                );
                ("stored", true)
            })
            .unwrap_err();
        assert!(refused.is_corrupt(), "{refused}");
        assert!(store.normalise_unfinished().unwrap_err().is_corrupt());
        assert_eq!(
            file_bytes(&path),
            before,
            "the damaged journal is preserved in place, not replaced"
        );

        // Repair happens outside the action API: the file is removed. Only
        // now does the store work again — never by starting empty earlier.
        fs::remove_file(&path).unwrap();
        assert_eq!(store.draft(&reference()).unwrap(), None);
        store_draft(
            &store,
            &ActionDraft {
                draft: draft("d-1", 1),
                stamp: stamp(),
            },
        );
        assert_eq!(revision_now(&path), 1);
    }

    #[test]
    fn a_document_this_version_cannot_read_is_preserved_too() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());

        // A document a future version wrote is preserved as corrupt input,
        // so this build never rewrites meaning it does not understand.
        fs::write(&path, r#"{"version": 2, "drafts": {}, "operations": {}}"#).unwrap();
        let before = file_bytes(&path);
        assert!(store.draft(&reference()).unwrap_err().is_corrupt());
        assert_eq!(file_bytes(&path), before);

        // So is a current-version document carrying keys this schema does
        // not know: a rewrite would silently narrow it.
        fs::write(
            &path,
            r#"{"version": 1, "drafts": {}, "operations": {}, "extra": 1}"#,
        )
        .unwrap();
        let before = file_bytes(&path);
        assert!(store.draft(&reference()).unwrap_err().is_corrupt());
        assert_eq!(file_bytes(&path), before);
    }

    #[test]
    fn a_terminal_operation_serves_its_receipt_only() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        let mut operation = running_operation();
        operation.phase = OperationPhase::Submitted;
        operation.receipt = Some(ReviewReceipt {
            review_id: Some("PRR_1".to_string()),
            decision: Some(ReviewDecision::Approve),
            remaining_pending_review: None,
        });
        store_operation(&store, &operation);

        // Terminal operations are not unfinished work; their receipts stay
        // in the journal for the reviewer to inspect.
        assert_eq!(store.unfinished_operation(&reference()).unwrap(), None);
        let wire: serde_json::Value = serde_json::from_str(&file_bytes(&path)).unwrap();
        assert_eq!(wire["operations"]["o-1"]["receipt"]["decision"], "approve");
    }

    #[test]
    fn two_unfinished_operations_for_a_reference_are_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let (_, store) = store_at(dir.path());
        let mut second = running_operation();
        second.id = String::from("o-2");
        store_operation(&store, &running_operation());
        store_operation(&store, &second);
        let failed = store.unfinished_operation(&reference()).unwrap_err();
        assert!(matches!(failed, ActionStoreFailure::Local(_)), "{failed}");
    }

    #[test]
    fn the_journal_persists_its_schema_and_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let (path, store) = store_at(dir.path());
        store_draft(
            &store,
            &ActionDraft {
                draft: draft("d-7", 4),
                stamp: stamp(),
            },
        );
        store_operation(&store, &running_operation());

        let wire: serde_json::Value = serde_json::from_str(&file_bytes(&path)).unwrap();
        let file = file_bytes(&path);
        assert_eq!(
            sorted_keys(&wire),
            ["drafts", "operations", "version"].map(str::to_string)
        );
        assert_eq!(wire["version"], 1);
        assert_eq!(
            sorted_keys(&wire["drafts"][reference_key(&reference())]),
            ["draft", "stamp"].map(str::to_string)
        );
        assert_eq!(
            sorted_keys(&wire["operations"]["o-1"]),
            [
                "draftId",
                "frozenDraft",
                "frozenRevision",
                "id",
                "lastError",
                "ledger",
                "pendingReviewId",
                "phase",
                "receipt",
                "stamp"
            ]
            .map(str::to_string)
        );
        // The fingerprint is the digest's hex, exactly 64 characters, and no
        // token, password or authorisation header is anywhere in the file.
        let fingerprint = wire["drafts"][reference_key(&reference())]["stamp"]["fingerprint"]
            .as_str()
            .unwrap();
        assert_eq!(fingerprint.len(), 64);
        assert!(fingerprint
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        for sample in ["ghp_", "github_pat_", "Bearer ", "password", "token"] {
            assert!(!file.contains(sample), "{sample} reached the journal");
        }
    }

    /// The settings store's own sidecar lock must be acquirable while an
    /// operation's execution lock is held: durable credential state is
    /// resolved and released under the settings lock BEFORE a store
    /// transaction, and the two are never held simultaneously. Were any code
    /// path to take the settings lock inside the execution region, this
    /// would deadlock instead of completing.
    #[test]
    fn an_execution_lock_never_holds_the_settings_lock() {
        let dir = tempfile::tempdir().unwrap();
        let service = AppService::bootstrap(dir.path().to_path_buf());
        let scratch = tempfile::tempdir().unwrap();
        let (_, store) = store_at(scratch.path());

        let lock = store.execution_lock("o-1");
        let _guard = lock.lock().unwrap();
        let (done, done_rx) = mpsc::channel::<std::io::Result<()>>();
        thread::spawn({
            let settings = service.settings.clone();
            move || {
                let _ = done.send(settings.set_wsl_poll_interval_secs(900));
            }
        });
        done_rx
            .recv_timeout(GENEROUS)
            .expect("a settings change completes while an operation's execution lock is held")
            .expect("the change applies");
    }

    #[test]
    fn bootstrapping_a_service_neither_opens_nor_normalises_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        let journal = dir.path().join("pull-request-actions.json");
        let seeded = serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "drafts": {},
            "operations": {
                "o-1": {
                    "id": "o-1",
                    "draftId": "d-7",
                    "frozenRevision": 3,
                    "frozenDraft": {
                        "id": "d-7",
                        "reference": {
                            "provider": "github",
                            "owner": "Acme",
                            "repo": "API",
                            "number": 42
                        },
                        "revision": 3,
                        "basis": {"head": basis().head, "base": basis().base},
                        "decision": "comment",
                        "summary": "Earlier.",
                        "items": []
                    },
                    "stamp": {"actor": stamp().actor, "fingerprint": stamp().fingerprint},
                    "phase": "running",
                    "ledger": [],
                    "lastError": null,
                    "receipt": null
                }
            }
        }))
        .unwrap();
        fs::write(&journal, &seeded).unwrap();

        // The browser server and the terminal frontend construct a service
        // on startup, too: startup must neither open nor normalise this
        // store, so the persisted live phase stays exactly as it is and no
        // provider request ever leaves startup.
        let service = AppService::bootstrap(dir.path().to_path_buf());
        assert_eq!(fs::read(&journal).unwrap(), seeded, "startup wrote nothing");
        assert_eq!(
            service
                .action_store
                .unfinished_operation(&reference())
                .unwrap()
                .as_ref()
                .map(|operation| operation.phase),
            Some(OperationPhase::Running)
        );

        // The desktop's native pass — the only boundary that may — then
        // normalises the residue, and everything the restart kept is intact.
        assert_eq!(
            service.action_store.normalise_unfinished().unwrap(),
            vec![String::from("o-1")]
        );
        assert_eq!(
            service
                .action_store
                .unfinished_operation(&reference())
                .unwrap()
                .as_ref()
                .map(|operation| operation.phase),
            Some(OperationPhase::Paused)
        );
    }

    #[test]
    fn detail_eviction_leaves_the_journal_alone() {
        let dir = tempfile::tempdir().unwrap();
        let service = AppService::bootstrap(dir.path().to_path_buf());
        let stored = ActionDraft {
            draft: draft("d-7", 4),
            stamp: stamp(),
        };
        store_draft(&service.action_store, &stored);

        // The detail LRU's eviction — what retrying a provider's cached
        // state after a credential change does — must not reach this file:
        // drafts and journals are not part of that cache.
        service
            .pull_request_details
            .forget(PullRequestProvider::Github);
        assert_eq!(
            service.action_store.draft(&reference()).unwrap(),
            Some(stored),
            "same UUID, revision and content"
        );
    }
}
