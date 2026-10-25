//! The GitHub review protocol's tests, at the fixed-document and
//! injected-transport level (`pull-request-actions`: *GitHub Submission
//! Owns a Commit-Pinned Pending Review*): no URL is ever spelled, the
//! markers are whole or absent, the classifier answers only what its
//! errors and statuses name, the adopting is exact, and the sender's
//! clearances and holds stand exactly where 3.1's transport put them.

use super::*;
use serde_json::json;

use crate::review_action_io::{ActionBody, ActionReply, GITHUB_GRAPHQL_URL};

const OP: &str = "8c1d7a1e-2b3f-4c5d-9e0f-1a2b3c4d5e6f";
const ITEM_A: &str = "1e6f9d0c-3a4b-5c6d-8e9f-0a1b2c3d4e5f";
const ITEM_B: &str = "2e6f9d0c-3a4b-5c6d-8e9f-0a1b2c3d4e5f0";
const ACTOR: &str = "U_viewer";
const NODE: &str = "PR_node7";
const HEAD: &str = "abc123abc123abc123abc123abc123abc123ab";
const NOW: u64 = 1_800_000_000;

// ------------------------------------------------------ the test fakes

/// One request's send, as the test transport records it: where it went
/// and what it carried. Never a token — a test fake that records the
/// credential value writes the very log line the feature must never
/// produce.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneSent {
    url: String,
    body: String,
}

/// The io one test's sender transports with: the replies it hands per
/// call, in the order asked, and every send recorded with its exact wire
/// body.
struct TestIo {
    now: std::sync::Mutex<u64>,
    github: std::sync::Mutex<std::collections::VecDeque<Option<ActionReply>>>,
    sent: std::sync::Mutex<Vec<OneSent>>,
}

impl TestIo {
    fn new() -> Self {
        Self {
            now: std::sync::Mutex::new(NOW),
            github: std::sync::Mutex::new(std::collections::VecDeque::new()),
            sent: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn hand(&self, reply: Option<ActionReply>) {
        self.github.lock().unwrap().push_back(reply);
    }

    fn sents(&self) -> Vec<OneSent> {
        self.sent.lock().unwrap().clone()
    }

    fn take_now(&self, to: u64) {
        *self.now.lock().unwrap() = to;
    }
}

impl ActionIo for TestIo {
    fn now(&self) -> u64 {
        *self.now.lock().unwrap()
    }

    fn github_post(&self, _token: &str, body: String) -> Option<ActionReply> {
        self.sent.lock().unwrap().push(OneSent {
            url: GITHUB_GRAPHQL_URL.to_string(),
            body,
        });
        self.github.lock().unwrap().pop_front().flatten()
    }

    fn bitbucket_get(&self, _username: &str, _token: &str, _url: &str) -> Option<ActionReply> {
        None
    }

    fn bitbucket_post(
        &self,
        _username: &str,
        _token: &str,
        _url: &str,
        _json: String,
    ) -> Option<ActionReply> {
        None
    }
}

/// A clearance a test's sender clears with: either the stamp's token, or
/// the reason it refuses.
struct TestAuthority {
    cleared: std::sync::Mutex<Result<String, BlockedReason>>,
}

impl TestAuthority {
    fn grant(token: &str) -> Self {
        Self {
            cleared: std::sync::Mutex::new(Ok(token.to_string())),
        }
    }

    /// The gate clears again with `token`.
    fn grants(&self, token: &str) {
        *self.cleared.lock().unwrap() = Ok(token.to_string());
    }

    fn refusals(&self, reason: BlockedReason) {
        *self.cleared.lock().unwrap() = Err(reason);
    }
}

impl GithubAuthority for TestAuthority {
    fn clear_to_send(&self) -> Result<String, BlockedReason> {
        self.cleared.lock().unwrap().clone()
    }
}

/// One sender over one test's io and limits, cleared while `enabled`
/// holds.
fn make_sender<'a>(
    io: &'a TestIo,
    limits: &'a GithubLimits,
    authority: &'a TestAuthority,
    enabled: &'a dyn Fn() -> bool,
) -> GithubReviewSender<'a> {
    GithubReviewSender::new(limits, io, enabled, authority)
}

// -------------------------------------------------------- the fixtures

/// One reply a test hands its io: a status, and when whole its body.
fn reply(status: u16, body: Option<&str>) -> Option<ActionReply> {
    Some(ActionReply {
        status,
        retry_after: None,
        remaining: None,
        reset: None,
        body: match body {
            Some(text) => ActionBody::Complete(text.as_bytes().to_vec()),
            None => ActionBody::Unread,
        },
    })
}

/// One flow of a whole 2xx body, as a mocked reply rides the transport's
/// verdict in.
fn read_flow(body: &str) -> ActionFlow {
    ActionFlow::Sent(ActionVerdict::Read {
        body: body.as_bytes().to_vec(),
    })
}

/// One error-naming 200 flow, for the classifier's own half.
fn errors_flow(errors: Value) -> ActionFlow {
    read_flow(&json!({ "errors": errors }).to_string())
}

/// One flow of a whole 2xx body over the pending query's shape.
fn reviews_flow(reviews: Value) -> ActionFlow {
    read_flow(&json!({ "data": { "node": { "reviews": reviews } } }).to_string())
}

