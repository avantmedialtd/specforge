## 1. Backend: parse scoped windows (`crates/openspec-app/src/quota.rs`)

- [ ] 1.1 Add a `ScopedQuotaWindow { model: String, utilization: u8, resets_at_unix: Option<u64> }` struct with `#[serde(rename_all = "camelCase")]`, mirroring `QuotaWindow`.
- [ ] 1.2 Add a `scoped: Vec<ScopedQuotaWindow>` field to `ClaudeQuotaState` with `#[serde(default)]`; update `disabled()` / `status_only()` constructors to initialize it empty.
- [ ] 1.3 In `parse_usage`, after the `five_hour` / `seven_day` parse, walk `json["limits"]` (an array), keep entries whose `kind == "weekly_scoped"`, and build a `ScopedQuotaWindow` from each: `percent` (integer, clamped `0..=100`) → `utilization`, `resets_at` → `resets_at_unix` via `parse_rfc3339_to_unix`, `scope.model.display_name` → `model`.
- [ ] 1.4 Skip scoped entries with no usable `scope.model.display_name`; ignore `is_active` (presence, not the flag, governs inclusion). Leave the existing "at least one top-level window present" guard unchanged.
- [ ] 1.5 Confirm `degrade_to_stale` carries the new list (it clones via `..prev.clone()`); no change expected, but verify a staled snapshot keeps its `scoped` entries.

## 2. Backend tests (`crates/openspec-app/src/quota.rs`)

- [ ] 2.1 Add a test parsing a `limits[]` fixture containing a `weekly_scoped` entry (`percent`, `resets_at`, `scope.model.display_name = "Fable"`) plus `session` / `weekly_all` entries; assert `scoped` has exactly the one Fable window with `utilization` and `resets_at_unix` set, and that `session` / `weekly_all` are NOT added to `scoped`.
- [ ] 2.2 Add a test that a response with no `limits` key (today's shape) parses to an empty `scoped` list while `five_hour` / `seven_day` still populate.
- [ ] 2.3 Add a test that a `weekly_scoped` entry missing `scope.model.display_name` is skipped rather than panicking.

## 3. IPC types (`src/types.ts`)

- [ ] 3.1 Add a `ScopedQuotaWindow { model: string; utilization: number; resetsAtUnix: number | null }` interface mirroring the Rust struct.
- [ ] 3.2 Add `scoped: ScopedQuotaWindow[]` to the `ClaudeQuotaState` interface.

## 4. Desktop rendering (`src/components/QuotaPill.tsx`, `src/App.css`)

- [ ] 4.1 After the weekly `WindowRow`, map `quota.scoped` to one `WindowRow` per entry: `label={w.model}`, `win={...}`, `segments={7}`, `lengthSecs={7 * 86400}`, keyed by model.
- [ ] 4.2 If `WindowRow`'s `win` prop is typed as `QuotaWindow`, either pass a `{ utilization, resetsAtUnix }` view of the scoped window or widen the prop; keep the row component otherwise unchanged.
- [ ] 4.3 Verify the `.quota-row-label` column renders a longer label (e.g. "Fable") without breaking layout; add an ellipsis/`min-width` tweak only if it clips.

## 5. TUI rendering (`crates/specforge-tui/src/ui.rs`)

- [ ] 5.1 In the `QuotaStatus::Ok` arm of `quota_gauge`, after the `seven_day` gauge, iterate the scoped windows and render one 7-cell gauge each (same path as the weekly gauge), labeled by the model name, honoring the existing ASCII / color-depth fallback ladder.
- [ ] 5.2 Append scoped gauges LAST so they are the first content truncated under the flush-right width pressure; truncate long model labels rather than overflowing.
- [ ] 5.3 Add a render/unit test covering a snapshot with one scoped window (label + 7-cell fill), alongside the existing `quota_fill_cells` / `quota_severity` tests.

## 6. Verification

- [ ] 6.1 `cargo test -p openspec-app` and `cargo test -p specforge-tui` pass (new + existing quota tests green).
- [ ] 6.2 `bun run build` passes (`tsc --noEmit` accepts the new `types.ts` shape and `QuotaPill.tsx` usage).
- [ ] 6.3 Launch the desktop app (`bun run wt:dev`) with `claude_quota_enabled` on and confirm the scoped model row (e.g. "Fable") appears below the weekly row with the correct percentage, threshold color, and weekly time axis.
- [ ] 6.4 Launch the TUI and confirm the scoped gauge renders in the title bar and truncates gracefully on a narrow terminal.
- [ ] 6.5 `openspec validate add-per-model-quota-windows --strict` passes.
