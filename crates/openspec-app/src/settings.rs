use openspec_core::{Author, IdentityConfig};
use serde::{Deserialize, Deserializer, Serialize};
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
    /// The size every reader window opens at — **one** remembered geometry for
    /// all of them, not one per document.
    ///
    /// Per-document memory is what the platform would give for free (window
    /// state is persisted per window label, and reader labels are derived from
    /// the document), and it is the reason this is stored here instead: that
    /// file would then accrue one entry per document ever opened, keyed by an
    /// opaque hash, with nothing that ever removes them. One shared size is
    /// bounded, readable, and what document applications do for new windows.
    #[serde(default)]
    pub reader_window: ReaderWindowGeometry,
    /// The reading width every markdown surface renders at — **one**
    /// application-wide value, not one per document and not one per window.
    ///
    /// The same reasoning `reader_window` gives above: per-document memory is
    /// what the platform would hand us for free, and it is exactly what makes
    /// the store unbounded — one entry per document ever opened, keyed by an
    /// opaque identifier, with nothing that ever removes them. A reading width
    /// is a preference about how the reader likes to read, not a property of
    /// any one document, so it is stored once.
    #[serde(default)]
    pub document_width: DocumentWidth,
}

/// The reading width of the markdown content column, as a rung on a fixed
/// ladder rather than a free measurement.
///
/// Each rung carries both tiers of the two-tier column (`visual-identity`:
/// *Markdown Body Adopts the Type System*) — the object column and the prose
/// measure — and the widths themselves live in the stylesheet, keyed off this
/// value. Nothing in Rust needs the pixel figures, so nothing here has to be
/// kept in step with them.
///
/// Tolerating an unrecognised stored value is load-bearing rather than tidy.
/// [`SettingsStore::load`] parses the whole file and falls back to
/// `AppSettings::default()` when that parse fails, so a strict enum meeting a
/// value written by a newer version — or edited by hand — would not report an
/// unknown reading width. It would silently reset favourites, the developer
/// identity, tree collapse state, the web-server configuration and the
/// reader-window geometry along with it.
///
/// [`Deserialize`] is therefore written by hand. `#[serde(other)]` is the
/// obvious way to say this and does not apply here: serde permits it only on a
/// unit variant of an internally or adjacently tagged enum, and this is an
/// ordinary externally tagged one stored as a bare string. The hand-written
/// impl also widens the guarantee past what `other` would have given — a value
/// that is not a string at all, which is exactly what hand-editing produces,
/// lands on the default rung instead of failing the parse.
///
/// [`Serialize`] stays derived, so the wire names are declared once by
/// `rename_all`. `every_rung_round_trips_under_its_wire_name` is what keeps the
/// two halves in agreement.
///
/// **The tolerance is read-side only, and an unknown value does not survive a
/// write.** [`SettingsStore::save`] serializes the whole struct, so the moment
/// any setter runs — including one for an unrelated preference, like starring a
/// change — the folded `Default` is written back over the value on disk and the
/// original is gone. This matters exactly once: a reader who selected a rung on
/// a newer build, then opened an older one, loses that selection permanently
/// rather than temporarily.
///
/// That is accepted rather than overlooked. Preserving it would mean carrying
/// the raw string in an `Unknown(String)` variant, which then has to be matched
/// everywhere a rung is used and mirrored into the TypeScript union — real
/// complexity across the whole contract, to protect a *preference* whose
/// degraded state is a legible default the reader can simply re-pick. Losing a
/// preference is not losing data; the neighbouring settings, which ARE data, is
/// what the tolerance exists to protect.
/// `an_unknown_rung_is_rewritten_to_the_default_by_any_setter` pins this so it
/// stays a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentWidth {
    /// A tight measure — roughly the 66 characters conventionally called ideal.
    Compact,
    /// The rendering the reading surface had before the ladder existed.
    #[default]
    Default,
    /// Wider than comfortable for prose, deliberately chosen.
    Wide,
    /// Objects take the surface; prose stays bounded. See the capability's
    /// *The Widest Preset Fills the Surface and Still Bounds Prose*.
    Full,
}

impl<'de> Deserialize<'de> for DocumentWidth {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Any JSON value is accepted and anything unrecognised becomes the
        // default rung; see the type's note for why this cannot be an error.
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match value.as_str() {
            Some("compact") => Self::Compact,
            Some("wide") => Self::Wide,
            Some("full") => Self::Full,
            _ => Self::Default,
        })
    }
}

