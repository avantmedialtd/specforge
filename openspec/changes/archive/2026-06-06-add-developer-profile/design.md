## Context

The Dashboard's gamified layer (added in `energize-dashboard`) is built on an append-only **activity log** in the app data directory. Achievements are detected two ways: **live**, by diffing the watcher's re-parse of a workspace (`diff_achievements` in `activity_log.rs`, plus the archival branch in `watcher.rs`), and **backfilled**, by mining git history over a bounded window (`build_backfill`, fed by `change_lifecycle` and `task_completion_history` in `git.rs`). Every achievement carries a workspace, an optional change id, a timestamp, a magnitude, and a `backfilled` flag — but **no author**. The dashboard aggregates the log across all workspaces with no notion of *who*.

This change adds the missing identity dimension. The user chose the **full gamified profile**: identity + attribution foundation, plus a profile surface, an identicon avatar, personal milestones, and a per-author leaderboard for shared repositories — with **both** the personal and the team views reachable, and live events attributed to the **local git identity**.

## Goals / Non-Goals

### Goals

- Resolve the local developer identity from `git config` and let the user fold multiple name/email identities onto one canonical "me" via aliases.
- Attribute every achievement to an author: live events to the watched repo's local identity, historical events to their real commit author.
- Make the existing gamification *personal* (profile, your milestones, your streak) while keeping the team view and adding a shared-repo leaderboard.
- Stay additive and migration-free over the append-only log, and keep SpecForge read-only and offline.

### Non-Goals

- No network avatars (Gravatar), no account, no cross-machine sync.
- No `.mailmap` / `Co-Authored-By` mining in this change — the alias model is the user-facing substitute.
- No XP/leveling, no sound.
- No per-task author precision beyond "the local identity at observation time."

## Decisions

### Store the raw observed author on the event; resolve "me" at query time

The log is append-only and aliases change over time. If each event baked in a resolved "me/other" bit, adding an alias later could never reclaim past events. So each `Achievement` stores the **raw observed author** it was witnessed with, and the *is-this-me* decision is a pure function of the **current** identity config, evaluated when the dashboard queries the log. Adding an alias retroactively reclaims every matching past event with no rewrite. This is also exactly what lets one log render both *Me* and *Everyone* and a multi-author leaderboard.

`Achievement` gains `author: Option<Author>` where `Author { name: String, email: Option<String> }`, serialised camelCase. The field is `#[serde(default)]` so existing logs (which have no author) still parse. A legacy author-less event is treated as the **local user** when resolving *Me*: before identity existed the app was single-user, so all prior activity was implicitly the one developer's — treating it as "me" preserves their streak and milestones rather than zeroing them.

### Attribution sources: local git identity (live) vs commit author (history)

- **Live (watcher):** when the re-parse diff or the archival branch records an achievement, it stamps the author with the watched repo's local identity — `git config user.name` / `user.email`, repo-local with the usual global fallback, read once per batch via a new `git.rs::git_identity(path)`. A flat (non-git) workspace with no resolvable git identity records an author-less event (treated as "me", as above), since a flat personal workspace has exactly one user.
- **Backfill (history):** `change_lifecycle`, `task_completion_history`, and the commit mining that feeds the leaderboard stop discarding `%an`/`%ae` and carry the author into each backfilled achievement. This is where teammate attribution actually comes from.

The known edge — a teammate's change arriving via `git pull` while the watcher is live, mis-attributed to the local identity — is accepted: it is bounded to *live* events, and the next backfill/reconcile pass already records the correctly-authored historical event (dedup by `(kind, change_id)` keeps them from double-counting).

### Identity model: a canonical "me" plus alias identities

The identity config (persisted in app data next to settings) holds:

- a **display name** (the canonical label shown on the profile), and
- a set of **alias identities** — `(name?, email?)` entries that all resolve to "me".

