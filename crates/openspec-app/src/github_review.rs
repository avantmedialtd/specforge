//! The GitHub review protocol, over one pending review owned by the
//! stamped actor, pinned to the frozen head commit, and identifiable by an
//! invisible operation marker
//! (`pull-request-actions`: *GitHub Submission Owns a Commit-Pinned
//! Pending Review*; design D5): the fixed GraphQL documents, the markers
//! and their parsing, what a pending-state read answers, what may adopt
//! its reviews, and one creation.
//!
//! Every document here is fixed at build time: the token travels to one
//! endpoint, 3.1's own `https://api.github.com/graphql`
//! (`github-pull-requests`: *GitHub Privacy and Safety*), and no runtime
//! value — not a node id, a commit, a marker or a cursor — is ever written
//! into a document's text. A request is its fixed document beside a
//! variables object, and that whole wire body is all
//! [`crate::review_action_io::GithubTransport`] sends.
//!
//! What this module carries, and what the operation runner composes from
//! it:
//!
//! - **the markers** — an invisible operation marker, spelling one
//!   operation UUID and carried exactly once inside a pending review's
//!   body for as long as it is pending; and an invisible item marker,
//!   spelling an operation UUID and one item UUID, carried inside one
//!   provisional comment or reply body until that item is confirmed and
//!   cleaned. Nothing reads a marker against almost-matching text: the
//!   same tags and separators, one spelling of each, or nothing.
//! - **the pending-state read** — one 100-object page of a pull request's
//!   pending reviews and, under each, one 100-object page of its
//!   comments, the connections' cursors riding variables so a bounded
//!   pass may continue without a new document. A page that answers more
//!   than its bound is inconclusive, and says nothing.
//! - **adoption** — a marked review is adopted for this operation only
//!   when it belongs to the stamped actor and its body, comments and
//!   replies consist solely of the objects its ledger names. Anything
//!   extra in it — whether added through github.com or another client, or
//!   beyond its ledger's markers and recorded ids — is foreign; a pending
//!   review that carries no marker of this operation is unrelated.
//!   Nothing under either pause — not the marked one, not the unrelated
//!   one — is adopted, submitted, edited or deleted.
//! - **creation** — one owned pending review, for the exact pull-request
//!   node and exact frozen head OID, whose body is the operation marker
//!   and nothing else, so whatever its contents beyond the ledger turn
//!   out to hold can have come from no source but its own writes.
//! - **the item writes** — one inline comment and one reply at a time,
//!   in their recorded order, each provisional body its item marker
//!   beside the frozen clean prose; a write may repeat only after a
//!   marker search has proven its item absent, and one found marker
//!   confirms it without writing again.
//! - **the cleanups** — each confirmed item's marker replaced, one edit
//!   per comment, by the clean prose the provisional body round-tripped
//!   to; the review-level operation marker stays for as long as the
//!   review is pending.
//! - **the submission** — one `submitPullRequestReview` over the
//!   adopted review, whose body variable is the clean summary and whose
//!   event is the frozen decision; a lost reply reconciles by reading
//!   the stable review's state.
//! - **the abandon** — one exact adopted review's deletion, only
//!   through the same gate, an ambiguous result reconciled by a bounded
//!   read and never repeated blindly.
//!
//! Replies reduce through one GraphQL reply classifier: a 200 whose
//! `errors` name `RATE_LIMITED` is rate-limited as far as this reply
//! goes, a 200 whose errors name an authorisation failure is that
//! fixed-sentence refusal, a `UNPROCESSABLE` error is the stale anchor,
//! and anything else a reply's errors or lack of them name leaves its
//! step unresolved until a reconciliation read decides
//! (`pull-request-actions`: *Review Operations Respect Provider Failure
//! Boundaries*, the GraphQL half 3.1's transport does not read). A reply's
//! body travels no further than the one parser that asked for it: the
//! journal and events are built from parsed stable ids, never bytes.

use serde_json::{json, Value};

use crate::github::GithubLimits;
use crate::pull_request_detail::DiffSide;
use crate::review_action_io::{
    page_objects, ActionFlow, ActionIo, ActionVerdict, CredentialRefused, GithubAuthority,
    GithubTransport, PageBudget,
};
use crate::review_actions::BlockedReason;

// ---- the markers ----

/// The invisible separator a marker is written with: U+2063, invisible to
/// every renderer a reviewer reads one with, and written exactly once
/// inside the marker's own text.
pub const MARKER_SEPARATOR: char = '\u{2063}';

/// The operation marker's own text: inside the pending review's body for
/// as long as it is pending, and nowhere else.
pub fn operation_marker(operation_id: &str) -> String {
    format!("{MARKER_SEPARATOR}SpecForge-operation:{operation_id}{MARKER_SEPARATOR}")
}

/// The item marker's own text: inside one provisional comment or reply
/// body until that item is confirmed and cleaned.
pub fn item_marker(operation_id: &str, item_id: &str) -> String {
    format!("{MARKER_SEPARATOR}SpecForge-item:{operation_id}:{item_id}{MARKER_SEPARATOR}")
}

/// How many times a marker occurs in one body, as the protocol reads it
/// (`pull-request-actions`: *GitHub Submission Owns a Commit-Pinned
/// Pending Review*): a marker found exactly once confirms; a duplicate
/// marker, like any count besides a trustworthy one or none, is
/// inconclusive, and a body so written proves nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occurrences {
    None,
    One,
    Ambiguous,
}

/// Counts `marker` in `body`: [`Occurrences::One`] for one, and
/// [`Occurrences::Ambiguous`] for two or more.
pub fn occurrences(body: &str, marker: &str) -> Occurrences {
    if marker.is_empty() {
        return Occurrences::Ambiguous;
    }
    match body.matches(marker).count() {
        0 => Occurrences::None,
        1 => Occurrences::One,
        _ => Occurrences::Ambiguous,
    }
}

// ---- the fixed documents and their request bodies ----

