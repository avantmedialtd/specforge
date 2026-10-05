//! Every key crossing the IPC boundary is camelCase — checked against what
//! serde actually emits, not against what the declarations appear to say.
//!
//! # Why this exists
//!
//! The Rust types and their `src/types.ts` mirrors are maintained by hand, with
//! no codegen, and **no general check could observe the two disagreeing**.
//! `cargo test` builds values in Rust and asserts on Rust; `tsc` checks the
//! mirror against itself; `bun test` uses fixtures shaped like the mirror.
//! Every gate stays green while the wire is wrong, and the only thing that
//! notices is a user clicking the feature.
//!
//! Two *targeted* guards already existed and are the precedent for this one,
//! not duplicates of it — extend whichever fits, but know all three exist:
//!
//! - `openspec-core/src/types.rs`'s `wire_shape_tests` covers `ArchiveScope` /
//!   `FileScope` **in the deserialize direction**, which this file cannot reach:
//!   `assert_camel_case` is `T: Serialize`, and those are argument types the
//!   frontend *sends*. That is the direction the `ArchiveScope` bug actually
//!   failed in (`missing field repo_id`).
//! - `openspec-app/src/events.rs`'s `tests` covers a few event payload keys by
//!   hand. All eleven payloads are now roots here too
//!   (`event_payloads_are_camel_case`), so a twelfth cannot be added unchecked;
//!   that module's assertions stay as the more specific statement of intent.
//!
//! The trap that exploited that gap twice: `rename_all` on an **enum** renames
//! its variants, while `rename_all_fields` (serde >= 1.0.184) renames the fields
//! inside its struct variants. On a plain struct, `rename_all` *does* rename
//! fields — so the same attribute name means two different things depending on
//! what it is attached to. It cost `ArchiveScope::Repo { repo_id }` (every
//! repository-scoped archive listing, broken at runtime) and
//! `WorkspaceView::Flat { display_name }` (flat workspaces' display names,
//! silently dropped).
//!
//! So this guard is deliberately *behavioural*: it serializes representative
//! values and walks the resulting JSON, which catches every shape of the
//! mistake — a missing `rename_all`, an enum's struct variant, a nested type
//! whose parent is correct — including in types that do not exist yet. A
//! source-scraping check would only catch the shape we already know about.
//!
//! # This is the crate that can see everything
//!
//! The roots below are traced from `crates/specforge/src/commands.rs`' return
//! types — re-trace them, rather than trusting this list, whenever a command is
//! added. Most live in `openspec-core` and the rest here, and `openspec-app`
//! depends on `openspec-core`, so this is the only place that can serialize all
//! of them in one test. It is also the crate that owns IPC payload shapes, so
//! the contract belongs here.
//!
//! # The guard's real limit — read this before extending it
//!
//! A key is only checked if the fixture actually *emits* it, so under-covering
//! a fixture silently under-covers the guard:
//!
//! - **Every `Option` here is `Some`.** A `None` field either serializes as
//!   `null` (which does reveal its key) or, under
//!   `skip_serializing_if = "Option::is_none"`, is omitted entirely — and then
//!   its key is never seen at all. `Author::name` / `Author::email` are exactly
//!   that case, so a fixture full of defaults would pass while covering
//!   nothing.
//! - **Every collection here is non-empty.** An empty `Vec` serializes as `[]`
//!   and reveals none of its element type's keys.
//! - **Every enum variant reachable through a root is exercised**, since a
//!   variant the fixture omits is never serialized.
//!
//! And one limit the walker cannot fix: it checks key **spelling**, not key
//! **identity**. Renaming `dirty_worktrees` to `dirty_paths` emits `dirtyPaths`
//! — no underscore, so this stays green while `src/types.ts` still declares
//! `dirtyWorktrees`. Exact key sets are asserted only for `WorkspaceView` (see
//! `openspec-core/tests/wire_shape.rs`); a rename is caught by review, not here.
//!
//! When you add a field, a variant, or a root, populate it here too. The
//! `finds_*` tests below pin the walker's own behaviour so it cannot rot into a
//! check that passes by never looking, and the string-valued enums are asserted
//! separately because the walker is structurally blind to them.
//!
//! `CacheEvent` is deliberately **not** a root: its serialized form does not
//! cross the boundary today (the shell translates each variant into an explicit
//! named payload), so asserting an IPC contract it does not have would be a
//! lie. It carries `rename_all_fields` anyway as trap-removal. If it ever does
//! cross the wire, it joins the roots then.

use openspec_app::bitbucket::BitbucketPullRequestsState;
use openspec_app::chatgpt_quota::{ChatGptQuotaState, ChatGptQuotaWindow};
use openspec_app::events::{
    CacheUpdatedPayload, ChangeAddedPayload, ChangeArchivedPayload, DocumentChangedPayload,
    GraphChangedPayload, InstancePayload, LogicalChangePayload, PanelMovedPayload,
    PullRequestProvider, PullRequestProviderChangedPayload, WorkspaceRemovedPayload,
};
use openspec_app::github::GithubPullRequestsState;
use openspec_app::pull_request_detail::{
    ConversationEntry, DiffSide, PullRequestCheck, PullRequestCheckState, PullRequestComment,
    PullRequestDetail, PullRequestDetailOutcome, PullRequestReference, ReviewState, ReviewThread,
};
use openspec_app::pull_request_links::{
    LinkedPullRequest, LinkedWorktree, PullRequestLinks, PullRequestRole, PullRequestWorktrees,
    WorktreePullRequests,
};
use openspec_app::pull_requests::{
    ChecksState, PullRequestSummary, PullRequestsStatus, ReviewSummary,
};
use openspec_app::quota::{ClaudeQuotaState, QuotaStatus, QuotaWindow, ScopedQuotaWindow};
use openspec_app::review_progress::{FileReviewProgress, FileReviewState, ReviewProgress};
use openspec_app::service::{ArtifactRead, IdentityInfo};
use openspec_app::settings::{
    BitbucketConfigView, DocumentWidth, GithubConfigView, PanelPosition, TailscaleConfig,
    WebServerConfig,
};

use openspec_core::dashboard::{
    DashboardData, HeatmapCell, ProgressData, RepoBreakdown, ShipEntry, StreakInfo, SummaryMetrics,
    TodayProgress,
};
use openspec_core::diff::{DiffContent, DiffFile, FileStatus, Hunk, Line, LineKind};
use openspec_core::garden::{GardenCommit, WorkspaceGarden};
use openspec_core::git::{CommitRef, RefKind, SpecCommitState, Trailer};
use openspec_core::graph::{CommitGraph, EdgeSegment, LaidOutCommit};
use openspec_core::identity::{Author, IdentityConfig};
use openspec_core::repo_view::{
    ChangeInstance, DivergenceLabel, LogicalChange, RepoView, WorkspaceView,
};
use openspec_core::types::{
    ArchivedChangeCopy, ArchivedChangeRow, ArchivedChangeSummary, ArtifactStatus, ChangeData,
    PaletteColor, RegisteredWorkspace, Section, Task, WorkspaceFileCopy, WorkspaceFileRow,
    WorkspaceFolder,
};

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

// ---------------------------------------------------------------- the walker

