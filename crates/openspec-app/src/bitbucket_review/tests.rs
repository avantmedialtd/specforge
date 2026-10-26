//! The BitBucket review protocol's tests (`pull-request-actions`:
//! *BitBucket Submission Is Sequential and Durably Partial*): every path
//! is the snapshot's own, every content write stands behind the commit
//! guard, a pushed basis sends nothing and stops what follows, a lost
//! write is never repeated, and the comments read answers only within its
//! bounds.

use super::*;
use serde_json::json;

use crate::events::PullRequestProvider;
use crate::github_review::{
    item_marker, occurrences, operation_marker, provisional_body, MarkerSearch, Occurrences, Search,
};
use crate::review_action_io::{ActionBody, ActionReply};

const NOW: u64 = 1_800_000_000;
const HEAD: &str = "abc123abc123abc123abc123abc123abc123ab";
const BASE: &str = "def456def456def456def456def456def456de";
const MOVED: &str = "9999999999999999999999999999999999999999";
const OP: &str = "8c1d7a1e-2b3f-4c5d-9e0f-1a2b3c4d5e6f";
const ITEM_A: &str = "1e6f9d0c-3a4b-5c6d-8e9f-0a1b2c3d4e5f";

// ------------------------------------------------------ the test fakes

/// One request the test io was asked to send: its URL and, for a POST,
/// the JSON it carried. A GET carries no body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OneSent {
    url: String,
    body: Option<String>,
}

/// The io one test's sender transports with: the replies it hands per
/// call, in the order asked, and every request recorded with its URL and
/// body. Its clock is a test's to move.
#[derive(Default)]
struct TestIo {
    now: std::sync::Mutex<u64>,
    bitbucket: std::sync::Mutex<std::collections::VecDeque<Option<ActionReply>>>,
    sent: std::sync::Mutex<Vec<OneSent>>,
}

impl TestIo {
    fn new() -> Self {
        Self {
            now: std::sync::Mutex::new(NOW),
            ..Self::default()
        }
    }

    fn hand(&self, reply: Option<ActionReply>) {
        self.bitbucket.lock().unwrap().push_back(reply);
    }

    fn sents(&self) -> Vec<OneSent> {
        self.sent.lock().unwrap().clone()
    }
}

impl ActionIo for TestIo {
    fn now(&self) -> u64 {
        *self.now.lock().unwrap()
    }

    fn github_post(&self, _token: &str, _body: String) -> Option<ActionReply> {
        None
    }

    fn bitbucket_get(&self, _username: &str, _token: &str, url: &str) -> Option<ActionReply> {
        self.sent.lock().unwrap().push(OneSent {
            url: url.to_string(),
            body: None,
        });
        self.bitbucket.lock().unwrap().pop_front().flatten()
    }

    fn bitbucket_post(
        &self,
        _username: &str,
        _token: &str,
        url: &str,
        json: String,
    ) -> Option<ActionReply> {
        self.sent.lock().unwrap().push(OneSent {
            url: url.to_string(),
            body: Some(json),
        });
        self.bitbucket.lock().unwrap().pop_front().flatten()
    }
}

/// A clearance a test's sender clears with: the fresh credential's two
/// parts, or the reason it refuses.
struct TestAuthority {
    cleared: std::sync::Mutex<Result<(String, String), BlockedReason>>,
}

impl TestAuthority {
    fn grant() -> Self {
        Self {
            cleared: std::sync::Mutex::new(Ok(("account".to_string(), "tok".to_string()))),
        }
    }

    fn refusals(&self, reason: BlockedReason) {
        *self.cleared.lock().unwrap() = Err(reason);
    }

    /// The clearance grants again, as a fresh resolution does.
    fn grants(&self) {
        *self.cleared.lock().unwrap() = Ok(("account".to_string(), "tok".to_string()));
    }
}

impl BitbucketAuthority for TestAuthority {
    fn clear_to_send(&self) -> Result<(String, String), BlockedReason> {
        self.cleared.lock().unwrap().clone()
    }
}

/// One sender over one test's io and limits, cleared while `enabled`
/// holds.
fn make_sender<'a>(
    io: &'a TestIo,
    limits: &'a BitbucketLimits,
    authority: &'a TestAuthority,
    enabled: &'a dyn Fn() -> bool,
) -> BitbucketReviewSender<'a> {
    BitbucketReviewSender::new(limits, io, enabled, authority)
}

