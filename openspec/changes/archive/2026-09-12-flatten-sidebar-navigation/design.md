## Context

Three frontends read one `AppService`. Two of them — the terminal Browse screen and the desktop Archive reader — already navigate a change the same way: pick it in a flat list, switch artifacts with tabs, choose a worktree copy above the tabs. The live desktop tree is the outlier, expanding each change through artifact, capability, section and task rows.

```
  terminal Browse          archive reader             live tree (today)
  ───────────────          ──────────────             ─────────────────
  ws                       ← Archive · date · title   ▾ ws
   ├ change ──▶ [P][D][T]  Worktree [copy ▾]            ▾ change
   └ change                [P][D][T][spec][spec]           Proposal
                           ─────────────────────           ▾ Specs
                           identity · 15m ago                  cap
                           # document                        Design
                                                             ▾ Tasks
                                                               ▸ 1. …
                                                                 task
```

The routing layer never modelled the depth: an artifact address is `scope + changeId + artifactKind [+ capability]`, the scope already carries an optional instance, and sections and tasks are a transient reveal that only `App.tsx`'s single `treeRef.current?.reveal(nodePath)` call touches. The tree's depth is therefore a presentation choice with no structural support beneath it, and roughly a dozen `spec-browser` requirements exist only to manage it.

The proposal fixes the three decisions that shape the work: the tree stops at the change row; artifact and instance selection move into the header the detail pane already renders, shared with the archive reader; and a heading-based outline beside the prose column replaces section and task nodes. This design settles how.

## Goals / Non-Goals

**Goals:**

- One place selects a change (the sidebar) and one place selects what to read of it (the header). The archive reader and the live reader are the same header in two modes.
- Every address that resolves today resolves to the same document tomorrow, with no grammar change.
- The outline reaches every shared reading surface by construction, including reader windows and file previews.
- The `spec-browser` capability loses its deep-tree requirements outright rather than carrying them as dead text.
- No Rust and no IPC change. The frontend bundle changes alone, and the desktop and web hosts change together.

**Non-Goals:**

- Remembering the last-read artifact per change. Per-key memory keyed by opaque identifiers accrues without bound and nothing prunes it — the reasoning `reader-window` and `document-width` already give for their own shared state.
- A folded or popover outline for narrow panes. The outline is shown beside the column or not at all.
- Outline entries for individual task lines. Headings only, with per-section counts.
- Changing the terminal frontend, which already has the target shape.
- Migrating or deleting the persisted collapse/expand override sets. They become unread.

## Decisions

### D1. Selecting a change opens its default artifact of its default instance

A change row's click navigates. The **default artifact** is the first present in the fixed order Proposal, Design, Tasks, then the first capability spec in listing order. The **default instance** of a multi-worktree change is the main worktree's when it hosts the change, otherwise the first instance in aggregator order — the rule `workspace-file-browser` adopted for a default copy, so the two surfaces agree.

*Rejected — the change row is disclosure-only, as today (`Deferred Interaction Nodes`).* With no artifact rows beneath it, a click that does nothing leaves the user with no way to open the change from the sidebar at all.

*Rejected — remember the last artifact per change.* See Non-Goals; and it would make the same click land in different places on different days.

### D2. One header component, two modes

The detail pane's `ChangeIdentityHeader` becomes the **change header**: identity row (unchanged), an instance-switcher row rendered only when there is more than one instance, and an artifact tab strip. It is built once and placed by two callers. `App` places it in live mode with the change's instances and artifact presence from `ChangeData`. `ArchiveView` places it in **read-only mode**: a leading row with the back control, date and title; no branch chip; copies in the switcher, labelled by the archive's existing `copyLabels`; tabs from the on-demand `archivedArtifactStatus`. The archive's own `archive-header`, `archive-copy-row` and `archive-artifact-tabs` are deleted.

Because the identity is now flush with the top of the pane in the archive too, the archive reader takes the same macOS titlebar clearance the live header takes. The clearance lives inside the measured sticky element already, so scroll anchors need no change.

*Rejected — a second header for the archive, kept in step by review.* The two headers have already drifted once (the archive stacks its identity under its own chrome). A shared component is the only equivalence that holds without a spec sentence policing it.

*Rejected — tabs in the sidebar under the change row.* That is the tree again, one level shallower.

### D3. Tabs are navigations with manual activation

A tab activation calls the existing `go(address)` with the artifact address a tree click forms today, so *History Entry Discipline* applies unchanged: one entry per artifact shown. Keyboard follows the WAI-ARIA tab pattern with **manual** activation — Left/Right/Home/End move focus among tabs, Enter or Space activates. Automatic activation would push a history entry for every tab the focus passes over.

The Tasks tab carries the progress meter and completion glyph the Tasks tree node carried (`visual-identity`: *Task Progress Meter*), fed by the same `totalTasks`/`completedTasks`.

### D4. The instance switcher is a row of buttons, not a `<select>`

