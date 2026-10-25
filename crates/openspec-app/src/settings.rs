use openspec_core::config_file::{preserve_corrupt_config, read_if_present, write_atomic};
use openspec_core::{Author, IdentityConfig};
use serde::{Deserialize, Deserializer, Serialize};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::credential_store::{
    new_bitbucket_entry_ids, new_github_entry_id, BitbucketCredential, CredentialEntryId,
    CredentialSource, CredentialStore, GithubCredential, BITBUCKET_NAMESPACE, GITHUB_NAMESPACE,
};
use crate::file_lock::PathLock;
use crate::review_skip::{self, PatternError, ReviewSkipPatterns, SkipRule};

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
    /// The size every pull-request window opens at — **one** remembered size
    /// for all of them, for the reason `reader_window` gives, and separate
    /// from it, since a side-by-side diff wants a wider window than a
    /// document (`pull-request-viewer`: *Pull-Request Window Geometry*).
    /// Desktop-only: it crosses no IPC, and only the desktop's
    /// `set_pull_request_window_size` sets it.
    #[serde(default)]
    pub pull_request_window: PullRequestWindowGeometry,
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
    /// Whether the main window renders the commit graph — **one**
    /// application-wide switch, shared by the desktop app and the browser skin
    /// (`commit-graph`: *Commit History Can Be Turned Off*).
    ///
    /// On by default, and an absent key loads on. Unlike the opt-in switches
    /// above, this one guards a surface every installation already has, so a
    /// bare `#[serde(default)]` — which loads `false` — would switch the graph
    /// off for every settings file written before the switch existed.
    ///
    /// Which panes the main window has is a setting; whether a pane is shown is
    /// not: the rail's visibility stays per-surface view state in the frontend.
    #[serde(default = "default_commit_history_enabled")]
    pub commit_history_enabled: bool,
    /// The opt-in BitBucket pull-request panel: its switch, the write-only
    /// credential pair, the refresh interval and the side-pane slot it renders
    /// in. `#[serde(default)]` makes an absent block — every file written
    /// before the feature existed — load with the feature off. See
    /// `crate::bitbucket`.
    #[serde(default)]
    pub bitbucket: BitbucketConfig,
    /// The opt-in GitHub pull-request panel: its switch, the write-only token,
    /// the refresh interval and its own side-pane slot — a twin of
    /// [`Self::bitbucket`], independent of it. An absent block loads with the
    /// feature off. See `crate::github`.
    #[serde(default)]
    pub github: GithubConfig,
    /// The review skip patterns: the files of a pull request the reader does
    /// not intend to review (`pull-request-viewer`: *Review Skip Patterns*).
    ///
    /// Empty until the reader stores a list, and `#[serde(default)]` reads
    /// every file written before the patterns existed as the empty list, so
    /// upgrading skips nothing (`review-skip-patterns` design D6). Only the
    /// setter validates; a pattern a hand-edited file holds that a list would
    /// not be accepted with is left out of matching and kept in the file.
    #[serde(default)]
    pub review_skip_patterns: Vec<String>,
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
/// [`SettingsStore::load`] parses the whole file, and when that parse fails it
/// moves the file aside to a `.corrupt-<n>` backup and starts from
/// `AppSettings::default()`. A strict enum meeting a value written by a newer
/// version — or edited by hand — would therefore not report an unknown reading
/// width. It would take favourites, the developer identity, tree collapse
/// state, the web-server configuration and the reader-window geometry out of
/// use along with it, recoverable only by restoring the backup by hand.
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

/// Where a pull-request panel sits: the top or the bottom of either side pane
/// (`bitbucket-pull-requests`: *Panel Position Is a Persisted Setting*;
/// `github-pull-requests`: *GitHub Panel Position Is a Persisted Setting*).
/// Each provider's panel holds its own value. One enum of four values rather
/// than a `{ side, edge }` pair, so it mirrors into `src/types.ts` as one union
/// and into Settings as one choice.
///
/// Kebab-case on the wire, and every variant is two words — so a dropped
/// `rename_all` would silently break the union; `wire_shape.rs` pins the four
/// strings.
///
/// [`Deserialize`] is hand-written for exactly the reason [`DocumentWidth`]'s
/// is: [`SettingsStore::load`] moves a file that fails to parse aside and starts
/// from the complete defaults, so a strict enum meeting a value from a newer
/// version (or a hand edit) would take every other preference out of use along
/// with it. An unrecognised value — or one that is not a string at all — lands
/// on the default slot instead. As there, the tolerance is read-side only: the next
/// write replaces an unknown value with the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelPosition {
    /// Above the workspace tree, in the tree-navigation pane.
    LeftTop,
    /// Between the workspace tree and the sidebar footer entrypoints.
    #[default]
    LeftBottom,
    /// Above the commit graph, in the commit-graph rail.
    RightTop,
    /// Below the commit graph, in the commit-graph rail.
    RightBottom,
}

/// The independently clearable credential persisted by SpecForge. An
/// environment override can therefore be active while this still reports a
/// secure or legacy value underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StoredCredentialSource {
    Secure,
    Legacy,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitbucketSecureEntries {
    username_entry: String,
    api_token_entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum BitbucketCredentialTransaction {
    Staging {
        entries: BitbucketSecureEntries,
    },
    Cleaning {
        retired: Vec<BitbucketSecureEntries>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum GithubCredentialTransaction {
    Staging { entry: String },
    Cleaning { retired: Vec<String> },
}

impl<'de> Deserialize<'de> for PanelPosition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Any JSON value is accepted and anything unrecognised becomes the
        // default slot; see `DocumentWidth` for why this cannot be an error.
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match value.as_str() {
            Some("left-top") => Self::LeftTop,
            Some("right-top") => Self::RightTop,
            Some("right-bottom") => Self::RightBottom,
            _ => Self::LeftBottom,
        })
    }
}

/// The opt-in BitBucket pull-request panel's persisted configuration.
///
/// The credential pair lives here, in the user-only settings file beside every
/// other preference (design D2), and is **write-only** as far as any frontend
/// is concerned: `api_token` is crate-private, the only accessor that returns
/// it is crate-private and read by the poller alone, and the one shape a
/// command serves is [`BitbucketConfigView`], which reports only whether a
/// token is set. `Debug` is written by hand for the same reason, so a stray
/// `{:?}` of the settings can never print the token.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitbucketConfig {
    /// Master switch. Off by default — no credential is read and no request is
    /// made until the user opts in from Settings.
    #[serde(default)]
    pub enabled: bool,
    /// The BitBucket username (or account email) the token belongs to.
    #[serde(default)]
    pub username: Option<String>,
    /// The BitBucket-specific API token. Never returned by any command.
    #[serde(default)]
    pub(crate) api_token: Option<String>,
    /// The committed secure pair. Entry names are non-secret; the values live
    /// only in the operating-system credential store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_secure_entries: Option<BitbucketSecureEntries>,
    /// Crash-recovery metadata for staging or retiring secure entries. It
    /// contains entry ids only, never either credential value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_transaction: Option<BitbucketCredentialTransaction>,
    /// How often (seconds) the poller refreshes while enabled. Default 120;
    /// floored by the poller so a tiny value can't hammer the API.
    #[serde(default = "default_bitbucket_refresh_secs")]
    pub refresh_secs: u64,
    /// Which of the four side-pane slots the panel renders in.
    #[serde(default)]
    pub panel_position: PanelPosition,
}

impl Default for BitbucketConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: None,
            api_token: None,
            active_secure_entries: None,
            credential_transaction: None,
            refresh_secs: default_bitbucket_refresh_secs(),
            panel_position: PanelPosition::default(),
        }
    }
}

impl std::fmt::Debug for BitbucketConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token is reported the way the view reports it — as presence.
        f.debug_struct("BitbucketConfig")
            .field("enabled", &self.enabled)
            .field("username_set", &self.username.is_some())
            .field("token_set", &self.api_token.is_some())
            .field("secure_pair_set", &self.active_secure_entries.is_some())
            .field("credential_transaction", &self.credential_transaction)
            .field("refresh_secs", &self.refresh_secs)
            .field("panel_position", &self.panel_position)
            .finish()
    }
}

/// The safe BitBucket configuration view. It carries neither credential value,
/// only availability and non-secret source state (`bitbucket-pull-requests`:
/// *Credentials Are Stored Write-Only*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BitbucketConfigView {
    pub enabled: bool,
    pub credential_available: bool,
    pub active_source: CredentialSource,
    pub stored_source: StoredCredentialSource,
    pub refresh_secs: u64,
    pub panel_position: PanelPosition,
}

/// The opt-in GitHub pull-request panel's persisted configuration — the twin
/// of [`BitbucketConfig`], with a single token in place of a username/token
/// pair (`github-pull-requests-panel` design D6).
///
/// Write-only on the same terms: `token` is crate-private, the only accessor
/// returning it is crate-private and read by the poller alone, and the one
/// shape a command serves is [`GithubConfigView`]. `Debug` is hand-written so a
/// stray `{:?}` of the settings can never print the token.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubConfig {
    /// Master switch. Off by default — no token is read and no request is made
    /// until the user opts in from Settings.
    #[serde(default)]
    pub enabled: bool,
    /// The GitHub token. Never returned by any command.
    #[serde(default)]
    pub(crate) token: Option<String>,
    /// The committed secure token's non-secret entry id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_secure_entry: Option<String>,
    /// Crash-recovery metadata containing entry ids only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_transaction: Option<GithubCredentialTransaction>,
    /// How often (seconds) the poller refreshes while enabled. Default 120 —
    /// through a default function, not `#[serde(default)]`, which would load a
    /// block missing the key at 0; floored by the poller either way.
    #[serde(default = "default_github_refresh_secs")]
    pub refresh_secs: u64,
    /// Which of the four side-pane slots the GitHub panel renders in,
    /// independent of the BitBucket panel's.
    #[serde(default)]
    pub panel_position: PanelPosition,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: None,
            active_secure_entry: None,
            credential_transaction: None,
            refresh_secs: default_github_refresh_secs(),
            panel_position: PanelPosition::default(),
        }
    }
}

impl std::fmt::Debug for GithubConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token is reported the way the view reports it — as presence.
        f.debug_struct("GithubConfig")
            .field("enabled", &self.enabled)
            .field("token_set", &self.token.is_some())
            .field("secure_token_set", &self.active_secure_entry.is_some())
            .field("credential_transaction", &self.credential_transaction)
            .field("refresh_secs", &self.refresh_secs)
            .field("panel_position", &self.panel_position)
            .finish()
    }
}

/// The safe GitHub configuration view. It carries no token, only availability
/// and non-secret source state (`github-pull-requests`: *The GitHub Token Is
/// Stored Write-Only*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubConfigView {
    pub enabled: bool,
    pub credential_available: bool,
    pub active_source: CredentialSource,
    pub stored_source: StoredCredentialSource,
    pub refresh_secs: u64,
    pub panel_position: PanelPosition,
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

/// The narrowest a pull-request window is resized to, and the width its
/// setter floors what it stores at. The window's minimum size is the same
/// pair, so a stored size always fits it.
pub const PULL_REQUEST_WINDOW_MIN_WIDTH: f64 = 600.0;
/// The shortest a pull-request window is resized to, and the height its
/// setter floors what it stores at.
pub const PULL_REQUEST_WINDOW_MIN_HEIGHT: f64 = 400.0;

/// The shared pull-request-window size, the reader size's twin with its own
/// default and floor. Position is absent for the same reason: a new window is
/// offset from the visible ones rather than reopened where an earlier one
/// sat.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestWindowGeometry {
    #[serde(default = "default_pull_request_width")]
    pub width: f64,
    #[serde(default = "default_pull_request_height")]
    pub height: f64,
}

impl Default for PullRequestWindowGeometry {
    fn default() -> Self {
        Self {
            width: default_pull_request_width(),
            height: default_pull_request_height(),
        }
    }
}

/// Wide enough for the file navigator beside a side-by-side diff
/// (`rich-diff-view` D8); the shell clamps it to the work area.
fn default_pull_request_width() -> f64 {
    1280.0
}

fn default_pull_request_height() -> f64 {
    860.0
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
            pull_request_window: PullRequestWindowGeometry::default(),
            document_width: DocumentWidth::default(),
            commit_history_enabled: default_commit_history_enabled(),
            bitbucket: BitbucketConfig::default(),
            github: GithubConfig::default(),
            review_skip_patterns: Vec::new(),
        }
    }
}

fn default_notifications_enabled() -> bool {
    true
}