/// Reads one pull request's pending reviews: one 100-object page of the
/// reviews connection and, under each review of it, one 100-object page
/// of its comments, the continuations' cursors riding `$reviewsAfter` and
/// `$commentsAfter`, never this document's text. A read-only query, fixed
/// at build time.
pub const PENDING_STATE_QUERY: &str = r#"query SpecForgePendingReviews($nodeId: ID!, $reviewsAfter: String, $commentsAfter: String) {
  node(id: $nodeId) {
    ... on PullRequest {
      id
      reviews(first: 100, after: $reviewsAfter, states: PENDING) {
        nodes {
          id
          author { __typename ... on User { id login } }
          body
          comments(first: 100, after: $commentsAfter) {
            nodes {
              id
              body
            }
            pageInfo { hasNextPage endCursor }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}"#;

/// Creates one owned pending review: the pull request
/// `$input.pullRequestId` names and the frozen head `$input.commitOID`
/// names, body `$input.body` — the operation marker and nothing else —
/// and `$input.clientMutationId` for the reply's own correlation, which
/// this protocol deduplicates by marker-bearing reads, never by it. Fixed
/// at build time.
pub const CREATE_REVIEW_MUTATION: &str = r#"mutation SpecForgeCreatePendingReview($input: AddPullRequestReviewInput!) {
  addPullRequestReview(input: $input) {
    pullRequestReview { id }
  }
}"#;

/// The runtime values of the pending-state read. A page's cursors are the
/// ones its ask carried; `None` means the connection's first page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVariables {
    /// The pull request's node id, as the trusted row named it.
    pub node_id: String,
    /// The reviews connection's continuation cursor, if asked.
    pub reviews_after: Option<String>,
    /// One review's comments connection's continuation cursor, if asked.
    pub comments_after: Option<String>,
}

impl PendingVariables {
    /// The connections' first page, for the pull request the row named.
    pub fn first(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            reviews_after: None,
            comments_after: None,
        }
    }
}

impl From<&PendingVariables> for Value {
    fn from(variables: &PendingVariables) -> Self {
        json!({
            "nodeId": variables.node_id,
            "reviewsAfter": variables.reviews_after.clone(),
            "commentsAfter": variables.comments_after.clone(),
        })
    }
}

/// The runtime values of the create. `body` is the operation marker and
/// nothing else, and `head_oid` is the frozen basis's own head, so a
/// review is never created against a later head than the operation it
/// continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateVariables {
    /// The pull request's node id, as the trusted row named it.
    pub pull_request_node_id: String,
    /// The head OID the frozen basis fixes.
    pub head_oid: String,
    /// The body the review is created with — always the operation marker.
    pub body: String,
    /// A client mutation id, for the reply's own correlation only.
    pub client_mutation_id: String,
}

impl CreateVariables {
    /// The create's values, for the operation marker the runner spelled.
    pub fn new(
        pull_request_node_id: impl Into<String>,
        head_oid: impl Into<String>,
        body: String,
        client_mutation_id: impl Into<String>,
    ) -> Self {
        Self {
            pull_request_node_id: pull_request_node_id.into(),
            head_oid: head_oid.into(),
            body,
            client_mutation_id: client_mutation_id.into(),
        }
    }
}

impl From<&CreateVariables> for Value {
    fn from(variables: &CreateVariables) -> Self {
        json!({
            "input": {
                "pullRequestId": variables.pull_request_node_id,
                "commitOID": variables.head_oid,
                "body": variables.body,
                "clientMutationId": variables.client_mutation_id,
            }
        })
    }
}

/// The wire body one request carries: its fixed document beside its
/// variables object. The document is never edited at runtime, and a
/// runtime value is never spelt into it
/// (`github-pull-requests`: *GitHub Privacy and Safety*).
pub fn request(document: &str, variables: Value) -> String {
    json!({ "query": document, "variables": variables }).to_string()
}

// ---- the GraphQL reply classifier ----

/// What one reply of the fixed documents is, as far as its status, its
/// headers and one error set can name it. Nothing a journal consumes is
/// built from a raw body here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GqlReply {
    /// A 200 whose whole body arrived, and read as `data`.
    Data(Value),
    /// A 200 whose `errors` name `RATE_LIMITED`, or a transport-mapped
    /// rate limit: a deadline sets the run back, never breaches it.
    RateLimited {
        /// A delay the reply named; `None` leaves the provider's default.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// A 200 whose errors name an authorisation failure, or a
    /// transport-mapped credential refusal.
    Permission(CredentialRefused),
    /// A 200 whose errors name an unprocessable anchor
    /// (`UNPROCESSABLE`), or a transport-mapped 422.
    Stale,
    /// The clearance refused: nothing was sent, or the run's fresh
    /// authority went out, and no request answered.
    Refused(BlockedReason),
    /// Anything else: a transport failure, a bounded or unparsable body,
    /// a 200 whose errors name nothing a rule here trusts, or a 200
    /// without `data`. The response leaves this step unresolved until a
    /// reconciliation read decides, and nothing is inferred from it.
    Unresolved,
}

/// Reads one flow through 3.1's transport verdict into a GraphQL reply.
pub fn classify_reply(flow: ActionFlow) -> GqlReply {
    match flow {
        ActionFlow::OffHost => GqlReply::Unresolved,
        ActionFlow::Refused { reason } => GqlReply::Refused(reason),
        ActionFlow::Deferred { .. } => GqlReply::Unresolved,
        ActionFlow::Sent(verdict) => match verdict {
            ActionVerdict::Read { body } => read_data(&body),
            ActionVerdict::RateLimited {
                delay_secs,
                secondary,
            } => GqlReply::RateLimited {
                delay_secs,
                secondary,
            },
            ActionVerdict::Permission { insufficient } => GqlReply::Permission(insufficient),
            ActionVerdict::Stale => GqlReply::Stale,
            ActionVerdict::Unresolved => GqlReply::Unresolved,
        },
    }
}

/// Reads a whole 2xx body as `data`, or as the errors its own text names.
/// A bounded, unparsable or dataless body leaves nothing.
fn read_data(body: &[u8]) -> GqlReply {
    let Ok(text) = std::str::from_utf8(body) else {
        return GqlReply::Unresolved;
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return GqlReply::Unresolved;
    };
    // The errors field: absent, an explicit null or an empty array is
    // the no-error spelling; an array is read whole; anything typed
    // otherwise is an answer nothing here may read.
    match value.get("errors") {
        Some(Value::Null) | None => {}
        Some(Value::Array(errors)) if errors.is_empty() => {}
        Some(Value::Array(errors)) => return read_errors(errors),
        _ => return GqlReply::Unresolved,
    }
    match value.get("data") {
        Some(data @ Value::Object(_)) => GqlReply::Data(data.clone()),
        _ => GqlReply::Unresolved,
    }
}

/// What kind one error's code name, as this protocol trusts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    Rate,
    Permission(CredentialRefused),
    Stale,
}

/// One error's own kind. `None` is an error whose code is no rule here's:
/// the whole reply then says nothing, however its other errors read.
///
/// GitHub names an error's kind in its top-level `type` (`RATE_LIMITED`,
/// `FORBIDDEN`, `INSUFFICIENT_SCOPES`, `UNPROCESSABLE`, `NOT_FOUND`), the
/// field `crate::github` already reads; `extensions.code` is read only
/// when `type` is absent, as on a query-validation error.
fn read_error(error: &Value) -> Option<ErrorKind> {
    let code = error
        .get("type")
        .or_else(|| error.pointer("/extensions/code"))?
        .as_str()?;
    match code {
        "RATE_LIMITED" => Some(ErrorKind::Rate),
        "UNAUTHENTICATED" | "UNAUTHORIZED" => {
            Some(ErrorKind::Permission(CredentialRefused::CredentialRefused))
        }
        "FORBIDDEN" | "INSUFFICIENT_SCOPES" | "INSUFFICIENT_SCOPE" => {
            Some(ErrorKind::Permission(CredentialRefused::NotPermitted))
        }
        "UNPROCESSABLE" => Some(ErrorKind::Stale),
        _ => None,
    }
}

/// Whether one error's message names a secondary rate limit
/// (`crate::github`: the same words a 403's body names).
fn names_secondary(error: &Value) -> bool {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(|message| {
            message
                .to_ascii_lowercase()
                .contains("secondary rate limit")
        })
        .unwrap_or(false)
}

