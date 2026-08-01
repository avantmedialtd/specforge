# Design — Artifact Link Handling

## Decision 1 — Open externally; no in-app preview

The decisive property is **sibling assets**. A real HTML mockup is a directory, not a file — `login.html` references `./styles.css`, `./app.js`, images. Any in-app preview that reads one file's bytes into an iframe (`srcdoc`) breaks every relative reference; fixing that means a custom asset protocol that serves whole directories into the webview. And the app ships `"csp": null`, so a mockup's scripts would run unconstrained inside SpecForge's own webview.

| | External open | In-app iframe |
|---|---|---|
| Sibling assets (css/js/img) | free (`file://` in the browser) | needs a custom asset protocol |
| Script sandboxing | the browser's | ours, with `csp: null` |
| Dev tooling on the mockup | browser devtools | none |
| Effort | one command + one override | large |

Opening externally forecloses nothing: the interception point and the validated resolver are exactly what a future in-app preview would sit behind.

## Decision 2 — One validated chokepoint; the opener is Rust-side only

The frontend never gains a general "open anything" capability. It invokes one command:

```
open_artifact_link(root, basePath, href)
```

- `root` — the authorized root the rendering surface already holds: the registered workspace folder for artifact views, or the browse root for file-browser previews. These are **not the same authorization**: browse roots include a repository's main worktree when any worktree of that repository is registered, even if that main worktree is not itself a registered workspace (the `ensure_browse_root` rule, per *Browsing Is Confined to Registered Workspaces* in `workspace-file-browser`). The open operation authorizes `root` with that same browse-root rule — a superset of registry membership — so a link clicked in a file-browser preview under an unregistered main worktree opens rather than landing in the quiet-failure path.
- `basePath` — the root-relative path of the markdown file *being viewed* (e.g. `openspec/changes/add-login/proposal.md`). Relative hrefs resolve against its parent directory.
- `href` — the raw href from the anchor.

The shared service (`openspec-app`) classifies and authorizes; only then does the Tauri command hand the result to `tauri-plugin-opener`'s **Rust API**. No `@tauri-apps/plugin-opener` JS package, no `opener:*` permission in `capabilities/default.json` — the plugin's invoke surface stays closed and the only frontend-visible operation is the validated command. This mirrors the `git_command` chokepoint precedent: one funnel, validation before every use.

External links go through the same command (the service classifies the href as external and the shell opens the URL), so the frontend contains no security-relevant dispatch — it classifies only for affordances (cursor, glyph, tooltip).

## Decision 3 — Containment boundary: the authorized root

Mockups can live anywhere in the project — `design/mockups/` at the repo root is as valid as `openspec/changes/<id>/mockups/`. So the boundary is the **authorized root**, deliberately wider than the `openspec/changes/` subtree that bounds artifact *reads*. The widening is safe because the operation reads and returns no file content over IPC; combined with Decision 4's document-type allow-list, its only effect is asking the OS to display a document inside a folder the user brought into the app.

One narrowing is recorded explicitly, because it interprets the "mockups can live anywhere" decision: *anywhere* is bounded to the authorized root. A link to a sibling checkout (`../shared-designs/nav.html`), an absolute path, or anything else outside the root is refused — containment **is** the security story. A user wanting cross-repo mockups registers that folder as a workspace and links within it.

Resolution pipeline in the service:

```
authorize root (browse-root rule: registered workspace,
    or main worktree of a repo with a registered worktree)
   ↓
classify scheme: http/https/mailto/tel → external; other schemes / fragment-only → inert
   ↓
strip fragment and query from the relative href, percent-decode exactly once
   ↓
reject absolute hrefs and guard basePath (no absolute paths, no `..` components,
    same up-front rejections read_workspace_file applies)
   ↓
join(parent(basePath), decoded href) → canonicalize    (fails → target missing → quiet error)
   ↓
canonical target starts_with canonical root             (else refused)
   ↓
document-type allow-list, directories refused           (Decision 4)
   ↓
open via the OS default handler
```

Fragment/query stripping and single decoding come **before** the join so `./mockups/login.html#hero` and `./my%20file.html` resolve to real files; they are irrelevant to safety because containment — `starts_with` on canonical paths, computed *after* decoding and joining — is the sole traversal authority. An encoded `%2e%2e%2f` decodes, joins, canonicalises, and fails containment like any other escape. Canonicalising before the containment check is also what closes the symlink escape: a symlink inside the root pointing outside resolves to its real path and fails `starts_with`.

Two residual notes, considered and accepted:

