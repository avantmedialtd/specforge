# chatgpt-quota Specification

## Purpose

Defines an opt-in ChatGPT usage-quota status line for SpecForge: resolving the Codex CLI's local OAuth credentials read-only from `auth.json`, polling ChatGPT's usage endpoint with caching and backoff, and rendering the primary (5-hour) and secondary (weekly) rate-limit windows as a labeled gauge in the desktop, web, and terminal frontends alongside — but independent of — the Claude quota gauge, disabled by default, degrading quietly when the feature is off, unauthenticated, or offline, never writing to the Codex CLI's session, and only ever transmitting credentials to the official endpoint.
## Requirements
### Requirement: Opt-in quota tracking

The system SHALL provide a "ChatGPT quota" feature that is disabled by default and controlled by a persisted `chatgpt_quota_enabled` setting, independent of the Claude quota setting. Usage polling and the status-line gauge SHALL be active only while the setting is enabled.

#### Scenario: Disabled by default on first run
- **WHEN** a user opens SpecForge for the first time with no prior setting
- **THEN** the ChatGPT quota feature is off, no ChatGPT gauge is shown, and no usage request is made

#### Scenario: Enabling starts tracking
- **WHEN** the user enables the ChatGPT quota setting
- **THEN** the system begins polling the usage endpoint and renders the ChatGPT gauge once a result is available

#### Scenario: Disabling stops tracking
- **WHEN** the user disables the ChatGPT quota setting
- **THEN** the system stops polling and removes the ChatGPT gauge from every frontend

#### Scenario: Independent of the Claude quota setting
- **WHEN** the ChatGPT quota setting is enabled while the Claude quota setting is disabled (or vice versa)
- **THEN** only the enabled provider's gauge is shown and only that provider's endpoint is polled

### Requirement: Local credential resolution

The system SHALL resolve the Codex CLI's OAuth credentials from `auth.json` in the Codex home directory (the `CODEX_HOME` environment variable when set, else `~/.codex`) without modifying the file. It SHALL use the file's OAuth access token, and SHALL treat the associated account id as optional — resolving it from the stored account id where present and otherwise from the access token's `chatgpt_account_id` claim, and proceeding without it when neither yields a value. It SHALL reject an access token whose JWT `exp` claim is at or past the current time by decoding the claim locally, and SHALL treat an `auth.json` that carries only an API key (no OAuth tokens) as unauthenticated. Because the Codex CLI can be configured to store credentials outside `auth.json`, an `auth.json` without OAuth tokens SHALL yield the unauthenticated state rather than an error. The file SHALL be accessed read-only — never refreshed, rewritten, or deleted.

#### Scenario: Token found in auth.json
- **WHEN** ChatGPT quota tracking is enabled and `auth.json` holds an unexpired OAuth access token and an account id
- **THEN** the system uses that token and account id to query usage and never writes to `auth.json`

#### Scenario: Missing account id still queries usage
- **WHEN** `auth.json` holds an unexpired OAuth access token but no resolvable account id
- **THEN** the system still issues the usage request, omitting the account-id header rather than treating the credentials as unusable

#### Scenario: Credentials held outside auth.json
- **WHEN** the Codex CLI is configured to store its credentials somewhere other than `auth.json`, leaving no OAuth tokens in the file
- **THEN** the system shows the unauthenticated state prompting the user to sign in with the Codex CLI, and does not report an error

#### Scenario: CODEX_HOME override honored
- **WHEN** the `CODEX_HOME` environment variable points at a non-default Codex home directory
- **THEN** the system resolves `auth.json` from that directory instead of `~/.codex`

#### Scenario: Expired token is rejected locally
- **WHEN** the stored access token's JWT `exp` claim is at or past the current time
- **THEN** the system makes no usage request and the gauge shows an unauthenticated state prompting the user to sign in with the Codex CLI

#### Scenario: API-key-only auth file
- **WHEN** `auth.json` exists but holds only an API key and no OAuth token
- **THEN** the system makes no usage request and the gauge shows the unauthenticated state

#### Scenario: No usable credentials
- **WHEN** ChatGPT quota tracking is enabled but no `auth.json` can be read
- **THEN** the system makes no usage request and the gauge shows the unauthenticated state

### Requirement: Usage polling with caching and backoff

