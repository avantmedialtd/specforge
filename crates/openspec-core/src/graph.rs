//! Pure commit-graph layout: assign each commit a lane (column) and emit the
//! edge segments connecting commits to their parents.
//!
//! Input is a list of [`RawCommit`]s in display order (newest first, as
//! `git log --date-order` produces them). Output is a [`CommitGraph`] the
//! frontend renders directly — it never re-derives topology.
//!
//! The algorithm sweeps newest → oldest maintaining a set of *lanes*, each a
//! slot waiting for the next commit expected in that lane:
//!
//! 1. A commit's column is the first lane already waiting for it (a child
//!    reserved it); else a freshly allocated lane (a branch tip).
//! 2. The commit's first parent continues straight down the same lane.
//! 3. Each additional parent (a merge) reserves another lane.
//! 4. Lanes are reclaimed the instant their commit is consumed, so the
//!    visible lane count reflects only branches concurrently alive — this is
//!    the compaction that keeps `--all` readable in a narrow rail.
//!
//! Edges are routed in the *parent's* lane: from a child they bend into the
//! parent's column in the band directly below the child, then run straight
//! down that column until the parent's row. Convergence (multiple children of
//! one parent) and pass-through verticals fall out of this rule for free.

use crate::git::{CommitRef, RawCommit, Trailer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A commit placed at a row/column with its display metadata. `row` is the
/// commit's index in the input order (0 = newest).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaidOutCommit {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub refs: Vec<CommitRef>,
    pub trailers: Vec<Trailer>,
    pub row: usize,
    pub column: usize,
}

/// A single line segment in the band between two adjacent rows. `band` is the
/// gap index (equal to the top row's index); the segment runs from
/// `from_column` at the top to `to_column` at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeSegment {
    pub band: usize,
    pub from_column: usize,
    pub to_column: usize,
}

/// The laid-out graph the frontend renders. `lane_count` is the number of
/// columns the renderer must size for; `truncated` is true when the input
/// was capped by the caller's window limit (there is older history below).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitGraph {
    pub commits: Vec<LaidOutCommit>,
    pub edges: Vec<EdgeSegment>,
    pub lane_count: usize,
    pub truncated: bool,
}