/// Every object key containing `_`, anywhere in `value`, each paired with the
/// path it was found at.
///
/// Descends into nested objects **and** through array elements, because the
/// failure this exists to catch is a correctly-spelled outer shape containing a
/// wrongly-spelled inner one — which is precisely what a shallow check misses.
///
/// `_` is a blunt predicate on purpose. No IPC key contains an underscore today
/// and none should: the convention is camelCase without exception. If a genuine
/// exception ever arrives, this fails loudly and the exception gets named
/// explicitly, which is the right outcome rather than a silent pass.
fn snake_case_keys(value: &Value, path: &str) -> Vec<String> {
    let mut found = Vec::new();
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if key.contains('_') {
                    found.push(child_path.clone());
                }
                found.extend(snake_case_keys(child, &child_path));
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                found.extend(snake_case_keys(item, &format!("{path}[{i}]")));
            }
        }
        _ => {}
    }
    found
}

/// Serializes `root` and fails naming the offending path(s) if any key is
/// snake_case.
fn assert_camel_case<T: Serialize>(label: &str, root: T) {
    let value =
        serde_json::to_value(root).unwrap_or_else(|e| panic!("{label} failed to serialize: {e}"));
    let offenders = snake_case_keys(&value, label);
    assert!(
        offenders.is_empty(),
        "{label}: {} snake_case key(s) on the wire: {}\n\
         `rename_all` on an *enum* renames variants — struct-variant fields need \
         `rename_all_fields = \"camelCase\"`.",
        offenders.len(),
        offenders.join(", ")
    );
}

// ------------------------------------------------ the walker's own behaviour
//
// Without these, a walker that returned `vec![]` unconditionally would make
// every root below pass. They are what stop this file becoming decoration.

#[test]
fn finds_a_snake_case_key_at_the_top_level() {
    let value = serde_json::json!({ "displayName": "ok", "display_name": "bad" });
    assert_eq!(snake_case_keys(&value, "root"), vec!["root.display_name"]);
}

#[test]
fn finds_a_snake_case_key_nested_inside_a_correct_parent() {
    let value = serde_json::json!({ "outer": { "inner": { "repo_id": "bad" } } });
    assert_eq!(
        snake_case_keys(&value, "root"),
        vec!["root.outer.inner.repo_id"]
    );
}

#[test]
fn finds_a_snake_case_key_inside_an_array_element() {
    let value = serde_json::json!({ "items": [{ "ok": 1 }, { "worktree_path": "bad" }] });
    assert_eq!(
        snake_case_keys(&value, "root"),
        vec!["root.items[1].worktree_path"]
    );
}

#[test]
fn reports_every_offender_not_just_the_first() {
    let value = serde_json::json!({ "a_one": 1, "b": { "c_two": 2 } });
    let mut found = snake_case_keys(&value, "root");
    found.sort();
    assert_eq!(found, vec!["root.a_one", "root.b.c_two"]);
}

#[test]
fn accepts_a_wholly_camel_case_payload() {
    let value = serde_json::json!({
        "displayName": "ok",
        "nested": { "repoId": "ok", "items": [{ "worktreePath": "ok" }] }
    });
    assert!(snake_case_keys(&value, "root").is_empty());
}

// -------------------------------------------------------------- the fixtures

fn author() -> Author {
    // Both fields are `skip_serializing_if = "Option::is_none"`: as `None`
    // their keys never appear, and the guard would never see them.
    Author::new(
        Some("Ada Lovelace".to_string()),
        Some("ada@example.com".to_string()),
    )
}

fn workspace_folder() -> WorkspaceFolder {
    WorkspaceFolder {
        uri: PathBuf::from("/tmp/ws"),
        name: "ws".to_string(),
    }
}

fn change_data() -> ChangeData {
    ChangeData {
        change_id: "add-thing".to_string(),
        title: Some("Add Thing".to_string()),
        sections: vec![Section {
            title: "1. Core".to_string(),
            tasks: vec![Task {
                text: "do it".to_string(),
                completed: true,
                indent: 2,
                line_number: 7,
            }],
        }],
        total_tasks: 1,
        completed_tasks: 1,
        artifacts: artifact_status(),
        workspace: workspace_folder(),
    }
}

fn artifact_status() -> ArtifactStatus {
    ArtifactStatus {
        proposal: true,
        specs: vec!["workspace-registry".to_string()],
        design: true,
        tasks: true,
    }
}

fn change_instance(divergence: DivergenceLabel, state: SpecCommitState) -> ChangeInstance {
    ChangeInstance {
        worktree_path: PathBuf::from("/tmp/repo"),
        branch: Some("master".to_string()),
        is_main_worktree: true,
        is_default_branch: true,
        is_archived_here: false,
        change: change_data(),
        modified_at: 1_700_000_000,
        divergence: Some(divergence),
        spec_commit_state: state,
    }
}

fn repo_view() -> RepoView {
    RepoView {
        repo_id: PathBuf::from("/tmp/repo/.git"),
        main_worktree: PathBuf::from("/tmp/repo"),
        name: "repo".to_string(),
        default_branch: Some("master".to_string()),
        active: vec![LogicalChange {
            name: "add-thing".to_string(),
            // Both `DivergenceLabel` variants and all three `SpecCommitState`
            // variants appear across these instances.
            instances: vec![
                change_instance(DivergenceLabel::Diverged, SpecCommitState::Committed),
                change_instance(DivergenceLabel::StaleVsArchived, SpecCommitState::Modified),
                change_instance(DivergenceLabel::Diverged, SpecCommitState::Untracked),
            ],
        }],
        archived: Vec::new(), // `skip_serializing` — never on the wire.
        display_name: Some("Repo Name".to_string()),
        color: Some(PaletteColor::Indigo),
        dirty: true,
        dirty_worktrees: vec![PathBuf::from("/tmp/repo")],
        worktrees: vec![PathBuf::from("/tmp/repo"), PathBuf::from("/tmp/repo-wt")],
        has_uncommitted_specs: true,
        disabled: false,
        worktree_refs: Vec::new(),
    }
}

fn flat_view() -> WorkspaceView {
    WorkspaceView::Flat {
        workspace: workspace_folder(),
        changes: vec![change_data()],
        display_name: Some("Nice Name".to_string()),
        color: Some(PaletteColor::Rose),
        disabled: false,
    }
}

fn commit_refs() -> Vec<CommitRef> {
    // Every `RefKind` variant.
    vec![
        CommitRef {
            name: "master".to_string(),
            kind: RefKind::LocalBranch,
        },
        CommitRef {
            name: "origin/master".to_string(),
            kind: RefKind::RemoteBranch,
        },
        CommitRef {
            name: "v0.1.0".to_string(),
            kind: RefKind::Tag,
        },
        CommitRef {
            name: "HEAD".to_string(),
            kind: RefKind::Head,
        },
    ]
}

fn commit_graph() -> CommitGraph {
    CommitGraph {
        commits: vec![LaidOutCommit {
            id: "abc123".to_string(),
            parents: vec!["def456".to_string()],
            author: "Ada".to_string(),
            date: "2026-09-09T00:00:00Z".to_string(),
            subject: "do a thing".to_string(),
            refs: commit_refs(),
            trailers: vec![Trailer {
                key: "Co-Authored-By".to_string(),
                value: "Someone".to_string(),
            }],
            row: 0,
            column: 1,
        }],
        edges: vec![EdgeSegment {
            band: 0,
            from_column: 0,
            to_column: 1,
        }],
        lane_count: 2,
        truncated: true,
    }
}

