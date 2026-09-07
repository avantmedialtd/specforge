//! The commit garden: a faithful, *today-scoped* commit graph per workspace,
//! shown stacked at the bottom of the Dashboard. Pure and Tauri-free so it is
//! unit-testable from `cargo test`.
//!
//! Each workspace's plot is a real DAG, not a stylized plant: the rail's lane
//! [`layout`] runs over the current local day's commits and produces the same
//! rows, lanes, and edges the commit-graph rail draws — only scoped to today and
//! with each node attributed to an **author**. Parents that predate today are
//! absent from the input, so a commit whose parent is from yesterday becomes a
//! lane root; the deciduous "only today" framing is just a filter on the input.
//!
//! Each node is coloured by the author of its commit, resolved with
//! you-precedence: [`is_me`] first, else the author's own normalised key. There
//! is no roster fold, so two git identities of one teammate draw in two colours,
//! exactly as two unrelated authors would; only the developer's own identities
//! collapse, and they collapse through the alias list. Resolution is
//! presentational and query-time — it never touches stored events.

use crate::git::{AuthoredCommit, CommitRef, RawCommit};
use crate::graph::{layout, EdgeSegment};
use crate::identity::{is_me, normalized_key, Author, IdentityConfig};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

/// One commit, laid out as a node in a workspace's today-graph and attributed
/// to an author. Mirrors the rail's laid-out commit (row/column/refs/subject)
/// plus the attribution fields that drive node colour.
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
    /// Stable attribution key seeding the node's colour: the developer's primary
    /// key, the raw author key, or `"unknown"`.
    pub author_key: String,
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
    /// Stable identity for the entry — a repository id, or a flat workspace's
    /// URI. Filled by the IPC layer alongside `label` (the pure derivation
    /// leaves it empty).
    ///
    /// Labels are display names and are **not** unique: two worktrees of
    /// unrelated repositories can both be called `specforge`, and two entries
    /// can carry the same display-name override. This key is what makes
    /// [`plot_order`] total and gives the frontend a list key that survives a
    /// reorder (`commit-garden`: *Deterministic Plot Order*).
    pub entry_key: String,
    /// The entry's **registry-wide** count of active (non-archived) changes,
    /// annotating the plot's caption. Like `label` it is a property of the
    /// entry rather than of today's commits, so the pure derivation leaves it
    /// at zero and the IPC layer fills it from the `WorkspaceView` it holds.
    /// Not comparable with the hero's in-flight tile, which is scoped to the
    /// canonical developer (`commit-garden`: *Plot Caption*).
    pub active_count: usize,
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

/// Order two plots for the garden section: today's commit count descending,
/// then display label ascending, then [`entry_key`] ascending.
///
/// All three keys are required. The commit count leads with the entry that
/// moved most today. The label is what stops two equally busy entries trading
/// places between refreshes. The entry key is what makes the order **total**:
/// labels are display names with no uniqueness guarantee, so without it two
/// entries sharing a label fall back to whichever order the registry happened
/// to emit — which the *Deterministic Plot Order* requirement forbids by name.
///
/// The active-change count deliberately does **not** participate: it rides
/// along in the caption as an annotation, and it is a live state count on a
/// section that is otherwise entirely about today.
///
/// This lives here, as a named function on pure data, rather than inline in the
/// service's `sort_by`: `cargo mutants` replaces whole function bodies, so a
/// comparator written as a closure inside an `async fn` produces no mutants of
/// its own and the gate is structurally blind to it.
///
/// [`entry_key`]: WorkspaceGarden::entry_key
pub fn plot_order(a: &WorkspaceGarden, b: &WorkspaceGarden) -> Ordering {
    b.commits
        .len()
        .cmp(&a.commits.len())
        .then_with(|| a.label.cmp(&b.label))
        .then_with(|| a.entry_key.cmp(&b.entry_key))
}

