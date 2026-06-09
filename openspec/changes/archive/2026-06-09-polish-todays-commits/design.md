## Context

The commit garden (`src/components/CommitGarden.tsx`) maps over every registered
workspace and, via `Plot`, renders a "quiet today" placeholder card for any
entry whose backend `WorkspaceGarden.dormant` flag is set — no commits today, a
non-git flat workspace, or the git-unavailable case (`garden.rs`,
`commands.rs`). The section title (`.dashboard-garden-section`) carries only a
`margin-bottom`, so it hugs the analytics overview above it.

Two capability specs touch this behavior: `commit-garden` ("Dormant and Degraded
States" mandates the placeholder) and `dashboard` ("Today's Ships Quiet State"
claims its quiet-day note mirrors the garden's dormant treatment).

## Goals / Non-Goals

**Goals:**
- Show only workspaces that committed today; hide the empty/quiet ones.
- Omit the whole section on a fully-quiet day.
- Give the section title breathing room from the analytics block above.

**Non-Goals:**
- No change to how the per-workspace graph itself is drawn (lanes, nodes, refs).
- No Rust/IPC change; the backend keeps emitting dormant gardens.
- No "show quiet workspaces" toggle (the retained `dormant` flag leaves room for
  one later, but it is out of scope here).

## Decisions

**D1 — Filter in the React component, not the Rust builder.** `CommitGarden`
filters `plants` to the non-dormant set; the backend payload is unchanged.
Rationale: this is a presentational choice, it is the smallest, fully-reversible
blast radius, and it keeps the `dormant` flag available for a future toggle.
Alternative (filter in `garden.rs`/command, drop the flag) was rejected as a
larger IPC-contract + test change for no user-visible gain.

**D2 — A fully-quiet day omits the entire section.** After filtering, if no
active plots remain, `CommitGarden` returns `null` (mirroring its existing
empty-registry `plants.length === 0` guard). Chosen over a section-level
"no commits today" note. Consequence: this diverges from the *Today's ships*
feed, which keeps a quiet-day note — so the `dashboard` spec's *Today's Ships
Quiet State* requirement is amended to drop its "mirroring the commit garden's
dormant treatment" clause; the ships note now stands on its own rationale.

**D3 — Title margin via the section, not the heading.** Add `margin-top` to
`.dashboard-garden-section` (the one bottom block with no card background, so it
needs explicit separation) rather than touching the shared
`.dashboard-panel-title` rule used elsewhere.

**Cleanup.** With dormant entries filtered upstream, the `Plot` dormant render
branch and the `.garden-plot--dormant` CSS rules become dead and are removed.

## Risks / Trade-offs

- A git-unavailable (silently broken) workspace now disappears from the garden
  rather than showing a placeholder → acceptable: the garden is a today's-delight
  surface, not a diagnostics panel, and the rest of the Dashboard still reflects
  workspace health.
- The section now blinks in/out day-to-day depending on activity → accepted as
  the explicit choice in D2; it matches the empty-registry behavior already in
  place.