fn default_commit_history_enabled() -> bool {
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

/// Two minutes: a list of open pull requests changes on the scale of a review
/// cycle, and each refresh costs `2 + W` requests (design D3).
fn default_bitbucket_refresh_secs() -> u64 {
    120
}

/// The same two minutes as BitBucket. One GitHub refresh is a single request
/// costing 4 of the 5,000 hourly GraphQL points, so the floor, not the budget,
/// is what bounds it (`github-pull-requests-panel` design D2).
fn default_github_refresh_secs() -> u64 {
    120
}

/// A credential field as stored: trimmed, and absent when empty — so submitting
/// an empty token clears the stored one rather than storing `""`.
fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// What [`SettingsStore::load`] found at the settings path
/// (`settings-view`: *A Damaged Settings File Is Never Overwritten by
/// Defaults*). Every outcome but `Loaded` starts the session from the defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsLoad {
    /// No file. Nothing is created until a setting is changed.
    Missing,
    /// The file parsed; its settings are in use.
    Loaded,
    /// The file was read but did not parse (`reason`), and was moved aside to
    /// `backup` before anything could save. Saves write a new file.
    MovedAside { backup: PathBuf, reason: String },
    /// The file exists but could not be read, or did not parse and could not
    /// be moved aside. It is left in place, and every save in this session
    /// fails with this reason rather than overwrite it.
    Unreadable(String),
}

/// File-backed app settings. Launch-on-login is **not** stored here — it
/// lives in the OS via the autostart plugin and is queried each time.
///
/// Two locks, always taken in this order: `write`, then `settings` — never
/// `write` while holding `settings`. Every mutation goes through
/// [`Self::update`], which holds `write` from the change through its save, so
/// saves reach disk in the order their changes were made and each includes
/// every earlier change. `settings` is held only to read or change memory,
/// never across file I/O, so the readers polled every tick never wait for a
/// save's fsync (`settings-view`: *The Settings File Is Saved Atomically and in
/// Order*).
pub struct SettingsStore {
    path: PathBuf,
    settings: Mutex<AppSettings>,
    /// Serialises change-and-save; guards no data of its own.
    write: Mutex<()>,
    /// The process-local-plus-inter-process sidecar lock shared by every store
    /// instance that points at this settings path.
    file_lock: PathLock,
    /// Operating-system secure storage in production; an injected in-memory
    /// implementation in tests.
    credentials: Arc<dyn CredentialStore>,
    load: SettingsLoad,
    /// Parks the next save, so a test can hold a setter inside its file I/O.
    #[cfg(test)]
    save_hook: Mutex<Option<tests::SaveHook>>,
    /// Injects one save failure after N successful saves in unit tests.
    #[cfg(test)]
    save_failure_after: Mutex<Option<usize>>,
}

impl SettingsStore {
    /// Loads the settings at `path`. Infallible: whatever it finds, the session
    /// starts, and [`Self::load_outcome`] says what was found. A missing file
    /// gives the defaults. A file that does not parse as a whole (per-field
    /// tolerance still applies inside the parse) is moved aside with
    /// [`preserve_corrupt_config`] first. A file that cannot be read — or could
    /// not be moved aside — is left untouched and the store refuses to save
    /// over it for the session.
    pub fn load(path: PathBuf) -> Self {
        #[cfg(test)]
        let credentials: Arc<dyn CredentialStore> =
            Arc::new(crate::credential_store::InMemoryCredentialStore::default());
        #[cfg(not(test))]
        let credentials: Arc<dyn CredentialStore> =
            Arc::new(crate::credential_store::OsCredentialStore);
        Self::load_with_credential_store(path, credentials)
    }

    /// Load settings while injecting the secret-store boundary. Tests and
    /// embedders use this to avoid the developer's OS keychain entirely.
    pub fn load_with_credential_store(
        path: PathBuf,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        let (settings, load) = match read_if_present(&path) {
            Ok(None) => (AppSettings::default(), SettingsLoad::Missing),
            Ok(Some(raw)) => match serde_json::from_slice(&raw) {
                Ok(settings) => (settings, SettingsLoad::Loaded),
                Err(parse) => {
                    let load = match preserve_corrupt_config(&path) {
                        Some(backup) => SettingsLoad::MovedAside {
                            backup,
                            reason: parse.to_string(),
                        },
                        None => SettingsLoad::Unreadable(format!(
                            "it does not parse ({parse}) and could not be moved aside"
                        )),
                    };
                    (AppSettings::default(), load)
                }
            },
            Err(read) => (
                AppSettings::default(),
                SettingsLoad::Unreadable(read.to_string()),
            ),
        };
        Self::with(path, settings, load, credentials)
    }

    fn with(
        path: PathBuf,
        settings: AppSettings,
        load: SettingsLoad,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        let file_lock = PathLock::for_resource(&path);
        Self {
            path,
            settings: Mutex::new(settings),
            write: Mutex::new(()),
            file_lock,
            credentials,
            load,
            #[cfg(test)]
            save_hook: Mutex::new(None),
            #[cfg(test)]
            save_failure_after: Mutex::new(None),
        }
    }

    /// What loading found at the settings path, for the caller to report.
    pub fn load_outcome(&self) -> &SettingsLoad {
        &self.load
    }

    /// The one path by which ordinary settings change: lock this path in this
    /// and every other process, recover any interrupted credential transaction,
    /// re-read the current file, apply `f`, save, and only then publish memory.
    ///
    /// Holds `write` throughout, and takes `settings` only to copy the current
    /// value and to store the saved one — never across the save. A save that
    /// fails leaves memory untouched, so the control that asked shows the
    /// stored value and not the attempted one, and no reader ever sees a value
    /// that did not reach disk. Copying outside `settings` and storing later is
    /// safe because every change holds `write`, so none can land in between.
    fn update<R>(&self, f: impl FnOnce(&mut AppSettings) -> R) -> io::Result<R> {
        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let current = self.read_current()?;
        let mut next = self.recover_credentials(current)?;
        let out = f(&mut next);
        self.persist_and_publish(next)?;
        Ok(out)
    }

