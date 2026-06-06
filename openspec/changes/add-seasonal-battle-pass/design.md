## Context

SpecForge's gamified layer (from `energize-dashboard`, extended by `add-developer-profile`) rests on an append-only **activity log** in the app data directory: authored, timestamped achievement events (`TaskCompleted`, `ArtifactReached`, `ChangeCreated`, `ChangeArchived`) plus git-mined **commit activity** for the dashboard's chart and leaderboard. The dashboard aggregates this log over a bounded window, day-bucketed in the viewer's local time zone, resolving *Me vs Everyone* at query time via the `developer-identity` capability. Milestones are crossed at cumulative thresholds; the avatar is a **deterministic, local, network-free identicon** generated in `identity.rs`.

This change adds a **monthly season** layer on top of that substrate. The design's load-bearing constraint is that progress here is *real shipped work*: a season therefore resets **standings** (a derived score, a tier ladder, a fresh objective set) and never **facts** (lifetime totals, the streak, earned milestones). Two properties of the substrate make the full battle-pass tractable: season standings are a **pure projection** of the existing authored log + commit mining (no new event kinds, no migration), and the identicon already proves the **deterministic-local-procedural** pattern that the battle-pass rewards reuse.

The user added one process constraint: badge-treatment **artwork is generated at development time by driving the native ChatGPT desktop app** (not Chrome automation, not an API/SDK), and baked into the bundle as a build-time asset — runtime stays offline and deterministic.

## Goals / Non-Goals

**Goals:**

- A monthly season with a resetting **season score → band/tier ladder**, derived from the existing Me-scoped log + commit mining, with **no new recorded event kinds**.
- A second, permanent **career tier** that only ever rises; the **streak survives** season boundaries.
- **Adaptive + overflow** pacing so the ladder is a stretch for any cadence and a prolific month is never wasted.
- **Rotating, generated objectives** (archetype templates × adaptive thresholds) so content never has to be hand-authored.
- **Procedural badge treatments** keyed by `(season, tier)`, applied to earned milestone badges; pure descriptor logic in the core, artwork + rendering in the desktop app; **soft-FOMO** so missed treatments can return via a vault and earned ones are never lost.
- **Silent** backfilled past seasons; an auto-minted **recap** at rollover.
- Preserve the **read-only**, **offline**, append-only invariants.

**Non-Goals:**

- No runtime network calls, no fetched/paid cosmetics, no cross-machine or multiplayer sync (bounded to a repo's own git authorship, as today).
- No new cosmetic categories beyond badge treatments (frames/palettes/titles deferred).
- No configurable cadence (monthly only); no hard/permanently-missable FOMO; no audio.
- No rewrite or re-attribution of the activity log; seasons are read-only derivations.

## Decisions

### Seasons are a derived projection; the only new persisted state is the locker

Season score, band/tier, objective progress, career tier, and recaps are all **recomputed** from the activity log + commit mining, exactly like today's dashboard views. This inherits append-only correctness, git backfill, Me/Everyone resolution, and watcher reactivity for free, and needs no migration. The **only** genuinely new persisted state is the **treatment locker** (which treatments are unlocked) and the **equipped** selection, written to the app data directory beside `AppSettings` with serde defaults (absent = empty). *Alternative considered:* materialising season standings into the log — rejected because it duplicates derivable state and risks drift against the append-only source of truth.

### A stable integer season index drives all determinism

`season_index = year*12 + (month - 1)` — a monotonic absolute month number. The season **window** is `[first instant of month, first instant of next month)` in the viewer's local time zone, reusing the existing local-day bucketing. Every deterministic artifact (season **name**, **objective** selection, **treatment** seed) is a pure function of `season_index`, so the same season reproduces identically across launches and is reconstructable for backfilled history. The season **name** is an adjective+noun chosen deterministically from `season_index` (the identicon's deterministic-from-key trick), giving flavor with zero per-season authoring.

### Season score sums the Me-scoped log and Me-authored commits

Commits are **not** in the activity-log enum — they are mined from git for the chart/leaderboard. So season score composes **two** existing sources over the window: (a) Me-resolved log events and (b) Me-authored commits from the same mining the leaderboard already uses. Starting calibration (named constants in `seasons.rs`, tunable):