Each instance renders as a button labelled by its worktree folder basename, followed by its branch chip (tinted per the identity header's rule) and its divergence label where it has one. The selected instance is marked. The archive's copy switcher is the same control with copy labels and no chip.

*Rejected — keep the archive's `<select>`.* A native option cannot carry a chip or a divergence label, and adopting it for live changes would strip the switcher of the two things that distinguish one worktree from another.

*Rejected — a menu.* Two or three instances is the realistic count; a menu hides them behind a click for no gain.

### D5. A multi-instance change is one two-line row

The sidebar row for a logical change with several instances keeps the two-line layout. Line 2's leading edge shows an **instance count chip** (`2 worktrees`) where a singleton shows its branch chip; the trailing status — meter or glyph, modification time — is the default instance's (D1). The divergence label is not shown on the row; it is shown per instance in the switcher. The favourite toggle keys on the logical change as before.

*Rejected — one row per instance.* Recreates a level of depth, and the address already names the instance, so nothing is gained that the header does not give.

### D6. Top-level disclosure is session-only

A top-level row keeps its chevron and opens by default; closing it lasts for the session. The `collapsedTreeNodeIds`/`expandedTreeNodeIds` settings, their IPC commands and the `expanded` set (which only ever held completed Tasks groups) become unread; the settings fields stay so an older settings file still parses, per the tolerance `document-width` shows an unknown value. A reveal opens a row transiently exactly as today, with nothing left it could write.

*Rejected — persist collapse for top-level rows only.* Keeps two settings commands and two IPC arms alive for one level of a tree that renders ten rows.

### D7. The outline is derived from the rendered AST, not from `ChangeData.sections`

`MarkdownView` already renders through a `components` map that sees each node's source position. Heading components record `{ level, text, line, id }` for levels 2–3 into a list the document view owns; the outline renders from that list. Scrolling reuses the `data-line` mechanism the task anchor uses today — the heading component stamps its line the same way `li` does. Identifiers follow the common GitHub derivation (lower-case, strip punctuation, hyphenate, `-n` suffix on duplicates), applied in document order by one pure function that is unit-tested.

Per-section progress is an **optional** `sections?: Section[]` prop on `DocumentView`, matched to entries by heading text. Only `App` passes it, and only for a live change's tasks artifact. The archive and file preview pass nothing, so they show no counts by construction.

*Rejected — derive the outline from `ChangeData.sections`.* Tasks-only, absent for archived changes and file previews, and it would show sections the rendered document does not (a parse disagreement) with no way to notice.

*Rejected — include level-four headings.* A capability spec has a scenario heading per scenario; the outline would be longer than the document.

### D8. Placement is a container query against the pane, per reading-width rung

The document view's root becomes an inline-size container. The outline occupies the column's trailing gutter, sticky below the header, and is displayed only when `@container (min-width: <column + outline + gutters>)` holds. The threshold differs per rung, and the rung is already stamped on the document root by `document-width`, so the rule is written once per rung selector rather than computed — `--doc-column` can hold the keyword `none`, so no rule may do arithmetic on it (`App.css` says so already). At `full` no rule matches.

The column is centred in the pane today; the outline sits in the trailing half of the free space, so the column does not move when it appears (`document-outline`: *Outline Placement and Visibility*). Current-entry tracking uses an `IntersectionObserver` on the headings, rooted at the scroll port.

*Rejected — a `ResizeObserver` in JS deciding visibility.* Correct, but a layout decision expressed in JavaScript reflows a frame late and needs a second implementation for reader windows; the container query is declarative and reaches both.

### D9. The selection union shrinks to three variants

`TreeSelection` becomes `workspace | repo | change`, where `change` carries its container (repo id or workspace uri) and the logical change name. `scrollAnchorForSelection` is deleted with its `default: return null`; `ScrollAnchor` survives as `{ kind: "line"; line: number } | null` for the outline and fragment links. `repoIdForSelection` and `renderTargetForSelection` keep their exhaustive switches over three arms. `nodeId.ts` returns a two-element path, `[containerId, changeRowId]`, or one element for a files address.

### D10. Fragment links become live as part of the outline, not separately

Heading identifiers exist for the outline; a fragment-only link resolves against the same identifiers by the same scroll. `MarkdownView`'s link interception gains one class — fragment-only, matched heading — that scrolls; an unmatched fragment falls into the existing dangling-link indication. Nothing else in link handling changes, and a path-plus-fragment link keeps its path class.

## Risks / Trade-offs

- **The largest deletion is in the most-tested component.** `WorkspaceTree.tsx` loses more than half its lines and the keyboard model is rewritten. The roving-tabindex contract is kept (one Tab stop, arrow traversal, typeahead, focus survival on refresh) but re-implemented over two levels; the existing keyboard scenarios that survive are the regression net, and each retired one is retired in the spec, not silently.
- **Losing the jump to a task line.** A reader who navigated by task will land on the section instead. Accepted in the proposal; the per-section counts keep the "where is the work" question answerable.
- **Outline thresholds are tuned by hand per rung.** Four numbers in CSS. Wrong ones either hide the outline on a pane that had room or overlap the column. The smoke tasks check each rung at the boundary.
- **Identifier collisions with authored anchors.** A document that already carries an explicit id on a heading (raw HTML) is not handled; markdown headings are the only source. Documented rather than solved.
- **The archive reader's title line moves** from its own header into the shared header's leading row. Same information, new position; `archive-browser`'s scenarios are updated to say where.
- **`src/CLAUDE.md` describes the nine-variant union.** It must be rewritten in the same change or the next reader is told to update switches that no longer exist.
- **Mutation gate short-circuits.** The diff touches no gated crate. Coverage is TypeScript unit tests on the pure functions (tab derivation, default artifact and instance, outline and identifier derivation, section matching, node path) plus the manual smoke in both hosts.
