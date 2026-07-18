# Light Up Completed Work in the Workspace Tree

## Why

The workspace tree signals completion as quietly as it possibly can. A finished change collapses its sections, strikes through and dims its tasks, and swaps its green in-progress meter for a **muted-grey** checkmark — `icon-checked` has no stylesheet rule anywhere, so the glyph inherits `.row-meta`'s `--text-muted`. The result is an accidental inversion: *in-progress* work is the only thing rendered in the success colour `--ok`, while *done* work fades to grey and hides itself. Green means "still working"; grey means "finished" — the opposite of the reading every user brings, where green is complete/good.

Burying completed work to draw the eye to what's left is a deliberate, defensible instinct, and it is worth keeping for individual task lines. But at the **milestone** boundaries — a whole section, a whole change — completion currently earns no positive mark at all. This change makes finishing legible and satisfying without adding motion or noise: it colours the done state and gives a completed change and its milestone glyphs the `--ok` green they should have had.

## What Changes

- The trailing completion glyph — shown when a Section, the Tasks artifact node, or a whole change is fully complete — changes from a muted-grey outline `✓` to a **filled `--ok` disc with a knocked-out check**. This becomes the row grammar's *second* sanctioned filled element, the symmetric partner to the in-progress task-progress meter: the meter is the *not-done* fill, the disc is the *done* fill.
- A **completed change's left rail** switches from its workspace's palette colour to `--ok` green — a persistent band that reads "this whole change is done" down the row. Row **selection continues to win**: a selected completed change still shows the `--accent` selection bar, exactly as the workspace-colour rail already yields to selection today.
- **Completed leaf tasks** render their label in `--ok` green (replacing the muted `--text-faint`), keeping the existing line-through. The line-through remains the colour-independent "done" signal; green is reinforcement.
- **No motion, no glow, no full-row wash.** The completion disc carries no `--glow-ok` halo (glows stay reserved for the in-progress meter and the other sanctioned surfaces), and completion never washes the row background (a full-row wash still means *selected* and nothing else). One token is added: **`--ok-strong`**, a deep, AA-readable "done" green for the foreground marks (disc fill, rail, completed-task label). The existing `--ok` is tuned as the *fill* inside the outlined progress meter and is too light (~2.6:1 on white) to use as a foreground colour; `--ok-strong` is `#047857` on light (~5.3:1, the same emerald as `--code-fg`) and `#34d399` on dark (9.34:1).

## Capabilities

### Modified Capabilities

- `visual-identity`: the row grammar gains a **Completed-State Styling** behaviour, and the *Outlined Chip Badges* invariant is amended to admit a **second** sanctioned filled element — the completion disc — alongside the in-progress meter. The completion glyph, the completed-change `--ok` rail (and its subordination to selection), and the `--ok` completed-leaf-task colour are specified. The single-sanctioned-glow and wash-means-selected invariants are explicitly preserved.

## Impact

- `src/components/icons.tsx` — add a filled-disc completion glyph (an `--ok` circle with a `--surface` knocked-out check) distinct from the bare `Check` polyline; the outline `Check` stays for any non-completion use.
- `src/components/WorkspaceTree.tsx` — swap the four completion-glyph sites (Instance row status cluster, Flat change row detail, Tasks artifact node meta, Section node meta) from `<Check className="icon-checked" />` to the disc; thread a "complete" flag into `Row` so a completed two-line change row renders the `--ok` rail instead of `tree-row--rail-{color}`.
- `src/App.css` — add the `--ok-strong` token (light `#047857`, dark `#34d399`); style the completion disc (≈15px, `--ok-strong` fill, `--surface` knocked-out check, no glow, visually distinct from the 4px status dots); add the `tree-row--complete` `--ok-strong` rail rule below the existing `.selected` rail override; change `.tree-row--struck .row-label` colour from `--text-faint` to `--ok-strong` while retaining `text-decoration: line-through`.
- **No Rust changes.** Completion is already derived (`completedTasks === totalTasks`, per-section `every(completed)`); this is a pure presentation change.
- **Deliberate scope boundaries** (so nobody "fixes" these later as oversights):
  - **No motion / celebration.** No animation when a thing flips to done — the transient "moment of completion" (progress racing to 100%, a pop, a forge-spark) was explored and deliberately deferred; this change is static colour only.
  - **No workspace-level roll-up.** The always-visible workspace/repo rows still show only a count; a per-workspace completion ring / "all changes done" indicator was explored and deferred to a separate change.
  - **No full-row completion wash.** A completed row is not tinted; the full-row wash remains the exclusive signal of selection. (The one token added, `--ok-strong`, is a solid foreground green — not a low-alpha `--ok-tint` wash, which is deliberately absent.)
  - **Atoms stay lighter than milestones.** Completed leaf tasks get green text + line-through, *not* the filled disc — the disc is reserved for milestone completion (section / change). This weight hierarchy is intended, not an inconsistency.
  - **Completion is still task-derived.** A change with no `tasks.md` (or zero tasks) is never "complete" and gets no green treatment, exactly as today; this change recolours the existing done predicate rather than redefining it.
