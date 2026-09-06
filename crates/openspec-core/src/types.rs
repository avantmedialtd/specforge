use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A registered workspace folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolder {
    /// Canonical absolute path to the workspace root.
    pub uri: PathBuf,
    /// Human-readable name, defaulting to the path's final component.
    pub name: String,
}

impl WorkspaceFolder {
    /// Builds a `WorkspaceFolder` from a path, deriving the display name from
    /// the path's final component. Falls back to the full path string if no
    /// final component is available.
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.display().to_string());
        Self { uri: path, name }
    }
}

/// Curated tint palette for top-level workspace/repo rows. Serialised as
/// kebab-case strings on disk and across the IPC boundary; any value outside
/// this enum is rejected by the presentation store.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PaletteColor {
    Indigo,
    Blue,
    Teal,
    Green,
    Amber,
    Orange,
    Rose,
    Purple,
}

/// A workspace as returned from `WorkspaceRegistry::list`. Carries the
/// basename-derived default name and a missing-on-disk flag. The optional
/// `display_name`, `color`, and `repo_id` fields are populated by the IPC
/// layer; `None` for `display_name`/`color` means render with no override,
/// and `repo_id` tells the frontend which presentation key (flat vs repo)
/// to send when editing this row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredWorkspace {
    pub uri: PathBuf,
    pub name: String,
    pub is_missing: bool,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub color: Option<PaletteColor>,
    /// Canonical path to the workspace's git common directory if it lives
    /// inside a repository; `None` for flat workspaces. Lets the frontend
    /// decide whether to address the per-workspace or per-repo presentation
    /// key when editing this row.
    #[serde(default)]
    pub repo_id: Option<PathBuf>,
    /// True when the user has parked this row from the Settings view. Unlike the
    /// tree pane's aggregated view — which omits disabled rows entirely — the
    /// listing keeps them and flags them, because Settings is where the toggle
    /// that brings them back lives.
    #[serde(default)]
    pub disabled: bool,
}

impl RegisteredWorkspace {
    pub fn from_folder(folder: &WorkspaceFolder) -> Self {
        Self {
            uri: folder.uri.clone(),
            name: folder.name.clone(),
            is_missing: !is_dir(&folder.uri),
            display_name: None,
            color: None,
            repo_id: None,
            disabled: false,
        }
    }
}

fn is_dir(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// A single task line parsed from `tasks.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub text: String,
    pub completed: bool,
    /// Leading-whitespace count on the source line.
    pub indent: usize,
    /// 1-indexed line number in the source file.
    pub line_number: usize,
}

/// A `## Heading` section within `tasks.md` and the tasks beneath it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    pub tasks: Vec<Task>,
}

/// Which of the four expected artifacts are present in a change directory.
/// `specs` holds the names of capability subdirectories that contain a
/// `spec.md` file, in sorted order.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactStatus {
    pub proposal: bool,
    pub specs: Vec<String>,
    pub design: bool,
    pub tasks: bool,
}

/// Aggregated state of a single change directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeData {
    pub change_id: String,
    pub title: Option<String>,
    pub sections: Vec<Section>,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub artifacts: ArtifactStatus,
    pub workspace: WorkspaceFolder,
}

/// Lightweight summary of one archived change, for the Archive browser. Built
/// from the archive directory name (`<YYYY-MM-DD>-<id>`) plus a heading-only
/// read of `proposal.md` — never a full change parse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedChangeSummary {
    /// Logical change id — the directory name with any `YYYY-MM-DD-` prefix stripped.
    pub id: String,
    /// Archive date `YYYY-MM-DD` from the directory-name prefix; `None` for a
    /// legacy archive directory with no date prefix.
    pub date: Option<String>,
    /// Title from the change's `proposal.md` heading, if present.
    pub title: Option<String>,
    /// The directory's own name under `openspec/changes/archive/`, verbatim.
    ///
    /// Carried rather than re-derived from `id` + `date`: the date strip is a
    /// single anchored match, so a change whose own id begins with a
    /// date-shaped prefix (`2026-06-04-2026-06-05-add-thing`) only round-trips
    /// when it is stripped exactly once. Every consumer that has to *address*
    /// this change on disk reads this field instead of reassembling one.
    pub dir_name: String,
}

/// Which top-level row an archive listing is scoped to — a repository group
/// (every tracked worktree of it) or a single flat, non-git workspace. Mirrors
/// the two shapes of [`crate::repo_view::WorkspaceView`], and is tagged the
/// same way so the frontend sends one discriminated union.
// `rename_all` on an enum renames the VARIANTS (`Repo` -> `repo`); it does not
// touch the fields inside a struct variant. `rename_all_fields` is what carries
// `repo_id` -> `repoId`, and without it the frontend's `{kind:"repo",repoId}`
// is rejected at the wire with `missing field repo_id` — a failure neither
// `cargo test` (which builds the enum in Rust) nor `tsc` can see.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ArchiveScope {
    /// A repository, addressed by the canonical path of its git common dir —
    /// the identity that is stable across its worktrees.
    Repo { repo_id: PathBuf },
    /// A single non-git workspace folder, addressed by its own path.
    Flat { workspace: PathBuf },
}