/// Classifies a whole non-empty error array, whose every error must name
/// one kind a rule here trusts. One unkindness or disagreement among them
/// leaves the whole reply unresolved.
fn read_errors(errors: &[Value]) -> GqlReply {
    let mut kind: Option<ErrorKind> = None;
    let mut secondary = false;
    for error in errors {
        let Some(its_kind) = read_error(error) else {
            return GqlReply::Unresolved;
        };
        if names_secondary(error) {
            secondary = true;
        }
        match kind {
            None => kind = Some(its_kind),
            Some(kind) if kind == its_kind => {}
            _ => return GqlReply::Unresolved,
        }
    }
    match kind {
        Some(ErrorKind::Rate) => GqlReply::RateLimited {
            delay_secs: None,
            secondary,
        },
        Some(ErrorKind::Permission(refused)) => GqlReply::Permission(refused),
        Some(ErrorKind::Stale) => GqlReply::Stale,
        None => GqlReply::Unresolved,
    }
}

// ---- the pending-state read ----

/// One pending comment of a pending review, as the pending-state read
/// answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingComment {
    /// The provider's stable comment id.
    pub id: String,
    /// The comment's body, as its own reply spelled it.
    pub body: String,
}

/// One pending review, as the pending-state read answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReview {
    /// The provider's stable review id.
    pub id: String,
    /// The review author's stable node id, `None` when the reply named
    /// none. Nothing without one is this operation's.
    pub author: Option<String>,
    /// The review's body, as its own reply spelled it.
    pub body: String,
    /// Its comments' page.
    pub comments: Page<PendingComment>,
}

/// One connection's page: its objects in reply order, and the cursor a
/// continuation continues from, when the reply named one. A page is never
/// trusted past its own bound
/// (`crate::review_action_io::page_objects`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub nodes: Vec<T>,
    /// The next page's cursor, when the reply named one.
    pub next: Option<String>,
}

/// One connection's cursor, as its page info names it: `Some(cursor)`
/// when the reply named a next page and its cursor, `None` when it named
/// the connection's end, and nothing when its own answer is one this read
/// cannot structure.
fn next_cursor(connection: &Value) -> Option<Option<String>> {
    let info = connection.pointer("/pageInfo")?.as_object()?;
    match info.get("hasNextPage") {
        Some(Value::Bool(true)) => Some(
            info.get("endCursor")
                .and_then(Value::as_str)
                .map(str::to_owned),
        ),
        Some(Value::Bool(false)) => Some(None),
        _ => None,
    }
}

/// How one pending-state read came out, as far as its reply, its bounds
/// and one answer can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingRead {
    /// A page of the pending reviews, whole and bound-true.
    Reviews { page: Page<PendingReview> },
    RateLimited {
        /// A delay the reply asked for; `None` leaves the provider's own
        /// existing default.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// The provider refused this read's authorisation: the run stops, and
    /// no retry under the stamped credential follows it.
    Permission(CredentialRefused),
    /// The provider named this read's anchor unprocessable: the run stops
    /// as stale, and no retry under any line follows.
    Stale,
    /// The clearance refused: nothing was sent, or the run's fresh
    /// authority went out, and no request answered.
    Refused(BlockedReason),
    /// Without a confident answer — a transport failure, a bound struck,
    /// a body unparsable or an error made of nothing a rule trusts. The
    /// runner reconciles by reading again before any mutation, and a
    /// blind repeat is never its ask.
    Unresolved,
}

/// Reads one flow through 3.1's transport verdict into a pending read.
pub fn parse_pending(flow: ActionFlow) -> PendingRead {
    match classify_reply(flow) {
        GqlReply::Data(data) => parse_pending_data(&data),
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => PendingRead::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => PendingRead::Permission(refused),
        GqlReply::Stale => PendingRead::Stale,
        GqlReply::Refused(reason) => PendingRead::Refused(reason),
        GqlReply::Unresolved => PendingRead::Unresolved,
    }
}

/// Reads one 2xx reply's `data` into the pending reviews' page, whole or
/// nothing: a listing beyond its bound, a review without its whole
/// names — id, author, body, comments, page info — or a page info that
/// names no answer leaves nothing this read may trust.
fn parse_pending_data(data: &Value) -> PendingRead {
    let Some(node) = data.get("node") else {
        return PendingRead::Unresolved;
    };
    let Some(reviews) = node.get("reviews") else {
        return PendingRead::Unresolved;
    };
    let Some(review_nodes) = reviews.get("nodes").and_then(Value::as_array) else {
        return PendingRead::Unresolved;
    };
    let Some(count) = page_objects(review_nodes.len()) else {
        return PendingRead::Unresolved;
    };
    let mut parsed = Vec::with_capacity(count);
    for review in review_nodes {
        let Ok(id) = field_string(review.get("id")) else {
            return PendingRead::Unresolved;
        };
        // The author: an object with its id, an explicit null or an
        // absent field answers as none; anything typed otherwise is a
        // page this read cannot structure.
        let author = match review.get("author") {
            Some(Value::Object(author)) => match field_string(author.get("id")) {
                Ok(id) => id,
                Err(()) => return PendingRead::Unresolved,
            },
            Some(Value::Null) | None => None,
            _ => return PendingRead::Unresolved,
        };
        let Ok(body) = field_string(review.get("body")) else {
            return PendingRead::Unresolved;
        };
        let (Some(id), Some(body)) = (id, body) else {
            return PendingRead::Unresolved;
        };
        let Some(comments) = review.get("comments") else {
            return PendingRead::Unresolved;
        };
        let Some(comment_nodes) = comments.get("nodes").and_then(Value::as_array) else {
            return PendingRead::Unresolved;
        };
        let Some(comment_count) = page_objects(comment_nodes.len()) else {
            return PendingRead::Unresolved;
        };
        let mut comments_parsed = Vec::with_capacity(comment_count);
        for comment in comment_nodes {
            let Some(comment) = comment.as_object() else {
                return PendingRead::Unresolved;
            };
            let Ok(comment_id) = field_string(comment.get("id")) else {
                return PendingRead::Unresolved;
            };
            let Ok(comment_body) = field_string(comment.get("body")) else {
                return PendingRead::Unresolved;
            };
            let (Some(comment_id), Some(comment_body)) = (comment_id, comment_body) else {
                return PendingRead::Unresolved;
            };
            comments_parsed.push(PendingComment {
                id: comment_id,
                body: comment_body,
            });
        }
        let Some(next) = next_cursor(comments) else {
            return PendingRead::Unresolved;
        };
        parsed.push(PendingReview {
            id,
            author,
            body,
            comments: Page {
                nodes: comments_parsed,
                next,
            },
        });
    }
    let Some(next) = next_cursor(reviews) else {
        return PendingRead::Unresolved;
    };
    PendingRead::Reviews {
        page: Page {
            nodes: parsed,
            next,
        },
    }
}

/// One whole reply field, as a JSON value: a string, an explicit null, or
/// an absent field — no other type, and nothing else is trusted.
fn field_string(value: Option<&Value>) -> Result<Option<String>, ()> {
    match value {
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Null) | None => Ok(None),
        _ => Err(()),
    }
}

// ---- the adoption ----

