# Design — Harden Temporarily-Disabled Workspaces

## Context

The predecessor's design opens with "the aggregated snapshot is a chokepoint …
one well-placed cut reaches everything", and enumerates the consumers of
`last_views`: the tree in all three frontends, the tray badge, the notification
dispatcher, and `compute_dashboard`. That enumeration is where the defects come
from. It is complete for `last_views` and it is *not* complete for the tray,
because the tray's second surface — the glyph — never read `last_views` at all.

Three structural facts explain why the drift survived review and CI:

- **The desktop's copy of the filter is invisible to both gates.**
  `.cargo/mutants.toml` excludes `crates/specforge/**/*.rs` outright (Tauri's
  `generate_context!` cannot build in a mutants scratch tree), and `commands.rs`
  has no `#[cfg(test)]` module. A predicate implemented there is unreachable by
  `cargo test` and unscoreable by `cargo mutants` by construction — so a second
  copy of a filter reads as tested when only its twin is.
- **The cache cannot express the predicate.** `WorkspaceCache` is a
  `HashMap<PathBuf, Vec<ChangeData>>` keyed by workspace path. A top-level row is
  a `PresentationKey::{Flat(path), Repo(common_dir)}`. There is no honest way to
  filter rows in a structure that does not know what a row is — which is exactly
  why the glyph predicate had to move rather than be patched in place.
- **Cold aggregation weakened an identity nobody was watching.** `RepoView::name`
  is the main worktree's basename, the routing slug is `slugify(view.name)`, and
  the Dashboard is deliberately unfiltered — so a cold row's name reaches the
  user and the address bar, and reaches them *only* while the row is parked.

Everything below serves one rule: a parked row is the same row, seen less.

## Goals / Non-Goals

**Goals**

- One implementation, in `openspec-core` or `openspec-app`, of every predicate
  that decides whether a top-level row is parked.
- A parked row's identity — repository id, main worktree, resulting name, and
  main-worktree instance tagging — identical to the identity it carries while
  enabled.
- Every surface that can hide a row can also explain the hiding and undo it:
  desktop, web, and terminal.
- No click that silently does nothing, and no failed write that reports only to
  a console the user cannot see.
- Every changed Rust line in `openspec-core` / `openspec-app` covered by an
  assertion that fails when the line is mutated.

**Non-Goals**

- Revisiting any predecessor decision. Cold aggregation (D1), the presentation
  store as the home of the flag (D2), one shared filter (D3), a live watcher and
  activity log (D4), top-level-row granularity (D5), the name `disabled` (D6),
  and the unfiltered Dashboard as the ambient cue (D7) all stand.
- Putting `disabled` on the wire for `WorkspaceView`. It stays
  `#[serde(skip_serializing)]`; frontends learn about parked rows from
  `list_workspaces`, which has always carried the flag.
- Correcting `releases/v0.16.1.md`. See the proposal's Impact section.
- Per-worktree disabling, auto-expiry, or an in-tree indicator.

## Decisions

### 1. The glyph predicate moves to `last_views`, and the badge shares its exclusion

`WatcherManager::any_change_touches_specs` reads the aggregated snapshot and
filters with `WorkspaceView::is_disabled` — the same predicate
`total_active_logical_count` uses — through a single named helper the two call
sites share. `WorkspaceCache::any_change_touches_specs` is deleted.

Freshness is not lost: `last_views` is refreshed before any `CacheEvent` is
broadcast, so a view-backed glyph is exactly as fresh as the cache-backed one
was, and `AppService::set_workspace_disabled` already recomputes and emits an
`Updated` carrier, so toggling the flag wakes the glyph updater.

*Rejected: pass a disabled-key set into `WorkspaceCache`.* The cache is keyed by
workspace path and has no notion of a top-level row, so it would have to
re-derive the repo grouping the aggregator already did — reimplementing the
predicate inside the one type that cannot express it.

