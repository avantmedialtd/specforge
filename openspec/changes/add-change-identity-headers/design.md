## Context

The request that started this was "show the spec folder name in the sidebar next to the branch, and clicking it copies the name". Exploring it turned up two facts that redirected the design.

**The sidebar cannot fit it.** A sole change row's line 2 currently carries the branch chip on its leading edge and the status cluster (`.row-meta`, `flex-shrink: 0`, `margin-left: auto`) on its trailing edge. At the default 340px sidebar the line has roughly 300px ≈ 45 characters of `--text-2xs` mono, and the branch plus status already claim about half of that. `consolidate-e2e-session-and-token-acquisition` is 44 characters. At the 180px `minLeftWidth` the whole line is ~21 characters. Any sidebar placement shows a heavily truncated slug.

**The sidebar cannot be copied from.** `.tree-row` sets `user-select: none` (`src/App.css:553`) — correct for a navigation tree, where text selection would fight row selection on every drag, but it means a copy in the sidebar must be a bespoke button rather than a selection. That button then needs a clipboard write, which needs a fallback for the non-loopback `--bind` case (plain HTTP on a non-`localhost` origin leaves `navigator.clipboard` undefined, and `crates/specforge-web` supports exactly that bind), which needs success feedback (no toast infrastructure exists anywhere in the frontend), which needs a keyboard equivalent (the *Change-Row Favorite Toggle* requirement established that nested row controls are never focusable, so a chord is the only keyboard path), which collides with `Cmd+C`'s native meaning.

Moving the identity to a surface where text is already selectable removes that entire chain. And the surface that should host it turns out to be missing a header altogether: `DetailPane` renders `<MarkdownView>` as its root (`src/components/DetailPane.tsx:263-270`), the only center-pane view with no header, so nothing on screen names the change being read.

## Goals / Non-Goals

**Goals**

- The change's directory name is visible, at full length, on the surface where the user is reading the change.
- That name is copyable with the gesture the user already knows, in every host the bundle runs in — the Tauri WebView, a `localhost` browser, and a browser reaching a non-loopback `--bind` over plain HTTP.
- The detail pane answers "which change am I reading, and from which worktree?" without consulting the sidebar.
- No new clipboard, permission, dependency, IPC, or keyboard surface.

**Non-Goals**

- Changing the sidebar. No row layout, chip, or truncation rule in the tree pane moves.
- A full breadcrumb (workspace › change › artifact). The header names the change and its worktree; naming the workspace and artifact too is a navigational design of its own.
- Copy affordances in `specforge-tui`. Terminal copy is an OSC 52 / host-selection concern with no shared implementation.
- A general-purpose copy-to-clipboard utility. Nothing here needs one, and adding one invites the sidebar-button design back in through a side door.

## Decisions

### Decision 1: The identity lives on the reading surface, not on the navigation surface

The sidebar's job is to let you *choose* a change; the center pane's job is to *show* it. A value you need to extract from the application belongs on the surface that is already a document, not on the one that is deliberately `user-select: none`.

This also fixes an orientation gap that exists independently of copying. Today, a user scrolled halfway down a long `design.md` has nothing on screen telling them which change it belongs to — the markdown's own `# H1` is the proposal *title*, and the sidebar highlight may be scrolled out of view or the sidebar hidden entirely (`Cmd+B`).

**Alternative rejected:** appending a chip to sidebar line 2. It shows roughly 20 of 44 characters at the default width, needs the branch chip pinned to `flex-shrink: 0` so the name absorbs all truncation, and still requires the whole clipboard chain above because the row is unselectable.

### Decision 2: `user-select: all`, not a clipboard API

Each identity string is rendered in an element carrying `user-select: all`, which selects the element's content atomically: one click highlights the entire token, and the platform's own copy gesture takes it.

This is chosen over plain selectable text because hyphens are word boundaries for double-click selection — double-clicking `consolidate-e2e-session-and-token-acquisition` yields `consolidate`, and triple-click takes the whole line including the branch chip. `user-select: all` makes a single click do exactly the right thing.

It is chosen over a copy button because it needs no clipboard permission, no `execCommand` fallback for the non-secure-context case, no success feedback component, and no keyboard binding — the platform's copy gesture is already bound, already discoverable, and already works in every host.

**Consequence:** the branch chip MUST be a sibling element of the name, never a descendant of the `user-select: all` element. Otherwise one click selects both and the copy carries the branch too. The spec states this explicitly rather than leaving it to implementation care.

### Decision 3: The header is sticky, and the scroll anchor compensates for it

`.split-pane-right` is the scroll container (`src/App.css:422-424`), and `DetailPane`'s `findScrollableAncestor` walks up to it. A header rendered inside it scrolls away, which loses the orientation benefit on exactly the long documents that need it most. So the header is `position: sticky; top: 0`.

