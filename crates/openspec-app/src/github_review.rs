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
use crate::review_action_io::{
    page_objects, ActionFlow, ActionIo, ActionVerdict, CredentialRefused, GithubAuthority,
    GithubTransport,
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

/// The one mutation 3.2's fixed documents may name. Whatever beyond it —
/// a merge, a close, a thread resolution or reopening, any write outside
/// one's own pending review, its single-line comments and its replies —
/// is refused by these documents' very shape
/// (`pull-request-actions`: *Review mutations are bounded*); 3.3's
/// documents extend this list, and the same test pins them there.
pub const ALLOWED_MUTATIONS: &[&str] = &["addPullRequestReview"];

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
fn read_error(error: &Value) -> Option<ErrorKind> {
    let code = error.pointer("/extensions/code")?.as_str()?;
    match code {
        "RATE_LIMITED" => Some(ErrorKind::Rate),
        "UNAUTHENTICATED" | "UNAUTHORIZED" => {
            Some(ErrorKind::Permission(CredentialRefused::CredentialRefused))
        }
        "FORBIDDEN" | "INSUFFICIENT_SCOPE" => {
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
fn comments_within(review: &PendingReview, expectations: &[LedgerExpectation]) -> bool {
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

    /// One page of the pending state, over the fixed ask its variables
    /// carry.
    pub fn pending_state(&self, variables: &PendingVariables) -> PendingRead {
        parse_pending(
            self.sender
                .graphql(request(PENDING_STATE_QUERY, Value::from(variables))),
        )
    }

    /// One create of an owned pending review, over the fixed ask its
    /// variables carry.
    pub fn create_review(&self, variables: &CreateVariables) -> Creation {
        parse_created(
            self.sender
                .graphql(request(CREATE_REVIEW_MUTATION, Value::from(variables))),
        )
    }
}

#[cfg(test)]
mod tests;
