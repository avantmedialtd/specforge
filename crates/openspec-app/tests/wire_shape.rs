//! Every key crossing the IPC boundary is camelCase — checked against what
//! serde actually emits, not against what the declarations appear to say.
//!
//! # Why this exists
//!
//! The Rust types and their `src/types.ts` mirrors are maintained by hand, with
//! no codegen, and **nothing in the repository could observe the two
//! disagreeing**. `cargo test` builds values in Rust and asserts on Rust; `tsc`
//! checks the mirror against itself; `bun test` uses fixtures shaped like the
//! mirror. Every gate stays green while the wire is wrong, and the only thing
//! that notices is a user clicking the feature.
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
//! types. They split across two crates — nine live in `openspec-core`, six here
//! — and `openspec-app` depends on `openspec-core`, so this is the only place
//! that can serialize all of them in one test. It is also the crate that owns
//! IPC payload shapes, so the contract belongs here.
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
//! - **Every enum variant is exercised**, since a variant the fixture omits is
//!   never serialized.
//!
//! When you add a field, a variant, or a root, populate it here too. The
//! `finds_*` tests below pin the walker's own behaviour so it cannot rot into a
//! check that passes by never looking.
//!
//! `CacheEvent` is deliberately **not** a root: its serialized form does not
//! cross the boundary today (the shell translates each variant into an explicit
//! named payload), so asserting an IPC contract it does not have would be a
//! lie. It carries `rename_all_fields` anyway as trap-removal. If it ever does
//! cross the wire, it joins the roots then.

use openspec_app::chatgpt_quota::{ChatGptQuotaState, ChatGptQuotaWindow};
use openspec_app::quota::{ClaudeQuotaState, QuotaStatus, QuotaWindow, ScopedQuotaWindow};
use openspec_app::service::{ArtifactRead, IdentityInfo};
use openspec_app::settings::{DocumentWidth, TailscaleConfig, WebServerConfig};

use openspec_core::dashboard::{
    DashboardData, HeatmapCell, ProgressData, RepoBreakdown, ShipEntry, StreakInfo, SummaryMetrics,
    TodayProgress,
};
use openspec_core::garden::{GardenCommit, WorkspaceGarden};
use openspec_core::git::{CommitFile, CommitRef, RefKind, SpecCommitState, Trailer};
use openspec_core::graph::{CommitGraph, EdgeSegment, LaidOutCommit};
use openspec_core::identity::{Author, IdentityConfig};
use openspec_core::repo_view::{
    ChangeInstance, DivergenceLabel, LogicalChange, RepoView, WorkspaceView,
};
use openspec_core::types::{
    ArchivedChangeSummary, ArtifactStatus, ChangeData, PaletteColor, RegisteredWorkspace, Section,
    Task, WorkspaceFolder,
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
        has_uncommitted_specs: true,
        disabled: false,
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
        },
    );
    assert_camel_case(
        "CommitFile",
        CommitFile {
            path: "src/main.rs".to_string(),
            status: "M".to_string(),
            additions: Some(12),
            deletions: Some(3),
        },
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

/// Every `QuotaStatus` and `DocumentWidth` variant, since a variant the
/// fixtures above omit is never serialized and so never checked.
#[test]
fn every_standalone_enum_variant_is_camel_case() {
    for status in [
        QuotaStatus::Disabled,
        QuotaStatus::Unauthenticated,
        QuotaStatus::Unavailable,
        QuotaStatus::Ok,
    ] {
        assert_camel_case("QuotaStatus", status);
    }
    for width in [
        DocumentWidth::Compact,
        DocumentWidth::Default,
        DocumentWidth::Wide,
        DocumentWidth::Full,
    ] {
        assert_camel_case("DocumentWidth", width);
    }
}