fn workspace_garden() -> WorkspaceGarden {
    WorkspaceGarden {
        label: "repo".to_string(),
        entry_key: "repo:/tmp/repo/.git".to_string(),
        active_count: 3,
        dormant: false,
        commits: vec![GardenCommit {
            id: "abc123".to_string(),
            row: 0,
            column: 1,
            subject: "do a thing".to_string(),
            refs: commit_refs(),
            date: "2026-09-09T00:00:00Z".to_string(),
            author: "Ada".to_string(),
            author_key: "ada@example.com".to_string(),
            is_me: true,
        }],
        edges: vec![EdgeSegment {
            band: 0,
            from_column: 0,
            to_column: 1,
        }],
        lane_count: 2,
    }
}

fn dashboard_data() -> DashboardData {
    DashboardData {
        summary: SummaryMetrics {
            active_changes: 2,
            completed_tasks: 5,
            total_tasks: 9,
            task_percent: 55,
            specs_touching: 3,
            repo_count: 1,
            worktree_count: 2,
            flat_count: 1,
        },
        repos: vec![RepoBreakdown {
            label: "repo".to_string(),
            active_count: 2,
            archived_count: 4,
        }],
        todays_ships: vec![ShipEntry {
            change_id: "add-thing".to_string(),
            title: Some("Add Thing".to_string()),
            workspace_label: "repo".to_string(),
            repo_id: PathBuf::from("/tmp/repo/.git"),
            worktree_path: PathBuf::from("/tmp/repo"),
            archive_dir: "2026-09-09-add-thing".to_string(),
            archived_at: Some(1_700_000_000),
        }],
        progress: ProgressData {
            today: TodayProgress {
                tasks_completed: 4,
                changes_archived: 1,
                commits_landed: 6,
                tasks_avg_centi: 250,
                changes_archived_avg_centi: 75,
                commits_avg_centi: 410,
            },
            streak: StreakInfo {
                current: 3,
                longest: 11,
            },
            heatmap: vec![HeatmapCell {
                day: "2026-09-09".to_string(),
                count: 6,
                tasks: 4,
                ships: 1,
                commits: 1,
                created: 2,
            }],
            in_flight: 2,
        },
    }
}

fn registered_workspace() -> RegisteredWorkspace {
    RegisteredWorkspace {
        uri: PathBuf::from("/tmp/ws"),
        name: "ws".to_string(),
        is_missing: false,
        display_name: Some("Nice Name".to_string()),
        color: Some(PaletteColor::Amber),
        repo_id: Some(PathBuf::from("/tmp/repo/.git")),
        disabled: true,
    }
}

/// The diff model commit detail and the pull-request view render
/// (`diff-view`: *Diff Model*): one file per status, every content state,
/// and every `Option` a file carries populated. Its hunk holds a removed line
/// flagged `no_newline` between two unflagged lines, so `noNewline` is
/// emitted once and its absence can be seen too.
fn diff_files() -> Vec<DiffFile> {
    let line = |kind, old_no, new_no, text: &str, no_newline| Line {
        kind,
        old_no,
        new_no,
        text: text.to_string(),
        no_newline,
    };
    let hunks = || DiffContent::Hunks {
        hunks: vec![Hunk {
            old_start: 10,
            old_lines: 2,
            new_start: 10,
            new_lines: 2,
            section: Some("fn main()".to_string()),
            lines: vec![
                line(LineKind::Context, Some(10), Some(10), "let a = 1;", false),
                line(LineKind::Removed, Some(11), None, "let b = 2;", true),
                line(LineKind::Added, None, Some(11), "let b = 3;", false),
            ],
        }],
    };
    let file = |status, content| DiffFile {
        old_path: Some("src/old.rs".to_string()),
        new_path: Some("src/new.rs".to_string()),
        old_mode: Some("100644".to_string()),
        new_mode: Some("100755".to_string()),
        status,
        additions: Some(1),
        deletions: Some(1),
        content,
    };
    vec![
        file(FileStatus::Added, hunks()),
        file(FileStatus::Modified, DiffContent::Withheld),
        file(FileStatus::Deleted, DiffContent::TooLarge),
        file(
            FileStatus::Renamed {
                similarity: Some(92),
            },
            DiffContent::Binary,
        ),
        file(
            FileStatus::Copied {
                similarity: Some(80),
            },
            hunks(),
        ),
        file(FileStatus::ModeChanged, hunks()),
        file(FileStatus::TypeChanged, hunks()),
    ]
}

// ------------------------------------------------------ the roots themselves

#[test]
fn workspace_view_both_variants_are_camel_case() {
    // The bug this whole change exists for lives in the `Flat` variant.
    assert_camel_case("WorkspaceView::Flat", flat_view());
    assert_camel_case("WorkspaceView::Repo", WorkspaceView::Repo(repo_view()));
    // As `get_workspace_views` actually returns them.
    assert_camel_case(
        "Vec<WorkspaceView>",
        vec![flat_view(), WorkspaceView::Repo(repo_view())],
    );
}

#[test]
fn core_command_payloads_are_camel_case() {
    assert_camel_case("RegisteredWorkspace", registered_workspace());
    assert_camel_case("DashboardData", dashboard_data());
    assert_camel_case("CommitGraph", commit_graph());
    assert_camel_case("WorkspaceGarden", workspace_garden());
    assert_camel_case("ChangeData", change_data());
    assert_camel_case("ArtifactStatus", artifact_status());
    assert_camel_case(
        "ArchivedChangeSummary",
        ArchivedChangeSummary {
            id: "add-thing".to_string(),
            date: Some("2026-09-09".to_string()),
            title: Some("Add Thing".to_string()),
            dir_name: "2026-09-09-add-thing".to_string(),
        },
    );
    assert_camel_case("Vec<DiffFile>", diff_files());
    // Both of these collapse per-worktree copies, so their nested `copies`
    // element types carry `worktree_path` — a multi-word field one level down,
    // reachable only because the walk descends through arrays.
    assert_camel_case(
        "Vec<ArchivedChangeRow>",
        vec![ArchivedChangeRow {
            id: "add-thing".to_string(),
            date: Some("2026-09-09".to_string()),
            title: Some("Add Thing".to_string()),
            copies: vec![ArchivedChangeCopy {
                worktree_path: PathBuf::from("/tmp/repo"),
                archive_dir: "2026-09-09-add-thing".to_string(),
                date: Some("2026-09-09".to_string()),
            }],
        }],
    );
    assert_camel_case(
        "Vec<WorkspaceFileRow>",
        vec![WorkspaceFileRow {
            path: "openspec/changes/demo/proposal.md".to_string(),
            copies: vec![WorkspaceFileCopy {
                worktree_path: PathBuf::from("/tmp/repo"),
            }],
            differs: true,
        }],
    );
}

#[test]
fn app_command_payloads_are_camel_case() {
    assert_camel_case(
        "IdentityInfo",
        IdentityInfo {
            config: IdentityConfig {
                display_name: Some("Ada".to_string()),
                aliases: vec![author()],
            },
            candidates: vec![author()],
        },
    );
    assert_camel_case(
        "ArtifactRead",
        ArtifactRead {
            body: "# Title".to_string(),
            modified_at: Some(1_700_000_000),
        },
    );
    assert_camel_case(
        "WebServerConfig",
        WebServerConfig {
            enabled: true,
            port: 4317,
            tailscale: TailscaleConfig {
                enabled: true,
                name: Some("host.tail1234.ts.net".to_string()),
                allowed_logins: vec!["ada@example.com".to_string()],
            },
        },
    );
}