*Rejected: filter in `crates/specforge/src/tray.rs::current_variant`.* Puts logic
in the Tauri crate against CLAUDE.md, leaves it untestable from `cargo test`, and
leaves the unfiltered predicate on `WatcherManager` for the next caller.

*Rejected: leave `WorkspaceCache::any_change_touches_specs` in place as dead
API.* It is `pub` on a `pub` struct, so no dead-code warning fires; it would sit
there as a correct-looking unfiltered twin of the thing that just caused this.

*Considered and taken: refactoring `total_active_logical_count` too.* Leaving its
inline filter alone would be a smaller diff and one fewer mutation obligation,
but two copies of the row-exclusion loop is precisely the drift that produced
this defect.

### 2. Cold identity reproduces git's own rule, rather than remembering a warm one

`git::main_worktree_for_common_dir(common_dir)` returns the common dir with a
trailing `.git` component stripped, and the common dir itself otherwise — which
is `worktree.c: get_main_worktree`'s rule, verified against
`git worktree list --porcelain` for the plain, submodule, `--separate-git-dir`,
and bare layouts. `resolve_main_worktree`'s cold path calls it, so warm and cold
agree *by construction* rather than incidentally, including when the warm path
falls through because git failed. No cache, no persisted state, no config
parsing, no filesystem I/O, no subprocess — the *Cold Aggregation of Disabled
Rows* prohibition on git invocations is preserved.

The same helper replaces the private `main_worktree_path` in `git.rs`, fixing the
identical latent bug in `default_branch`'s third fallback step, which for a
submodule runs `git branch --show-current` against the **superproject**.

*Rejected: cache the warm-resolved main worktree per `RepoId`.* `presentation.json`
persists the flag across restarts, so a repository parked at launch has never
been resolved warm in this process and falls straight back to the broken
heuristic — a cold-start hole in the exact case the feature is for. The same hole
sinks reusing the previous snapshot's `main_worktree` from `last_views`.

*Rejected: read `core.worktree` from `<common>/config`.* A submodule's config
does carry it, but `git init --separate-git-dir=` writes none at all, so it
covers one of three broken layouts — and it means hand-parsing git config
(sections, includes, quoting) for an answer that would still not match the warm
value, because `git worktree list` reports the git dir for submodules, not the
configured worktree.

*Rejected: derive the main worktree from the registry entries already in
`RepoGatherInput.worktrees`.* Cannot reproduce the warm answer when the main
worktree is not itself registered — a supported case, since the user may register
only a linked worktree.

### 3. The desktop command delegates; the presentation join goes with it

`get_workspace_views` becomes `Ok(svc.workspace_views())`, taking
`State<'_, AppService>`. The `AppService` is already managed state and other
commands already take it, so no `lib.rs` wiring changes; `list_workspaces` still
needs the watcher and presentation handles, so neither `manage` call is removed.

There is exactly one behavioural difference, and it is deliberate: the desktop
copy returned `Err` on a poisoned presentation mutex, while
`AppService::workspace_views` skips the join and returns unjoined rows. After
delegation a poisoned mutex degrades the desktop tree to default labels instead
of failing to render — identical to what `specforge-web` and `specforge-tui`
already do, and the disabled filter runs before the lock either way. Nobody
should "restore" the `?` by making `AppService::workspace_views` return `Result`;
that would ripple into three call sites for a case no code path can currently
produce.

*Reported, not proposed:* `list_workspaces` in `commands.rs` is a second verbatim
duplicate of `AppService::list_workspaces`, including this feature's `disabled`
join. Collapsing it makes `SharedPresentation` dead in `commands.rs` and pulls
`crates/specforge/src/lib.rs` into the diff. It is the strongest remaining
instance of this defect class and belongs in its own change.

### 4. Unregister's cascade stays gated — the identity was wrong, not the gate

`remove_workspace` keeps `was_user_registered` as the gate and keeps
`presentation_keys_to_drop` as written; only the canonicalisation that forms the
lookup key changes.

