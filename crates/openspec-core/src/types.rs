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

/// A workspace as returned from `WorkspaceRegistry::list`, including a flag
/// indicating whether the folder still exists on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredWorkspace {
    pub uri: PathBuf,
    pub name: String,
    pub is_missing: bool,
}

impl RegisteredWorkspace {
    pub(crate) fn from_folder(folder: &WorkspaceFolder) -> Self {
        Self {
            uri: folder.uri.clone(),
            name: folder.name.clone(),
            is_missing: !is_dir(&folder.uri),
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