/// What this operation's ledger names, as the adoption may trust it: the
/// operation's marker, its stamped actor, and one expectation per ledger
/// step a marked review may carry — one item's marker, and the provider
/// comment id the ledger recorded for that item, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerExpectations {
    /// The operation marker this review's body must carry.
    pub operation_marker: String,
    /// The stamped actor's node id this review's author must equal.
    pub actor: String,
    /// One expectation per ledger step, in the operation's write order.
    pub items: Vec<LedgerExpectation>,
}

/// One ledger step's expectations, as the adoption checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerExpectation {
    /// The item marker this step's write carried.
    pub marker: String,
    /// The provider comment id the ledger recorded for the item, if any.
    pub provider_id: Option<String>,
}

/// What one pending review's comments must be, whole or nothing to adopt:
///
/// - a comment whose provider id the ledger recorded — already
///   reconciled — is named whether or not its own marker is still in its
///   body, its cleanup's timing being its run's next step and not this
///   read's: as long as no other expectation's marker is beside it;
/// - a comment none recorded must spell exactly one unreconciled
///   expectation's marker, inside its body and exactly once, and no other
///   expectation's marker beside it;
/// - and no expectation may be named by more than one comment, whatever
///   their bodies: a duplicate marker names nothing a run may trust
///   (`pull-request-actions`: *GitHub Submission Owns a Commit-Pinned
///   Pending Review*).
pub fn comments_within(review: &PendingReview, expectations: &[LedgerExpectation]) -> bool {
    let mut named = vec![0; expectations.len()];
    for comment in &review.comments.nodes {
        // Is this comment the ledger's already-reconciled object?
        let recorded = expectations.iter().enumerate().find(|(_, expectation)| {
            expectation.provider_id.as_deref() == Some(comment.id.as_str())
        });
        let Some((recorded_at, _)) = recorded else {
            // An unreconciled comment: exactly one unreconciled
            // expectation's marker in its body, exactly once.
            let mut matched: Option<usize> = None;
            for (at, expectation) in expectations.iter().enumerate() {
                if expectation.provider_id.is_some() {
                    // A reconciled item's marker, inside a comment whose
                    // recorded id is another item's or none's: no write
                    // this ledger's.
                    continue;
                }
                match occurrences(&comment.body, &expectation.marker) {
                    Occurrences::None => {}
                    Occurrences::One => {
                        if matched.is_some() {
                            return false;
                        }
                        matched = Some(at);
                    }
                    Occurrences::Ambiguous => return false,
                }
            }
            let Some(at) = matched else {
                return false;
            };
            named[at] += 1;
            continue;
        };
        // An already-reconciled comment: no marker but its own may be in
        // its body, at any count its cleanup leaves.
        for (at, expectation) in expectations.iter().enumerate() {
            if at == recorded_at {
                continue;
            }
            if occurrences(&comment.body, &expectation.marker) != Occurrences::None {
                return false;
            }
        }
        named[recorded_at] += 1;
    }
    named.iter().all(|count| *count <= 1)
}

/// Why a pause stops the operation
/// (`pull-request-actions`: *GitHub Submission Owns a Commit-Pinned
/// Pending Review*): no content under any of them is adopted, submitted,
/// edited or deleted, however else the reviews read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pause {
    /// The marked review exists but cannot be trusted for its own
    /// contents: its author is not the stamped actor, or its body or
    /// comments carry content its ledger doesn't name — foreign even
    /// though GitHub holds them in the same review
    /// (`pull-request-actions`: *Content added elsewhere makes the owned
    /// review foreign*).
    Foreign { review_id: Option<String> },
    /// The pending reviews read carry no marker of this operation: none
    /// of them is its own
    /// (`pull-request-actions`: *An unrelated pending review is
    /// untouched*).
    Unrelated { count: usize },
    /// Both a foreign marked review and unrelated pending ones exist.
    Both { count: usize },
}

/// How one pending state's reviews may be adopted for an operation, as
/// far as its ledger's expectations and the read's own bounds can prove
/// (`pull-request-actions`: *The pending review is pinned to the
/// confirmed head*, for its contents' whole authority).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adoption {
    /// No pending review anywhere: free to create the owned one.
    None,
    /// Exactly one, and it is this operation's by actor and marker, and
    /// whole by ledger: free to continue it.
    Adopt { review_id: String },
    /// Nothing may change: pause the operation over what it read, for the
    /// reviewer's manual handling on GitHub.
    Pause(Pause),
}

/// Reads one pending state's reviews into what this step may do: adopt
/// the marked review whose contents are its ledger's, create one when
/// none is pending, or pause over foreign or unrelated reviews — without
/// touching anything of either.
pub fn adopt(reviews: &[PendingReview], expected: &LedgerExpectations) -> Adoption {
    if reviews.is_empty() {
        return Adoption::None;
    }
    let mut marked: Option<&PendingReview> = None;
    let mut unrelated = 0;
    for review in reviews {
        match occurrences(&review.body, &expected.operation_marker) {
            Occurrences::None => unrelated += 1,
            Occurrences::One => {
                if marked.replace(review).is_some() {
                    // Two pending reviews spelling this operation's
                    // marker: neither one whole answer.
                    return Adoption::Pause(Pause::Foreign { review_id: None });
                }
            }
            // A body carrying the operation marker more than once is not
            // one marker any of us wrote: foreign.
            Occurrences::Ambiguous => {
                return Adoption::Pause(Pause::Foreign {
                    review_id: Some(review.id.clone()),
                });
            }
        }
    }
    let Some(marked) = marked else {
        return Adoption::Pause(Pause::Unrelated { count: unrelated });
    };
    // The body the ledger names is the operation marker and nothing
    // beside it: every word past it was written by another hand, and is
    // an unrecognised body edit making the review foreign.
    let owned = marked.author.as_deref() == Some(expected.actor.as_str())
        && marked.body == expected.operation_marker
        && comments_within(marked, &expected.items);
    match owned {
        true if unrelated == 0 => Adoption::Adopt {
            review_id: marked.id.clone(),
        },
        true => Adoption::Pause(Pause::Both { count: unrelated }),
        false if unrelated == 0 => Adoption::Pause(Pause::Foreign {
            review_id: Some(marked.id.clone()),
        }),
        false => Adoption::Pause(Pause::Both { count: unrelated }),
    }
}

// ---- the creation ----

/// How one create came out, as far as its reply and the stable ids it
/// names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Creation {
    /// One owned pending review, whose stable id this carries.
    Created {
        /// The review's provider stable id.
        review_id: String,
    },
    RateLimited {
        /// A delay the reply asked for; `None` leaves the provider's own
        /// existing default.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// The provider refused the create's authorisation: the run stops.
    Permission(CredentialRefused),
    /// The provider named the create's anchor unprocessable: the run
    /// stops as stale.
    Stale,
    /// The clearance refused: nothing was sent, or the run's fresh
    /// authority went out, and no request answered.
    Refused(BlockedReason),
    /// Without a stable review id the reply could be trusted to name: the
    /// runner reconciles by reading the pending state before any retry,
    /// and a blind repeat is never its ask.
    Unresolved,
}