// -------------------------------------------------------- the fixtures

/// The canonical reference one snapshot row supplies.
fn reference() -> PullRequestReference {
    PullRequestReference {
        provider: PullRequestProvider::Bitbucket,
        owner: "acme".to_string(),
        repo: "api".to_string(),
        number: 7,
    }
}

/// The frozen basis every guarded write is made under.
fn basis() -> CommitBasis {
    CommitBasis {
        head: HEAD.to_string(),
        base: BASE.to_string(),
    }
}

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

/// One pull request resource, as the commit read asks it.
fn pull_request(head: &str, base: &str) -> String {
    json!({
        "source": { "commit": { "hash": head } },
        "destination": { "commit": { "hash": base } },
    })
    .to_string()
}

/// One comments page, as the reconciliation read asks it.
fn comments(values: Value, next: Value) -> String {
    json!({ "values": values, "next": next }).to_string()
}

// ------------------------------------------------------- the paths

/// Every path is the trusted reference's own: both components encoded as
/// one segment, the number last, the comments asked in 100-object pages,
/// and nothing a reply could have named.
#[test]
fn the_paths_are_snapshot_derived_and_bounded() {
    assert_eq!(
        pull_request_url(&reference()),
        "https://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7"
    );
    assert_eq!(
        comments_url(&reference()),
        "https://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7/comments?pagelen=100"
    );
    // Components that would change the path's structure are encoded as
    // one segment each, never spliced in.
    let awkward = PullRequestReference {
        provider: PullRequestProvider::Bitbucket,
        owner: "my ws".to_string(),
        repo: "a/b".to_string(),
        number: 7,
    };
    assert_eq!(
        pull_request_url(&awkward),
        "https://api.bitbucket.org/2.0/repositories/my%20ws/a%2Fb/pullrequests/7"
    );
    assert!(!pull_request_url(&awkward).contains("my ws"));
    assert!(!pull_request_url(&awkward).contains("a/b"));
    for url in [pull_request_url(&reference()), comments_url(&reference())] {
        assert!(url.starts_with("https://api.bitbucket.org/2.0/"));
        assert!(!url.contains('@'), "{url} carries no user information");
    }
}

// --------------------------------------------------- the commit guard

/// A commit read proves the basis only when it answers both commits
/// whole and they are exactly the frozen pair; every other answer — a
/// moved commit, a missing or typed-otherwise field, a rate limit, a
/// refusal, nothing readable — matches nothing.
#[test]
fn a_commit_read_matches_only_both_frozen_commits() {
    let read = |body: &str| parse_commits(read_flow(body));

    let current = read(&pull_request(HEAD, BASE));
    assert_eq!(
        current,
        CommitRead::Read {
            head: HEAD.to_string(),
            base: BASE.to_string(),
        }
    );
    assert!(current.matches(&basis()));
    // Either commit moved: no match.
    for (head, base) in [(MOVED, BASE), (HEAD, MOVED), (MOVED, MOVED)] {
        let moved = read(&pull_request(head, base));
        assert!(
            !moved.matches(&basis()),
            "{head}/{base} is not the frozen basis"
        );
    }
    // A missing hash, one typed otherwise, non-JSON, and a JSON value
    // that is no object: nothing to compare.
    for losing in [
        json!({ "source": { "commit": { "hash": HEAD } } }),
        json!({ "destination": { "commit": { "hash": BASE } } }),
        json!({ "source": { "commit": { "hash": 42 } }, "destination": { "commit": { "hash": BASE } } }),
    ] {
        assert_eq!(read(&losing.to_string()), CommitRead::Unresolved);
    }
    assert_eq!(read("not json"), CommitRead::Unresolved);
    assert_eq!(read("[]"), CommitRead::Unresolved);
    // The transport's own kinds ride whole.
    assert_eq!(
        parse_commits(ActionFlow::Sent(ActionVerdict::RateLimited {
            delay_secs: Some(30),
            secondary: false,
        })),
        CommitRead::RateLimited {
            delay_secs: Some(30),
            secondary: false,
        }
    );
    assert_eq!(
        parse_commits(ActionFlow::Sent(ActionVerdict::Permission {
            insufficient: CredentialRefused::NotPermitted,
        })),
        CommitRead::Permission(CredentialRefused::NotPermitted)
    );
    assert_eq!(
        parse_commits(ActionFlow::Refused {
            reason: BlockedReason::NotListed,
        }),
        CommitRead::Refused(BlockedReason::NotListed)
    );
    assert_eq!(
        parse_commits(ActionFlow::Sent(ActionVerdict::Stale)),
        CommitRead::Unresolved,
        "a read names no stale anchor"
    );
    // An off-host attempt is not even sent, and proves nothing.
    assert_eq!(parse_commits(ActionFlow::OffHost), CommitRead::Unresolved);
}

