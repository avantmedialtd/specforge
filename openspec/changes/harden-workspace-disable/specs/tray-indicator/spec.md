# tray-indicator Specification Delta

## MODIFIED Requirements

### Requirement: Spec-Activity Glyph Variant

The tray icon SHALL render in one of two visual variants — a default variant and
a *spec-activity* variant — selected from the aggregated view of top-level rows,
the same snapshot the active-change badge counts. The spec-activity variant SHALL
be shown whenever any non-archived change belonging to a top-level row that is
**not disabled** has at least one capability spec delta (i.e. its
`ArtifactStatus.specs` is non-empty). The default variant SHALL be shown in every
other case, including when no workspaces are registered, when the aggregated view
has not yet been populated, when no active change of an enabled row has spec
deltas, and when the view cannot be re-evaluated due to a transient error (in
which case the most recently determined variant is retained until the next
successful evaluation).

A top-level row that the user has disabled (see the *Workspace Disable State*
requirement in the `workspace-registry` capability) SHALL NOT drive the variant,
regardless of how many of its changes carry spec deltas. The badge and the glyph
are one attention surface and SHALL share a single row-exclusion point: a parked
row's continued presence in the cache — which the *Cold Aggregation of Disabled
Rows* requirement deliberately preserves — MUST NOT reach either of them.

The variant selection SHALL be recomputed on every `CacheEvent` from the watcher,
using the same broadcast stream that drives the active-change badge, and the
aggregated view SHALL already reflect the event's effect when the recomputation
reads it. Toggling a row's disabled state SHALL itself trigger a recomputation,
so the variant follows the toggle without waiting for a filesystem event. The
variant SHALL persist across monitor scale-factor changes — when re-rasterization
is triggered by a scale change, the currently-selected variant (not the default)
SHALL be re-rasterized.

#### Scenario: Variant flips to spec-activity when a spec delta appears

- **WHEN** a registered workspace has no active changes touching specs
- **AND** a new change directory appears whose `ArtifactStatus.specs` is non-empty, or an existing change directory gains a non-empty `ArtifactStatus.specs`
- **THEN** the tray icon flips to the spec-activity variant within the watcher debounce window

#### Scenario: Variant reverts to default when the last spec delta disappears

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** every change in every registered workspace becomes one with an empty `ArtifactStatus.specs`, whether by file deletion, archival, or workspace removal
- **THEN** the tray icon reverts to the default variant within the watcher debounce window

#### Scenario: Any-workspace aggregation

- **WHEN** two registered workspaces are present, one with no spec activity and one with at least one active change whose `ArtifactStatus.specs` is non-empty
- **THEN** the tray icon shows the spec-activity variant

#### Scenario: Disabled workspace does not drive the spec-activity variant

- **WHEN** the only top-level row holding an active change with a non-empty `ArtifactStatus.specs` is disabled
- **THEN** the tray icon shows the default variant
- **AND** the row's changes remain in the cache and on the Dashboard

#### Scenario: Changes appearing in a disabled workspace never flip the glyph

- **WHEN** a workspace is disabled
- **AND** a change with a non-empty `ArtifactStatus.specs` appears in one of its worktrees
- **THEN** the tray icon does not flip to the spec-activity variant

#### Scenario: Disabling one row does not suppress another row's spec activity

- **WHEN** two top-level rows each hold an active change, only one of which has a non-empty `ArtifactStatus.specs`
- **AND** the row without spec deltas is disabled
- **THEN** the tray icon still shows the spec-activity variant

#### Scenario: Re-enabling restores the spec-activity variant

- **WHEN** the tray icon shows the default variant because the only spec-touching row is disabled
- **AND** the user re-enables that row
- **THEN** the tray icon flips to the spec-activity variant without waiting for a filesystem event

#### Scenario: Default variant on empty registry

- **WHEN** no workspaces are registered
- **THEN** the tray icon shows the default variant

#### Scenario: Default variant before the aggregated view is first populated

- **WHEN** the application has just launched and the aggregated view of the registered workspaces has not yet been populated
- **THEN** the tray icon shows the default variant

#### Scenario: Stale variant retained on transient parse error

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** a filesystem event triggers a re-parse that returns an error
- **THEN** the cache entry is left unchanged
- **AND** the tray icon continues to show the spec-activity variant

#### Scenario: Initial variant reflects pre-existing state at startup

- **WHEN** the application launches with at least one registered, enabled workspace holding an active change with a non-empty `ArtifactStatus.specs`
- **THEN** the aggregated view is populated before the first variant is selected
- **AND** the tray icon's first painted frame uses the spec-activity variant, not the default

#### Scenario: Current variant survives scale-factor change

- **WHEN** the tray icon is currently showing the spec-activity variant
- **AND** the main window moves to a monitor with a different scale factor
- **THEN** the spec-activity variant is re-rasterized at the new scale and applied to the existing tray handle
- **AND** the tray icon does not briefly revert to the default variant
