//! Per-top-level-row presentation overrides (display name + tint colour),
//! persisted alongside the workspace registry. Identity is decoupled from
//! the registry on purpose: a single repository group has multiple registered
//! workspace paths but renders as one top-level row, so its display name and
//! tint can't live on any individual `WorkspaceFolder`.
//!
//! The store is independent of the registry — the Tauri shell mediates between
//! the two so cascade-removal on unregister stays explicit.

use crate::types::PaletteColor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fmt, fs, io};
use thiserror::Error;

/// The two kinds of top-level row that can have presentation overrides.
/// Serialises to/from a stringly form: `"flat:<path>"` for a non-git workspace
/// and `"repo:<path>"` for a repository group keyed by its canonical git
/// common directory. The string form keeps the on-disk file human-inspectable
/// and survives round-tripping through Tauri command arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PresentationKey {
    Flat(PathBuf),
    Repo(PathBuf),
}

impl PresentationKey {
    pub fn flat(path: impl Into<PathBuf>) -> Self {
        Self::Flat(path.into())
    }

    pub fn repo(path: impl Into<PathBuf>) -> Self {
        Self::Repo(path.into())
    }
}

impl fmt::Display for PresentationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flat(p) => write!(f, "flat:{}", p.display()),
            Self::Repo(p) => write!(f, "repo:{}", p.display()),
        }
    }
}

impl FromStr for PresentationKey {
    type Err = PresentationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix("flat:") {
            Ok(Self::Flat(PathBuf::from(rest)))
        } else if let Some(rest) = s.strip_prefix("repo:") {
            Ok(Self::Repo(PathBuf::from(rest)))
        } else {
            Err(PresentationError::InvalidKey(s.to_string()))
        }
    }
}

impl Serialize for PresentationKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PresentationKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresentationEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<PaletteColor>,
}

impl PresentationEntry {
    fn is_empty(&self) -> bool {
        self.display_name.is_none() && self.color.is_none()
    }
}