// -------------------------------------------------- the comments read

/// A comments page answers each comment's numbered id and raw body —
/// empty where a deleted one carries none — and only a continuation the
/// credential may follow. An over-bound page, a comment without its
/// number, a body typed otherwise, a non-array listing, or a next link
/// off the API answers nothing, so a marker hunt over it can never call
/// absence.
#[test]
fn a_comments_page_answers_its_numbers_its_bodies_and_one_followable_next() {
    // A whole page: one body, one empty body (a deleted comment's), one
    // comment carrying no content at all.
    let page = parse_comments(read_flow(&comments(
        json!([
            {"id": 11, "content": {"raw": "first"}},
            {"id": 12, "content": {"raw": ""}},
            {"id": 13},
        ]),
        Value::Null,
    )));
    let CommentsRead::Page(page) = page else {
        panic!("a whole page answers");
    };
    assert_eq!(
        page.nodes,
        vec![
            crate::github_review::PendingComment {
                id: "11".to_string(),
                body: "first".to_string(),
            },
            crate::github_review::PendingComment {
                id: "12".to_string(),
                body: String::new(),
            },
            crate::github_review::PendingComment {
                id: "13".to_string(),
                body: String::new(),
            },
        ]
    );
    assert_eq!(page.next, None);

    // A continuation the credential may follow rides whole.
    let next = format!("{}&page=2", comments_url(&reference()));
    let CommentsRead::Page(continued) =
        parse_comments(read_flow(&comments(json!([]), json!(next))))
    else {
        panic!("a followable continuation is answered");
    };
    assert_eq!(continued.next.as_deref(), Some(next.as_str()));

    // A next link off the API, a doubled page's bound, a comment
    // without its number, a body typed otherwise, a listing that is no
    // array: nothing, however much of a page was read.
    for losing in [
        comments(
            json!([{"id": 1, "content": {"raw": "a"}}]),
            json!("https://evil.example/2.0/repositories/acme/api/pullrequests/7/comments?page=2"),
        ),
        comments(
            json!((0..101)
                .map(|id| json!({"id": id, "content": {"raw": "b"}}))
                .collect::<Vec<_>>()),
            Value::Null,
        ),
        comments(json!([{"content": {"raw": "no id"}}]), Value::Null),
        comments(
            json!([{"id": "1", "content": {"raw": "string id"}}]),
            Value::Null,
        ),
        comments(json!([{"id": 1, "content": {"raw": 42}}]), Value::Null),
        json!({ "values": {}, "next": null }).to_string(),
        json!({ "next": null }).to_string(),
        json!({ "values": [] , "next": 7 }).to_string(),
    ] {
        assert_eq!(
            parse_comments(read_flow(&losing)),
            CommentsRead::Unresolved,
            "{losing} names no whole page"
        );
    }

    // The transport's kinds, whole — a rate limit from either its status
    // or its body, and the one refusal it can name.
    assert_eq!(
        parse_comments(reply_flow(429, None)),
        CommentsRead::RateLimited {
            delay_secs: None,
            secondary: false,
        }
    );
    assert_eq!(
        parse_comments(action_flow(ActionVerdict::RateLimited {
            delay_secs: None,
            secondary: true,
        })),
        CommentsRead::RateLimited {
            delay_secs: None,
            secondary: true,
        }
    );
    assert_eq!(
        parse_comments(reply_flow(403, None)),
        CommentsRead::Permission(CredentialRefused::NotPermitted)
    );
    assert_eq!(
        parse_comments(ActionFlow::Refused {
            reason: BlockedReason::DetailRequired,
        }),
        CommentsRead::Refused(BlockedReason::DetailRequired)
    );

    // A marker hunt over a parsed page finds the item's one comment.
    let marker = item_marker(OP, ITEM_A);
    let CommentsRead::Page(page) = parse_comments(read_flow(&comments(
        json!([
            {"id": 21, "content": {"raw": "unrelated"}},
            {"id": 22, "content": {"raw": provisional_body(&marker, "the reply")}},
        ]),
        Value::Null,
    ))) else {
        panic!("a whole page answers");
    };
    let mut hunt = MarkerSearch::new();
    hunt.seek(&page, &marker);
    assert_eq!(
        hunt.outcome(page.next.is_none()),
        Search::Found {
            comment_id: "22".to_string()
        }
    );
    // And only once: a doubled marker is ambiguous, never a finding.
    assert_eq!(
        occurrences(&provisional_body(&marker, "x"), &marker),
        Occurrences::One
    );
}