That has a consequence the existing scroll-anchor effect does not currently handle. `DetailPane.tsx:215-218` scrolls a section anchor to `relative - 16` and centres a task anchor within `scrollParent.clientHeight`. With a sticky header of height *H* occupying the top of the scroll port, a section anchored at `relative - 16` lands *underneath* the header, and the task centring is off by *H*/2. Both offsets take *H* into account: the section offset becomes `16 + H`, and the task centring uses the effective visible box `clientHeight - H`.

*H* is published as a CSS custom property so the effect reads one value rather than hard-coding a number that drifts when the header's padding changes.

**Alternative rejected:** a non-sticky header. Simpler, and leaves the scroll-anchor effect untouched, but it is absent precisely when it is useful.

### Decision 4: Derive the branch from the views, do not extend `ArtifactRenderTarget`

`ArtifactRenderTarget` is `{ kind, workspace, changeId, artifactKind, capability? }` (`src/types.ts:610-616`) and carries no branch. The tempting fix is to add one, since `renderTargetForSelection` already holds the `ChangeInstance` in hand at `src/App.tsx:106`.

It is the wrong fix. Targets do not only come from tree clicks — the routing layer resolves them from URL addresses too, where no instance is in scope. A field populated on one path and absent on the other produces a header that shows the branch when you click a row and drops it when you open the same artifact from a link, which is worse than never showing it.

Instead the header resolves the branch itself, matching `target.workspace` (which is the worktree path for a git instance) against the instances in the workspace views. One lookup, one code path, correct for every target origin. A flat workspace's `workspace` matches no instance, yields no branch, and renders no chip — which is the correct outcome, since a flat workspace has no git worktree identity.

### Decision 5: The header aligns to the prose column, not to the pane

`.markdown-view` is `max-width: 880px; margin: 0 auto` (`src/App.css:1165-1172`) — a centred prose column, not a full-bleed pane. A header spanning the full pane width would float free of the text it describes on a wide window, sitting far to the left of the column it heads.

The header therefore takes the same `max-width` and centring as the prose column and shares its horizontal padding, so the identity sits directly above the first line of the document. The sticky background still spans the pane so content does not show through it while scrolling.

### Decision 6: The three surfaces diverge, deliberately

They are not three instances of one component, and pretending otherwise would force wrong content onto two of them.

| Surface | Shows | Why it differs |
|---|---|---|
| Detail pane | change directory name + branch chip | Renders a live change from a real worktree, so both the change and the checkout are meaningful. |
| Archive reading view | archive directory name (`YYYY-MM-DD-<id>`), `archive/` prefix stripped | An archived change has no live worktree, so no branch exists to name. The dated directory is what exists on disk. |
| File browser preview | selected file's root-relative path | `FilesRenderTarget` is `{ root }` (`src/types.ts:638-641`) — workspace-scoped, with no change context at all. The path is the only identity available, and it contains the change name whenever the file lives under `openspec/changes/`. |

What they share is the treatment, not the content: a monospace identity at the dense meta tier, `user-select: all`, above the rendered document.

On the archive surface, the dated directory name is preferred over the undated change id because the feature's purpose is to hand an agent a filesystem address, and `2026-08-14-add-web-ui-touch-support` is the folder that exists. `ArchiveView.tsx:170` already prefixes it with `archive/` when constructing the render target; that prefix is a path detail of the read API, not part of the folder's name, so it is stripped for display.

## Risks / Trade-offs

- **`user-select: all` is a quiet affordance.** Nothing signals that a click selects the whole token; the user discovers it by clicking. Mitigated by the identity being unmistakably a value (monospace, distinct from prose) and by the selection being visible the instant it happens. The failure mode is benign — a user who does not discover it can still drag-select as with any text. A `title` hint is deliberately not added, since a tooltip on a fully-visible string is noise.
- **The sticky header costs vertical space.** It occupies the top of the scroll port permanently. The `add-web-ui-touch-support` change established that the shell must fit the visible viewport, and short viewports are already the tight case. Mitigated by keeping the header to a single dense-tier line — it is a strip, not a banner.
- **The scroll-anchor offset is now coupled to the header's height.** If the header's padding changes and the CSS custom property is not updated, section anchors land under it. Mitigated by reading the value from CSS rather than hard-coding it, so there is one place to change and it is the same place the padding is set.
- **Three surfaces, three shapes.** Someone will read this as inconsistency. The requirements state the divergence and its reason explicitly, so a future reader sees a decision rather than a drift.
- **No mutation-testing coverage.** `.cargo/mutants.toml` scopes the gate to `openspec-core` and `openspec-app`; a frontend-only diff short-circuits the job and reports green without running. Green there means "not run". Coverage comes from ordinary frontend tests over the pure helpers — branch resolution by worktree path, and the `archive/` prefix strip — which are extracted as testable functions rather than inlined in JSX for exactly that reason.