*Rejected: make the presentation cleanup unconditional.* It does not fix the
defect — a failed lookup destroys `target_repo_id` too, so an unconditional pass
would still drop only the `Flat` key and never the repo-keyed entry that carries
a repository group's flag — and it introduces two regressions: unregistering a
*discovered* worktree (which `specforge-web`'s dispatch accepts as an arbitrary
client-supplied path) would run the cascade for a row the user never
unregistered, and a call for a path the registry does not know would remove
nothing yet still drop a `Flat` entry, i.e. silent data loss on a no-op.

*Rejected: derive the cleanup identity structurally from `unregister`'s return
value* (snapshot the entries, then locate the removed one). Genuinely more robust
— the two sites could never drift again — but ~15 lines plus a helper whose own
mutants need fresh tests, to close a gap that using the registry's own
canonicalisation closes exactly: identical function, identical input, identical
fallback, therefore identical key.

*Refuted while verifying, and worth recording so it is not "fixed" later:* the
deleted-folder case does **not** orphan an entry. When canonicalisation fails
both sites fall back to the raw path, the registry stores that fallback as its
key, and the frontend always passes the registry's own `uri` — so the lookup
hits. The defect is Windows-only, and it leaks the whole entry (name and tint as
well as `disabled`), not just the flag.

### 5. The disabled count is a count of top-level rows — defined once, derived twice

The number is defined as *the count of top-level rows the tree filter removes*:
`workspace_views().iter().filter(|v| v.is_disabled()).count()` on the unfiltered
snapshot, which is identically `raw.len() - AppService::workspace_views().len()`.
`gather_views` emits exactly one slot per top-level row, and `is_disabled` reads
the row's own flag, so a repository registered at two worktrees counts once by
construction.

The two frontends derive it from different data because they *have* different
data. `specforge-tui` is in-process and reads the unfiltered snapshot directly.
The React frontend cannot: `disabled` is deliberately absent from the wire
`WorkspaceView`, so its only sight of a parked row is `list_workspaces`, and it
must deduplicate by a key mirroring `PresentationKey` — `repo:<repoId>` or
`flat:<uri>`, prefixed so the two namespaces cannot collide and so flat rows do
not all fold into one `null` bucket. Each side carries the assertion that pins
the definition: the terminal asserts `disabled_row_count == raw - rendered
headers`; the frontend asserts that two rows sharing a `repoId` count once.

*Rejected: put the count on the `DashboardData` payload.* It is an IPC type with
a hand-written TypeScript mirror, and the `dashboard` capability contractually
states the payload is unaffected by the disabled flag. The count is a view
concern, not a record concern.

*Rejected: count from the TUI's own `settings_workspaces` mirror.* That is the
React bug transliterated: it counts registered entries.

*Deliberately deferred:* a shared `AppService::disabled_row_count()` is the
cleanest long-term home and would collapse the terminal's expression to a call.
It is not taken here only because `service.rs` is being changed by a different
part of this batch; the terminal's expression is byte-for-byte the body such a
helper would have, so adopting it later is a one-line change with no behaviour
delta — and whoever adds it must bring the two-worktrees assertion with it, since
`openspec-app` is in mutation scope.

### 6. A parked Dashboard row is marked and actionable — which amends a shipped scenario

A ship belonging to a parked row stays a `<button>`, is visibly marked as
disabled, and routes to Settings, where the switch that hid it lives. Resolution
moves from worktree path to repository identity: `ShipEntry` gains `repoId`, and
one pure function classifies each row as openable, parked, or unavailable. That
also repairs a defect unrelated to parking — a ship archived inside a feature
worktree that hosts no *active* change was already a dead click for an **enabled**
repository, because the resolver walks only `view.active[].instances` and
`RepoView.archived` is not serialized.

This contradicts the shipped scenario *Ships from a disabled workspace still
appear*, whose final clause reads "selecting it opens the archive browser as it
would for an enabled workspace". That clause is not implementable without
reversing D3: the archive address carries a registry slug derived from a
`WorkspaceView`, and a parked row has no view. The requirement is therefore
amended to the outcome that is actually deliverable — the selection is
acknowledged, never a silent no-op, and the user is given the way back.

