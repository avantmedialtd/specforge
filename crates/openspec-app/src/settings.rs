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
    /// Favorited changes, keyed by position-independent change identity in the
    /// same node-id grammar the collapse sets persist: `repo:<rid>/lc:<name>`
    /// for repo-group changes, `flat:<uri>/change:<id>` for flat-workspace
    /// changes — never an instance-scoped id, so a favorite survives
    /// singleton↔multi-instance promotion.
    #[serde(default)]
    pub favorite_change_ids: Vec<String>,
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
    /// Re-scan cadence, in seconds, for the polling watcher used on WSL 9P
    /// shares (see `openspec-core`'s WSL support). Default 10s. Only consulted
    /// on Windows — WSL workspaces cannot occur elsewhere — but the field is
    /// stored on every platform so the settings file stays portable.
    #[serde(default = "default_wsl_poll_interval_secs")]
    pub wsl_poll_interval_secs: u64,
    /// Master switch for the opt-in Claude usage-quota status line. Off by
    /// default — an absent key loads as `false` via `#[serde(default)]` — so no
    /// OAuth token is read and no network request is made until the user opts
    /// in from Settings. See `crate::quota`.
    #[serde(default)]
    pub claude_quota_enabled: bool,
    /// How often (seconds) the quota poller refreshes while enabled. Default
    /// 60s (one minute); floored by the poller so a tiny value can't hammer the
    /// endpoint.
    #[serde(default = "default_claude_quota_refresh_secs")]
    pub claude_quota_refresh_secs: u64,
    /// Master switch for the opt-in ChatGPT usage-quota status line. Off by
    /// default — an absent key loads as `false` via `#[serde(default)]` — so
    /// no Codex CLI credential is read and no network request is made until
    /// the user opts in from Settings. A twin of `claude_quota_enabled`; see
    /// `crate::chatgpt_quota`.
    #[serde(default)]
    pub chatgpt_quota_enabled: bool,
    /// How often (seconds) the ChatGPT quota poller refreshes while enabled.
    /// Default 60s (one minute); floored by the poller so a tiny value can't
    /// hammer the endpoint.
    #[serde(default = "default_chatgpt_quota_refresh_secs")]
    pub chatgpt_quota_refresh_secs: u64,
    /// Optional embedded web UI. When enabled, the desktop app also serves the
    /// browser skin on `127.0.0.1:<port>` from the *same* `AppService` (so the
    /// web view mirrors live desktop state). Off by default; takes effect at
    /// launch. `#[serde(default)]` makes an absent block load as disabled.
    #[serde(default)]
    pub web: WebServerConfig,
}

/// Configuration for the optional embedded web server (the desktop app's
/// "serve the web UI" toggle). The bind address is always the loopback
/// interface; only the port is configurable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_web_port")]
    pub port: u16,
    /// Optional Tailscale Serve access. When enabled, the web server's
    /// trust-boundary guard also accepts the host's own tailnet (MagicDNS) name,
    /// so `tailscale serve` can proxy to the still-loopback-bound server. Off by
    /// default. See the `web-ui` capability's *Tailscale Serve Access*.
    #[serde(default)]
    pub tailscale: TailscaleConfig,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_web_port(),
            tailscale: TailscaleConfig::default(),
        }
    }
}

/// Tailscale Serve access settings. The bind address never changes — enabling
/// this only widens the request-authority allowlist to include this host's own
/// tailnet name (resolved from `name`, or discovered when absent).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleConfig {
    /// Whether to trust the host's tailnet name (off by default).
    #[serde(default)]
    pub enabled: bool,
    /// Manual MagicDNS-name override. When `None`, the name is discovered from
    /// local Tailscale state; when set, it takes precedence (and is the fallback
    /// when discovery is unavailable).
    #[serde(default)]
    pub name: Option<String>,
    /// When non-empty, a Tailscale-proxied request is accepted only if it carries
    /// a `Tailscale-User-Login` in this list. Empty = trust the whole tailnet.
    #[serde(default)]
    pub allowed_logins: Vec<String>,
}

fn default_web_port() -> u16 {
    4317
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            notifications_enabled: default_notifications_enabled(),
            collapsed_tree_node_ids: Vec::new(),
            expanded_tree_node_ids: Vec::new(),
            favorite_change_ids: Vec::new(),
            identity: IdentityConfig::default(),
            people: Vec::new(),
            wsl_poll_interval_secs: default_wsl_poll_interval_secs(),
            claude_quota_enabled: false,
            claude_quota_refresh_secs: default_claude_quota_refresh_secs(),
            chatgpt_quota_enabled: false,
            chatgpt_quota_refresh_secs: default_chatgpt_quota_refresh_secs(),
            web: WebServerConfig::default(),
        }
    }
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_wsl_poll_interval_secs() -> u64 {
    10
}

fn default_claude_quota_refresh_secs() -> u64 {
    60
}

