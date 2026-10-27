//! The operating-system confirmation that stands between a review composer
//! and the first provider write (`pull-request-actions`: *Native Confirmation
//! Freezes One Exact Draft Revision*; design D4).
//!
//! The service freezes one exact draft revision as a private [`ReviewFreeze`]
//! — a Rust-only value that never crosses IPC — and hands back the non-secret
//! [`NativeConfirmation`] summary. This module turns that summary into the
//! native dialog's text, shows it, and only on acceptance hands the frozen
//! value back to the service. Cancelling or dismissing the dialog drops the
//! frozen value: no operation journal is created and no provider request is
//! sent. Acceptance itself re-authorises current state, re-checks that the
//! frozen draft UUID is still current, and atomically persists the operation
//! before its first write; those rules live in `AppService` and are tested
//! there.
//!
//! The dialog is deliberately native rather than a React modal: a renderer
//! modal would be controlled by the same webview the command ACL constrains,
//! and confirming only the decision would let the body change between
//! confirmation and mutation.

use openspec_app::events::PullRequestProvider;
use openspec_app::review_actions::{
    DraftMutationRefusal, NativeConfirmation, ReviewDecision, ReviewFreeze,
};
use openspec_app::{AppService, PullRequestReference};
use serde::Serialize;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

/// What the native submission command answers: the reviewer cancelled (the
/// frozen value was dropped, nothing was persisted or sent) or accepted (the
/// operation was durably persisted and its run began; it may have submitted
/// or paused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewConfirmationOutcome {
    Cancelled,
    Accepted,
}

/// The service operations the confirmation sequence needs. [`AppService`] is
/// the only production implementation; the tests below substitute one that
/// models the durable facts the shell must respect without a live provider.
pub(crate) trait ReviewActions {
    /// The private frozen revision acceptance must hand back unchanged.
    type Frozen;

    /// Validate and freeze `expected_revision`, returning the non-secret
    /// summary and the private frozen value.
    fn freeze(
        &self,
        reference: &PullRequestReference,
        expected_revision: u64,
    ) -> Result<(NativeConfirmation, Self::Frozen), DraftMutationRefusal>;

    /// Accept the frozen value exactly once and start its operation.
    fn submit(&self, frozen: Self::Frozen) -> Result<(), DraftMutationRefusal>;
}

impl ReviewActions for AppService {
    type Frozen = ReviewFreeze;

    fn freeze(
        &self,
        reference: &PullRequestReference,
        expected_revision: u64,
    ) -> Result<(NativeConfirmation, ReviewFreeze), DraftMutationRefusal> {
        self.freeze_review_draft(reference, expected_revision)
    }

    fn submit(&self, frozen: ReviewFreeze) -> Result<(), DraftMutationRefusal> {
        self.submit_review_confirmation(frozen)
    }
}

/// Freeze one draft revision, ask the reviewer, and submit only on
/// acceptance. `confirm` receives the non-secret summary and returns whether
/// the reviewer accepted; the production command passes the native dialog,
/// and the tests pass a scripted answer.
///
/// A freeze refusal happens before `confirm`, so no dialog opens for a draft
/// that cannot be submitted. Cancelling returns without calling `submit`, so
/// the frozen value is dropped and nothing is written. Acceptance passes
/// exactly the value `freeze` returned — never a re-read or a newer revision.
pub(crate) fn confirm_and_submit<A: ReviewActions>(
    actions: &A,
    reference: &PullRequestReference,
    expected_revision: u64,
    confirm: impl FnOnce(&NativeConfirmation) -> bool,
) -> Result<ReviewConfirmationOutcome, DraftMutationRefusal> {
    let (confirmation, frozen) = actions.freeze(reference, expected_revision)?;
    if !confirm(&confirmation) {
        return Ok(ReviewConfirmationOutcome::Cancelled);
    }
    actions.submit(frozen)?;
    Ok(ReviewConfirmationOutcome::Accepted)
}

/// Show the native confirmation for `confirmation` and report whether the
/// reviewer accepted. Must run off the main thread: `blocking_show` waits for
/// the platform dialog while the main thread services it.
pub(crate) fn ask_natively(
    window: &tauri::WebviewWindow,
    confirmation: &NativeConfirmation,
) -> bool {
    window
        .dialog()
        .message(confirmation_text(confirmation))
        .title("Submit review")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Submit review".to_string(),
            "Cancel".to_string(),
        ))
        .parent(window)
        .blocking_show()
}