/// Reads one flow through 3.1's transport verdict into a create outcome.
pub fn parse_created(flow: ActionFlow) -> Creation {
    match classify_reply(flow) {
        GqlReply::Data(data) => {
            match data
                .pointer("/addPullRequestReview/pullRequestReview/id")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                Some(review_id) => Creation::Created { review_id },
                None => Creation::Unresolved,
            }
        }
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => Creation::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => Creation::Permission(refused),
        GqlReply::Stale => Creation::Stale,
        GqlReply::Refused(reason) => Creation::Refused(reason),
        GqlReply::Unresolved => Creation::Unresolved,
    }
}

// ---- provisional bodies ----

/// The provisional body one item's write carries: its invisible item
/// marker beside the frozen clean prose (`pull-request-actions`: *GitHub
/// Submission Owns a Commit-Pinned Pending Review*). The clean body is
/// exactly what the body becomes once its marker is removed, so the two
/// round-trip byte for byte.
pub fn provisional_body(item_marker: &str, clean: &str) -> String {
    format!("{item_marker}{clean}")
}

/// The clean body a provisional body carries: its item marker removed
/// exactly once. `None` when the body does not carry its marker exactly
/// once — a marker to clean is named, never guessed at, and a body whose
/// count is other than one proves nothing about its prose.
pub fn clean_body(provisional: &str, item_marker: &str) -> Option<String> {
    match occurrences(provisional, item_marker) {
        Occurrences::One => Some(provisional.replacen(item_marker, "", 1)),
        _ => None,
    }
}

// ---- the item writes, cleanups, submission and deletion ----

/// Adds one frozen inline comment to the adopted pending review: the
/// `$input.pullRequestReviewId` names it, `$input.path` and
/// `$input.line` on `$input.side` the line it comments, and
/// `$input.body` the provisional marker-bearing body. The reply names
/// the thread it opened and the one comment that thread holds. Fixed
/// at build time.
pub const ADD_THREAD_MUTATION: &str = r#"mutation SpecForgeAddThread($input: AddPullRequestReviewThreadInput!) {
  addPullRequestReviewThread(input: $input) {
    thread {
      id
      comments(first: 100) {
        nodes {
          id
          body
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}"#;

/// Adds one frozen reply to the adopted pending review:
/// `$input.pullRequestReviewId` names it, `$input.pullRequestReviewThreadId`
/// the review thread the ledger names (the detail reads thread node ids,
/// never a comment id), `$input.body` the provisional marker-bearing
/// body. GitHub's `addPullRequestReviewComment` with `inReplyTo` takes a
/// comment id and is deprecated in favour of this mutation. Fixed at
/// build time.
pub const ADD_REPLY_MUTATION: &str = r#"mutation SpecForgeAddReply($input: AddPullRequestReviewThreadReplyInput!) {
  addPullRequestReviewThreadReply(input: $input) {
    comment {
      id
      body
    }
  }
}"#;

/// Cleans one item's marker by replacing its body with the frozen clean
/// prose: `$input.pullRequestReviewCommentId` names the reconciled
/// comment, `$input.body` rides the clean body whole. Fixed at build time.
pub const COMMENT_CLEANUP_MUTATION: &str = r#"mutation SpecForgeCommentCleanup($input: UpdatePullRequestReviewCommentInput!) {
  updatePullRequestReviewComment(input: $input) {
    pullRequestReviewComment {
      id
      body
    }
  }
}"#;

/// Submits the adopted pending review with the clean summary and the
/// selected event: `$input.pullRequestReviewId` names it, and
/// `$input.body` is the one value that atomically replaces the review
/// body's operation marker as the review becomes submitted. Fixed at
/// build time.
pub const SUBMIT_REVIEW_MUTATION: &str = r#"mutation SpecForgeSubmitReview($input: SubmitPullRequestReviewInput!) {
  submitPullRequestReview(input: $input) {
    pullRequestReview {
      id
      state
      body
    }
  }
}"#;

/// Reads one review's stable state, over the id the ledger adopted by:
/// the read a lost final reply asks for, its `state` naming whether
/// this operation's submission landed. A read-only query, fixed at
/// build time.
pub const REVIEW_STATE_QUERY: &str = r#"query SpecForgeReviewState($reviewId: ID!) {
  node(id: $reviewId) {
    ... on PullRequestReview {
      id
      state
      body
    }
  }
}"#;

/// Deletes one exact pending review for an explicit Abandon:
/// `$input.pullRequestReviewId` is the review id a whole adoption named — never any other
/// review's, whatever else is pending. Fixed at build time.
pub const DELETE_REVIEW_MUTATION: &str = r#"mutation SpecForgeDeleteReview($input: DeletePullRequestReviewInput!) {
  deletePullRequestReview(input: $input) {
    clientMutationId
  }
}"#;

/// The one set of mutations this protocol's fixed documents name: the
/// owned pending review's create, its inline-comment and thread-reply
/// writes, the cleanup edit that removes one item marker, the submit
/// that replaces the operation marker atomically, and the exact
/// adopted review's deletion for an explicit Abandon. Whatever beyond
/// it — a merge, a close or a reopen of the pull request, a thread
/// resolution or its reopening, a branch deletion, an edit or dismissal
/// of another review — is refused by these documents' very shape
/// (`pull-request-actions`: *Review mutations are bounded*).
pub const ALLOWED_MUTATIONS: &[&str] = &[
    "addPullRequestReview",
    "addPullRequestReviewThread",
    "addPullRequestReviewThreadReply",
    "updatePullRequestReviewComment",
    "submitPullRequestReview",
    "deletePullRequestReview",
];

/// The side one line comment rides, as GitHub's GraphQL names it
/// (`pull_request_detail`: the same `LEFT` old and `RIGHT` new the
/// detail reads spell).
fn side_field(side: DiffSide) -> &'static str {
    match side {
        DiffSide::Old => "LEFT",
        DiffSide::New => "RIGHT",
    }
}

/// The runtime values of one inline comment's write: the review it
/// attaches to, one path and one line of its side, and the provisional
/// marker-bearing body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddThreadVariables {
    /// The adopted pending review's stable id.
    pub pull_request_review_id: String,
    /// The file path, as the frozen draft named it.
    pub path: String,
    /// The one line the comment rides.
    pub line: u32,
    /// Which side of the diff the line is on.
    pub side: DiffSide,
    /// The provisional body the write carries.
    pub body: String,
}

impl From<&AddThreadVariables> for Value {
    fn from(variables: &AddThreadVariables) -> Self {
        json!({
            "input": {
                "pullRequestReviewId": variables.pull_request_review_id,
                "path": variables.path,
                "line": variables.line,
                "side": side_field(variables.side),
                "body": variables.body,
            }
        })
    }
}

/// The runtime values of one reply's write: the review it belongs to,
/// the review thread the ledger names, and the provisional body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddReplyVariables {
    /// The adopted pending review's stable id.
    pub pull_request_review_id: String,
    /// The review thread's node id, as the frozen draft names it.
    pub thread_id: String,
    /// The provisional body the write carries.
    pub body: String,
}

impl From<&AddReplyVariables> for Value {
    fn from(variables: &AddReplyVariables) -> Self {
        json!({
            "input": {
                "pullRequestReviewId": variables.pull_request_review_id,
                "pullRequestReviewThreadId": variables.thread_id,
                "body": variables.body,
            }
        })
    }
}

