## MODIFIED Requirements

### Requirement: Quota status-line gauge

When the feature is enabled and a usage snapshot is available, the system SHALL render a status-line gauge in both the desktop and terminal frontends showing the 5-hour window utilization and the weekly window utilization. The gauge SHALL color each window green below 70%, orange at or above 70%, and red at or above 90% utilization. When a window is fully consumed, the gauge SHALL display a countdown to that window's reset in place of the percentage. The pooled 5-hour and weekly windows SHALL be sourced from the usage response's top-level `five_hour` and `seven_day` windows.

When a window's reset time is known, the gauge SHALL additionally render a time axis over the utilization fill: the 5-hour window divided into 5 equal hour segments and the weekly window divided into 7 equal day segments, with a live "now" marker positioned at the fraction of the window's fixed duration (5 hours / 7 days) that has elapsed. The elapsed fraction SHALL be derived from the window's reset time and fixed length and SHALL be clamped to the window's bounds, and the marker SHALL advance between polls. The utilization fill and the time marker are independent: the fill shows budget spent and the marker shows time elapsed, so the two together convey whether usage is ahead of or behind the elapsed time. When a window's reset time is unknown, the gauge SHALL render the unsegmented utilization bar with neither segments nor a marker.

When the usage snapshot includes per-model scoped weekly limits — the usage response's `limits` entries of kind `weekly_scoped`, each carrying a model display name, a utilization percent, and a reset time — the gauge SHALL render one additional weekly window per scoped model in both frontends, labeled by that model's display name. Each per-model window SHALL use the same weekly rendering as the general weekly window: 7 day segments, the live "now" marker (when its reset time is known), the green/orange/red thresholds, and the exhausted-window reset countdown. A scoped window SHALL be shown whenever its limit is present in the snapshot, independent of any "active" flag the limit carries. The pooled 5-hour and weekly windows are unaffected by the presence or absence of scoped windows, and when no scoped weekly limits are present the gauge SHALL render only the 5-hour and weekly windows.

#### Scenario: Desktop gauge
- **WHEN** quota tracking is enabled and a snapshot is available
- **THEN** the desktop app shows a quota pill in the sidebar footer with 5-hour and weekly utilization bars colored by threshold

#### Scenario: Terminal gauge
- **WHEN** quota tracking is enabled and a snapshot is available
- **THEN** the TUI shows the same 5-hour and weekly utilization in its title bar, honoring its ASCII/emoji and color-depth fallbacks

#### Scenario: Exhausted window shows reset countdown
- **WHEN** a tracked window is at 100% utilization
- **THEN** the gauge shows a countdown to that window's reset time instead of the percentage

#### Scenario: Five-hour window shows hour segments and a now marker
- **WHEN** the 5-hour window has a known reset time
- **THEN** the gauge divides that window's bar into 5 hour segments and draws a "now" marker at the elapsed fraction of the 5-hour window, over the utilization fill

#### Scenario: Weekly window shows day segments and a now marker
- **WHEN** the weekly window has a known reset time
- **THEN** the gauge divides that window's bar into 7 day segments and draws a "now" marker at the elapsed fraction of the 7-day window, over the utilization fill

#### Scenario: Marker reflects pace against the fill
- **WHEN** a window's utilization fill is greater than its elapsed-time marker
- **THEN** the gauge visibly shows the fill extending past the marker, indicating usage is running ahead of the elapsed time

#### Scenario: Marker advances between polls
- **WHEN** a snapshot is displayed and time passes without a new usage poll
- **THEN** the "now" marker advances toward the window's reset to stay live, like the existing reset countdown

#### Scenario: Unknown reset time falls back to the plain bar
- **WHEN** a window's reset time is unknown
- **THEN** the gauge renders that window's unsegmented utilization bar with no segments and no marker, and does not display a misleading time axis

#### Scenario: Per-model scoped weekly window
- **WHEN** the usage snapshot includes a scoped weekly limit for a model (e.g. a `weekly_scoped` limit whose model display name is "Fable")
- **THEN** the gauge shows an additional weekly window for that model in both frontends, labeled by the model's display name, colored by threshold and drawn with the weekly time axis and reset countdown

#### Scenario: Multiple scoped models each get their own window
- **WHEN** the snapshot includes more than one scoped weekly limit
- **THEN** the gauge shows one labeled weekly window per scoped model

#### Scenario: A scoped window shows regardless of its active flag
- **WHEN** a scoped weekly limit is present in the snapshot but its "active" flag is false
- **THEN** the gauge still shows that model's window, because presence — not the active flag — governs display

#### Scenario: No scoped limits leaves the gauge unchanged
- **WHEN** the snapshot carries no scoped weekly limits
- **THEN** the gauge shows only the 5-hour and weekly windows, exactly as before this change
