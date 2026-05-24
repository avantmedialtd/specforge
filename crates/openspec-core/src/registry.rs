use crate::types::{RegisteredWorkspace, WorkspaceFolder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("not an OpenSpec workspace (no `openspec/` subdirectory): {0}")]
    NotAnOpenSpecWorkspace(PathBuf),
    #[error("workspace already registered: {0}")]
    AlreadyRegistered(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    workspaces: Vec<WorkspaceFolder>,
}

/// In-memory store of registered workspaces, with disk persistence backed
/// by a JSON file at `config_path`.
#[derive(Debug)]
pub struct WorkspaceRegistry {
    config_path: PathBuf,
    workspaces: HashMap<PathBuf, WorkspaceFolder>,
}

impl WorkspaceRegistry {
    /// Creates a registry tied to `config_path` without attempting to read it.
    /// Use `load` to populate from an existing config file.
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            workspaces: HashMap::new(),
        }
    }

    /// Loads the registry from `config_path`. A missing file is treated as
    /// an empty registry; a corrupt file is reported as `InvalidData`.
    pub fn load(config_path: PathBuf) -> io::Result<Self> {
        let workspaces = if config_path.exists() {
            let raw = fs::read_to_string(&config_path)?;
            let config: ConfigFile = serde_json::from_str(&raw)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            config
                .workspaces
                .into_iter()
                .map(|w| (w.uri.clone(), w))
                .collect()
        } else {
            HashMap::new()
        };
        Ok(Self {
            config_path,
            workspaces,
        })
    }

    /// Validates `path` and adds it to the registry. Returns the canonicalised
    /// `WorkspaceFolder` on success. Persists to disk on success.
    ///
    /// Validation:
    /// - the path must exist
    /// - it must be a directory
    /// - it must contain an `openspec/` subdirectory
    /// - it must not already be registered (compared by canonical path)
    pub fn register(&mut self, path: PathBuf) -> Result<WorkspaceFolder, RegistrationError> {
        if !path.exists() {
            return Err(RegistrationError::PathNotFound(path));
        }

        let canonical = path.canonicalize()?;

        if !canonical.is_dir() {
            return Err(RegistrationError::NotADirectory(canonical));
        }

        if !canonical.join("openspec").is_dir() {
            return Err(RegistrationError::NotAnOpenSpecWorkspace(canonical));
        }

        if self.workspaces.contains_key(&canonical) {
            return Err(RegistrationError::AlreadyRegistered(canonical));
        }

        let folder = WorkspaceFolder::from_path(canonical.clone());
        self.workspaces.insert(canonical, folder.clone());
        self.save()?;
        Ok(folder)
    }

    /// Removes the registered workspace at `path`, if any. Returns `true` if
    /// something was removed. Persists to disk when a change is made.
    pub fn unregister(&mut self, path: &Path) -> io::Result<bool> {
        let removed = self.workspaces.remove(path).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Returns the current registered workspaces, alphabetised by name, with
    /// `is_missing` recomputed against the current filesystem state.
    pub fn list(&self) -> Vec<RegisteredWorkspace> {
        let mut items: Vec<RegisteredWorkspace> = self
            .workspaces
            .values()
            .map(RegisteredWorkspace::from_folder)
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then(a.uri.cmp(&b.uri)));
        items
    }

    /// Number of registered workspaces.
    pub fn len(&self) -> usize {
        self.workspaces.len()
    }

    /// Returns the underlying `WorkspaceFolder` entries, useful for callers
    /// (the Tauri shell) that need to drive watchers on startup. Order is
    /// unspecified; sort at the caller if it matters.
    pub fn folders(&self) -> Vec<WorkspaceFolder> {
        self.workspaces.values().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = ConfigFile {
            workspaces: self.workspaces.values().cloned().collect(),
        };
        let raw = serde_json::to_string_pretty(&config)?;
        fs::write(&self.config_path, raw)
    }
}
