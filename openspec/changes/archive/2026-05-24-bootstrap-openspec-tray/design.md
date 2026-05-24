# Design

## Context

The repository (`specforge`) is greenfield apart from the `openspec/` scaffold and `LICENSE`. The target product is a cross-platform menu-bar app that surfaces OpenSpec state for several registered workspaces at a glance.

A reference implementation already exists as a VSCode extension at `../artifex/vscode-extension`. That extension uses the VSCode native `TreeDataProvider` API (no webview, no React, no markdown rendering) to show a workspace → change → artifact → section → task tree. Its `taskParser.ts`, `titleExtractor.ts`, and `types.ts` define the OpenSpec parsing logic and data model that this project will port to Rust. Its UI is not portable.

The constraints that shaped this design:

- The badge is the product. If its semantics are wrong the icon becomes noise within a week.
- The tray must not bounce to the IDE. Opening VSCode on click defeats the entire premise.
- The Rust + Tauri choice is settled. So is the React + TypeScript + Vite frontend.
- The user already runs this exact tree view in VSCode every day and wants it summoned outside the editor.

## Goals / Non-Goals

**Goals:**

- Surface a single number ("how many active changes exist across all my workspaces") permanently in the menu bar.
- Let the user open a window that shows the same tree they see in VSCode, with the markdown of any selected artifact rendered alongside.
- Make the window behave like a regular macOS application (Dock icon, normal chrome, persisted state) — *not* a slide-down popover.
- Establish a headless Rust core (`openspec-core`) that any future CLI, daemon, or alternate UI can consume without rewriting the parsing or watching layer.
- Ship a signed and notarised macOS binary first; keep Windows / Linux compilable on the same codebase.

**Non-Goals:**