/// The shared reader-window size. Position is deliberately absent: a new reader
/// cascades from the topmost visible one rather than reopening where some
/// earlier window happened to sit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderWindowGeometry {
    #[serde(default = "default_reader_width")]
    pub width: f64,
    #[serde(default = "default_reader_height")]
    pub height: f64,
}

impl Default for ReaderWindowGeometry {
    fn default() -> Self {
        Self {
            width: default_reader_width(),
            height: default_reader_height(),
        }
    }
}

/// Narrower than the main window: a reader holds one column of prose, with no
/// tree or rail beside it.
fn default_reader_width() -> f64 {
    720.0
}

fn default_reader_height() -> f64 {
    820.0
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
            wsl_poll_interval_secs: default_wsl_poll_interval_secs(),
            claude_quota_enabled: false,
            claude_quota_refresh_secs: default_claude_quota_refresh_secs(),
            chatgpt_quota_enabled: false,
            chatgpt_quota_refresh_secs: default_chatgpt_quota_refresh_secs(),
            web: WebServerConfig::default(),
            reader_window: ReaderWindowGeometry::default(),
            document_width: DocumentWidth::default(),
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

    /// Re-scan cadence (seconds) for the WSL polling watcher. Default 10s.
    /// Only read on Windows (the apply path and the get command are gated), so
    /// it is dead code elsewhere.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn wsl_poll_interval_secs(&self) -> u64 {
        self.settings.lock().unwrap().wsl_poll_interval_secs
    }

    /// The size reader windows open at.
    pub fn reader_window(&self) -> ReaderWindowGeometry {
        self.settings.lock().unwrap().reader_window
    }

    /// Record the size a reader window was resized to, so the next one adopts
    /// it. Clamped to a sane floor so a window dragged to nothing cannot make
    /// every future reader unusable.
    pub fn set_reader_window(&self, width: f64, height: f64) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.reader_window = ReaderWindowGeometry {
            width: width.max(320.0),
            height: height.max(240.0),
        };
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
    }

    /// The reading width every markdown surface renders at.
    pub fn document_width(&self) -> DocumentWidth {
        self.settings.lock().unwrap().document_width
    }

    /// Record the reader's chosen reading width. No clamping to do — the value
    /// is a rung, and an unrecognised one has already become `Default` at
    /// deserialization.
    pub fn set_document_width(&self, value: DocumentWidth) -> io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        settings.document_width = value;
        let snapshot = settings.clone();
        drop(settings);
        self.save(&snapshot)
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

    /// The defaults a reader window opens at when nothing has been stored.
    /// Values, not just "some number": a reader that opened at 1×1 or off-screen
    /// would be unusable, and nothing else in the system would notice.
    #[test]
    fn reader_geometry_defaults_are_a_usable_window() {
        let geometry = ReaderWindowGeometry::default();
        assert_eq!(geometry.width, 720.0);
        assert_eq!(geometry.height, 820.0);
        assert_eq!(AppSettings::default().reader_window, geometry);
    }

    #[test]
    fn reader_geometry_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.reader_window(), ReaderWindowGeometry::default());

        store.set_reader_window(1024.0, 900.0).unwrap();
        assert_eq!(store.reader_window().width, 1024.0);
        assert_eq!(store.reader_window().height, 900.0);

        // Reloaded from the file, not from the in-memory copy: the next reader
        // adopts this size after a restart, which is the whole point of storing
        // it here rather than letting the window-state plugin do it per label.
        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.reader_window().width, 1024.0);
        assert_eq!(reloaded.reader_window().height, 900.0);
    }

    /// A window dragged to nothing must not make every future reader unusable.
    #[test]
    fn reader_geometry_is_clamped_to_a_usable_floor() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));

        store.set_reader_window(10.0, 5.0).unwrap();

        assert_eq!(store.reader_window().width, 320.0);
        assert_eq!(store.reader_window().height, 240.0);
    }

    /// An older settings file has no `readerWindow` key at all; it must load as
    /// the defaults rather than as zeroes.
    #[test]
    fn settings_without_reader_geometry_load_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"notificationsEnabled": true}"#).unwrap();

        let store = SettingsStore::load(path);

        assert_eq!(store.reader_window(), ReaderWindowGeometry::default());
    }

    #[test]
    fn document_width_defaults_to_the_default_rung() {
        assert_eq!(DocumentWidth::default(), DocumentWidth::Default);
        assert_eq!(
            AppSettings::default().document_width,
            DocumentWidth::Default
        );
    }

    #[test]
    fn document_width_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.document_width(), DocumentWidth::Default);

        store.set_document_width(DocumentWidth::Full).unwrap();

        assert_eq!(
            SettingsStore::load(path).document_width(),
            DocumentWidth::Full
        );
    }

    /// Every rung must survive the round trip under the name the frontend
    /// sends. A rename on either side of the hand-mirrored contract would
    /// otherwise land silently on `Default` through `#[serde(other)]` — the
    /// same attribute that makes an unknown value safe is what would hide a
    /// typo in a known one.
    #[test]
    fn every_rung_round_trips_under_its_wire_name() {
        for (rung, wire) in [
            (DocumentWidth::Compact, "compact"),
            (DocumentWidth::Default, "default"),
            (DocumentWidth::Wide, "wide"),
            (DocumentWidth::Full, "full"),
        ] {
            let json = serde_json::to_string(&rung).unwrap();
            assert_eq!(json, format!("\"{wire}\""), "{rung:?} serializes to {wire}");
            assert_eq!(
                serde_json::from_str::<DocumentWidth>(&json).unwrap(),
                rung,
                "{wire} deserializes back to {rung:?}"
            );
        }
    }

    /// The load-bearing case for `#[serde(other)]`.
    ///
    /// `SettingsStore::load` parses the file in one piece and falls back to the
    /// complete defaults when that parse fails, so a strict enum meeting a
    /// value from a newer version would not surface an unknown reading width —
    /// it would silently reset every other preference in the file. The
    /// assertion is therefore on the *neighbours*, not only on the width.
    #[test]
    fn unrecognised_document_width_loads_as_default_and_keeps_its_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{
                "notificationsEnabled": false,
                "documentWidth": "ultrawide",
                "favoriteChangeIds": ["repo:/r/main/lc:add-dark-mode"],
                "collapsedTreeNodeIds": ["repo:/r/main"],
                "identity": { "displayName": "Ada" },
                "readerWindow": { "width": 1024.0, "height": 900.0 },
                "web": { "enabled": true, "port": 4399 }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);
        let settings = store.snapshot();

        assert_eq!(settings.document_width, DocumentWidth::Default);

        // The point of the test: none of these reverted to their defaults.
        assert!(!settings.notifications_enabled, "notifications kept");
        assert_eq!(
            settings.favorite_change_ids,
            vec!["repo:/r/main/lc:add-dark-mode".to_string()],
            "favorites kept"
        );
        assert_eq!(
            settings.collapsed_tree_node_ids,
            vec!["repo:/r/main".to_string()],
            "collapse state kept"
        );
        assert_eq!(
            settings.identity.display_name.as_deref(),
            Some("Ada"),
            "identity kept"
        );
        assert_eq!(settings.reader_window.width, 1024.0, "reader geometry kept");
        assert!(settings.web.enabled, "web config kept");
        assert_eq!(settings.web.port, 4399, "web port kept");
    }

    /// The read-side tolerance does NOT extend to the write side, and that is
    /// deliberate — see the type's note. Pinned here because the behaviour is
    /// destructive and invisible: the value is lost on the next unrelated
    /// setter, not on anything to do with the reading width.
    #[test]
    fn an_unknown_rung_is_rewritten_to_the_default_by_any_setter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"documentWidth": "ultrawide"}"#).unwrap();

        let store = SettingsStore::load(path.clone());
        // A setter for something else entirely.
        store.set_notifications_enabled(false).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains("\"documentWidth\": \"default\""),
            "the unknown rung is replaced on disk, not preserved: {raw}"
        );
        assert!(
            !raw.contains("ultrawide"),
            "and the original value is gone: {raw}"
        );
    }

    /// An older settings file has no `documentWidth` key at all.
    #[test]
    fn settings_without_document_width_load_the_default_rung() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"notificationsEnabled": true}"#).unwrap();

        assert_eq!(
            SettingsStore::load(path).document_width(),
            DocumentWidth::Default
        );
    }

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
    /// gate, the seasonal locker or the contributor roster still loads: serde
    /// ignores the unknown `gamificationEnabled` / `season` / `people` keys (no
    /// `deny_unknown_fields` anywhere in the workspace), and the next write
    /// drops them. No migration runs — and note the roster is discarded rather
    /// than preserved, which is the accepted, irreversible outcome of removing
    /// the named-people roster.
    #[test]
    fn legacy_gamification_season_and_people_keys_are_ignored_and_dropped() {
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
              "people": [
                {
                  "displayName": "Grace",
                  "identities": [{ "name": "grace", "email": "grace@example.com" }]
                }
              ],
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
        assert!(
            !raw.contains("people"),
            "legacy contributor roster survived a write: {raw}"
        );
        assert_eq!(SettingsStore::load(path).wsl_poll_interval_secs(), 7);
    }
}
