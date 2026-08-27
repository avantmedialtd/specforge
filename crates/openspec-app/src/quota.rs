//! Opt-in Claude Code usage-quota tracking.
//!
//! Ports the approach of the `claude-quota` menu-bar widget into the headless
//! app layer: resolve the local Claude Code OAuth token (read-only), query
//! Anthropic's usage endpoint, and expose the 5-hour and weekly utilization for
//! both frontends to render as a status-line gauge.
//!
//! Everything here is gated behind the `claude_quota_enabled` setting — with the
//! feature off, no token is read and no request is made. The token is only ever
//! sent to the official endpoint and is never written to logs, and is accessed
//! strictly read-only (never refreshed or rewritten, so it cannot log the user
//! out).
//!
//! The poll loop runs on a plain `std::thread` (like `AppService::spawn_backfill`)
//! so the app layer stays runtime-agnostic — it is consumed by both the Tauri
//! runtime and the terminal frontend's runtime.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openspec_core::{CacheEvent, WatcherManager};
use serde::Serialize;

use crate::settings::SettingsStore;
use crate::usage_http::{self, Verdict};

/// The official usage endpoint — the same one Claude Code's `/usage` screen
/// queries. Internal/undocumented, so responses are parsed defensively.
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// OAuth beta header Claude Code sends with the usage request.
const OAUTH_BETA: &str = "oauth-2025-04-20";
/// Poller wake cadence: how often the loop re-checks the enabled flag and
/// whether a refresh is due. Independent of the (longer) refresh interval, this
/// is what lets a Settings toggle take effect within a couple of seconds.
const TICK: Duration = Duration::from_secs(2);
/// Floor for the configurable refresh interval, so a tiny/zero setting can't
/// hammer the endpoint.
const MIN_REFRESH_SECS: u64 = 30;
/// Fallback backoff when a 429 carries no `Retry-After`.
const DEFAULT_BACKOFF_SECS: u64 = 300;

/// Status of the latest quota fetch — drives whether (and how) the gauge renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QuotaStatus {
    /// The feature is disabled — render nothing.
    Disabled,
    /// Enabled, but no usable token was found (missing or expired).
    Unauthenticated,
    /// Enabled, but usage could not be obtained or parsed.
    Unavailable,
    /// A usage snapshot is available.
    Ok,
}

/// One usage window: integer utilization percent plus an optional reset instant
/// (Unix epoch seconds) so each frontend can render a *live* countdown without
/// re-parsing a timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    /// Utilization, clamped to `0..=100`.
    pub utilization: u8,
    /// When the window resets, as Unix epoch seconds (`None` if unknown).
    pub resets_at_unix: Option<u64>,
}

/// One per-model scoped weekly window: a [`QuotaWindow`] plus the display name
/// of the model the limit is scoped to (e.g. "Fable"). Sourced from the usage
/// response's `limits` array, so the label rides with the data rather than being
/// hardcoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedQuotaWindow {
    /// The model this weekly limit is scoped to, by display name.
    pub model: String,
    /// Utilization, clamped to `0..=100`.
    pub utilization: u8,
    /// When the window resets, as Unix epoch seconds (`None` if unknown).
    pub resets_at_unix: Option<u64>,
}

/// The quota snapshot both frontends render. `stale` marks a cached snapshot
/// served after a transient failure, so the gauge can de-emphasize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuotaState {
    pub status: QuotaStatus,
    pub stale: bool,
    pub five_hour: Option<QuotaWindow>,
    pub seven_day: Option<QuotaWindow>,
    /// Per-model scoped weekly windows (e.g. Fable), each labeled by its model
    /// display name. Empty when the response carries no scoped limits.
    #[serde(default)]
    pub scoped: Vec<ScopedQuotaWindow>,
}

impl ClaudeQuotaState {
    /// The initial / disabled snapshot: nothing to show.
    pub fn disabled() -> Self {
        Self {
            status: QuotaStatus::Disabled,
            stale: false,
            five_hour: None,
            seven_day: None,
            scoped: Vec::new(),
        }
    }