/// Index of the first free (`None`) lane, allocating a new one if every lane
/// is occupied.
fn first_free(lanes: &mut Vec<Option<String>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(idx) => idx,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// Reserve a lane for `parent`. If a lane already waits for it (two branches
/// sharing a parent), that lane is reused so the parent occupies a single
/// column. Otherwise `prefer` is used when it is free, else the first free
/// lane. Keeping the first parent in the commit's own column keeps the
/// mainline running straight rather than jogging into a reclaimed hole.
fn place_parent(lanes: &mut Vec<Option<String>>, parent: &str, prefer: Option<usize>) {
    if lanes.iter().any(|l| l.as_deref() == Some(parent)) {
        return;
    }
    let idx = match prefer {
        Some(p) if lanes.get(p).is_some_and(Option::is_none) => p,
        _ => first_free(lanes),
    };
    lanes[idx] = Some(parent.to_string());
}

/// Lay out `commits` (newest first) into a renderable graph. `truncated`
/// records whether the caller capped the input — surfaced so the UI can show
/// a "load more" affordance rather than implying the history is complete.
pub fn layout(commits: Vec<RawCommit>, truncated: bool) -> CommitGraph {
    // Pass 1: assign every commit a column and keep lanes reserved for
    // not-yet-seen parents.
    let mut lanes: Vec<Option<String>> = Vec::new();
    let mut columns: Vec<usize> = Vec::with_capacity(commits.len());
    let mut lane_count = 0;

    for commit in &commits {
        let cid = commit.id.as_str();
        // The commit sits in the first lane waiting for it, or a new lane if
        // nothing points here (a branch tip / first sighting).
        let col = match lanes.iter().position(|l| l.as_deref() == Some(cid)) {
            Some(idx) => idx,
            None => first_free(&mut lanes),
        };
        columns.push(col);

        // Consume every lane waiting for this commit (collapsing forks).
        for slot in lanes.iter_mut() {
            if slot.as_deref() == Some(cid) {
                *slot = None;
            }
        }

        // First parent continues in this column; further parents fan out.
        if let Some(first) = commit.parents.first() {
            place_parent(&mut lanes, first, Some(col));
        }
        for extra in commit.parents.iter().skip(1) {
            place_parent(&mut lanes, extra, None);
        }

        lane_count = lane_count.max(col + 1);
    }

    // Pass 2: route edges in each parent's lane.
    let pos: HashMap<&str, (usize, usize)> = commits
        .iter()
        .enumerate()
        .map(|(row, c)| (c.id.as_str(), (row, columns[row])))
        .collect();

    let mut edges: HashSet<EdgeSegment> = HashSet::new();
    for (row, commit) in commits.iter().enumerate() {
        let child_col = columns[row];
        for parent in &commit.parents {
            // Parents outside the loaded window have no node; their edge is
            // omitted — truncation is signalled by `truncated` instead.
            let Some(&(parent_row, parent_col)) = pos.get(parent.as_str()) else {
                continue;
            };
            // Bend from the child's column into the parent's lane, directly
            // below the child.
            edges.insert(EdgeSegment {
                band: row,
                from_column: child_col,
                to_column: parent_col,
            });
            // Run straight down the parent's lane until the parent's row.
            for band in (row + 1)..parent_row {
                edges.insert(EdgeSegment {
                    band,
                    from_column: parent_col,
                    to_column: parent_col,
                });
            }
        }
    }

    let mut edges: Vec<EdgeSegment> = edges.into_iter().collect();
    edges.sort();

    let commits = commits
        .into_iter()
        .enumerate()
        .map(|(row, c)| LaidOutCommit {
            id: c.id,
            parents: c.parents,
            author: c.author,
            date: c.date,
            subject: c.subject,
            refs: c.refs,
            trailers: c.trailers,
            row,
            column: columns[row],
        })
        .collect();

    CommitGraph {
        commits,
        edges,
        lane_count,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `RawCommit` with just an id and parent ids — layout ignores
    /// the display metadata.
    fn c(id: &str, parents: &[&str]) -> RawCommit {
        RawCommit {
            id: id.to_string(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            refs: Vec::new(),
            trailers: Vec::new(),
        }
    }

    /// Column assigned to commit `id`.
    fn col_of(g: &CommitGraph, id: &str) -> usize {
        g.commits.iter().find(|c| c.id == id).unwrap().column
    }

    fn has_edge(g: &CommitGraph, band: usize, from: usize, to: usize) -> bool {
        g.edges.contains(&EdgeSegment {
            band,
            from_column: from,
            to_column: to,
        })
    }

    #[test]
    fn linear_history_is_one_lane() {
        let g = layout(vec![c("c", &["b"]), c("b", &["a"]), c("a", &[])], false);
        assert_eq!(g.lane_count, 1);
        assert_eq!(col_of(&g, "a"), 0);
        assert_eq!(col_of(&g, "b"), 0);
        assert_eq!(col_of(&g, "c"), 0);
        // Straight verticals all the way down column 0.
        assert!(has_edge(&g, 0, 0, 0));
        assert!(has_edge(&g, 1, 0, 0));
    }

    #[test]
    fn fork_and_merge_uses_two_lanes() {
        // m merges main1 + feat1; both descend from base.
        let g = layout(
            vec![
                c("m", &["main1", "feat1"]),
                c("main1", &["base"]),
                c("feat1", &["base"]),
                c("base", &[]),
            ],
            false,
        );
        assert_eq!(g.lane_count, 2);
        assert_eq!(col_of(&g, "m"), 0);
        assert_eq!(col_of(&g, "main1"), 0, "first parent keeps the lane");
        assert_eq!(col_of(&g, "feat1"), 1, "second parent fans to a new lane");
        assert_eq!(col_of(&g, "base"), 0, "shared parent reuses one lane");
        // The merge fans right from m's lane into feat1's lane.
        assert!(has_edge(&g, 0, 0, 1), "merge fork edge");
        // feat1 converges back into base's lane.
        assert!(has_edge(&g, 2, 1, 0), "feat converges into base");
    }

    #[test]
    fn octopus_merge_allocates_a_lane_per_parent() {
        let g = layout(
            vec![
                c("m", &["p1", "p2", "p3"]),
                c("p1", &["base"]),
                c("p2", &["base"]),
                c("p3", &["base"]),
                c("base", &[]),
            ],
            false,
        );
        assert_eq!(g.lane_count, 3, "three parents → three lanes");
        assert_eq!(col_of(&g, "p1"), 0);
        assert_eq!(col_of(&g, "p2"), 1);
        assert_eq!(col_of(&g, "p3"), 2);
        assert_eq!(col_of(&g, "base"), 0, "all merge back into one lane");
    }

    #[test]
    fn two_tips_sharing_a_parent_converge() {
        // a and b are independent tips that share parent base.
        let g = layout(
            vec![c("a", &["base"]), c("b", &["base"]), c("base", &[])],
            false,
        );
        assert_eq!(g.lane_count, 2);
        assert_eq!(col_of(&g, "a"), 0);
        assert_eq!(col_of(&g, "b"), 1, "second tip takes its own lane");
        assert_eq!(col_of(&g, "base"), 0);
        assert!(has_edge(&g, 1, 1, 0), "b converges into base's lane");
    }

    #[test]
    fn dormant_lane_runs_straight_through_intervening_commits() {
        // feat's parent (base) sits at the bottom; the mainline m1..m3 runs
        // between them, so feat's lane stays reserved across those rows.
        let g = layout(
            vec![
                c("feat", &["base"]),
                c("m1", &["m2"]),
                c("m2", &["m3"]),
                c("m3", &["base"]),
                c("base", &[]),
            ],
            false,
        );
        assert_eq!(g.lane_count, 2);
        assert_eq!(col_of(&g, "feat"), 0);
        assert_eq!(col_of(&g, "m1"), 1);
        assert_eq!(col_of(&g, "base"), 0);
        // feat's lane (column 0) passes straight through the middle band where
        // m2 sits in column 1.
        assert!(has_edge(&g, 2, 0, 0), "dormant lane passes through");
    }

    #[test]
    fn truncated_flag_is_carried_through() {
        let g = layout(vec![c("a", &["b"])], true);
        assert!(g.truncated);
        // b is outside the window: no edge is emitted for it.
        assert!(g.edges.is_empty());
    }
}
