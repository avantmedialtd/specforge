## Context

SpecForge's introductions were written when it was a tray badge with a window behind it. That is still how the README, the bundle metadata, the About panel, the npm wrapper and the OpenSpec project context describe it (the proposal lists each line). The marketing site moved to "A visual companion" and "in full view", which is closer, but its "One change, end to end" arc still ends at commits and diffs. By v0.27.0, a change can be followed from its proposal to the pull request it became, with review progress kept per hunk.

The positioning was chosen in an explore session, against two alternatives (see D1). Existing contracts constrain the copy:

- `marketing-site` pins the H1 and bans "viewer" in site copy (*Landing page states the product's positioning*).
- `product-identity` requires the About tagline to stay consistent with `bundle.shortDescription` (*About Panel States Product and Format*).
- `site/e2e/tests/landing.spec.ts` pins the H1, the four H2s, five sections, three pillar articles, "spec-driven development" in the kicker and "supports OpenSpec today" in the hero summary.
- The site names no version number, and an e2e test enforces it.

## Goals / Non-Goals

**Goals:**

- One positioning statement on every surface that introduces SpecForge: it follows OpenSpec changes across repositories and worktrees, from the proposal to the pull request, read-only.
- Pull requests and the Dashboard named wherever the product is introduced.
- README statements about network access that are true.
- An agent context that describes the product the agents are actually building.

**Non-Goals:**

- New screenshots (`docs/screenshot.png`, `site/public/screenshot.*`). Both are still the v0.2.0 capture.
- The README's Architecture section, whose two-layer diagram predates `openspec-app`, `specforge-tui` and `specforge-web`.
- Any CSS, layout or behaviour change.
- Docs pages beyond the `/docs` intro. `site/pages/docs/settings` still says "the two quota gauges" and has no pull request content, which is a follow-up.
- `crates/specforge-tui/README.md` and the release notes.

## Decisions

### D1. Lead with the scope; name the surfaces second

**Chosen:** every introduction leads with "from proposal to pull request" across repositories and worktrees, then names the three frontends. The menu-bar or tray count appears as one of them.

**Rejected — agent-first ("where you keep watch over your coding agents"):** SpecForge has no agent integration. It makes no model calls and has no MCP server or hooks, and Claude Code appears only as the token source for the usage gauge (`crates/openspec-app/src/quota.rs`). It reads what agents leave behind, so this framing would overclaim.

**Rejected — surfaces-first ("wherever you look: menu bar, Dock, tmux, browser"):** it answers "menu-bar viewer" head-on but describes where SpecForge shows up, not what it shows. It would also suggest that every surface carries every feature, and the terminal UI has no pull request view.

### D2. Change the H1

**Chosen:** "Spec-driven work, from proposal to pull request." The H1 is the most prominent statement on the site, so it should carry the scope.

**Rejected — keep "Spec-driven work, in full view.":** it states a quality rather than a scope, the same move as "visual companion". Leaving it would put the new framing in the kicker alone, which is the smallest text in the hero.

### D3. Re-cut the arc inside three pillars

The pillar grid is three equal columns with dividers drawn by `first-child` and `last-child` rules (`site/src/styles.css:910-929`):

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 420 150" width="420" height="150" font-family="sans-serif" font-size="11">
  <text x="0" y="12" fill="currentColor">Chosen: three articles, arc re-cut</text>
  <rect x="0" y="20" width="130" height="40" fill="none" stroke="currentColor"/>
  <rect x="140" y="20" width="130" height="40" fill="none" stroke="currentColor"/>
  <rect x="280" y="20" width="130" height="40" fill="none" stroke="currentColor"/>
  <text x="8" y="44" fill="currentColor">01 Navigate + Dashboard</text>
  <text x="148" y="44" fill="currentColor">02 Read</text>
  <text x="288" y="44" fill="currentColor">03 Verify: Git → PR</text>
  <text x="0" y="86" fill="currentColor">Rejected: a fourth article in repeat(3, 1fr)</text>
  <rect x="0" y="94" width="130" height="22" fill="none" stroke="currentColor"/>
  <rect x="140" y="94" width="130" height="22" fill="none" stroke="currentColor"/>
  <rect x="280" y="94" width="130" height="22" fill="none" stroke="currentColor"/>
  <rect x="0" y="122" width="130" height="22" fill="none" stroke="currentColor" stroke-dasharray="4 3"/>
  <text x="140" y="137" fill="currentColor">04 Review wraps alone</text>
</svg>
```

**Chosen:** keep three articles. 01 / Navigate starts from the Dashboard, 02 / Read is unchanged, and 03 / Verify runs from commits and diffs to the pull request and the hunks already viewed. No CSS changes, and `landing.spec.ts:148` (three articles) still holds.

**Rejected — add "04 / Review" as a fourth article:** in `repeat(3, 1fr)` it wraps onto a row of its own at desktop widths, and the `last-child` padding rule then applies to the orphan.

**Rejected — switch to `repeat(4, 1fr)`:** each text column narrows to about three-quarters of its current width at the same 34 px gutters. That changes the hero's rhythm, which a copy change should not do.

### D4. Say exactly which features make network requests

**Chosen:**

- Rewrite the closing sentence of the Claude gauge bullet (`README.md:47`) so it names all four opt-in network features and says each is off by default.
- Correct the TUI Settings description (`README.md:73`) to cover all four toggles (`crates/specforge-tui/src/app.rs:33-39`).
- Add one sentence to the site's "Never changes your work" card about pull requests.

**Rejected — leave "This is SpecForge's only network call":** it has been false since the ChatGPT gauge and the pull request panels shipped, and a reader reasonably relies on it.

**Rejected — drop the sentence entirely:** "with the toggle off, nothing is read or sent" is the reassurance the bullet exists to give, and it is true of every one of the four.

### D5. Keep the menu bar as where the count appears

**Chosen:** the README intro, `bundle.longDescription` and the `/docs` steps keep the menu bar, system tray and status area, as the place the active-change count lives. The badge is real, it de-duplicates worktrees, and Getting started depends on it.

**Rejected — remove every mention of the menu bar:** that would overcorrect, and it would hide the one feature that makes the desktop app ambient.

### D6. Rewrite the agent context

**Chosen:** the first sentence of `context` in `openspec/config.yaml` describes the scope and the three frontends. Every `openspec instructions` call renders this block into the prompt, so it sets how agents describe SpecForge in new specs and docs.

**Rejected — leave it as it is:** the architecture paragraph below it already names `specforge-tui` and `specforge-web`, so the opening sentence is the only stale line. Left alone, it keeps steering new artifacts toward "a Tauri 2 desktop app that browses OpenSpec workspaces".

### D7. Pin contracts, not prose

**Chosen:** the specs quote verbatim only the short strings that act as identifiers: the H1, the title, the kicker, the pillars H2, `bundle.shortDescription`, the About tagline, the README headline and the npm `description`. Longer copy (the README intro, Why and Features, the site paragraphs, the `/docs` intro) is specified by what it must name, so its wording can still be edited.

**Rejected — pin every sentence:** each copy edit would then need a spec delta.

**Rejected — pin nothing new:** the old framing survived precisely because only the H1 was pinned.

### D8. The copy, surface by surface

| Surface | Copy |
|---|---|
| README headline (`README.md:7`) | **Follow spec-driven work across all your repositories, from proposal to pull request.** |
| `bundle.shortDescription` | SpecForge — follow OpenSpec changes from proposal to pull request, across your repositories |
| About tagline (`menu.rs:61`) | Follow OpenSpec changes from proposal to pull request, across your repositories. |
| npm wrapper `description` | Run the SpecForge web server: follow OpenSpec changes from proposal to pull request, in a browser. |
| Site `<title>` | SpecForge — Spec-driven work, from proposal to pull request |
| Site meta description | Follow each OpenSpec change from proposal to pull request: artifacts, worktrees, commits, diffs and reviews, in one local, read-only interface. |
| Site kicker | A read-only companion for spec-driven development |
| Site H1 | Spec-driven work, from proposal to pull request. |
| Pillars H2 | Follow the thinking all the way to the pull request. |

**README intro (`README.md:19`):**

> SpecForge is a read-only companion for **spec-driven development**. It follows every change in flight across your repositories and their worktrees: the proposal, design, specs and tasks, the commits and diffs behind them, and, once you opt in, the GitHub or BitBucket pull request each change became, with your review progress kept per hunk. It runs as a desktop app with a count in your menu bar (macOS), system tray (Windows) or status area (Linux), as a terminal UI, or as a local web server any browser can open. The spec format it reads today is [OpenSpec](https://github.com/Fission-AI/OpenSpec); this repository is itself an OpenSpec workspace ([`openspec/`](openspec/)) if you want a live example.

**README Why, second paragraph (`README.md:33`):**

> SpecForge keeps that state in one place, from the count in your menu bar to the pull request a change became, so you can check any registered workspace without bouncing through an IDE. It is **read-only** by design: it observes and renders, but never edits specs, toggles checkboxes, touches git, or posts to a pull request.

**README Features, new bullets.** These go after *Live commit graph*. *One badge for every project*, *Desktop notifications* and *macOS Dock badge* move below *Always live*.

> - **Dashboard.** The home screen rolls every registered workspace into one overview: summary metrics, today's ships, and a commit garden of each repository's commits for the day.
> - **Pull requests (opt-in).** GitHub and BitBucket panels list your open pull requests, each linked to the local worktree it comes from. Open one to read its description, conversation, checks and a syntax-highlighted diff inside SpecForge. Mark files and hunks as viewed, and after a new push SpecForge shows what changed since. Read-only toward both hosts, so it never comments, approves or merges. Off by default; each panel needs a token.
> - **Image diffs.** A changed PNG, JPEG, GIF, WebP, ICO, BMP or AVIF file renders as a side-by-side or stacked comparison, with its dimensions and size, instead of "Binary file not shown".

**README network sentences.** At `README.md:47`, "This is SpecForge's only network call — with the toggle off, nothing is read or sent." becomes:

> Like SpecForge's other network features (the ChatGPT gauge and the two pull request panels), it is off by default, and with its toggle off nothing is read or sent.

At `README.md:73`, "toggles the two quota gauges (Claude, ChatGPT)" becomes "toggles the two quota gauges (Claude, ChatGPT) and the two pull request panels (BitBucket, GitHub)".

**`bundle.longDescription`:**

> SpecForge follows every OpenSpec change across your registered repositories and worktrees, from its proposal, design, specs and tasks to the commits, diffs and pull request behind it, with the active-change count in your menu bar or system tray. It is read-only: it never edits a spec, touches git or posts to a pull request.

**`openspec/config.yaml`, first sentence of `context`:**

> SpecForge is a read-only companion for spec-driven work: it follows OpenSpec changes across repositories and worktrees, from proposal to pull request, through a Tauri 2 desktop app, a terminal UI and a local web server. This repo dogfoods OpenSpec (active changes in openspec/changes/<id>/, archive in openspec/changes/archive/, capability specs in openspec/specs/<capability>/spec.md).

**Site hero summary (`+Page.tsx`, after the H1):**

> SpecForge supports OpenSpec today, placing each change beside the repository-wide Git graph and the pull request it became. Local, read-only, free and MIT licensed.

**Pillars intro paragraph:**

> SpecForge keeps the artifacts, task state and repository evidence together, so a review starts with context and can move directly to what is in Git and on the pull request.

**01 / Navigate** (the H3 "Find the work in context." stays):

> Start from the Dashboard's overview of every registered workspace, then move across active and archived changes without walking folder trees. Progress, modified times and Git-state badges show where attention belongs.

**03 / Verify** (the H3 "Connect intent to evidence." stays):

> Keep the repository-wide `git log --all` graph beside the change and open any commit's files and diff. When the branch reaches GitHub or BitBucket, its pull request opens in the same window, linked to its worktree, and SpecForge keeps track of the files and hunks you have viewed.

**"Never changes your work" card**, a sentence appended after "…merge, rebase or reset anything.":

> Pull requests are read only once you add a token, and are never commented on, approved or merged.

**`/docs` intro (`site/pages/docs/+Page.tsx:8`):**

> SpecForge follows every change in flight across the workspaces you register, from proposal to pull request, in a desktop app, a terminal or a browser. Setup is four steps.

## Risks / Trade-offs

- **"From proposal to pull request" reads as a promise to someone who never adds a token.** → Every prose surface that names pull requests also says they are opt-in or need a token: the README intro ("once you opt in"), the README's pull requests bullet, and the site's read-only card. The short identifiers (H1, title, `bundle.shortDescription`) cannot carry the caveat, so it sits in the prose next to them.
- **The terminal UI has no pull request view.** → The statement describes SpecForge as a whole. The README intro names the terminal UI without claiming a pull request view there, and the Verify pillar describes "the same window", which only the desktop and browser UIs have.
- **The copy drifts again as features ship.** → The new `product-identity` requirement lists every introducing surface, so a future positioning change has a checklist, and the specs pin identifiers rather than feature lists (D7).
- **Search engines and social cards keep the old title and description for a while.** → Accepted. Bumping `modified` in `site/pages/index/+documentProps.ts` dates the change for the site's own metadata.
- **Changing the agent context changes how future artifacts are written.** → That is the intent, and the change is one sentence. The architecture, naming and verification guidance in the same block is untouched.
- **The screenshots still show v0.2.0 under copy that now promises pull requests.** → A follow-up. Neither the README nor the site names a version, so a new capture needs no copy change.
