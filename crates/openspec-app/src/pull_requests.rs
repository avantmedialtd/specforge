//! The row types both pull-request panels render, and the one ordering rule
//! they share.
//!
//! `crate::bitbucket` and `crate::github` are structural twins — each owns its
//! recipe, its snapshot, its handle and its poll loop — but a row is a row: the
//! panel component, the review cell and the desktop's `open_pull_request` check
//! treat a BitBucket row and a GitHub row identically. Keeping the row here
//! means neither provider module owns the other's wire shape
//! (`github-pull-requests-panel` design D1, D5).
//!
//! Every type is `camelCase` on the wire and hand-mirrored in `src/types.ts`;
//! `tests/wire_shape.rs` pins the keys.

use serde::Serialize;

/// Status of a provider's latest refresh — drives whether (and how) its panel
/// renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestsStatus {
    /// The feature is disabled — the panel is not rendered at all.
    Disabled,
    /// Enabled, but no credential is configured or the provider rejected it.
    Unauthenticated,
    /// Enabled, but the list could not be obtained or parsed, and there are no
    /// previous rows to keep showing.
    Unavailable,
    /// A list is available (possibly empty, possibly stale).
    Ok,
}

/// A pull request's review state, pre-summarised so no frontend re-derives it
/// from a provider's shapes (`bitbucket-pull-requests`: *Review-State
/// Summary*; `github-pull-requests`: *GitHub Row Signals*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    /// Reviewers who approved, excluding the author.
    pub approvals: u32,
    /// Reviewers who requested changes, excluding the author.
    pub changes_requested: u32,
    /// Reviewers (or, on GitHub, requested teams) who have not responded.
    pub pending: u32,
}

/// The rollup of a pull request's latest-commit checks, reduced to the three
/// states the panel distinguishes. GitHub only; a BitBucket row carries none,
/// because its build statuses cost extra requests per pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChecksState {
    Passing,
    Failing,
    Pending,
}

/// One row of a pull-request panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSummary {
    /// The id (BitBucket) or number (GitHub), unique within its repository
    /// only.
    pub id: u64,
    pub title: String,
    /// The destination repository's `owner/name` (BitBucket: `workspace/repo`).
    pub repo_full_name: String,
    pub source_branch: String,
    pub destination_branch: String,
    /// The pull request's web page. Empty when the response carried no link on
    /// the provider's own site — such a row cannot be opened.
    pub url: String,
    pub draft: bool,
    /// The updated time as Unix epoch seconds, so each frontend renders a
    /// relative time without re-parsing a timestamp. `0` when unreadable.
    pub updated_at_unix: u64,
    /// `None` when the response did not carry the review inputs, so an unknown
    /// review state stays distinguishable from one with no activity.
    pub review: Option<ReviewSummary>,
    /// Open (unresolved) BitBucket tasks. Always `0` on a GitHub row: GitHub
    /// has no tasks, and its conversations are counted separately.
    pub open_tasks: u32,
    /// The author's login. Set on GitHub rows (the "To review" section shows
    /// it); `None` on BitBucket rows and for a deleted account.
    pub author: Option<String>,
    /// The latest commit's checks. `None` when no checks ran, and on every
    /// BitBucket row.
    pub checks: Option<ChecksState>,
    /// True only when the provider reports the pull request as conflicting. An
    /// uncomputed mergeability reads as `false`, never as unknown.
    pub conflicting: bool,
    /// Unresolved review conversations (GitHub). Always `0` on a BitBucket row.
    pub unresolved_threads: u32,
    /// The head (source) repository's `owner/name` — the repository the
    /// source branch lives in, which differs from `repo_full_name` for a
    /// pull request from a fork. Empty when the provider reports none (a
    /// deleted fork); such a row links to no worktree
    /// (`pull-request-worktree-links`: *Pull-Request Rows Carry Their Head
    /// Repository*).
    pub source_repo_full_name: String,
}

/// Several lists of rows as one, most recently updated first. The sort is
/// stable, so rows updated in the same second keep their input order: list by
/// list, then each list's own order.
pub(crate) fn merge_newest_first(lists: Vec<Vec<PullRequestSummary>>) -> Vec<PullRequestSummary> {
    let mut rows: Vec<PullRequestSummary> = lists.into_iter().flatten().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.updated_at_unix));
    rows
}

/// A count from a response, clamped rather than wrapped when it exceeds `u32`.
pub(crate) fn saturating_u32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u64, updated_at_unix: u64) -> PullRequestSummary {
        PullRequestSummary {
            id,
            title: format!("PR {id}"),
            repo_full_name: "ws/repo".to_string(),
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            url: format!("https://bitbucket.org/ws/repo/pull-requests/{id}"),
            draft: false,
            updated_at_unix,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: "ws/repo".to_string(),
        }
    }

    #[test]
    fn rows_merge_newest_first_across_lists() {
        let w1 = vec![row(1, 500), row(2, 100)];
        let w2 = vec![row(3, 900), row(4, 300)];
        let merged = merge_newest_first(vec![w1, w2]);
        assert_eq!(
            merged.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![3, 1, 4, 2]
        );
    }

    #[test]
    fn rows_updated_in_the_same_second_keep_list_order() {
        // An adversarial tie: the stable sort is what decides, so a reversed or
        // unstable ordering fails here even though every key is equal.
        let w1 = vec![row(1, 500), row(2, 500)];
        let w2 = vec![row(3, 500)];
        let merged = merge_newest_first(vec![w1, w2]);
        assert_eq!(
            merged.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn a_count_beyond_u32_saturates() {
        assert_eq!(saturating_u32(7), 7);
        assert_eq!(saturating_u32(u64::from(u32::MAX)), u32::MAX);
        assert_eq!(saturating_u32(u64::from(u32::MAX) + 1), u32::MAX);
    }

    #[test]
    fn checks_states_are_camel_case_on_the_wire() {
        let wire = |state| serde_json::to_value(state).unwrap();
        assert_eq!(wire(ChecksState::Passing), "passing");
        assert_eq!(wire(ChecksState::Failing), "failing");
        assert_eq!(wire(ChecksState::Pending), "pending");
    }
}
