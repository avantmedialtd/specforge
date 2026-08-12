//! The commit garden: a faithful, *today-scoped* commit graph per workspace,
//! shown stacked at the bottom of the Dashboard. Pure and Tauri-free so it is
//! unit-testable from `cargo test`.
//!
//! Each workspace's plot is a real DAG, not a stylized plant: the rail's lane
//! [`layout`] runs over the current local day's commits and produces the same
//! rows, lanes, and edges the commit-graph rail draws — only scoped to today and
//! with each node attributed to a **person**. Parents that predate today are
//! absent from the input, so a commit whose parent is from yesterday becomes a
//! lane root; the deciduous "only today" framing is just a filter on the input.
//!
//! Each node is coloured by the person who authored its commit, resolved exactly
//! as the leaderboard does: [`is_me`] first (you-precedence), then the
//! [`roster_index`] fold, else the raw author. Resolution is presentational and
//! query-time — it never touches stored events.

use crate::git::{AuthoredCommit, CommitRef, RawCommit};
use crate::graph::{layout, EdgeSegment};
use crate::identity::{is_me, normalized_key, roster_index, Author, IdentityConfig, Person};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One commit, laid out as a node in a workspace's today-graph and attributed
/// to a person. Mirrors the rail's laid-out commit (row/column/refs/subject)
/// plus the person fields that drive node colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GardenCommit {
    /// Commit sha — a stable key and the node's identity. Not shown as text.
    pub id: String,
    /// Row in display order (0 = newest), from the layout.
    pub row: usize,
    /// Lane (column) the node occupies, from the layout.
    pub column: usize,
    pub subject: String,
    /// Branch/tag/HEAD decorations on this commit.
    pub refs: Vec<CommitRef>,
    /// Author date, ISO-8601 (`%aI`); the frontend formats the local time on hover.
    pub date: String,
    /// Raw author display, surfaced on hover.
    pub author: String,
    /// Stable attribution key seeding the node's colour: the resolved person's
    /// primary key, the raw author key, or `"unknown"`.
    pub person_key: String,
    /// Display label for the committer — a custom person name, or the raw author.
    pub label: String,
    /// Whether this commit resolves to the canonical developer ("me"); the
    /// frontend tints such nodes with the application accent.
    pub is_me: bool,
}

/// One workspace's plot in the garden: a faithful today-scoped commit graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGarden {
    /// Display label for the entry; filled by the IPC layer from the presentation
    /// store (the pure derivation leaves it empty).
    pub label: String,
    /// True when there is nothing to draw today (no commits, or a non-git /
    /// git-unavailable entry) — the plot renders a dormant placeholder.
    pub dormant: bool,
    /// Today's commits, laid out newest-first into lanes.
    pub commits: Vec<GardenCommit>,
    /// Edge segments connecting commits to their parents within the day-graph.
    pub edges: Vec<EdgeSegment>,
    /// Number of lanes the renderer must size for.
    pub lane_count: usize,
}

/// The viewer's current local calendar day. Impure (reads the clock); kept thin
/// so [`compute_garden`] stays a pure function of an explicit `today`.
pub fn local_today() -> NaiveDate {
    Local::now().date_naive()
}

/// Parse a `%aI` author date into the viewer's local time zone.
fn parse_local(iso: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.with_timezone(&Local))
}

/// The viewer-local calendar day of an author date.
fn local_date(iso: &str) -> Option<NaiveDate> {
    parse_local(iso).map(|dt| dt.date_naive())
}

/// Resolve a commit author to `(colour key, label, is_me)` with you-precedence:
/// the canonical developer wins over any roster person.
fn resolve(
    author: &Author,
    config: &IdentityConfig,
    roster: &HashMap<String, (String, String)>,
) -> (String, String, bool) {
    if is_me(author, config) {
        let key = config
            .primary_key()
            .or_else(|| normalized_key(author))
            .unwrap_or_else(|| "me".to_string());
        return (key, config.label(), true);
    }
    match normalized_key(author) {
        Some(k) => match roster.get(&k) {
            Some((canonical, label)) => (canonical.clone(), label.clone(), false),
            None => (k, author.display(), false),
        },
        None => ("unknown".to_string(), "Unknown".to_string(), false),
    }
}

