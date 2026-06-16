## ADDED Requirements

### Requirement: Opt-in quota tracking

The system SHALL provide a "Claude quota" feature that is disabled by default and controlled by a persisted `claude_quota_enabled` setting. Usage polling and the status-line gauge SHALL be active only while the setting is enabled.

#### Scenario: Disabled by default on first run
- **WHEN** a user opens SpecForge for the first time with no prior setting
- **THEN** the quota feature is off, no quota gauge is shown, and no usage request is made

#### Scenario: Enabling starts tracking
- **WHEN** the user enables the quota setting
- **THEN** the system begins polling the usage endpoint and renders the quota gauge once a result is available

#### Scenario: Disabling stops tracking
- **WHEN** the user disables the quota setting
- **THEN** the system stops polling and removes the quota gauge from both frontends

### Requirement: Local credential resolution

The system SHALL resolve the active account's Claude Code OAuth token from local storage without modifying it. It SHALL read the token from `~/.claude/.credentials.json` (or the active `CLAUDE_CONFIG_DIR`) where present, and SHALL fall back to the macOS Keychain on macOS. The token SHALL be accessed read-only — never refreshed, rewritten, or deleted.

#### Scenario: Token found in credentials file
- **WHEN** quota tracking is enabled and the active account's `.credentials.json` holds a valid, unexpired OAuth token
- **THEN** the system uses that token to query usage and never writes to the credentials store

#### Scenario: macOS Keychain fallback
- **WHEN** quota tracking is enabled on macOS and no credentials-file token is available but a Claude Code Keychain entry exists
- **THEN** the system reads the token from the Keychain read-only and uses it

#### Scenario: No usable token
- **WHEN** quota tracking is enabled but no token can be resolved because it is missing or expired
- **THEN** the system makes no usage request and the gauge shows an unauthenticated state prompting the user to sign in with Claude Code

### Requirement: Usage polling with caching and backoff

While enabled, the system SHALL poll Anthropic's usage endpoint on an interval governed by a persisted `claude_quota_refresh_secs` setting that defaults to 300 seconds. It SHALL cache the latest result, SHALL NOT keep more than one usage request in flight per interval, and SHALL honor an HTTP 429 `Retry-After` response by deferring the next request until the hinted delay elapses. Polling SHALL run off the UI thread and SHALL NOT block frontend rendering.

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

When the feature is enabled and a usage snapshot is available, the system SHALL render a status-line gauge in both the desktop and terminal frontends showing the 5-hour window utilization and the weekly window utilization. The gauge SHALL color each window green below 70%, orange at or above 70%, and red at or above 90% utilization. When a window is fully consumed, the gauge SHALL display a countdown to that window's reset in place of the percentage.

#### Scenario: Desktop gauge
- **WHEN** quota tracking is enabled and a snapshot is available
- **THEN** the desktop app shows a quota pill in the sidebar footer with 5-hour and weekly utilization bars colored by threshold

#### Scenario: Terminal gauge
- **WHEN** quota tracking is enabled and a snapshot is available
- **THEN** the TUI shows the same 5-hour and weekly utilization in its title bar, honoring its ASCII/emoji and color-depth fallbacks

#### Scenario: Exhausted window shows reset countdown
- **WHEN** a tracked window is at 100% utilization
- **THEN** the gauge shows a countdown to that window's reset time instead of the percentage

### Requirement: Graceful degradation

The system SHALL degrade quietly when quota data cannot be obtained, preferring the last known snapshot or an unobtrusive status over an error that disrupts the rest of the UI. A transient network failure SHALL NOT clear an existing snapshot. Because the usage endpoint is undocumented, an unexpected response shape SHALL be treated as an "unavailable" state rather than causing a crash.

#### Scenario: Offline keeps the last snapshot
- **WHEN** a usage request fails due to a transient network error and a previous snapshot exists
- **THEN** the gauge continues to show the last known snapshot, visually de-emphasized as stale

#### Scenario: Unexpected response shape
- **WHEN** the usage endpoint returns a response that cannot be parsed into the expected windows
- **THEN** the system shows an "unavailable" state and does not crash or block other features

### Requirement: Quota privacy and safety

The system SHALL transmit the OAuth token only to Anthropic's official usage endpoint and SHALL NOT send quota credentials or data to any other destination. The token value SHALL NOT be written to logs or diagnostic output. All quota network activity SHALL occur only while the feature is enabled.

#### Scenario: Token never logged
- **WHEN** the system reads and uses the OAuth token
- **THEN** the token value never appears in application logs or diagnostic output

#### Scenario: Only the official endpoint
- **WHEN** the system queries usage
- **THEN** the request targets only Anthropic's official usage endpoint and no third-party service
