# Show Per-Model Quota Windows in the Claude Gauge

## Why

The Claude quota gauge shows only the two *pooled* windows — the 5-hour window
and the general weekly window. But Anthropic's usage endpoint now meters some
models against their **own** weekly cap, and that cap can bind well before the
pooled one. On a live response today the pooled weekly window sits at **39%**
while the **Fable** model's scoped weekly limit is already at **59%** — a wall
the current gauge can't see, because it never looks at the per-model data.

That per-model data is no longer where the original design expected it. The
legacy top-level fields (`seven_day_opus`, `seven_day_sonnet`, …) are all `null`
now; Anthropic moved per-model limits into a new `limits[]` array, where each
scoped cap arrives as an entry of kind `weekly_scoped` tagged with the model's
display name. Surfacing those entries turns "how much of my shared budget is
left" into "…and which specific model am I about to run out of."

## What Changes

- **Read the endpoint's modern `limits[]` array** and render each
  `weekly_scoped` entry as an additional weekly window in both frontends,
  **labeled by the model's display name** (`scope.model.display_name`, e.g.
  "Fable"). The label is data-driven — no model name is hardcoded, so if the set
  of scoped models changes, the gauge follows.
- Each scoped window **is** a weekly window (same `resets_at`, 7-day length), so
  it reuses the existing weekly rendering wholesale: 7 day segments, the live
  "now" marker, the green/orange/red thresholds, and the exhausted-window reset
  countdown. No new visual grammar.
- **The pooled 5-hour and weekly windows are unchanged** — they keep reading the
  snapshot's top-level `five_hour` / `seven_day` fields. This change is purely
  additive on top of the working gauge; it does not migrate those two windows to
  `limits[]`.
- A scoped window is shown whenever it is **present** in the snapshot,
  independent of the entry's `is_active` flag (which marks the currently-binding
  limit, not display eligibility). When the snapshot carries no scoped limits, no
  extra windows appear — the gauge looks exactly as it does today.
- **Desktop**: one extra `.quota-row` per scoped model, appended after the weekly
  row. **TUI**: one extra 7-cell gauge per scoped model, appended last so it is
  the first to clip under the title bar's width pressure; long model names
  truncate.

Explicitly **out of scope** (possible follow-ups, not this change):
- Surfacing `extra_usage` / `spend` credits (a separate billing structure the
  same response now carries) — still a non-goal, as in v1.
- Adopting the endpoint's server-computed per-limit `severity` in place of the
  client-side 70/90 thresholds.
- Migrating the pooled 5-hour and weekly windows off the top-level fields onto
  `limits[]` (`session` / `weekly_all`). The top-level fields are still populated
  and the fallback risk isn't worth taking here.

Nothing is **BREAKING** — the addition is gated behind the same opt-in
`claude_quota_enabled` setting, and a snapshot with no scoped limits renders
identically to today.

## Capabilities

### New Capabilities
<!-- None. This extends how an existing capability renders and parses; it adds no new capability. -->

### Modified Capabilities
- `claude-quota`: the **Quota status-line gauge** requirement gains per-model
  scoped weekly windows — sourced from the usage response's `limits[]`
  (`weekly_scoped` entries), labeled by model display name, and rendered with the
  same weekly time axis, thresholds, and reset countdown as the general weekly
  window. When no scoped limits are present, the gauge is unchanged.

## Impact

- **`crates/openspec-app/src/quota.rs`**: add a `ScopedQuotaWindow { model,
  utilization, resets_at_unix }` type and a `scoped: Vec<ScopedQuotaWindow>`
  field on `ClaudeQuotaState` (serde-default empty). Extend `parse_usage` to walk
  `json["limits"]`, keep entries whose `kind == "weekly_scoped"`, and read each
  one's `percent` (an integer, unlike the top-level windows' float
  `utilization`), `resets_at`, and `scope.model.display_name`. The existing
  `five_hour` / `seven_day` parse is untouched; `degrade_to_stale` already clones
  every field, so stale handling carries the new list for free.
- **`src/types.ts`**: mirror `ScopedQuotaWindow` and add `scoped:
  ScopedQuotaWindow[]` to `ClaudeQuotaState` (camelCase, hand-kept in sync as
  usual).
- **`src/components/QuotaPill.tsx`**: after the weekly `WindowRow`, map
  `quota.scoped` to one `WindowRow` per model — `label={w.model}`, `segments={7}`,
  `lengthSecs={7 * 86400}` — reusing the existing row component as-is.
- **`crates/specforge-tui/src/ui.rs`**: in the `QuotaStatus::Ok` arm, after the
  weekly gauge, render one 7-cell gauge per scoped window labeled by model,
  honoring the existing ASCII / color-depth ladder; new unit test for parsing a
  `weekly_scoped` limit into a labeled window.
- **`openspec/specs/claude-quota/spec.md`**: the gauge requirement is modified
  (see the spec delta) to cover per-model scoped weekly windows.
- **Risk**: low and additive. The one genuinely new parse path is the `limits[]`
  walk (integer `percent` vs. the top-level float `utilization`); everything
  downstream — segments, marker, thresholds, countdown, stale handling — is
  reused. A missing or empty `limits` array yields an empty list and today's
  exact behavior.