/// One pending review on the wire, as the query asks its fields.
fn review_fixture(id: &str, author: Value, body_text: &str, comments: Vec<(&str, &str)>) -> Value {
    json!({
        "id": id,
        "author": author,
        "body": body_text,
        "comments": {
            "nodes": comments
                .iter()
                .map(|(comment_id, comment_body)| {
                    json!({ "id": comment_id, "body": comment_body })
                })
                .collect::<Vec<_>>(),
            "pageInfo": { "hasNextPage": false },
        },
    })
}

/// One pending review as the parse answers it, for the adoption's tests.
fn pending_review(
    id: &str,
    author: Option<&str>,
    body_text: &str,
    comments: Vec<(&str, &str)>,
) -> PendingReview {
    PendingReview {
        id: id.to_string(),
        author: author.map(str::to_owned),
        body: body_text.to_string(),
        comments: Page {
            nodes: comments
                .into_iter()
                .map(|(id, body)| PendingComment {
                    id: id.to_string(),
                    body: body.to_string(),
                })
                .collect(),
            next: None,
        },
    }
}

/// One operation's ledger expectations: the operation marker, the stamped
/// actor, and one entry per step — its item marker, and the provider
/// comment id the ledger recorded for it, if any.
fn expectations(items: Vec<(&str, Option<&str>)>) -> LedgerExpectations {
    LedgerExpectations {
        operation_marker: operation_marker(OP),
        actor: ACTOR.to_string(),
        items: items
            .into_iter()
            .map(|(marker, provider_id)| LedgerExpectation {
                marker: marker.to_string(),
                provider_id: provider_id.map(str::to_owned),
            })
            .collect(),
    }
}

// ------------------------------------------------------- the documents

/// Every document is fixed at build time: no endpoint text, no marker
/// text or its separator, no interpolation, no runtime value — the query
/// names its one page-bounded PENDING read through variables alone, and
/// the create its one mutation with the reply shape the parser reads
/// (`github-pull-requests`: *GitHub Privacy and Safety*).
#[test]
fn the_documents_are_fixed_and_carry_variables_only() {
    for document in [PENDING_STATE_QUERY, CREATE_REVIEW_MUTATION] {
        assert!(!document.contains("http"), "{document:?} names no endpoint");
        assert!(
            !document.contains("api.github.com"),
            "{document:?} never spells its URL's parts"
        );
        assert!(
            !document.contains("SpecForge-operation:"),
            "{document:?} never spells an operation marker"
        );
        assert!(
            !document.contains("SpecForge-item:"),
            "{document:?} never spells an item marker"
        );
        assert!(
            !document.contains(MARKER_SEPARATOR),
            "{document:?} carries no marker separator"
        );
        assert!(
            !document.contains("{{"),
            "{document:?} is no interpolated text"
        );
        assert!(
            !document.contains(OP) && !document.contains(ITEM_A) && !document.contains(NODE),
            "{document:?} carries no runtime value"
        );
    }
    // Both connections are bounded to one hundred, pending only, with
    // every continuation and the node id riding variables.
    assert_eq!(
        PENDING_STATE_QUERY.matches("first: 100").count(),
        2,
        "one hundred objects, the reviews and their comments alike"
    );
    assert_eq!(
        PENDING_STATE_QUERY.matches("states: PENDING").count(),
        1,
        "pending reviews only, whatever else GitHub holds"
    );
    assert_eq!(
        PENDING_STATE_QUERY.matches("after: $reviewsAfter").count(),
        1,
        "the reviews continuation rides a variable"
    );
    assert_eq!(
        PENDING_STATE_QUERY.matches("after: $commentsAfter").count(),
        1,
        "the comments continuation rides a variable"
    );
    assert_eq!(
        PENDING_STATE_QUERY.matches("node(id: $nodeId)").count(),
        1,
        "the pull request is named by one variable"
    );
    // The create: its one mutation, over one input variable, with the
    // reply shape the parser reads its review id out of.
    assert_eq!(
        CREATE_REVIEW_MUTATION
            .matches("addPullRequestReview(input: $input)")
            .count(),
        1
    );
    assert!(CREATE_REVIEW_MUTATION.contains("pullRequestReview"));
}

/// The mutation allowlist is exactly the create, and no document of this
/// module names any other write — no merge or close or reopen, no thread
/// resolution or its reopening, no branch deletion, no change to any
/// other review, its comments or its replies
/// (`pull-request-actions`: *Review mutations are bounded*).
#[test]
fn the_mutation_allowlist_stays_whole_and_bounded() {
    assert_eq!(ALLOWED_MUTATIONS, ["addPullRequestReview"]);
    for document in [PENDING_STATE_QUERY, CREATE_REVIEW_MUTATION] {
        for forbidden in [
            "mergePullRequest",
            "closePullRequest",
            "reopenPullRequest",
            "closePullRequestReview",
            "resolveReviewThread",
            "unresolveReviewThread",
            "reopenReviewThread",
            "deleteBranch",
            "updatePullRequestReview",
            "deletePullRequestReview",
            "addPullRequestReviewComment",
            "updatePullRequestReviewComment",
            "dismissPullRequestReview",
            "submitPullRequestReview",
        ] {
            assert!(
                !document.contains(forbidden),
                "{document:?} must never name {forbidden}"
            );
        }
    }
}