/// The event payloads — the *other* wire, and the one the roots above miss.
///
/// `event_envelope` and `notice_envelope` serialize these onto both transports
/// (the Tauri `emit` in `specforge/src/events.rs` and the SSE bridge in
/// `specforge-web/src/sse.rs`), and `src/types.ts` mirrors them by hand, so
/// they are as much an IPC contract as any command return. Every one but
/// `PanelMovedPayload` and the two notices' payloads carries a multi-word
/// field. `events.rs`'s own `#[cfg(test)]` module asserts a few of these keys;
/// this covers all eleven mechanically, so adding a twelfth payload cannot
/// quietly go unchecked.
#[test]
fn event_payloads_are_camel_case() {
    let workspace = PathBuf::from("/tmp/ws");
    let repo_id = PathBuf::from("/tmp/repo/.git");
    assert_camel_case(
        "DocumentChangedPayload",
        DocumentChangedPayload {
            root: workspace.clone(),
            rel_path: "openspec/changes/demo/proposal.md".to_string(),
        },
    );
    assert_camel_case(
        "CacheUpdatedPayload",
        CacheUpdatedPayload {
            workspace: workspace.clone(),
        },
    );
    assert_camel_case(
        "ChangeAddedPayload",
        ChangeAddedPayload {
            workspace: workspace.clone(),
            change_id: "add-thing".to_string(),
        },
    );
    assert_camel_case(
        "ChangeArchivedPayload",
        ChangeArchivedPayload {
            workspace: workspace.clone(),
            change_id: "add-thing".to_string(),
        },
    );
    assert_camel_case(
        "WorkspaceRemovedPayload",
        WorkspaceRemovedPayload { workspace },
    );
    assert_camel_case(
        "LogicalChangePayload",
        LogicalChangePayload {
            repo_id: repo_id.clone(),
            change_name: "add-thing".to_string(),
        },
    );
    assert_camel_case(
        "InstancePayload",
        InstancePayload {
            repo_id: repo_id.clone(),
            change_name: "add-thing".to_string(),
            worktree_path: PathBuf::from("/tmp/repo"),
        },
    );
    assert_camel_case("GraphChangedPayload", GraphChangedPayload { repo_id });
    assert_camel_case(
        "PanelMovedPayload",
        PanelMovedPayload {
            provider: PullRequestProvider::Github,
            position: PanelPosition::RightBottom,
        },
    );
    // The two service notices' payloads: `review-progress-changed` carries the
    // reference itself.
    assert_camel_case("PullRequestReference", pull_request_reference());
    assert_camel_case(
        "PullRequestProviderChangedPayload",
        PullRequestProviderChangedPayload {
            provider: PullRequestProvider::Bitbucket,
            enabled: true,
        },
    );
}

fn pull_request_reference() -> PullRequestReference {
    PullRequestReference {
        provider: PullRequestProvider::Github,
        owner: "acme".to_string(),
        repo: "api".to_string(),
        number: 42,
    }
}