/// Sort a garden's plots into presentation order, per [`plot_order`].
pub fn sort_plots(plots: &mut [WorkspaceGarden]) {
    plots.sort_by(plot_order);
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

/// Resolve a commit author to `(colour key, is_me)` with you-precedence: every
/// identity that resolves as the canonical developer collapses onto one key,
/// and every other author is keyed on their own normalised git key. An author
/// with no usable key falls back to `"unknown"`.
fn resolve(author: &Author, config: &IdentityConfig) -> (String, bool) {
    if is_me(author, config) {
        let key = config
            .primary_key()
            .or_else(|| normalized_key(author))
            .unwrap_or_else(|| "me".to_string());
        return (key, true);
    }
    match normalized_key(author) {
        Some(k) => (k, false),
        None => ("unknown".to_string(), false),
    }
}

/// Build one workspace's today-graph from `commits` (newest first, as
/// [`commit_log_authored`] returns them) filtered to the viewer's local `today`.
/// An entry with no commits today returns a dormant plot. `label`,
/// `entry_key` and `active_count` are the entry's own properties rather than
/// today's, so all three are left at their empty/zero default for the IPC
/// layer to fill.
///
/// [`commit_log_authored`]: crate::git::commit_log_authored
pub fn compute_garden(
    commits: Vec<AuthoredCommit>,
    today: NaiveDate,
    config: &IdentityConfig,
) -> WorkspaceGarden {
    let today_commits: Vec<AuthoredCommit> = commits
        .into_iter()
        .filter(|c| local_date(&c.date) == Some(today))
        .collect();

    if today_commits.is_empty() {
        return WorkspaceGarden {
            label: String::new(),
            entry_key: String::new(),
            active_count: 0,
            dormant: true,
            commits: Vec::new(),
            edges: Vec::new(),
            lane_count: 0,
        };
    }

    // Attribute each commit from its full identity (name + email) *before*
    // layout, since the laid-out commit keeps only the display name.
    let attribution: HashMap<String, (String, bool)> = today_commits
        .iter()
        .map(|c| (c.id.clone(), resolve(&c.author, config)))
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
            let (author_key, is_me_flag) = attribution
                .get(&lc.id)
                .cloned()
                .unwrap_or_else(|| ("unknown".to_string(), false));
            GardenCommit {
                id: lc.id,
                row: lc.row,
                column: lc.column,
                subject: lc.subject,
                refs: lc.refs,
                date: lc.date,
                author: lc.author,
                author_key,
                is_me: is_me_flag,
            }
        })
        .collect();

    WorkspaceGarden {
        label: String::new(),
        entry_key: String::new(),
        active_count: 0,
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
        let g = compute_garden(commits, today(), &config());
        assert!(!g.dormant);
        assert_eq!(g.commits.len(), 1);
        assert_eq!(g.commits[0].subject, "subject today1");
    }

    #[test]
    fn no_commits_today_is_dormant() {
        let commits = vec![commit("yday", &[], author(None, Some("a@x.io")), YDAY_ISO)];
        let g = compute_garden(commits, today(), &config());
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
        let g = compute_garden(commits, today(), &config());
        assert_eq!(g.commits.len(), 3);
        // Two concurrent lanes, and the merge fans into a second column.
        assert_eq!(g.lane_count, 2);
        assert_ne!(node(&g, "feat1").column, node(&g, "main1").column);
        // The merge produced edges connecting m to both parents.
        assert!(!g.edges.is_empty());
    }

    /// You-precedence: every identity on the developer's alias list collapses
    /// onto one accented key, whichever alias the commit was authored with.
    #[test]
    fn every_developer_alias_shares_one_accented_key() {
        let config = IdentityConfig {
            display_name: Some("Me".into()),
            aliases: vec![
                author(Some("Me"), Some(ME)),
                author(Some("Me"), Some("me@home.dev")),
            ],
        };
        let commits = vec![
            commit("c1", &[], author(Some("Me"), Some(ME)), TODAY_ISO),
            commit("c2", &[], author(None, Some("me@home.dev")), TODAY_ISO),
        ];
        let g = compute_garden(commits, today(), &config);
        assert!(node(&g, "c1").is_me);
        assert!(node(&g, "c2").is_me);
        assert_eq!(node(&g, "c1").author_key, ME);
        assert_eq!(node(&g, "c2").author_key, ME);
    }

    /// Without a roster, every non-developer author keys on their own raw git
    /// identity — so one teammate committing under two identities draws in two
    /// colours, exactly as two unrelated authors would. That is the accepted
    /// consequence of removing the named-people roster, pinned here so it reads
    /// as a decision rather than a regression.
    #[test]
    fn other_authors_key_on_their_raw_identity() {
        let commits = vec![
            commit(
                "c1",
                &[],
                author(Some("Rando"), Some("rando@x.io")),
                TODAY_ISO,
            ),
            commit(
                "c2",
                &[],
                author(Some("Jane"), Some("jane@corp.com")),
                TODAY_ISO,
            ),
            commit(
                "c3",
                &[],
                author(Some("Jane"), Some("jdoe@corp.com")),
                TODAY_ISO,
            ),
        ];
        let g = compute_garden(commits, today(), &config());
        assert!(!node(&g, "c1").is_me);
        assert_eq!(node(&g, "c1").author_key, "rando@x.io");
        // `author` is the only human-readable name the garden carries now that
        // the resolved label is gone. Only the desktop renders it (the hover
        // title); the terminal draws colour and subject and shows no author at
        // all — so this assertion is the payload's sole guard.
        assert_eq!(node(&g, "c1").author, "Rando");
        // Jane's two identities do NOT fold: two keys, hence two colours.
        assert_ne!(node(&g, "c2").author_key, node(&g, "c3").author_key);
        assert_eq!(node(&g, "c2").author_key, "jane@corp.com");
        assert_eq!(node(&g, "c3").author_key, "jdoe@corp.com");
    }

    #[test]
    fn authorless_commit_falls_back_to_unknown() {
        let commits = vec![commit("c", &[], author(None, None), TODAY_ISO)];
        let g = compute_garden(commits, today(), &config());
        assert_eq!(g.commits.len(), 1);
        assert_eq!(g.commits[0].author_key, "unknown");
        // The capital-U display the spec scenario names, from `Author::display`.
        // Distinct from the lowercase colour key above, and asserted nowhere
        // else in the workspace.
        assert_eq!(g.commits[0].author, "Unknown");
        assert!(!g.commits[0].is_me);
    }

    /// A plot carrying `commits` commits today, for ordering tests. Only the
    /// fields [`plot_order`] reads are meaningful; the graph itself is
    /// irrelevant to the comparator, which is exactly why these tests need no
    /// repository on disk and no clock.
    fn plot(label: &str, entry_key: &str, commits: usize, active: usize) -> WorkspaceGarden {
        WorkspaceGarden {
            label: label.to_string(),
            entry_key: entry_key.to_string(),
            active_count: active,
            dormant: commits == 0,
            commits: (0..commits)
                .map(|i| GardenCommit {
                    id: format!("{entry_key}-{i}"),
                    row: i,
                    column: 0,
                    subject: String::new(),
                    refs: Vec::new(),
                    date: String::new(),
                    author: String::new(),
                    author_key: String::new(),
                    is_me: false,
                })
                .collect(),
            edges: Vec::new(),
            lane_count: 1,
        }
    }

    fn labels(plots: &[WorkspaceGarden]) -> Vec<&str> {
        plots.iter().map(|p| p.label.as_str()).collect()
    }

    fn keys(plots: &[WorkspaceGarden]) -> Vec<&str> {
        plots.iter().map(|p| p.entry_key.as_str()).collect()
    }

    #[test]
    fn plots_lead_with_todays_busiest_entry() {
        let mut plots = vec![
            plot("alpha", "a", 1, 0),
            plot("zulu", "z", 4, 0),
            plot("mike", "m", 2, 0),
        ];
        sort_plots(&mut plots);
        assert_eq!(labels(&plots), ["zulu", "mike", "alpha"]);
        // The commit counts, not only the labels: a sort that dropped the
        // leading key entirely would still produce a plausible-looking list.
        assert_eq!(
            plots.iter().map(|p| p.commits.len()).collect::<Vec<_>>(),
            [4, 2, 1]
        );
    }

    #[test]
    fn equal_commit_counts_are_broken_by_ascending_label() {
        let mut plots = vec![
            plot("zulu", "z", 2, 0),
            plot("alpha", "a", 2, 0),
            plot("mike", "m", 2, 0),
        ];
        sort_plots(&mut plots);
        assert_eq!(labels(&plots), ["alpha", "mike", "zulu"]);

        // The same entries, permuted: the order is a function of the entries,
        // not of the order they arrived in.
        let mut permuted = vec![
            plot("mike", "m", 2, 0),
            plot("alpha", "a", 2, 0),
            plot("zulu", "z", 2, 0),
        ];
        sort_plots(&mut permuted);
        assert_eq!(labels(&permuted), ["alpha", "mike", "zulu"]);
    }

    #[test]
    fn active_count_never_reaches_the_comparator() {
        // The active counts are deliberately ANTI-correlated with the labels:
        // ascending by label yields 3, 1, 2, and ascending by active count
        // would yield mike, zulu, alpha - so neither an ascending nor a
        // descending active-count key passes this by coincidence. A fixture
        // whose active counts happen to ascend with its labels proves nothing.
        let mut plots = vec![
            plot("zulu", "z", 2, 2),
            plot("alpha", "a", 2, 3),
            plot("mike", "m", 2, 1),
        ];
        sort_plots(&mut plots);
        assert_eq!(
            plots
                .iter()
                .map(|p| (p.label.as_str(), p.active_count))
                .collect::<Vec<_>>(),
            [("alpha", 3), ("mike", 1), ("zulu", 2)]
        );
    }

    #[test]
    fn duplicate_labels_are_broken_by_entry_key() {
        // Two worktrees of unrelated projects can carry the same display label.
        // Without the third key their order would be whatever the registry
        // emitted, which *Deterministic Plot Order* forbids by name.
        let mut plots = vec![
            plot("specforge", "key-z", 3, 0),
            plot("specforge", "key-a", 3, 0),
        ];
        sort_plots(&mut plots);
        assert_eq!(keys(&plots), ["key-a", "key-z"]);

        let mut permuted = vec![
            plot("specforge", "key-a", 3, 0),
            plot("specforge", "key-z", 3, 0),
        ];
        sort_plots(&mut permuted);
        assert_eq!(keys(&permuted), ["key-a", "key-z"]);
    }

    #[test]
    fn plot_order_is_a_total_order_on_distinct_entries() {
        // Antisymmetry: no two distinct entries compare Equal, so the rendered
        // order never depends on the sort's stability.
        let entries = [
            plot("alpha", "a", 2, 0),
            plot("alpha", "b", 2, 0),
            plot("beta", "a", 2, 0),
            plot("alpha", "a", 3, 0),
        ];
        for (i, x) in entries.iter().enumerate() {
            for (j, y) in entries.iter().enumerate() {
                if i == j {
                    assert_eq!(plot_order(x, y), Ordering::Equal);
                } else {
                    assert_ne!(
                        plot_order(x, y),
                        Ordering::Equal,
                        "{i} vs {j} tied, so their order falls back to input order"
                    );
                    assert_eq!(plot_order(x, y).reverse(), plot_order(y, x));
                }
            }
        }
    }
}