/// The wire body one request carries: its fixed document spelled whole
/// beside its variables object — and nowhere else on the wire does a
/// runtime value appear, the document carrying none of them.
#[test]
fn a_request_carries_only_its_document_and_its_variables() {
    // The pending read, over one cursor's ask.
    let pending_wire = request(
        PENDING_STATE_QUERY,
        Value::from(&PendingVariables {
            node_id: NODE.to_string(),
            reviews_after: Some("RV_cursor3".to_string()),
            comments_after: None,
        }),
    );
    let parsed: Value = serde_json::from_str(&pending_wire).unwrap();
    assert_eq!(
        parsed.get("query").and_then(Value::as_str),
        Some(PENDING_STATE_QUERY),
        "the fixed document, never edited"
    );
    let variables = parsed.get("variables").and_then(Value::as_object).unwrap();
    assert_eq!(
        variables
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        ["commentsAfter", "nodeId", "reviewsAfter"]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        "exactly the document's three variables ride it"
    );
    assert_eq!(variables.get("nodeId").and_then(Value::as_str), Some(NODE));
    assert_eq!(
        variables.get("reviewsAfter").and_then(Value::as_str),
        Some("RV_cursor3")
    );
    assert_eq!(
        variables.get("commentsAfter"),
        Some(&Value::Null),
        "an unasked continuation rides an explicit none"
    );

    // The create, over one operation marker's body and the frozen head.
    let create_wire = request(
        CREATE_REVIEW_MUTATION,
        Value::from(&CreateVariables {
            pull_request_node_id: NODE.to_string(),
            head_oid: HEAD.to_string(),
            body: operation_marker(OP),
            client_mutation_id: OP.to_string(),
        }),
    );
    let parsed: Value = serde_json::from_str(&create_wire).unwrap();
    assert_eq!(
        parsed.get("query").and_then(Value::as_str),
        Some(CREATE_REVIEW_MUTATION)
    );
    let input = parsed
        .pointer("/variables/input")
        .and_then(Value::as_object)
        .unwrap();
    assert_eq!(
        input
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        ["clientMutationId", "commitOID", "body", "pullRequestId"]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        "exactly the four input values the mutation's type asks for"
    );
    assert_eq!(
        input.get("pullRequestId").and_then(Value::as_str),
        Some(NODE)
    );
    assert_eq!(input.get("commitOID").and_then(Value::as_str), Some(HEAD));
    assert_eq!(
        input.get("body").and_then(Value::as_str),
        Some(operation_marker(OP).as_str())
    );
    assert_eq!(
        input.get("clientMutationId").and_then(Value::as_str),
        Some(OP)
    );
}

// --------------------------------------------------- the classification

/// One reply names, by its own errors and the transport's verdicts, what
/// the boundary names: a rate limit, one refusal wording or the other,
/// the stale anchor, and data — with a secondary named only where the
/// message says so.
#[test]
fn a_replys_errors_name_the_rate_limit_the_refusal_and_the_stale_anchor() {
    // A whole data-only body.
    assert_eq!(
        classify_reply(read_flow(r#"{"data":{"node":{"id":"PR"}}}"#)),
        GqlReply::Data(json!({"node": {"id": "PR"}}))
    );
    // The rate limit, as one error names it — with and without a
    // secondary in the message.
    for (message, secondary) in [
        ("You have exceeded a secondary rate limit", true),
        ("Something went wrong while executing your query", false),
    ] {
        assert_eq!(
            classify_reply(errors_flow(json!([{
                "message": message,
                "extensions": { "code": "RATE_LIMITED" },
            }]))),
            GqlReply::RateLimited {
                delay_secs: None,
                secondary
            },
            "\"{message}\" names secondary={secondary}"
        );
    }
    // Permission's two refusals, one per wording.
    for (code, refused) in [
        ("UNAUTHENTICATED", CredentialRefused::CredentialRefused),
        ("UNAUTHORIZED", CredentialRefused::CredentialRefused),
        ("FORBIDDEN", CredentialRefused::NotPermitted),
        ("INSUFFICIENT_SCOPE", CredentialRefused::NotPermitted),
    ] {
        assert_eq!(
            classify_reply(errors_flow(json!([{
                "message": "bad credential",
                "extensions": { "code": code },
            }]))),
            GqlReply::Permission(refused),
            "{code} refuses as {refused:?}"
        );
    }
    // The stale anchor.
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "Base commit is not valid",
            "extensions": { "code": "UNPROCESSABLE" },
        }]))),
        GqlReply::Stale
    );

    // The transport's own verdicts, mapped as they stand.
    assert_eq!(
        classify_reply(ActionFlow::Sent(ActionVerdict::RateLimited {
            delay_secs: Some(10),
            secondary: false
        })),
        GqlReply::RateLimited {
            delay_secs: Some(10),
            secondary: false
        }
    );
    assert_eq!(
        classify_reply(ActionFlow::Sent(ActionVerdict::Permission {
            insufficient: CredentialRefused::NotPermitted
        })),
        GqlReply::Permission(CredentialRefused::NotPermitted)
    );
    assert_eq!(
        classify_reply(ActionFlow::Sent(ActionVerdict::Stale)),
        GqlReply::Stale
    );
    assert_eq!(classify_reply(ActionFlow::OffHost), GqlReply::Unresolved);
    assert_eq!(
        classify_reply(ActionFlow::Deferred { until: NOW + 600 }),
        GqlReply::Unresolved
    );
    assert_eq!(
        classify_reply(ActionFlow::Refused {
            reason: BlockedReason::NotListed
        }),
        GqlReply::Refused(BlockedReason::NotListed)
    );
}

