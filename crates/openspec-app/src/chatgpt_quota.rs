//! Opt-in ChatGPT (Codex CLI) usage-quota tracking.
//!
//! A deliberate structural twin of `crate::quota` (the Claude Code poller) —
//! see `design.md`'s Decision 1 for why this isn't a shared provider
//! abstraction: the two providers differ in credential resolution (a JSON
//! file plus optional Keychain vs. a JWT-only file), expiry semantics
//! (`expiresAt` millis vs. a JWT `exp` claim), payload shape
//! (`five_hour`/`seven_day` vs. `rate_limit.primary_window`/
//! `secondary_window`), and headers — leaving only the ~50-line poll loop
//! genuinely shared.
//!
//! Resolves the Codex CLI's local OAuth token (read-only, from
//! `$CODEX_HOME/auth.json`), queries ChatGPT's usage endpoint, and exposes
//! the primary (5-hour) and secondary (weekly) window utilization for both
//! frontends to render as a status-line gauge.
//!
//! One wire-format trap worth stating up front: each window's `used_percent`
//! reports the budget **remaining**, not the budget consumed. Comparing the
//! same weekly window, the Codex desktop app showed 92% used while this field
//! held 8. `parse_window` publishes the complement, so `utilization` means
//! consumed budget everywhere above this module — matching what
//! `crate::quota` publishes for Claude, whose endpoint reports consumption
//! directly. The asymmetry is confined to the two parsers.
//!
//! Everything here is gated behind the `chatgpt_quota_enabled` setting — with
//! the feature off, no file is read and no request is made. The token is only
//! ever sent to the official endpoint and is never written to logs, and is
//! accessed strictly read-only (never refreshed or rewritten — SpecForge can
//! never log the user out of Codex).
//!
//! The poll loop runs on a plain `std::thread` (like `quota::spawn_poller`)
//! so the app layer stays runtime-agnostic — it is consumed by both the Tauri
//! runtime and the terminal frontend's runtime.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openspec_core::{CacheEvent, WatcherManager};
use serde::Serialize;

use crate::quota::QuotaStatus;
use crate::settings::SettingsStore;

/// The usage endpoint the Codex CLI's own `/status` screen queries — internal
/// and undocumented, so responses are parsed defensively.
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
/// Identifies SpecForge to the (Cloudflare-fronted) endpoint; an unlabelled
/// client is more likely to be challenged (design.md, Risks).
const USER_AGENT: &str = concat!("SpecForge/", env!("CARGO_PKG_VERSION"));
/// Poller wake cadence: how often the loop re-checks the enabled flag and
/// whether a refresh is due. Independent of the (longer) refresh interval —
/// see `quota.rs`'s identical constant.
const TICK: Duration = Duration::from_secs(2);
/// Floor for the configurable refresh interval, so a tiny/zero setting can't
/// hammer the endpoint.
const MIN_REFRESH_SECS: u64 = 30;
/// Fallback backoff when a 429 carries no `Retry-After`.
const DEFAULT_BACKOFF_SECS: u64 = 300;
/// Network timeout for a single usage request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// One usage window: integer utilization percent, an optional reset instant
/// (Unix epoch seconds), and the server-reported window length in seconds
/// (`limit_window_seconds`) — unlike Claude's fixed 5h/7d windows, ChatGPT's
/// response states each window's actual length, so frontends derive the time
/// axis from `window_secs` instead of a hardcoded duration (falling back to
/// 5h/7d only when the server omits it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptQuotaWindow {
    /// Utilization, clamped to `0..=100`.
    pub utilization: u8,
    /// When the window resets, as Unix epoch seconds (`None` if unknown).
    pub resets_at_unix: Option<u64>,
    /// The window's length in seconds, from `limit_window_seconds`. `None`
    /// when the response omits it.
    pub window_secs: Option<u64>,
}

/// The quota snapshot both frontends render. `stale` marks a cached snapshot
/// served after a transient failure, so the gauge can de-emphasize it.
/// Reuses [`QuotaStatus`] from `crate::quota` — one status enum for both
/// providers (design.md, Decision 1), so no `openspec-core` changes are
/// needed to add this second gauge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptQuotaState {
    pub status: QuotaStatus,
    pub stale: bool,
    pub primary: Option<ChatGptQuotaWindow>,
    pub secondary: Option<ChatGptQuotaWindow>,
}