*Rejected: the click un-parks the workspace and then navigates.* Parking is an
explicit Settings decision; a navigation gesture that silently reverses a
persisted user choice is the wrong kind of surprise.

*Rejected: render the row inert (a `<div>` plus a tooltip).* Honest, but it
leaves the user with no action, and a tooltip is invisible to touch and
keyboard users.

*Rejected: add a `disabled` boolean to `ShipEntry`.* `repoId` is an identity
rather than a flag: it keeps the "`disabled` never crosses the wire" invariant
intact and fixes the enabled-feature-worktree click for free.

### 7. Address resolution gains a fourth outcome rather than softer copy

`resolveAddress` returns `resolved | ambiguous | disabled | notFound`. A parked
row's slug is reconstructed from `list_workspaces` by `matchParkedSlug`
(`src/workspaceRows.ts`) — `slugify(ws.name)` for every parked row, flat or
repository, plus the `${base}-${shortHash(ws.repoId ?? ws.uri)}` suffixed form,
which are the two forms `slug.ts` emits and the same identity it hashes — and
only the three *scope-miss* sites are rerouted, never the change-missing or
artifact-missing ones, which remain genuine misses inside a resolvable
workspace. The notice names the workspace and offers a one-click re-enable; no
navigation follows, because the command emits `workspace-presentation-updated`,
`useWorkspaces` refetches, and the unchanged address resolves on the next render.

The reconstruction does no path arithmetic on `repoId`, deliberately and against
this design's first draft, which derived a repository's base from
`slugify(basename(dirname(repoId)))`. That is the very heuristic Decision 2
deletes from `openspec-core`: the relationship between a git common dir and its
worktree's name is a layout detail (`<super>/.git/modules/<sub>` for a submodule,
anywhere at all under `--separate-git-dir`), so transliterating it into
TypeScript would have reintroduced in the frontend the bug the Rust fix removes.
The registered listing already carries a name; the reconstruction uses that.

Decision 2 is therefore *not* load-bearing here, though the draft assumed it was:
the reconstruction reads the listing and never a view, so whether a row would be
gathered cold or warm does not enter. What decides a match is whether the
registered folder's name is the name the row's view carried — see Risks for where
that parts company, which is a different place from the edge first drafted.

*Rejected: keep one `notFound` branch and soften its copy when any row is
parked.* Smaller and fully honest, but it cannot name the workspace or offer the
targeted re-enable, which is the whole point.

*Rejected: un-skip `disabled` on the wire and drop the filter from
`get_workspace_views`.* Reverses D3 and would let a frontend mistake a cold row's
defaulted `dirty`/`branch` fields for real git state.

*Rejected: a new `list_parked_rows` command.* `list_workspaces` already carries
every input the reconstruction needs.

### 8. The terminal toggle is Space, immediate, and carries its failure back

Space is the Settings screen's existing "flip the focused row" verb — Toggle and
Appearance rows both use it — and it is unbound on workspace rows and not
intercepted by the global key router. `set_workspace_disabled` is `async` and
awaits a `spawn_blocking` sweep, so the synchronous `update` cannot call it: the
handler clones the service and the sender and `tokio::spawn`s the call. Nothing
is flipped locally; the row renders `(disabled)` from the refreshed mirror, with
the footer and help overlay advertising the key.

The spawned task sends `Msg::DisableResult(Result<(), String>)` rather than the
bare `Msg::Cache` the remove flow uses, because **a refresh alone cannot signal
a failure**. With Decision 9's rollback in place the mirror always reports what
is actually stored, so a failed park leaves the row byte-identical to one the
user never touched — indistinguishable from the keypress never landing. Carrying
the `Result` back lets the handler run the identical refresh and then report on
`Err` via `model.status`, matching `cycle_workspace_color`'s mechanism and
wording on the same row. Order matters: `Model::refresh` ends by overwriting
`status` with the workspace summary, so the report must follow the refresh, not
precede it.