| Contribution | Weight | Rationale |
|---|---|---|
| `ChangeArchived` (ship) | 50 | the headline act |
| objective completed | bonus (scales) | the accelerant; pulls the player toward varied play |
| `ArtifactReached` | 15 | meaningful lifecycle progress |
| `TaskCompleted` | 8 × magnitude | the within-change increment |
| active day (streak credit) | 10 / day | rewards showing up |
| commit (Me-authored) | 2 | fine-grained motion |
| `ChangeCreated` | 5 | low, to deter gaming via empty changes |

Score is derived and append-only-monotonic within a season (the log only grows), so a recompute can only *raise* a season's score, never lower a crossed tier.

### Battle-pass ladder: fixed tier count, named bands, adaptive total, overflow lane

One number reads two ways — fine **tiers** (e.g. 30/season) grouped into named **bands** (`Bronze → Silver → Gold → Platinum → Diamond → Master`, distinct from career-tier labels to avoid confusion), so the UI shows both `tier 14 → 15` and `Gold II`. **Pacing** scales the *total* score needed to complete the pass to the developer's **trailing baseline** (the recent-daily-average the Today's-Progress comparison already computes) × days in the month, then distributes it across the fixed tier count. The baseline's influence is **clamped** between a floor and ceiling so a brand-new user isn't handed a trivial pass and a hyper-prolific one isn't handed an impossible one. Past the final tier, an unbounded **overflow lane** keeps accruing prestige tiers (each still granting a treatment, flagged overflow). *Alternatives considered:* **pure-fixed** thresholds (simple, but flat and quickly exhausted for high-cadence work) and **pure-adaptive** (constant completion %, but reads as "running faster speeds up the treadmill"). The hybrid keeps the underdog stretch for every cadence without wasting effort.

### Objectives are generated: archetype × adaptive threshold × rotation

A small set of **archetypes**, each derivable from existing event kinds:

- **Volume** — ship N / complete N tasks / land N commits this season
- **Cadence** — active on D distinct days
- **Streak** — hold a K-day streak
- **Burst** — M tasks in one day, or 2 ships in one day
- **Breadth** — ship a change touching a capability spec (the dashboard already counts this)
- **Finish** — take a change from created → archived within the season
- **Comeback** — return after a gap of ≥ G days (underdog-flavored; rewards coming back)

Each season deterministically selects K objectives (e.g. 3) by `season_index`, **rotating** so the same archetype doesn't repeat back-to-back, with **thresholds scaled to the baseline** (beat recent-you, not a global bar). Progress is a pure query over the in-window log; completion grants bonus score. This is the engine that makes "hard to come up with new ones" a non-problem.

### Treatments: pure descriptor in the core, artwork in the app, dev-time via ChatGPT

`treatment(season_index, tier_index)` is a **pure descriptor** — `{ id, rarity = f(tier_index), palette indices into the token system, effect/pattern kind, generator_version }` — and lives in `openspec-core` (`seasons.rs`), fully testable from `cargo test`. The **artwork and rendering** live in the desktop app (frontend + `specforge`), consistent with the convention that the headless core carries logic, not assets. The visual building blocks (base textures/overlays the descriptor composes with token-palette tints and CSS/SVG effects) are **generated at development time by driving the native ChatGPT desktop app** and baked into the bundle; nothing is fetched at runtime, preserving the identicon's offline guarantee. `generator_version` is encoded into the treatment **id** so that if the generator or band math changes in a future release, previously-unlocked treatments still resolve to a stable rendering. A treatment is **applied to** an earned badge (a finish), not a one-skin-per-badge mapping — so the badge set deepens rather than runs dry. *Alternative considered:* shipping a finite curated asset set with no procedural composition (simpler, but caps the collection and reintroduces per-season authoring); rejected in favor of `(season,tier)`-seeded composition over a dev-time-generated library.

### Locker, unlocking, and soft-FOMO vault

