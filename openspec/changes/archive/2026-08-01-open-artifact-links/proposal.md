# Artifact Link Handling

## Why

Clicking any link in a rendered artifact today navigates the **entire WebView away from the app UI**. `MarkdownView.tsx` overrides `li`, `input`, and `pre` (the mermaid interception) but has no `a` override; the Tauri shell registers no navigation guard; and `tauri.conf.json` ships `"csp": null` — so nothing between the anchor and the WKWebView stops the default navigation. A stray link in a proposal takes the whole app with it.

At the same time, links are the natural way to attach **HTML mockups** to specs: a proposal that says `[login mockup](./mockups/login.html)` should let the reader open that mockup with one click. Today that file is not even *readable* through the app — `read_artifact` serves a fixed four-file whitelist, and `read_workspace_file` hard-rejects anything that is not `.md` — and the click itself is just the hijack above.

Both problems have one fix: intercept every link click in rendered markdown and give each link class a deliberate behaviour, with the system opener doing the actual opening.

An in-app HTML preview was considered and rejected for v1: real mockups reference sibling assets (`./styles.css`, `./app.js`, images) that break under any read-one-file-into-an-iframe scheme, and with `csp: null` the app's own webview is the wrong sandbox for third-party scripts. Handing the OS a path gives the default browser's rendering, sandboxing, and devtools for free.

## What Changes

- **Intercept all link activations in rendered markdown.** An `a` component override in `MarkdownView` prevents default navigation for every link and dispatches by class, and a shell-side `on_navigation` guard (permitting only the app's own origin) backstops the paths a DOM handler cannot see — the webview context menu's open-link action, link drag-out. The webview never navigates away, for any href, by any route.
- **External links open externally.** `http(s)` links open in the system browser; `mailto:`/`tel:` links open via the OS handler. This fixes the hijack for the most common link kinds.
- **Relative file links open via the OS default handler, gated by a document-type allow-list.** The href — fragment/query stripped, percent-decoded once — is resolved against the *viewed file's* directory, validated, and opened with the system opener; for `.html` that is the default browser, which resolves the mockup's sibling assets itself. Mockups may live anywhere inside the authorized root, not just inside the change directory ("anywhere" is deliberately bounded to that root — containment *is* the security story; cross-repo mockups mean registering that folder too). Only document types (`.html`, images, `.css`, `.pdf`, `.txt`, `.json`, `.csv`) open; executables, scripts, and directories are refused — a link can never execute a file.
- **A single validated chokepoint.** One new shared-service operation authorizes the root by the same rule that authorizes file browsing (registered workspace, or a repository main worktree accepted because a worktree of that repository is registered), then resolves and contains the target (canonicalised `starts_with`, which also closes symlink and encoded-traversal escapes) before the desktop shell invokes the opener. The opener plugin is used from Rust only — no JS opener surface, no opener permission exposed to the frontend.
- **Everything else is deliberately inert, and visibly so.** Relative markdown links (case-insensitive; reserved for future in-app navigation), fragment-only hrefs, and non-openable schemes (`javascript:`, `file:`, `data:`, …) do nothing, and carry an affordance distinguishing policy from breakage — matching the *Deferred Interaction Nodes* and inert-checkbox precedents.
- **Per-frontend behaviour, mermaid-style.** Opening is a desktop-frontend concern. On the web transport path, external links open in a new tab with an opener-isolated window and workspace-file links degrade to a non-navigating presentation of the target path; the open operation is deliberately absent from the web dispatch surface (a recorded carve-out from the command-transport mirror contract), so no browser request can make the server host open files. The terminal UI renders the link with its destination visible — OSC 8 where supported, textual otherwise — and never spawns an opener.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `spec-browser`: adds a *Link Handling in Rendered Artifacts* requirement — default-deny interception with a shell navigation backstop, external links to the system browser, workspace-file links resolved against the viewed file and opened via the OS default handler subject to browse-root authorization + canonicalised containment + a document-type allow-list enforced at the shared application boundary, inert classes, and quiet failure.
- `web-ui`: extends *Command Transport Mirrors the In-Process Command Surface* with a carve-out for host-side effectful operations, and adds a *Link Handling in the Browser Skin* requirement — external links in a new tab with an opener-isolated window, workspace-file links degrade without navigating, and no server-side open endpoint.
- `terminal-ui`: extends *Artifact Markdown Rendering* — link destinations discoverable (OSC 8 where the terminal supports it, textual fallback otherwise); the terminal frontend spawns no opener process.
- `workspace-file-browser`: adds *Preview Link Handling* — the preview's rendered markdown is governed by the spec-browser link contract with the browse root as resolution and containment root; listings and the guarded read remain markdown-only.

## Impact

- **No changes to existing IPC payloads or `src/types.ts` types.** One new command (`open_artifact_link`) taking three strings; no parser, cache, or watcher changes; `ArtifactStatus` and the change payload are untouched.
- `crates/openspec-app/src/service.rs`: new resolve-and-authorize operation applying the same up-front rejections `read_workspace_file` applies (absolute paths, `..` components), then canonicalise-and-contain via `starts_with` on canonical paths — which also closes symlink escapes — plus the document-type allow-list. Root authorization reuses the browse-root rule so file-browser previews under an accepted main worktree can open links.
- `crates/specforge/`: `tauri-plugin-opener` dependency and registration in `lib.rs`; a thin command in `commands.rs` that calls the service resolver then the opener's Rust API; an `on_navigation` guard on the main webview.
- `src/components/MarkdownView.tsx`: the `a` override and link classification; `src/api.ts`: the wrapped command; every surface that renders `MarkdownView` supplies the viewed file's root-relative path as the resolution base.
- `crates/specforge-web/`: link handling stays client-side; the dispatch surface does not expose the open operation.
- **Windows/WSL:** containment for `\\wsl.localhost` workspaces compares dunce-canonical forms via the existing `paths::canonicalize`; the runtime behaviour (ShellExecute on a 9P UNC path, browser loading sibling assets over `file://wsl.localhost/…`) joins the existing needs-a-real-Windows+WSL2-box verification list.
- **Deliberately not addressed:** in-app HTML preview (needs an asset-serving story), in-app navigation for relative `.md` links (the natural v2 use of the same interception point), and listing `.html` files in the workspace file browser — links from specs *are* the discovery mechanism, and the browser's `.md`-only contract is unchanged.