- **TOCTOU.** The target can be swapped between the containment check and the opener invocation. Accepted: the boundary defends against *authored content* (links written into specs), not against a concurrently-writing local attacker, who already has user-level filesystem access and needs no help from us.
- **WSL workspaces (Windows).** For a `\\wsl.localhost\<distro>\…` workspace, containment compares dunce-backed canonical forms via the same `paths::canonicalize` every `RepoId`-forming site uses, so verbatim/UNC variants cannot split one root into two identities; the validated UNC path is then handed to the opener (ShellExecute) like any other Windows path, and the browser resolves sibling assets over `file://wsl.localhost/…`. The pure path logic is unit-testable everywhere; the runtime behaviour joins the existing list of things needing a real Windows+WSL2 box to verify.

## Decision 4 — Only document types open

"OS default handler" is **execution** for some target types: on macOS `open x.command` runs it in Terminal and `open x.app` launches the bundle (cloned files carry no quarantine attribute), on Windows ShellExecute runs `.exe`/`.bat`/`.cmd` directly. Workspaces are often cloned third-party repos, and artifact authors are not always the person clicking. Registering a folder for spec *browsing* is not consent to one-click execution of arbitrary files in it.

So the resolver opens only targets on a **document-type allow-list** — initially `.html`/`.htm`, images (`.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`, `.webp`, `.avif`), `.css`, `.pdf`, `.txt`, `.json`, `.csv`, matched case-insensitively — and refuses directories (which also covers `.app` bundles, since bundles are directories). Everything else, executable or merely unknown, is refused into the quiet-failure path. An allow-list needs no executable-bit checks and no ever-incomplete deny-list of script extensions; extending it is a one-line spec change. Sibling assets are unaffected — the browser loads a mockup's `./app.js` itself; the allow-list governs only what the *clicked target* may be.

## Decision 5 — Default-deny interception, with a shell-side backstop

The `a` override in `MarkdownView` calls `preventDefault()` unconditionally — no href class is ever handed to the webview's navigator — then dispatches:

| href | Behaviour |
|---|---|
| `http:` / `https:` | open in system browser (via the command) |
| `mailto:` / `tel:` | open via the OS handler (via the command) — contact links in specs are as safe as `http(s)` and going inert would read as broken |
| relative, non-markdown | resolve + validate + open via OS default handler (Decisions 3–4) |
| relative markdown (`.md`/`.markdown`, case-insensitive) | **inert** — reserved for future in-app navigation |
| fragment-only (`#…`) | inert — headings carry no ids today; in-pane scroll is a separate feature |
| any other scheme (`javascript:`, `file:`, `data:`, …) | inert |

Markdown matching is case-insensitive (mirroring `read_workspace_file`'s `eq_ignore_ascii_case`) and agreed between the frontend classifier and the service, so `./NOTES.MD` cannot slip through as "non-markdown" and open in a text editor — exactly the behaviour the inert class exists to prevent, protecting the v2 in-app-navigation path through the `TreeSelection` union. Inert links get a distinguishing affordance so a dead link reads as policy, not breakage.

A DOM click handler alone cannot deliver "never navigates": the WKWebView context menu's *Open Link* navigates the main frame without dispatching any click event, and link drag-out is likewise outside it. The shell therefore registers an `on_navigation` guard on the main webview permitting only the app's own origin/devUrl and denying everything else — the backstop that catches every renderer bypass, present and future. Defence in depth: the override handles links correctly; the guard guarantees the invariant.

## Decision 6 — Per-frontend behaviour, on the mermaid template

The *Mermaid Diagram Rendering* requirement established the shape: a rich-frontend enhancement, backend payload unchanged, `terminal-ui` degrades explicitly. Link handling follows it:

- **Desktop** — the only frontend that opens anything. Opening is inherently a "this machine" action, and the desktop shell is the only frontend that *is* the user's machine.
- **Web skin** — the shared bundle, on its runtime-detected web transport path, opens external links in a new tab with an opener-isolated window (`rel="noopener noreferrer"`); workspace-file links do not navigate and instead present the target path (the file exists on the *server's* filesystem, not necessarily the viewer's). The web dispatch surface does not expose `open_artifact_link` at all — a browser must never be able to make the server host open files. This is a deliberate carve-out from the command-transport mirror contract, recorded in the `web-ui` delta.
- **Terminal** — renders the link with its destination discoverable. It may emit an OSC 8 terminal hyperlink where the hosting terminal is known to support it, but falls back to showing the target textually otherwise — same capability-ladder shape as the existing *Graceful Degradation* requirement — so the destination is never silently swallowed. The TUI process spawns nothing.

## Decision 7 — Quiet failure, like an invalid diagram

A dangling href (`./mockups/old.html` after a rename), a refused target (outside the root, not on the allow-list, a directory), or an unauthorized root must not blank the pane, crash, or navigate. The command returns an error; the frontend surfaces it as a quiet, transient indication that the link could not be opened — the same tone the mermaid requirement mandates for invalid diagram source. The rendered artifact stays fully usable.