// ---------------------------------------------------- the write bodies

/// An inline write names its path and exactly one side's line — `to` the
/// new side, `from` the old — with its provisional body; a reply names
/// the thread's numeric parent; the summary is one general comment with
/// no anchor and no parent. The body's marker rides inside `content.raw`
/// and nowhere else.
#[test]
fn an_inline_write_names_its_path_side_and_provisional_body() {
    let marker = item_marker(OP, ITEM_A);
    let body = provisional_body(&marker, "the frozen prose");
    let inline_keys = |value: &Value| -> std::collections::BTreeSet<String> {
        value
            .pointer("/inline")
            .and_then(Value::as_object)
            .unwrap()
            .keys()
            .cloned()
            .collect()
    };
    let set = |names: &[&str]| -> std::collections::BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    };
    for (side, anchor, other) in [(DiffSide::New, "to", "from"), (DiffSide::Old, "from", "to")] {
        let value = Value::from(&InlineVariables {
            path: "src/lib.rs".to_string(),
            line: 42,
            side,
            body: body.clone(),
        });
        assert_eq!(
            value.pointer("/content/raw").and_then(Value::as_str),
            Some(body.as_str())
        );
        assert_eq!(
            value.pointer("/inline/path").and_then(Value::as_str),
            Some("src/lib.rs")
        );
        assert_eq!(
            value
                .pointer(&format!("/inline/{anchor}"))
                .and_then(Value::as_u64),
            Some(42),
            "{side:?} anchors the {anchor} line"
        );
        assert!(
            value.pointer(&format!("/inline/{other}")).is_none(),
            "{side:?} anchors no {other} line"
        );
        assert_eq!(
            inline_keys(&value),
            set(&["path", anchor]),
            "only the path and the one side ride the anchor"
        );
    }

    // A reply: the parent number and the body, nothing else.
    let value = Value::from(&ReplyVariables {
        in_reply_to: 12_345,
        body: provisional_body(&marker, "the reply"),
    });
    assert_eq!(
        value.pointer("/parent/id").and_then(Value::as_u64),
        Some(12_345)
    );
    assert_eq!(
        value.pointer("/content/raw").and_then(Value::as_str),
        Some(provisional_body(&marker, "the reply").as_str())
    );
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        set(&["content", "parent"])
    );
    assert!(value.pointer("/inline").is_none());

    // The summary: content alone, and the operation's own marker where
    // the operation carries one.
    let summary = provisional_body(&operation_marker(OP), "the summary");
    let value = Value::from(&SummaryVariables {
        body: summary.clone(),
    });
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        set(&["content"])
    );
    assert_eq!(
        value.pointer("/content/raw").and_then(Value::as_str),
        Some(summary.as_str())
    );
    assert!(value.pointer("/inline").is_none());
    assert!(value.pointer("/parent").is_none());
}

// ---------------------------------------------------- the guarded send

/// Helpers for the guard's tests: one flow of an error-naming status
/// reply and one of a bare verdict.
fn reply_flow(status: u16, body: Option<&str>) -> ActionFlow {
    ActionFlow::Sent(crate::review_action_io::classify(reply(status, body), NOW))
}