/// Whatever a reply's errors or lack of them name beyond the classifier's
/// kinds stands unresolved, nothing inferred: an error code no rule
/// names, one untrustworthy error among trustworthy ones, two kinds that
/// disagree, a non-JSON body, a dataless 200, or an empty error array.
#[test]
fn a_reply_of_no_kind_a_rule_trusts_stands_unresolved() {
    // An error code no rule names.
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "meh",
            "extensions": { "code": "SOME_THING" },
        }]))),
        GqlReply::Unresolved
    );
    // One trustworthy and one untrustworthy together.
    assert_eq!(
        classify_reply(errors_flow(json!([
            {
                "message": "a",
                "extensions": { "code": "UNPROCESSABLE" },
            },
            {
                "message": "b",
                "extensions": { "code": "SOME_THING" },
            },
        ]))),
        GqlReply::Unresolved
    );
    // Two trustworthy kinds that disagree.
    assert_eq!(
        classify_reply(errors_flow(json!([
            {
                "message": "a",
                "extensions": { "code": "UNPROCESSABLE" },
            },
            {
                "message": "b",
                "extensions": { "code": "UNAUTHORIZED" },
            },
        ]))),
        GqlReply::Unresolved
    );
    // An untrustworthy error carrying no code at all.
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "no code names me",
        }]))),
        GqlReply::Unresolved
    );
    // A non-JSON body.
    assert_eq!(classify_reply(read_flow("not json")), GqlReply::Unresolved);
    // A 200 whose errors name nothing, or whose data is not an object.
    assert_eq!(classify_reply(read_flow("{}")), GqlReply::Unresolved);
    assert_eq!(
        classify_reply(read_flow(r#"{"data":42}"#)),
        GqlReply::Unresolved
    );
    assert_eq!(
        classify_reply(read_flow(r#"{"data":null}"#)),
        GqlReply::Unresolved
    );
    // An empty or null errors array is the no-error spelling: the reply
    // is its data whole when it has one, unresolved when it has none.
    assert_eq!(
        classify_reply(read_flow(r#"{"errors":[],"data":{}}"#)),
        GqlReply::Data(json!({}))
    );
    assert_eq!(
        classify_reply(read_flow(r#"{"errors":null,"data":{}}"#)),
        GqlReply::Data(json!({}))
    );
    assert_eq!(
        classify_reply(read_flow(r#"{"errors":[]}"#)),
        GqlReply::Unresolved
    );
    // An unresolved transport verdict stands unresolved in its turn.
    assert_eq!(
        classify_reply(ActionFlow::Sent(ActionVerdict::Unresolved)),
        GqlReply::Unresolved
    );
}

// ---------------------------------------------------- the pending read

/// One pending read answers a whole page with both cursors named as
/// their reply named them, or nothing: an empty page is a whole answer
/// of none, and a review without an author rides with none.
#[test]
fn a_pending_read_answers_whole_pages_with_their_cursors_named() {
    // One review, whole, both connections at their end, and an absent
    // author answering as none rather than unresolved: authorship is
    // this protocol's own check to make.
    assert_eq!(
        parse_pending(reviews_flow(json!({
            "nodes": [review_fixture("RV_1", Value::Null, "one", vec![])],
            "pageInfo": { "hasNextPage": false },
        }))),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![pending_review("RV_1", None, "one", vec![])],
                next: None,
            }
        }
    );

    // Two reviews on one page, in reply order.
    let two = json!({
        "nodes": [
            review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![]),
            review_fixture("RV_2", Value::Null, "two", vec![]),
        ],
        "pageInfo": { "hasNextPage": false },
    });
    assert_eq!(
        parse_pending(reviews_flow(two)),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![
                    pending_review("RV_1", Some(ACTOR), "one", vec![]),
                    pending_review("RV_2", None, "two", vec![]),
                ],
                next: None,
            }
        }
    );

    // The connections' cursors, each where its page info named one.
    let with_comment_page = review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![]);
    let with_comment_page = match with_comment_page {
        Value::Object(mut fixed) => {
            fixed["comments"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "CVC" });
            Value::Object(fixed)
        }
        _ => unreachable!("the fixture builds an object"),
    };
    assert_eq!(
        parse_pending(reviews_flow(json!({
            "nodes": [with_comment_page],
            "pageInfo": { "hasNextPage": true, "endCursor": "RVC" },
        }))),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![PendingReview {
                    comments: Page {
                        nodes: vec![],
                        next: Some("CVC".to_string()),
                    },
                    ..pending_review("RV_1", Some(ACTOR), "one", vec![])
                }],
                next: Some("RVC".to_string()),
            }
        }
    );

    // A next page named without a cursor to ask for continues by none:
    // the page is whole, the pass stops rather than guess an ask.
    let cursorless = review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![]);
    let cursorless = match cursorless {
        Value::Object(mut fixed) => {
            fixed["comments"]["pageInfo"] = json!({ "hasNextPage": true });
            Value::Object(fixed)
        }
        _ => unreachable!("the fixture builds an object"),
    };
    assert_eq!(
        parse_pending(reviews_flow(json!({
            "nodes": [cursorless],
            "pageInfo": { "hasNextPage": false },
        }))),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![PendingReview {
                    comments: Page {
                        nodes: vec![],
                        next: None,
                    },
                    ..pending_review("RV_1", Some(ACTOR), "one", vec![])
                }],
                next: None,
            }
        }
    );

    // An empty page is a whole answer of none.
    assert_eq!(
        parse_pending(reviews_flow(json!({
            "nodes": [],
            "pageInfo": { "hasNextPage": false },
        }))),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![],
                next: None,
            }
        }
    );

    // The other answers ride their kinds whole.
    assert_eq!(
        parse_pending(errors_flow(json!([{
            "message": "slow",
            "extensions": { "code": "RATE_LIMITED" },
        }]))),
        PendingRead::RateLimited {
            delay_secs: None,
            secondary: false
        }
    );
    assert_eq!(
        parse_pending(errors_flow(json!([{
            "message": "bad credential",
            "extensions": { "code": "UNAUTHORIZED" },
        }]))),
        PendingRead::Permission(CredentialRefused::CredentialRefused)
    );
    assert_eq!(
        parse_pending(errors_flow(json!([{
            "message": "stale",
            "extensions": { "code": "UNPROCESSABLE" },
        }]))),
        PendingRead::Stale
    );
    assert_eq!(
        parse_pending(ActionFlow::Refused {
            reason: BlockedReason::NotListed
        }),
        PendingRead::Refused(BlockedReason::NotListed)
    );
}