While enabled, the system SHALL poll ChatGPT's usage endpoint on an interval governed by a persisted `chatgpt_quota_refresh_secs` setting that defaults to 60 seconds and is floored so a tiny or zero value cannot hammer the endpoint. It SHALL cache the latest result, SHALL NOT keep more than one usage request in flight per interval, and SHALL honor an HTTP 429 `Retry-After` response by deferring the next request until the hinted delay elapses. Polling SHALL run off the UI thread and SHALL NOT block frontend rendering.

#### Scenario: Periodic refresh
- **WHEN** the feature is enabled and the refresh interval elapses
- **THEN** the system issues at most one usage request and updates the cached snapshot on success

#### Scenario: Rate-limit backoff
- **WHEN** the usage endpoint responds with HTTP 429 and a Retry-After hint
- **THEN** the system serves the last cached snapshot and defers the next request until the hinted delay elapses

#### Scenario: No request while disabled
- **WHEN** the feature is disabled
- **THEN** the system issues no usage requests regardless of the refresh interval

### Requirement: Quota status-line gauge

When the feature is enabled and a usage snapshot is available, the system SHALL render a "ChatGPT"-labeled status-line gauge in the desktop, web, and terminal frontends, beside — and visually distinguishable from — any Claude quota gauge, showing the utilization of the primary (5-hour) and secondary (weekly) rate-limit windows sourced from the usage response's `rate_limit.primary_window` and `rate_limit.secondary_window`. The gauge SHALL use the same visual grammar as the *Quota status-line gauge* requirement in the `claude-quota` capability: green below 70%, orange at or above 70%, red at or above 90% utilization, and a countdown to the window's reset in place of the percentage when a window is fully consumed.

Windows of a recognised standard length SHALL be labeled with the same vocabulary as the Claude gauge — `wk` for a week-length window and `5h` for a five-hour one, each matched within a tolerance so a length that is not exactly 604800 or 18000 seconds still resolves — and any other length SHALL keep the label derived from its reported duration.

When a window's reset time is known, the gauge SHALL render the time axis (segment ticks and a live "now" marker) using the window length reported by the usage response (`limit_window_seconds`) rather than a hardcoded duration: segmented by hours for windows up to 24 hours ($$n = \mathrm{round}(secs/3600)$$ segments) and by days for longer windows ($$n = \mathrm{round}(secs/86400)$$ segments), falling back to a 5-hour primary and 7-day secondary length only when the response omits the window length. A window whose reset time is unknown SHALL render the unsegmented utilization bar with neither segments nor a marker.

The marker SHALL behave as it does for the Claude gauge: the elapsed fraction SHALL be derived from the window's reset time and length and SHALL be clamped to the window's bounds, and the marker SHALL advance between polls so the axis stays live. The utilization fill and the time marker are independent — the fill shows budget spent and the marker shows time elapsed — so the two together convey whether usage is running ahead of or behind the elapsed time.

Because the terminal title bar is width-constrained and renders provider groups side by side, the TUI gauge SHALL degrade group by group when the row is too narrow: it SHALL drop whole trailing provider groups until the remainder fits, preferring to show the Claude group over the ChatGPT group, and SHALL render no gauge only when even a single group cannot fit. Enabling the ChatGPT gauge SHALL NOT cause an already-visible Claude gauge to disappear at any terminal width.

#### Scenario: Desktop gauge
- **WHEN** ChatGPT quota tracking is enabled and a snapshot is available
- **THEN** the desktop app shows a "ChatGPT" strip in the sidebar footer with primary and secondary utilization bars colored by threshold

#### Scenario: Web gauge
- **WHEN** ChatGPT quota tracking is enabled and a snapshot is available in the web frontend
- **THEN** the web frontend shows the same "ChatGPT" strip as the desktop sidebar footer

#### Scenario: Terminal gauge
- **WHEN** ChatGPT quota tracking is enabled and a snapshot is available
- **THEN** the TUI shows a ChatGPT-labeled gauge group in its title bar, distinguishable from the Claude group, honoring the TUI's ASCII/emoji and color-depth fallbacks

#### Scenario: Both providers shown side by side
- **WHEN** both the Claude and ChatGPT quota features are enabled with snapshots available
- **THEN** each provider renders its own labeled gauge and neither replaces, hides, or restyles the other

