## Context

The workspace tree (`src/components/WorkspaceTree.tsx`) renders top-level rows (`RepoNode`, `FlatWorkspaceNode`) as disclosure parents in every case, even when they have no active changes. When empty, an italic faint child row reading "no active changes" is rendered underneath, alongside a dead `{totalActiveInstances === 0 && null}` block whose own in-source comment notes that the count badge on the parent row already conveys the signal. No live spec scenario requires the placeholder; the original "empty states" task lives in an archived bootstrap change.

This is a small, single-file UI cleanup. A `design.md` is required only because the schema's `tasks` artifact depends on it; the genuine design surface is shallow. The document exists to record the one judgment call worth pinning down before implementation, not to invent architecture.

## Goals / Non-Goals

**Goals:**
- The empty top-level row state rests on the existing count badge alone — no duplicate textual placeholder.
- Empty top-level rows render as leaves (no disclosure chevron, no `onToggle`), so the chrome matches the absence of children.
- The dead `{totalActiveInstances === 0 && null}` block and its now-orphaned local are removed.

**Non-Goals:**
- Hiding the `0` count badge when a top-level row is empty. The same badge is load-bearing for non-empty rows and consistent presence is intentional; that is a separate question.
- Repairing stale entries in the persisted `collapsed` set for nodes that have since become empty. The pre-existing behaviour (a stale override silently re-applies when the node next gains children) is unchanged by this proposal and out of scope.
- Removing the now-unused `.row-empty` CSS class. Leaving it costs nothing in this change; a sweep is fine as a follow-up if it stays unused.
- Adding React test infrastructure. None exists today; introducing it for one assertion is disproportionate.

## Decisions

**Render an empty top-level row as a leaf (Option B), not as an expandable disclosure with an empty body (Option A).**
- Chrome should match content. A chevron promises "click me to see what's inside" — an empty body breaks that promise on every click.
- The count badge `0` on the parent row remains the canonical empty signal.
- A leaf has no `onToggle`, so the persisted `collapsed` set cannot grow new stale entries from interactions with an already-empty row.
- *Alternative considered:* keep the disclosure but render an empty body (Option A). Cheaper diff, but leaves the chevron as a no-op affordance.

**Drive `isEmpty` off the same array that feeds the count badge (`repo.active.length` for `RepoNode`, `changes.length` for `FlatWorkspaceNode`).**
- Single source of truth: the badge and the leaf/disclosure decision read the same length. They cannot drift.
- `totalActiveInstances` becomes unused under this rule and is removed alongside the dead block.

**Do not gate the change behind a flag or a setting.**
- No user persistence depends on the placeholder. No user workflow notices the chevron disappearing on an empty row.
- The watcher-driven state transition (empty → non-empty when a change is added) is unchanged: the count badge ticks `0` → `1`, the disclosure (re)appears default-open, and `notifications.rs` fires its `ChangeAdded` notification. Multiple signals; nothing lost.

## Risks / Trade-offs

- [A user previously collapsed a repo when it had children; that repo later empties; later still it gains a new change] → The new disclosure renders closed because the user's `collapsed` override is honoured. The badge `0` → `1` and the system notification still surface the arrival. This is identical to today's behaviour and is not introduced by this change; mitigation (e.g., clearing stale `collapsed` entries on empty-transition) belongs in a separate proposal.
- [Future contributor restores the placeholder, thinking the empty leaf "looks unfinished"] → Mitigated by the new spec scenario, which encodes the design intent so a re-introduction would visibly conflict with `spec-browser/spec.md`.
- [`.row-empty` CSS class becomes unreferenced] → Cosmetic; no user-visible effect. A follow-up sweep can remove it. Leaving it carries no functional risk.