/// Answers past a page's bound, and answers nothing to read a page by —
/// a page over-listed, a review without its id or body or page info,
/// a comment without its names, a field typed otherwise than named —
/// say nothing whatever their replies carried.
#[test]
fn a_pending_read_answers_nothing_over_its_bound_or_half_named() {
    // One past the bound, whatever the reviews carry.
    let over = json!({
        "nodes": (0..101)
            .map(|at| review_fixture(&format!("RV{at}"), json!({"id": ACTOR}), "b", vec![]))
            .collect::<Vec<_>>(),
        "pageInfo": { "hasNextPage": false },
    });
    assert_eq!(parse_pending(reviews_flow(over)), PendingRead::Unresolved);

    // A node absent, a connection absent, an array misnamed.
    assert_eq!(parse_pending(read_flow("{}")), PendingRead::Unresolved);
    assert_eq!(
        parse_pending(reviews_flow(json!({"pageInfo": {"hasNextPage": false}}))),
        PendingRead::Unresolved
    );
    assert_eq!(
        parse_pending(reviews_flow(json!({
            "nodes": {},
            "pageInfo": { "hasNextPage": false },
        }))),
        PendingRead::Unresolved
    );

    // Whole reviews that name nothing to trust, seven ways: no id, no
    // body, a typed-otherwise author, no comments connection, no
    // comments' page info, a half-named comment, and an empty object.
    let strip = |key: &str| -> Vec<Value> {
        let mut review = review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![])
            .as_object()
            .unwrap()
            .clone();
        review.remove(key);
        vec![Value::Object(review)]
    };
    let no_comment_pages = vec![{
        let mut review = review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![])
            .as_object()
            .unwrap()
            .clone();
        review["comments"] = json!({ "pageInfo": { "hasNextPage": false } });
        Value::Object(review)
    }];
    let half_named_comment = vec![{
        let mut review = review_fixture("RV_1", json!({"id": ACTOR}), "one", vec![])
            .as_object()
            .unwrap()
            .clone();
        review["comments"] = json!({
            "nodes": [json!({ "id": "C1" }), json!({ "body": "half" })],
            "pageInfo": { "hasNextPage": false },
        });
        Value::Object(review)
    }];
    for broken in [
        json!({"nodes": strip("id"), "pageInfo": {"hasNextPage": false}}),
        json!({"nodes": strip("body"), "pageInfo": {"hasNextPage": false}}),
        json!({"nodes": strip("comments"), "pageInfo": {"hasNextPage": false}}),
        json!({"nodes": no_comment_pages, "pageInfo": {"hasNextPage": false}}),
        json!({"nodes": half_named_comment, "pageInfo": {"hasNextPage": false}}),
        json!({
            "nodes": [review_fixture("RV_1", json!(42), "one", vec![])],
            "pageInfo": { "hasNextPage": false },
        }),
        json!({
            "nodes": [json!({})],
            "pageInfo": { "hasNextPage": false },
        }),
    ] {
        assert_eq!(parse_pending(reviews_flow(broken)), PendingRead::Unresolved);
    }
}

// -------------------------------------------------------- the adoption

