## Context

The `developer-identity` capability already models one canonical developer ("you"): `IdentityConfig { display_name, aliases: Vec<Author> }`, where `is_me` resolves an observed author against every alias at **query time** — so adding an alias retroactively reclaims past activity without rewriting the append-only log. This is elegant, but hardwired to a single person. Everyone else is unnamed.

Two consequences motivate this change:

1. **The leaderboard.** `dashboard::compute_leaderboard`'s `resolve()` collapses your identities into one `is_me` row labelled `config.label()`, but a non-me author becomes `(normalized_key(a), a.display(), false)` — its raw git name/email/`"Unknown"`. Each row renders `display` beside an `Identicon` keyed by `author_key`, and the list is shown only for **more than one distinct author**. A teammate who commits under two identities is therefore two rows, split stats, two avatars.
2. **The add button.** Settings → Identity's only add path is the "Detected git identities" group, gated behind `suggestions.length > 0` where `suggestions = candidates − existing aliases` and `candidates` come from `detect_candidate_identities` (git config across workspaces). Once your sole detected identity is claimed the group vanishes, leaving **no way to add an alias by hand**. The backend (`set_identity_aliases`) already accepts an arbitrary list — the gap is purely a missing input control.

A key safety fact, verified: `season_score` is computed from **Me-scoped events and Me-authored commit count** (`seasons.rs`), and `SeasonStanding` reads neither the leaderboard nor any rank. So naming/merging *other* people cannot move your tier, objectives, or treatments — the leaderboard is purely presentational.

## Goals / Non-Goals

**Goals:**
- Let any observed git author be given a custom display name on the leaderboard.
- Let several git identities be folded into one person so their leaderboard row is summed, not split — the `is_me` folding, generalized to everyone.
- Let identities be entered **by hand** (name and/or email), fixing the missing add button for yourself and enabling attaching identities to other people.
- Preserve the existing guarantees: query-time resolution, retroactive relabelling, no log rewrite, on-device only.

**Non-Goals:**
- Hiding/excluding authors (e.g. bots) from the leaderboard. The roster can *group* bot identities into one row, but a true hide switch is a separate change.
- Applying aliases anywhere but the leaderboard. The commit graph rail and commit-detail view — whose authors are name-only `%an` strings — are untouched, sidestepping the name-vs-email keying mismatch.
- Any change to season scoring, naming, objectives, or treatment determinism.
- Cross-machine sync of the roster.

## Decisions

### Decision 1: Additive roster, not a unified refactor

Keep `IdentityConfig` exactly as the distinguished *you* entry; add a sibling roster `people: Vec<Person>` (each `Person { display_name, identities: Vec<Author> }`) for everyone else. `compute_leaderboard`'s `resolve()` keeps its `is_me` branch unchanged and adds a roster lookup only in the non-me branch.

- **Why:** `IdentityConfig` carries a lot of special semantics — the profile avatar, `primary_key`, `label`, season standing. A unified `Roster { people, me_index }` would have to preserve all of it while also forcing a settings migration. The additive shape achieves the identical user-visible result (merged, named rows) with a far smaller blast radius, and old settings deserialize with an empty roster via serde default — **no migration**.
- **Alternative rejected — unified roster** (one `Vec<Person>` with a distinguished "me"): conceptually tidier, but destabilizes the avatar/profile/season code and needs a migration, for no user-visible gain.

### Decision 2: Single-assignment invariant with you-precedence

Any one git identity (by normalized key) belongs to **at most one** person across the entire roster, *you included*. Resolution checks `is_me` first, so an identity you claim always resolves to you; the editing layer enforces the invariant by removing an identity from any roster person when it is added elsewhere (or to you).

- **Why:** Without it, the same key could sit in two people and resolution would be order-dependent and contradictory. "You win" matches the existing precedence (`resolve` tests `is_me` before anything else) and the intuition that claiming an identity as your own is authoritative.
- **Alternative rejected — allow duplicates, resolve by first match:** silent, surprising double-membership; rejected.

### Decision 3: Seed other people from observed leaderboard authors

The candidate pool for naming/merging is the set of **observed non-me authors** on the leaderboard (from `commit_activity_with_authors` / authored achievements), surfaced to Settings — *not* `detect_candidate_identities` (which only reads local git config and so can only ever find *your* identities).

- **Why:** Nothing can auto-infer that `jane@corp` and `jdoe@corp` are one Jane; the only place those identities appear is the commit history the leaderboard already mines. Offering that list is what makes grouping possible.
- **Alternative rejected — manual entry only:** workable but tedious; you'd retype emails you can see on the board. Manual entry still exists (Decision 5) as the escape hatch for identities not yet observed.

### Decision 4: A person's canonical key is its primary (first) identity's key

A merged person's leaderboard `author_key` (and thus identicon) is the normalized key of its **first** identity, mirroring how "you" already uses `IdentityConfig::primary_key`.

- **Why:** Reuses an established convention; needs no new identifier concept.
- **Trade-off:** Removing the primary identity shifts the person's key, so the identicon changes. Accepted for simplicity (see Risks). A synthetic stable per-person id was considered and rejected as over-engineering for a presentational row.

### Decision 5: Free-form manual identity entry

Settings → Identity gains a name+email form that constructs an `Author` and appends it — to *you* or to a selected roster person. At least one identity is kept for "you" (Remove stays disabled at one); the empty-identity case (both fields blank → no `normalized_key`) is rejected by the form.

- **Why:** This is the missing-add-button fix. It is also the only way to record an identity you don't (yet) commit under locally — a personal email, a GitHub `noreply` address, a retired account.

### Decision 6: Query-time, presentational application to both leaderboards

`resolve()` consults a `key → (canonical_person_key, display_name)` map built once from the roster; merged identities upsert into one entry that sums ships/tasks/commits. Applied to **both** `leaderboard` and `season_leaderboard`. No stored event is rewritten; the transform is recomputed each render against the current roster.

- **Why:** Preserves retroactivity and the no-log-rewrite guarantee for free, exactly as `is_me` already works.

## Risks / Trade-offs

- **Merging can make the leaderboard vanish** → When merging the only *other* author into yourself drops the distinct-author count to one, the list is omitted (the ">1 author" gate). This is *correct* — it was never a real contest — but surprising. Mitigation: it is intended behavior; document it in the spec scenario so it reads as designed, not broken.
- **Identicon shifts when a primary identity is removed** (Decision 4) → low-impact (avatars are decorative identicons, not photos). Mitigation: keep primary = first; a future synthetic id can stabilize it if it ever matters.
- **Name-only vs email keys** → would bite if aliases applied to the graph/detail (`%an` only). Mitigation: scope is leaderboard-only, where authors are email-bearing (`%an`+`%ae`) and key uniformly on email.
- **Bots still rank** → grouping into one "Bots" row declutters but doesn't remove them. Mitigation: explicit non-goal; a hide switch is a clean follow-up.

## Migration Plan

None required. `people` is an additive, serde-defaulted field on persisted settings — existing identity configs load with an empty roster and behave exactly as today. Rollback is to ignore the field; the leaderboard simply falls back to raw labels.

## Open Questions

- **Editing affordance for grouping:** multi-select observed authors → "Combine into a person", versus a person card with "+ add identity" that opens the observed-author picker. The merge UX is the part most likely to need iteration; the spec fixes the *behavior* (named, merged rows) and leaves the exact control to implementation.
- **Display of a person with zero observed activity in the window:** a roster person whose identities had no activity in the bounded window simply produces no row — confirm that's acceptable (it is, for a leaderboard).
