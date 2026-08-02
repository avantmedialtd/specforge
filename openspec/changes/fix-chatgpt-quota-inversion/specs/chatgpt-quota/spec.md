## MODIFIED Requirements

### Requirement: Quota status-line gauge

When the feature is enabled and a usage snapshot is available, the system SHALL render a "ChatGPT"-labeled status-line gauge in the desktop, web, and terminal frontends, beside — and visually distinguishable from — any Claude quota gauge, showing the utilization of the primary (5-hour) and secondary (weekly) rate-limit windows sourced from the usage response's `rate_limit.primary_window` and `rate_limit.secondary_window`. The gauge SHALL use the same visual grammar as the *Quota status-line gauge* requirement in the `claude-quota` capability: green below 70%, orange at or above 70%, red at or above 90% utilization, and a countdown to the window's reset in place of the percentage when a window is fully consumed.

The percentage each window carries in the usage response reports the share of that window's budget **remaining**, not the share consumed — established by comparing the same weekly window in the Codex desktop app (92% used) against this response (8%). The system SHALL therefore derive the utilization it displays as $utilization = 100 - remaining$, clamped to $0..100$, so that every surface shows consumed budget and the gauge agrees with the Codex app. The displayed figure SHALL always mean consumed budget, identically to the Claude gauge, so that two provider rows in one status line never carry opposite meanings.

Windows of a recognised standard length SHALL be labeled with the same vocabulary as the Claude gauge — `wk` for a week-length window and `5h` for a five-hour one, each matched within a tolerance so a length that is not exactly 604800 or 18000 seconds still resolves — and any other length SHALL keep the label derived from its reported duration.

When a window's reset time is known, the gauge SHALL render the time axis (segment ticks and a live "now" marker) using the window length reported by the usage response (`limit_window_seconds`) rather than a hardcoded duration: segmented by hours for windows up to 24 hours ($n = \mathrm{round}(secs/3600)$ segments) and by days for longer windows ($n = \mathrm{round}(secs/86400)$ segments), falling back to a 5-hour primary and 7-day secondary length only when the response omits the window length. A window whose reset time is unknown SHALL render the unsegmented utilization bar with neither segments nor a marker.

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

#### Scenario: Displayed percentage is consumed budget
- **WHEN** the usage response reports 8 as the weekly window's percentage, meaning 8% of that window's budget remains
- **THEN** the gauge shows 92% for that window, matching the consumption the Codex desktop app reports for the same window

#### Scenario: A barely-used window reads low
- **WHEN** the usage response reports 100 for a window, meaning the whole budget remains
- **THEN** the gauge shows 0% and colors that window green, rather than showing it as fully consumed

#### Scenario: An exhausted window reads as spent
- **WHEN** the usage response reports 0 for a window, meaning none of the budget remains
- **THEN** the gauge treats that window as 100% utilized and shows its reset countdown

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