    pub fn snapshot(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_notifications_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.notifications_enabled = value;
        })
    }

    pub fn set_collapsed_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()> {
        self.update(|settings| {
            settings.collapsed_tree_node_ids = ids;
        })
    }

    pub fn set_expanded_tree_node_ids(&self, ids: Vec<String>) -> io::Result<()> {
        self.update(|settings| {
            settings.expanded_tree_node_ids = ids;
        })
    }

    /// Apply a favorites delta — ids to star and ids to unstar — and return
    /// the merged list. A delta (rather than whole-list replacement) keeps one
    /// client's toggle from erasing favorites another client persisted in the
    /// meantime. Like every setter it goes through [`Self::update`], whose
    /// write lock keeps two concurrent updates from reaching disk out of order
    /// without holding the data lock across the save.
    pub fn update_favorite_change_ids(
        &self,
        add: Vec<String>,
        remove: Vec<String>,
    ) -> io::Result<Vec<String>> {
        self.update(|settings| {
            settings
                .favorite_change_ids
                .retain(|id| !remove.contains(id));
            for id in add {
                if !settings.favorite_change_ids.contains(&id) {
                    settings.favorite_change_ids.push(id);
                }
            }
            settings.favorite_change_ids.clone()
        })
    }

    /// The current developer-identity configuration.
    pub fn identity(&self) -> IdentityConfig {
        self.settings.lock().unwrap().identity.clone()
    }

    /// Replace the whole identity configuration (used by first-run seeding).
    pub fn set_identity(&self, identity: IdentityConfig) -> io::Result<()> {
        self.update(|settings| {
            settings.identity = identity;
        })
    }

    /// Set the canonical display name (cleared to `None` when empty).
    pub fn set_display_name(&self, name: Option<String>) -> io::Result<()> {
        self.update(|settings| {
            settings.identity.display_name = name.filter(|s| !s.trim().is_empty());
        })
    }

    /// Replace the set of alias identities that resolve to "me".
    pub fn set_identity_aliases(&self, aliases: Vec<Author>) -> io::Result<()> {
        self.update(|settings| {
            settings.identity.aliases = aliases;
        })
    }

    /// Re-scan cadence (seconds) for the WSL polling watcher. Default 10s.
    /// Only read on Windows (the apply path and the get command are gated).
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
        self.update(|settings| {
            settings.reader_window = ReaderWindowGeometry {
                width: width.max(320.0),
                height: height.max(240.0),
            };
        })
    }

    /// The size pull-request windows open at.
    pub fn pull_request_window(&self) -> PullRequestWindowGeometry {
        self.settings.lock().unwrap().pull_request_window
    }

    /// Record the size a pull-request window was resized to, so the next one
    /// adopts it. Floored at the windows' own minimum,
    /// [`PULL_REQUEST_WINDOW_MIN_WIDTH`] × [`PULL_REQUEST_WINDOW_MIN_HEIGHT`],
    /// where the reader setter keeps its 320×240.
    pub fn set_pull_request_window(&self, width: f64, height: f64) -> io::Result<()> {
        self.update(|settings| {
            settings.pull_request_window = PullRequestWindowGeometry {
                width: width.max(PULL_REQUEST_WINDOW_MIN_WIDTH),
                height: height.max(PULL_REQUEST_WINDOW_MIN_HEIGHT),
            };
        })
    }

    /// The reading width every markdown surface renders at.
    pub fn document_width(&self) -> DocumentWidth {
        self.settings.lock().unwrap().document_width
    }

    /// Record the reader's chosen reading width. No clamping to do — the value
    /// is a rung, and an unrecognised one has already become `Default` at
    /// deserialization.
    pub fn set_document_width(&self, value: DocumentWidth) -> io::Result<()> {
        self.update(|settings| {
            settings.document_width = value;
        })
    }

    /// Whether the main window renders the commit graph (on by default).
    pub fn commit_history_enabled(&self) -> bool {
        self.settings.lock().unwrap().commit_history_enabled
    }

    /// Turn the commit graph on or off — once, for every surface.
    pub fn set_commit_history_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.commit_history_enabled = value;
        })
    }

    /// Set the WSL polling-watcher re-scan cadence (seconds) and persist it.
    pub fn set_wsl_poll_interval_secs(&self, value: u64) -> io::Result<()> {
        self.update(|settings| {
            settings.wsl_poll_interval_secs = value;
        })
    }

    /// Whether the opt-in Claude usage-quota status line is enabled (off by
    /// default). Read by the quota poller every tick so a toggle takes effect
    /// promptly without restarting it.
    pub fn claude_quota_enabled(&self) -> bool {
        self.settings.lock().unwrap().claude_quota_enabled
    }

    pub fn set_claude_quota_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.claude_quota_enabled = value;
        })
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
        self.update(|settings| {
            settings.chatgpt_quota_enabled = value;
        })
    }

    /// The ChatGPT quota poll cadence (seconds); the poller floors this so a
    /// very small value can't hammer the endpoint.
    pub fn chatgpt_quota_refresh_secs(&self) -> u64 {
        self.settings.lock().unwrap().chatgpt_quota_refresh_secs
    }

    /// Whether the opt-in BitBucket pull-request panel is enabled (off by
    /// default). Read by the pull-request poller every tick so a toggle takes
    /// effect promptly without restarting it.
    pub fn bitbucket_enabled(&self) -> bool {
        self.settings.lock().unwrap().bitbucket.enabled
    }

    pub fn set_bitbucket_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.bitbucket.enabled = value;
        })
    }

    /// The pull-request poll cadence (seconds); the poller floors this so a
    /// very small value can't hammer the API. Not exposed in Settings.
    pub fn bitbucket_refresh_secs(&self) -> u64 {
        self.settings.lock().unwrap().bitbucket.refresh_secs
    }

    /// The side-pane slot the pull-request panel renders in.
    pub fn bitbucket_panel_position(&self) -> PanelPosition {
        self.settings.lock().unwrap().bitbucket.panel_position
    }

    /// Record the panel's slot. Announcing the move to windows already open
    /// (`pull-request-panel-moved`) is the caller's job, on its own transport.
    pub fn set_bitbucket_panel_position(&self, value: PanelPosition) -> io::Result<()> {
        self.update(|settings| {
            settings.bitbucket.panel_position = value;
        })
    }

    /// Replace the BitBucket pair through a recoverable secure-store/settings
    /// transaction. A non-empty token requires a non-empty username. An empty
    /// token clears both secure entries and both legacy plaintext fields.
    pub fn set_bitbucket_credentials(&self, username: String, api_token: String) -> io::Result<()> {
        let username = non_empty(username);
        let api_token = non_empty(api_token);
        let Some(api_token) = api_token else {
            return self.clear_bitbucket_credentials();
        };
        let Some(username) = username else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a BitBucket username is required with a non-empty API token",
            ));
        };

        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let current = self.recover_credentials(self.read_current()?)?;
        self.stage_bitbucket_replacement(current, &username, &api_token)
    }

    /// The highest-precedence complete pair stored by SpecForge, with its
    /// source and stable fingerprint. Environment precedence is applied by the
    /// provider so process environment remains injectable in tests.
    pub(crate) fn bitbucket_stored_credential(&self) -> Option<BitbucketCredential> {
        let config = self.settings.lock().unwrap().bitbucket.clone();
        if let Some((username, api_token)) = self.secure_bitbucket_credentials(&config) {
            return Some(BitbucketCredential::new(
                username,
                api_token,
                CredentialSource::Secure,
            ));
        }
        Some(BitbucketCredential::new(
            config.username?,
            config.api_token?,
            CredentialSource::Legacy,
        ))
    }

    /// The BitBucket configuration every frontend may read — the token is
    /// reported only as whether one is set.
    pub fn bitbucket_config_view(&self) -> BitbucketConfigView {
        self.bitbucket_config_view_with(|name| std::env::var(name).ok())
    }

    fn bitbucket_config_view_with(
        &self,
        env: impl Fn(&str) -> Option<String>,
    ) -> BitbucketConfigView {
        let config = self.settings.lock().unwrap().bitbucket.clone();
        let stored_credential = self.bitbucket_stored_credential();
        let secure_available = stored_credential
            .as_ref()
            .is_some_and(|credential| credential.context().source() == CredentialSource::Secure);
        let legacy_available = config.username.is_some() && config.api_token.is_some();
        let legacy_stored = config.username.is_some() || config.api_token.is_some();
        let environment_available = bitbucket_environment_available(env);
        let active_source = if environment_available {
            CredentialSource::Environment
        } else if secure_available {
            CredentialSource::Secure
        } else if legacy_available {
            CredentialSource::Legacy
        } else {
            CredentialSource::None
        };
        let stored_source = if config.active_secure_entries.is_some() {
            StoredCredentialSource::Secure
        } else if legacy_stored {
            StoredCredentialSource::Legacy
        } else {
            StoredCredentialSource::None
        };
        BitbucketConfigView {
            enabled: config.enabled,
            credential_available: active_source != CredentialSource::None,
            active_source,
            stored_source,
            refresh_secs: config.refresh_secs,
            panel_position: config.panel_position,
        }
    }

    /// Whether the opt-in GitHub pull-request panel is enabled (off by
    /// default). Read by the GitHub poller every tick so a toggle takes effect
    /// promptly without restarting it.
    pub fn github_enabled(&self) -> bool {
        self.settings.lock().unwrap().github.enabled
    }

    pub fn set_github_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.github.enabled = value;
        })
    }

    /// The GitHub poll cadence (seconds); the poller floors this. Not exposed
    /// in Settings.
    pub fn github_refresh_secs(&self) -> u64 {
        self.settings.lock().unwrap().github.refresh_secs
    }

    /// The side-pane slot the GitHub panel renders in.
    pub fn github_panel_position(&self) -> PanelPosition {
        self.settings.lock().unwrap().github.panel_position
    }

    /// Record the GitHub panel's slot. Announcing the move to windows already
    /// open (`pull-request-panel-moved`, provider `github`) is the caller's
    /// job, on its own transport.
    pub fn set_github_panel_position(&self, value: PanelPosition) -> io::Result<()> {
        self.update(|settings| {
            settings.github.panel_position = value;
        })
    }

    /// Replace the GitHub token through a recoverable secure-store/settings
    /// transaction. An empty value clears secure and legacy stored values while
    /// leaving any process environment override untouched.
    pub fn set_github_token(&self, token: String) -> io::Result<()> {
        let Some(token) = non_empty(token) else {
            return self.clear_github_token();
        };
        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let current = self.recover_credentials(self.read_current()?)?;
        self.stage_github_replacement(current, &token)
    }

    /// The highest-precedence token stored by SpecForge, with its source and
    /// stable fingerprint. Environment precedence is applied by GitHub's
    /// resolver so tests can inject it without mutating the process.
    pub(crate) fn github_stored_credential(&self) -> Option<GithubCredential> {
        let config = self.settings.lock().unwrap().github.clone();
        if let Some(token) = self.secure_github_token(&config) {
            return Some(GithubCredential::new(token, CredentialSource::Secure));
        }
        Some(GithubCredential::new(
            config.token?,
            CredentialSource::Legacy,
        ))
    }

    /// The GitHub configuration every frontend may read — the token is
    /// reported only as whether one is set.
    pub fn github_config_view(&self) -> GithubConfigView {
        self.github_config_view_with(|name| std::env::var(name).ok())
    }

    fn github_config_view_with(&self, env: impl Fn(&str) -> Option<String>) -> GithubConfigView {
        let config = self.settings.lock().unwrap().github.clone();
        let stored_credential = self.github_stored_credential();
        let secure_available = stored_credential
            .as_ref()
            .is_some_and(|credential| credential.context().source() == CredentialSource::Secure);
        let legacy_available = config.token.is_some();
        let environment_available = github_environment_available(env);
        let active_source = if environment_available {
            CredentialSource::Environment
        } else if secure_available {
            CredentialSource::Secure
        } else if legacy_available {
            CredentialSource::Legacy
        } else {
            CredentialSource::None
        };
        let stored_source = if config.active_secure_entry.is_some() {
            StoredCredentialSource::Secure
        } else if legacy_available {
            StoredCredentialSource::Legacy
        } else {
            StoredCredentialSource::None
        };
        GithubConfigView {
            enabled: config.enabled,
            credential_available: active_source != CredentialSource::None,
            active_source,
            stored_source,
            refresh_secs: config.refresh_secs,
            panel_position: config.panel_position,
        }
    }

    /// The review skip patterns as `get_review_skip_patterns` answers them:
    /// the stored list, empty until one is stored, and each of its patterns a
    /// list would not be accepted with, which matching leaves out.
    pub fn review_skip_patterns(&self) -> ReviewSkipPatterns {
        let patterns = self.settings.lock().unwrap().review_skip_patterns.clone();
        let (_, errors) = SkipRule::compile(&patterns);
        ReviewSkipPatterns { patterns, errors }
    }

    /// The stored review skip patterns, compiled, for a progress read or a
    /// mark write to match files with. Compiled afresh on every call, which a
    /// list this small allows (`review-skip-patterns` design D5).
    pub fn skip_rule(&self) -> SkipRule {
        let stored = self.settings.lock().unwrap().review_skip_patterns.clone();
        SkipRule::compile(&stored).0
    }

    /// Store `patterns` as the review skip patterns, the empty list included,
    /// each pattern without its surrounding whitespace. A list the setting
    /// does not accept is not stored, in memory or on disk, and every refused
    /// pattern is returned with why; an accepted one returns none. Announcing
    /// the change (`review-skip-patterns-changed`) is the caller's job, on its
    /// own transport.
    pub fn set_review_skip_patterns(&self, patterns: Vec<String>) -> io::Result<Vec<PatternError>> {
        let stored = match review_skip::validate(&patterns) {
            Ok(accepted) => accepted,
            Err(refused) => return Ok(refused),
        };
        self.update(|settings| settings.review_skip_patterns = stored)?;
        Ok(Vec::new())
    }

    /// The embedded web-server configuration (enabled + loopback port). Read once
    /// at desktop launch to decide whether to start the server.
    pub fn web_config(&self) -> WebServerConfig {
        self.settings.lock().unwrap().web.clone()
    }

    /// Enable or disable the embedded web server. Persisted; takes effect at the
    /// next launch (the server is started once at startup).
    pub fn set_web_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.web.enabled = value;
        })
    }

    /// Set the embedded web server's loopback port. Persisted; takes effect at
    /// the next launch.
    pub fn set_web_port(&self, value: u16) -> io::Result<()> {
        self.update(|settings| {
            settings.web.port = value;
        })
    }

    /// Enable or disable Tailscale Serve access (trusting the host's tailnet name
    /// in the web guard). Persisted; takes effect when the server next builds its
    /// router.
    pub fn set_web_tailscale_enabled(&self, value: bool) -> io::Result<()> {
        self.update(|settings| {
            settings.web.tailscale.enabled = value;
        })
    }

    /// Set the manual Tailscale MagicDNS-name override (cleared to `None` when
    /// empty, restoring auto-discovery).
    pub fn set_web_tailscale_name(&self, value: Option<String>) -> io::Result<()> {
        self.update(|settings| {
            settings.web.tailscale.name = value.filter(|s| !s.trim().is_empty());
        })
    }

    /// Replace the Tailscale per-user login allow-list (empty = trust the whole
    /// tailnet).
    pub fn set_web_tailscale_allowed_logins(&self, value: Vec<String>) -> io::Result<()> {
        self.update(|settings| {
            settings.web.tailscale.allowed_logins = value
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        })
    }

    /// Recover both providers' non-secret transaction journals under the
    /// settings lock. A staging record always rolls back its uncommitted
    /// entries; a cleaning record preserves the committed active/cleared state
    /// and retries deletion of retired entries.
    fn recover_credentials(&self, mut current: AppSettings) -> io::Result<AppSettings> {
        if let Some(transaction) = current.github.credential_transaction.clone() {
            match transaction {
                GithubCredentialTransaction::Staging { entry } => {
                    self.credentials
                        .delete(&github_entry(&entry))
                        .map_err(credential_error)?;
                }
                GithubCredentialTransaction::Cleaning { retired } => {
                    for entry in retired {
                        self.credentials
                            .delete(&github_entry(&entry))
                            .map_err(credential_error)?;
                    }
                }
            }
            current.github.credential_transaction = None;
            self.persist_and_publish(current.clone())?;
        }

        if let Some(transaction) = current.bitbucket.credential_transaction.clone() {
            match transaction {
                BitbucketCredentialTransaction::Staging { entries } => {
                    self.delete_bitbucket_entries(&entries)?;
                }
                BitbucketCredentialTransaction::Cleaning { retired } => {
                    for entries in retired {
                        self.delete_bitbucket_entries(&entries)?;
                    }
                }
            }
            current.bitbucket.credential_transaction = None;
            self.persist_and_publish(current.clone())?;
        }
        Ok(current)
    }

    fn stage_github_replacement(&self, prior: AppSettings, token: &str) -> io::Result<()> {
        let staged = new_github_entry_id();
        let staged_name = staged.entry().to_string();
        let retired = prior.github.active_secure_entry.clone();

        // Persist the staged id before putting a secret beneath it. It remains
        // ineligible because `active_secure_entry` still names the prior value.
        let mut staging = prior.clone();
        staging.github.credential_transaction = Some(GithubCredentialTransaction::Staging {
            entry: staged_name.clone(),
        });
        self.persist_and_publish(staging)?;

        if let Err(error) = self.credentials.write(&staged, token) {
            self.rollback_github_staging(&prior, &staged_name);
            return Err(credential_error(error));
        }

        let mut committed = prior.clone();
        committed.github.active_secure_entry = Some(staged_name.clone());
        committed.github.token = None;
        committed.github.credential_transaction =
            retired
                .clone()
                .map(|entry| GithubCredentialTransaction::Cleaning {
                    retired: vec![entry],
                });
        if let Err(error) = self.persist_and_publish(committed.clone()) {
            self.rollback_github_staging(&prior, &staged_name);
            return Err(error);
        }

        if let Some(retired) = retired {
            if let Err(error) = self.credentials.delete(&github_entry(&retired)) {
                self.rollback_committed_github(&prior, &staged_name);
                return Err(credential_error(error));
            }
            self.clear_github_transaction_best_effort(committed);
        }
        Ok(())
    }

    fn clear_github_token(&self) -> io::Result<()> {
        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let prior = self.recover_credentials(self.read_current()?)?;
        let retired = prior.github.active_secure_entry.clone();
        let mut committed = prior.clone();
        committed.github.active_secure_entry = None;
        committed.github.token = None;
        committed.github.credential_transaction =
            retired
                .clone()
                .map(|entry| GithubCredentialTransaction::Cleaning {
                    retired: vec![entry],
                });
        self.persist_and_publish(committed.clone())?;

        if let Some(retired) = retired {
            if let Err(error) = self.credentials.delete(&github_entry(&retired)) {
                // The secure value was not removed, so restoring the previous
                // settings restores the previous resolved state and authority.
                let _ = self.persist_and_publish(prior);
                return Err(credential_error(error));
            }
            self.clear_github_transaction_best_effort(committed);
        }
        Ok(())
    }

    fn rollback_github_staging(&self, prior: &AppSettings, staged: &str) {
        if self.credentials.delete(&github_entry(staged)).is_ok() {
            let _ = self.persist_and_publish(prior.clone());
        }
        // On either failure the persisted `staging` record remains, making the
        // uncommitted entry ineligible and recoverable on the next mutation.
    }

    fn rollback_committed_github(&self, prior: &AppSettings, staged: &str) {
        let mut rollback = prior.clone();
        rollback.github.credential_transaction = Some(GithubCredentialTransaction::Staging {
            entry: staged.to_string(),
        });
        if self.persist_and_publish(rollback).is_ok()
            && self.credentials.delete(&github_entry(staged)).is_ok()
        {
            let _ = self.persist_and_publish(prior.clone());
        }
    }

    fn clear_github_transaction_best_effort(&self, committed: AppSettings) {
        let mut clean = committed;
        clean.github.credential_transaction = None;
        let _ = self.persist_and_publish(clean);
    }

    fn stage_bitbucket_replacement(
        &self,
        prior: AppSettings,
        username: &str,
        api_token: &str,
    ) -> io::Result<()> {
        let (username_id, token_id) = new_bitbucket_entry_ids();
        let staged = BitbucketSecureEntries {
            username_entry: username_id.entry().to_string(),
            api_token_entry: token_id.entry().to_string(),
        };
        let retired = prior.bitbucket.active_secure_entries.clone();
        let retired_values = retired
            .as_ref()
            .map(|entries| self.read_bitbucket_entries(entries))
            .transpose()?;

        let mut staging = prior.clone();
        staging.bitbucket.credential_transaction = Some(BitbucketCredentialTransaction::Staging {
            entries: staged.clone(),
        });
        self.persist_and_publish(staging)?;

        if let Err(error) = self.credentials.write(&username_id, username) {
            self.rollback_bitbucket_staging(&prior, &staged);
            return Err(credential_error(error));
        }
        if let Err(error) = self.credentials.write(&token_id, api_token) {
            self.rollback_bitbucket_staging(&prior, &staged);
            return Err(credential_error(error));
        }

        let mut committed = prior.clone();
        committed.bitbucket.active_secure_entries = Some(staged.clone());
        committed.bitbucket.username = None;
        committed.bitbucket.api_token = None;
        committed.bitbucket.credential_transaction =
            retired
                .clone()
                .map(|entries| BitbucketCredentialTransaction::Cleaning {
                    retired: vec![entries],
                });
        if let Err(error) = self.persist_and_publish(committed.clone()) {
            self.rollback_bitbucket_staging(&prior, &staged);
            return Err(error);
        }

        if let Some(retired) = retired {
            if let Err(error) = self.delete_bitbucket_pair_rollback_safe(
                &retired,
                retired_values.expect("retired values were read above"),
            ) {
                self.rollback_committed_bitbucket(&prior, &staged);
                return Err(error);
            }
            self.clear_bitbucket_transaction_best_effort(committed);
        }
        Ok(())
    }

    fn clear_bitbucket_credentials(&self) -> io::Result<()> {
        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let prior = self.recover_credentials(self.read_current()?)?;
        let retired = prior.bitbucket.active_secure_entries.clone();
        let retired_values = retired
            .as_ref()
            .map(|entries| self.read_bitbucket_entries(entries))
            .transpose()?;
        let mut committed = prior.clone();
        committed.bitbucket.active_secure_entries = None;
        committed.bitbucket.username = None;
        committed.bitbucket.api_token = None;
        committed.bitbucket.credential_transaction =
            retired
                .clone()
                .map(|entries| BitbucketCredentialTransaction::Cleaning {
                    retired: vec![entries],
                });
        self.persist_and_publish(committed.clone())?;

        if let Some(retired) = retired {
            if let Err(error) = self.delete_bitbucket_pair_rollback_safe(
                &retired,
                retired_values.expect("retired values were read above"),
            ) {
                let _ = self.persist_and_publish(prior);
                return Err(error);
            }
            self.clear_bitbucket_transaction_best_effort(committed);
        }
        Ok(())
    }

    fn rollback_bitbucket_staging(&self, prior: &AppSettings, staged: &BitbucketSecureEntries) {
        if self.delete_bitbucket_entries(staged).is_ok() {
            let _ = self.persist_and_publish(prior.clone());
        }
    }

    fn rollback_committed_bitbucket(&self, prior: &AppSettings, staged: &BitbucketSecureEntries) {
        let mut rollback = prior.clone();
        rollback.bitbucket.credential_transaction = Some(BitbucketCredentialTransaction::Staging {
            entries: staged.clone(),
        });
        if self.persist_and_publish(rollback).is_ok()
            && self.delete_bitbucket_entries(staged).is_ok()
        {
            let _ = self.persist_and_publish(prior.clone());
        }
    }

    fn clear_bitbucket_transaction_best_effort(&self, committed: AppSettings) {
        let mut clean = committed;
        clean.bitbucket.credential_transaction = None;
        let _ = self.persist_and_publish(clean);
    }

    fn secure_github_token(&self, config: &GithubConfig) -> Option<String> {
        let entry = config.active_secure_entry.as_deref()?;
        self.credentials
            .read(&github_entry(entry))
            .ok()
            .flatten()
            .filter(|secret| !secret.is_empty())
    }

    fn secure_bitbucket_credentials(&self, config: &BitbucketConfig) -> Option<(String, String)> {
        let entries = config.active_secure_entries.as_ref()?;
        self.read_bitbucket_entries(entries).ok()
    }

    fn read_bitbucket_entries(
        &self,
        entries: &BitbucketSecureEntries,
    ) -> io::Result<(String, String)> {
        let username = self
            .credentials
            .read(&bitbucket_entry(&entries.username_entry))
            .map_err(credential_error)?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| io::Error::other("the secure BitBucket username is unavailable"))?;
        let token = self
            .credentials
            .read(&bitbucket_entry(&entries.api_token_entry))
            .map_err(credential_error)?
            .filter(|value| !value.is_empty())
            .ok_or_else(|| io::Error::other("the secure BitBucket API token is unavailable"))?;
        Ok((username, token))
    }

    fn delete_bitbucket_entries(&self, entries: &BitbucketSecureEntries) -> io::Result<()> {
        self.credentials
            .delete(&bitbucket_entry(&entries.username_entry))
            .map_err(credential_error)?;
        self.credentials
            .delete(&bitbucket_entry(&entries.api_token_entry))
            .map_err(credential_error)
    }

    /// Delete a committed old pair. If deleting the second half fails after
    /// the first was removed, restore that first half before the caller rolls
    /// settings back to the prior active pair.
    fn delete_bitbucket_pair_rollback_safe(
        &self,
        entries: &BitbucketSecureEntries,
        values: (String, String),
    ) -> io::Result<()> {
        self.credentials
            .delete(&bitbucket_entry(&entries.username_entry))
            .map_err(credential_error)?;
        if let Err(error) = self
            .credentials
            .delete(&bitbucket_entry(&entries.api_token_entry))
        {
            self.credentials
                .write(&bitbucket_entry(&entries.username_entry), &values.0)
                .map_err(credential_error)?;
            return Err(credential_error(error));
        }
        Ok(())
    }

    /// Read and recover durable credential metadata under the same lock used by
    /// mutations. Review-action authorisation uses this rather than trusting a
    /// stale process snapshot.
    pub fn recover_credential_transactions(&self) -> io::Result<()> {
        let _write = self.write.lock().unwrap();
        self.ensure_savable()?;
        let _file = self.file_lock.lock()?;
        let current = self.read_current()?;
        let recovered = self.recover_credentials(current)?;
        *self.settings.lock().unwrap() = recovered;
        Ok(())
    }

    fn ensure_savable(&self) -> io::Result<()> {
        if let SettingsLoad::Unreadable(reason) = &self.load {
            return Err(io::Error::other(format!(
                "{} could not be read at startup ({reason}); SpecForge will not save over it",
                self.path.display()
            )));
        }
        Ok(())
    }

    /// Re-read while holding the path lock. Unlike startup loading, a newly
    /// damaged file is not moved or normalised; the requested mutation fails
    /// and leaves its bytes untouched.
    fn read_current(&self) -> io::Result<AppSettings> {
        match read_if_present(&self.path)? {
            None => Ok(AppSettings::default()),
            Some(raw) => serde_json::from_slice(&raw).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{} became unparsable; it was left untouched ({error})",
                        self.path.display()
                    ),
                )
            }),
        }
    }

    fn persist_and_publish(&self, settings: AppSettings) -> io::Result<()> {
        self.save(&settings)?;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    /// Writes `settings` atomically ([`write_atomic`]), or — when loading
    /// found a file it could not use and left in place — fails without
    /// touching the file system.
    fn save(&self, settings: &AppSettings) -> io::Result<()> {
        #[cfg(test)]
        {
            let hook = self.save_hook.lock().unwrap().take();
            if let Some(hook) = hook {
                hook.park();
            }
            let mut failure = self.save_failure_after.lock().unwrap();
            if let Some(remaining) = failure.as_mut() {
                if *remaining == 0 {
                    *failure = None;
                    return Err(io::Error::other("injected settings save failure"));
                }
                *remaining -= 1;
            }
        }
        self.ensure_savable()?;
        let raw = serde_json::to_vec_pretty(settings)?;
        write_atomic(&self.path, &raw)
    }
}

