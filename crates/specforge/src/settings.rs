use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    #[serde(default)]
    pub collapsed_tree_node_ids: Vec<String>,
    #[serde(default)]
    pub expanded_tree_node_ids: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            notifications_enabled: default_notifications_enabled(),
            collapsed_tree_node_ids: Vec::new(),
            expanded_tree_node_ids: Vec::new(),
        }
    }
}

fn default_notifications_enabled() -> bool {
    true
}

/// File-backed app settings. Launch-on-login is **not** stored here — it
/// lives in the OS via the autostart plugin and is queried each time.
pub struct SettingsStore {
    path: PathBuf,
    settings: Mutex<AppSettings>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let settings = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            settings: Mutex::new(settings),
        }
    }

    pub fn snapshot(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_notifications_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.notifications_enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    pub fn set_collapsed_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.collapsed_tree_node_ids = ids;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    pub fn set_expanded_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.expanded_tree_node_ids = ids;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    fn save(&self, settings: &AppSettings) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(settings)?;
        fs::write(&self.path, raw)
    }
}