/// The runtime values of one cleanup edit: the reconciled comment it
/// edits, and the frozen clean body it now carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentCleanupVariables {
    /// The comment's stable id, as the ledger recorded it.
    pub comment_id: String,
    /// The clean body the edit replaces the provisional one with.
    pub body: String,
}

impl From<&CommentCleanupVariables> for Value {
    fn from(variables: &CommentCleanupVariables) -> Self {
        json!({
            "input": {
                "pullRequestReviewCommentId": variables.comment_id,
                "body": variables.body,
            }
        })
    }
}

/// The submit event the operation's decision carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitEvent {
    Comment,
    Approve,
    RequestChanges,
}

impl SubmitEvent {
    /// The event as GitHub's GraphQL names it.
    pub fn name(self) -> &'static str {
        match self {
            SubmitEvent::Comment => "COMMENT",
            SubmitEvent::Approve => "APPROVE",
            SubmitEvent::RequestChanges => "REQUEST_CHANGES",
        }
    }
}

/// The runtime values of one submit: the adopted review it sends, the
/// frozen decision's event, and the clean summary that atomically
/// replaces the operation marker. An approval's summary may be empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitReviewVariables {
    /// The adopted pending review's stable id.
    pub review_id: String,
    /// The frozen decision, at submission an event.
    pub event: SubmitEvent,
    /// The clean summary; as empty as the decision asks for.
    pub body: String,
}

impl From<&SubmitReviewVariables> for Value {
    fn from(variables: &SubmitReviewVariables) -> Self {
        json!({
            "input": {
                "pullRequestReviewId": variables.review_id,
                "event": variables.event.name(),
                "body": variables.body,
            }
        })
    }
}

/// The runtime values of one state read: the review id the ledger
/// adopted by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewStateVariables {
    pub review_id: String,
}

impl From<&ReviewStateVariables> for Value {
    fn from(variables: &ReviewStateVariables) -> Self {
        json!({ "reviewId": variables.review_id })
    }
}

/// The runtime values of one deletion: the exact adopted review, and
/// nothing beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteVariables {
    pub review_id: String,
}

impl From<&DeleteVariables> for Value {
    fn from(variables: &DeleteVariables) -> Self {
        json!({
            "input": {
                "pullRequestReviewId": variables.review_id,
            }
        })
    }
}

/// What one item write came to, whole or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Addition {
    /// The provider confirmed the write, whose stable comment id it
    /// named — and, for a fresh line's write, the thread it opened.
    Added {
        /// The thread's stable id, for a fresh line's write.
        thread_id: Option<String>,
        /// The comment's stable id, the one the ledger records.
        comment_id: String,
    },
    RateLimited {
        /// A delay the reply asked for; `None` leaves the provider's own
        /// existing default.
        delay_secs: Option<u64>,
        /// The reply named a secondary rate limit, which holds every
        /// request of its provider, not only this one's.
        secondary: bool,
    },
    /// The provider refused this write's authorisation: the run stops.
    Permission(CredentialRefused),
    /// The provider named this write's anchor unprocessable: the run
    /// stops as stale.
    Stale,
    /// The clearance refused: nothing was sent, or the run's fresh
    /// authority went out, and no request answered.
    Refused(BlockedReason),
    /// Without a stable id the reply could be trusted to name — the
    /// runner reads the pending state before any retry, and a blind
    /// repeat is never its ask.
    Unresolved,
}

/// What one cleanup edit came to, whole or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cleanup {
    /// The edit landed: the comment's stable id, and the body it now
    /// carries, which the runner compares with the frozen clean prose.
    Cleaned {
        /// The comment's stable id, as the ledger recorded it.
        comment_id: String,
        /// The body the comment now carries.
        body: String,
    },
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    Stale,
    Refused(BlockedReason),
    /// The cleanup edit's answer names nothing to trust. The run pauses
    /// before repeating it: a read reconciles what landed.
    Unresolved,
}

/// What state one named review is in, as far as its reply names it.
/// Whatever this version does not name is [`ReviewStatus::Unknown`],
/// never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Pending,
    Submitted,
    Dismissed,
    Unknown,
}

/// What a submission, or a fresh state read over the same stable id,
/// named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Final {
    /// The reply named the review whole: its stable id, its state, and
    /// the body the review now carries.
    Completed {
        review_id: String,
        status: ReviewStatus,
        body: String,
    },
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    Stale,
    Refused(BlockedReason),
    /// Whatever the reply named else — a lost body, an unknown state, no
    /// review at the id — leaves the run's last step unresolved, and a
    /// fresh state read is the ask that decides it.
    Unresolved,
}

/// What one deletion came to, whole or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// The provider answered the deletion: the run reconciles its
    /// absence by reading, which a conclusive reply needs no less.
    Removed,
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    Stale,
    Refused(BlockedReason),
    /// The deletion's answer names nothing to trust. The run never
    /// repeats it blindly: one complete in-bound read reconciles what
    /// landed, and its uncertainty stays terminal in the receipt.
    Unresolved,
}

/// Reads one flow through 3.1's transport verdict into an item write's
/// addition.
pub fn parse_added(flow: ActionFlow) -> Addition {
    match classify_reply(flow) {
        GqlReply::Data(data) => parse_added_data(&data),
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => Addition::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => Addition::Permission(refused),
        GqlReply::Stale => Addition::Stale,
        GqlReply::Refused(reason) => Addition::Refused(reason),
        GqlReply::Unresolved => Addition::Unresolved,
    }
}

fn parse_added_data(data: &Value) -> Addition {
    let thread = data.pointer("/addPullRequestReviewThread/thread");
    let comment = data.pointer("/addPullRequestReviewThreadReply/comment");
    match (thread, comment) {
        (Some(thread), None) => parse_added_thread(thread),
        (None, Some(comment)) => match field_string(comment.get("id")) {
            Ok(Some(comment_id)) => Addition::Added {
                thread_id: None,
                comment_id,
            },
            _ => Addition::Unresolved,
        },
        _ => Addition::Unresolved,
    }
}

fn parse_added_thread(thread: &Value) -> Addition {
    let Ok(thread_id) = field_string(thread.get("id")) else {
        return Addition::Unresolved;
    };
    let Some(thread_id) = thread_id else {
        return Addition::Unresolved;
    };
    let Some(comments) = thread
        .get("comments")
        .and_then(|connection| connection.get("nodes"))
        .and_then(Value::as_array)
    else {
        return Addition::Unresolved;
    };
    // A thread a fresh line's write opened holds exactly one comment;
    // anything else — an empty thread, or one the reply named with
    // more than its own fresh comment — is an answer this step cannot
    // name.
    if comments.len() != 1 {
        return Addition::Unresolved;
    }
    let comment = &comments[0];
    match field_string(comment.get("id")) {
        Ok(Some(comment_id)) => Addition::Added {
            thread_id: Some(thread_id),
            comment_id,
        },
        _ => Addition::Unresolved,
    }
}

/// Reads one flow through 3.1's transport verdict into a cleanup edit.
pub fn parse_cleaned(flow: ActionFlow) -> Cleanup {
    match classify_reply(flow) {
        GqlReply::Data(data) => parse_cleaned_data(&data),
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => Cleanup::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => Cleanup::Permission(refused),
        GqlReply::Stale => Cleanup::Stale,
        GqlReply::Refused(reason) => Cleanup::Refused(reason),
        GqlReply::Unresolved => Cleanup::Unresolved,
    }
}

