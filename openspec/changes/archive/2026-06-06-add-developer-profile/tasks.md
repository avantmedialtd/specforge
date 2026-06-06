# Tasks

## 1. Identity foundation (`developer-identity`)

- [x] 1.1 Add `crates/openspec-core/src/identity.rs`: `Author { name, email }` (camelCase IPC type), an `IdentityConfig { display_name, aliases: Vec<Author> }`, a `normalized_key(author)` helper (lowercased/trimmed email, else lowercased/trimmed name), and a pure `is_me(author, config) -> bool` resolver (membership of the normalised key against the canonical identity + aliases; email is the strong signal, name-only matches name-only)
- [x] 1.2 Add `git.rs::git_identity(path) -> Option<Author>` reading `git config user.name` / `user.email` (repo-local with global fallback); `None` when git is missing or unset
- [x] 1.3 Add `detect_candidate_identities(workspaces) -> Vec<Author>`: the distinct git identities across registered git-backed workspaces, for seeding/Settings suggestions
- [x] 1.4 Re-export the identity surface from `lib.rs`; unit-test `normalized_key`, `is_me` (email match, name-only match, alias match, non-match), and detection

## 2. Identity persistence + commands (shell)

- [x] 2.1 Persist `IdentityConfig` alongside `AppSettings` in `crates/specforge/src/settings.rs` (display name + aliases), with serde defaults so an absent config loads as empty
- [x] 2.2 Seed the config on first run from `detect_candidate_identities` when no identity config exists (default: the single detected local identity becomes the canonical "me"); never auto-claim beyond the one obvious local identity
- [x] 2.3 Add commands `get_identity`, `set_display_name`, `set_identity_aliases` in `commands.rs`; wire state in `lib.rs`; add to `src/api.ts`

## 3. Author attribution on achievements (`activity-log`)

- [x] 3.1 Add `author: Option<Author>` to `Achievement` (`#[serde(default)]`, camelCase) so existing append-only logs still parse; legacy author-less events resolve as the local user
- [x] 3.2 Live stamping: `diff_achievements` and the archival branch in `watcher.rs` take the watched repo's local identity (via `git_identity`, read once per batch) and stamp it on every recorded event; flat workspaces with no git identity record author-less events
- [x] 3.3 Backfill stamping: thread `%an`/`%ae` through `change_lifecycle`, `task_completion_history`, and the leaderboard commit mining in `git.rs`, and through `build_backfill` / `missing_lifecycle_events` so backfilled achievements carry their real commit author (keep the `(kind, change_id)` dedup intact)
- [x] 3.4 Add author-scoped query helpers on `ActivityLog` (filter the window/totals by an `is_me` predicate); keep the existing unfiltered queries for the Everyone scope

## 4. Dashboard Me/Everyone scope (`dashboard`)

- [x] 4.1 `compute_progress` and the streak/heatmap/milestone aggregation in `dashboard.rs` take a `scope` (Me | Everyone); the Me path filters the log via `is_me` against the current identity config, the Everyone path uses the unfiltered log
- [x] 4.2 `get_dashboard` accepts the scope; thread it through the command and `useDashboard.ts`; add a scope control to `DashboardView.tsx` defaulting to **Me**, with both views always reachable
- [x] 4.3 Mirror the scope + author types in `src/types.ts`

## 5. Developer profile surface (`dashboard`)

- [x] 5.1 Profile band in `DashboardView.tsx`: canonical display name + a locally-generated identicon avatar (deterministic from the normalised identity key, tinted from the token palette; no network)
- [x] 5.2 Scope the streak and milestones to the resolved "me" so the profile reads as a personal highlight reel; keep the encouraging zero state
- [x] 5.3 Settings → Identity section: show detected candidate identities, let the user set the display name and add/remove aliases (calls the section-2 commands); copy uses "git identity"/"OpenSpec" terms correctly per `product-identity`

## 6. Per-author leaderboard for shared repos (`dashboard`)

- [x] 6.1 Aggregate per-author shipped/tasks/commits over the window from the authored achievements + commit authorship; expose as a dashboard payload field
- [x] 6.2 Render the leaderboard **only** when a repository's history holds more than one distinct author; hide it for solo repos; the local user's row includes their live activity
- [x] 6.3 Mirror the leaderboard type in `src/types.ts`; style in `App.css`

## 7. Tests + verify

- [x] 7.1 `openspec-core` unit tests: `is_me` resolution incl. alias reclaim of past events; live attribution stamps the local identity; backfill carries commit author; author-scoped queries; legacy author-less events resolve as "me"
- [x] 7.2 `dashboard.rs` tests: Me vs Everyone scope yields different scoped totals from one log; leaderboard appears only for multi-author history and ranks correctly
- [x] 7.3 `cargo test` (workspace) green; `bun run build` (tsc + bundle) green; `cargo fmt` + `clippy` clean
- [x] 7.4 `openspec validate add-developer-profile --strict` passes
- [x] 7.5 Visual check via `bun run wt:dev`: Me/Everyone toggle, profile band + identicon, and the leaderboard on a multi-author repo render correctly in light and dark