fn default_chatgpt_quota_refresh_secs() -> u64 {
    60
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

    /// Apply a favorites delta — ids to star and ids to unstar — and return
    /// the merged list. A delta (rather than whole-list replacement) keeps one
    /// client's toggle from erasing favorites another client persisted in the
    /// meantime, and the save happens *before* the lock is released so two
    /// concurrent updates cannot reach disk out of order.
    pub fn update_favorite_change_ids(
        &self,
        add: Vec<String>,
        remove: Vec<String>,
    ) -> io::Result<Vec<String>> {
        let mut settings = self.settings.lock().unwrap();
        settings
            .favorite_change_ids
            .retain(|id| !remove.contains(id));
        for id in add {
            if !settings.favorite_change_ids.contains(&id) {
                settings.favorite_change_ids.push(id);
            }
        }
        let snapshot = settings.clone();
        self.save(&snapshot)?;
        drop(settings);
        Ok(snapshot.favorite_change_ids)
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

    /// Whether the opt-in Claude usage-quota status line is enabled (off by
    /// default). Read by the quota poller every tick so a toggle takes effect
    /// promptly without restarting it.
    pub fn claude_quota_enabled(&self) -> bool {
        self.settings.lock().unwrap().claude_quota_enabled
    }

    pub fn set_claude_quota_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.claude_quota_enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// The quota poll cadence (seconds); the poller floors this so a very small
    /// value can't hammer the endpoint.
    pub fn claude_quota_refresh_secs(&self) -> u64 {
        self.settings.lock().unwrap().claude_quota_refresh_secs
    }

    /// Whether the opt-in ChatGPT usage-quota status line is enabled (off by
    /// default). Read by the ChatGPT quota poller every tick so a toggle
    /// takes effect promptly without restarting it.
    pub fn chatgpt_quota_enabled(&self) -> bool {
        self.settings.lock().unwrap().chatgpt_quota_enabled
    }

    pub fn set_chatgpt_quota_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.chatgpt_quota_enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// The ChatGPT quota poll cadence (seconds); the poller floors this so a
    /// very small value can't hammer the endpoint.
    pub fn chatgpt_quota_refresh_secs(&self) -> u64 {
        self.settings.lock().unwrap().chatgpt_quota_refresh_secs
    }

    /// The embedded web-server configuration (enabled + loopback port). Read once
    /// at desktop launch to decide whether to start the server.
    pub fn web_config(&self) -> WebServerConfig {
        self.settings.lock().unwrap().web.clone()
    }

    /// Enable or disable the embedded web server. Persisted; takes effect at the
    /// next launch (the server is started once at startup).
    pub fn set_web_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.web.enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Set the embedded web server's loopback port. Persisted; takes effect at
    /// the next launch.
    pub fn set_web_port(&self, value: u16) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.web.port = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Enable or disable Tailscale Serve access (trusting the host's tailnet name
    /// in the web guard). Persisted; takes effect when the server next builds its
    /// router.
    pub fn set_web_tailscale_enabled(&self, value: bool) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.web.tailscale.enabled = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Set the manual Tailscale MagicDNS-name override (cleared to `None` when
    /// empty, restoring auto-discovery).
    pub fn set_web_tailscale_name(&self, value: Option<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.web.tailscale.name = value.filter(|s| !s.trim().is_empty());
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// Replace the Tailscale per-user login allow-list (empty = trust the whole
    /// tailnet).
    pub fn set_web_tailscale_allowed_logins(&self, value: Vec<String>) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.web.tailscale.allowed_logins = value
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorite_change_ids_delta_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        let merged = store
            .update_favorite_change_ids(
                vec![
                    "repo:/r/main/lc:add-dark-mode".to_string(),
                    "flat:/w/notes/change:web-ui-auth".to_string(),
                ],
                Vec::new(),
            )
            .unwrap();
        assert_eq!(merged.len(), 2);

        // A second delta merges instead of replacing: the add lands alongside
        // the stored entries and the remove deletes exactly its target.
        let merged = store
            .update_favorite_change_ids(
                vec!["repo:/r/main/lc:new-one".to_string()],
                vec!["flat:/w/notes/change:web-ui-auth".to_string()],
            )
            .unwrap();
        assert_eq!(
            merged,
            vec![
                "repo:/r/main/lc:add-dark-mode".to_string(),
                "repo:/r/main/lc:new-one".to_string(),
            ]
        );

        // Re-adding an existing id does not duplicate it.
        let merged = store
            .update_favorite_change_ids(
                vec!["repo:/r/main/lc:add-dark-mode".to_string()],
                Vec::new(),
            )
            .unwrap();
        assert_eq!(merged.len(), 2);

        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.snapshot().favorite_change_ids, merged);
    }

    #[test]
    fn pre_feature_settings_file_loads_with_empty_favorites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"notificationsEnabled":false,"collapsedTreeNodeIds":["repo:/r/main"]}"#,
        )
        .unwrap();

        let snapshot = SettingsStore::load(path).snapshot();
        assert_eq!(snapshot.favorite_change_ids, Vec::<String>::new());
        assert_eq!(snapshot.collapsed_tree_node_ids, vec!["repo:/r/main"]);
        assert!(!snapshot.notifications_enabled);
    }

    /// Every setter round-trips through disk and every getter reads back the
    /// set value on a freshly loaded store. This is the blanket assertion that
    /// keeps `cargo mutants` meaningful for the whole file now that its
    /// exclude_globs entry is gone — a mutant in any setter's assignment or
    /// any getter's return path changes what the reloaded store reports.
    #[test]
    fn every_setter_round_trips_and_every_getter_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        store.set_notifications_enabled(false).unwrap();
        store
            .set_collapsed_tree_node_ids(vec!["c1".to_string()])
            .unwrap();
        store
            .set_expanded_tree_node_ids(vec!["e1".to_string()])
            .unwrap();
        store.set_display_name(Some("Ada".to_string())).unwrap();
        let alias = Author {
            name: Some("ada".to_string()),
            email: Some("ada@example.com".to_string()),
        };
        store.set_identity_aliases(vec![alias.clone()]).unwrap();
        let person = Person {
            display_name: Some("Grace".to_string()),
            identities: vec![Author {
                name: Some("grace".to_string()),
                email: None,
            }],
        };
        store.set_people(vec![person.clone()]).unwrap();
        store.set_wsl_poll_interval_secs(42).unwrap();
        store.set_claude_quota_enabled(true).unwrap();
        store.set_chatgpt_quota_enabled(true).unwrap();
        store.set_web_enabled(true).unwrap();
        store.set_web_port(4444).unwrap();
        store.set_web_tailscale_enabled(true).unwrap();
        store
            .set_web_tailscale_name(Some("host.tail.net".to_string()))
            .unwrap();
        store
            .set_web_tailscale_allowed_logins(vec![" a@b ".to_string(), "  ".to_string()])
            .unwrap();

        let reloaded = SettingsStore::load(path);
        let snapshot = reloaded.snapshot();
        assert!(!snapshot.notifications_enabled);
        assert_eq!(snapshot.collapsed_tree_node_ids, vec!["c1"]);
        assert_eq!(snapshot.expanded_tree_node_ids, vec!["e1"]);
        let identity = reloaded.identity();
        assert_eq!(identity.display_name.as_deref(), Some("Ada"));
        assert_eq!(identity.aliases.len(), 1);
        assert_eq!(identity.aliases[0].email, alias.email);
        let people = reloaded.people();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].display_name, person.display_name);
        assert_eq!(people[0].identities.len(), 1);
        assert_eq!(reloaded.wsl_poll_interval_secs(), 42);
        assert!(reloaded.claude_quota_enabled());
        assert_eq!(reloaded.claude_quota_refresh_secs(), 60);
        assert!(reloaded.chatgpt_quota_enabled());
        assert_eq!(reloaded.chatgpt_quota_refresh_secs(), 60);
        let web = reloaded.web_config();
        assert!(web.enabled);
        assert_eq!(web.port, 4444);
        assert!(web.tailscale.enabled);
        assert_eq!(web.tailscale.name.as_deref(), Some("host.tail.net"));
        assert_eq!(web.tailscale.allowed_logins, vec!["a@b"]);
    }

    /// Empty-string normalisation: the clearing setters store `None`, and the
    /// tailscale name/logins trim their input.
    #[test]
    fn clearing_setters_normalise_empty_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        store.set_display_name(Some("  ".to_string())).unwrap();
        store.set_web_tailscale_name(Some(" ".to_string())).unwrap();

        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.identity().display_name, None);
        assert_eq!(reloaded.web_config().tailscale.name, None);
    }
    /// An existing settings file written by a version that had the gamification
    /// gate and the seasonal locker still loads: serde ignores the unknown
    /// `gamificationEnabled` / `season` keys (no `deny_unknown_fields` anywhere
    /// in the workspace), and the next write drops them. No migration runs.
    #[test]
    fn legacy_gamification_and_season_keys_are_ignored_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
              "notificationsEnabled": false,
              "gamificationEnabled": true,
              "season": {
                "unlocked": ["s1-t2-g1"],
                "equipped": "s1-t2-g1",
                "lastRecappedSeasonIndex": 24317
              },
              "wslPollIntervalSecs": 42
            }"#,
        )
        .unwrap();

        // Loads without error, and the surviving settings round-trip.
        let store = SettingsStore::load(path.clone());
        assert!(!store.snapshot().notifications_enabled);
        assert_eq!(store.wsl_poll_interval_secs(), 42);

        // The next write serialises the whole struct, so the orphaned keys go.
        store.set_wsl_poll_interval_secs(7).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("gamificationEnabled"),
            "legacy gamification key survived a write: {raw}"
        );
        assert!(
            !raw.contains("season"),
            "legacy season block survived a write: {raw}"
        );
        assert_eq!(SettingsStore::load(path).wsl_poll_interval_secs(), 7);
    }
}