/// The dialog's text: the provider, repository and pull-request number, the
/// exact decision, whether a summary is present, the inline-comment and
/// reply counts, and the frozen revision. No draft prose, receipt body or
/// credential-derived value is ever named.
fn confirmation_text(confirmation: &NativeConfirmation) -> String {
    let provider = match confirmation.reference.provider {
        PullRequestProvider::Github => "GitHub",
        PullRequestProvider::Bitbucket => "BitBucket",
    };
    let decision = match confirmation.decision {
        ReviewDecision::Comment => "Comment",
        ReviewDecision::Approve => "Approve",
        ReviewDecision::RequestChanges => "Request changes",
    };
    let summary = if confirmation.summary_present {
        "included"
    } else {
        "none"
    };
    format!(
        "Submit this review to {provider}?\n\n\
         Repository: {owner}/{repo}\n\
         Pull request: #{number}\n\
         Decision: {decision}\n\
         Summary: {summary}\n\
         Inline comments: {inline}\n\
         Replies: {replies}\n\
         Frozen draft revision: {revision}\n\n\
         SpecForge will post this review to {provider} now. It cannot undo it.",
        owner = confirmation.reference.owner,
        repo = confirmation.reference.repo,
        number = confirmation.reference.number,
        inline = confirmation.inline_comments,
        replies = confirmation.replies,
        revision = confirmation.frozen_revision,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use openspec_app::review_actions::{CommitBasis, DraftItem, ReviewDecision, ReviewDraft};
    use std::cell::RefCell;

    fn reference() -> PullRequestReference {
        PullRequestReference {
            provider: PullRequestProvider::Github,
            owner: "acme".to_string(),
            repo: "api".to_string(),
            number: 7,
        }
    }

    /// The confirmation summary a frozen revision 12 would carry.
    fn confirmation(revision: u64) -> NativeConfirmation {
        NativeConfirmation {
            reference: reference(),
            decision: ReviewDecision::RequestChanges,
            summary_present: true,
            inline_comments: 2,
            replies: 1,
            frozen_revision: revision,
        }
    }

    /// A current draft as a refusal's `staleRevision` carries it.
    fn draft(id: &str, revision: u64) -> ReviewDraft {
        ReviewDraft {
            id: id.to_string(),
            reference: reference(),
            revision,
            basis: CommitBasis {
                head: "head".to_string(),
                base: "base".to_string(),
            },
            decision: ReviewDecision::Comment,
            summary: String::new(),
            items: Vec::<DraftItem>::new(),
        }
    }

    /// The durable facts the shell sequence must respect, modelled without a
    /// live service: the current draft identity and revision, the operation
    /// journal (at most one operation per frozen revision) and the provider
    /// request count.
    #[derive(Default)]
    struct FakeActions {
        state: RefCell<FakeState>,
    }

    #[derive(Default)]
    struct FakeState {
        draft_id: String,
        revision: u64,
        /// One entry per accepted operation, naming its frozen revision.
        journal: Vec<u64>,
        /// One entry per provider run a persisted operation would start.
        requests: usize,
    }

    struct FakeFreeze {
        draft_id: String,
        revision: u64,
    }

    impl FakeActions {
        fn with_draft(revision: u64) -> Self {
            Self {
                state: RefCell::new(FakeState {
                    draft_id: "draft-1".to_string(),
                    revision,
                    ..FakeState::default()
                }),
            }
        }

        fn journal(&self) -> Vec<u64> {
            self.state.borrow().journal.clone()
        }

        fn requests(&self) -> usize {
            self.state.borrow().requests
        }
    }

    impl ReviewActions for FakeActions {
        type Frozen = FakeFreeze;

        fn freeze(
            &self,
            _reference: &PullRequestReference,
            expected_revision: u64,
        ) -> Result<(NativeConfirmation, FakeFreeze), DraftMutationRefusal> {
            let state = self.state.borrow();
            if state.revision != expected_revision {
                return Err(DraftMutationRefusal::StaleRevision {
                    current: Box::new(draft(&state.draft_id, state.revision)),
                });
            }
            Ok((
                confirmation(expected_revision),
                FakeFreeze {
                    draft_id: state.draft_id.clone(),
                    revision: expected_revision,
                },
            ))
        }

        fn submit(&self, frozen: FakeFreeze) -> Result<(), DraftMutationRefusal> {
            let mut state = self.state.borrow_mut();
            // Start over replaced the draft while the dialog was open.
            if state.draft_id != frozen.draft_id {
                return Err(DraftMutationRefusal::StaleRevision {
                    current: Box::new(draft(&state.draft_id, state.revision)),
                });
            }
            // One revision confirms into at most one operation.
            if state.journal.contains(&frozen.revision) {
                return Err(DraftMutationRefusal::OperationFinished);
            }
            state.journal.push(frozen.revision);
            state.requests += 1;
            Ok(())
        }
    }

    #[test]
    fn cancelling_writes_nothing() {
        let actions = FakeActions::with_draft(12);
        let mut shown = None;
        let outcome = confirm_and_submit(&actions, &reference(), 12, |confirmation| {
            shown = Some(confirmation.frozen_revision);
            false
        })
        .unwrap();
        assert_eq!(outcome, ReviewConfirmationOutcome::Cancelled);
        assert_eq!(shown, Some(12), "the dialog named the frozen revision");
        assert!(actions.journal().is_empty(), "no operation journal");
        assert_eq!(actions.requests(), 0, "no provider request");
    }

    #[test]
    fn accepting_starts_exactly_the_frozen_revision() {
        let actions = FakeActions::with_draft(12);
        let mut seen = None;
        let outcome = confirm_and_submit(&actions, &reference(), 12, |confirmation| {
            seen = Some(confirmation.clone());
            true
        })
        .unwrap();
        assert_eq!(outcome, ReviewConfirmationOutcome::Accepted);
        assert_eq!(seen, Some(confirmation(12)), "the dialog named revision 12");
        assert_eq!(
            actions.journal(),
            [12],
            "the operation persists the revision the dialog froze"
        );
        assert_eq!(actions.requests(), 1);
    }

    #[test]
    fn start_over_invalidates_an_older_open_confirmation() {
        let actions = FakeActions::with_draft(12);
        let outcome = confirm_and_submit(&actions, &reference(), 12, |_| {
            // Another window replaces the draft while the dialog is open.
            let mut state = actions.state.borrow_mut();
            state.draft_id = "draft-2".to_string();
            state.revision = 1;
            true
        });
        assert_eq!(
            outcome,
            Err(DraftMutationRefusal::StaleRevision {
                current: Box::new(draft("draft-2", 1))
            })
        );
        assert!(actions.journal().is_empty(), "no operation was persisted");
        assert_eq!(actions.requests(), 0, "nothing was sent");
    }

    #[test]
    fn two_accepted_dialogs_cannot_create_duplicate_operations() {
        let actions = FakeActions::with_draft(12);
        let first = confirm_and_submit(&actions, &reference(), 12, |_| true);
        let second = confirm_and_submit(&actions, &reference(), 12, |_| true);
        assert_eq!(first, Ok(ReviewConfirmationOutcome::Accepted));
        assert_eq!(second, Err(DraftMutationRefusal::OperationFinished));
        assert_eq!(actions.journal(), [12], "exactly one operation");
        assert_eq!(actions.requests(), 1, "exactly one provider run");
    }

    #[test]
    fn a_refused_freeze_opens_no_dialog() {
        let actions = FakeActions::with_draft(12);
        let mut opened = false;
        let outcome = confirm_and_submit(&actions, &reference(), 11, |_| {
            opened = true;
            true
        });
        assert!(matches!(
            outcome,
            Err(DraftMutationRefusal::StaleRevision { .. })
        ));
        assert!(!opened, "nothing was confirmed");
        assert!(actions.journal().is_empty());
        assert_eq!(actions.requests(), 0);
    }

    /// The real service refuses a freeze with no current draft before the
    /// shell ever reaches a dialog: a refused boundary cannot be confirmed.
    #[test]
    fn the_real_service_refuses_a_missing_draft_before_any_dialog() {
        let dir = tempfile::tempdir().unwrap();
        let svc = AppService::bootstrap(dir.path().to_path_buf());
        let mut opened = false;
        let outcome = confirm_and_submit(&svc, &reference(), 1, |_| {
            opened = true;
            true
        });
        assert_eq!(outcome, Err(DraftMutationRefusal::NoDraft));
        assert!(!opened);
    }

    /// The command's only new wire value stays paired with the frontend
    /// contract: the camel-case spelling in `src/types.ts` is what
    /// `serde(rename_all = "camelCase")` produces.
    #[test]
    fn the_confirmation_outcome_spelling_matches_typescript() {
        assert_eq!(
            serde_json::to_string(&ReviewConfirmationOutcome::Cancelled).unwrap(),
            "\"cancelled\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewConfirmationOutcome::Accepted).unwrap(),
            "\"accepted\""
        );
        let types = include_str!("../../../src/types.ts");
        assert!(
            types.contains("export type ReviewConfirmationOutcome = \"cancelled\" | \"accepted\""),
            "src/types.ts no longer pins the confirmation outcome"
        );
    }

    #[test]
    fn the_confirmation_names_every_required_fact_and_no_prose() {
        let text = confirmation_text(&confirmation(12));
        assert!(text.contains("GitHub"), "{text}");
        assert!(text.contains("acme/api"), "{text}");
        assert!(text.contains("#7"), "{text}");
        assert!(text.contains("Decision: Request changes"), "{text}");
        assert!(text.contains("Summary: included"), "{text}");
        assert!(text.contains("Inline comments: 2"), "{text}");
        assert!(text.contains("Replies: 1"), "{text}");
        assert!(text.contains("Frozen draft revision: 12"), "{text}");

        let without_summary = NativeConfirmation {
            summary_present: false,
            ..confirmation(3)
        };
        assert!(confirmation_text(&without_summary).contains("Summary: none"));

        let bitbucket = NativeConfirmation {
            reference: PullRequestReference {
                provider: PullRequestProvider::Bitbucket,
                ..reference()
            },
            decision: ReviewDecision::Comment,
            ..confirmation(1)
        };
        let text = confirmation_text(&bitbucket);
        assert!(text.contains("BitBucket"), "{text}");
        assert!(text.contains("Decision: Comment"), "{text}");
    }
}