The **normalised author key** is the lowercased, trimmed email when present, else the lowercased, trimmed name. Resolution `is_me(author, config)` is: normalise the author's key and test membership against the normalised keys of the canonical identity and every alias. Email is the strong signal; a name-only author matches only a name-only alias. This is a deliberately small, `.mailmap`-flavoured model — enough to merge the realistic spread (work/personal/noreply emails, "istvan" vs "István Antal", the Claude co-author bot if the user chooses to claim or exclude it) without becoming a rules engine.

On first run the config is seeded by *detecting* candidate identities: the distinct `git config` identities across registered git-backed workspaces. The Settings → Identity section shows the detected identities, lets the user pick the canonical display name, and add/remove aliases. Detection only *suggests*; nothing is auto-claimed beyond the single obvious local identity.

### Avatar: a locally-generated identicon, not a network fetch

The app makes no network calls and is privacy-conscious. The avatar is a deterministic **identicon** derived from the normalised identity key (a hash → a small symmetric pixel grid, tinted from the existing token palette), generated on the frontend. Gravatar is rejected here because it would leak an email hash to a third party and require network access the app otherwise never uses; an opt-in Gravatar mode is left to a future change.

### Dashboard scope: Me and Everyone, both reachable

`compute_progress` and the streak/heatmap/milestone aggregation take a `scope` (Me | Everyone). The frontend exposes a scope control that defaults to **Me**; flipping to **Everyone** recomputes against the unfiltered log. Both are always reachable — the team view is never removed, honouring "show both." Milestones and the streak are computed over the scoped event set, so *your* streak reflects *your* activity. `get_dashboard` gains the scope argument; the heavy git mining is unchanged and shared across scopes (only the in-memory log filter differs), so switching scope is cheap.

### Leaderboard only where there is a contest

The per-author leaderboard aggregates, per registered repository (or across all, see below), each author's shipped changes, tasks completed, and commits over the window, ranked. It renders **only when a repository's history holds more than one distinct author** — a leaderboard of one is noise, so a solo personal repo shows none. Because live events only ever carry the local identity, the leaderboard's non-local rows are driven by commit authorship from backfill; the local user's row also includes their live activity. The leaderboard is read-only and local.

### Identity lives in `openspec-core`, persistence in the shell

The alias model, normalised key, and `is_me` resolver are pure and testable, so they live in a new `openspec-core/src/identity.rs` (no Tauri dependency), exercised by `cargo test`. The *persistence* of the user's chosen config rides alongside `AppSettings` in the Tauri shell (`settings.rs`), matching how the activity-log path and settings are already injected from the shell. New commands `get_identity`, `set_display_name`, `set_identity_aliases` mirror the existing settings commands.

## Risks / Trade-offs

- **Live mis-attribution on pull.** Covered above; bounded to live events, corrected by historical backfill.
- **Alias model is intentionally thin.** No `.mailmap`/`Co-Authored-By` mining means a developer who commits under many unmanaged emails must add them as aliases by hand. Acceptable for v1; the detection step surfaces the candidates to make this a few clicks.
- **Leaderboard taste.** Competitive framing can sour a small team. It is opt-in by data (only shows for multi-author repos), read-only, and never leaves the machine; framing stays light (counts, not rankresults-with-losers language).
- **Legacy events as "me".** Treating author-less historical log entries as the local user is a heuristic; in the rare case an existing log already mixed authors (it couldn't have — pre-identity the app never recorded one), it would over-credit. In practice every pre-change event was the single user's, so this is safe and preserves their existing streak/milestones.
- **Backward compatibility.** The new `author` field is optional with a serde default; old logs load unchanged and are enriched going forward (live) and on the next backfill (history).

## Migration / Rollout

Additive and migration-free. On first run after the change, the identity config is seeded from detected git identities and the user confirms their display name/aliases in Settings → Identity (a sensible default — the single detected local identity — works with zero interaction). The activity log keeps its existing entries (author-less → treated as "me"); new live events carry the local identity and the next bounded backfill enriches history with commit authorship. The Dashboard defaults to the *Me* scope, so the immediate visible effect for a solo user is none, while a shared-repo user stops seeing teammates inflate their personal numbers and gains the Everyone view and leaderboard. No workspace files are touched; no network calls are made.