/// An adopting is exact: a marked review of the stamped actor, whose
/// body is its operation marker and nothing beside, and whose comments
/// are its ledger's — recorded ids by name, unreconciled items by their
/// own markers exactly once, one per comment — is adopted; an empty
/// pending state is free to create.
#[test]
fn an_adopted_review_is_whole_and_an_empty_state_is_free() {
    // Nothing pending: free to create.
    assert_eq!(
        adopt(&[], &expectations(vec![])),
        Adoption::None,
        "no pending review anywhere"
    );

    // One marked review, its actor the stamp's, nothing else in it.
    let marked = vec![pending_review(
        "RV_1",
        Some(ACTOR),
        &operation_marker(OP),
        vec![],
    )];
    assert_eq!(
        adopt(&marked, &expectations(vec![])),
        Adoption::Adopt {
            review_id: "RV_1".to_string()
        }
    );

    // The ledger's items marked on their own comments and not yet read
    // back: exactly once, one item's marker per comment, in whatever
    // order the review's page lists them.
    let item_marker_a = item_marker(OP, ITEM_A);
    let item_marker_b = item_marker(OP, ITEM_B);
    assert_eq!(
        adopt(
            &[pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C2", &item_marker_b), ("C1", &item_marker_a)],
            )],
            &expectations(vec![(&item_marker_a, None), (&item_marker_b, None)]),
        ),
        Adoption::Adopt {
            review_id: "RV_1".to_string()
        },
        "a lost create's items reconcile at the item level"
    );

    // Already-reconciled comments: by their recorded ids, bare or still
    // carrying their own marker — either way theirs alone.
    for body in ["a clean body", &item_marker_a] {
        assert_eq!(
            adopt(
                &[pending_review(
                    "RV_1",
                    Some(ACTOR),
                    &operation_marker(OP),
                    vec![("C_provider_1", body)],
                )],
                &expectations(vec![(&item_marker_a, Some("C_provider_1"))]),
            ),
            Adoption::Adopt {
                review_id: "RV_1".to_string()
            },
            "a recorded comment's body {body:?} is its ledger's"
        );
    }

    // A recorded comment beside an unreconciled one: both theirs.
    assert_eq!(
        adopt(
            &[pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C_provider_1", "clean"), ("C2", &item_marker_b)],
            )],
            &expectations(vec![
                (&item_marker_a, Some("C_provider_1")),
                (&item_marker_b, None),
            ]),
        ),
        Adoption::Adopt {
            review_id: "RV_1".to_string()
        }
    );
}

/// Content the ledger doesn't name makes the review foreign: an extra
/// comment (whether from another client or a stray write), a comment
/// carrying two of the ledger's markers, a duplicated item marker, a
/// marker dropped on a comment its id can no longer name, a body past
/// its own marker, and an author other than the stamp — under each the
/// operation pauses, naming the review, and never carries on.
#[test]
fn a_foreign_review_pauses_over_its_unrecognised_content() {
    let item_marker_a = item_marker(OP, ITEM_A);
    let item_marker_b = item_marker(OP, ITEM_B);
    for (reviews, adoption, expected) in [
        // One extra comment its ledger names none by.
        (
            vec![pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C_extra", "a plain review comment")],
            )],
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string()),
            }),
            expectations(vec![(&item_marker_a, None)]),
        ),
        // A comment carrying two of the ledger's markers: neither one
        // whole answer.
        (
            vec![pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C1", &format!("{item_marker_a} and {item_marker_b}"))],
            )],
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string()),
            }),
            expectations(vec![(&item_marker_a, None), (&item_marker_b, None)]),
        ),
        // An ambiguous item marker, written twice into one comment.
        (
            vec![pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C1", &format!("{item_marker_a} {item_marker_a}"))],
            )],
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string()),
            }),
            expectations(vec![(&item_marker_a, None)]),
        ),
        // Two comments carrying the same item's marker: the marker names
        // one item, not two comments.
        (
            vec![pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C1", &item_marker_a), ("C2", &item_marker_a)],
            )],
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string()),
            }),
            expectations(vec![(&item_marker_a, None)]),
        ),
        // The body past its own marker: an unrecognised edit it wrote
        // none of.
        (
            vec![pending_review(
                "RV_1",
                Some(ACTOR),
                &format!("words before {}", operation_marker(OP)),
                vec![],
            )],
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string()),
            }),
            expectations(vec![(&item_marker_a, None)]),
        ),
    ] {
        assert_eq!(adopt(&reviews, &expected), adoption);
    }

    // A recorded comment whose id its ledger names, but carrying another
    // item's marker beside: an unrecognised write into a reconciled one.
    assert_eq!(
        adopt(
            &[pending_review(
                "RV_1",
                Some(ACTOR),
                &operation_marker(OP),
                vec![("C_provider_1", &item_marker_b)],
            )],
            &expectations(vec![
                (&item_marker_a, Some("C_provider_1")),
                (&item_marker_b, None),
            ]),
        ),
        Adoption::Pause(Pause::Foreign {
            review_id: Some("RV_1".to_string())
        })
    );

    // An author other than the stamp, or none: foreign, whatever its
    // content.
    for author in [Some("U_other"), None] {
        assert_eq!(
            adopt(
                &[pending_review(
                    "RV_1",
                    author,
                    &operation_marker(OP),
                    vec![],
                )],
                &expectations(vec![(&item_marker_a, Some("C_provider_1"))]),
            ),
            Adoption::Pause(Pause::Foreign {
                review_id: Some("RV_1".to_string())
            })
        );
    }
}