/// The reference is an argument the frontend *sends*, which every serialize
/// check here is blind to — the direction the `ArchiveScope` bug failed in.
/// This is the literal JSON `src/types.ts`'s `PullRequestReference` makes.
#[test]
fn the_pull_request_reference_deserializes_the_json_the_frontend_sends() {
    let sent: PullRequestReference =
        serde_json::from_str(r#"{"provider":"github","owner":"acme","repo":"api","number":42}"#)
            .expect("the frontend's JSON must parse");
    assert_eq!(sent, pull_request_reference());
    assert_eq!(sent.owner, "acme", "spelt as sent");
    let sent: PullRequestReference =
        serde_json::from_str(r#"{"provider":"bitbucket","owner":"Acme","repo":"API","number":7}"#)
            .expect("the frontend's JSON must parse");
    assert_eq!(sent.provider, PullRequestProvider::Bitbucket);
    assert_eq!(
        (sent.owner.as_str(), sent.repo.as_str(), sent.number),
        ("Acme", "API", 7)
    );
    // And back, key for key, as `review-progress-changed` carries it.
    assert_eq!(
        serde_json::to_value(pull_request_reference()).unwrap(),
        serde_json::json!({ "provider": "github", "owner": "acme", "repo": "api", "number": 42 })
    );
}

/// A pull-request row with every `Option` populated, so every key — the
/// nested review included — is emitted and seen.
fn pull_request_row(id: u64, url: &str) -> PullRequestSummary {
    PullRequestSummary {
        id,
        title: "Add the panel".to_string(),
        repo_full_name: "acme/specforge".to_string(),
        source_branch: "feature/panel".to_string(),
        destination_branch: "main".to_string(),
        url: url.to_string(),
        draft: true,
        updated_at_unix: 1_700_000_000,
        review: Some(ReviewSummary {
            approvals: 2,
            changes_requested: 1,
            pending: 1,
        }),
        open_tasks: 3,
        author: Some("ada".to_string()),
        checks: Some(ChecksState::Failing),
        conflicting: true,
        unresolved_threads: 2,
        source_repo_full_name: "ada/specforge".to_string(),
    }
}

/// The two pull-request panels' command returns: each provider's snapshot
/// (`get_bitbucket_pull_requests`, `get_github_pull_requests`) and each
/// write-only configuration view (`get_bitbucket_config`,
/// `get_github_config`). Every `Option` is `Some` and every collection
/// non-empty, so every key is actually emitted and seen.
#[test]
fn pull_request_payloads_are_camel_case() {
    assert_camel_case(
        "BitbucketPullRequestsState",
        BitbucketPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: true,
            fetched_at_unix: Some(1_700_000_000),
            pull_requests: vec![pull_request_row(
                42,
                "https://bitbucket.org/acme/specforge/pull-requests/42",
            )],
            skipped_workspaces: vec!["locked-workspace".to_string()],
        },
    );
    assert_camel_case(
        "GithubPullRequestsState",
        GithubPullRequestsState {
            status: PullRequestsStatus::Ok,
            stale: true,
            fetched_at_unix: Some(1_700_000_000),
            authored: vec![pull_request_row(
                42,
                "https://github.com/acme/specforge/pull/42",
            )],
            review_requested: vec![pull_request_row(
                43,
                "https://github.com/acme/specforge/pull/43",
            )],
            withheld: 2,
        },
    );
    assert_camel_case(
        "BitbucketConfigView",
        BitbucketConfigView {
            enabled: true,
            username: Some("ada@example.com".to_string()),
            token_set: true,
            refresh_secs: 120,
            panel_position: PanelPosition::LeftTop,
        },
    );
    assert_camel_case(
        "GithubConfigView",
        GithubConfigView {
            enabled: true,
            token_set: true,
            refresh_secs: 120,
            panel_position: PanelPosition::RightBottom,
        },
    );
}

/// The links snapshot `get_pull_request_links` serves
/// (`pull-request-worktree-links`: *The Pull-Request Links Snapshot*), with
/// every `Option` populated so every key is emitted and seen, and its keys
/// checked by identity against the `src/types.ts` mirror.
#[test]
fn pull_request_links_are_camel_case() {
    let links = PullRequestLinks {
        worktrees: vec![WorktreePullRequests {
            worktree_path: PathBuf::from("/code/api"),
            pull_requests: vec![LinkedPullRequest {
                provider: PullRequestProvider::Github,
                role: PullRequestRole::ReviewRequested,
                id: 42,
                title: "Add rate limits".to_string(),
                url: "https://github.com/acme/api/pull/42".to_string(),
                repo_full_name: "acme/api".to_string(),
                draft: true,
                checks: Some(ChecksState::Pending),
                conflicting: true,
                review: Some(ReviewSummary {
                    approvals: 1,
                    changes_requested: 1,
                    pending: 1,
                }),
            }],
        }],
        pull_requests: vec![PullRequestWorktrees {
            url: "https://github.com/acme/api/pull/42".to_string(),
            worktrees: vec![LinkedWorktree {
                repo_id: PathBuf::from("/code/api/.git"),
                worktree_path: PathBuf::from("/code/api"),
                branch: Some("feature".to_string()),
            }],
        }],
    };
    assert_camel_case("PullRequestLinks", links.clone());
    let wire = serde_json::to_value(&links).unwrap();
    let entry = &wire["worktrees"][0]["pullRequests"][0];
    for key in [
        "provider",
        "role",
        "id",
        "title",
        "url",
        "repoFullName",
        "draft",
        "checks",
        "conflicting",
        "review",
    ] {
        assert!(entry.get(key).is_some(), "linked pull request key {key}");
    }
    assert!(wire["worktrees"][0].get("worktreePath").is_some());
    assert!(
        wire["pullRequests"][0].get("url").is_some(),
        "the key worktreesForPullRequest looks rows up by"
    );
    let worktree = &wire["pullRequests"][0]["worktrees"][0];
    for key in ["repoId", "worktreePath", "branch"] {
        assert!(worktree.get(key).is_some(), "linked worktree key {key}");
    }
}

/// A comment with every `Option` populated.
fn pull_request_comment(id: &str) -> PullRequestComment {
    PullRequestComment {
        id: id.to_string(),
        author: Some("ada".to_string()),
        body: "Looks close".to_string(),
        posted_at_unix: 1_700_000_000,
        url: Some(format!(
            "https://github.com/acme/api/pull/42#issuecomment-{id}"
        )),
        minimized_reason: Some("outdated".to_string()),
        deleted: true,
    }
}

/// The detail `get_pull_request_detail` serves (`pull-request-viewer`:
/// *Pull-Request View*), with every `Option` populated and every collection
/// non-empty: a comment and a review summary, a check in each state, threads
/// on each side, and every file status and content state of the diff model.
fn pull_request_detail() -> PullRequestDetail {
    let states = [
        PullRequestCheckState::Passing,
        PullRequestCheckState::Failing,
        PullRequestCheckState::Pending,
        PullRequestCheckState::Neutral,
        PullRequestCheckState::Skipped,
        PullRequestCheckState::Cancelled,
        PullRequestCheckState::Unknown,
    ];
    let thread = |id: &str, side, start_side| ReviewThread {
        id: id.to_string(),
        path: "src/api.ts".to_string(),
        side,
        line: Some(12),
        start_side: Some(start_side),
        start_line: Some(10),
        original_line: Some(11),
        original_start_line: Some(9),
        resolved: true,
        outdated: true,
        comments: vec![pull_request_comment(&format!("{id}-c"))],
    };
    PullRequestDetail {
        reference: pull_request_reference(),
        row: pull_request_row(42, "https://github.com/acme/api/pull/42"),
        head_branch: "feature/limits".to_string(),
        base_branch: "main".to_string(),
        head_commit: "a".repeat(40),
        base_commit: "b".repeat(40),
        author: Some("ada".to_string()),
        description: "Adds rate limits.".to_string(),
        conversation: [
            None,
            Some(ReviewState::Approved),
            Some(ReviewState::ChangesRequested),
            Some(ReviewState::Commented),
            Some(ReviewState::Dismissed),
        ]
        .into_iter()
        .enumerate()
        .map(|(n, review)| ConversationEntry {
            review,
            comment: pull_request_comment(&n.to_string()),
        })
        .collect(),
        checks: states
            .into_iter()
            .map(|state| PullRequestCheck {
                name: format!("{state:?}"),
                state,
                url: Some("https://ci.example/run/7".to_string()),
            })
            .collect(),
        threads: vec![
            thread("t1", DiffSide::Old, DiffSide::New),
            thread("t2", DiffSide::New, DiffSide::Old),
        ],
        files: diff_files(),
        unlisted_files: 200,
        read_at_unix: 1_700_000_000,
        no_longer_listed: true,
    }
}

/// Every answer `get_pull_request_detail` can give, in the order the
/// `src/types.ts` union declares them.
fn pull_request_detail_outcomes() -> Vec<PullRequestDetailOutcome> {
    vec![
        PullRequestDetailOutcome::Detail {
            detail: Box::new(pull_request_detail()),
        },
        PullRequestDetailOutcome::NotListed,
        PullRequestDetailOutcome::NotCached,
        PullRequestDetailOutcome::Refused,
        PullRequestDetailOutcome::Unauthenticated,
        PullRequestDetailOutcome::Unavailable,
        PullRequestDetailOutcome::Deferred {
            until_unix: 1_700_000_600,
            detail: Some(Box::new(pull_request_detail())),
        },
        PullRequestDetailOutcome::Transient,
    ]
}

#[test]
fn pull_request_detail_outcomes_are_camel_case() {
    for outcome in pull_request_detail_outcomes() {
        assert_camel_case("PullRequestDetailOutcome", outcome);
    }
    // As `get_pull_request_file` returns a withheld file: a `DiffFile`.
    assert_camel_case("Vec<DiffFile>", diff_files());
}

/// The outcome union `src/types.ts` matches on `kind`, by exact value. A
/// dropped `rename_all` would send `NotListed`, and a dropped
/// `rename_all_fields` would send `until_unix` beside a well-spelt `kind`.
#[test]
fn pull_request_detail_outcome_discriminants_match_the_declared_union() {
    let kinds: Vec<Value> = pull_request_detail_outcomes()
        .into_iter()
        .map(|outcome| serde_json::to_value(outcome).unwrap()["kind"].clone())
        .collect();
    assert_eq!(
        kinds,
        [
            "detail",
            "notListed",
            "notCached",
            "refused",
            "unauthenticated",
            "unavailable",
            "deferred",
            "transient",
        ]
    );
    let deferred = serde_json::to_value(&pull_request_detail_outcomes()[6]).unwrap();
    assert_eq!(deferred["untilUnix"], 1_700_000_600);
    assert!(deferred["detail"].is_object());
    let detail = serde_json::to_value(&pull_request_detail_outcomes()[0]).unwrap();
    assert!(detail["detail"].is_object());
    let refused = serde_json::to_value(PullRequestDetailOutcome::Refused).unwrap();
    assert_eq!(
        refused,
        serde_json::json!({ "kind": "refused" }),
        "no content"
    );
}

/// The detail's keys by identity, as `src/types.ts` reads them on
/// `PullRequestDetail`, `ConversationEntry`, `PullRequestComment`,
/// `PullRequestCheck` and `ReviewThread`. A rename to another camelCase
/// spelling would pass the walker; it fails here.
#[test]
fn pull_request_detail_keys_match_the_declared_mirror() {
    let keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    let sorted = |names: &[&str]| -> Vec<String> {
        let mut names: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        names.sort();
        names
    };
    let wire = serde_json::to_value(pull_request_detail()).unwrap();
    assert_eq!(
        keys(&wire),
        sorted(&[
            "reference",
            "row",
            "headBranch",
            "baseBranch",
            "headCommit",
            "baseCommit",
            "author",
            "description",
            "conversation",
            "checks",
            "threads",
            "files",
            "unlistedFiles",
            "readAtUnix",
            "noLongerListed",
        ])
    );
    assert_eq!(
        keys(&wire["conversation"][0]),
        sorted(&["review", "comment"])
    );
    assert_eq!(
        keys(&wire["conversation"][0]["comment"]),
        sorted(&[
            "id",
            "author",
            "body",
            "postedAtUnix",
            "url",
            "minimizedReason",
            "deleted",
        ])
    );
    assert_eq!(keys(&wire["checks"][0]), sorted(&["name", "state", "url"]));
    assert_eq!(
        keys(&wire["threads"][0]),
        sorted(&[
            "id",
            "path",
            "side",
            "line",
            "startSide",
            "startLine",
            "originalLine",
            "originalStartLine",
            "resolved",
            "outdated",
            "comments",
        ])
    );
    assert_eq!(wire["reference"]["provider"], "github");
    assert_eq!(wire["row"]["repoFullName"], "acme/specforge");
}

/// The mirror declares every `Option` as `T | null`: an absent value crosses
/// as `null`, its key still present.
#[test]
fn pull_request_detail_absent_values_cross_as_null() {
    let comment = PullRequestComment {
        author: None,
        url: None,
        minimized_reason: None,
        ..pull_request_comment("c")
    };
    let wire = serde_json::to_value(&comment).unwrap();
    for key in ["author", "url", "minimizedReason"] {
        assert_eq!(wire.get(key), Some(&Value::Null), "comment key {key}");
    }
    let entry = serde_json::to_value(ConversationEntry {
        review: None,
        comment,
    })
    .unwrap();
    assert_eq!(entry.get("review"), Some(&Value::Null));
    let thread = serde_json::to_value(ReviewThread {
        line: None,
        start_side: None,
        start_line: None,
        original_line: None,
        original_start_line: None,
        ..pull_request_detail().threads.remove(0)
    })
    .unwrap();
    for key in [
        "line",
        "startSide",
        "startLine",
        "originalLine",
        "originalStartLine",
    ] {
        assert_eq!(thread.get(key), Some(&Value::Null), "thread key {key}");
    }
    let deferred = serde_json::to_value(PullRequestDetailOutcome::Deferred {
        until_unix: 1,
        detail: None,
    })
    .unwrap();
    assert_eq!(deferred.get("detail"), Some(&Value::Null));
    let check = serde_json::to_value(PullRequestCheck {
        name: "build".to_string(),
        state: PullRequestCheckState::Passing,
        url: None,
    })
    .unwrap();
    assert_eq!(check.get("url"), Some(&Value::Null));
}

/// What `get_review_progress` serves (`pull-request-viewer`: *Review
/// Progress*): a file in each state, one of them keyed by the head, and the
/// last mark's head present.
fn review_progress() -> ReviewProgress {
    let file = |path: &str, state, keyed_by_head| FileReviewProgress {
        path: path.to_string(),
        state,
        keyed_by_head,
    };
    ReviewProgress {
        files: vec![
            file("src/api.ts", FileReviewState::Viewed, false),
            file("logo.png", FileReviewState::ChangedSinceViewed, true),
            file("README.md", FileReviewState::Unviewed, false),
        ],
        viewed: 1,
        changed_since_viewed: 1,
        total: 3,
        last_marked_head: Some("a".repeat(40)),
    }
}

/// The progress's keys by identity, as `src/types.ts` reads them on
/// `ReviewProgress` and `FileReviewProgress`; `lastMarkedHead` crosses as
/// `null` before any mark, its key still present.
#[test]
fn review_progress_keys_match_the_declared_mirror() {
    assert_camel_case("ReviewProgress", review_progress());
    let keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    let wire = serde_json::to_value(review_progress()).unwrap();
    assert_eq!(
        keys(&wire),
        [
            "changedSinceViewed",
            "files",
            "lastMarkedHead",
            "total",
            "viewed"
        ]
    );
    assert_eq!(keys(&wire["files"][1]), ["keyedByHead", "path", "state"]);
    assert_eq!(wire["files"][1]["keyedByHead"], true);
    assert_eq!(
        (&wire["viewed"], &wire["changedSinceViewed"], &wire["total"]),
        (&Value::from(1), &Value::from(1), &Value::from(3))
    );
    let unmarked = serde_json::to_value(ReviewProgress {
        last_marked_head: None,
        ..review_progress()
    })
    .unwrap();
    assert_eq!(unmarked.get("lastMarkedHead"), Some(&Value::Null));
}

/// `FileReviewState` — `src/types.ts`: `"viewed" | "changedSinceViewed" |
/// "unviewed"`. The two-word state is the one a dropped `rename_all` would
/// break silently.
#[test]
fn file_review_state_matches_the_declared_union() {
    assert_wire_value("Viewed", FileReviewState::Viewed, "viewed");
    assert_wire_value(
        "ChangedSinceViewed",
        FileReviewState::ChangedSinceViewed,
        "changedSinceViewed",
    );
    assert_wire_value("Unviewed", FileReviewState::Unviewed, "unviewed");
}

/// `DiffSide` — `src/types.ts`: `"old" | "new"`.
#[test]
fn diff_side_matches_the_declared_union() {
    assert_wire_value("DiffSide::Old", DiffSide::Old, "old");
    assert_wire_value("DiffSide::New", DiffSide::New, "new");
}

/// `PullRequestCheckState` — `src/types.ts`: `"passing" | "failing" |
/// "pending" | "neutral" | "skipped" | "cancelled" | "unknown"`.
#[test]
fn pull_request_check_state_matches_the_declared_union() {
    for (state, expected) in [
        (PullRequestCheckState::Passing, "passing"),
        (PullRequestCheckState::Failing, "failing"),
        (PullRequestCheckState::Pending, "pending"),
        (PullRequestCheckState::Neutral, "neutral"),
        (PullRequestCheckState::Skipped, "skipped"),
        (PullRequestCheckState::Cancelled, "cancelled"),
        (PullRequestCheckState::Unknown, "unknown"),
    ] {
        assert_wire_value("PullRequestCheckState", state, expected);
    }
}

/// `ReviewState` — `src/types.ts`: `"approved" | "changesRequested" |
/// "commented" | "dismissed"`. The two-word state is the one a dropped
/// `rename_all` would break silently.
#[test]
fn review_state_matches_the_declared_union() {
    assert_wire_value("Approved", ReviewState::Approved, "approved");
    assert_wire_value(
        "ChangesRequested",
        ReviewState::ChangesRequested,
        "changesRequested",
    );
    assert_wire_value("Commented", ReviewState::Commented, "commented");
    assert_wire_value("Dismissed", ReviewState::Dismissed, "dismissed");
}

/// `PullRequestRole` — `src/types.ts`: `"authored" | "reviewRequested"`.
#[test]
fn pull_request_role_matches_the_declared_union() {
    assert_wire_value("Authored", PullRequestRole::Authored, "authored");
    assert_wire_value(
        "ReviewRequested",
        PullRequestRole::ReviewRequested,
        "reviewRequested",
    );
}

/// The row's keys by identity, not just by spelling: the four GitHub signals
/// `src/types.ts` reads on `PullRequestSummary`, and the two lists and count
/// it reads on `GithubPullRequestsState`. A rename to another camelCase
/// spelling would pass the walker; it fails here.
#[test]
fn github_row_and_snapshot_keys_match_the_declared_mirror() {
    let row = serde_json::to_value(pull_request_row(1, "https://github.com/a/b/pull/1")).unwrap();
    for key in [
        "author",
        "checks",
        "conflicting",
        "unresolvedThreads",
        "openTasks",
        "sourceRepoFullName",
    ] {
        assert!(row.get(key).is_some(), "row key {key}");
    }
    let snapshot = serde_json::to_value(GithubPullRequestsState {
        status: PullRequestsStatus::Ok,
        stale: false,
        fetched_at_unix: None,
        authored: Vec::new(),
        review_requested: Vec::new(),
        withheld: 0,
    })
    .unwrap();
    for key in [
        "authored",
        "reviewRequested",
        "withheld",
        "fetchedAtUnix",
        "stale",
        "status",
    ] {
        assert!(snapshot.get(key).is_some(), "snapshot key {key}");
    }
    let payload = serde_json::to_value(PanelMovedPayload {
        provider: PullRequestProvider::Bitbucket,
        position: PanelPosition::LeftTop,
    })
    .unwrap();
    assert_eq!(payload["provider"], "bitbucket");
}

/// The diff model's keys by identity, as `src/types.ts` reads them on
/// `DiffFile`, `Hunk`, `Line` and the rename and copy statuses. A rename to
/// another camelCase spelling would pass the walker; it fails here.
#[test]
fn diff_model_keys_match_the_declared_mirror() {
    let wire = serde_json::to_value(diff_files()).unwrap();
    let file = &wire[0];
    for key in [
        "oldPath",
        "newPath",
        "oldMode",
        "newMode",
        "status",
        "additions",
        "deletions",
        "content",
    ] {
        assert!(file.get(key).is_some(), "file key {key}");
    }
    let hunk = &file["content"]["hunks"][0];
    for key in [
        "oldStart", "oldLines", "newStart", "newLines", "section", "lines",
    ] {
        assert!(hunk.get(key).is_some(), "hunk key {key}");
    }
    let line = &hunk["lines"][0];
    for key in ["kind", "oldNo", "newNo", "text"] {
        assert!(line.get(key).is_some(), "line key {key}");
    }
    assert_eq!(wire[3]["status"]["similarity"], 92);
    assert_eq!(wire[4]["status"]["similarity"], 80);
}

/// The mirror declares every `Option` as `T | null`, never as an optional
/// key: an absent value crosses as `null`, with its key still present.
#[test]
fn diff_model_absent_values_cross_as_null() {
    let file = DiffFile {
        old_path: None,
        new_path: None,
        old_mode: None,
        new_mode: None,
        status: FileStatus::Renamed { similarity: None },
        additions: None,
        deletions: None,
        content: DiffContent::Hunks {
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
                section: None,
                lines: vec![Line {
                    kind: LineKind::Added,
                    old_no: None,
                    new_no: Some(1),
                    text: String::new(),
                    no_newline: false,
                }],
            }],
        },
    };
    let wire = serde_json::to_value(file).unwrap();
    for key in [
        "oldPath",
        "newPath",
        "oldMode",
        "newMode",
        "additions",
        "deletions",
    ] {
        assert_eq!(wire.get(key), Some(&Value::Null), "file key {key}");
    }
    assert_eq!(wire["status"].get("similarity"), Some(&Value::Null));
    let hunk = &wire["content"]["hunks"][0];
    assert_eq!(hunk.get("section"), Some(&Value::Null));
    assert_eq!(hunk["lines"][0].get("oldNo"), Some(&Value::Null));
}