fn action_flow(verdict: ActionVerdict) -> ActionFlow {
    ActionFlow::Sent(verdict)
}

/// One write's own answer names its numbered id, or names nothing: the
/// id is the ledger's own value, whatever else the reply carried.
#[test]
fn a_write_answer_names_its_numbered_comment_or_nothing() {
    assert_eq!(
        parse_written(read_flow(
            &json!({"id": 99, "content": {"raw": "the provisional body"}}).to_string()
        )),
        WriteOutcome::Written {
            comment_id: "99".to_string()
        }
    );
    // A string id, no id, a non-JSON body, a dataless 2xx: nothing.
    for losing in [
        json!({"id": "99"}),
        json!({"content": {"raw": "no id"}}),
        json!({"id": null}),
    ] {
        assert_eq!(
            parse_written(read_flow(&losing.to_string())),
            WriteOutcome::Unresolved,
            "{losing} names no id"
        );
    }
    assert_eq!(
        parse_written(read_flow("not json")),
        WriteOutcome::Unresolved
    );
    assert_eq!(parse_written(read_flow("{}")), WriteOutcome::Unresolved);
    // The transport's kinds ride whole.
    assert_eq!(
        parse_written(action_flow(ActionVerdict::RateLimited {
            delay_secs: Some(10),
            secondary: true,
        })),
        WriteOutcome::RateLimited {
            delay_secs: Some(10),
            secondary: true,
        }
    );
    assert_eq!(
        parse_written(action_flow(ActionVerdict::Permission {
            insufficient: CredentialRefused::CredentialRefused,
        })),
        WriteOutcome::Permission(CredentialRefused::CredentialRefused)
    );
    assert_eq!(
        parse_written(action_flow(ActionVerdict::Stale)),
        WriteOutcome::Stale
    );
    assert_eq!(
        parse_written(ActionFlow::Refused {
            reason: BlockedReason::StaleCommits,
        }),
        WriteOutcome::Refused(BlockedReason::StaleCommits)
    );
    assert_eq!(parse_written(ActionFlow::OffHost), WriteOutcome::Unresolved);
}

/// Each of the three content writes stands behind the same guard: a
/// matching commit read before it, its own POST to the comments path, and
/// a matching read after an answer that may have landed. The POST bodies
/// are exactly the inline anchor, the numbering parent and the bare
/// content, and one request answers one item.
#[test]
fn every_content_write_reads_the_basis_before_and_after_its_post() {
    let io = TestIo::new();
    let limits = BitbucketLimits::new();
    let authority = TestAuthority::grant();
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);
    let marker = item_marker(OP, ITEM_A);

    let hand_match = |id: u64| {
        io.hand(reply(200, Some(&pull_request(HEAD, BASE))));
        io.hand(reply(200, Some(&json!({"id": id}).to_string())));
        io.hand(reply(200, Some(&pull_request(HEAD, BASE))));
    };
    hand_match(81);
    assert_eq!(
        sender.write_inline(
            &reference(),
            &basis(),
            &InlineVariables {
                path: "src/lib.rs".to_string(),
                line: 42,
                side: DiffSide::New,
                body: provisional_body(&marker, "the comment"),
            },
        ),
        WriteRun::Sent(WriteOutcome::Written {
            comment_id: "81".to_string()
        })
    );
    hand_match(82);
    assert_eq!(
        sender.write_reply(
            &reference(),
            &basis(),
            &ReplyVariables {
                in_reply_to: 12_345,
                body: provisional_body(&marker, "the reply"),
            },
        ),
        WriteRun::Sent(WriteOutcome::Written {
            comment_id: "82".to_string()
        })
    );
    hand_match(83);
    assert_eq!(
        sender.write_summary(
            &reference(),
            &basis(),
            &SummaryVariables {
                body: provisional_body(&operation_marker(OP), "the summary"),
            },
        ),
        WriteRun::Sent(WriteOutcome::Written {
            comment_id: "83".to_string()
        })
    );

    // Nine requests: for each write, the before-read, the POST and the
    // after-read, in that order, each to the snapshot's own paths.
    let sent = io.sents();
    assert_eq!(sent.len(), 9);
    for (at, one) in sent.iter().enumerate() {
        let expected = if at % 3 == 1 {
            comments_url(&reference())
        } else {
            pull_request_url(&reference())
        };
        assert_eq!(one.url, expected, "request {at} goes to its own path");
        assert_eq!(
            one.body.is_some(),
            at % 3 == 1,
            "only the POST carries a body"
        );
    }
    // The three POST bodies, whole.
    let bodies: Vec<Value> = sent
        .iter()
        .skip(1)
        .step_by(3)
        .map(|one| serde_json::from_str(one.body.as_ref().unwrap()).unwrap())
        .collect();
    assert_eq!(bodies.len(), 3);
    assert_eq!(
        bodies[0].pointer("/inline/to").and_then(Value::as_u64),
        Some(42)
    );
    assert_eq!(
        bodies[1].pointer("/parent/id").and_then(Value::as_u64),
        Some(12_345)
    );
    assert!(bodies[2].pointer("/inline").is_none());
    assert!(bodies[2].pointer("/parent").is_none());
    assert_eq!(
        bodies[2].pointer("/content/raw").and_then(Value::as_str),
        Some(provisional_body(&operation_marker(OP), "the summary").as_str())
    );
}