The locker persists `{ unlocked: set of treatment ids, equipped: optional treatment id }`. On each recompute, for the **current** season, tiers whose threshold the season score has crossed unlock their treatments into the locker (a monotonic add — never revoked). Unlocking the current season's *live* tier triggers a tier-up acknowledgement; **backfilled** past seasons unlock their earned treatments **silently** (you did the work — mirrors "backfilled milestones shown as earned, no celebration"). **Soft-FOMO:** a treatment is *earnable now* if it belongs to the current season **or** to the current **vault** — a deterministic subset of past seasons' treatments rotated back in by `season_index`. Treatments you never reached are not lost forever; earned ones are kept permanently. For v1, **equipped** is a single global treatment applied across badges; the data model leaves room for a per-badge map later.

### Career tier is derived and monotonic; the streak crosses boundaries

The **career tier** is `max` tier implied by lifetime cumulative totals (already in the log) — a pure, monotonic function needing no new persistence, rendered **distinctly** from the seasonal band so the two tracks never read as one. The **current streak** is explicitly a career line and is **not** reset by a season boundary; only score/band/objectives/track reset.

### Rollover detection and the recap

On launch and when a month boundary is crossed while running (checked on watcher ticks), the active `season_index` is compared against a persisted `last_recapped_season_index`. The just-ended season gets a **recap** — a pure synthesis of its window (shipped, best streak, band reached, objectives completed, treatments unlocked) — surfaced once as a "Wrapped" moment; older or backfilled seasons are available in history **without** celebration. At first launch, `last_recapped_season_index` is initialised to the current season so historical months don't spam recaps. *This is the only place a season "fires" an effect; everything else is passive recomputation.*

### Surfacing: reuse the scope-control pattern; inherit celebration/reduced-motion/read-only

The **This-Season / All-Time** lens reuses the existing Me/Everyone scope-control UI pattern over the today/streak/heatmap/milestone views. The **seasonal leaderboard** is the existing per-author aggregation windowed to the season (omitted for solo history, as today). Tier-up uses the existing celebration infrastructure and is suppressed under `prefers-reduced-motion`. The locker writes only to app-data; **no workspace file or git state is mutated**.

## Risks / Trade-offs

- **Adaptive pacing could feel like it "punishes speed."** → Clamp the baseline's influence (floor/ceiling) and add the overflow lane so prolific months convert to prestige instead of a harder wall.
- **A productivity tool inducing guilt on a slow month (vacation, illness, non-OpenSpec work).** → Adaptive thresholds *lower* the bar after a quiet stretch; the Comeback objective rewards returning; soft-FOMO keeps earned treatments and rotates missed ones back via the vault; the career tier never demotes; recaps stay celebratory ("you showed up D days").
- **Live events mis-attributed to the local identity** (a teammate's change syncing in during the watch window). → Inherited, bounded trade-off from `activity-log`; historical events are correctly authored by backfill, so season standings self-correct from history.
- **Backfill spamming celebrations/recaps at first launch.** → Backfilled seasons unlock silently; `last_recapped_season_index` is seeded to the current season.
- **The locker (new persisted state) drifting from derived score.** → Unlock is monotonic-add only; recompute can never revoke; the vault, not revocation, handles re-earnability.
- **Generator/band math changing across app versions would re-render old unlocks.** → `generator_version` is baked into each treatment id so previously-earned treatments resolve to a stable rendering.
- **ChatGPT-desktop-app automation is more fragile than browser automation and needs Automation/Accessibility permission.** → Assets are dev-time only (no runtime impact); fall back to hand-running provided prompts in the app if scripting is blocked.

## Migration Plan

Purely additive. No schema change to `Achievement` and no migration of the append-only log — season standings are derivations. The one new persisted artifact is the locker file in app-data, which loads as empty when absent (serde defaults). First launch backfills past seasons silently and seeds `last_recapped_season_index`. **Rollback:** remove the season surfaces; the locker file becomes inert and the rest of the dashboard is unaffected, since nothing else depends on new persisted state.

## Open Questions

- **Score weights and tier count** are a starting calibration; tune against this repo's real cadence before locking.
- **Vault rotation policy** — how many past-season treatments rotate back, and how chosen by `season_index` — propose a deterministic subset; finalize in specs/impl.
- **Equip granularity** — v1 ships a single global equipped treatment; confirm whether per-badge equipping is wanted later.
- **Does `ChangeCreated` contribute to score at all?** Proposed low (5) to deter gaming; could be 0 if empty-change creation proves exploitable.
