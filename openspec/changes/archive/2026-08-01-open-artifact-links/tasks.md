# Tasks — Artifact Link Handling

Group 1 builds the validated resolver as shared, GUI-free service logic; group 2 wires the
desktop opener and the navigation backstop behind it; group 3 teaches the renderer; groups
4–5 make the web and terminal behaviour explicit rather than accidental.

## 1. The validated chokepoint (shared service, unit-testable)

- [x] 1.1 Add an `open_artifact_link(root, base_path, href)` resolution-and-authorization operation to `crates/openspec-app/src/service.rs`. Authorize `root` with the same browse-root rule `ensure_browse_root` applies (registered/registry-discovered workspace, or a repository main worktree accepted because a worktree of that repository is registered) — file-browser previews pass roots that need not themselves be registered. Classify the href: `http(s)`/`mailto:`/`tel:` → external; relative markdown (case-insensitive, mirroring `read_workspace_file`'s `eq_ignore_ascii_case`), fragment-only, and all other schemes → inert; remaining relative hrefs → file. For file hrefs: strip fragment and query, percent-decode exactly once, guard `base_path` with the same up-front rejections `read_workspace_file` applies (no absolute paths, no `..` components), resolve against `parent(base_path)`, canonicalise with the dunce-backed `paths::canonicalize`, require the canonical target to start with the canonical root — canonicalising **before** the containment check, so symlinks and encoded traversals are refused — then require a case-insensitive document-type allow-list match (`.html`, `.htm`, `.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`, `.css`, `.pdf`, `.txt`, `.json`, `.csv`) and refuse directories
- [x] 1.2 Return a classified result (external URL to open / validated path to open / inert / refusal reason) rather than performing any I/O beyond canonicalisation, so the operation is unit-testable without a GUI and reusable by any frontend
- [x] 1.3 Unit-test in `openspec-app`: unauthorized root refused before resolution; accepted main-worktree root authorized; `..` traversal refused, plain and percent-encoded (`%2e%2e%2f`); symlink-outside-root refused; target anywhere *inside* the root (outside the change directory) allowed; nonexistent target reported as such; `./my%20file.html` resolves; `./login.html#hero` and `?v=2` open `login.html`; `.md` and `.MD` hrefs classified inert; `javascript:`/`file:`/fragment hrefs classified inert; `http(s)`/`mailto:` classified external without path resolution; `./run.sh` and a directory target refused by the allow-list; containment on a `\\wsl.localhost`-rooted workspace passes/refuses correctly (pure path logic, runs on every platform)

## 2. Desktop opener and navigation backstop (Tauri shell)

- [x] 2.1 Add `tauri-plugin-opener` to `crates/specforge/Cargo.toml` and register it in `lib.rs`; do **not** add the JS package or any `opener:*` permission to `capabilities/default.json` — the plugin is invoked from Rust only, behind the validated command
- [x] 2.2 Add the `open_artifact_link` command to `crates/specforge/src/commands.rs`: call the service operation, then open the validated result via the opener's Rust API (URL for external, path for workspace files); refusals and dangling targets return an error the frontend can surface quietly
- [x] 2.3 Register an `on_navigation` guard on the main webview permitting only the app's own origin/devUrl and denying everything else — the backstop for activation paths no DOM handler sees (webview context-menu open-link, link drag-out) and for any future renderer regression

## 3. Renderer interception (rich frontend)

- [x] 3.1 Add an `a` component override to `src/components/MarkdownView.tsx` that calls `preventDefault()` for **every** anchor click and dispatches by class: external (`http(s)`/`mailto:`/`tel:`) → command; relative non-markdown → command; relative markdown (case-insensitive), fragment-only, and all other schemes → inert. `MarkdownView` accepts the viewed file's root-relative `basePath` as a prop; the frontend classification is for affordances only — the service re-classifies authoritatively
- [x] 3.2 Wrap the command in `src/api.ts` via `invokeLogged`
- [x] 3.3 Thread `basePath` (and the matching root) from every surface that renders `MarkdownView`: the detail pane derives it from the artifact reference it already holds (`openspec/changes/<id>/proposal.md`, `…/design.md`, `…/tasks.md`, `…/specs/<capability>/spec.md`) with the registered workspace as root; the workspace file browser passes the browsed file's path with its browse root; any archive-side markdown view passes its `openspec/changes/archive/…` path
- [x] 3.4 Distinguish link affordances visually (external vs workspace-file vs inert) using existing design tokens, and surface open-failures as a quiet transient indication in the pane — no blanking, no navigation, matching the invalid-mermaid tone
- [x] 3.5 Verify in the running app (`bun tauri dev`): an `http(s)` link opens the browser and the app view stays put; a `./mockups/*.html` link opens in the browser with its sibling CSS applied; a link with `#fragment` opens; a dangling link and a `./run.sh` link show the quiet failure; a `javascript:` href does nothing; right-click → open-link on an external URL does not navigate the app window

## 4. Web skin degrades explicitly

- [x] 4.1 When the shared bundle runs on its web transport path (runtime host detection in `src/api.ts` — there is no separate web bundle), render external links with `target="_blank" rel="noopener noreferrer"`, render workspace-file links as non-navigating affordances that present the target path, and confirm `crates/specforge-web`'s dispatch surface does not expose the open operation (add the not-mirrored rejection test)

## 5. Terminal renders the destination

- [x] 5.1 Present link destinations in the TUI's markdown rendering — OSC 8 hyperlinks where the hosting terminal is known to support them, the target shown textually otherwise, so the destination is never swallowed — and confirm the terminal frontend spawns no opener process for link content
