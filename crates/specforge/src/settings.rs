use openspec_core::{Author, IdentityConfig, Person};
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
    /// Master switch for the gamified progress layer (seasons, streak, heatmap,
    /// milestones, leaderboard, badge finishes, celebrations). Off by default —
    /// an absent key loads as `false` via `#[serde(default)]` — so the Dashboard
    /// shows only its analytics until the user opts in from Settings.
    #[serde(default)]
    pub gamification_enabled: bool,
    /// The developer-identity configuration (canonical display name + the
    /// aliases that resolve to "me"). Persisted alongside the other settings;
    /// `#[serde(default)]` makes an absent config load as empty.
    #[serde(default)]
    pub identity: IdentityConfig,
    /// The contributor roster: named people other than "me", each folding one or
    /// more git identities, used to name and merge authors on the per-author
    /// leaderboard. Presentation only; `#[serde(default)]` makes an absent roster
    /// load empty, so existing settings need no migration.
    #[serde(default)]
    pub people: Vec<Person>,
    /// Seasonal battle-pass state: the treatment locker, the equipped finish,
    /// and the rollover bookmark. The *only* new persisted state seasons add —
    /// everything else (score, band/tier, objectives, recaps) is derived from
    /// the activity log. `#[serde(default)]` makes an absent block load empty.
    #[serde(default)]
    pub season: SeasonState,
    /// Re-scan cadence, in seconds, for the polling watcher used on WSL 9P
    /// shares (see `openspec-core`'s WSL support). Default 10s. Only consulted
    /// on Windows — WSL workspaces cannot occur elsewhere — but the field is
    /// stored on every platform so the settings file stays portable.
    #[serde(default = "default_wsl_poll_interval_secs")]
    pub wsl_poll_interval_secs: u64,
}

/// Persisted seasonal state. `unlocked` is the monotonic set of unlocked
/// treatment ids (`s{season}-t{tier}-g{gen}`); `equipped` is the id of the
/// finish currently worn over earned badges; `last_recapped_season_index` is
/// the rollover bookmark that keeps a recap from being surfaced twice.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonState {
    #[serde(default)]
    pub unlocked: Vec<String>,
    #[serde(default)]
    pub equipped: Option<String>,
    #[serde(default)]
    pub last_recapped_season_index: Option<i64>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            notifications_enabled: default_notifications_enabled(),
            collapsed_tree_node_ids: Vec::new(),
            expanded_tree_node_ids: Vec::new(),
            gamification_enabled: false,
            identity: IdentityConfig::default(),
            people: Vec::new(),
            season: SeasonState::default(),
            wsl_poll_interval_secs: default_wsl_poll_interval_secs(),
        }
    }
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_wsl_poll_interval_secs() -> u64 {
    10
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

    /// The current developer-identity configuration.
    pub fn identity(&self) -> IdentityConfig {
        self.settings.lock().unwrap().identity.clone()
    }

    /// Replace the whole identity configuration (used by first-run seeding).
    pub fn set_identity(&self, identity: IdentityConfig) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.identity = identity;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Set the canonical display name (cleared to `None` when empty).
    pub fn set_display_name(&self, name: Option<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.identity.display_name = name.filter(|s| !s.trim().is_empty());
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Replace the set of alias identities that resolve to "me".
    pub fn set_identity_aliases(&self, aliases: Vec<Author>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.identity.aliases = aliases;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// The current contributor roster (named people other than "me").
    pub fn people(&self) -> Vec<Person> {
        self.settings.lock().unwrap().people.clone()
    }

    /// Replace the whole contributor roster.
    pub fn set_people(&self, people: Vec<Person>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.people = people;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Re-scan cadence (seconds) for the WSL polling watcher. Default 10s.
    /// Only read on Windows (the apply path and the get command are gated), so
    /// it is dead code elsewhere.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn wsl_poll_interval_secs(&self) -> u64 {
        self.settings.lock().unwrap().wsl_poll_interval_secs
    }

    /// Set the WSL polling-watcher re-scan cadence (seconds) and persist it.
    pub fn set_wsl_poll_interval_secs(&self, value: u64) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.wsl_poll_interval_secs = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Whether the gamified progress layer is enabled (off by default).
    pub fn gamification_enabled(&self) -> bool {
        self.settings.lock().unwrap().gamification_enabled
    }

    pub fn set_gamification_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.gamification_enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// The current seasonal state (locker + equipped + rollover bookmark).
    pub fn season_state(&self) -> SeasonState {
        self.settings.lock().unwrap().season.clone()
    }

    /// Add `ids` to the treatment locker, never revoking — unlocking is
    /// monotonic. Returns true when at least one id was newly added (so callers
    /// can decide whether a live tier-up celebration is warranted). Persists
    /// only when something changed.
    pub fn unlock_treatments(&self, ids: Vec<String>) -> io::Result<bool> {
        let mut settings = self.settings.lock().unwrap();
        let mut added = false;
        for id in ids {
            if !settings.season.unlocked.contains(&id) {
                settings.season.unlocked.push(id);
                added = true;
            }
        }
        if !added {
            return Ok(false);
        }
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)?;
        Ok(true)
    }

    /// Equip a treatment by id (or clear with `None`).
    pub fn set_equipped_treatment(&self, id: Option<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.season.equipped = id.filter(|s| !s.trim().is_empty());
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Advance the rollover bookmark so a recap is not surfaced twice.
    pub fn set_last_recapped_season(&self, index: i64) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.season.last_recapped_season_index = Some(index);
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
