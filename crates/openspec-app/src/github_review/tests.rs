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
    for document in [
        PENDING_STATE_QUERY,
        CREATE_REVIEW_MUTATION,
        ADD_THREAD_MUTATION,
        ADD_REPLY_MUTATION,
        COMMENT_CLEANUP_MUTATION,
        SUBMIT_REVIEW_MUTATION,
        REVIEW_STATE_QUERY,
        DELETE_REVIEW_MUTATION,
    ] {
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
    assert_eq!(
        ALLOWED_MUTATIONS,
        [
            "addPullRequestReview",
            "addPullRequestReviewThread",
            "addPullRequestReviewThreadReply",
            "updatePullRequestReviewComment",
            "submitPullRequestReview",
            "deletePullRequestReview"
        ]
    );
    for document in [
        PENDING_STATE_QUERY,
        CREATE_REVIEW_MUTATION,
        ADD_THREAD_MUTATION,
        ADD_REPLY_MUTATION,
        COMMENT_CLEANUP_MUTATION,
        SUBMIT_REVIEW_MUTATION,
        REVIEW_STATE_QUERY,
        DELETE_REVIEW_MUTATION,
    ] {
        for forbidden in [
            "mergePullRequest",
            "closePullRequest",
            "reopenPullRequest",
            "closePullRequestReview",
            "resolveReviewThread",
            "unresolveReviewThread",
            "reopenReviewThread",
            "deleteBranch",
            // In its call spelling: the Comment-clean mutation carries
            // this same prefix and is allowed.
            "updatePullRequestReview(",
            "dismissPullRequestReview",
        ] {
            assert!(
                !document.contains(forbidden),
                "{document:?} must never name {forbidden}"
            );
        }
    }
    // Each allowed mutation is named by exactly its own document, one
    // ask per wire body: no second mutation rides anywhere.
    let named = [
        ("addPullRequestReview", CREATE_REVIEW_MUTATION),
        ("addPullRequestReviewThread", ADD_THREAD_MUTATION),
        ("addPullRequestReviewThreadReply", ADD_REPLY_MUTATION),
        ("updatePullRequestReviewComment", COMMENT_CLEANUP_MUTATION),
        ("submitPullRequestReview", SUBMIT_REVIEW_MUTATION),
        ("deletePullRequestReview", DELETE_REVIEW_MUTATION),
    ];
    for (mutation, document) in named {
        // The call spelling: names that share a prefix with another
        // mutation are told apart by their open parenthesis.
        let call = format!("{mutation}(");
        assert_eq!(
            document.matches(&call).count(),
            1,
            "{document:?} names {mutation} once and only once"
        );
        for (other, _) in named {
            if other != mutation {
                assert!(
                    !document.contains(&format!("{other}(")),
                    "{document:?} names {other} beyond its own {mutation}"
                );
            }
        }
    }
    // The reads name no mutation at all.
    for document in [PENDING_STATE_QUERY, REVIEW_STATE_QUERY] {
        for allowed in ALLOWED_MUTATIONS {
            assert!(
                !document.contains(allowed),
                "a query names no {allowed} — it reads"
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
                "type": "RATE_LIMITED",
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
        ("INSUFFICIENT_SCOPES", CredentialRefused::NotPermitted),
        ("INSUFFICIENT_SCOPE", CredentialRefused::NotPermitted),
    ] {
        assert_eq!(
            classify_reply(errors_flow(json!([{
                "message": "bad credential",
                "type": code,
            }]))),
            GqlReply::Permission(refused),
            "{code} refuses as {refused:?}"
        );
    }
    // The stale anchor.
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "Base commit is not valid",
            "type": "UNPROCESSABLE",
        }]))),
        GqlReply::Stale
    );
    // GitHub names the kind in the top-level `type`; `extensions.code` is
    // read only where `type` is absent, and never overrides it.
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "rate limited",
            "extensions": { "code": "RATE_LIMITED" },
        }]))),
        GqlReply::RateLimited {
            delay_secs: None,
            secondary: false
        }
    );
    assert_eq!(
        classify_reply(errors_flow(json!([{
            "message": "Resource not accessible by integration",
            "type": "FORBIDDEN",
            "extensions": { "code": "UNPROCESSABLE" },
        }]))),
        GqlReply::Permission(CredentialRefused::NotPermitted)
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
            "type": "SOME_THING",
        }]))),
        GqlReply::Unresolved
    );
    // One trustworthy and one untrustworthy together.
    assert_eq!(
        classify_reply(errors_flow(json!([
            {
                "message": "a",
                "type": "UNPROCESSABLE",
            },
            {
                "message": "b",
                "type": "SOME_THING",
            },
        ]))),
        GqlReply::Unresolved
    );
    // Two trustworthy kinds that disagree.
    assert_eq!(
        classify_reply(errors_flow(json!([
            {
                "message": "a",
                "type": "UNPROCESSABLE",
            },
            {
                "message": "b",
                "type": "UNAUTHORIZED",
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
            "type": "RATE_LIMITED",
        }]))),
        PendingRead::RateLimited {
            delay_secs: None,
            secondary: false
        }
    );
    assert_eq!(
        parse_pending(errors_flow(json!([{
            "message": "bad credential",
            "type": "UNAUTHORIZED",
        }]))),
        PendingRead::Permission(CredentialRefused::CredentialRefused)
    );
    assert_eq!(
        parse_pending(errors_flow(json!([{
            "message": "stale",
            "type": "UNPROCESSABLE",
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

// ------------------------------------------------- provisional bodies

/// A provisional body is its item marker beside exactly the clean prose,
/// and cleaning removes that one marker and nothing else: every clean
/// body round-trips byte for byte — empty prose, multi-line prose, and
/// prose carrying the marker separator inside it — while a body with no
/// marker or a doubled one names nothing to clean.
#[test]
fn a_provisional_body_round_trips_its_one_marker_to_its_clean_prose() {
    let marker = item_marker(OP, ITEM_A);
    for clean in [
        "",
        "one line",
        "first line\nsecond line\n\nfourth",
        "prose that carries the separator \u{2063} inside it",
        " prose with edge whitespace ",
    ] {
        let provisional = provisional_body(&marker, clean);
        assert!(
            provisional.starts_with(&marker),
            "the marker leads {provisional:?}"
        );
        assert_eq!(
            occurrences(&provisional, &marker),
            Occurrences::One,
            "exactly one marker in {provisional:?}"
        );
        assert_eq!(
            clean_body(&provisional, &marker),
            Some(clean.to_string()),
            "{provisional:?} cleans back to its own prose"
        );
    }
    // No marker at all: nothing this cleanup may name.
    assert_eq!(clean_body("plain prose, no marker", &marker), None);
    // The marker twice: no one body either version wrote.
    assert_eq!(clean_body(&format!("{marker}{marker}prose"), &marker), None);
    // Another item's marker is not this one's.
    let other = item_marker(OP, ITEM_B);
    assert_eq!(
        clean_body(&provisional_body(&other, "prose"), &marker),
        None
    );
}

// ------------------------------------------------ the item wire values

/// Every item write's variables object carries exactly the input keys
/// its fixed document's input type asks for — the review id, the anchor,
/// the provisional body — and no document is ever asked to interpolate
/// one. The line's side is GitHub's `LEFT` old and `RIGHT` new; the
/// state read names its review at the top level, not under `input`;
/// and a submit is its three values with the decision's own event name.
#[test]
fn the_item_writes_carry_their_exact_input_values() {
    let keys = |value: &Value, pointer: &str| -> std::collections::BTreeSet<String> {
        value
            .pointer(pointer)
            .and_then(Value::as_object)
            .expect("the variables object is there")
            .keys()
            .cloned()
            .collect()
    };
    let set = |names: &[&str]| -> std::collections::BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    };

    // One inline comment's write, over each side.
    for (side, spelled) in [(DiffSide::Old, "LEFT"), (DiffSide::New, "RIGHT")] {
        let variables = Value::from(&AddThreadVariables {
            pull_request_review_id: "RV_7".to_string(),
            path: "src/lib.rs".to_string(),
            line: 42,
            side,
            body: provisional_body(&item_marker(OP, ITEM_A), "the comment"),
        });
        assert_eq!(
            keys(&variables, "/input"),
            set(&["pullRequestReviewId", "path", "line", "side", "body"]),
            "the thread input's five values, exactly"
        );
        assert_eq!(
            variables
                .pointer("/input/pullRequestReviewId")
                .and_then(Value::as_str),
            Some("RV_7")
        );
        assert_eq!(
            variables.pointer("/input/path").and_then(Value::as_str),
            Some("src/lib.rs")
        );
        assert_eq!(
            variables.pointer("/input/line").and_then(Value::as_u64),
            Some(42)
        );
        assert_eq!(
            variables.pointer("/input/side").and_then(Value::as_str),
            Some(spelled),
            "{side:?} is GitHub's {spelled}"
        );
    }

    // One reply's write.
    let reply = Value::from(&AddReplyVariables {
        pull_request_review_id: "RV_7".to_string(),
        thread_id: "PRRT_parent".to_string(),
        body: provisional_body(&item_marker(OP, ITEM_B), "the reply"),
    });
    assert_eq!(
        keys(&reply, "/input"),
        set(&["pullRequestReviewId", "pullRequestReviewThreadId", "body"])
    );
    assert_eq!(
        reply
            .pointer("/input/pullRequestReviewThreadId")
            .and_then(Value::as_str),
        Some("PRRT_parent")
    );

    // One cleanup edit.
    let cleanup = Value::from(&CommentCleanupVariables {
        comment_id: "C_7".to_string(),
        body: "the clean prose".to_string(),
    });
    assert_eq!(
        keys(&cleanup, "/input"),
        set(&["pullRequestReviewCommentId", "body"])
    );
    assert_eq!(
        cleanup
            .pointer("/input/pullRequestReviewCommentId")
            .and_then(Value::as_str),
        Some("C_7")
    );

    // One submit, each event as GitHub names it, and an empty summary
    // riding as empty rather than omitted.
    for (event, spelled) in [
        (SubmitEvent::Comment, "COMMENT"),
        (SubmitEvent::Approve, "APPROVE"),
        (SubmitEvent::RequestChanges, "REQUEST_CHANGES"),
    ] {
        let submit = Value::from(&SubmitReviewVariables {
            review_id: "RV_7".to_string(),
            event,
            body: String::new(),
        });
        assert_eq!(
            keys(&submit, "/input"),
            set(&["pullRequestReviewId", "event", "body"])
        );
        assert_eq!(
            submit.pointer("/input/event").and_then(Value::as_str),
            Some(spelled)
        );
        assert_eq!(
            submit.pointer("/input/body").and_then(Value::as_str),
            Some(""),
            "an empty approval's summary rides an explicit empty body"
        );
    }

    // One state read: the id at the top level, no input object beside it.
    let state = Value::from(&ReviewStateVariables {
        review_id: "RV_7".to_string(),
    });
    assert_eq!(keys(&state, ""), set(&["reviewId"]));
    assert!(
        state.get("input").is_none(),
        "a query takes no input object"
    );
    assert_eq!(state.get("reviewId").and_then(Value::as_str), Some("RV_7"));

    // One deletion: the exact adopted review's id, and nothing beside.
    let deletion = Value::from(&DeleteVariables {
        review_id: "RV_7".to_string(),
    });
    assert_eq!(keys(&deletion, "/input"), set(&["pullRequestReviewId"]));
    assert_eq!(
        deletion
            .pointer("/input/pullRequestReviewId")
            .and_then(Value::as_str),
        Some("RV_7")
    );
}

// -------------------------------------------------- the item parsers

/// An item write answers the stable comment id its reply named — and,
/// for a fresh line, the one thread it opened holding exactly one
/// comment. A thread with no comment or more than one, a losing reply,
/// or a reply naming neither shape answers nothing the ledger may
/// record.
#[test]
fn an_addition_names_one_comment_and_its_one_thread() {
    let thread_reply = |comments: Value| {
        read_flow(
            &json!({
                "data": {"addPullRequestReviewThread": {"thread": {
                    "id": "TH_1",
                    "comments": {
                        "nodes": comments,
                        "pageInfo": {"hasNextPage": false},
                    },
                }}}
            })
            .to_string(),
        )
    };
    // One fresh comment in the thread the write opened: both ids named.
    assert_eq!(
        parse_added(thread_reply(json!([
            {"id": "C_1", "body": "the provisional body"}
        ]))),
        Addition::Added {
            thread_id: Some("TH_1".to_string()),
            comment_id: "C_1".to_string(),
        }
    );
    // No comment in the thread: nothing this step may record.
    assert_eq!(parse_added(thread_reply(json!([]))), Addition::Unresolved);
    // Two comments: the reply's thread is not one the write opened alone.
    assert_eq!(
        parse_added(thread_reply(json!([
            {"id": "C_1", "body": "one"},
            {"id": "C_2", "body": "two"},
        ]))),
        Addition::Unresolved
    );
    // A thread without its id, and a comment without one: nothing.
    assert_eq!(
        parse_added(read_flow(
            &json!({
                "data": {"addPullRequestReviewThread": {"thread": {
                    "comments": {"nodes": [{"id": "C_1"}], "pageInfo": {"hasNextPage": false}},
                }}}
            })
            .to_string()
        )),
        Addition::Unresolved
    );
    assert_eq!(
        parse_added(thread_reply(json!([{"body": "no id"}]))),
        Addition::Unresolved
    );

    // One reply: the comment's id with no thread beside it.
    assert_eq!(
        parse_added(read_flow(
            &json!({
                "data": {"addPullRequestReviewThreadReply": {"comment": {
                    "id": "C_2",
                    "body": "the provisional reply",
                }}}
            })
            .to_string()
        )),
        Addition::Added {
            thread_id: None,
            comment_id: "C_2".to_string(),
        }
    );
    // A reply without its id, both shapes at once, and neither: nothing.
    for losing in [
        json!({"data": {"addPullRequestReviewThreadReply": {"comment": {"body": "no id"}}}}),
        json!({"data": {
            "addPullRequestReviewThread": {"thread": {"id": "TH_1", "comments": {"nodes": [{"id": "C_1"}]}}},
            "addPullRequestReviewThreadReply": {"comment": {"id": "C_2"}},
        }}),
        json!({"data": {}}),
    ] {
        assert_eq!(
            parse_added(read_flow(&losing.to_string())),
            Addition::Unresolved,
            "{losing} names no one write"
        );
    }

    // A cleanup edit answers the comment it edited and its new body.
    assert_eq!(
        parse_cleaned(read_flow(
            &json!({
                "data": {"updatePullRequestReviewComment": {"pullRequestReviewComment": {
                    "id": "C_1",
                    "body": "the clean prose",
                }}}
            })
            .to_string()
        )),
        Cleanup::Cleaned {
            comment_id: "C_1".to_string(),
            body: "the clean prose".to_string(),
        }
    );
    // An edit answer without its id or its body: nothing to compare
    // the frozen prose with.
    for losing in [
        json!({"data": {"updatePullRequestReviewComment": {"pullRequestReviewComment": {"id": "C_1"}}}}),
        json!({"data": {"updatePullRequestReviewComment": {"pullRequestReviewComment": {"body": "clean"}}}}),
        json!({"data": {}}),
    ] {
        assert_eq!(
            parse_cleaned(read_flow(&losing.to_string())),
            Cleanup::Unresolved,
            "{losing} names no whole cleanup"
        );
    }

    // A deletion answers with its mutation object, and only that.
    assert_eq!(
        parse_removed(read_flow(
            &json!({"data": {"deletePullRequestReview": {"clientMutationId": OP}}}).to_string()
        )),
        Removal::Removed
    );
    for losing in [
        json!({"data": {}}),
        json!({"data": {"deletePullRequestReview": null}}),
        json!({"data": {"deletePullRequestReview": "gone"}}),
    ] {
        assert_eq!(
            parse_removed(read_flow(&losing.to_string())),
            Removal::Unresolved,
            "{losing} names no deletion"
        );
    }
}

/// Every item flow carries the transport's kinds whole: a rate limit with
/// its delay and secondary, the credential refusal's two sentences, the
/// stale anchor, a clearance's refusal, and no answer at all — none of
/// them inferred from anything the protocol did not name.
#[test]
fn every_item_flow_carries_its_transport_kind_whole() {
    let rate_limited = || {
        errors_flow(json!([{
            "message": "You have exceeded a secondary rate limit",
            "type": "RATE_LIMITED",
        }]))
    };
    let stale = || {
        errors_flow(json!([{
            "message": "Base commit is not valid",
            "type": "UNPROCESSABLE",
        }]))
    };
    let unauthorized = || {
        errors_flow(json!([{
            "message": "bad credential",
            "type": "UNAUTHORIZED",
        }]))
    };
    let forbidden = || {
        errors_flow(json!([{
            "message": "not permitted",
            "type": "FORBIDDEN",
        }]))
    };
    let refused = || ActionFlow::Refused {
        reason: BlockedReason::NotListed,
    };
    let nothing = || read_flow("{}");

    for (flow, addition) in [
        (
            rate_limited(),
            Addition::RateLimited {
                delay_secs: None,
                secondary: true,
            },
        ),
        (
            unauthorized(),
            Addition::Permission(CredentialRefused::CredentialRefused),
        ),
        (
            forbidden(),
            Addition::Permission(CredentialRefused::NotPermitted),
        ),
        (stale(), Addition::Stale),
        (refused(), Addition::Refused(BlockedReason::NotListed)),
        (nothing(), Addition::Unresolved),
    ] {
        assert_eq!(parse_added(flow), addition);
    }
    for (flow, cleanup) in [
        (
            rate_limited(),
            Cleanup::RateLimited {
                delay_secs: None,
                secondary: true,
            },
        ),
        (
            unauthorized(),
            Cleanup::Permission(CredentialRefused::CredentialRefused),
        ),
        (
            forbidden(),
            Cleanup::Permission(CredentialRefused::NotPermitted),
        ),
        (stale(), Cleanup::Stale),
        (refused(), Cleanup::Refused(BlockedReason::NotListed)),
        (nothing(), Cleanup::Unresolved),
    ] {
        assert_eq!(parse_cleaned(flow), cleanup);
    }
    for (flow, removal) in [
        (
            rate_limited(),
            Removal::RateLimited {
                delay_secs: None,
                secondary: true,
            },
        ),
        (
            unauthorized(),
            Removal::Permission(CredentialRefused::CredentialRefused),
        ),
        (
            forbidden(),
            Removal::Permission(CredentialRefused::NotPermitted),
        ),
        (stale(), Removal::Stale),
        (refused(), Removal::Refused(BlockedReason::NotListed)),
        (nothing(), Removal::Unresolved),
    ] {
        assert_eq!(parse_removed(flow), removal);
    }
    for (flow, final_outcome) in [
        (
            rate_limited(),
            Final::RateLimited {
                delay_secs: None,
                secondary: true,
            },
        ),
        (
            unauthorized(),
            Final::Permission(CredentialRefused::CredentialRefused),
        ),
        (
            forbidden(),
            Final::Permission(CredentialRefused::NotPermitted),
        ),
        (stale(), Final::Stale),
        (refused(), Final::Refused(BlockedReason::NotListed)),
        (nothing(), Final::Unresolved),
    ] {
        assert_eq!(parse_final(flow), final_outcome);
    }
}

/// Only the by-id state read can say a review is gone: a null node, alone
/// or beside nothing but `NOT_FOUND` errors. Any other error beside a null
/// node, and the submit's own reply, prove nothing of the kind.
#[test]
fn only_a_null_node_read_names_a_missing_review() {
    for missing in [
        json!({"data": {"node": null}}),
        json!({
            "data": {"node": null},
            "errors": [{
                "type": "NOT_FOUND",
                "path": ["node"],
                "message": "Could not resolve to a node with the global id of 'PRR_x'",
            }],
        }),
    ] {
        assert_eq!(
            parse_review_state(read_flow(&missing.to_string())),
            Final::Missing,
            "{missing} names no review"
        );
    }
    for not_missing in [
        json!({"data": {"node": null}, "errors": [{"type": "FORBIDDEN"}]}),
        json!({"data": {"node": null}, "errors": [{"type": "NOT_FOUND"}, {"type": "RATE_LIMITED"}]}),
        json!({"data": {}}),
    ] {
        assert_ne!(
            parse_review_state(read_flow(&not_missing.to_string())),
            Final::Missing,
            "{not_missing} does not prove the review gone"
        );
    }
    assert_eq!(
        parse_final(read_flow(&json!({"data": {"node": null}}).to_string())),
        Final::Unresolved,
        "only the by-id state read may name a missing review"
    );
}

/// A final answer names the review whole — submit's reply and a state
/// read's alike: its stable id, one of the three pending-review states,
/// and the body it carries. A state this version does not carry is
/// unknown by name, never a guess, and an answer with a field missing
/// or typed otherwise names nothing.
#[test]
fn a_final_names_the_review_its_state_and_its_body() {
    // The submit's own reply, by its pointer.
    assert_eq!(
        parse_final(read_flow(
            &json!({
                "data": {"submitPullRequestReview": {"pullRequestReview": {
                    "id": "RV_1",
                    "state": "APPROVED",
                    "body": "the summary",
                }}}
            })
            .to_string()
        )),
        Final::Completed {
            review_id: "RV_1".to_string(),
            status: ReviewStatus::Submitted(SubmitEvent::Approve),
            body: "the summary".to_string(),
        }
    );
    // A fresh state read's reply, by the node pointer, for each state
    // GitHub's `PullRequestReviewState` names. `SUBMITTED` is a review
    // comment's state, never a review's, so it is unknown here.
    for (state, status) in [
        ("PENDING", ReviewStatus::Pending),
        ("COMMENTED", ReviewStatus::Submitted(SubmitEvent::Comment)),
        ("APPROVED", ReviewStatus::Submitted(SubmitEvent::Approve)),
        (
            "CHANGES_REQUESTED",
            ReviewStatus::Submitted(SubmitEvent::RequestChanges),
        ),
        ("DISMISSED", ReviewStatus::Dismissed),
        ("SUBMITTED", ReviewStatus::Unknown),
        ("A_STATE_THIS_VERSION_DOES_NOT_CARRY", ReviewStatus::Unknown),
    ] {
        assert_eq!(
            parse_final(read_flow(
                &json!({
                    "data": {"node": {
                        "id": "RV_1",
                        "state": state,
                        "body": "body text",
                    }}
                })
                .to_string()
            )),
            Final::Completed {
                review_id: "RV_1".to_string(),
                status,
                body: "body text".to_string(),
            },
            "{state} reads as {status:?}"
        );
    }
    // A review without its id, its body or its state; a state typed
    // otherwise; a null node: each names nothing.
    for losing in [
        json!({"data": {"node": {"state": "COMMENTED", "body": "b"}}}),
        json!({"data": {"node": {"id": "RV_1", "state": "COMMENTED"}}}),
        json!({"data": {"node": {"id": "RV_1", "body": "b"}}}),
        json!({"data": {"node": {"id": "RV_1", "state": 7, "body": "b"}}}),
        json!({"data": {"node": null}}),
        json!({"data": {}}),
    ] {
        assert_eq!(
            parse_final(read_flow(&losing.to_string())),
            Final::Unresolved,
            "{losing} names no whole review"
        );
    }
}

// -------------------------------------------------- the marker hunt

/// One page of comments as the pending read answers them, for a hunt's
/// own tests.
fn comment_page(comments: &[(&str, &str)]) -> Page<PendingComment> {
    Page {
        nodes: comments
            .iter()
            .map(|(id, body)| PendingComment {
                id: id.to_string(),
                body: body.to_string(),
            })
            .collect(),
        next: None,
    }
}

/// A hunt answers only whole proofs: one comment carrying the marker
/// exactly once is found — wherever in the bound's pages it sat — while
/// a second candidate or a doubled marker pauses the run as ambiguous;
/// absence needs the whole connection read, and a connection with pages
/// unread is uncertainty, never absence.
#[test]
fn a_marker_hunt_finds_one_comment_or_proves_it_absent() {
    let marker = item_marker(OP, ITEM_A);

    // Nothing seen, and the connection wholly read: absent.
    assert_eq!(
        MarkerSearch::new().outcome(true),
        Search::Absent,
        "a whole read without the marker proves it absent"
    );
    // Nothing seen, pages unread: uncertain, never absent.
    assert_eq!(MarkerSearch::new().outcome(false), Search::Uncertain);

    // One comment carries it exactly once: found, whether or not more
    // pages were asked for — one whole marker is proof enough.
    for connection_done in [true, false] {
        let mut hunt = MarkerSearch::new();
        hunt.seek(
            &comment_page(&[
                ("C_other", "unrelated prose"),
                ("C_found", &provisional_body(&marker, "the item's prose")),
            ]),
            &marker,
        );
        assert!(!hunt.is_lost());
        assert_eq!(
            hunt.outcome(connection_done),
            Search::Found {
                comment_id: "C_found".to_string()
            }
        );
    }

    // Found on the second page of a chain, after a page of none.
    let mut hunt = MarkerSearch::new();
    hunt.seek(
        &comment_page(&[("C_1", "no marker here"), ("C_2", "nor here")]),
        &marker,
    );
    assert_eq!(hunt.outcome(false), Search::Uncertain);
    let mut hunt = MarkerSearch::new();
    hunt.seek(&comment_page(&[("C_1", "no marker here")]), &marker);
    hunt.seek(
        &comment_page(&[("C_2", &provisional_body(&marker, "found here"))]),
        &marker,
    );
    assert_eq!(
        hunt.outcome(true),
        Search::Found {
            comment_id: "C_2".to_string()
        }
    );

    // Two comments carrying the same item marker: the marker names one
    // item, not two comments.
    let mut hunt = MarkerSearch::new();
    hunt.seek(
        &comment_page(&[
            ("C_1", &provisional_body(&marker, "one")),
            ("C_2", &provisional_body(&marker, "two")),
        ]),
        &marker,
    );
    assert!(hunt.is_lost());
    assert_eq!(hunt.outcome(true), Search::Ambiguous);

    // One comment carrying it twice: no body either write produced.
    let mut hunt = MarkerSearch::new();
    hunt.seek(
        &comment_page(&[("C_1", &format!("{marker}{marker}prose"))]),
        &marker,
    );
    assert!(hunt.is_lost());
    assert_eq!(hunt.outcome(true), Search::Ambiguous);

    // Candidates across pages: ambiguous as soon as the second arrives.
    let mut hunt = MarkerSearch::new();
    hunt.seek(
        &comment_page(&[("C_1", &provisional_body(&marker, "one"))]),
        &marker,
    );
    assert!(!hunt.is_lost());
    hunt.seek(
        &comment_page(&[("C_2", &provisional_body(&marker, "two"))]),
        &marker,
    );
    assert!(hunt.is_lost());
    assert_eq!(hunt.outcome(false), Search::Ambiguous);
}

/// A lost item write is never repeated on its own: an unanswered write
/// rides out unresolved, and the next reconciliation read either proves
/// the marker present — confirming the item by the provider's comment id
/// with no second write — or proves it absent, which alone permits the
/// runner's retry.
#[test]
fn a_lost_item_write_reconciles_by_its_marker_without_repeating() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);
    let marker = item_marker(OP, ITEM_A);
    let variables = AddReplyVariables {
        pull_request_review_id: "RV_7".to_string(),
        thread_id: "PRRT_parent".to_string(),
        body: provisional_body(&marker, "the frozen reply"),
    };

    // The write's reply never arrives.
    io.hand(None);
    assert_eq!(sender.add_reply(&variables), Addition::Unresolved);
    assert_eq!(io.sents().len(), 1, "one write, and no blind repeat");

    // The reconciliation read the runner asks next: the item's marker is
    // on one comment the provider recorded. The hunt confirms the item,
    // and the ledger records the provider's id.
    let marked = json!({
        "data": {"node": {"reviews": {
            "nodes": [review_fixture(
                "RV_7",
                json!({"id": ACTOR}),
                &operation_marker(OP),
                vec![("C_landed", &provisional_body(&marker, "the frozen reply"))],
            )],
            "pageInfo": {"hasNextPage": false},
        }}}
    })
    .to_string();
    io.hand(reply(200, Some(&marked)));
    let PendingRead::Reviews { page } = sender.pending_state(&PendingVariables::first(NODE)) else {
        panic!("the reconciliation read answers")
    };
    let mut hunt = MarkerSearch::new();
    for review in &page.nodes {
        hunt.seek(&review.comments, &marker);
    }
    assert_eq!(
        hunt.outcome(page.next.is_none() && page.nodes.iter().all(|r| r.comments.next.is_none())),
        Search::Found {
            comment_id: "C_landed".to_string()
        },
        "the marker proves the write landed; no repeat is asked"
    );
    assert_eq!(io.sents().len(), 2, "the write and its read, nothing more");

    // The absent side: a fresh hunt over a whole connection without the
    // marker proves it absent, which is what alone permits a retry.
    let mut absent = MarkerSearch::new();
    absent.seek(&comment_page(&[("C_other", "unrelated")]), &marker);
    assert_eq!(absent.outcome(true), Search::Absent);
}

// ------------------------------------------------- the item sends

/// The six item requests stand on the same one endpoint: each carries
/// its exact fixed document and only its variables object, and each
/// parse answers what its reply named. A submit rides the frozen
/// decision's event with the clean summary — an empty approval's
/// included — and a deletion rides the exact adopted review's id alone.
#[test]
fn the_sender_writes_cleans_submits_and_deletes_over_its_documents() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);
    let marker = item_marker(OP, ITEM_A);

    // A fresh line's write, whose thread holds its one comment.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"addPullRequestReviewThread": {"thread": {
                    "id": "TH_1",
                    "comments": {"nodes": [{"id": "C_1", "body": "the provisional body"}]},
                }}}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.add_thread(&AddThreadVariables {
            pull_request_review_id: "RV_7".to_string(),
            path: "src/lib.rs".to_string(),
            line: 42,
            side: DiffSide::New,
            body: provisional_body(&marker, "the frozen comment"),
        }),
        Addition::Added {
            thread_id: Some("TH_1".to_string()),
            comment_id: "C_1".to_string(),
        }
    );

    // A reply's write.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"addPullRequestReviewThreadReply": {"comment": {
                    "id": "C_2",
                    "body": "the provisional reply",
                }}}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.add_reply(&AddReplyVariables {
            pull_request_review_id: "RV_7".to_string(),
            thread_id: "PRRT_parent".to_string(),
            body: provisional_body(&item_marker(OP, ITEM_B), "the frozen reply"),
        }),
        Addition::Added {
            thread_id: None,
            comment_id: "C_2".to_string(),
        }
    );

    // A cleanup edit lands with the clean prose.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"updatePullRequestReviewComment": {"pullRequestReviewComment": {
                    "id": "C_1",
                    "body": "the frozen comment",
                }}}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.clean_comment(&CommentCleanupVariables {
            comment_id: "C_1".to_string(),
            body: "the frozen comment".to_string(),
        }),
        Cleanup::Cleaned {
            comment_id: "C_1".to_string(),
            body: "the frozen comment".to_string(),
        }
    );

    // An empty approval submits the review as submitted with an empty
    // body — the marker atomically gone.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"submitPullRequestReview": {"pullRequestReview": {
                    "id": "RV_7",
                    "state": "APPROVED",
                    "body": "",
                }}}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.submit_review(&SubmitReviewVariables {
            review_id: "RV_7".to_string(),
            event: SubmitEvent::Approve,
            body: String::new(),
        }),
        Final::Completed {
            review_id: "RV_7".to_string(),
            status: ReviewStatus::Submitted(SubmitEvent::Approve),
            body: String::new(),
        }
    );

    // A lost final reply's reconciliation read: still pending, marker
    // body whole.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"node": {
                    "id": "RV_7",
                    "state": "PENDING",
                    "body": operation_marker(OP),
                }}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.review_state(&ReviewStateVariables {
            review_id: "RV_7".to_string(),
        }),
        Final::Completed {
            review_id: "RV_7".to_string(),
            status: ReviewStatus::Pending,
            body: operation_marker(OP),
        }
    );

    // The exact adopted review's deletion.
    io.hand(reply(
        200,
        Some(
            &json!({
                "data": {"deletePullRequestReview": {"clientMutationId": OP}}
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.delete_review(&DeleteVariables {
            review_id: "RV_7".to_string(),
        }),
        Removal::Removed
    );

    // Six sends, each to the one fixed endpoint, each the exact document
    // its step names beside its own variables.
    let sent = io.sents();
    assert_eq!(sent.len(), 6);
    let documents = [
        ADD_THREAD_MUTATION,
        ADD_REPLY_MUTATION,
        COMMENT_CLEANUP_MUTATION,
        SUBMIT_REVIEW_MUTATION,
        REVIEW_STATE_QUERY,
        DELETE_REVIEW_MUTATION,
    ];
    for (one, document) in sent.iter().zip(documents) {
        assert_eq!(one.url, GITHUB_GRAPHQL_URL);
        let parsed: Value = serde_json::from_str(&one.body).unwrap();
        assert_eq!(
            parsed.get("query").and_then(Value::as_str),
            Some(document),
            "the fixed document rides whole"
        );
        assert_eq!(
            parsed
                .get("variables")
                .and_then(Value::as_object)
                .map(|v| v.len()),
            Some(1),
            "one variables object, whatever the document's top-level keys are"
        );
    }
    // The line's own values, the reply's anchor, the cleanup's id and
    // body, the submit's event and empty summary, the state's top-level
    // review id, and the deletion's one id.
    let values: Vec<Value> = sent
        .iter()
        .map(|one| serde_json::from_str(&one.body).unwrap())
        .collect();
    assert_eq!(
        values[0]
            .pointer("/variables/input/pullRequestReviewId")
            .and_then(Value::as_str),
        Some("RV_7")
    );
    assert_eq!(
        values[0]
            .pointer("/variables/input/path")
            .and_then(Value::as_str),
        Some("src/lib.rs")
    );
    assert_eq!(
        values[0]
            .pointer("/variables/input/line")
            .and_then(Value::as_u64),
        Some(42)
    );
    assert_eq!(
        values[0]
            .pointer("/variables/input/side")
            .and_then(Value::as_str),
        Some("RIGHT")
    );
    assert_eq!(
        values[0]
            .pointer("/variables/input/body")
            .and_then(Value::as_str),
        Some(provisional_body(&marker, "the frozen comment").as_str())
    );
    assert_eq!(
        values[1]
            .pointer("/variables/input/pullRequestReviewThreadId")
            .and_then(Value::as_str),
        Some("PRRT_parent")
    );
    assert_eq!(
        values[2]
            .pointer("/variables/input/pullRequestReviewCommentId")
            .and_then(Value::as_str),
        Some("C_1")
    );
    assert_eq!(
        values[2]
            .pointer("/variables/input/body")
            .and_then(Value::as_str),
        Some("the frozen comment")
    );
    assert_eq!(
        values[3]
            .pointer("/variables/input/event")
            .and_then(Value::as_str),
        Some("APPROVE")
    );
    assert_eq!(
        values[3]
            .pointer("/variables/input/body")
            .and_then(Value::as_str),
        Some(""),
        "the empty approval's summary rides empty, not omitted"
    );
    assert_eq!(
        values[4]
            .pointer("/variables/reviewId")
            .and_then(Value::as_str),
        Some("RV_7")
    );
    assert!(
        values[4].pointer("/variables/input").is_none(),
        "a state read names no input object"
    );
    assert_eq!(
        values[5]
            .pointer("/variables/input/pullRequestReviewId")
            .and_then(Value::as_str),
        Some("RV_7")
    );
}

/// Every item write stands behind the same clearance as the read and the
/// create: a refusal sends nothing and names its reason, while an
/// admitted write carries the transport's rate limit, its credential
/// refusal and its stale anchor whole.
#[test]
fn every_item_write_stands_behind_the_same_clearance() {
    let io = TestIo::new();
    let limits = GithubLimits::new();
    let authority = TestAuthority::grant("tok");
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    // A refusal: no request, the reason's own sentence.
    authority.refusals(BlockedReason::DetailRequired);
    let refused = BlockedReason::DetailRequired;
    assert_eq!(
        sender.add_thread(&AddThreadVariables {
            pull_request_review_id: "RV_7".to_string(),
            path: "a.rs".to_string(),
            line: 1,
            side: DiffSide::Old,
            body: "body".to_string(),
        }),
        Addition::Refused(refused)
    );
    assert_eq!(
        sender.add_reply(&AddReplyVariables {
            pull_request_review_id: "RV_7".to_string(),
            thread_id: "PRRT_parent".to_string(),
            body: "body".to_string(),
        }),
        Addition::Refused(refused)
    );
    assert_eq!(
        sender.clean_comment(&CommentCleanupVariables {
            comment_id: "C_1".to_string(),
            body: "clean".to_string(),
        }),
        Cleanup::Refused(refused)
    );
    assert_eq!(
        sender.submit_review(&SubmitReviewVariables {
            review_id: "RV_7".to_string(),
            event: SubmitEvent::Comment,
            body: "summary".to_string(),
        }),
        Final::Refused(refused)
    );
    assert_eq!(
        sender.review_state(&ReviewStateVariables {
            review_id: "RV_7".to_string(),
        }),
        Final::Refused(refused)
    );
    assert_eq!(
        sender.delete_review(&DeleteVariables {
            review_id: "RV_7".to_string(),
        }),
        Removal::Refused(refused)
    );
    assert!(io.sents().is_empty(), "a refusal sends nothing, ever");

    // Admitted: the reply's own kinds ride through whole.
    authority.grants("tok");
    io.hand(reply(
        200,
        Some(
            &json!({
                "errors": [{
                    "message": "You have exceeded a secondary rate limit",
                    "type": "RATE_LIMITED",
                }]
            })
            .to_string(),
        ),
    ));
    assert_eq!(
        sender.add_reply(&AddReplyVariables {
            pull_request_review_id: "RV_7".to_string(),
            thread_id: "PRRT_parent".to_string(),
            body: "body".to_string(),
        }),
        Addition::RateLimited {
            delay_secs: None,
            secondary: true,
        }
    );
    // The rate-limited reply set the provider's deadline; this test's next
    // write is about clearance, so step past it.
    io.take_now(NOW + 3_601);
    io.hand(reply(403, Some("{\"message\":\"forbidden\"}")));
    assert_eq!(
        sender.clean_comment(&CommentCleanupVariables {
            comment_id: "C_1".to_string(),
            body: "clean".to_string(),
        }),
        Cleanup::Permission(CredentialRefused::NotPermitted)
    );
    io.hand(reply(422, None));
    assert_eq!(
        sender.submit_review(&SubmitReviewVariables {
            review_id: "RV_7".to_string(),
            event: SubmitEvent::Approve,
            body: String::new(),
        }),
        Final::Stale
    );
    assert_eq!(io.sents().len(), 3, "three admitted requests, three sends");
}
