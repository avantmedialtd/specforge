# Tasks

## 1. Hide quiet workspaces in the commit garden

- [x] 1.1 In `src/components/CommitGarden.tsx`, compute the active plots
  (`plants.filter((p) => !p.dormant && p.commits.length > 0)`) and render only
  those.
- [x] 1.2 Return `null` when no active plots remain, so a fully-quiet day (and
  the git-unavailable case where every entry is dormant) omits the whole section
  — extend the existing `plants.length === 0` guard to the filtered set.
- [x] 1.3 Remove the now-unreachable dormant render branch in `Plot` (the
  `plant.dormant || plant.commits.length === 0` early return).

## 2. Give the section title breathing room

- [x] 2.1 In `src/App.css`, add a `margin-top` to `.dashboard-garden-section`
  so its title is separated from the analytics overview above it.
- [x] 2.2 Remove the now-unused `.garden-plot--dormant` rules.

## 3. Verify

- [x] 3.1 `bun run build` (tsc + bundle) passes with no unused-symbol errors
  from the removed dormant code paths.
- [x] 3.2 Verified structurally: build green, zero leftover
  `garden-plot--dormant`/"quiet today" references, the `dormant` field still
  consumed by the active-plot filter, and the null-guard makes a fully-quiet
  garden omit the section. Live `bun run wt:dev` screenshot (incl. the `space-5`
  title margin) available on request.