impl ChatGptQuotaState {
    /// The initial / disabled snapshot: nothing to show.
    pub fn disabled() -> Self {
        Self {
            status: QuotaStatus::Disabled,
            stale: false,
            primary: None,
            secondary: None,
        }
    }

    /// A snapshot carrying only a status (no windows).
    fn status_only(status: QuotaStatus) -> Self {
        Self {
            status,
            stale: false,
            primary: None,
            secondary: None,
        }
    }
}

/// Cheaply-cloneable handle to the latest ChatGPT quota snapshot, shared
/// between the poller (writer) and the frontends (readers). A twin of
/// `quota::QuotaHandle`.
#[derive(Clone)]
pub struct ChatGptQuotaHandle(Arc<Mutex<ChatGptQuotaState>>);

impl ChatGptQuotaHandle {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ChatGptQuotaState::disabled())))
    }

    /// The current snapshot.
    pub fn get(&self) -> ChatGptQuotaState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: ChatGptQuotaState) {
        *self.0.lock().unwrap() = state;
    }
}

impl Default for ChatGptQuotaHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---- credential resolution (read-only) ----

/// The Codex CLI's config home: honor `CODEX_HOME`, else `~/.codex`.
fn codex_home_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CODEX_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    dirs::home_dir().map(|h| h.join(".codex"))
}

/// Resolved OAuth credentials: the bearer access token plus an optional
/// account id. The token value is never logged.
struct Credentials {
    access_token: String,
    account_id: Option<String>,
}

/// Read `<codex home>/auth.json` read-only and resolve usable OAuth
/// credentials. `None` when the file can't be read/parsed, carries no OAuth
/// tokens (an API-key-only login), or the access token is expired. Never
/// writes to the file — see [`credentials_from_auth_json`] for the parse.
fn resolve_credentials() -> Option<Credentials> {
    let dir = codex_home_dir()?;
    let raw = std::fs::read_to_string(dir.join("auth.json")).ok()?;
    credentials_from_auth_json(&raw)
}

/// Parse `auth.json`'s contents into usable credentials — split out from
/// [`resolve_credentials`] so parsing is unit-testable without touching the
/// filesystem. The account id is resolved from the stored `tokens.account_id`
/// first, else the `chatgpt_account_id` claim inside `tokens.id_token`;
/// absent either way, credentials are still usable (the header is simply
/// omitted from the request).
fn credentials_from_auth_json(raw: &str) -> Option<Credentials> {
    let json: serde_json::Value = serde_json::from_str(raw).ok()?;
    let tokens = json.get("tokens")?;
    let access_token = tokens
        .get("access_token")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())?;
    if is_expired(access_token) {
        return None;
    }
    let account_id = tokens
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            tokens
                .get("id_token")
                .and_then(serde_json::Value::as_str)
                .and_then(chatgpt_account_id_claim)
        });
    Some(Credentials {
        access_token: access_token.to_string(),
        account_id,
    })
}

/// Whether an access token's JWT `exp` claim (Unix epoch seconds) is at or
/// past now. A token whose payload can't be decoded, or that carries no `exp`
/// claim, is treated as *not* expired — mirroring `quota.rs`'s Claude check,
/// which only rejects an expiry it can positively confirm.
fn is_expired(token: &str) -> bool {
    let Some(payload) = jwt_payload(token) else {
        return false;
    };
    let Some(exp) = payload.get("exp").and_then(serde_json::Value::as_u64) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    exp <= now
}