/// `noNewline` is `true` on the line it flags and absent from every other
/// line (`diff-view`: *The model crosses the wire with exact discriminants*).
/// The mirror declares it `noNewline?: true`, so a `false` on the wire would
/// already be a type the frontend does not expect.
#[test]
fn no_newline_is_present_only_on_the_line_it_flags() {
    let wire = serde_json::to_value(diff_files()).unwrap();
    let flagged = &wire[0]["content"]["hunks"][0]["lines"][1];
    assert_eq!(flagged["kind"], "removed");
    assert_eq!(flagged["noNewline"], true);
    let mut unflagged = 0;
    for file in wire.as_array().unwrap() {
        let Some(hunks) = file["content"].get("hunks") else {
            continue;
        };
        for line in hunks[0]["lines"].as_array().unwrap() {
            if line["kind"] == "removed" {
                assert_eq!(line.get("noNewline"), Some(&Value::Bool(true)));
            } else {
                assert_eq!(line.get("noNewline"), None, "{line}");
                unflagged += 1;
            }
        }
    }
    assert_eq!(unflagged, 8, "two unflagged lines in each of four hunks");
}

#[test]
fn quota_payloads_are_camel_case() {
    assert_camel_case(
        "ClaudeQuotaState",
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: true,
            five_hour: Some(QuotaWindow {
                utilization: 42,
                resets_at_unix: Some(1_700_000_000),
            }),
            seven_day: Some(QuotaWindow {
                utilization: 17,
                resets_at_unix: Some(1_700_600_000),
            }),
            scoped: vec![ScopedQuotaWindow {
                model: "Fable".to_string(),
                utilization: 8,
                resets_at_unix: Some(1_700_600_000),
            }],
        },
    );
    assert_camel_case(
        "ChatGptQuotaState",
        ChatGptQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            primary: Some(ChatGptQuotaWindow {
                utilization: 30,
                resets_at_unix: Some(1_700_000_000),
                window_secs: Some(18_000),
            }),
            secondary: Some(ChatGptQuotaWindow {
                utilization: 55,
                resets_at_unix: Some(1_700_600_000),
                window_secs: Some(604_800),
            }),
        },
    );
}