#### Scenario: Exhausted window shows reset countdown
- **WHEN** a tracked window is at 100% utilization
- **THEN** the gauge shows a countdown to that window's reset time instead of the percentage

#### Scenario: Standard window lengths use the Claude gauge's labels
- **WHEN** the snapshot carries a week-length window and a five-hour window
- **THEN** the gauge labels them `wk` and `5h` respectively, matching the Claude gauge's vocabulary rather than a duration derived from the reported length

#### Scenario: Non-standard window length keeps the derived label
- **WHEN** a window's reported length matches neither a week nor five hours
- **THEN** the gauge labels it from its reported duration, as hours for windows up to 24 hours and days beyond

#### Scenario: Time axis derives from the reported window length
- **WHEN** a window's snapshot carries a server-reported window length and a reset time
- **THEN** the gauge segments that window's time axis from the reported length (hours up to 24 hours, days beyond) and positions the live "now" marker at the elapsed fraction of that length

#### Scenario: Missing window length falls back to standard durations
- **WHEN** a window's snapshot carries a reset time but no window length
- **THEN** the gauge assumes 5 hours for the primary window and 7 days for the secondary window

#### Scenario: Marker advances between polls
- **WHEN** a snapshot is displayed and time passes without a new usage poll
- **THEN** the "now" marker advances toward the window's reset to stay live, like the reset countdown

#### Scenario: Marker reflects pace against the fill
- **WHEN** a window's utilization fill is greater than its elapsed-time marker
- **THEN** the gauge visibly shows the fill extending past the marker, indicating usage is running ahead of the elapsed time

#### Scenario: Elapsed fraction is clamped
- **WHEN** a window's reset time has already passed or lies further out than its window length
- **THEN** the marker stays within the bar's bounds rather than being drawn outside it

#### Scenario: Narrow terminal drops the ChatGPT group first
- **WHEN** both provider gauges are enabled in the TUI and the title bar is too narrow to fit both groups
- **THEN** the ChatGPT group is dropped and the Claude group still renders, rather than both gauges disappearing

#### Scenario: Unknown reset time falls back to the plain bar
- **WHEN** a window's reset time is unknown
- **THEN** the gauge renders that window's unsegmented utilization bar with no segments and no marker

#### Scenario: Missing window is omitted
- **WHEN** the usage response carries only one of the two windows
- **THEN** the gauge renders the present window and omits the missing one

### Requirement: Graceful degradation

The system SHALL degrade quietly when ChatGPT quota data cannot be obtained, preferring the last known snapshot or an unobtrusive status over an error that disrupts the rest of the UI. A transient network failure SHALL NOT clear an existing snapshot. Because the usage endpoint is internal to the Codex CLI and undocumented, an unexpected response shape SHALL be treated as an "unavailable" state rather than causing a crash.

#### Scenario: Offline keeps the last snapshot
- **WHEN** a usage request fails due to a transient network error and a previous snapshot exists
- **THEN** the gauge continues to show the last known snapshot, visually de-emphasized as stale

#### Scenario: Unexpected response shape
- **WHEN** the usage endpoint returns a response that cannot be parsed into at least one rate-limit window
- **THEN** the system shows an "unavailable" state and does not crash or block other features

#### Scenario: One provider's failure leaves the other untouched
- **WHEN** the ChatGPT usage fetch is failing while the Claude quota feature is enabled and healthy
- **THEN** the Claude gauge renders normally and only the ChatGPT gauge shows its degraded state

### Requirement: Quota privacy and safety

The system SHALL transmit the ChatGPT OAuth token and account id only to ChatGPT's official usage endpoint and SHALL NOT send quota credentials or data to any other destination. The token value SHALL NOT be written to logs or diagnostic output. The system SHALL never write to `auth.json`, so it cannot invalidate or refresh the Codex CLI's session. All quota network activity SHALL occur only while the feature is enabled.

#### Scenario: Token never logged
- **WHEN** the system reads and uses the OAuth token
- **THEN** the token value never appears in application logs or diagnostic output

#### Scenario: Only the official endpoint
- **WHEN** the system queries usage
- **THEN** the request targets only ChatGPT's official usage endpoint and no third-party service

#### Scenario: Codex session never disturbed
- **WHEN** the system resolves credentials, including when the stored token is expired
- **THEN** `auth.json` is never written, refreshed, or deleted by SpecForge