/// Extract the `chatgpt_account_id` claim from an id-token JWT.
fn chatgpt_account_id_claim(id_token: &str) -> Option<String> {
    jwt_payload(id_token)?
        .get("chatgpt_account_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Decode a JWT's payload segment (the middle `.`-delimited part) as JSON,
/// without verifying the signature — we are the client reading our own
/// token, not a verifier. `None` for a malformed token.
fn jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64_url_decode(payload)?;
    serde_json::from_slice(&bytes).ok()
}

/// Minimal base64url (RFC 4648 §5) decoder, tolerant of missing padding —
/// just enough to read a JWT segment locally. Hand-rolled rather than adding
/// a dependency for one decode (this change adds no new dependencies; see
/// design.md's Impact section).
fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    fn sextet(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4 + 3);
    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            return None; // malformed: a final group needs at least 2 chars
        }
        let mut sx = [0u8; 4];
        for (i, &b) in chunk.iter().enumerate() {
            sx[i] = sextet(b)?;
        }
        out.push((sx[0] << 2) | (sx[1] >> 4));
        if chunk.len() > 2 {
            out.push(((sx[1] & 0x0F) << 4) | (sx[2] >> 2));
        }
        if chunk.len() > 3 {
            out.push(((sx[2] & 0x03) << 6) | sx[3]);
        }
    }
    Some(out)
}

// ---- usage fetch + parse ----

/// Outcome of one usage fetch attempt.
enum FetchResult {
    /// A fresh snapshot with at least one window.
    Ok(ChatGptQuotaState),
    /// The endpoint rejected the token (401).
    Unauthenticated,
    /// 2xx but the body couldn't be parsed into any window.
    Unavailable,
    /// Rate-limited (429); back off for the hinted (or default) delay.
    RateLimited { retry_after: Option<u64> },
    /// Transient transport error (offline, timeout) — keep the last snapshot.
    Transient,
}

/// Fetch usage with the bearer token and map the response to a
/// [`FetchResult`]. `account_id` is sent as `ChatGPT-Account-Id` only when
/// resolved — the Codex CLI itself omits the header rather than failing when
/// it's absent (design.md, Decision 4).
fn fetch_usage(access_token: &str, account_id: Option<&str>) -> FetchResult {
    let mut req = ureq::get(USAGE_URL)
        .timeout(REQUEST_TIMEOUT)
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("User-Agent", USER_AGENT)
        .set("Content-Type", "application/json");
    if let Some(id) = account_id {
        req = req.set("ChatGPT-Account-Id", id);
    }
    match req.call() {
        Ok(r) => match r.into_string() {
            Ok(body) => match parse_usage(&body) {
                Some(state) => FetchResult::Ok(state),
                None => FetchResult::Unavailable,
            },
            Err(_) => FetchResult::Unavailable,
        },
        Err(ureq::Error::Status(401, _)) => FetchResult::Unauthenticated,
        Err(ureq::Error::Status(429, resp)) => FetchResult::RateLimited {
            retry_after: resp
                .header("Retry-After")
                .and_then(|h| h.trim().parse::<u64>().ok()),
        },
        // Other HTTP statuses and transport errors are transient from the
        // gauge's point of view: keep showing the last known snapshot.
        Err(_) => FetchResult::Transient,
    }
}

/// Parse the (undocumented) usage response into windows, tolerant of missing
/// fields. Returns `None` when neither `rate_limit.primary_window` nor
/// `secondary_window` is present (→ `Unavailable`).
fn parse_usage(body: &str) -> Option<ChatGptQuotaState> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let rate_limit = json.get("rate_limit");
    let primary = rate_limit.and_then(|rl| parse_window(rl.get("primary_window")));
    let secondary = rate_limit.and_then(|rl| parse_window(rl.get("secondary_window")));
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(ChatGptQuotaState {
        status: QuotaStatus::Ok,
        stale: false,
        primary,
        secondary,
    })
}

/// One window object: `{ "used_percent": <0..100>, "reset_at": <unix-secs>,
/// "limit_window_seconds": <secs> }`.
///
/// Despite its name on the wire, `used_percent` carries the budget **remaining**
/// in the window rather than the budget consumed: comparing the same weekly
/// window, the Codex desktop app reported 92% used while this field held 8. The
/// utilization we publish is therefore its complement, so every surface shows
/// consumed budget — the same meaning the Claude gauge's utilization carries.
/// See the module header and `design.md` for the evidence.
fn parse_window(v: Option<&serde_json::Value>) -> Option<ChatGptQuotaWindow> {
    let obj = v?;
    let remaining = obj.get("used_percent")?.as_f64()?;
    let utilization = (100.0 - remaining).round().clamp(0.0, 100.0) as u8;
    let resets_at_unix = obj.get("reset_at").and_then(serde_json::Value::as_u64);
    let window_secs = obj
        .get("limit_window_seconds")
        .and_then(serde_json::Value::as_u64);
    Some(ChatGptQuotaWindow {
        utilization,
        resets_at_unix,
        window_secs,
    })
}