// -------------------------------------------------- string-valued enums
//
// `snake_case_keys` walks objects and arrays; a unit enum variant serializes
// to a bare `Value::String`, which has no keys at all. Passing one to
// `assert_camel_case` therefore ALWAYS passes — it is not a weak check, it is
// a vacuous one. (Confirmed by planting `rename_all = "SCREAMING_SNAKE_CASE"`
// on `QuotaStatus`: every assertion above stayed green.)
//
// The underscore predicate would be the wrong tool even if it could see
// values, because the mistake here does not produce an underscore: drop
// `rename_all` from `DivergenceLabel` and the wire carries `"StaleVsArchived"`
// — no `_`, and `src/types.ts` declares `"staleVsArchived"`. So these are
// asserted as exact strings, transcribed from the TypeScript unions they must
// match. This is the sibling trap `crates/CLAUDE.md` warns about: string-valued
// enums use `kebab-case` here (`PaletteColor`) and camelCase elsewhere, and a
// multi-word variant silently breaks the union either way.

/// Asserts one enum variant's exact wire string.
fn assert_wire_value<T: Serialize>(label: &str, value: T, expected: &str) {
    let got = serde_json::to_value(value).expect("serialize");
    assert_eq!(
        got,
        Value::String(expected.to_string()),
        "{label}: wire value drifted from the union src/types.ts declares"
    );
}