    /// A snapshot carrying only a status (no windows).
    fn status_only(status: QuotaStatus) -> Self {
        Self {
            status,
            stale: false,
            five_hour: None,
            seven_day: None,
            scoped: Vec::new(),
        }
    }
}

/// Cheaply-cloneable handle to the latest quota snapshot, shared between the
/// poller (writer) and the frontends (readers).
#[derive(Clone)]
pub struct QuotaHandle(Arc<Mutex<ClaudeQuotaState>>);

impl QuotaHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ClaudeQuotaState::disabled())))
    }

    /// The current snapshot.
    pub fn get(&self) -> ClaudeQuotaState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: ClaudeQuotaState) {
        *self.0.lock().unwrap() = state;
    }
}

impl Default for QuotaHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---- credential resolution (read-only) ----

/// The active account's Claude Code config directory: honor `CLAUDE_CONFIG_DIR`,
/// else `~/.claude`.
fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Read the OAuth access token without modifying any store. Tries the
/// credentials file first (Linux/Windows, and macOS when present), then the
/// macOS Keychain. Returns `None` when no usable (present, unexpired) token can
/// be resolved. The token value is never logged.
fn resolve_token() -> Option<String> {
    let dir = claude_config_dir()?;
    token_from_file(&dir).or_else(|| token_from_keychain(&dir))
}