// ---- poll loop ----

/// Derive the next snapshot when a previous fetch failed transiently: keep an
/// existing `Ok` snapshot but mark it stale; otherwise show `Unavailable`.
fn degrade_to_stale(prev: &ChatGptQuotaState) -> ChatGptQuotaState {
    if prev.status == QuotaStatus::Ok {
        ChatGptQuotaState {
            stale: true,
            ..prev.clone()
        }
    } else {
        ChatGptQuotaState::status_only(QuotaStatus::Unavailable)
    }
}

/// Run the quota poll loop on the calling thread. Honors the enabled flag and
/// the refresh interval, caches the latest snapshot, and emits the existing
/// `CacheEvent::QuotaUpdated` whenever the snapshot changes — the same event
/// the Claude poller emits (design.md, Decision 2); every consumer re-reads
/// the snapshot it cares about, so a spurious cross-provider re-read is
/// harmless. Never issues a request while disabled.
fn run_poller(settings: Arc<SettingsStore>, watcher: WatcherManager, handle: ChatGptQuotaHandle) {
    let mut last_poll: Option<Instant> = None;
    let mut backoff_until: Option<Instant> = None;

    loop {
        if !settings.chatgpt_quota_enabled() {
            // Idle: collapse to Disabled once (announcing so the gauge hides),
            // then keep sleeping without ever touching the network.
            if handle.get().status != QuotaStatus::Disabled {
                handle.set(ChatGptQuotaState::disabled());
                watcher.emit(CacheEvent::QuotaUpdated);
            }
            last_poll = None;
            backoff_until = None;
            std::thread::sleep(TICK);
            continue;
        }

        let refresh =
            Duration::from_secs(settings.chatgpt_quota_refresh_secs().max(MIN_REFRESH_SECS));
        let now = Instant::now();
        let due = last_poll.is_none_or(|t| now.duration_since(t) >= refresh);
        let backed_off = backoff_until.is_some_and(|t| now < t);

        if due && !backed_off {
            let prev = handle.get();
            let next = match resolve_credentials() {
                None => ChatGptQuotaState::status_only(QuotaStatus::Unauthenticated),
                Some(creds) => {
                    match fetch_usage(&creds.access_token, creds.account_id.as_deref()) {
                        FetchResult::Ok(state) => state,
                        FetchResult::Unauthenticated => {
                            ChatGptQuotaState::status_only(QuotaStatus::Unauthenticated)
                        }
                        FetchResult::Unavailable => {
                            ChatGptQuotaState::status_only(QuotaStatus::Unavailable)
                        }
                        FetchResult::RateLimited { retry_after } => {
                            let secs = retry_after.unwrap_or(DEFAULT_BACKOFF_SECS);
                            backoff_until = Some(now + Duration::from_secs(secs));
                            degrade_to_stale(&prev)
                        }
                        FetchResult::Transient => degrade_to_stale(&prev),
                    }
                }
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

/// Spawn the ChatGPT quota poll loop on a background thread (mirroring
/// `quota::spawn_poller`). The thread lives for the process; while the
/// feature is disabled it only re-checks the flag and never reaches the
/// network.
pub fn spawn_poller(
    settings: Arc<SettingsStore>,
    watcher: WatcherManager,
    handle: ChatGptQuotaHandle,
) {
    std::thread::spawn(move || run_poller(settings, watcher, handle));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- base64url / JWT test helpers ----

    fn base64_url_encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied();
            let b2 = chunk.get(2).copied();
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            let c1 = ((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4);
            out.push(ALPHABET[c1 as usize] as char);
            if let Some(byte1) = b1 {
                let c2 = ((byte1 & 0x0F) << 2) | (b2.unwrap_or(0) >> 6);
                out.push(ALPHABET[c2 as usize] as char);
            }
            if let Some(byte2) = b2 {
                out.push(ALPHABET[(byte2 & 0x3F) as usize] as char);
            }
        }
        out
    }

    /// A syntactically-valid JWT (`header.payload.signature`) carrying an
    /// arbitrary payload — the header and signature are never decoded by this
    /// module, so their content doesn't matter.
    fn make_jwt(payload_json: &str) -> String {
        format!("h.{}.s", base64_url_encode(payload_json.as_bytes()))
    }

    #[test]
    fn base64_url_round_trips() {
        // "{}" is a known short encoding, computed by hand: 2 bytes → 3
        // chars, no padding.
        assert_eq!(base64_url_encode(b"{}"), "e30");
        assert_eq!(base64_url_decode("e30").unwrap(), b"{}");
        // Round trip a longer, non-boundary-aligned payload too.
        let payload = br#"{"exp":123,"chatgpt_account_id":"acct-1"}"#;
        let encoded = base64_url_encode(payload);
        assert_eq!(base64_url_decode(&encoded).unwrap(), payload);
    }

    #[test]
    fn parses_both_windows() {
        let body = r#"{
            "rate_limit": {
                "primary_window": {"used_percent": 62.4, "reset_at": 2000000000, "limit_window_seconds": 18000},
                "secondary_window": {"used_percent": 18, "reset_at": 2000600000, "limit_window_seconds": 604800}
            }
        }"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(state.status, QuotaStatus::Ok);
        assert!(!state.stale);
        // The wire value is budget *remaining*, so utilization is its
        // complement: 62.4 remaining → 37.6 consumed → 38 after rounding.
        let primary = state.primary.expect("primary window");
        assert_eq!(primary.utilization, 38);
        assert_eq!(primary.resets_at_unix, Some(2_000_000_000));
        assert_eq!(primary.window_secs, Some(18_000));
        let secondary = state.secondary.expect("secondary window");
        assert_eq!(secondary.utilization, 82); // 18 remaining → 82 consumed
        assert_eq!(secondary.window_secs, Some(604_800));
    }

    #[test]
    fn tolerates_a_missing_secondary_window() {
        let body = r#"{"rate_limit": {"primary_window": {"used_percent": 100}}}"#;
        let state = parse_usage(body).expect("should parse with one window");
        // 100 remaining → nothing consumed yet.
        assert_eq!(state.primary.expect("primary").utilization, 0);
        assert!(state.secondary.is_none());
    }

    #[test]
    fn reported_percent_is_remaining_not_consumed() {
        // The field the endpoint calls `used_percent` holds what is LEFT: the
        // real-world observation that drove this mapping was the Codex desktop
        // app reporting a weekly window as 92% used while this field held 8.
        let body = r#"{"rate_limit": {
            "primary_window": {"used_percent": 8, "limit_window_seconds": 604800}
        }}"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(
            state.primary.expect("primary").utilization,
            92,
            "8 remaining must display as 92% consumed, matching the Codex app"
        );
    }

    #[test]
    fn an_exhausted_window_reads_as_fully_consumed() {
        // Nothing left → the gauge must show a spent window (and its countdown),
        // not an idle one.
        let body = r#"{"rate_limit": {"primary_window": {"used_percent": 0}}}"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(state.primary.expect("primary").utilization, 100);
    }

    #[test]
    fn unparseable_or_empty_response_is_unavailable() {
        assert!(parse_usage("not json").is_none());
        assert!(parse_usage("{}").is_none());
        assert!(parse_usage(r#"{"rate_limit": {}}"#).is_none());
        assert!(parse_usage(r#"{"plan_type": "plus"}"#).is_none());
    }

    #[test]
    fn clamps_out_of_range_utilization() {
        // Out-of-range remaining values still land inside 0..=100 after the
        // complement: 250 remaining → -150 consumed → 0; -10 → 110 → 100.
        let body = r#"{"rate_limit": {
            "primary_window": {"used_percent": 250},
            "secondary_window": {"used_percent": -10}
        }}"#;
        let state = parse_usage(body).expect("should parse");
        assert_eq!(state.primary.unwrap().utilization, 0);
        assert_eq!(state.secondary.unwrap().utilization, 100);
    }

    #[test]
    fn missing_limit_window_seconds_yields_none() {
        let body = r#"{"rate_limit": {"primary_window": {"used_percent": 5, "reset_at": 100}}}"#;
        let state = parse_usage(body).expect("should parse");
        let primary = state.primary.expect("primary");
        assert_eq!(primary.resets_at_unix, Some(100));
        assert_eq!(primary.window_secs, None);
    }

    #[test]
    fn expired_access_token_is_rejected() {
        assert!(is_expired(&make_jwt(r#"{"exp": 1}"#)));
    }

    #[test]
    fn unexpired_access_token_is_accepted() {
        assert!(!is_expired(&make_jwt(r#"{"exp": 9999999999}"#)));
    }

    #[test]
    fn token_without_exp_claim_is_not_rejected() {
        assert!(!is_expired(&make_jwt(r#"{"sub": "abc"}"#)));
    }

    #[test]
    fn malformed_token_is_not_rejected() {
        assert!(!is_expired("not-a-jwt"));
    }

    #[test]
    fn api_key_only_auth_file_has_no_credentials() {
        let raw = r#"{"OPENAI_API_KEY": "sk-live-abc"}"#;
        assert!(credentials_from_auth_json(raw).is_none());
    }

    #[test]
    fn unreadable_auth_file_has_no_credentials() {
        assert!(credentials_from_auth_json("not json").is_none());
        assert!(credentials_from_auth_json("{}").is_none());
    }

    #[test]
    fn expired_token_in_auth_file_has_no_credentials() {
        let expired = make_jwt(r#"{"exp": 1}"#);
        let raw = format!(r#"{{"tokens": {{"access_token": "{expired}"}}}}"#);
        assert!(credentials_from_auth_json(&raw).is_none());
    }

    #[test]
    fn missing_account_id_still_yields_usable_credentials() {
        let token = make_jwt(r#"{"exp": 9999999999}"#);
        let raw = format!(r#"{{"tokens": {{"access_token": "{token}"}}}}"#);
        let creds = credentials_from_auth_json(&raw).expect("token alone is usable");
        assert_eq!(creds.access_token, token);
        assert!(creds.account_id.is_none());
    }

    #[test]
    fn stored_account_id_is_preferred_over_the_jwt_claim() {
        let token = make_jwt(r#"{"exp": 9999999999}"#);
        let id_token = make_jwt(r#"{"chatgpt_account_id": "from-claim"}"#);
        let raw = format!(
            r#"{{"tokens": {{"access_token": "{token}", "account_id": "from-store", "id_token": "{id_token}"}}}}"#
        );
        let creds = credentials_from_auth_json(&raw).expect("should resolve");
        assert_eq!(creds.account_id.as_deref(), Some("from-store"));
    }

    #[test]
    fn falls_back_to_the_id_token_claim_when_account_id_is_absent() {
        let token = make_jwt(r#"{"exp": 9999999999}"#);
        let id_token = make_jwt(r#"{"chatgpt_account_id": "from-claim"}"#);
        let raw =
            format!(r#"{{"tokens": {{"access_token": "{token}", "id_token": "{id_token}"}}}}"#);
        let creds = credentials_from_auth_json(&raw).expect("should resolve");
        assert_eq!(creds.account_id.as_deref(), Some("from-claim"));
    }

    #[test]
    fn stale_keeps_ok_windows_but_flags_it() {
        let ok = ChatGptQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            primary: Some(ChatGptQuotaWindow {
                utilization: 40,
                resets_at_unix: Some(100),
                window_secs: Some(18_000),
            }),
            secondary: None,
        };
        let staled = degrade_to_stale(&ok);
        assert_eq!(staled.status, QuotaStatus::Ok);
        assert!(staled.stale);
        assert_eq!(staled.primary.unwrap().utilization, 40);

        let disabled = ChatGptQuotaState::disabled();
        assert_eq!(degrade_to_stale(&disabled).status, QuotaStatus::Unavailable);
    }
}