/// One worktree's copy of an archived change, as pooled into an
/// [`ArchivedChangeRow`]. The `(worktree_path, archive_dir)` pair — not the
/// logical id — is what addresses a read.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedChangeCopy {
    /// Canonical path of the tracked worktree holding this copy.
    pub worktree_path: PathBuf,
    /// This copy's archive directory name within that worktree.
    pub archive_dir: String,
    /// This copy's own archive date, from its directory-name prefix. Two
    /// worktrees can archive one change on different days, so a copy's date
    /// need not be the row's.
    pub date: Option<String>,
}

/// One logical archived change, pooled across a top-level row's tracked
/// worktrees and de-duplicated on the bare logical id (`archive-browser`:
/// *Union Archive Listing Across a Repository's Worktrees*).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedChangeRow {
    /// The bare logical change id every copy shares.
    pub id: String,
    /// Display/ordering date: the NEWEST date across the row's copies, so a
    /// change re-archived later in a second worktree sorts by its most recent
    /// archival. `None` only when every copy is an un-dated legacy directory.
    pub date: Option<String>,
    /// Title from the first copy (in `copies` order) that has one.
    pub title: Option<String>,
    /// Every copy this row collapsed, in a deterministic total order.
    pub copies: Vec<ArchivedChangeCopy>,
}

#[cfg(test)]
mod wire_shape_tests {
    use super::*;

    /// The frontend hand-mirrors these shapes in `src/types.ts`, so the wire is
    /// the only place the two can disagree — and neither `cargo test` (which
    /// builds the value in Rust) nor `tsc` (which checks the mirror against
    /// itself) can see a mismatch. These assert against the literal JSON that
    /// `src/api.ts` sends and reads.
    ///
    /// Regression: `#[serde(rename_all)]` on an ENUM renames its variants, not
    /// the fields of a struct variant. Without `rename_all_fields`, `repo_id`
    /// stayed snake_case while the frontend sent `repoId`, so every
    /// repository-scoped archive listing failed at runtime with
    /// `missing field repo_id` — with the whole suite green.
    #[test]
    fn archive_scope_repo_deserializes_the_camel_case_json_the_frontend_sends() {
        let scope: ArchiveScope = serde_json::from_str(r#"{"kind":"repo","repoId":"/r/.git"}"#)
            .expect("the frontend's JSON must parse");
        assert_eq!(
            scope,
            ArchiveScope::Repo {
                repo_id: PathBuf::from("/r/.git")
            }
        );
    }

    #[test]
    fn archive_scope_flat_deserializes_the_camel_case_json_the_frontend_sends() {
        let scope: ArchiveScope = serde_json::from_str(r#"{"kind":"flat","workspace":"/w"}"#)
            .expect("the frontend's JSON must parse");
        assert_eq!(
            scope,
            ArchiveScope::Flat {
                workspace: PathBuf::from("/w")
            }
        );
    }

    /// Serialization is the other direction of the same contract: the key the
    /// frontend reads must be the key Rust writes.
    #[test]
    fn archive_scope_repo_serializes_the_key_the_frontend_reads() {
        let v = serde_json::to_value(ArchiveScope::Repo {
            repo_id: PathBuf::from("/r/.git"),
        })
        .unwrap();
        assert_eq!(v["kind"], "repo");
        assert_eq!(v["repoId"], "/r/.git");
        assert!(v.get("repo_id").is_none(), "snake_case key must not appear");
    }

    /// `(worktreePath, archiveDir)` is the pair that addresses a read, so a
    /// casing slip there breaks the reader rather than the listing.
    #[test]
    fn archived_change_row_serializes_camel_case_copy_keys() {
        let v = serde_json::to_value(ArchivedChangeRow {
            id: "add-thing".into(),
            date: Some("2026-09-06".into()),
            title: None,
            copies: vec![ArchivedChangeCopy {
                worktree_path: PathBuf::from("/r/wt"),
                archive_dir: "2026-09-06-add-thing".into(),
                date: Some("2026-09-06".into()),
            }],
        })
        .unwrap();
        assert_eq!(v["copies"][0]["worktreePath"], "/r/wt");
        assert_eq!(v["copies"][0]["archiveDir"], "2026-09-06-add-thing");
        assert!(v["copies"][0].get("worktree_path").is_none());
    }
}