/// The other pauses name their exact review state: two marked reviews
/// say nothing either is it; a marker's duplicate in one body names that
/// review; an unrelated-only state names how many; and both name both.
#[test]
fn the_other_pauses_name_their_exact_review_state() {
    let item_marker_a = item_marker(OP, ITEM_A);
    let empty = expectations(vec![(&item_marker_a, None)]);

    // Two marked reviews: neither one whole answer, and no id named.
    assert_eq!(
        adopt(
            &[
                pending_review("RV_1", Some(ACTOR), &operation_marker(OP), vec![],),
                pending_review("RV_2", Some(ACTOR), &operation_marker(OP), vec![],),
            ],
            &empty,
        ),
        Adoption::Pause(Pause::Foreign { review_id: None })
    );

    // The operation marker twice in one body: not one whole marker
    // written, and the review it names stands named.
    assert_eq!(
        adopt(
            &[pending_review(
                "RV_1",
                Some(ACTOR),
                &format!("{0}{0}", operation_marker(OP)),
                vec![],
            )],
            &empty,
        ),
        Adoption::Pause(Pause::Foreign {
            review_id: Some("RV_1".to_string())
        })
    );

    // Unrelated-only states name their counts.
    for (count, extra) in [(1usize, false), (2, true)] {
        let mut reviews = vec![pending_review(
            "RV_1",
            Some("U_other"),
            "another review entirely",
            vec![],
        )];
        if extra {
            reviews.push(pending_review("RV_2", None, "one more, himself", vec![]));
        }
        assert_eq!(
            adopt(&reviews, &empty),
            Adoption::Pause(Pause::Unrelated { count })
        );
    }

    // A marked review beside unrelated ones is Both, whether its own
    // content is whole or not.
    let unrelated = pending_review("RV_2", Some("U_other"), "another review", vec![]);
    assert_eq!(
        adopt(
            &[
                pending_review("RV_1", Some(ACTOR), &operation_marker(OP), vec![],),
                unrelated.clone(),
            ],
            &empty,
        ),
        Adoption::Pause(Pause::Both { count: 1 })
    );
    assert_eq!(
        adopt(
            &[
                pending_review("RV_1", Some("U_other"), &operation_marker(OP), vec![],),
                unrelated,
            ],
            &empty,
        ),
        Adoption::Pause(Pause::Both { count: 1 })
    );
}

// ------------------------------------------------------- from the sender

/// Over its fixed requests, the sender reads the pending state and
/// creates an owned review: each send going to the one GraphQL endpoint
/// the io names, as the very wire body its fixed document and variables
/// spell, and the parse answering what each reply named.
#[test]
fn the_sender_reads_and_creates_over_its_one_endpoint() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    // The pending read's happy path.
    let empty_body = json!({
        "data": {"node": {"reviews": {
            "nodes": [],
            "pageInfo": {"hasNextPage": false},
        }}}
    })
    .to_string();
    io.hand(reply(200, Some(&empty_body)));
    assert_eq!(
        sender.pending_state(&PendingVariables::first(NODE)),
        PendingRead::Reviews {
            page: Page {
                nodes: vec![],
                next: None,
            }
        }
    );
    // Exactly one send, to the one fixed endpoint, carrying the exact
    // wire body the document and variables spell.
    let sents = io.sents();
    assert_eq!(sents.len(), 1);
    assert_eq!(sents[0].url, GITHUB_GRAPHQL_URL);
    let sent: Value = serde_json::from_str(&sents[0].body).unwrap();
    assert_eq!(
        sent.get("query").and_then(Value::as_str),
        Some(PENDING_STATE_QUERY)
    );
    assert_eq!(
        sent.pointer("/variables/nodeId").and_then(Value::as_str),
        Some(NODE)
    );
    assert_eq!(
        sent.pointer("/variables/reviewsAfter"),
        Some(&Value::Null),
        "the first page asks through an explicit none"
    );

    // The create's happy path.
    let created_body = json!({
        "data": {"addPullRequestReview": {"pullRequestReview": {"id": "RV_9"}}}
    })
    .to_string();
    io.hand(reply(200, Some(&created_body)));
    assert_eq!(
        sender.create_review(&CreateVariables {
            pull_request_node_id: NODE.to_string(),
            head_oid: HEAD.to_string(),
            body: operation_marker(OP),
            client_mutation_id: OP.to_string(),
        }),
        Creation::Created {
            review_id: "RV_9".to_string()
        }
    );
    let sents = io.sents();
    assert_eq!(sents.len(), 2);
    let sent: Value = serde_json::from_str(&sents[1].body).unwrap();
    assert_eq!(
        sent.get("query").and_then(Value::as_str),
        Some(CREATE_REVIEW_MUTATION)
    );
    let input = sent
        .pointer("/variables/input")
        .and_then(Value::as_object)
        .expect("the create's input rides whole");
    assert_eq!(
        input.get("pullRequestId").and_then(Value::as_str),
        Some(NODE)
    );
    assert_eq!(input.get("commitOID").and_then(Value::as_str), Some(HEAD));
    assert_eq!(
        input.get("body").and_then(Value::as_str),
        Some(operation_marker(OP).as_str())
    );
}