*Rejected: treating the refreshed mirror as self-correcting.* This was the
original design, and it was wrong twice over. Before Decision 9 it was wrong
because the mirror reported the *attempted* value (the store mutated in memory
before persisting), so a bare refresh actively lied. After Decision 9 it is
wrong because the mirror reports the stored value and therefore says nothing at
all. Either way it reintroduces, in the surface this change adds, precisely the
swallowed-failure defect the same change fixes on the desktop (finding F7).

### 9. A presentation write that fails rolls its in-memory entry back

`WorkspacePresentationStore`'s three mutators — `set`, `set_disabled` and
`remove` — each edited `self.entries` and *then* called `save()`, with no
rollback. `entries` is not a private cache: `AppService::list_workspaces` reads
it through `lookup_row`, and the aggregator stamps `is_disabled` from the
`disabled_keys` snapshot of the same map. A failed write therefore left every
frontend reporting the value the user *attempted* while the file still held the
old one, until the next launch reloaded it and silently reverted.

That directly contradicts the *Settings View* requirement this change adds,
whose second half is "SHALL continue to show the stored state rather than the
attempted one". The three mutators now route through one private
`mutate_and_save`, which captures the whole previous `Option<PresentationEntry>`
before mutating and restores it if the save fails. Capturing the `Option` rather
than reconstructing the entry is what covers all three pre-states: key absent
(remove on rollback), key present with different fields (re-insert), and the
case where the mutator itself pruned an entry its edit emptied (also re-insert).

This is the single point that makes the promise true for the desktop, the web UI
and the terminal at once, and for all three per-workspace controls rather than
just the toggle.

*Rejected: rolling back in each frontend.* Three copies of a rule the store can
enforce once, and the web UI would still have needed its own.

*Not addressed here:* `WorkspaceRegistry::register` and `unregister` have the
same shape — in-memory mutation, then `save()`, no rollback. No requirement in
this change turns on it, and registry writes have a different failure story
(`register` also runs worktree discovery), so it is left as its own change
rather than widened into this one.

*Rejected: `d` as a mnemonic.* Free, but Space is the screen's established verb;
binding both is two branches for one action.

*Rejected: a confirmation overlay.* Parking is reversible from the same row; the
colour cycle sets the precedent for an immediate, unconfirmed row mutation, while
confirmation is reserved for removal.

*Rejected: optimistically flipping the mirror before the await.* It can desync
from the store on failure — the one thing the re-read guarantees against.

### 9. A shared switch says what it governs; a rejected write says it was rejected

Settings keeps one row per registered folder — they differ in `uri`, `name`, and
`isMissing`, and each must stay individually removable — and rows whose
presentation key is shared carry a note naming how many folders move with them,
in both the visible copy and the toggle's accessible label. A failed toggle
renders inline in its own row, reusing the file's existing `prettifyError` +
`.settings-error` convention (extracted so it is testable without pulling React
into the test process).

*Rejected: collapse sibling worktrees into one grouped row.* Each folder must
remain individually removable and individually flagged missing, so the grouped
row needs a nested per-worktree sub-list — a redesign of the Settings list to
explain a shared boolean.

*Rejected: disable the switch on all but the first sibling row.* Hides a legal
action and still explains nothing.

*Rejected: lift one error string to the section.* With N rows the user cannot
tell which switch failed, and it collides with the add-workspace error.

## Risks / Trade-offs

**One number, two derivations.** Decision 5 accepts what Decision 3 refuses
everywhere else: the disabled-row count is computed in TypeScript from
`list_workspaces` and in Rust from the unfiltered snapshot. The justification is
that the wire deliberately withholds the input the shared derivation would need,
so the alternative is not "one implementation" but "a new IPC field on a payload
the spec says is unaffected". *Mitigation:* the number is defined normatively in
the `dashboard` capability as disabled top-level rows rather than registered
folders, and each side carries an assertion tying its derivation to that
definition.

