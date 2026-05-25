use crate::types::ChangeData;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// In-memory cache of parsed OpenSpec state per registered workspace.
///
/// Keys are canonical workspace paths. Values are the workspace's
/// non-archived `ChangeData` entries, sorted in the order returned by
/// [`crate::parse_all_changes`].
///
/// The cache is a passive data store — it does not watch the filesystem.
/// Mutation happens via the watcher in [`crate::watcher`].
#[derive(Debug, Default, Clone)]
pub struct WorkspaceCache {
    inner: HashMap<PathBuf, Vec<ChangeData>>,
}

impl WorkspaceCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace the cached changes for a workspace.
    pub fn insert(&mut self, workspace: PathBuf, changes: Vec<ChangeData>) {
        self.inner.insert(workspace, changes);
    }

    /// Drop the cache entry for a workspace.
    pub fn remove(&mut self, workspace: &Path) -> Option<Vec<ChangeData>> {
        self.inner.remove(workspace)
    }

    /// Returns the cached changes for a workspace, or an empty slice if
    /// the workspace has no entry.
    pub fn changes_for(&self, workspace: &Path) -> &[ChangeData] {
        self.inner
            .get(workspace)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Total count of non-archived changes across all cached workspaces.
    /// Drives the tray badge.
    pub fn total_active_count(&self) -> usize {
        self.inner.values().map(Vec::len).sum()
    }

    /// Whether any cached change in any workspace has at least one capability
    /// spec delta (non-empty `ArtifactStatus.specs`). Drives the tray glyph
    /// variant selection.
    pub fn any_change_touches_specs(&self) -> bool {
        self.inner
            .values()
            .flatten()
            .any(|c| !c.artifacts.specs.is_empty())
    }

    /// Returns a clone of the full cache contents.
    pub fn snapshot(&self) -> HashMap<PathBuf, Vec<ChangeData>> {
        self.inner.clone()
    }

    /// Number of workspaces with cache entries.
    pub fn workspace_count(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
