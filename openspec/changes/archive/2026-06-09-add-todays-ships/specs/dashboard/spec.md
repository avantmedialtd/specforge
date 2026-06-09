## ADDED Requirements

### Requirement: Today's Ships Feed

The Dashboard SHALL present a "Today's ships" feed: the changes archived today, aggregated across every registered workspace, ordered newest-archived first. A change SHALL be considered shipped today when its archived directory (`openspec/changes/archive/<YYYY-MM-DD>-<id>/`) is dated to the viewer's local calendar day, consistent with the day boundary used by the commit garden and the *Today's Progress* hero. The feed's membership SHALL be determined from the dated archive directory and SHALL NOT require git. Each feed entry SHALL identify its change — by its title when available, otherwise its change id — and its owning workspace or repository. When the change's archival instant is recoverable from git history, each entry SHALL additionally present a relative archive time (for example, "archived 2h ago"); when it is not recoverable, the entry SHALL render without the relative time. Entries SHALL be ordered by archival instant, newest first, falling back to a stable order when the instant is unavailable.

#### Scenario: Feed lists changes archived today

- **WHEN** the today's ships feed renders
- **AND** one or more changes were archived to a directory dated the viewer's local today
- **THEN** those changes are listed, newest-archived first
- **AND** changes archived on an earlier day are not listed

#### Scenario: Entry shows a relative archive time when git supplies it

- **WHEN** a shipped change's archival instant is recoverable from git history
- **THEN** its feed entry shows a relative archive time

#### Scenario: Entry identifies change and workspace

- **WHEN** the today's ships feed renders an entry
- **THEN** the entry shows the change's title when available, otherwise its change id
- **AND** the entry shows the change's owning workspace or repository

### Requirement: Ship Selection Opens the Archive Browser

Selecting an entry in the today's ships feed SHALL open the Archive browser with that archived change pre-selected, rather than navigating to the active-change read path — an archived change no longer resides under `openspec/changes/<id>/`. This navigation SHALL be read-only, consistent with the Dashboard's read-only operation.

#### Scenario: Selecting a ship opens it in the Archive browser

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the Archive browser opens with that change pre-selected

#### Scenario: Ship selection performs no mutation

- **WHEN** the user selects an entry in the today's ships feed
- **THEN** the only effect is navigation into the Archive browser
- **AND** no spec, task, change, or git state is modified

### Requirement: Today's Ships Quiet State

When no change has been archived on the viewer's local today, the today's ships feed SHALL present a quiet-day note rather than hiding the feed or showing stale prior-day entries, mirroring the commit garden's dormant treatment so the two "today" surfaces read consistently.

#### Scenario: Nothing shipped yet today

- **WHEN** no change is archived to a directory dated the viewer's local today
- **THEN** the today's ships feed shows a quiet-day note
- **AND** it does not show changes archived on earlier days

## MODIFIED Requirements

### Requirement: Reactive Dashboard Updates

While the Dashboard is the active center-pane surface, it SHALL reflect on-disk changes within the watcher's debounce window without user action. After the watcher finishes processing a debounced batch — a change added, a change archived, content edited within a tracked change, or a repository's refs changing — the Dashboard SHALL refresh its metrics to observe the post-batch state.

#### Scenario: Dashboard updates when a change is added

- **WHEN** the Dashboard is the active surface
- **AND** a new change directory is created on disk in a registered workspace
- **THEN** the Dashboard's active-change count reflects the new change within the debounce window

#### Scenario: Dashboard updates when a change is archived

- **WHEN** the Dashboard is the active surface
- **AND** a change is moved to `openspec/changes/archive/` on disk
- **THEN** the Dashboard's active/archived counts and lifecycle metrics reflect the archival within the debounce window
- **AND** when the archive directory is dated the viewer's local today, the today's ships feed reflects it within the debounce window

#### Scenario: Dashboard updates on commit activity

- **WHEN** the Dashboard is the active surface
- **AND** a new commit is created in a registered git-backed repository
- **THEN** the activity chart reflects the new commit within the debounce window

### Requirement: Graceful Degradation Without Git

When the `git` binary is unavailable, or a registered workspace is not inside a git repository, the Dashboard SHALL still render its non-git sections (summary metrics, per-repository breakdown, today's ships feed) and SHALL render the git-derived sections (activity chart, lifecycle metrics) using only the data recoverable from the available git-backed repositories. The today's ships feed's membership is determined from the dated archive directory and SHALL render without git; only its per-entry relative archive time, which is git-derived, SHALL be omitted when git is unavailable. The Dashboard SHALL NOT error when git is absent.

#### Scenario: Git binary missing

- **WHEN** the `git` binary is not on PATH
- **THEN** the Dashboard renders its summary metrics, per-repository breakdown, and today's ships feed
- **AND** the today's ships entries render without their relative archive times
- **AND** the activity chart and lifecycle metrics render an empty or unavailable state rather than erroring

#### Scenario: Mixed git and non-git workspaces

- **WHEN** some registered workspaces are git-backed and others are flat
- **THEN** the summary metrics and breakdown include every workspace
- **AND** the activity chart and lifecycle metrics include only the git-backed repositories

### Requirement: Gamification Opt-In

The gamified progress layer SHALL be gated behind a setting that is **disabled by default**. The gated layer comprises the gamified, activity-log-derived views (today's progress, streak, contribution heatmap), the commit garden, the live celebrations, the per-author leaderboard, and every season surface (the season home, the equipped-treatment finish on the avatar, the seasonal leaderboard, the live tier-up, and the permanent career-tier readout). When the setting is disabled, the Dashboard SHALL render only its analytics — the cross-workspace summary metrics, per-repository breakdown, git-mined activity chart, change-lifecycle metrics, and today's ships feed — and SHALL NOT compute or present any gated section; the Settings *Badge finishes* surface SHALL likewise be hidden. Enabling the setting SHALL restore the gamified layer. The setting SHALL persist in the application's data directory.

#### Scenario: Gamification is off by default

- **WHEN** the gamification setting has never been enabled
- **THEN** the gamified layer is disabled
- **AND** the Dashboard renders only its analytics sections
- **AND** the commit garden is not shown

#### Scenario: Enabling restores the gamified layer

- **WHEN** the gamification setting is enabled
- **THEN** the Dashboard presents the gamified layer — today's progress, streak, heatmap, the season surfaces, the leaderboard, celebrations, and the commit garden

#### Scenario: Disabled hides the Settings locker

- **WHEN** gamification is disabled
- **THEN** the Settings badge-finishes locker is not shown

#### Scenario: Disabled skips gamified computation

- **WHEN** gamification is disabled
- **THEN** the gamified sections are not computed for the Dashboard payload
- **AND** the commit garden data is not computed

## REMOVED Requirements

### Requirement: Recent Activity Feed

**Reason**: The Dashboard's feed slot is repurposed from in-flight changes to changes archived today. The in-progress list is dropped because the workspace tree already enumerates active changes and the *Today's Progress* hero shows the in-flight count, leaving the old mtime-ordered feed redundant.

**Migration**: The feed now shows changes archived today — see the **Today's Ships Feed** requirement, which supersedes this one (membership by dated archive directory, git-enriched relative archive times, and Archive-browser navigation in place of active-change navigation).