#[derive(Debug, Error)]
pub enum PresentationError {
    #[error("invalid presentation key: {0}")]
    InvalidKey(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    entries: HashMap<String, PresentationEntry>,
}

/// In-memory presentation store with JSON persistence at `config_path`. Empty
/// entries (both fields `None`) are pruned on save so the file does not
/// accumulate dead keys.
#[derive(Debug)]
pub struct WorkspacePresentationStore {
    config_path: PathBuf,
    entries: HashMap<PresentationKey, PresentationEntry>,
}

impl WorkspacePresentationStore {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            entries: HashMap::new(),
        }
    }

    /// Loads the store from `config_path`. A missing file yields an empty
    /// store; a corrupt or unparseable file is reported as `InvalidData`.
    pub fn load(config_path: PathBuf) -> io::Result<Self> {
        let mut entries: HashMap<PresentationKey, PresentationEntry> = HashMap::new();
        if config_path.exists() {
            let raw = fs::read_to_string(&config_path)?;
            let config: ConfigFile = serde_json::from_str(&raw)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for (k, v) in config.entries {
                if let Ok(key) = k.parse::<PresentationKey>() {
                    if !v.is_empty() {
                        entries.insert(key, v);
                    }
                }
            }
        }
        Ok(Self {
            config_path,
            entries,
        })
    }

    /// Look up the entry for `key`, if any.
    pub fn get(&self, key: &PresentationKey) -> Option<&PresentationEntry> {
        self.entries.get(key)
    }

    /// Look up the entry's `(display_name, color)` pair, returning `(None, None)`
    /// when no entry exists. Convenience for IPC join sites.
    pub fn lookup(&self, key: &PresentationKey) -> (Option<String>, Option<PaletteColor>) {
        match self.entries.get(key) {
            Some(e) => (e.display_name.clone(), e.color),
            None => (None, None),
        }
    }

    /// Upserts the presentation entry for `key`. An empty-string display name
    /// is normalised to `None` so clearing the field falls back to the
    /// basename-derived default. If both fields end up `None`, the entry is
    /// removed instead of stored. Persists to disk on success.
    pub fn set(
        &mut self,
        key: PresentationKey,
        display_name: Option<String>,
        color: Option<PaletteColor>,
    ) -> Result<(), PresentationError> {
        let normalised_name = display_name.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        if normalised_name.is_none() && color.is_none() {
            self.entries.remove(&key);
        } else {
            self.entries.insert(
                key,
                PresentationEntry {
                    display_name: normalised_name,
                    color,
                },
            );
        }
        self.save()?;
        Ok(())
    }

    /// Removes the entry for `key`, if present. Persists to disk if anything
    /// changed. Used by the shell's `unregister_workspace` to cascade-clean
    /// presentation entries when the underlying registration is removed.
    pub fn remove(&mut self, key: &PresentationKey) -> Result<(), PresentationError> {
        if self.entries.remove(key).is_some() {
            self.save()?;
        }
        Ok(())
    }

    /// Number of currently-stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut serialised: HashMap<String, PresentationEntry> = HashMap::new();
        for (k, v) in &self.entries {
            if !v.is_empty() {
                serialised.insert(k.to_string(), v.clone());
            }
        }
        let config = ConfigFile {
            entries: serialised,
        };
        let raw = serde_json::to_string_pretty(&config)?;
        fs::write(&self.config_path, raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn key_round_trips_through_string() {
        let flat = PresentationKey::flat("/Users/x/foo");
        let repo = PresentationKey::repo("/Users/x/r/.git");
        assert_eq!(flat.to_string(), "flat:/Users/x/foo");
        assert_eq!(repo.to_string(), "repo:/Users/x/r/.git");
        assert_eq!(
            "flat:/Users/x/foo".parse::<PresentationKey>().unwrap(),
            flat
        );
        assert_eq!(
            "repo:/Users/x/r/.git".parse::<PresentationKey>().unwrap(),
            repo
        );
        assert!("invalid".parse::<PresentationKey>().is_err());
    }

    #[test]
    fn set_and_get_round_trip_through_disk() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("presentation.json");
        let key = PresentationKey::flat("/ws");
        {
            let mut store = WorkspacePresentationStore::new(path.clone());
            store
                .set(key.clone(), Some("My Workspace".into()), Some(PaletteColor::Teal))
                .unwrap();
        }
        let store = WorkspacePresentationStore::load(path).unwrap();
        let entry = store.get(&key).unwrap();
        assert_eq!(entry.display_name.as_deref(), Some("My Workspace"));
        assert_eq!(entry.color, Some(PaletteColor::Teal));
    }

    #[test]
    fn empty_display_name_is_normalised_to_none() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("presentation.json");
        let mut store = WorkspacePresentationStore::new(path);
        let key = PresentationKey::flat("/ws");
        store
            .set(key.clone(), Some("".into()), Some(PaletteColor::Rose))
            .unwrap();
        let entry = store.get(&key).unwrap();
        assert_eq!(entry.display_name, None);
        assert_eq!(entry.color, Some(PaletteColor::Rose));

        // Whitespace-only is also normalised.
        store
            .set(key.clone(), Some("   ".into()), Some(PaletteColor::Rose))
            .unwrap();
        assert_eq!(store.get(&key).unwrap().display_name, None);
    }

    #[test]
    fn setting_both_to_none_removes_the_entry() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("presentation.json");
        let mut store = WorkspacePresentationStore::new(path);
        let key = PresentationKey::flat("/ws");
        store
            .set(key.clone(), Some("Name".into()), Some(PaletteColor::Blue))
            .unwrap();
        assert!(store.get(&key).is_some());
        store.set(key.clone(), None, None).unwrap();
        assert!(store.get(&key).is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn missing_file_loads_as_empty_store() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("missing.json");
        let store = WorkspacePresentationStore::load(path).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn invalid_color_is_rejected_at_deserialisation() {
        let raw = r#"{"entries":{"flat:/ws":{"color":"chartreuse"}}}"#;
        let parsed: Result<ConfigFile, _> = serde_json::from_str(raw);
        assert!(parsed.is_err(), "non-palette colour must fail to parse");
    }

    #[test]
    fn invalid_key_is_skipped_at_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("p.json");
        std::fs::write(
            &path,
            r#"{"entries":{"not-a-key":{"displayName":"X","color":"teal"}}}"#,
        )
        .unwrap();
        let store = WorkspacePresentationStore::load(path).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn remove_clears_entries_for_both_key_kinds() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("p.json");
        let mut store = WorkspacePresentationStore::new(path);
        let flat = PresentationKey::flat("/ws");
        let repo = PresentationKey::repo("/r/.git");
        store
            .set(flat.clone(), Some("F".into()), Some(PaletteColor::Indigo))
            .unwrap();
        store
            .set(repo.clone(), Some("R".into()), Some(PaletteColor::Amber))
            .unwrap();
        assert_eq!(store.len(), 2);
        store.remove(&flat).unwrap();
        assert!(store.get(&flat).is_none());
        assert!(store.get(&repo).is_some());
        store.remove(&repo).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn lookup_returns_none_pair_for_missing_key() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("p.json");
        let store = WorkspacePresentationStore::new(path);
        let key = PresentationKey::flat("/ws");
        assert_eq!(store.lookup(&key), (None, None));
    }
}
