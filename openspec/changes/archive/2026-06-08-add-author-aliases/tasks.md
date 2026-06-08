## 1. Core: roster model (`openspec-core/src/identity.rs`)

- [x] 1.1 Add a `Person` type — `{ display_name: Option<String>, identities: Vec<Author> }` with `#[serde(rename_all = "camelCase")]` — plus `primary_key()` (first identity's `normalized_key`) and `label()` (display_name, else primary identity's `display()`), mirroring `IdentityConfig`.
- [x] 1.2 Add a pure resolver that builds a `HashMap<normalized_key, (canonical_person_key, label)>` from `&[Person]`, so an observed author can be mapped to its canonical person key + display name in one lookup.
- [x] 1.3 Add a pure `assign_identity(people: &mut Vec<Person>, target, author)` helper that enforces single-assignment: it removes the identity's key from every other person before adding it to the target (you-precedence is handled at resolution time by checking `is_me` first).
- [x] 1.4 Unit tests: folded identities share one canonical key; `label()` precedence; `assign_identity` is exclusive (reassigning removes from the prior holder); an empty `Author` (no usable key) is ignored by the resolver.

## 2. Core: leaderboard application (`openspec-core/src/dashboard.rs`)

- [x] 2.1 Extend `compute_leaderboard` to accept the roster (`people: &[Person]`) and, in the **non-me** branch of `resolve()`, map the author through the roster resolver → `(canonical_person_key, person_label, false)`; unmatched authors keep `(normalized_key, a.display(), false)`. The `is_me` branch is unchanged, preserving you-precedence.
- [x] 2.2 Confirm `upsert` collapses merged identities into one row that sums ships/tasks/commits (it keys on the resolved key, so this falls out — add a test proving it).
- [x] 2.3 Unit tests: two identities folded onto one person produce a single summed row with the custom name; an unrostered author keeps its raw label; merging the only other author into "me" yields a single distinct author; roster resolution leaves `season_score`/standing inputs untouched.

## 3. Shell: persistence & commands (`specforge`)

- [x] 3.1 `settings.rs`: persist `people: Vec<Person>` on `AppSettings` with `#[serde(default)]` (empty roster for existing settings — no migration), plus a getter and setter.
- [x] 3.2 `commands.rs`: add a `set_people` command (writes the roster) and extend the identity read command to return `people` alongside the existing config; keep all writes in the app data dir, never in a workspace.
- [x] 3.3 `commands.rs`: add an `observed_authors` command that aggregates non-me leaderboard authors across registered workspaces (via `commit_activity_with_authors`), deduped by normalised key, as the candidate pool for the roster UI.
- [x] 3.4 Thread the persisted `people` into wherever the dashboard's `leaderboard` and `season_leaderboard` are computed, so both views apply the roster.

## 4. Frontend: types & API (`src/types.ts`, `src/api.ts`)

- [x] 4.1 `types.ts`: add a `Person` interface (camelCase mirror) and extend the identity info payload to carry `people` and the observed-author candidates.
- [x] 4.2 `api.ts`: wrap the new commands (`setPeople`, `observedAuthors`) in `invokeLogged`, and extend the identity fetch.

## 5. Frontend: Settings → Identity (`src/components/SettingsView.tsx`)

- [x] 5.1 Add a free-form **"add identity"** form (name + email inputs) to the "Your identities" group that builds an `Author` and appends it via the existing alias setter — fixing the missing add button; reject an entry with neither field.
- [x] 5.2 Add a **"People"** section: list roster people (rename, list/remove their identities), create/remove a person, and fold observed authors or manually-entered identities onto a person — calling `setPeople`, with single-assignment enforced (backend authoritative).
- [x] 5.3 Confirm the leaderboard render (`DashboardView.tsx`) needs no change — it already renders the backend-resolved `display` and identicon by `authorKey`; note this in the task if verified true.

## 6. Verification

- [x] 6.1 `cargo test -p openspec-core` — identity and dashboard suites green.
- [x] 6.2 `bun run build` — `tsc --noEmit` clean (TS mirror matches the Rust types) and the bundle builds.
- [x] 6.3 Booted the worktree app live (slot 1, port 1430): clean compile + boot validated the new command registration and the live `get_dashboard`/`compute_leaderboard(&people)` path, and the free-form **"+ Add identity"** form is now unconditionally rendered (fixing the missing add button). Interactive roster *mutation* was deliberately skipped — the main app instance is running on the shared app-config dir, so saving test people would corrupt real settings; the merge / you-precedence / raw-label behaviour is covered by unit tests instead.