fn parse_cleaned_data(data: &Value) -> Cleanup {
    let Some(comment) = data.pointer("/updatePullRequestReviewComment/pullRequestReviewComment")
    else {
        return Cleanup::Unresolved;
    };
    let Ok(comment_id) = field_string(comment.get("id")) else {
        return Cleanup::Unresolved;
    };
    let Ok(body) = field_string(comment.get("body")) else {
        return Cleanup::Unresolved;
    };
    match (comment_id, body) {
        (Some(comment_id), Some(body)) => Cleanup::Cleaned { comment_id, body },
        _ => Cleanup::Unresolved,
    }
}

/// Reads one state's name as this protocol trusts it. A pending
/// review's states name exactly these three; anything beyond — a state
/// this version does not carry, or an unnameable text — is unknown,
/// never a guess.
fn review_status(state: &str) -> ReviewStatus {
    match state {
        "PENDING" => ReviewStatus::Pending,
        "SUBMITTED" => ReviewStatus::Submitted,
        "DISMISSED" => ReviewStatus::Dismissed,
        _ => ReviewStatus::Unknown,
    }
}

/// Reads one flow into a final outcome, for either the submit's reply
/// or one state read's: both name the review whole or nothing.
pub fn parse_final(flow: ActionFlow) -> Final {
    match classify_reply(flow) {
        GqlReply::Data(data) => parse_final_data(&data),
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => Final::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => Final::Permission(refused),
        GqlReply::Stale => Final::Stale,
        GqlReply::Refused(reason) => Final::Refused(reason),
        GqlReply::Unresolved => Final::Unresolved,
    }
}

fn parse_final_data(data: &Value) -> Final {
    let review = data
        .pointer("/submitPullRequestReview/pullRequestReview")
        .or_else(|| data.get("node"));
    let Some(review) = review else {
        return Final::Unresolved;
    };
    let Ok(review_id) = field_string(review.get("id")) else {
        return Final::Unresolved;
    };
    let Ok(body) = field_string(review.get("body")) else {
        return Final::Unresolved;
    };
    let Ok(state) = field_string(review.get("state")) else {
        return Final::Unresolved;
    };
    let (Some(review_id), Some(body), Some(state)) = (review_id, body, state) else {
        return Final::Unresolved;
    };
    Final::Completed {
        review_id,
        status: review_status(&state),
        body,
    }
}

/// Reads one flow into a deletion outcome.
pub fn parse_removed(flow: ActionFlow) -> Removal {
    match classify_reply(flow) {
        GqlReply::Data(data) => match data.get("deletePullRequestReview") {
            Some(Value::Object(_)) => Removal::Removed,
            _ => Removal::Unresolved,
        },
        GqlReply::RateLimited {
            delay_secs,
            secondary,
        } => Removal::RateLimited {
            delay_secs,
            secondary,
        },
        GqlReply::Permission(refused) => Removal::Permission(refused),
        GqlReply::Stale => Removal::Stale,
        GqlReply::Refused(reason) => Removal::Refused(reason),
        GqlReply::Unresolved => Removal::Unresolved,
    }
}

// ---- the item marker's hunt ----

/// What one item marker's hunt over the adopted review's comments
/// answers (`pull-request-actions`: *GitHub Submission Owns a
/// Commit-Pinned Pending Review*, the errors half): a comment found
/// carrying it exactly once confirms the item without repeating it; a
/// duplicate marker — one comment twice, or two comments — names
/// nothing the run may trust and pauses it; a whole connection read
/// within its bound proves the marker absent, and only then may a
/// retried write be sought; a connection with further pages than the
/// pass could read never proves anything, and pauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Search {
    /// One comment carries the item marker exactly once, over the whole
    /// connection within its bound.
    Found { comment_id: String },
    /// A duplicate or doubled marker: no proof, the run pauses.
    Ambiguous,
    /// The whole comments connection was read within its bound and the
    /// marker named no comment.
    Absent,
    /// The connection held pages the pass never read: no proof either
    /// way, the run pauses.
    Uncertain,
}

/// Accumulates one marker's hunt page by page: the runner reads whole
/// bound-true pages where the pending read answered, hands each to
/// [`Self::seek`], and asks [`Self::outcome`], whole or nothing, once
/// it stops the hunt.
#[derive(Debug, Default)]
pub struct MarkerSearch {
    candidates: Vec<String>,
    ambiguous: bool,
}

impl MarkerSearch {
    /// A hunt yet to find anything.
    pub fn new() -> Self {
        Self::default()
    }

    /// One page's comments, whole as the pending read answered it: the
    /// marker's occurrences leave one candidate per carrying comment,
    /// and a doubled marker or a second candidate leaves nothing the
    /// hunt may answer.
    pub fn seek(&mut self, page: &Page<PendingComment>, marker: &str) {
        for comment in &page.nodes {
            match occurrences(&comment.body, marker) {
                Occurrences::None => {}
                Occurrences::One => {
                    self.candidates.push(comment.id.clone());
                    if self.candidates.len() > 1 {
                        self.ambiguous = true;
                    }
                }
                Occurrences::Ambiguous => self.ambiguous = true,
            }
        }
    }

    /// The hunt's end, whole or nothing: `connection_done` is whether
    /// the connection's last read page named no next page to ask for.
    pub fn outcome(self, connection_done: bool) -> Search {
        if self.ambiguous || self.candidates.len() > 1 {
            return Search::Ambiguous;
        }
        if let [comment_id] = self.candidates.as_slice() {
            return Search::Found {
                comment_id: comment_id.clone(),
            };
        }
        if connection_done {
            return Search::Absent;
        }
        Search::Uncertain
    }

    /// Whether the hunt has already found more than one candidate or a
    /// doubled marker — the runner may stop asking pages.
    pub fn is_lost(&self) -> bool {
        self.ambiguous
    }
}

// ---- the bounded pass ----

/// What one whole pending-state pass answered: every pending review the
/// pull request had, with as much of each review's comments connection as
/// the pass's ten-page budget could read, and whether the connections all
/// ended within it. Only a `complete` pass may prove a marker absent;
/// anything less pauses rather than guesses (`pull-request-actions`: *GitHub
/// Submission Owns a Commit-Pinned Pending Review*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingPass {
    /// The read's reviews and comments, whole or bounded short.
    Read {
        /// Every pending review the reviews connection listed, each with
        /// the comments this pass read.
        reviews: Vec<PendingReview>,
        /// Whether every connection ended within the page budget.
        complete: bool,
    },
    RateLimited {
        delay_secs: Option<u64>,
        secondary: bool,
    },
    Permission(CredentialRefused),
    Stale,
    Refused(BlockedReason),
    /// A transport failure, a bound struck, or a page this pass cannot
    /// structure: nothing here proves anything.
    Unresolved,
}

impl PendingPass {
    /// Whether this pass read every connection whole.
    pub fn complete(&self) -> bool {
        matches!(self, Self::Read { complete: true, .. })
    }
}

// ---- the sender ----