/// A push between items stops the next write before it sends anything:
/// the before-read proves another commit, and no POST is ever made under
/// a basis that is no longer the frozen one.
#[test]
fn a_push_before_a_write_sends_no_mutation() {
    let io = TestIo::new();
    let limits = BitbucketLimits::new();
    let authority = TestAuthority::grant();
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    io.hand(reply(200, Some(&pull_request(MOVED, BASE))));
    assert_eq!(
        sender.write_reply(
            &reference(),
            &basis(),
            &ReplyVariables {
                in_reply_to: 1,
                body: "body".to_string(),
            },
        ),
        WriteRun::NotSent(CommitRead::Read {
            head: MOVED.to_string(),
            base: BASE.to_string(),
        })
    );
    let sent = io.sents();
    assert_eq!(sent.len(), 1, "the before-read ran; nothing was written");
    assert!(sent.iter().all(|one| one.body.is_none()));
}

/// A push across a successful write keeps the write as a confirmed
/// partial effect: the after-read proves the moved basis, the run stops,
/// and the write is never repeated. A lost write across the same push
/// stays uncertain in exactly the same way.
#[test]
fn a_push_across_a_write_keeps_it_a_partial_effect() {
    for (answer, outcome) in [
        (
            reply(200, Some(&json!({"id": 91}).to_string())),
            WriteOutcome::Written {
                comment_id: "91".to_string(),
            },
        ),
        (None, WriteOutcome::Unresolved),
    ] {
        let io = TestIo::new();
        let limits = BitbucketLimits::new();
        let authority = TestAuthority::grant();
        let enabled = || true;
        let sender = make_sender(&io, &limits, &authority, &enabled);

        io.hand(reply(200, Some(&pull_request(HEAD, BASE))));
        io.hand(answer);
        io.hand(reply(200, Some(&pull_request(HEAD, MOVED))));
        let run = sender.write_inline(
            &reference(),
            &basis(),
            &InlineVariables {
                path: "a.rs".to_string(),
                line: 1,
                side: DiffSide::Old,
                body: "body".to_string(),
            },
        );
        assert_eq!(
            run,
            WriteRun::BasisMoved {
                outcome: outcome.clone(),
                after: CommitRead::Read {
                    head: HEAD.to_string(),
                    base: MOVED.to_string(),
                },
            }
        );
        // The before-read, one POST, the after-read: the write is never
        // sent a second time, whatever its answer was.
        let sent = io.sents();
        assert_eq!(sent.len(), 3);
        assert_eq!(
            sent.iter().filter(|one| one.body.is_some()).count(),
            1,
            "one write, one POST"
        );
    }
}

