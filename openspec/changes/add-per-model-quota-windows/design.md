## Context

The Claude quota gauge (in `crates/openspec-app/src/quota.rs`, rendered by
`src/components/QuotaPill.tsx` and `crates/specforge-tui/src/ui.rs`) polls
`/api/oauth/usage` and renders exactly two windows: the pooled 5-hour window and
the pooled weekly window, both parsed from the response's top-level `five_hour`
and `seven_day` objects.

A live check of the endpoint shows the response has since grown a second, richer
representation. Alongside the top-level windows it now carries a `limits[]`
array, and the per-model caps that the original design expected under top-level
fields (`seven_day_opus`, `seven_day_sonnet`, …) are all `null` — that
representation is dead. The real per-model data arrives as `limits[]` entries of
kind `weekly_scoped`, each carrying `percent` (an integer), `resets_at`, an
`is_active` flag, and `scope.model.display_name`. On the sampled account the
`Fable` model's scoped weekly limit read **59%** while the pooled weekly read
**39%** — a binding constraint the current gauge never surfaces.

This design covers reading those `weekly_scoped` entries and rendering one extra
weekly window per scoped model, without disturbing the working pooled windows.

## Goals / Non-Goals

**Goals:**
- Surface every `weekly_scoped` limit as an additional weekly window in both
  frontends, labeled by the model's display name.
- Drive the label entirely from response data (`scope.model.display_name`) — no
  hardcoded model name anywhere.
- Reuse the existing weekly rendering (7 day segments, live marker, thresholds,
  reset countdown) rather than inventing new visual grammar.
- Keep the change additive and opt-in: a snapshot with no scoped limits renders
  exactly as today.

**Non-Goals:**
- Migrating the pooled 5-hour / weekly windows off the top-level fields onto the
  `limits[]` `session` / `weekly_all` entries.
- Surfacing the response's `extra_usage` / `spend` credits structure.
- Adopting the endpoint's server-computed per-limit `severity` in place of the
  client-side 70 / 90 thresholds.
- Any scoped window that is not weekly (only `weekly_scoped` is observed today).

## Decisions

### Decision: Source scoped windows from `limits[]`; leave the pooled windows on the top-level fields
The per-model top-level fields are `null`, so the only live source of scoped
caps is `limits[]`. But the top-level `five_hour` / `seven_day` are still
populated and the existing parse works, so the pooled windows keep reading them.
`parse_usage` gains a second, independent pass over `limits[]` that only extracts
`weekly_scoped` entries.
*Alternative — migrate all windows to `limits[]` (`session` → 5h, `weekly_all` →
weekly, `weekly_scoped` → per-model):* cleaner conceptually and future-proof, but
it rewrites the working, shipped pooled-window path for no present benefit and
takes on the risk that `limits[]` is absent on some account/version. Deferred as
a possible follow-up.

### Decision: New `ScopedQuotaWindow { model, utilization, resets_at_unix }` plus a `scoped: Vec<_>` on the snapshot
Scoped windows need a label the plain `QuotaWindow` does not carry, and there can
be more than one, so the natural shape is a list of labeled windows. `scoped`
gets `#[serde(default)]` so older/absent data deserializes to an empty list and
`ClaudeQuotaState` stays `Eq`. `degrade_to_stale` already rebuilds via
`..prev.clone()`, so stale handling carries the list unchanged.
*Alternative — reuse `QuotaWindow` with a parallel `Vec<String>` of labels:*
rejected; pairing two vectors by index is fragile. *Alternative — a single
`Option<QuotaWindow>` for one model:* rejected; the data is a list and multiple
scoped models are possible.

### Decision: Select by `kind == "weekly_scoped"`, label from `scope.model.display_name`, show when present
Filtering on the entry `kind` (not on a model name) means the gauge follows
whatever set of models the endpoint scopes, with zero hardcoding. The label comes
from `scope.model.display_name` (the model `id` is `null`). `is_active` is **not**
used as a display gate: in the sampled response the `session` and `weekly_all`
entries are `is_active: false` while the `Fable` entry is `true`, so `is_active`
marks the currently-binding limit, not display eligibility — gating on it would
be wrong (and, applied consistently, would hide the pooled windows too). A scoped
window shows whenever its entry is present.
*Alternative — filter on `display_name == "Fable"`:* rejected; hardcodes the name
and drops future scoped models. *Alternative — gate on `is_active`:* rejected on
the semantics above.

### Decision: Parse the integer `percent`, not the float `utilization`
`limits[]` entries express utilization as an integer `percent`, unlike the
top-level windows' float `utilization`. The scoped pass reads `percent` and
clamps to `0..=100`; this is the one genuinely new line of parsing. `resets_at`
reuses the existing `parse_rfc3339_to_unix`.

### Decision: Render a scoped window as a weekly window; append it last
A `weekly_scoped` entry shares the weekly reset instant and the 7-day length, so
each frontend renders it with the existing weekly parameters (7 segments,
`lengthSecs = 7 d`, the live now-marker, threshold colors, exhausted-window
countdown). Desktop appends one `.quota-row` per scoped model after the weekly
row (vertical space is free). The TUI appends one 7-cell gauge per scoped model
after the weekly gauge — **last**, so it is the first content clipped under the
title bar's flush-right width pressure; long model names truncate.

## Risks / Trade-offs

- **`limits[]` is a newer / internal shape that may shift** → keep all `limits`
  parsing in the one defensive module; a missing or empty `limits` array yields an
  empty `scoped` list and today's exact behavior, so a shape change degrades to
  the current two-window gauge rather than crashing.
- **TUI width pressure from N extra gauges** → scoped gauges are appended last and
  truncate first; the pooled 5h / weekly gauges are never sacrificed. Long model
  display names truncate within their gauge.
- **`is_active` semantics are inferred, not documented** → treating presence as
  the display rule means a scoped limit that lingers in the response after
  becoming irrelevant would still show. Acceptable: the endpoint controls
  presence, and a stale-but-present cap is more honest than hiding a real one.
- **Duplicate data in `limits[]`** → `session` and `weekly_all` duplicate the
  top-level windows; taking only `weekly_scoped` avoids any double-count.
- **Desktop label-column width** → the `.quota-row-label` column is sized for
  short labels (`5h`, `wk`); a long model display name may need the column to grow
  or ellipsize. Minor CSS; "Fable" fits as-is.

## Migration Plan

Additive and opt-in; no data migration. `scoped` defaults to an empty list via
serde, so existing in-flight snapshots and persisted settings load unchanged, and
the feature stays gated behind `claude_quota_enabled`. Rollback is removing the
field and the extra rendering — a snapshot without scoped windows already renders
as it does today.

## Open Questions

- If a model ever gains a **scoped 5-hour** limit (a `session`-grouped scoped
  entry), this design surfaces only `weekly_scoped`; handling a scoped session
  window would be a follow-up.
- Whether to eventually adopt the endpoint's server-provided `severity` per limit
  in place of the client-side 70 / 90 thresholds (kept as a non-goal here for
  consistency with the pooled windows).