/// One GitHub review step's sender: 3.1's transport, whose every request
/// is cleared, admitted, re-checked and sent under the discipline it
/// owns, and nothing here may reconstruct (`crate::review_action_io`).
pub struct GithubReviewSender<'a> {
    sender: GithubTransport<'a>,
}

impl<'a> GithubReviewSender<'a> {
    /// One sender over `limits` and `io`, whose requests `authority`
    /// clears exactly as 3.1's transport does, while `enabled` holds.
    pub fn new(
        limits: &'a GithubLimits,
        io: &'a dyn ActionIo,
        enabled: &'a dyn Fn() -> bool,
        authority: &'a dyn GithubAuthority,
    ) -> Self {
        Self {
            sender: GithubTransport::new(limits, io, enabled, authority),
        }
    }

    /// Records the deadline a protocol-level rate limit calls for; the
    /// fixed GraphQL document names one inside a 200 reply, so its parser
    /// is what sees it.
    fn noted<T>(&self, outcome: T, limit: Option<(Option<u64>, bool)>) -> T {
        if let Some((delay_secs, secondary)) = limit {
            self.sender.rate_limited(delay_secs, secondary);
        }
        outcome
    }

    /// One page of the pending state, over the fixed ask its variables
    /// carry.
    pub fn pending_state(&self, variables: &PendingVariables) -> PendingRead {
        let outcome = parse_pending(
            self.sender
                .graphql(request(PENDING_STATE_QUERY, Value::from(variables))),
        );
        let limit = match &outcome {
            PendingRead::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One bounded pending-state pass: every reviews page the connection
    /// names, each review's comments read to an end or the budget's, at
    /// most ten pages and 100 objects each in total. A connection that
    /// still names a page when the budget is spent leaves the pass
    /// incomplete, never absent.
    pub fn pending_pass(&self, node_id: &str) -> PendingPass {
        use std::collections::HashMap;

        let mut budget = PageBudget::full();
        let mut reviews: Vec<PendingReview> = Vec::new();
        let mut page_cursors: Vec<Option<String>> = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut reviews_after: Option<String> = None;
        let mut complete = true;

        // The reviews connection, one page at a time.
        loop {
            if !budget.spend() {
                complete = false;
                break;
            }
            let read = self.pending_state(&PendingVariables {
                node_id: node_id.to_string(),
                reviews_after: reviews_after.clone(),
                comments_after: None,
            });
            let page = match read {
                PendingRead::Reviews { page } => page,
                PendingRead::RateLimited {
                    delay_secs,
                    secondary,
                } => {
                    return PendingPass::RateLimited {
                        delay_secs,
                        secondary,
                    }
                }
                PendingRead::Permission(refused) => return PendingPass::Permission(refused),
                PendingRead::Stale => return PendingPass::Stale,
                PendingRead::Refused(reason) => return PendingPass::Refused(reason),
                PendingRead::Unresolved => return PendingPass::Unresolved,
            };
            for review in page.nodes {
                if seen.contains_key(&review.id) {
                    // A review listed twice is not one connection's answer.
                    complete = false;
                    continue;
                }
                seen.insert(review.id.clone(), reviews.len());
                page_cursors.push(reviews_after.clone());
                reviews.push(review);
            }
            match page.next {
                Some(cursor) => reviews_after = Some(cursor),
                None => break,
            }
        }

        // Each review's comments connection, continued from the reviews
        // page it was found on. The fixed document applies one
        // `$commentsAfter` under every review, so a continuation re-reads
        // that page and this pass keeps the matching review's comments.
        for at in 0..reviews.len() {
            let mut cursor = reviews[at].comments.next.clone();
            while let Some(after) = cursor {
                if !budget.spend() {
                    complete = false;
                    break;
                }
                let id = reviews[at].id.clone();
                let read = self.pending_state(&PendingVariables {
                    node_id: node_id.to_string(),
                    reviews_after: page_cursors[at].clone(),
                    comments_after: Some(after),
                });
                let page = match read {
                    PendingRead::Reviews { page } => page,
                    PendingRead::RateLimited {
                        delay_secs,
                        secondary,
                    } => {
                        return PendingPass::RateLimited {
                            delay_secs,
                            secondary,
                        }
                    }
                    PendingRead::Permission(refused) => return PendingPass::Permission(refused),
                    PendingRead::Stale => return PendingPass::Stale,
                    PendingRead::Refused(reason) => return PendingPass::Refused(reason),
                    PendingRead::Unresolved => return PendingPass::Unresolved,
                };
                let Some(next) = page.nodes.into_iter().find(|review| review.id == id) else {
                    // The continuation did not name the review it
                    // continued: nothing here proves its comments whole.
                    complete = false;
                    break;
                };
                reviews[at].comments.nodes.extend(next.comments.nodes);
                cursor = next.comments.next;
                reviews[at].comments.next = cursor.clone();
            }
        }

        PendingPass::Read { reviews, complete }
    }

    /// One create of an owned pending review, over the fixed ask its
    /// variables carry.
    pub fn create_review(&self, variables: &CreateVariables) -> Creation {
        let outcome = parse_created(
            self.sender
                .graphql(request(CREATE_REVIEW_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Creation::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One inline comment's write onto the adopted review, over the
    /// fixed ask its variables carry.
    pub fn add_thread(&self, variables: &AddThreadVariables) -> Addition {
        let outcome = parse_added(
            self.sender
                .graphql(request(ADD_THREAD_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Addition::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One reply's write onto the adopted review, over the fixed ask its
    /// variables carry.
    pub fn add_reply(&self, variables: &AddReplyVariables) -> Addition {
        let outcome = parse_added(
            self.sender
                .graphql(request(ADD_REPLY_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Addition::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One cleanup edit of one reconciled comment, over the fixed ask
    /// its variables carry.
    pub fn clean_comment(&self, variables: &CommentCleanupVariables) -> Cleanup {
        let outcome = parse_cleaned(
            self.sender
                .graphql(request(COMMENT_CLEANUP_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Cleanup::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One submit of the adopted review, with the clean summary and the
    /// frozen decision's event, over the fixed ask its variables carry.
    pub fn submit_review(&self, variables: &SubmitReviewVariables) -> Final {
        let outcome = parse_final(
            self.sender
                .graphql(request(SUBMIT_REVIEW_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Final::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One state read over the review id the ledger adopted by, the
    /// read a lost final reply reconciles with.
    pub fn review_state(&self, variables: &ReviewStateVariables) -> Final {
        let outcome = parse_final(
            self.sender
                .graphql(request(REVIEW_STATE_QUERY, Value::from(variables))),
        );
        let limit = match &outcome {
            Final::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }

    /// One exact adopted review's deletion for an explicit Abandon, over
    /// the fixed ask its variables carry.
    pub fn delete_review(&self, variables: &DeleteVariables) -> Removal {
        let outcome = parse_removed(
            self.sender
                .graphql(request(DELETE_REVIEW_MUTATION, Value::from(variables))),
        );
        let limit = match &outcome {
            Removal::RateLimited {
                delay_secs,
                secondary,
            } => Some((*delay_secs, *secondary)),
            _ => None,
        };
        self.noted(outcome, limit)
    }
}

#[cfg(test)]
mod tests;