fn github_entry(entry: &str) -> CredentialEntryId {
    CredentialEntryId::new(GITHUB_NAMESPACE, entry)
}

fn bitbucket_entry(entry: &str) -> CredentialEntryId {
    CredentialEntryId::new(BITBUCKET_NAMESPACE, entry)
}

fn credential_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn github_environment_available(env: impl Fn(&str) -> Option<String>) -> bool {
    ["GH_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .any(|name| env(name).is_some_and(|value| !value.is_empty()))
}

fn bitbucket_environment_available(env: impl Fn(&str) -> Option<String>) -> bool {
    env("BITBUCKET_USERNAME").is_some_and(|value| !value.is_empty())
        && env("BITBUCKET_API_TOKEN").is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
    use std::sync::Arc;
    use std::time::Duration;

    fn memory_credentials() -> Arc<crate::credential_store::InMemoryCredentialStore> {
        Arc::new(crate::credential_store::InMemoryCredentialStore::default())
    }

    /// The stored GitHub token, read the way only tests may: no transport
    /// ever returns it.
    fn stored_github_token(store: &SettingsStore) -> Option<String> {
        store
            .github_stored_credential()
            .map(|credential| credential.token().to_string())
    }

    /// The stored BitBucket pair, read the way only tests may: no transport
    /// ever returns it.
    fn stored_bitbucket_pair(store: &SettingsStore) -> Option<(String, String)> {
        let credential = store.bitbucket_stored_credential()?;
        Some((
            credential.username().to_string(),
            credential.api_token().to_string(),
        ))
    }

    fn load_with_credentials(
        path: PathBuf,
        credentials: &Arc<crate::credential_store::InMemoryCredentialStore>,
    ) -> SettingsStore {
        let store: Arc<dyn CredentialStore> = credentials.clone();
        SettingsStore::load_with_credential_store(path, store)
    }

    fn write_settings(path: &std::path::Path, settings: &AppSettings) {
        fs::write(path, serde_json::to_vec_pretty(settings).unwrap()).unwrap();
    }

    fn read_settings(path: &std::path::Path) -> AppSettings {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    /// Parks one save inside [`SettingsStore::save`]: it reports that it got
    /// there, then waits to be released. Dropping the release side (a test
    /// that panicked) releases it too, so nothing is left hanging.
    pub(super) struct SaveHook {
        entered: Sender<()>,
        release: Receiver<()>,
    }

    impl SaveHook {
        pub(super) fn park(self) {
            let _ = self.entered.send(());
            let _ = self.release.recv();
        }
    }

    /// Parks `store`'s next save. Returns the receiver that hears the save
    /// arrive and the sender that releases it.
    fn park_next_save(store: &SettingsStore) -> (Receiver<()>, Sender<()>) {
        let (entered, arrived) = channel();
        let (release, released) = channel();
        *store.save_hook.lock().unwrap() = Some(SaveHook {
            entered,
            release: released,
        });
        (arrived, release)
    }

    fn fail_save_after(store: &SettingsStore, successful_saves: usize) {
        *store.save_failure_after.lock().unwrap() = Some(successful_saves);
    }

    /// Generous: a correct build answers at once, so this bound is only ever
    /// reached by a broken one, which then fails rather than hangs.
    const GENEROUS: Duration = Duration::from_secs(30);

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
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);
        assert_eq!(store.reader_window(), ReaderWindowGeometry::default());

        store.set_reader_window(1024.0, 900.0).unwrap();
        assert_eq!(store.reader_window().width, 1024.0);
        assert_eq!(store.reader_window().height, 900.0);

        // Reloaded from the file, not from the in-memory copy: the next reader
        // adopts this size after a restart, which is the whole point of storing
        // it here rather than letting the window-state plugin do it per label.
        let reloaded = load_with_credentials(path, &credentials);
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

    /// `pull-request-viewer`: *The first window fits a side-by-side diff*:
    /// with nothing stored, and in a settings file written before the key
    /// existed, a pull-request window opens at 1280×860.
    #[test]
    fn pull_request_geometry_defaults_fit_a_side_by_side_diff() {
        let geometry = PullRequestWindowGeometry::default();
        assert_eq!((geometry.width, geometry.height), (1280.0, 860.0));
        assert_eq!(AppSettings::default().pull_request_window, geometry);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"readerWindow": {"width": 1024.0, "height": 900.0}}"#,
        )
        .unwrap();
        assert_eq!(SettingsStore::load(path).pull_request_window(), geometry);
    }

    /// `pull-request-viewer`: *A resize sets the size for the next window and
    /// survives a restart*, read back from the file.
    #[test]
    fn pull_request_geometry_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        store.set_pull_request_window(1440.0, 900.0).unwrap();
        assert_eq!(
            SettingsStore::load(path).pull_request_window(),
            PullRequestWindowGeometry {
                width: 1440.0,
                height: 900.0,
            }
        );
    }

    /// `pull-request-viewer`: *The size has a floor*: 300×200 is stored as
    /// 600×400, the floor itself is kept, and so is anything above it.
    #[test]
    fn pull_request_geometry_is_floored_at_six_hundred_by_four_hundred() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        let stored = |width, height| {
            store.set_pull_request_window(width, height).unwrap();
            let geometry = store.pull_request_window();
            (geometry.width, geometry.height)
        };
        assert_eq!(stored(300.0, 200.0), (600.0, 400.0));
        assert_eq!(stored(599.5, 399.5), (600.0, 400.0));
        assert_eq!(stored(600.0, 400.0), (600.0, 400.0));
        assert_eq!(stored(600.5, 400.5), (600.5, 400.5));
        assert_eq!(stored(300.0, 900.0), (600.0, 900.0), "each side alone");
        assert_eq!(
            (
                PULL_REQUEST_WINDOW_MIN_WIDTH,
                PULL_REQUEST_WINDOW_MIN_HEIGHT
            ),
            (600.0, 400.0)
        );
    }

    /// `pull-request-viewer`: *Reader windows keep their own size*: each
    /// setter moves its own size only, and the reader's floor stays 320×240.
    #[test]
    fn pull_request_and_reader_sizes_are_set_apart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        store.set_pull_request_window(1600.0, 1000.0).unwrap();
        assert_eq!(store.reader_window(), ReaderWindowGeometry::default());

        store.set_reader_window(500.0, 300.0).unwrap();
        let reloaded = SettingsStore::load(path);
        assert_eq!(
            (
                reloaded.reader_window().width,
                reloaded.reader_window().height
            ),
            (500.0, 300.0),
            "under the pull-request floor, over the reader's"
        );
        assert_eq!(
            reloaded.pull_request_window(),
            PullRequestWindowGeometry {
                width: 1600.0,
                height: 1000.0,
            }
        );
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
    /// `SettingsStore::load` parses the file in one piece, and when that parse
    /// fails it moves the file aside and starts from the complete defaults. A
    /// strict enum meeting a value from a newer version would therefore not
    /// surface an unknown reading width — it would take every other preference
    /// in the file out of use. The assertion is therefore on the *neighbours*,
    /// not only on the width.
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

    #[test]
    fn commit_history_defaults_to_on() {
        assert!(AppSettings::default().commit_history_enabled);
    }

    #[test]
    fn commit_history_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        assert!(store.commit_history_enabled());

        store.set_commit_history_enabled(false).unwrap();
        assert!(!store.commit_history_enabled());
        assert!(
            !SettingsStore::load(path.clone()).commit_history_enabled(),
            "off survives a restart"
        );

        store.set_commit_history_enabled(true).unwrap();
        assert!(
            SettingsStore::load(path).commit_history_enabled(),
            "and so does turning it back on"
        );
    }

    /// An older settings file has no `commitHistoryEnabled` key at all. It must
    /// load with the graph ON: every installation had the graph before the
    /// switch existed, and must keep it.
    #[test]
    fn settings_without_commit_history_load_it_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"notificationsEnabled": false, "documentWidth": "wide"}"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);

        assert!(store.commit_history_enabled());
        assert!(!store.snapshot().notifications_enabled, "neighbours kept");
        assert_eq!(
            store.document_width(),
            DocumentWidth::Wide,
            "neighbours kept"
        );
    }

    /// The on-disk key is a compatibility contract: a file written with the
    /// graph turned off must load it off — a renamed key would silently turn
    /// every such reader's graph back on.
    #[test]
    fn a_stored_commit_history_off_loads_off() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"commitHistoryEnabled": false}"#).unwrap();

        assert!(!SettingsStore::load(path).commit_history_enabled());
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
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);

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
        store.set_bitbucket_enabled(true).unwrap();
        store
            .set_bitbucket_panel_position(PanelPosition::RightTop)
            .unwrap();
        store
            .set_bitbucket_credentials(" ada ".to_string(), " tok ".to_string())
            .unwrap();
        store.set_github_enabled(true).unwrap();
        store
            .set_github_panel_position(PanelPosition::RightBottom)
            .unwrap();
        store.set_github_token(" ghp_tok ".to_string()).unwrap();
        // Off, because on is the default: only the non-default value proves
        // the setter wrote and the getter read.
        store.set_commit_history_enabled(false).unwrap();

        let reloaded = load_with_credentials(path, &credentials);
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
        assert!(!reloaded.commit_history_enabled());
        assert!(reloaded.bitbucket_enabled());
        assert_eq!(reloaded.bitbucket_refresh_secs(), 120);
        assert_eq!(reloaded.bitbucket_panel_position(), PanelPosition::RightTop);
        assert_eq!(
            stored_bitbucket_pair(&reloaded),
            Some(("ada".to_string(), "tok".to_string())),
            "both halves are stored trimmed"
        );
        assert!(reloaded.github_enabled());
        assert_eq!(reloaded.github_refresh_secs(), 120);
        assert_eq!(
            reloaded.github_panel_position(),
            PanelPosition::RightBottom,
            "independent of the BitBucket slot"
        );
        assert_eq!(
            stored_github_token(&reloaded).as_deref(),
            Some("ghp_tok"),
            "stored trimmed"
        );
        let view = reloaded.github_config_view_with(|_| None);
        assert!(view.enabled);
        assert!(view.credential_available);
        assert_eq!(view.active_source, CredentialSource::Secure);
        assert_eq!(view.stored_source, StoredCredentialSource::Secure);
        assert_eq!(view.refresh_secs, 120);
        assert_eq!(view.panel_position, PanelPosition::RightBottom);
    }

    /// A settings file written before the GitHub feature existed — including
    /// one that already carries a `bitbucket` block — loads with GitHub off, at
    /// the default slot, polling every two minutes, with no token.
    #[test]
    fn an_absent_github_block_loads_disabled_at_left_bottom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"bitbucket": {"enabled": true, "panelPosition": "right-top"}}"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);

        assert!(!store.github_enabled());
        assert_eq!(store.github_panel_position(), PanelPosition::LeftBottom);
        assert_eq!(store.github_refresh_secs(), 120);
        assert_eq!(stored_github_token(&store), None);
        let view = store.github_config_view_with(|_| None);
        assert!(!view.enabled);
        assert!(!view.credential_available);
        assert_eq!(view.active_source, CredentialSource::None);
        assert_eq!(view.stored_source, StoredCredentialSource::None);
        // The neighbouring block is untouched by the new default.
        assert!(store.bitbucket_enabled());
        assert_eq!(store.bitbucket_panel_position(), PanelPosition::RightTop);
    }

    /// A `github` block missing `refreshSecs` loads at 120, not at a derived
    /// zero — the reason the field has a default function.
    #[test]
    fn a_github_block_without_an_interval_polls_every_two_minutes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"github": {"enabled": true}}"#).unwrap();

        let store = SettingsStore::load(path);

        assert!(store.github_enabled());
        assert_eq!(store.github_refresh_secs(), 120);
    }

    /// An unknown GitHub slot degrades to the default without disturbing the
    /// BitBucket block beside it or anything else in the file.
    #[test]
    fn unrecognised_github_panel_position_loads_as_left_bottom_and_keeps_its_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "github": { "enabled": true, "refreshSecs": 300, "panelPosition": "floating" },
                "bitbucket": { "enabled": true, "panelPosition": "right-top" },
                "identity": { "displayName": "Ada" }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);

        assert_eq!(store.github_panel_position(), PanelPosition::LeftBottom);
        assert!(store.github_enabled(), "the switch kept");
        assert_eq!(store.github_refresh_secs(), 300, "the interval kept");
        assert_eq!(
            store.bitbucket_panel_position(),
            PanelPosition::RightTop,
            "the BitBucket block kept"
        );
        assert_eq!(
            store.snapshot().identity.display_name.as_deref(),
            Some("Ada"),
            "identity kept"
        );
    }

    /// Submitting an empty (or blank) GitHub token clears the stored one.
    #[test]
    fn an_empty_github_token_clears_the_stored_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);

        store.set_github_token("ghp_s3cret".to_string()).unwrap();
        assert_eq!(stored_github_token(&store).as_deref(), Some("ghp_s3cret"));
        assert_eq!(
            store.github_config_view_with(|_| None).stored_source,
            StoredCredentialSource::Secure
        );

        store.set_github_token("   ".to_string()).unwrap();
        assert_eq!(stored_github_token(&store), None);
        assert_eq!(
            store.github_config_view_with(|_| None).stored_source,
            StoredCredentialSource::None
        );
        assert_eq!(
            stored_github_token(&load_with_credentials(path, &credentials)),
            None,
            "on disk too"
        );
    }

    /// The GitHub token never leaves through the view or through `Debug`.
    #[test]
    fn the_github_token_is_never_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let credentials = memory_credentials();
        let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
        store.set_github_token("ghp_s3cret".to_string()).unwrap();

        let view = serde_json::to_value(store.github_config_view()).unwrap();
        assert_eq!(
            view,
            serde_json::json!({
                "enabled": false,
                "credentialAvailable": true,
                "activeSource": "secure",
                "storedSource": "secure",
                "refreshSecs": 120,
                "panelPosition": "left-bottom"
            })
        );
        assert!(!view.to_string().contains("ghp_s3cret"));
        let debug = format!("{:?}", store.snapshot().github);
        assert!(!debug.contains("ghp_s3cret"), "{debug}");
        assert!(debug.contains("token_set: true"), "{debug}");
    }

    /// A settings file written before the feature existed has no `bitbucket`
    /// block at all: the feature loads off, at the default slot, with no
    /// credential stored.
    #[test]
    fn an_absent_bitbucket_block_loads_disabled_at_left_bottom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"notificationsEnabled": true}"#).unwrap();

        let store = SettingsStore::load(path);

        assert!(!store.bitbucket_enabled());
        assert_eq!(store.bitbucket_panel_position(), PanelPosition::LeftBottom);
        assert_eq!(store.bitbucket_refresh_secs(), 120);
        assert_eq!(stored_bitbucket_pair(&store), None);
        let view = store.bitbucket_config_view_with(|_| None);
        assert!(!view.enabled);
        assert!(!view.credential_available);
        assert_eq!(view.active_source, CredentialSource::None);
        assert_eq!(view.stored_source, StoredCredentialSource::None);
        assert_eq!(view.panel_position, PanelPosition::LeftBottom);
    }

    /// The load-bearing case for `PanelPosition`'s hand-written `Deserialize`,
    /// asserted on the neighbours for the reason
    /// `unrecognised_document_width_loads_as_default_and_keeps_its_neighbours`
    /// gives: a strict enum would not report an unknown slot, it would fail the
    /// whole parse, move the file aside and take every other preference in it
    /// out of use.
    #[test]
    fn unrecognised_panel_position_loads_as_left_bottom_and_keeps_its_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "bitbucket": {
                    "enabled": true,
                    "username": "ada",
                    "refreshSecs": 300,
                    "panelPosition": "center-stage"
                },
                "favoriteChangeIds": ["repo:/r/main/lc:add-dark-mode"],
                "identity": { "displayName": "Ada" }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);
        let settings = store.snapshot();

        assert_eq!(store.bitbucket_panel_position(), PanelPosition::LeftBottom);
        // The rest of the block survives beside the unknown slot…
        assert!(store.bitbucket_enabled(), "the switch kept");
        assert_eq!(store.bitbucket_refresh_secs(), 300, "the interval kept");
        assert_eq!(
            store.snapshot().bitbucket.username.as_deref(),
            Some("ada"),
            "the legacy username kept internally"
        );
        // …and so does everything outside it.
        assert_eq!(
            settings.favorite_change_ids,
            vec!["repo:/r/main/lc:add-dark-mode".to_string()],
            "favorites kept"
        );
        assert_eq!(
            settings.identity.display_name.as_deref(),
            Some("Ada"),
            "identity kept"
        );
    }

    /// Submitting an empty token clears the complete stored pair in memory, in
    /// secure storage and on disk.
    #[test]
    fn an_empty_token_clears_the_stored_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);

        store
            .set_bitbucket_credentials("ada".to_string(), "s3cret".to_string())
            .unwrap();
        assert_eq!(
            stored_bitbucket_pair(&store),
            Some(("ada".to_string(), "s3cret".to_string()))
        );
        assert_eq!(
            store.bitbucket_config_view_with(|_| None).stored_source,
            StoredCredentialSource::Secure
        );

        store
            .set_bitbucket_credentials("ada".to_string(), String::new())
            .unwrap();
        assert_eq!(
            stored_bitbucket_pair(&store),
            None,
            "no pair without a token"
        );
        let view = store.bitbucket_config_view_with(|_| None);
        assert!(!view.credential_available, "the pair is gone");
        assert_eq!(view.stored_source, StoredCredentialSource::None);

        let reloaded = load_with_credentials(path.clone(), &credentials);
        assert_eq!(stored_bitbucket_pair(&reloaded), None);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("s3cret"),
            "the token is gone from disk: {raw}"
        );

        // A whitespace-only token is empty too.
        store
            .set_bitbucket_credentials("ada".to_string(), "s3cret".to_string())
            .unwrap();
        store
            .set_bitbucket_credentials("ada".to_string(), "   ".to_string())
            .unwrap();
        assert_eq!(
            store.bitbucket_config_view_with(|_| None).stored_source,
            StoredCredentialSource::None
        );
    }

    /// The only shape a command serves must not carry the token, under any
    /// key: the view reports presence and nothing else.
    #[test]
    fn the_config_view_serialises_without_the_token() {
        let dir = tempfile::tempdir().unwrap();
        let credentials = memory_credentials();
        let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
        store
            .set_bitbucket_credentials("ada".to_string(), "s3cret-token".to_string())
            .unwrap();

        let json = serde_json::to_value(store.bitbucket_config_view()).unwrap();

        assert!(json.get("apiToken").is_none(), "no apiToken key: {json}");
        assert!(
            !json.to_string().contains("s3cret-token"),
            "the token value appears nowhere: {json}"
        );
        assert_eq!(json["credentialAvailable"], true);
        assert_eq!(json["activeSource"], "secure");
        assert_eq!(json["storedSource"], "secure");
        assert!(json.get("username").is_none(), "no username key: {json}");
        assert_eq!(json["refreshSecs"], 120);
        assert_eq!(json["panelPosition"], "left-bottom");
        assert_eq!(json["enabled"], false);
    }

    /// Legacy settings may still contain a token, but a `Debug` diagnostic must
    /// never reveal either half of the pair.
    #[test]
    fn the_debug_form_never_prints_the_token() {
        let config = BitbucketConfig {
            username: Some("ada".to_string()),
            api_token: Some("s3cret-token".to_string()),
            ..BitbucketConfig::default()
        };

        let printed = format!("{config:?}");

        assert!(!printed.contains("s3cret-token"), "{printed}");
        assert!(printed.contains("token_set: true"), "{printed}");
        assert!(printed.contains("username_set: true"), "{printed}");
        assert!(!printed.contains("ada"), "{printed}");
    }

    #[test]
    fn stale_instances_re_read_and_preserve_secure_and_unrelated_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "notificationsEnabled": true,
                "github": { "token": "legacy-token", "panelPosition": "left-bottom" }
            }"#,
        )
        .unwrap();
        let credentials = memory_credentials();
        let first = load_with_credentials(path.clone(), &credentials);
        let stale = load_with_credentials(path.clone(), &credentials);

        first.set_github_token("secure-token".into()).unwrap();
        stale
            .set_github_panel_position(PanelPosition::RightTop)
            .unwrap();
        first.set_notifications_enabled(false).unwrap();

        let durable = load_with_credentials(path.clone(), &credentials);
        assert_eq!(
            stored_github_token(&durable).as_deref(),
            Some("secure-token")
        );
        assert_eq!(durable.github_panel_position(), PanelPosition::RightTop);
        assert!(!durable.snapshot().notifications_enabled);
        assert_eq!(
            durable.github_config_view_with(|_| None).stored_source,
            StoredCredentialSource::Secure
        );
        let raw = fs::read_to_string(path).unwrap();
        assert!(
            !raw.contains("legacy-token"),
            "legacy value resurrected: {raw}"
        );
        assert!(!raw.contains("secure-token"), "secure value leaked: {raw}");
    }

    #[test]
    fn a_file_damaged_after_load_is_refused_and_left_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"notificationsEnabled":true}"#).unwrap();
        let store = SettingsStore::load(path.clone());
        let damaged = b"{\"github\":";
        fs::write(&path, damaged).unwrap();

        let error = store.set_notifications_enabled(false).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).unwrap(), damaged);
        assert_eq!(
            dir_entries(dir.path()),
            ["settings.json", "settings.json.lock"],
            "the newly damaged file is neither moved nor normalised"
        );
        assert!(store.snapshot().notifications_enabled);
    }

    #[test]
    fn an_incomplete_non_empty_bitbucket_pair_changes_neither_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);
        store
            .set_bitbucket_credentials("ada".into(), "old-token".into())
            .unwrap();
        let before = fs::read(&path).unwrap();
        let before_entries = credentials.entry_count();

        let error = store
            .set_bitbucket_credentials("   ".into(), "new-token".into())
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(credentials.entry_count(), before_entries);
        assert_eq!(
            stored_bitbucket_pair(&store),
            Some(("ada".into(), "old-token".into()))
        );
    }

    #[test]
    fn environment_active_and_stored_sources_are_reported_independently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let store = load_with_credentials(path, &credentials);
        store.set_github_token("stored-gh".into()).unwrap();
        store
            .set_bitbucket_credentials("stored-user".into(), "stored-bb".into())
            .unwrap();

        let github = store.github_config_view_with(|name| {
            (name == "GH_TOKEN").then(|| "environment-gh".to_string())
        });
        assert_eq!(github.active_source, CredentialSource::Environment);
        assert_eq!(github.stored_source, StoredCredentialSource::Secure);

        let partial_environment = store.bitbucket_config_view_with(|name| {
            (name == "BITBUCKET_USERNAME").then(|| "environment-user".to_string())
        });
        assert_eq!(partial_environment.active_source, CredentialSource::Secure);
        assert_eq!(
            partial_environment.stored_source,
            StoredCredentialSource::Secure
        );

        let complete_environment = store.bitbucket_config_view_with(|name| match name {
            "BITBUCKET_USERNAME" => Some("environment-user".to_string()),
            "BITBUCKET_API_TOKEN" => Some("environment-token".to_string()),
            _ => None,
        });
        assert_eq!(
            complete_environment.active_source,
            CredentialSource::Environment
        );
        assert_eq!(
            complete_environment.stored_source,
            StoredCredentialSource::Secure
        );
    }

    #[test]
    fn legacy_sources_are_read_without_automatic_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = r#"{
            "github": { "token": "legacy-gh" },
            "bitbucket": { "username": "legacy-user", "apiToken": "legacy-bb" }
        }"#;
        fs::write(&path, original).unwrap();
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);

        assert_eq!(stored_github_token(&store).as_deref(), Some("legacy-gh"));
        assert_eq!(
            stored_bitbucket_pair(&store),
            Some(("legacy-user".into(), "legacy-bb".into()))
        );
        let github = store.github_config_view_with(|_| None);
        assert_eq!(github.active_source, CredentialSource::Legacy);
        assert_eq!(github.stored_source, StoredCredentialSource::Legacy);
        let bitbucket = store.bitbucket_config_view_with(|_| None);
        assert_eq!(bitbucket.active_source, CredentialSource::Legacy);
        assert_eq!(bitbucket.stored_source, StoredCredentialSource::Legacy);
        assert_eq!(credentials.entry_count(), 0);
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }

    #[test]
    fn incomplete_legacy_bitbucket_state_is_stored_but_not_available() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"bitbucket":{"username":"orphan"}}"#).unwrap();
        let store = SettingsStore::load(path);

        let view = store.bitbucket_config_view_with(|_| None);
        assert!(!view.credential_available);
        assert_eq!(view.active_source, CredentialSource::None);
        assert_eq!(view.stored_source, StoredCredentialSource::Legacy);
    }

    #[test]
    fn successful_secure_saves_persist_metadata_but_no_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let store = load_with_credentials(path.clone(), &credentials);
        store.set_github_token("never-persist-gh".into()).unwrap();
        store
            .set_bitbucket_credentials("never-persist-user".into(), "never-persist-bb".into())
            .unwrap();

        let raw = fs::read_to_string(path).unwrap();
        for secret in ["never-persist-gh", "never-persist-user", "never-persist-bb"] {
            assert!(!raw.contains(secret), "secret reached settings.json: {raw}");
        }
        assert!(raw.contains("activeSecureEntry"), "{raw}");
        assert!(raw.contains("activeSecureEntries"), "{raw}");
        assert!(!raw.contains("credentialTransaction"), "{raw}");
        assert_eq!(credentials.entry_count(), 3);
    }

    #[test]
    fn github_initial_save_rolls_back_each_pre_commit_failure() {
        use crate::credential_store::CredentialStoreOperation;

        // The durable staging write itself fails.
        {
            let dir = tempfile::tempdir().unwrap();
            let credentials = memory_credentials();
            let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
            fail_save_after(&store, 0);
            assert!(store.set_github_token("new-token".into()).is_err());
            assert_eq!(stored_github_token(&store), None);
            assert_eq!(credentials.entry_count(), 0);
        }

        // Secure storage rejects the staged value.
        {
            let dir = tempfile::tempdir().unwrap();
            let credentials = memory_credentials();
            let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
            credentials.fail_next(CredentialStoreOperation::Write);
            assert!(store.set_github_token("new-token".into()).is_err());
            assert_eq!(stored_github_token(&store), None);
            assert_eq!(credentials.entry_count(), 0);
            assert_eq!(
                store.snapshot().github.credential_transaction,
                None,
                "the staged journal is rolled back"
            );
        }

        // Staging and the secure write succeed, but activation fails.
        {
            let dir = tempfile::tempdir().unwrap();
            let credentials = memory_credentials();
            let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
            fail_save_after(&store, 1);
            assert!(store.set_github_token("new-token".into()).is_err());
            assert_eq!(stored_github_token(&store), None);
            assert_eq!(credentials.entry_count(), 0);
            assert_eq!(store.snapshot().github.credential_transaction, None);
        }
    }

    #[test]
    fn github_replace_and_clear_failures_preserve_the_previous_credential() {
        use crate::credential_store::CredentialStoreOperation;

        let run = |operation: &str| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let store = load_with_credentials(path.clone(), &credentials);
            store.set_github_token("old-token".into()).unwrap();
            let before = fs::read(&path).unwrap();

            match operation {
                "write" => credentials.fail_next(CredentialStoreOperation::Write),
                "activation" => fail_save_after(&store, 1),
                "cleanup" => credentials.fail_next(CredentialStoreOperation::Delete),
                "clear-commit" => fail_save_after(&store, 0),
                "clear-delete" => credentials.fail_next(CredentialStoreOperation::Delete),
                _ => unreachable!(),
            }
            let result = if operation.starts_with("clear") {
                store.set_github_token(String::new())
            } else {
                store.set_github_token("replacement".into())
            };
            assert!(result.is_err(), "{operation} unexpectedly succeeded");
            assert_eq!(stored_github_token(&store).as_deref(), Some("old-token"));
            assert_eq!(credentials.entry_count(), 1, "{operation}");
            assert_eq!(fs::read(&path).unwrap(), before, "{operation}");
            assert_eq!(store.snapshot().github.credential_transaction, None);
        };

        for operation in [
            "write",
            "activation",
            "cleanup",
            "clear-commit",
            "clear-delete",
        ] {
            run(operation);
        }
    }

    #[test]
    fn bitbucket_initial_save_rolls_back_each_pre_commit_failure() {
        use crate::credential_store::CredentialStoreOperation;

        let run = |failure: &str| {
            let dir = tempfile::tempdir().unwrap();
            let credentials = memory_credentials();
            let store = load_with_credentials(dir.path().join("settings.json"), &credentials);
            match failure {
                "staging" => fail_save_after(&store, 0),
                "username-write" => credentials.fail_next(CredentialStoreOperation::Write),
                "token-write" => credentials.fail_after(CredentialStoreOperation::Write, 1),
                "activation" => fail_save_after(&store, 1),
                _ => unreachable!(),
            }

            assert!(store
                .set_bitbucket_credentials("new-user".into(), "new-token".into())
                .is_err());
            assert_eq!(stored_bitbucket_pair(&store), None, "{failure}");
            assert_eq!(credentials.entry_count(), 0, "{failure}");
            assert_eq!(
                store.snapshot().bitbucket.credential_transaction,
                None,
                "{failure}"
            );
        };

        for failure in ["staging", "username-write", "token-write", "activation"] {
            run(failure);
        }
    }

    #[test]
    fn bitbucket_replace_and_clear_failures_preserve_the_previous_pair() {
        use crate::credential_store::CredentialStoreOperation;

        let run = |operation: &str| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let store = load_with_credentials(path.clone(), &credentials);
            store
                .set_bitbucket_credentials("old-user".into(), "old-token".into())
                .unwrap();
            let before = fs::read(&path).unwrap();

            match operation {
                "username-write" => credentials.fail_next(CredentialStoreOperation::Write),
                "token-write" => credentials.fail_after(CredentialStoreOperation::Write, 1),
                "activation" => fail_save_after(&store, 1),
                "first-cleanup-delete" => credentials.fail_next(CredentialStoreOperation::Delete),
                "second-cleanup-delete" => {
                    credentials.fail_after(CredentialStoreOperation::Delete, 1)
                }
                "clear-commit" => fail_save_after(&store, 0),
                "clear-first-delete" => credentials.fail_next(CredentialStoreOperation::Delete),
                "clear-second-delete" => {
                    credentials.fail_after(CredentialStoreOperation::Delete, 1)
                }
                _ => unreachable!(),
            }
            let result = if operation.starts_with("clear") {
                store.set_bitbucket_credentials(String::new(), String::new())
            } else {
                store.set_bitbucket_credentials(
                    "replacement-user".into(),
                    "replacement-token".into(),
                )
            };

            assert!(result.is_err(), "{operation} unexpectedly succeeded");
            assert_eq!(
                stored_bitbucket_pair(&store),
                Some(("old-user".into(), "old-token".into())),
                "{operation}"
            );
            assert_eq!(credentials.entry_count(), 2, "{operation}");
            assert_eq!(fs::read(&path).unwrap(), before, "{operation}");
            assert_eq!(store.snapshot().bitbucket.credential_transaction, None);
        };

        for operation in [
            "username-write",
            "token-write",
            "activation",
            "first-cleanup-delete",
            "second-cleanup-delete",
            "clear-commit",
            "clear-first-delete",
            "clear-second-delete",
        ] {
            run(operation);
        }
    }

    #[test]
    fn github_recovery_discards_pre_commit_staging_without_authorising_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let old = new_github_entry_id();
        let staged = new_github_entry_id();
        credentials.write(&old, "old-token").unwrap();
        credentials.write(&staged, "staged-token").unwrap();
        let mut settings = AppSettings::default();
        settings.github.active_secure_entry = Some(old.entry().to_string());
        settings.github.credential_transaction = Some(GithubCredentialTransaction::Staging {
            entry: staged.entry().to_string(),
        });
        write_settings(&path, &settings);
        let store = load_with_credentials(path.clone(), &credentials);

        // Even before recovery, resolution follows only the committed active id.
        assert_eq!(stored_github_token(&store).as_deref(), Some("old-token"));
        store.recover_credential_transactions().unwrap();

        assert_eq!(stored_github_token(&store).as_deref(), Some("old-token"));
        assert!(credentials.contains(&old));
        assert!(!credentials.contains(&staged));
        assert_eq!(credentials.entry_count(), 1);
        assert_eq!(read_settings(&path).github.credential_transaction, None);
    }

    #[test]
    fn github_recovery_preserves_post_commit_replace_and_clear() {
        // Replacement: the new active token survives and the retired one is
        // removed before an unrelated later mutation is applied.
        {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let retired = new_github_entry_id();
            let active = new_github_entry_id();
            credentials.write(&retired, "old-token").unwrap();
            credentials.write(&active, "new-token").unwrap();
            let mut settings = AppSettings::default();
            settings.github.active_secure_entry = Some(active.entry().to_string());
            settings.github.credential_transaction = Some(GithubCredentialTransaction::Cleaning {
                retired: vec![retired.entry().to_string()],
            });
            write_settings(&path, &settings);
            let store = load_with_credentials(path.clone(), &credentials);

            store.set_notifications_enabled(false).unwrap();

            assert_eq!(stored_github_token(&store).as_deref(), Some("new-token"));
            assert!(!credentials.contains(&retired));
            assert!(credentials.contains(&active));
            let durable = read_settings(&path);
            assert_eq!(durable.github.credential_transaction, None);
            assert!(!durable.notifications_enabled);
        }

        // Clear: no active id is resurrected while cleanup finishes.
        {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let retired = new_github_entry_id();
            credentials.write(&retired, "old-token").unwrap();
            let mut settings = AppSettings::default();
            settings.github.credential_transaction = Some(GithubCredentialTransaction::Cleaning {
                retired: vec![retired.entry().to_string()],
            });
            write_settings(&path, &settings);
            let store = load_with_credentials(path.clone(), &credentials);

            store.recover_credential_transactions().unwrap();

            assert_eq!(stored_github_token(&store), None);
            assert!(!credentials.contains(&retired));
            assert_eq!(read_settings(&path).github.credential_transaction, None);
        }
    }

    #[test]
    fn bitbucket_recovery_discards_pre_commit_pair_staging() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let (old_username, old_token) = new_bitbucket_entry_ids();
        let (staged_username, staged_token) = new_bitbucket_entry_ids();
        credentials.write(&old_username, "old-user").unwrap();
        credentials.write(&old_token, "old-token").unwrap();
        credentials.write(&staged_username, "staged-user").unwrap();
        credentials.write(&staged_token, "staged-token").unwrap();
        let active = BitbucketSecureEntries {
            username_entry: old_username.entry().to_string(),
            api_token_entry: old_token.entry().to_string(),
        };
        let staged = BitbucketSecureEntries {
            username_entry: staged_username.entry().to_string(),
            api_token_entry: staged_token.entry().to_string(),
        };
        let mut settings = AppSettings::default();
        settings.bitbucket.active_secure_entries = Some(active);
        settings.bitbucket.credential_transaction =
            Some(BitbucketCredentialTransaction::Staging { entries: staged });
        write_settings(&path, &settings);
        let store = load_with_credentials(path.clone(), &credentials);

        assert_eq!(
            stored_bitbucket_pair(&store),
            Some(("old-user".into(), "old-token".into()))
        );
        store.recover_credential_transactions().unwrap();

        assert_eq!(
            stored_bitbucket_pair(&store),
            Some(("old-user".into(), "old-token".into()))
        );
        assert!(credentials.contains(&old_username));
        assert!(credentials.contains(&old_token));
        assert!(!credentials.contains(&staged_username));
        assert!(!credentials.contains(&staged_token));
        assert_eq!(credentials.entry_count(), 2);
        assert_eq!(read_settings(&path).bitbucket.credential_transaction, None);
    }

    #[test]
    fn bitbucket_recovery_preserves_post_commit_replace_and_clear() {
        // Replacement.
        {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let (old_username, old_token) = new_bitbucket_entry_ids();
            let (new_username, new_token) = new_bitbucket_entry_ids();
            for (id, value) in [
                (&old_username, "old-user"),
                (&old_token, "old-token"),
                (&new_username, "new-user"),
                (&new_token, "new-token"),
            ] {
                credentials.write(id, value).unwrap();
            }
            let retired = BitbucketSecureEntries {
                username_entry: old_username.entry().to_string(),
                api_token_entry: old_token.entry().to_string(),
            };
            let active = BitbucketSecureEntries {
                username_entry: new_username.entry().to_string(),
                api_token_entry: new_token.entry().to_string(),
            };
            let mut settings = AppSettings::default();
            settings.bitbucket.active_secure_entries = Some(active);
            settings.bitbucket.credential_transaction =
                Some(BitbucketCredentialTransaction::Cleaning {
                    retired: vec![retired],
                });
            write_settings(&path, &settings);
            let store = load_with_credentials(path.clone(), &credentials);

            store.set_notifications_enabled(false).unwrap();

            assert_eq!(
                stored_bitbucket_pair(&store),
                Some(("new-user".into(), "new-token".into()))
            );
            assert!(!credentials.contains(&old_username));
            assert!(!credentials.contains(&old_token));
            assert_eq!(credentials.entry_count(), 2);
            assert_eq!(read_settings(&path).bitbucket.credential_transaction, None);
        }

        // Clear.
        {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            let credentials = memory_credentials();
            let (old_username, old_token) = new_bitbucket_entry_ids();
            credentials.write(&old_username, "old-user").unwrap();
            credentials.write(&old_token, "old-token").unwrap();
            let retired = BitbucketSecureEntries {
                username_entry: old_username.entry().to_string(),
                api_token_entry: old_token.entry().to_string(),
            };
            let mut settings = AppSettings::default();
            settings.bitbucket.credential_transaction =
                Some(BitbucketCredentialTransaction::Cleaning {
                    retired: vec![retired],
                });
            write_settings(&path, &settings);
            let store = load_with_credentials(path.clone(), &credentials);

            store.recover_credential_transactions().unwrap();

            assert_eq!(stored_bitbucket_pair(&store), None);
            assert_eq!(credentials.entry_count(), 0);
            assert_eq!(read_settings(&path).bitbucket.credential_transaction, None);
        }
    }

    #[test]
    fn failed_cleanup_recovery_keeps_non_secret_journal_for_retry() {
        use crate::credential_store::CredentialStoreOperation;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let credentials = memory_credentials();
        let retired = new_github_entry_id();
        credentials.write(&retired, "retired-token").unwrap();
        let mut settings = AppSettings::default();
        settings.github.credential_transaction = Some(GithubCredentialTransaction::Cleaning {
            retired: vec![retired.entry().to_string()],
        });
        write_settings(&path, &settings);
        let store = load_with_credentials(path.clone(), &credentials);
        credentials.fail_next(CredentialStoreOperation::Delete);

        assert!(store.recover_credential_transactions().is_err());

        assert!(credentials.contains(&retired));
        assert!(matches!(
            read_settings(&path).github.credential_transaction,
            Some(GithubCredentialTransaction::Cleaning { .. })
        ));
        store.recover_credential_transactions().unwrap();
        assert!(!credentials.contains(&retired));
        assert_eq!(read_settings(&path).github.credential_transaction, None);
    }

    /// Every slot must survive the round trip under the name the frontend
    /// sends; the tolerant `Deserialize` would otherwise land a misspelt known
    /// slot on the default without complaint.
    #[test]
    fn every_panel_position_round_trips_under_its_wire_name() {
        for (slot, wire) in [
            (PanelPosition::LeftTop, "left-top"),
            (PanelPosition::LeftBottom, "left-bottom"),
            (PanelPosition::RightTop, "right-top"),
            (PanelPosition::RightBottom, "right-bottom"),
        ] {
            let json = serde_json::to_string(&slot).unwrap();
            assert_eq!(json, format!("\"{wire}\""), "{slot:?} serializes to {wire}");
            assert_eq!(
                serde_json::from_str::<PanelPosition>(&json).unwrap(),
                slot,
                "{wire} deserializes back to {slot:?}"
            );
        }
        // A value that is not a string at all is the default slot too.
        assert_eq!(
            serde_json::from_str::<PanelPosition>("42").unwrap(),
            PanelPosition::LeftBottom
        );
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

    // ------------------------------------------------- review skip patterns

    fn patterns(list: &[&str]) -> Vec<String> {
        list.iter().map(|pattern| pattern.to_string()).collect()
    }

    /// The pattern `store`'s rule names `path` by.
    fn skipped_by(store: &SettingsStore, path: &str) -> Option<String> {
        store.skip_rule().first_match(&[path]).map(str::to_string)
    }

    /// `pull-request-viewer`: *The list starts empty*: a file from before the
    /// patterns reads as the empty list, with no pattern in error, and skips
    /// nothing, its neighbours kept; so does a store with no file at all.
    #[test]
    fn a_settings_file_without_skip_patterns_reads_the_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"notificationsEnabled": false, "commitHistoryEnabled": false}"#,
        )
        .unwrap();
        let fresh = SettingsStore::load(dir.path().join("fresh.json"));

        let store = SettingsStore::load(path);

        for read in [&store, &fresh] {
            assert_eq!(
                read.review_skip_patterns(),
                ReviewSkipPatterns {
                    patterns: Vec::new(),
                    errors: Vec::new(),
                }
            );
            for path in [
                "crates/core/tests/parse.rs",
                "src/app.test.ts",
                "src/lib.rs",
            ] {
                assert_eq!(skipped_by(read, path), None, "{path}");
            }
        }
        assert!(!store.snapshot().notifications_enabled, "neighbours kept");
        assert!(!store.commit_history_enabled(), "neighbours kept");
        assert!(AppSettings::default().review_skip_patterns.is_empty());
    }

    /// A stored list is trimmed, survives a restart, and is what the rule
    /// matches with.
    #[test]
    fn a_stored_list_is_trimmed_and_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        assert_eq!(
            store
                .set_review_skip_patterns(patterns(&[" docs/** ", "*.md"]))
                .unwrap(),
            [],
            "accepted, so nothing refused"
        );

        for read in [&store, &SettingsStore::load(path)] {
            assert_eq!(
                read.review_skip_patterns(),
                ReviewSkipPatterns {
                    patterns: patterns(&["docs/**", "*.md"]),
                    errors: Vec::new(),
                }
            );
            assert_eq!(
                skipped_by(read, "docs/guide.rs").as_deref(),
                Some("docs/**")
            );
            assert_eq!(skipped_by(read, "README.md").as_deref(), Some("*.md"));
            assert_eq!(skipped_by(read, "crates/core/tests/parse.rs"), None);
        }
    }

    /// `pull-request-viewer`: *An empty list skips nothing*: once every
    /// pattern is removed, the empty list is stored, as a list, and no file
    /// matches it.
    #[test]
    fn the_empty_list_is_stored_and_skips_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        store
            .set_review_skip_patterns(patterns(&["**/tests/**"]))
            .unwrap();
        assert_eq!(
            skipped_by(&store, "crates/core/tests/parse.rs").as_deref(),
            Some("**/tests/**")
        );

        assert_eq!(store.set_review_skip_patterns(Vec::new()).unwrap(), []);

        for read in [&store, &SettingsStore::load(path.clone())] {
            assert_eq!(read.review_skip_patterns().patterns, Vec::<String>::new());
            assert_eq!(skipped_by(read, "crates/core/tests/parse.rs"), None);
        }
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"reviewSkipPatterns\": []"), "{raw}");
    }

    /// `pull-request-viewer`: *A pattern that does not compile is refused on
    /// its field*, *A glob with a leading slash is refused*: the refusal
    /// names each refused pattern and why, and neither memory nor the file
    /// changes; with no file yet, none is written.
    #[test]
    fn a_refused_list_leaves_the_settings_and_the_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let fresh = SettingsStore::load(path.clone());
        let refused = fresh
            .set_review_skip_patterns(patterns(&["*.md", "/build/**"]))
            .unwrap();
        assert_eq!(
            refused
                .iter()
                .map(|error| (error.index, error.pattern.as_str()))
                .collect::<Vec<_>>(),
            [(1, "/build/**")]
        );
        assert!(!path.exists(), "nothing written");
        assert!(fresh.review_skip_patterns().patterns.is_empty());
        assert_eq!(skipped_by(&fresh, "README.md"), None);

        fresh
            .set_review_skip_patterns(patterns(&["docs/**"]))
            .unwrap();
        let before = fs::read(&path).unwrap();
        let refused = fresh
            .set_review_skip_patterns(patterns(&["/(unclosed/", "*.md"]))
            .unwrap();
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].index, 0);
        assert!(
            refused[0].reason.contains("does not compile"),
            "{}",
            refused[0].reason
        );
        assert_eq!(fs::read(&path).unwrap(), before, "the file is untouched");
        assert_eq!(
            fresh.review_skip_patterns().patterns,
            patterns(&["docs/**"])
        );
        assert_eq!(skipped_by(&fresh, "README.md"), None);
    }

    /// `pull-request-viewer`: *A hand-edited pattern that does not compile is
    /// ignored*: the others still match, and the getter reports it, by its
    /// place, for the Settings view to show on its field.
    #[test]
    fn a_hand_edited_pattern_that_does_not_compile_is_reported_and_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"reviewSkipPatterns": ["/(unclosed/", "**/tests/**"]}"#,
        )
        .unwrap();

        let store = SettingsStore::load(path);
        let read = store.review_skip_patterns();

        assert_eq!(read.patterns, patterns(&["/(unclosed/", "**/tests/**"]));
        assert_eq!(
            read.errors
                .iter()
                .map(|error| (error.index, error.pattern.as_str()))
                .collect::<Vec<_>>(),
            [(0, "/(unclosed/")]
        );
        assert_eq!(
            skipped_by(&store, "crates/core/tests/parse.rs").as_deref(),
            Some("**/tests/**")
        );
    }

    // ---- atomic, ordered saves and damaged files (atomic-settings-persistence)

    /// Every entry in `dir`, by file name, sorted.
    fn dir_entries(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn sample_identity() -> IdentityConfig {
        IdentityConfig {
            display_name: Some("Ada".into()),
            aliases: vec![Author {
                name: Some("Ada".into()),
                email: Some("ada@example.com".into()),
            }],
        }
    }

    #[test]
    fn a_missing_file_loads_the_defaults_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.load_outcome(), &SettingsLoad::Missing);
        assert_eq!(
            serde_json::to_value(store.snapshot()).unwrap(),
            serde_json::to_value(AppSettings::default()).unwrap()
        );
        assert_eq!(dir_entries(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_parsed_file_loads_as_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"notificationsEnabled": false}"#).unwrap();
        let store = SettingsStore::load(path);
        assert_eq!(store.load_outcome(), &SettingsLoad::Loaded);
        assert!(!store.snapshot().notifications_enabled);
    }

    #[test]
    fn a_truncated_file_is_moved_aside_and_the_next_save_writes_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let truncated = r#"{"notificationsEnabled": false, "github": {"tok"#;
        fs::write(&path, truncated).unwrap();

        let store = SettingsStore::load(path.clone());

        let backup = dir.path().join("settings.json.corrupt-0");
        assert!(
            matches!(
                store.load_outcome(),
                SettingsLoad::MovedAside { backup: b, reason } if *b == backup && reason.contains("EOF")
            ),
            "{:?}",
            store.load_outcome()
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), truncated);
        assert!(!path.exists(), "moved, not copied");
        assert!(store.snapshot().notifications_enabled, "defaults in use");

        store.set_identity(sample_identity()).unwrap();
        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.load_outcome(), &SettingsLoad::Loaded);
        assert_eq!(reloaded.identity(), sample_identity());
        assert_eq!(fs::read_to_string(&backup).unwrap(), truncated);
    }

    #[test]
    fn an_empty_file_and_a_whole_document_type_mismatch_are_moved_aside() {
        for (name, raw) in [
            ("empty", ""),
            ("array", "[1, 2]"),
            ("bad field", r#"{"notificationsEnabled": "yes"}"#),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            fs::write(&path, raw).unwrap();
            let store = SettingsStore::load(path);
            let backup = dir.path().join("settings.json.corrupt-0");
            assert!(
                matches!(store.load_outcome(), SettingsLoad::MovedAside { backup: b, .. } if *b == backup),
                "{name}: {:?}",
                store.load_outcome()
            );
            assert_eq!(fs::read_to_string(&backup).unwrap(), raw, "{name}");
        }
    }

    #[test]
    fn an_existing_backup_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(dir.path().join("settings.json.corrupt-0"), "first").unwrap();
        fs::write(&path, "{ second").unwrap();
        let store = SettingsStore::load(path);
        let expected = dir.path().join("settings.json.corrupt-1");
        assert!(
            matches!(store.load_outcome(), SettingsLoad::MovedAside { backup, .. } if *backup == expected),
            "{:?}",
            store.load_outcome()
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json.corrupt-0")).unwrap(),
            "first"
        );
    }

    #[test]
    fn tolerated_unknown_values_load_without_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "notificationsEnabled": false,
                "documentWidth": "ultrawide",
                "github": { "enabled": true, "panelPosition": "floating" }
            }"#,
        )
        .unwrap();
        let store = SettingsStore::load(path);
        assert_eq!(store.load_outcome(), &SettingsLoad::Loaded);
        let settings = store.snapshot();
        assert!(!settings.notifications_enabled);
        assert!(settings.github.enabled);
        assert_eq!(settings.github.panel_position, PanelPosition::default());
        assert_eq!(settings.document_width, DocumentWidth::default());
        assert_eq!(dir_entries(dir.path()), ["settings.json"]);
    }

    #[test]
    fn an_unreadable_file_is_left_in_place_and_never_saved_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::create_dir(&path).unwrap();
        let read_error = fs::read(&path).unwrap_err().to_string();

        let store = SettingsStore::load(path.clone());

        assert_eq!(
            store.load_outcome(),
            &SettingsLoad::Unreadable(read_error.clone())
        );
        assert!(path.is_dir(), "not moved");
        assert_eq!(dir_entries(dir.path()), ["settings.json"]);
        let err = store.set_github_enabled(true).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "{} could not be read at startup ({read_error}); SpecForge will not save over it",
                path.display()
            )
        );
        assert!(!store.github_enabled(), "the refused change is not stored");
    }

    #[test]
    fn a_held_store_leaves_a_regular_file_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = r#"{"github": {"enabled": false, "token": "ghp_secret"}}"#;
        fs::write(&path, original).unwrap();
        let store = SettingsStore::with(
            path.clone(),
            AppSettings::default(),
            SettingsLoad::Unreadable("permission denied".into()),
            Arc::new(crate::credential_store::InMemoryCredentialStore::default()),
        );

        let err = store.set_notifications_enabled(false).unwrap_err();
        assert!(
            err.to_string()
                .contains("(permission denied); SpecForge will not save over it"),
            "{err}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(dir_entries(dir.path()), ["settings.json"]);
        assert!(
            store.snapshot().notifications_enabled,
            "pre-call value kept"
        );
    }

    #[test]
    fn a_failed_save_stores_nothing_and_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());
        store.set_github_token("ghp_kept".into()).unwrap();
        let before = fs::read(&path).unwrap();

        // Wedge the rename: it refuses to replace a directory.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.set_github_token("ghp_attempted".into()).is_err());
        assert_eq!(stored_github_token(&store).as_deref(), Some("ghp_kept"));
        assert_eq!(
            dir_entries(dir.path()),
            ["settings.json", "settings.json.lock"],
            "only the stable lock sidecar exists"
        );

        // The favourites path returns a value; a failed save stores nothing there too.
        assert!(store
            .update_favorite_change_ids(vec!["c1".into()], Vec::new())
            .is_err());
        assert!(store.snapshot().favorite_change_ids.is_empty());

        fs::remove_dir(&path).unwrap();
        fs::write(&path, &before).unwrap();
        assert_eq!(
            store
                .update_favorite_change_ids(vec!["c1".into(), "c2".into()], Vec::new())
                .unwrap(),
            ["c1", "c2"]
        );
    }

    #[test]
    fn concurrent_setters_all_reach_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone());

        std::thread::scope(|scope| {
            let store = &store;
            scope.spawn(move || store.set_notifications_enabled(false).unwrap());
            scope.spawn(move || store.set_collapsed_tree_node_ids(vec!["c".into()]).unwrap());
            scope.spawn(move || store.set_expanded_tree_node_ids(vec!["e".into()]).unwrap());
            scope.spawn(move || store.set_document_width(DocumentWidth::Wide).unwrap());
            scope.spawn(move || store.set_commit_history_enabled(false).unwrap());
            scope.spawn(move || store.set_github_enabled(true).unwrap());
            scope.spawn(move || store.set_bitbucket_enabled(true).unwrap());
            scope.spawn(move || store.set_web_port(4999).unwrap());
        });

        let on_disk = SettingsStore::load(path).snapshot();
        assert_eq!(
            serde_json::to_value(&on_disk).unwrap(),
            serde_json::to_value(store.snapshot()).unwrap()
        );
        assert!(!on_disk.notifications_enabled);
        assert_eq!(on_disk.collapsed_tree_node_ids, ["c"]);
        assert_eq!(on_disk.expanded_tree_node_ids, ["e"]);
        assert_eq!(on_disk.document_width, DocumentWidth::Wide);
        assert!(!on_disk.commit_history_enabled);
        assert!(on_disk.github.enabled);
        assert!(on_disk.bitbucket.enabled);
        assert_eq!(on_disk.web.port, 4999);
    }

    /// `settings-view`: *Concurrent changes both reach disk*, made
    /// deterministic: setter A is parked inside its save, so setter B starts
    /// while A's save is in flight, and must wait for it.
    #[test]
    fn a_second_setter_waits_for_the_save_in_flight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = Arc::new(SettingsStore::load(path.clone()));
        let (arrived, release) = park_next_save(&store);

        let a = {
            let store = store.clone();
            std::thread::spawn(move || store.set_notifications_enabled(false))
        };
        arrived.recv_timeout(GENEROUS).expect("A reached its save");
        let (b_returned, b_done) = channel();
        let b = {
            let store = store.clone();
            std::thread::spawn(move || {
                let saved = store.set_github_enabled(true);
                let _ = b_returned.send(());
                saved
            })
        };

        // B queues behind A's save: it does not return, change memory or reach
        // disk. A broken build would let B through at once; a slow one can only
        // make this pass, never fail.
        assert_eq!(
            b_done.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout)
        );
        assert!(!store.github_enabled(), "B has not changed memory");
        assert!(store.snapshot().notifications_enabled, "A not stored yet");
        assert!(!path.exists(), "A is parked before its rename");

        release.send(()).unwrap();
        a.join().unwrap().unwrap();
        b.join().unwrap().unwrap();

        let on_disk = SettingsStore::load(path).snapshot();
        assert_eq!(
            serde_json::to_value(&on_disk).unwrap(),
            serde_json::to_value(store.snapshot()).unwrap()
        );
        assert!(!on_disk.notifications_enabled, "A's change reached disk");
        assert!(on_disk.github.enabled, "B's change reached disk");
    }

    /// `settings-view`: *Reading settings does not wait on a save*. A setter is
    /// parked inside its save; readers on another thread still answer, with
    /// the stored value rather than the one being saved.
    #[test]
    fn readers_do_not_wait_for_a_save_in_progress() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(SettingsStore::load(dir.path().join("settings.json")));
        let (arrived, release) = park_next_save(&store);
        let writer = {
            let store = store.clone();
            std::thread::spawn(move || store.set_github_enabled(true))
        };
        arrived
            .recv_timeout(GENEROUS)
            .expect("the setter reached its save");

        let (tx, rx) = channel();
        let reader = store.clone();
        std::thread::spawn(move || {
            let _ = tx.send((reader.snapshot().github.enabled, reader.github_enabled()));
        });
        let answered = rx.recv_timeout(GENEROUS);
        release.send(()).unwrap();

        assert_eq!(answered, Ok((false, false)));
        writer.join().unwrap().unwrap();
        assert!(store.github_enabled(), "stored once the save completed");
    }

    #[test]
    fn a_damaged_file_that_cannot_be_moved_aside_holds_the_store() {
        let dir = tempfile::tempdir().unwrap();
        // Every `.corrupt-<n>` sibling of a 250-byte name exceeds the 255-byte
        // file-name limit, so the move fails for real.
        let path = dir.path().join("s".repeat(250));
        fs::write(&path, "{ damaged").unwrap();

        let store = SettingsStore::load(path.clone());

        assert!(
            matches!(store.load_outcome(), SettingsLoad::Unreadable(reason) if reason.contains("could not be moved aside")),
            "{:?}",
            store.load_outcome()
        );
        assert!(store.set_notifications_enabled(false).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ damaged");
    }
}
