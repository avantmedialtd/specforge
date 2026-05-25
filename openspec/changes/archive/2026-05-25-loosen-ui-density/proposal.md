# Loosen UI density

## Why

The current type and spacing scale was tuned on a 2× Retina display where one CSS px renders at roughly half a device px. On a 4K monitor running at 100% OS scale — the developer's primary target — every token is rendered at its literal physical pixel size, so the body text reads at ~9–10pt print size and tree rows compress to ~22px each. The sidebar, where users scan many proposals and tasks, feels cramped, and small monospace meta labels (chips, change-ids, branch names) are hard to read without leaning forward.

We're retuning the scale once for 4K @ 100% rather than introducing a density toggle. A toggle doubles the design surface and forces every future component decision to be made twice; we'd rather commit to a comfortable default and revisit only if a user reports the new scale is too loose on their setup.

## What Changes

- Bump every type-size token by ~2px (xs 10→12, sm 11→13, base 12→14, md 13→15, lg 15→17, xl 20→22, 2xl 28→30).
- Raise the default UI line-height token (`--leading-tight`) from 1.4 to 1.5. Prose and code line-heights stay as-is.
- Increase `.tree-row` vertical padding from 2px to 5px, `.workspace-row` padding from `--space-3` (12px) to `--space-4` (16px), and `.settings-toggle-row` vertical padding from 4px to 6px.
- No new components, no toggle, no settings persistence.

This is a **non-breaking** visual change: token names and the spacing scale itself are unchanged, only the px values.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `visual-identity`: the pinned px values in the type-scale requirement and the markdown-body size requirement change to the new numbers. Token names, structure, dark-scheme overrides, font-family choices, and chip/outline conventions are unchanged.

## Impact

- `src/App.css` — token block values, `.tree-row` / `.workspace-row` / `.settings-toggle-row` padding.
- `openspec/specs/visual-identity/spec.md` — pinned px values in the type-scale and markdown-body requirements.
- No Rust code changes. No new dependencies. No IPC surface change.
- Acceptance is visual: confirm the dev build at 4K @ 100% feels comfortably scannable while still showing a full workspace's worth of rows in the sidebar.