**The parked-slug reconstruction has one soft edge, and it is not a layout
edge.** The token a link carries was minted from the *view's* name, which for a
repository row is its **main worktree's** basename (`RepoView::name`, from
`main_worktree_for_common_dir`). The reconstruction has only the *registered
folder's* name to work from, so the two agree exactly when the registered folder
is that main worktree — the ordinary case, in every git layout — and disagree
when it is not. The case that degrades is therefore a repository registered only
at a linked worktree: `/proj` (common dir `/proj/.git`) registered solely at
`/proj/.claude/worktrees/feature` mints `proj` while enabled and reconstructs
`feature` while parked, so its address falls through to `notFound`. Decision 2's
third rejected alternative already names that registration as supported, and this
repository's own worktree convention produces it. `--separate-git-dir` degrades
for the same single reason and not a layout-specific one — git calls the store
directory the main worktree, and the store directory is never the folder the user
registered — while a bare repository registered at its own store directory
matches, since there the two names are one string. Flat rows are exact: a flat
row's registered name is the name its view carries. The approximation runs in
*both* directions, and the second is the one worth naming: because the listing is
matched per registered folder, a token that addressed nothing can match a parked
repository via a *secondary* worktree's basename slug, reporting "disabled" for a
row no address ever named. *Mitigation:* both wrong answers are bounded and
recoverable, because this lookup is consulted only where the token already
matched no live view — so the real choice is between `notFound` and "this parked
repository", both dead ends for the address, and the parked one at least hands
the user the control that un-parks the row it named. `matchParkedSlug`'s doc
comment states both edges and records why narrowing was rejected: it would need
path arithmetic over `repoId`, which this module refuses, and a wrong guess
would turn a *real* parked link into `notFound` — trading a mild wrong answer for
the exact outcome this path exists to remove.
`workspaceRows.test.ts` mints its tokens with the real emitters (`slugFor`,
`archiveSlugFor`) rather than with literals, so the two sides cannot drift apart
unnoticed. Closing the edge needs the row's main-worktree name to reach the
frontend — a new field on `RegisteredWorkspace`, i.e. a wire change — which is a
follow-up, not a blocker.

**F4's defect is unreachable on CI.** `dunce::canonicalize` *is*
`std::fs::canonicalize` off Windows, so the regression test is a plain contract
test on Linux and macOS and can only fail-before on Windows. A green CI run is
not evidence the Windows path was exercised — the same caveat CLAUDE.md already
records for the WSL backends.

**The mutation gate cannot see three of the four crates being changed.**
`crates/specforge`, `crates/specforge-tui`, and `crates/specforge-web` are all
excluded, and the workflow's diff pathspec covers only `openspec-core/src` and
`openspec-app/src`. Decision 3 improves this by moving desktop behaviour into a gated
crate, but the terminal work is ungated by construction; its tests are written to
kill the obvious mutants anyway, so the gate stays satisfiable if the exclusion is
ever lifted.

**The `total_active_logical_count` refactor puts a well-covered function back in
scope.** Every existing assertion on it is 0 or 1, so a `replace r.active.len()
with 1` mutant would survive a diff that touches its body. *Mitigation:* the new
test registers one enabled repository with **two** active changes and asserts 2,
then parks it and asserts 0 — which also kills the `sum`, `changes.len()`, and
inverted-filter mutants that a single-row fixture cannot.

**A behaviour change reaches enabled repositories.** The `unavailable` arm of the
ship-row classifier routes a previously-dead click to Settings even when the
cause is a vanished worktree rather than parking. That is the intended catch-all
— the alternative is restoring a silent no-op — but it is a visible change for
rows that have nothing to do with this feature.

**`ShipEntry` gains a field on a hand-mirrored IPC type.** There is no codegen:
`repo_id` in `crates/openspec-core/src/dashboard.rs` and `repoId` in
`src/types.ts` must land together, or every ship row silently classifies as
unavailable. Its killing assertion goes on the existing
`ships_filter_to_today_and_join_clock_and_title` test, beside the fixture that
already sets the repo id.