/// Parse `<dir>/.credentials.json` → `claudeAiOauth.accessToken`, rejecting an
/// expired token. Read-only; never writes.
fn token_from_file(dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(dir.join(".credentials.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    token_from_oauth(json.get("claudeAiOauth")?)
}

/// Extract a non-expired access token from a `claudeAiOauth` JSON object.
/// `expiresAt` is epoch milliseconds; a token at or past expiry is rejected.
fn token_from_oauth(oauth: &serde_json::Value) -> Option<String> {
    if let Some(expires_ms) = oauth.get("expiresAt").and_then(serde_json::Value::as_u64) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if expires_ms <= now_ms {
            return None;
        }
    }
    oauth
        .get("accessToken")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// macOS Keychain fallback: read the Claude Code credentials item read-only via
/// the `security` CLI (never writing or refreshing it). `None` off macOS, when
/// the item is absent, or for a non-default config dir (whose path-hashed
/// service name is out of scope for v1 — the credentials file covers it).
#[cfg(target_os = "macos")]
fn token_from_keychain(dir: &Path) -> Option<String> {
    let service = keychain_service(dir)?;
    let out = std::process::Command::new("security")
        .args(["find-generic-password", "-s", &service, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8(out.stdout).ok()?;
    let json: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    token_from_oauth(json.get("claudeAiOauth")?)
}

#[cfg(not(target_os = "macos"))]
fn token_from_keychain(_dir: &Path) -> Option<String> {
    None
}

/// The Keychain service name Claude Code uses for the *default* `~/.claude`
/// account. A custom `CLAUDE_CONFIG_DIR` uses a path-hashed suffix upstream;
/// that is out of scope for v1, so we return `None` and rely on the file.
#[cfg(target_os = "macos")]
fn keychain_service(dir: &Path) -> Option<String> {
    if dir.file_name().map(|n| n == ".claude").unwrap_or(false) {
        Some("Claude Code-credentials".to_string())
    } else {
        None
    }
}

// ---- usage fetch + parse ----

/// Outcome of one usage fetch attempt.
enum FetchResult {
    /// A fresh snapshot with at least one window.
    Ok(ClaudeQuotaState),
    /// The endpoint rejected the token (401).
    Unauthenticated,
    /// 2xx but the body couldn't be parsed into any window.
    Unavailable,
    /// Rate-limited (429); back off for the hinted (or default) delay.
    RateLimited { retry_after: Option<u64> },
    /// Transient transport error (offline, timeout) — keep the last snapshot.
    Transient,
}

/// Fetch usage with the bearer token and map the response to a [`FetchResult`].
fn fetch_usage(token: &str) -> FetchResult {
    let resp = usage_http::get(USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", OAUTH_BETA)
        .header("Content-Type", "application/json")
        .call();
    // A transport error (offline, timeout, TLS) never reaches `classify` — it
    // has no status to classify — and is transient like any other: keep showing
    // the last known snapshot.
    let Ok(mut r) = resp else {
        return FetchResult::Transient;
    };
    let retry_after = r
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    match usage_http::classify(r.status().as_u16(), retry_after.as_deref()) {
        Verdict::Read => match r.body_mut().read_to_string() {
            Ok(body) => match parse_usage(&body) {
                Some(state) => FetchResult::Ok(state),
                None => FetchResult::Unavailable,
            },
            Err(_) => FetchResult::Unavailable,
        },
        Verdict::Unauthenticated => FetchResult::Unauthenticated,
        Verdict::RateLimited { retry_after } => FetchResult::RateLimited { retry_after },
        Verdict::Transient => FetchResult::Transient,
    }
}

/// Parse the (undocumented) usage response into windows, tolerant of missing
/// fields. Returns `None` when neither top-level window is present
/// (→ `Unavailable`). Per-model scoped weekly windows, when present, are read
/// from the response's `limits` array.
fn parse_usage(body: &str) -> Option<ClaudeQuotaState> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let five_hour = parse_window(json.get("five_hour"));
    let seven_day = parse_window(json.get("seven_day"));
    if five_hour.is_none() && seven_day.is_none() {
        return None;
    }
    Some(ClaudeQuotaState {
        status: QuotaStatus::Ok,
        stale: false,
        five_hour,
        seven_day,
        scoped: parse_scoped_windows(&json),
    })
}

/// One window object: `{ "utilization": <0..100>, "resets_at": <rfc3339> }`.
fn parse_window(v: Option<&serde_json::Value>) -> Option<QuotaWindow> {
    let obj = v?;
    let util = obj.get("utilization")?.as_f64()?;
    let utilization = util.round().clamp(0.0, 100.0) as u8;
    let resets_at_unix = obj
        .get("resets_at")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_rfc3339_to_unix);
    Some(QuotaWindow {
        utilization,
        resets_at_unix,
    })
}

/// Per-model scoped weekly windows, read from the response's `limits` array.
/// Each entry of kind `weekly_scoped` carries an integer `percent`, a
/// `resets_at` instant, and a `scope.model.display_name` label. Non-scoped
/// entries (`session`, `weekly_all`) — which duplicate the top-level windows —
/// are skipped, as are entries without a usable model name. `is_active` marks
/// the currently-binding limit, not display eligibility, so it is ignored here.
/// A missing or malformed `limits` array yields an empty list.
fn parse_scoped_windows(json: &serde_json::Value) -> Vec<ScopedQuotaWindow> {
    json.get("limits")
        .and_then(serde_json::Value::as_array)
        .map(|limits| limits.iter().filter_map(parse_scoped_window).collect())
        .unwrap_or_default()
}

/// One `weekly_scoped` limit → a labeled window. `None` for any other kind, a
/// missing/empty model display name, or a missing `percent`.
fn parse_scoped_window(entry: &serde_json::Value) -> Option<ScopedQuotaWindow> {
    if entry.get("kind").and_then(serde_json::Value::as_str) != Some("weekly_scoped") {
        return None;
    }
    let model = entry
        .get("scope")?
        .get("model")?
        .get("display_name")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let percent = entry.get("percent")?.as_f64()?;
    let utilization = percent.round().clamp(0.0, 100.0) as u8;
    let resets_at_unix = entry
        .get("resets_at")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_rfc3339_to_unix);
    Some(ScopedQuotaWindow {
        model,
        utilization,
        resets_at_unix,
    })
}

/// Parse an RFC-3339 timestamp to Unix epoch seconds (negative clamped to 0).
fn parse_rfc3339_to_unix(s: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp().max(0) as u64)
}

// ---- poll loop ----

/// Derive the next snapshot when a previous fetch failed transiently: keep an
/// existing `Ok` snapshot but mark it stale; otherwise show `Unavailable`.
fn degrade_to_stale(prev: &ClaudeQuotaState) -> ClaudeQuotaState {
    if prev.status == QuotaStatus::Ok {
        ClaudeQuotaState {
            stale: true,
            ..prev.clone()
        }
    } else {
        ClaudeQuotaState::status_only(QuotaStatus::Unavailable)
    }
}

/// Run the quota poll loop on the calling thread. Honors the enabled flag and
/// the refresh interval, caches the latest snapshot, and emits
/// `CacheEvent::QuotaUpdated` whenever the snapshot changes. Never issues a
/// request while disabled.
fn run_poller(settings: Arc<SettingsStore>, watcher: WatcherManager, handle: QuotaHandle) {
    let mut last_poll: Option<Instant> = None;
    let mut backoff_until: Option<Instant> = None;

    loop {
        if !settings.claude_quota_enabled() {
            // Idle: collapse to Disabled once (announcing so the gauge hides),
            // then keep sleeping without ever touching the network.
            if handle.get().status != QuotaStatus::Disabled {
                handle.set(ClaudeQuotaState::disabled());
                watcher.emit(CacheEvent::QuotaUpdated);
            }
            last_poll = None;
            backoff_until = None;
            std::thread::sleep(TICK);
            continue;
        }

        let refresh =
            Duration::from_secs(settings.claude_quota_refresh_secs().max(MIN_REFRESH_SECS));
        let now = Instant::now();
        let due = last_poll.is_none_or(|t| now.duration_since(t) >= refresh);
        let backed_off = backoff_until.is_some_and(|t| now < t);

        if due && !backed_off {
            let prev = handle.get();
            let next = match resolve_token() {
                None => ClaudeQuotaState::status_only(QuotaStatus::Unauthenticated),
                Some(token) => match fetch_usage(&token) {
                    FetchResult::Ok(state) => state,
                    FetchResult::Unauthenticated => {
                        ClaudeQuotaState::status_only(QuotaStatus::Unauthenticated)
                    }
                    FetchResult::Unavailable => {
                        ClaudeQuotaState::status_only(QuotaStatus::Unavailable)
                    }
                    FetchResult::RateLimited { retry_after } => {
                        let secs = retry_after.unwrap_or(DEFAULT_BACKOFF_SECS);
                        backoff_until = Some(now + Duration::from_secs(secs));
                        degrade_to_stale(&prev)
                    }
                    FetchResult::Transient => degrade_to_stale(&prev),
                },
            };
            last_poll = Some(now);
            if next != prev {
                handle.set(next);
                watcher.emit(CacheEvent::QuotaUpdated);
            }
        }

        std::thread::sleep(TICK);
    }
}

/// Spawn the quota poll loop on a background thread (mirroring
/// `AppService::spawn_backfill`). The thread lives for the process; while the
/// feature is disabled it only re-checks the flag and never reaches the network.
pub fn spawn_poller(settings: Arc<SettingsStore>, watcher: WatcherManager, handle: QuotaHandle) {
    std::thread::spawn(move || run_poller(settings, watcher, handle));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_windows() {
        let body = r#"{
            "five_hour": {"utilization": 62.4, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day": {"utilization": 18, "resets_at": "2030-01-08T00:00:00+00:00"},
            "seven_day_opus": {"utilization": 5}
        }"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(state.status, QuotaStatus::Ok);
        assert!(!state.stale);
        let five = state.five_hour.expect("five-hour window");
        assert_eq!(five.utilization, 62); // rounded from 62.4
        assert!(five.resets_at_unix.is_some());
        assert_eq!(state.seven_day.expect("weekly window").utilization, 18);
    }

    #[test]
    fn tolerates_a_missing_weekly_window() {
        let body = r#"{"five_hour": {"utilization": 100}}"#;
        let state = parse_usage(body).expect("should parse with one window");
        assert_eq!(state.five_hour.expect("five-hour").utilization, 100);
        assert!(state.seven_day.is_none());
    }

    #[test]
    fn unparseable_or_empty_response_is_unavailable() {
        assert!(parse_usage("not json").is_none());
        assert!(parse_usage("{}").is_none());
        assert!(parse_usage(r#"{"extra_usage": {"is_enabled": true}}"#).is_none());
    }

    #[test]
    fn clamps_out_of_range_utilization() {
        let body = r#"{"five_hour": {"utilization": 250}, "seven_day": {"utilization": -10}}"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(state.five_hour.unwrap().utilization, 100);
        assert_eq!(state.seven_day.unwrap().utilization, 0);
    }

    #[test]
    fn expired_token_is_rejected() {
        let oauth = serde_json::json!({"accessToken": "tok", "expiresAt": 1u64});
        assert!(token_from_oauth(&oauth).is_none());
    }

    #[test]
    fn unexpired_token_is_accepted() {
        // Far-future expiry (year ~2286 in epoch millis).
        let oauth = serde_json::json!({"accessToken": "tok", "expiresAt": 9_999_999_999_000u64});
        assert_eq!(token_from_oauth(&oauth).as_deref(), Some("tok"));
    }

    #[test]
    fn stale_keeps_ok_windows_but_flags_it() {
        let ok = ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(QuotaWindow {
                utilization: 40,
                resets_at_unix: Some(100),
            }),
            seven_day: None,
            scoped: vec![ScopedQuotaWindow {
                model: "Fable".to_string(),
                utilization: 59,
                resets_at_unix: Some(200),
            }],
        };
        let staled = degrade_to_stale(&ok);
        assert_eq!(staled.status, QuotaStatus::Ok);
        assert!(staled.stale);
        assert_eq!(staled.five_hour.unwrap().utilization, 40);
        // The stale snapshot carries the scoped windows through unchanged.
        assert_eq!(staled.scoped.len(), 1);
        assert_eq!(staled.scoped[0].model, "Fable");
        assert_eq!(staled.scoped[0].utilization, 59);

        // A previous non-Ok snapshot degrades to Unavailable.
        let disabled = ClaudeQuotaState::disabled();
        assert_eq!(degrade_to_stale(&disabled).status, QuotaStatus::Unavailable);
    }

    #[test]
    fn parses_scoped_weekly_window_from_limits() {
        // Shape mirrors the live response: pooled `session` / `weekly_all`
        // entries (which duplicate the top-level windows) plus a `weekly_scoped`
        // Fable entry. Only the scoped one becomes a scoped window.
        let body = r#"{
            "five_hour": {"utilization": 5, "resets_at": "2030-01-01T00:00:00Z"},
            "seven_day": {"utilization": 39, "resets_at": "2030-01-08T00:00:00Z"},
            "limits": [
                {"kind": "session", "group": "session", "percent": 5, "is_active": false, "scope": null},
                {"kind": "weekly_all", "group": "weekly", "percent": 39, "is_active": false, "scope": null},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 59,
                 "resets_at": "2030-01-08T00:00:00Z", "is_active": true,
                 "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null}}
            ]
        }"#;
        let state = parse_usage(body).expect("should parse");
        // session / weekly_all are NOT scoped windows.
        assert_eq!(state.scoped.len(), 1, "only the weekly_scoped entry");
        let fable = &state.scoped[0];
        assert_eq!(fable.model, "Fable");
        assert_eq!(fable.utilization, 59);
        assert!(fable.resets_at_unix.is_some());
        // The pooled windows still come from the top-level fields.
        assert_eq!(state.five_hour.expect("5h").utilization, 5);
        assert_eq!(state.seven_day.expect("wk").utilization, 39);
    }

    #[test]
    fn no_limits_array_yields_no_scoped_windows() {
        // Today's shape (no `limits` key) parses to an empty scoped list while
        // the top-level windows still populate.
        let body = r#"{
            "five_hour": {"utilization": 20},
            "seven_day": {"utilization": 30}
        }"#;
        let state = parse_usage(body).expect("should parse");
        assert!(state.scoped.is_empty());
        assert_eq!(state.five_hour.expect("5h").utilization, 20);
        assert_eq!(state.seven_day.expect("wk").utilization, 30);
    }

    #[test]
    fn scoped_entry_without_model_name_is_skipped() {
        // A `weekly_scoped` entry lacking a usable model display name is skipped
        // rather than panicking or producing an unlabeled window.
        let body = r#"{
            "five_hour": {"utilization": 10},
            "limits": [
                {"kind": "weekly_scoped", "group": "weekly", "percent": 42,
                 "scope": {"model": {"id": null, "display_name": ""}}},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 42, "scope": null}
            ]
        }"#;
        let state = parse_usage(body).expect("should parse on the 5h window");
        assert!(state.scoped.is_empty());
    }
}