/// `RefKind` — `src/types.ts`: `"localBranch" | "remoteBranch" | "tag" | "head"`.
#[test]
fn ref_kind_matches_the_declared_union() {
    assert_wire_value("RefKind::LocalBranch", RefKind::LocalBranch, "localBranch");
    assert_wire_value(
        "RefKind::RemoteBranch",
        RefKind::RemoteBranch,
        "remoteBranch",
    );
    assert_wire_value("RefKind::Tag", RefKind::Tag, "tag");
    assert_wire_value("RefKind::Head", RefKind::Head, "head");
}

/// `DivergenceLabel` — `src/types.ts`: `"diverged" | "staleVsArchived"`. The
/// two-word variant is the one that matters: it is the only thing standing
/// between a dropped `rename_all` and a silently unmatched union member.
#[test]
fn divergence_label_matches_the_declared_union() {
    assert_wire_value(
        "DivergenceLabel::Diverged",
        DivergenceLabel::Diverged,
        "diverged",
    );
    assert_wire_value(
        "DivergenceLabel::StaleVsArchived",
        DivergenceLabel::StaleVsArchived,
        "staleVsArchived",
    );
}

/// `SpecCommitState` — `src/types.ts`: `"committed" | "modified" | "untracked"`.
#[test]
fn spec_commit_state_matches_the_declared_union() {
    assert_wire_value("Committed", SpecCommitState::Committed, "committed");
    assert_wire_value("Modified", SpecCommitState::Modified, "modified");
    assert_wire_value("Untracked", SpecCommitState::Untracked, "untracked");
}

/// `PaletteColor` — **kebab-case**, not camelCase, and all eight variants, since
/// the fixtures above use only a few. Every variant is one word today; the day
/// one is not, this is what notices.
#[test]
fn palette_color_matches_the_declared_union() {
    for (variant, expected) in [
        (PaletteColor::Indigo, "indigo"),
        (PaletteColor::Blue, "blue"),
        (PaletteColor::Teal, "teal"),
        (PaletteColor::Green, "green"),
        (PaletteColor::Amber, "amber"),
        (PaletteColor::Orange, "orange"),
        (PaletteColor::Rose, "rose"),
        (PaletteColor::Purple, "purple"),
    ] {
        assert_wire_value("PaletteColor", variant, expected);
    }
}

/// `QuotaStatus` — `src/types.ts`: `"disabled" | "unauthenticated" | "unavailable" | "ok"`.
#[test]
fn quota_status_matches_the_declared_union() {
    assert_wire_value("Disabled", QuotaStatus::Disabled, "disabled");
    assert_wire_value(
        "Unauthenticated",
        QuotaStatus::Unauthenticated,
        "unauthenticated",
    );
    assert_wire_value("Unavailable", QuotaStatus::Unavailable, "unavailable");
    assert_wire_value("Ok", QuotaStatus::Ok, "ok");
}

/// `PanelPosition` — **kebab-case**, and every variant is two words, so this is
/// the one string enum where a dropped `rename_all` cannot go unnoticed by the
/// union — and exactly the case the underscore walker cannot see. `src/types.ts`:
/// `"left-top" | "left-bottom" | "right-top" | "right-bottom"`.
#[test]
fn panel_position_matches_the_declared_union() {
    assert_wire_value("LeftTop", PanelPosition::LeftTop, "left-top");
    assert_wire_value("LeftBottom", PanelPosition::LeftBottom, "left-bottom");
    assert_wire_value("RightTop", PanelPosition::RightTop, "right-top");
    assert_wire_value("RightBottom", PanelPosition::RightBottom, "right-bottom");
}

/// `ChecksState` — `src/types.ts`: `"passing" | "failing" | "pending"`.
#[test]
fn checks_state_matches_the_declared_union() {
    assert_wire_value("Passing", ChecksState::Passing, "passing");
    assert_wire_value("Failing", ChecksState::Failing, "failing");
    assert_wire_value("Pending", ChecksState::Pending, "pending");
}

/// `PullRequestProvider` — `src/types.ts`: `"bitbucket" | "github"`.
#[test]
fn pull_request_provider_matches_the_declared_union() {
    assert_wire_value("Bitbucket", PullRequestProvider::Bitbucket, "bitbucket");
    assert_wire_value("Github", PullRequestProvider::Github, "github");
}

/// `PullRequestsStatus` — `src/types.ts`:
/// `"disabled" | "unauthenticated" | "unavailable" | "ok"`.
#[test]
fn pull_requests_status_matches_the_declared_union() {
    assert_wire_value("Disabled", PullRequestsStatus::Disabled, "disabled");
    assert_wire_value(
        "Unauthenticated",
        PullRequestsStatus::Unauthenticated,
        "unauthenticated",
    );
    assert_wire_value(
        "Unavailable",
        PullRequestsStatus::Unavailable,
        "unavailable",
    );
    assert_wire_value("Ok", PullRequestsStatus::Ok, "ok");
}

/// `DocumentWidth` — `src/types.ts`: `"compact" | "default" | "wide" | "full"`.
#[test]
fn document_width_matches_the_declared_union() {
    assert_wire_value("Compact", DocumentWidth::Compact, "compact");
    assert_wire_value("Default", DocumentWidth::Default, "default");
    assert_wire_value("Wide", DocumentWidth::Wide, "wide");
    assert_wire_value("Full", DocumentWidth::Full, "full");
}

/// `LineKind` — `src/types.ts`: `"context" | "added" | "removed"`.
#[test]
fn line_kind_matches_the_declared_union() {
    assert_wire_value("LineKind::Context", LineKind::Context, "context");
    assert_wire_value("LineKind::Added", LineKind::Added, "added");
    assert_wire_value("LineKind::Removed", LineKind::Removed, "removed");
}

/// The tagged enum's discriminant is a string value too, and `kind` is what the
/// TypeScript union matches on — so it is subject to the same blind spot.
#[test]
fn workspace_view_discriminants_match_the_declared_union() {
    assert_eq!(serde_json::to_value(flat_view()).unwrap()["kind"], "flat");
    assert_eq!(
        serde_json::to_value(WorkspaceView::Repo(repo_view())).unwrap()["kind"],
        "repo"
    );
}

/// `FileStatus` and `DiffContent`, each a union `src/types.ts` matches on
/// `kind`: every status, then every content state, by exact value. A dropped
/// `rename_all` would send `ModeChanged` or `TooLarge`, which no member of
/// either union matches and the walker cannot see, since a value has no
/// underscore to find.
#[test]
fn diff_model_discriminants_match_the_declared_unions() {
    let wire = serde_json::to_value(diff_files()).unwrap();
    let files = wire.as_array().unwrap();
    let kinds = |field: &str| -> Vec<Value> {
        files
            .iter()
            .map(|file| file[field]["kind"].clone())
            .collect()
    };
    assert_eq!(
        kinds("status"),
        [
            "added",
            "modified",
            "deleted",
            "renamed",
            "copied",
            "modeChanged",
            "typeChanged",
        ]
    );
    assert_eq!(
        kinds("content")[..4],
        ["hunks", "withheld", "tooLarge", "binary"]
    );
}
