# Add a gamified developer profile with identity attribution

## Why

The Dashboard already *gamifies* — Today's Progress, a streak, a contribution heatmap, milestones, confetti on a ship. But it gamifies an **anonymous aggregate**: every achievement is stamped with a workspace and a change id and *no author*. The board answers "what happened across my workspaces?" — never "what did **I** accomplish?" In a single-checkout personal repo that distinction is invisible; the moment a workspace is shared, the git backfill folds *teammates'* commits, ships, and task completions straight into "your" numbers, silently inflating the streak and milestones with work you didn't do.

The fix the user identified is exactly right: **give the game a player.** Resolve who the local developer is from `git config user.name` / `user.email`, let them fold their inevitable spread of identities (work email, personal email, `noreply` addresses, name variants) onto one canonical "me" via aliases, and attribute every achievement to an author. With that in place the existing gamification becomes *personal* (a profile, your own milestones, your streak) while still being able to show the team — and in a shared repo, a per-author leaderboard turns the aggregate into friendly competition.

Two facts about the existing substrate shape the whole design:

- **Only historical events have a real author.** Backfilled achievements come from git history — every commit already carries an author (`%an`/`%ae`) that the current backfill *discards*. Live achievements come from the **watcher**, which only sees files change on disk; there is no commit and therefore no author. The only identity we can pin on a live event is the watched repo's local `git config` identity — a heuristic that is correct on a personal machine and the agreed model here.
- **The activity log is append-only.** Aliases will change over time (a developer adds a new email months later). If "me vs other" were baked into each event at write time, a newly-added alias could never reclaim past events. So each achievement must store the **raw observed author**, and the *me / everyone* resolution must happen at **query time** against the current alias config. This is what lets the same log render both a personal view and a team view with no rewrite.

## What Changes

- **A new `developer-identity` capability.** Resolve the local developer identity from `git config user.name` / `user.email` across registered git-backed workspaces; persist an identity config in the app data directory holding a canonical display name and a set of **alias identities** (name/email pairs) that all resolve to "me". A normalised author key (lowercased email, falling back to lowercased name) is the unit of attribution. Resolution is a pure function — *is this observed author me?* — evaluated against the current config.
- **Achievements gain an author (`activity-log`).** Each recorded achievement stores the raw observed author identity. Live (watcher) events are stamped with the watched repo's local git identity; backfilled events are stamped with their real commit author — the backfill stops discarding `%an`/`%ae`. The field is optional and defaults to absent, so existing append-only logs keep parsing; legacy author-less events are treated as the local user (pre-identity, all activity was implicitly one person's).
- **The Dashboard gains a "me / everyone" scope, shown together (`dashboard`).** Today's Progress, the streak, the heatmap, and milestones resolve per author at query time. A scope control switches between *Me* and *Everyone*; both are reachable (the team view is never hidden), defaulting to *Me*. Teammates' backfilled history no longer inflates your personal counts, but the everyone view still shows the whole board.
- **A developer profile surface (`dashboard`).** A profile band identifies the developer — canonical display name and a locally-generated identicon avatar (deterministic from the identity; no network) — and presents *your* milestones and streak as a personal highlight reel.
- **A per-author leaderboard for shared repositories (`dashboard`).** When a registered repository's history holds more than one author, a leaderboard ranks authors by shipped changes, tasks completed, and commits over the window — driven by commit authorship (and the local user's live activity). It is hidden for solo repositories where it would be a leaderboard of one.

## Capabilities

### Added Capabilities

- `developer-identity`: resolving the local developer identity from git config, an alias model that folds multiple name/email identities onto one canonical "me", a normalised author key for attribution, query-time *is-this-me* resolution, and app-data persistence that never writes into a workspace.

### Modified Capabilities

- `activity-log`: the *Activity Event Log* and *Achievement Detection from Watcher Re-Parses* requirements gain an author dimension — each event records the raw observed author; live events are attributed to the watched repo's local git identity. The *Git Backfill of Historical Achievements* requirement is extended so backfilled events carry their real commit author rather than discarding it. Author storage is additive and backward-compatible with existing logs.
- `dashboard`: the *Today's Progress Hero*, *Streak and Contribution Heatmap*, and *Milestones and Badges* requirements are scoped by author and gain a *Me / Everyone* control that keeps both views reachable. New requirements add the *Developer Profile* surface (display name + identicon avatar) and a *Per-Author Leaderboard* for shared repositories. The read-only invariant is unchanged.

## Impact

- **Specs:** one new capability (`developer-identity`); two modified (`activity-log`, `dashboard`).
- **Code:**
  - `crates/openspec-core/src/git.rs` — new `git_identity(path)` reading `user.name` / `user.email`; backfill (`change_lifecycle`, `task_completion_history`, and the commit mining feeding the leaderboard) threads author through instead of discarding it.
  - `crates/openspec-core/src/identity.rs` *(new)* — identity config, alias model, normalised author key, and the pure *is-me* resolver.
  - `crates/openspec-core/src/activity_log.rs` — `Achievement` gains an optional `author`; `diff_achievements` and `build_backfill` accept and stamp it; query helpers gain author-scoped variants.
  - `crates/openspec-core/src/watcher.rs` — live achievement recording stamps the watched repo's local identity.
  - `crates/openspec-core/src/dashboard.rs` — `compute_progress` and the milestone/streak/heatmap aggregation take a scope (me/everyone); new profile and leaderboard aggregation.
  - `crates/specforge/src/settings.rs` — identity config (display name + aliases) persisted alongside `AppSettings`.
  - `crates/specforge/src/commands.rs` — `get_identity`, `set_display_name`, `set_identity_aliases`; `get_dashboard` takes the scope.
  - `crates/openspec-core/src/lib.rs`, `crates/specforge/src/lib.rs` — wiring and re-exports.
  - Frontend: `src/types.ts` (author, identity, leaderboard, profile, scope mirrors), `src/api.ts`, `src/hooks/useDashboard.ts`, `src/components/DashboardView.tsx`, a settings *Identity* section, `src/App.css`.
- **Behaviour delta:** the personal views stop counting teammates' work; the everyone view and the new leaderboard surface the team. Solo personal repos look identical to today (you are the only author), minus any cross-author inflation that never applied. No workspace files are written; no network calls are made.
- **Risk:** moderate but contained. The append-only log is extended additively (optional field, query-time resolution), so no migration and no rewrite. The leaderboard and everyone view are the only places teammate identities surface, and they are read-only and local. The main correctness edge — a teammate's change syncing in via `git pull` while the watcher is live, mis-attributed to the local identity — is the agreed-upon trade-off for live attribution and is bounded to live events (history stays correctly authored).

## Out of Scope

- **Network avatars / Gravatar.** Avatars are generated locally as identicons from the identity; the app makes no network calls and does not leak an email hash to a third party. A future change could offer opt-in Gravatar.
- **Multi-machine identity sync.** The identity config is local to this install's app data; no account, no cloud sync.
- **XP / levels / sound.** This change keeps the existing gentle gamification framing; it does not add an XP or leveling system or audio.
- **Rewriting historical attribution beyond git authorship.** Author comes from `git config` (live) and commit authorship (history). `Co-Authored-By` trailers and `.mailmap` resolution are not mined in this change (the alias model is the user-facing substitute); they are a possible later enhancement.
- **Per-task author precision.** Live task completions are attributed to the local identity at observation time, not to whoever physically edited the line — consistent with the existing "no per-task timestamps" stance.
