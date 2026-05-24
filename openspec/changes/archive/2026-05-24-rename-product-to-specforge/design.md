## Context

The application was scaffolded as "OpenSpec Tray" — a name that described its tray-icon surface and the format it reads. Since then the brand has resolved to "SpecForge" (matching the repo directory and `github.com/avantmedia/specforge`), but the rename never propagated. Today, three names coexist: the SpecForge brand (repo + URL), "OpenSpec" (everything user-visible), and "OpenSpec Tray" (Cargo/npm/bundle-id and the archived bootstrap proposal).

The non-obvious wrinkle: the app reads a spec format also called **OpenSpec** (the `openspec/` directory convention used by this project itself). A blind find-replace would corrupt valid references to that format — the `NotAnOpenSpecWorkspace` error type, parser paths that join `"openspec"`, the file-dialog title "Choose an OpenSpec workspace folder", and so on. The rename must distinguish two senses of "OpenSpec":

1. **OpenSpec the format** — paths, error types, copy that describes what the user is selecting/parsing. Stays.
2. **OpenSpec the (legacy) product name** — `productName`, window title, tray menu strings, bundle id, crate/npm names. Becomes SpecForge.

## Goals / Non-Goals

**Goals:**

- Every user-visible product surface presents the app as "SpecForge".
- Every developer-facing identifier for *the app* (crate name, npm name, bundle id, lib name) reads `specforge`.
- Every reference to the OpenSpec spec format is left intact.
- Capture the brand-vs-format distinction as an enforceable spec (`product-identity`) so future copy changes have a rule to consult.
- Preserve git history across the crate directory rename via `git mv`.

**Non-Goals:**

- Renaming `openspec-core`. It is a parser for the OpenSpec format; the name accurately describes it, and keeping it leaves the door open to publishing it as an independent crate.
- Changing the GitHub repo URL, organisation, or project description (all already "SpecForge").
- Migrating any persisted user data from `com.avantmedia.openspec-tray` → `com.avantmedia.specforge` on macOS. We're at `0.1.0` with no released builds; the only affected install is the maintainer's dev machine, who can re-register their workspace once.
- Touching Tauri command names, event names (`cache-updated`, `change-added`, `change-archived`), or the IPC contract with the frontend. None of these leak the product name.
- Renaming files inside `crates/openspec-tray/src/` (e.g. `tray.rs`, `events.rs`, `commands.rs`). The crate directory rename is enough.

## Decisions

### Decision: Distinguish "OpenSpec the format" from "SpecForge the product" in a written spec, not just in this change's prose

The brand-vs-format distinction will continue to bite anyone who edits user copy or adds a new surface. Encoding it as requirements in a new `product-identity` capability spec gives future changes a normative rule to apply ("does this string identify the app or describe the format it's reading?"). Without that spec, the next person to add a settings page or onboarding screen will re-introduce the same conflation.

**Alternative considered:** leave the distinction in this change's design.md and trust reviewer memory. Rejected because design.md lives under `openspec/changes/` and gets archived once the change ships, so it's not where future changes will look.

### Decision: Keep `openspec-core` named as-is

`openspec-core` is the headless parser of the OpenSpec format. Renaming it to `specforge-core` would (a) misdescribe it — it parses any OpenSpec workspace, not just SpecForge's — and (b) close off a useful future option (publishing it independently). The crate has no user-visible footprint, so the name has no branding cost.

**Alternative considered:** rename both crates to `specforge-core` and `specforge-app` for uniformity. Rejected as above.

### Decision: Rename the bundle identifier to `com.avantmedia.specforge`, accept the per-user-data reset

The Tauri bundle identifier becomes the macOS application identifier and determines where `tauri-plugin-window-state` and our config file land on disk. Changing it abandons whatever the prior identifier stored. At `0.1.0` with no released builds this is essentially free — only the maintainer's local install is affected, and re-registering one workspace is trivial. A post-release rename would need a migration step (read from the old path, write to the new) or a documented one-time reset; neither is justified to write now.

**Alternative considered:** keep `com.avantmedia.openspec-tray` to avoid the data reset. Rejected — the identifier is publicly visible (it appears in macOS's app metadata, crash reports, and the autostart plist) and would be the one place that still said "OpenSpec Tray" in shipped builds.

### Decision: Use `git mv` for the crate directory rename

`crates/openspec-tray/` → `crates/specforge/` happens via `git mv` so blame/history flow through. This also means the Cargo workspace member entry in the root `Cargo.toml` must be updated in the same commit. The crate's internal file structure (`src/lib.rs`, `src/tray.rs`, etc.) stays put.

### Decision: Tray menu labels become "Show SpecForge" / "Quit SpecForge"

Standard macOS-style menu phrasing, matches the new product name. The tray tooltip becomes plain `"SpecForge"` — the dynamic per-workspace text (`tray.rs:90` fallback path) is already title-derived and needs no rebrand.

## Risks / Trade-offs

- **[Risk] Accidental over-rename inside `openspec-core` or parser paths breaks workspace detection.** → Mitigation: the spec for `product-identity` explicitly enumerates the "format references that MUST be preserved" and the tasks list applies the rename surgically file-by-file, not via a sweeping find-replace. The implementer reviews each `openspec`-substring match before changing it.
- **[Risk] Bundle-id change loses the maintainer's current settings (registered workspaces, window position).** → Mitigation: acknowledged and accepted; documented in the migration plan below. One-time re-registration after first launch of the renamed build.
- **[Risk] `git mv` of `crates/openspec-tray/` confuses anyone with in-flight branches against the old path.** → Mitigation: there are no other in-flight changes (`openspec list` is empty), and the only existing branch is the bootstrap which is archived. Low concern.
- **[Trade-off] Keeping `openspec-core` named as it is means the project ships with a SpecForge product whose core crate has a different brand prefix. This is mildly inconsistent.** → Accepted because the crate's name is *descriptively* correct (it parses OpenSpec), and the inconsistency is invisible to users.

## Migration Plan

1. Land the rename in a single commit (or tightly grouped commits) on a feature branch.
2. On first run of the renamed build on the maintainer's machine, the app starts with an empty workspace registry and default window state — both expected. Maintainer re-registers their workspace via the existing settings flow.
3. Old `com.avantmedia.openspec-tray` data on disk (under `~/Library/Application Support/`) can be deleted manually; we do not write a migration tool.
4. Rollback: revert the commit. Bundle identifier reverts; same data-reset story applies in reverse.

## Open Questions

None. The brand-vs-format line is sharp, the keep-list for OpenSpec format references is exhaustively derivable from grep, and the bundle-id decision is unambiguous at this release stage.