/// A create whose reply is lost rides out unresolved — no retry with the
/// stamp until a read proves what happened — and the next pending read
/// adopts the review a transport failure never unhid, so the run asks
/// for its create exactly once.
#[test]
fn a_lost_create_is_reconciled_by_its_next_pending_read() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);
    let variables = CreateVariables {
        pull_request_node_id: NODE.to_string(),
        head_oid: HEAD.to_string(),
        body: operation_marker(OP),
        client_mutation_id: OP.to_string(),
    };

    // The create's reply never arrives: a transport failure.
    io.hand(None);
    let outcome = sender.create_review(&variables);
    assert_eq!(outcome, Creation::Unresolved, "no id was ever named");
    assert_eq!(
        io.sents().len(),
        1,
        "one create, and nothing after it but a read"
    );

    // The reconciliation read that follows (no retry between) finds the
    // review the lost create did leave behind, marked as this
    // operation's: adopted by id.
    let marked_body = json!({
        "data": {"node": {"reviews": {
            "nodes": [review_fixture(
                "RV_lost",
                json!({"id": ACTOR}),
                &operation_marker(OP),
                vec![],
            )],
            "pageInfo": {"hasNextPage": false},
        }}}
    })
    .to_string();
    io.hand(reply(200, Some(&marked_body)));
    let read = sender.pending_state(&PendingVariables::first(NODE));
    let PendingRead::Reviews { page } = read else {
        panic!("the reconciliation read answered, and did not re-create")
    };
    assert_eq!(
        adopt(&page.nodes, &expectations(vec![])),
        Adoption::Adopt {
            review_id: "RV_lost".to_string()
        }
    );
    // Two sends whole: the create and the read. The create was asked
    // exactly once, and a third send is never the answer.
    assert_eq!(io.sents().len(), 2);
    let last: Value = serde_json::from_str(&io.sents()[1].body).unwrap();
    assert_eq!(
        last.get("query").and_then(Value::as_str),
        Some(PENDING_STATE_QUERY),
        "the second send is the read, not a second create"
    );
}

// ---------------------------------------------------- refusal and deferral

/// A refused clearance sends nothing and names its reason; a disabled
/// provider is refused with NotListed by the gate, sending nothing
/// whatever its io held; and a held deadline stands the sender down,
/// unresolved, until the deadline passes a fresh ask again.
#[test]
fn a_refused_disabled_or_held_run_sends_nothing() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");

    // A clearance refusal, on both reads and writes.
    authority.refusals(BlockedReason::DetailRequired);
    let enabled = || true;
    let refused = make_sender(&io, &limits, &authority, &enabled);
    assert_eq!(
        refused.pending_state(&PendingVariables::first(NODE)),
        PendingRead::Refused(BlockedReason::DetailRequired)
    );
    assert_eq!(
        refused.create_review(&CreateVariables {
            pull_request_node_id: NODE.to_string(),
            head_oid: HEAD.to_string(),
            body: operation_marker(OP),
            client_mutation_id: OP.to_string(),
        }),
        Creation::Refused(BlockedReason::DetailRequired)
    );
    assert!(
        io.sents().is_empty(),
        "a refusal sends nothing, no token rides"
    );

    // A disabled provider: NotListed, and no send — under a granting
    // clearance, since 3.1's transport asks the clearance first and the
    // gate's refuse stands second.
    let disabled = || false;
    let fresh = TestAuthority::grant("tok");
    let disabled_sender = GithubReviewSender::new(&limits, &io, &disabled, &fresh);
    assert_eq!(
        disabled_sender.pending_state(&PendingVariables::first(NODE)),
        PendingRead::Refused(BlockedReason::NotListed)
    );
    assert!(io.sents().is_empty());

    // A held deadline defers both, nothing sent; past it, a refused
    // clearance is cleared again and a fresh ask sends.
    authority.grants("tok");
    let held = GithubLimits::new();
    held.update_deadlines(|deadlines| {
        *deadlines = crate::github::GithubDeadlines {
            graphql: NOW + 600,
            ..crate::github::GithubDeadlines::default()
        }
    });
    let holding = make_sender(&io, &held, &authority, &enabled);
    assert_eq!(
        holding.pending_state(&PendingVariables::first(NODE)),
        PendingRead::Unresolved
    );
    assert_eq!(
        holding.create_review(&CreateVariables {
            pull_request_node_id: NODE.to_string(),
            head_oid: HEAD.to_string(),
            body: operation_marker(OP),
            client_mutation_id: OP.to_string(),
        }),
        Creation::Unresolved
    );
    assert!(io.sents().is_empty(), "a hold sends nothing");
    io.take_now(NOW + 600);
    io.hand(reply(
        200,
        Some(
            &json!({"data": {"node": {"reviews": {
                "nodes": [],
                "pageInfo": {"hasNextPage": false},
            }}}})
            .to_string(),
        ),
    ));
    assert!(
        matches!(
            holding.pending_state(&PendingVariables::first(NODE)),
            PendingRead::Reviews { .. }
        ),
        "at exactly the deadline a send clears again"
    );
}
