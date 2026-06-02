# Tasks

## 1. Two-line layout for sole change rows

- [x] 1.1 In `src/components/WorkspaceTree.tsx`, gave `Row` an optional line-2 `detail` slot: when present the content area renders the primary `label` over the `detail` line as one stacked `.row-stack` block, chevron/swatch stay in the leading gutter, and the single-line `meta` path is unchanged for rows that don't use it
- [x] 1.2 Routed the **flattened singleton instance row** (`InstanceNode` with `isSingleton`) through the two-line path: line 1 = change name, line 2 = worktree identity + status — verified live (e.g. `two-line-change-rows`, every MushRoom change row)
- [x] 1.3 Routed the **flat-workspace change row** (`FlatChangeNode`) through the two-line path: line 1 = title (fallback `changeId`), line 2 = `changeId` + completion ✓ only (no worktree identity). Code + build verified; not visually exercised (no non-git workspace is registered)
- [x] 1.4 Multi-instance child rows, the disclosure parent, Repo/flat headers, and all artifact/section/task rows keep the single-line path — headers + artifact rows confirmed single-line in screenshots; multi-instance children unchanged in code (no multi-instance change available to exercise live)

## 2. Worktree identity + status on the detail line

- [x] 2.1 Removed the `.row-branch` chip from inside `.row-label`: `labelForInstance` is now single-arg (child rows only); the singleton's branch no longer rides the greedy ellipsizing label — confirmed (no inline chip on line 1)
- [x] 2.2 Line 2 leading edge shows branch **and** worktree folder basename together via `worktreeIdentity` (`branch · folder`), folder alone for detached HEAD, omitted for non-git flat rows — verified (`worktree-cogsworth · cogsworth`)
- [x] 2.3 Line 2 trailing edge renders the shared `statusCluster` (progress meter or ✓, relative mtime, divergence) in a `.row-meta` pushed right — verified (meter + `1h ago` etc.)
- [x] 2.4 The active-instance dot is gated to multi-instance children (`isPrimary` in the child branch only); sole change rows carry no dot — confirmed (no dot on any singleton row)

## 3. CSS — stacked rows as one selectable unit

- [x] 3.1 In `src/App.css`, styled `.row-stack` / `.row-line--detail` / `.row-worktree`: line 1 `--text-sm`, line 2 muted `--text-2xs`, indented to line 1's text origin (shares the stack inline-start past chevron/swatch) — verified
- [x] 3.2 Selection (2px `--accent` border-left + `--accent-tint` wash) spans both lines — verified by selecting `two-line-change-rows` (wash covers name + detail line). Hover uses the same `.tree-row` rule, so it spans both by construction
- [x] 3.3 Line 2 ellipsizes gracefully — verified (`worktree-two-line-change-r…`). Line 1 keeps the unchanged `.row-label` ellipsis (truncates against the row edge when the name alone overflows)

## 4. Worktree-label styling decision (open question)

- [x] 4.1 Resolved to a **plain muted-mono subtitle** (no outlined chip): on its own roomy line the border read as unnecessary weight, and `branch · folder` as plain text matches the approved mockups. Recorded in `design.md`

## 5. Verify behaviour

- [x] 5.1 `bun run build` (`tsc --noEmit && vite build`) passes — clean, no `noUnusedLocals`/`noUnusedParameters` fallout from the reworked helpers
- [x] 5.2 Ran the app and screenshotted: git singleton rows show the name on line 1 and the branch chip + status on line 2; long names no longer clip the worktree label
- [x] 5.3 Flat-workspace change row: code + build verified only — no non-git workspace is registered in the shared state to exercise it live
- [x] 5.4 Headers and artifact/section/task rows are visually unchanged (single-line); multi-instance child single-lining is unchanged in code (no multi-instance change available to exercise live)
- [x] 5.5 Click anywhere on the row selects the change (verified — selection + detail-pane wiring is the unchanged `.tree-row` onClick); the chevron still toggles the artifact subtree (unchanged `toggle(nodeId, true)`)

## 6. Workspace-colour emphasis (apply-time iteration)

- [x] 6.1 Thread the owning workspace `PaletteColor` from `RepoNode` / `FlatWorkspaceNode` through `LogicalChangeRow` to `InstanceNode` / `FlatChangeNode` (it previously stopped at the top-level swatch)
- [x] 6.2 Give the change name the only line-1 emphasis: heavier weight (`.row-line--primary .row-label { font-weight: 500 }`), plain high-contrast ink, no colour or swatch on the text
- [x] 6.3 Add the workspace-colour **rail**: `.tree-row--rail-<color>` tints the inline-start border to `--ws-swatch-<color>`; `.tree-row.selected` overrides to `--accent` by specificity, so selection still shows the accent bar and the rail returns on deselect
- [x] 6.4 Tint the branch **chip** to the workspace colour: `.row-worktree--<color>` sets text + border to the contrast-safe `--ws-text-<color>` shade
- [x] 6.5 Add `--ws-text-*` palette tokens (light + dark scheme), each computed to ≥4.6:1 on its background; removed the dead name-tint CSS from the rejected experiment
- [x] 6.6 Verified live: each change shows its workspace's rail + tinted chip (SpecForge amber, MushRoom indigo), the name stays the neutral-bold anchor, and selection overrides the rail

## 7. Sync spec deltas

- [x] 7.1 `spec-browser` deltas match the shipped behaviour: *Two-Line Sole-Change-Row Layout* now describes the heading-weight name, the workspace-tinted branch chip, and the workspace-colour rail (with selection override); flat-row scenario is `changeId` + ✓; *Instance Row Chrome* scoped to multi-instance children
- [x] 7.2 `openspec validate two-line-change-rows --strict` passes