/// Build one workspace's today-graph from `commits` (newest first, as
/// [`commit_log_authored`] returns them) filtered to the viewer's local `today`.
/// An entry with no commits today returns a dormant plot. `label` is left empty
/// for the IPC layer to fill.
///
/// [`commit_log_authored`]: crate::git::commit_log_authored
pub fn compute_garden(
    commits: Vec<AuthoredCommit>,
    today: NaiveDate,
    config: &IdentityConfig,
    people: &[Person],
) -> WorkspaceGarden {
    let today_commits: Vec<AuthoredCommit> = commits
        .into_iter()
        .filter(|c| local_date(&c.date) == Some(today))
        .collect();

    if today_commits.is_empty() {
        return WorkspaceGarden {
            label: String::new(),
            dormant: true,
            commits: Vec::new(),
            edges: Vec::new(),
            lane_count: 0,
        };
    }

    // Attribute each commit to a person from its full identity (name + email)
    // *before* layout, since the laid-out commit keeps only the display name.
    let roster = roster_index(people);
    let person: HashMap<String, (String, String, bool)> = today_commits
        .iter()
        .map(|c| (c.id.clone(), resolve(&c.author, config, &roster)))
        .collect();

    // Reuse the rail's faithful lane layout over the day-subgraph: rows, lanes,
    // and edges are identical to what the rail would draw for these commits.
    let raw: Vec<RawCommit> = today_commits
        .iter()
        .map(|c| RawCommit {
            id: c.id.clone(),
            parents: c.parents.clone(),
            author: c.author.display(),
            date: c.date.clone(),
            subject: c.subject.clone(),
            refs: c.refs.clone(),
            trailers: Vec::new(),
        })
        .collect();
    let laid = layout(raw, false);

    let commits = laid
        .commits
        .into_iter()
        .map(|lc| {
            let (person_key, label, is_me_flag) = person
                .get(&lc.id)
                .cloned()
                .unwrap_or_else(|| ("unknown".to_string(), "Unknown".to_string(), false));
            GardenCommit {
                id: lc.id,
                row: lc.row,
                column: lc.column,
                subject: lc.subject,
                refs: lc.refs,
                date: lc.date,
                author: lc.author,
                person_key,
                label,
                is_me: is_me_flag,
            }
        })
        .collect();

    WorkspaceGarden {
        label: String::new(),
        dormant: false,
        commits,
        edges: laid.edges,
        lane_count: laid.lane_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A single instant defines "today" and all today-commits share it, so the
    // filter is robust regardless of the test machine's time zone; yesterday is
    // exactly 24h earlier (a non-DST June date), one local day back everywhere.
    const TODAY_ISO: &str = "2026-06-08T12:00:00+00:00";
    const YDAY_ISO: &str = "2026-06-07T12:00:00+00:00";
    const ME: &str = "me@example.com";

    fn today() -> NaiveDate {
        local_date(TODAY_ISO).unwrap()
    }

    fn author(name: Option<&str>, email: Option<&str>) -> Author {
        Author::new(name.map(str::to_string), email.map(str::to_string))
    }

    fn commit(id: &str, parents: &[&str], a: Author, iso: &str) -> AuthoredCommit {
        AuthoredCommit {
            id: id.to_string(),
            parents: parents.iter().map(|s| s.to_string()).collect(),
            author: a,
            date: iso.to_string(),
            subject: format!("subject {id}"),
            refs: Vec::new(),
        }
    }

    fn config() -> IdentityConfig {
        IdentityConfig {
            display_name: Some("Me".into()),
            aliases: vec![author(Some("Me"), Some(ME))],
        }
    }

    fn node<'a>(g: &'a WorkspaceGarden, id: &str) -> &'a GardenCommit {
        g.commits
            .iter()
            .find(|c| c.subject == format!("subject {id}"))
            .unwrap()
    }

    #[test]
    fn filters_to_local_today_and_drops_prior_days() {
        let commits = vec![
            commit("today1", &[], author(None, Some("a@x.io")), TODAY_ISO),
            commit("yday", &["old"], author(None, Some("a@x.io")), YDAY_ISO),
        ];
        let g = compute_garden(commits, today(), &config(), &[]);
        assert!(!g.dormant);
        assert_eq!(g.commits.len(), 1);
        assert_eq!(g.commits[0].subject, "subject today1");
    }

    #[test]
    fn no_commits_today_is_dormant() {
        let commits = vec![commit("yday", &[], author(None, Some("a@x.io")), YDAY_ISO)];
        let g = compute_garden(commits, today(), &config(), &[]);
        assert!(g.dormant);
        assert!(g.commits.is_empty());
        assert!(g.edges.is_empty());
        assert_eq!(g.lane_count, 0);
    }

    #[test]
    fn concurrent_branches_lay_out_as_a_faithful_dag() {
        // m merges main1 + feat1; their shared parent `base` predates today so it
        // is absent — both branch tips root in the day-graph.
        let commits = vec![
            commit("m", &["main1", "feat1"], author(None, Some(ME)), TODAY_ISO),
            commit("main1", &["base"], author(None, Some(ME)), TODAY_ISO),
            commit(
                "feat1",
                &["base"],
                author(None, Some("dev@x.io")),
                TODAY_ISO,
            ),
        ];
        let g = compute_garden(commits, today(), &config(), &[]);
        assert_eq!(g.commits.len(), 3);
        // Two concurrent lanes, and the merge fans into a second column.
        assert_eq!(g.lane_count, 2);
        assert_ne!(node(&g, "feat1").column, node(&g, "main1").column);
        // The merge produced edges connecting m to both parents.
        assert!(!g.edges.is_empty());
    }

    #[test]
    fn you_precedence_and_roster_folding() {
        let people = vec![Person {
            display_name: Some("Jane".into()),
            identities: vec![
                author(Some("Jane"), Some("jane@corp.com")),
                author(None, Some("jdoe@corp.com")),
            ],
        }];
        let commits = vec![
            commit("c1", &[], author(Some("Me"), Some(ME)), TODAY_ISO), // me
            commit("c2", &[], author(None, Some("jane@corp.com")), TODAY_ISO), // Jane
            commit("c3", &[], author(None, Some("jdoe@corp.com")), TODAY_ISO), // Jane (folded)
            commit(
                "c4",
                &[],
                author(Some("Rando"), Some("rando@x.io")),
                TODAY_ISO,
            ), // unrostered
        ];
        let g = compute_garden(commits, today(), &config(), &people);
        // "me" wins, labelled by the identity config.
        assert!(node(&g, "c1").is_me);
        assert_eq!(node(&g, "c1").label, "Me");
        // Jane's two identities fold to one colour key + label, and are not me.
        assert!(!node(&g, "c2").is_me);
        assert_eq!(node(&g, "c2").person_key, node(&g, "c3").person_key);
        assert_eq!(node(&g, "c2").label, "Jane");
        // An unrostered author keeps its raw key + label.
        assert_eq!(node(&g, "c4").person_key, "rando@x.io");
        assert_eq!(node(&g, "c4").label, "Rando");
    }

    #[test]
    fn authorless_commit_falls_back_to_unknown() {
        let commits = vec![commit("c", &[], author(None, None), TODAY_ISO)];
        let g = compute_garden(commits, today(), &config(), &[]);
        assert_eq!(g.commits.len(), 1);
        assert_eq!(g.commits[0].label, "Unknown");
        assert_eq!(g.commits[0].person_key, "unknown");
        assert!(!g.commits[0].is_me);
    }
}
