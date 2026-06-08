# Add custom aliases for git authors

## Why

The Dashboard's per-author leaderboard labels every contributor other than you with their raw git identity — a name, an email, or literally `"Unknown"` — and a person who commits under several git identities (a work and a personal email, a GitHub `…@users.noreply.github.com` address, a renamed account) is split across multiple rows with their ships, tasks, and commits divided between them. There is no way to give a contributor a friendly name or to fold their scattered identities into one honest row.

The same gap bites you directly: Settings → Identity can only add identities the app *auto-detected* from a workspace's git config, and it hides the add control entirely when there is nothing fresh to suggest — so once your sole detected identity is claimed, there is no add button at all and you cannot record an extra alias for yourself by hand.

## What Changes

- Generalize the single canonical developer ("you") into a **roster of named people**: keep the existing identity configuration as the distinguished *you* entry, and add a sibling roster where each person carries a custom display name and a set of git identities that all fold onto them — the same identity-folding that already resolves "is this author me?", now available for everyone.
- Add a **free-form "add identity" form** (type a name and/or email) to Settings → Identity. This fixes the missing add button — you can record any number of self-aliases by hand, not only auto-detected ones — and it is how identities are attached to other people on the roster.
- The Dashboard's per-author leaderboard (both the all-time and this-season views) **resolves each observed author through the roster at query time**: folded identities collapse into one summed row labelled with the person's custom name. Because resolution runs against the *current* roster rather than baked-in stored events, naming or merging an author **retroactively** relabels and re-sums all past activity without rewriting the append-only activity log.
- A **single-assignment rule with you-precedence**: any one git identity belongs to at most one person across the whole roster, you included; because "is this me?" is evaluated first, an identity you claim resolves to you and is dropped from any roster person.
- Scope is the **leaderboard only**. The commit graph rail and commit-detail view (whose authors are name-only `%an` strings) are intentionally untouched. Hiding/excluding authors (e.g. bots) is **out of scope** — though the roster does let you fold several bot identities into one row as a side effect.

## Capabilities

### New Capabilities
<!-- None — this extends two existing capabilities. -->

### Modified Capabilities

- `developer-identity`: extend the identity configuration from a single canonical developer to a roster of named people, each with folded git identities; add free-form (manual) identity entry; and state the single-assignment / you-precedence resolution rule. The local-only, no-log-rewrite, query-time guarantees are preserved.
- `dashboard`: the per-author leaderboard (all-time and seasonal) applies the roster — merging folded identities into one summed, custom-named row — as a purely presentational, query-time transform that does not affect season score or any deterministic generation.

## Impact

- **Rust core (`openspec-core`)**: `identity.rs` gains the roster type and resolution; `dashboard.rs` `compute_leaderboard` consults the roster in its non-me branch. Both stay pure and Tauri-free.
- **Tauri shell (`specforge`)**: settings persistence carries the roster alongside the existing identity config (empty roster by default — old settings load unchanged, no migration); commands expose reading/writing the roster and the observed-author candidate list.
- **Frontend (`src/`)**: Settings → Identity grows a free-form add form and a "People" section for naming/merging observed contributors; the leaderboard renders the resolved names/rows. `src/types.ts` mirrors the new Rust types.
- **No new external dependencies.** Identity data stays on-device, consistent with the existing `developer-identity` privacy guarantee.
