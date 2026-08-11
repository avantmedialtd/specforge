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
}