/// An after-read that proves nothing leaves the write unverified: a
/// rate-limited, refused or unreadable read never lets the run continue
/// as if the basis still held, and the write is not repeated.
#[test]
fn an_unreadable_after_read_leaves_the_write_unverified() {
    for after in [reply(429, None), None, reply(403, None)] {
        let io = TestIo::new();
        let limits = BitbucketLimits::new();
        let authority = TestAuthority::grant();
        let enabled = || true;
        let sender = make_sender(&io, &limits, &authority, &enabled);

        io.hand(reply(200, Some(&pull_request(HEAD, BASE))));
        io.hand(reply(200, Some(&json!({"id": 92}).to_string())));
        io.hand(after);
        let run = sender.write_summary(
            &reference(),
            &basis(),
            &SummaryVariables {
                body: "summary".to_string(),
            },
        );
        let WriteRun::Unverified { outcome, after } = run else {
            panic!("an unproved after-read leaves the write unverified")
        };
        assert_eq!(
            outcome,
            WriteOutcome::Written {
                comment_id: "92".to_string()
            }
        );
        assert!(
            !matches!(after, CommitRead::Read { .. }),
            "the read proved no commits"
        );
        assert_eq!(
            io.sents().iter().filter(|one| one.body.is_some()).count(),
            1,
            "one write, no repeat"
        );
    }
}

/// A clearance refusal reaches the guard before any request: nothing is
/// sent at all, and the write names the gate's own reason. A rate-limited
/// before-read sends the write nothing and carries its delay.
#[test]
fn a_refused_or_limited_before_read_keeps_the_write_unsent() {
    let io = TestIo::new();
    let limits = BitbucketLimits::new();
    let authority = TestAuthority::grant();
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    // The clearance refuses: not even the before-read is sent.
    authority.refusals(BlockedReason::DetailRequired);
    assert_eq!(
        sender.write_reply(
            &reference(),
            &basis(),
            &ReplyVariables {
                in_reply_to: 1,
                body: "body".to_string(),
            },
        ),
        WriteRun::NotSent(CommitRead::Refused(BlockedReason::DetailRequired))
    );
    assert!(io.sents().is_empty());

    // Admitted but rate-limited before the write: no POST, the delay
    // carried whole.
    authority.grants();
    io.hand(reply(429, None));
    assert_eq!(
        sender.write_reply(
            &reference(),
            &basis(),
            &ReplyVariables {
                in_reply_to: 1,
                body: "body".to_string(),
            },
        ),
        WriteRun::NotSent(CommitRead::RateLimited {
            delay_secs: None,
            secondary: false,
        })
    );
    let sent = io.sents();
    assert_eq!(sent.len(), 1, "the before-read alone was sent");
    assert!(sent[0].body.is_none());
}

/// A write whose own answer is a refusal or a stale anchor is sent once
/// and never followed by another: no after-read, no retry, nothing else.
#[test]
fn a_non_retryable_write_answer_ends_the_step() {
    let io = TestIo::new();
    let limits = BitbucketLimits::new();
    let authority = TestAuthority::grant();
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    io.hand(reply(200, Some(&pull_request(HEAD, BASE))));
    io.hand(reply(403, None));
    assert_eq!(
        sender.write_inline(
            &reference(),
            &basis(),
            &InlineVariables {
                path: "a.rs".to_string(),
                line: 1,
                side: DiffSide::New,
                body: "body".to_string(),
            },
        ),
        WriteRun::Sent(WriteOutcome::Permission(CredentialRefused::NotPermitted))
    );
    // The before-read and the one refused POST: nothing after, and no
    // second write.
    let sent = io.sents();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent.iter().filter(|one| one.body.is_some()).count(), 1);
}

/// A continuation from anywhere but the API is never requested: the
/// transport refuses the URL before a credential can ride it, and the
/// page answers nothing — a stale link stored from an older reply can
/// never carry the token elsewhere.
#[test]
fn a_continuation_off_the_api_is_never_requested() {
    let io = TestIo::new();
    let limits = BitbucketLimits::new();
    let authority = TestAuthority::grant();
    let enabled = || true;
    let sender = make_sender(&io, &limits, &authority, &enabled);

    for off in [
        "https://evil.example/2.0/repositories/acme/api/pullrequests/7/comments?page=2",
        "http://api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7/comments?page=2",
        "https://user@api.bitbucket.org/2.0/repositories/acme/api/pullrequests/7/comments",
        "https://api.bitbucket.org/1.0/comments",
    ] {
        assert_eq!(
            sender.comment_page(&reference(), Some(off)),
            CommentsRead::Unresolved,
            "{off} is no page this client asks"
        );
    }
    assert!(
        io.sents().is_empty(),
        "an off-API URL is refused before any send"
    );
}