- Editing specs.
- Interactive task checkboxes (toggling `[ ]` / `[x]` from the UI).
- State transitions initiated from the UI (e.g. "archive this change" buttons).
- Inbox / unread semantics for the badge.
- Worktree detection or grouping.
- Auto-discovery of workspaces (no scanning `~/Developer`, no parsing VSCode's `Workspaces` files).
- Sync from git or remote OpenSpec repositories.
- A settings UI for fine-grained notification rules; v1 ships sensible defaults only.
- Spec-to-spec link navigation inside rendered markdown.
- Multi-language UI.

## Decisions

### Two-crate workspace: `openspec-core` + `openspec-tray`

The Rust workspace splits cleanly into a headless core and a Tauri shell.

- `openspec-core` owns: the registered-workspace list and its persistence, filesystem watching, OpenSpec parsing, the in-memory cache, and the Tauri command / event surface that the frontend talks to.
- `openspec-tray` owns: the Tauri application, the tray icon and badge, window creation and lifecycle, autostart and notification plugin wiring.

**Alternatives considered:** A single crate. Rejected because (a) UI tests are painful and headless-core tests are straightforward, and (b) every future option (CLI, daemon, alternate UI) becomes a rewrite if the parser lives inside the Tauri binary. The cost of the split — a stable IPC boundary and serde-serializable shared types — is real but small at this scope.

### Port the parser to Rust (not run TypeScript in the webview)

The artifex extension's parser is ~500 lines of TypeScript. The two real options were to port it to Rust inside `openspec-core`, or to keep it in TypeScript and run it inside the WebView with the Rust layer reduced to file watching plus IPC.

The Rust port wins because the headless-core goal is the whole point of the crate split. Putting parsing in the WebView hollows out `openspec-core` and makes the "future CLI" non-fictional.

**Alternatives considered:** Keep TS parser, run in WebView. Faster to v1 but quietly defeats the architecture. Sidecar Node process. Rejected — adds a runtime dependency and process-management complexity for no upside.

**Implication:** the artifex VSCode extension's `taskParser.ts` becomes the spec for the Rust port. If both ship to users simultaneously they must produce identical `ChangeData` for the same input — worth golden tests against fixture markdown to keep them in sync until the extension is retired.

### Regular app window, not a tray popover

Earlier in design the assumption was a slide-down popover anchored to the tray icon, dismissed on focus loss. That has been replaced with a regular Tauri window — Dock icon visible while running (no `LSUIElement: true`), normal chrome, resizable, position and size persisted via `tauri-plugin-window-state`. Closing the window hides it; the tray icon keeps the app alive; Cmd-Q quits.

**Why:** the popover approach forces an 800×600-class window to behave like a 400×400 menu, fights focus-loss when child dialogs (folder pickers) take focus, and burns engineering effort on cross-platform tray-position math. The user already lives in the VSCode equivalent every day and is comfortable with normal window behaviour.

### Master-detail UI with rendered markdown in the detail pane

Left pane: the same tree the artifex extension exposes (workspace → change → 4 artifact nodes → sections → tasks). Right pane: rendered markdown of the selected artifact, using `react-markdown` + `remark-gfm` + `rehype-highlight`.

Click behaviour in v1:

| Node | Behaviour |
|---|---|
| Workspace | nothing |
| Change | nothing |
| Proposal | render `proposal.md` |
| Specs (artifact node) | nothing |
| Individual capability spec | render that `spec.md` |
| Design | render `design.md` |
| Tasks | render `tasks.md` |
| Section | scroll the rendered `tasks.md` to that section's heading |
| Individual task | scroll the rendered `tasks.md` to that task's line |

The "nothing" cases are deliberate v1 deferrals — they need their own UX design (workspace-level summary, change overview view) and would slow v1 without adding ambient-awareness value.

### "Active" means "not archived"

Active-change count, used both by the badge and as a filter for what shows in the tree by default, is defined as: directories inside `openspec/changes/` whose immediate parent is *not* `openspec/changes/archive/`. The check is a directory listing, not a parse. No frontmatter conventions, no per-change state machine, no "needs my action" heuristic.

**Alternatives considered:** Inherit the artifex extension's `getActiveChangesWithUncheckedTasks` definition (unfinished tasks). Rejected — it requires parsing every change just to count, and conflates "in progress" with "no tasks.md yet" with "designed but not started." `not archived` is cheap (one stat per directory), unambiguous, and matches what the user means when they say "active." A richer attention-queue model is a later change.

### Manual workspace registration

Workspaces are added explicitly through the settings view. No `~/Developer` glob, no manifest auto-discovery, no drag-folder-onto-tray. The list is persisted in a config file managed by `openspec-core`.

**Why:** the user explicitly excluded auto-discovery. Manual registration is also the right shape for a tool that aggregates across personal, Avant Media, and MushRoom projects — each user has a different set and an opt-in list is clearer than implicit detection.

### Notifications limited to new changes and state transitions

The notification surface is intentionally narrow: a new change directory appears, or an existing change moves to / from `archive/`. Per-file edits never notify. Sensible defaults only in v1; no settings UI for fine-grained rules.

### macOS first, cross-platform abstraction in place

Day-one development targets macOS (signing, packaging, polish). Windows and Linux compile but are not the support target for v1. The tray badge implementation goes behind a single `set_badge(count: Option<u32>)` Rust function so the macOS `with_title()` approach and the Windows / Linux icon-swap approach live in one place rather than spreading through the codebase.

## Risks / Trade-offs

- **[Parser drift between Rust port and TypeScript original]** → Maintain a fixture suite of OpenSpec change directories under `crates/openspec-core/tests/fixtures/`. Run the same fixtures through both implementations until the artifex extension is retired or the two converge on a shared protocol.
- **[Watcher self-write loops]** → Even though v1 is read-only, eventual interactive features (task toggling) will write to `tasks.md`. Track recently-written paths with a short TTL in the watcher so self-emitted events are filtered. Land the infrastructure now even though no writes happen in v1.
- **[macOS notarisation is a multi-evening side quest]** → Budget two evenings, not the £79. Decision already made to register the Apple Developer account under Avant Media Ltd so the cert signs anything the company ships in future. CI signing is deferred to a follow-up change.
- **[Tauri 2 plugin maturity]** → `tauri-plugin-autostart`, `tauri-plugin-notification`, `tauri-plugin-window-state` are all official and stable enough, but version pin everything explicitly in `Cargo.toml` and `package.json` to avoid silent breakage when Tauri 2 minor versions move.
- **[No auto-update mechanism in v1]** → A daemon-shaped app the user installs once and forgets needs auto-update or it stays at 0.1.0 forever. Out of scope for v1 but should be the first follow-up; the GitHub Releases JSON pattern with `tauri-plugin-updater` is the path of least resistance.
- **[WebView2 / WKWebView styling divergence]** → Acceptable in v1 since macOS is the target. Will need attention before Windows / Linux are promoted to first-class.
- **[Removing a registered workspace whose folder no longer exists]** → Graceful handling: a registered workspace whose folder has been deleted should appear with a clear "missing" state in settings, not crash the watcher.

## Open Questions

- **Settings UI activation surface.** Gear icon top-right of the main window is the working assumption — but a menu-bar app convention is "right-click the tray icon → Settings…". Both are easy to wire; left as an implementation-time decision.
- **Markdown link handling.** External links open in the system browser. Internal `[[link]]` style spec-to-spec navigation is explicitly out of scope, but raw `./design.md` style relative links inside rendered markdown need a deliberate decision: ignore, render as plain text, or treat as inert? Defaulting to "render as link, no-op on click" for v1.
- **Config file location.** macOS convention is `~/Library/Application Support/openspec-tray/config.json` via Tauri's `path_resolver`. Cross-platform equivalents follow `dirs` crate conventions. Pin during implementation.
