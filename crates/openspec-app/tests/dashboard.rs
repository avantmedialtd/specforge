//! The dashboard assembly used to live behind `#[tauri::command]` in the Tauri
//! shell, so it could not be exercised from `cargo test`. After the extraction
//! into `openspec-app::AppService` it is plain, in-process Rust — these tests
//! are the regression net that the extraction unlocked.

use openspec_app::AppService;
use openspec_core::git::invocation_log;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

/// The assembly is callable with no Tauri and no registered workspaces, and
/// carries the progress layer with no opt-in to set — there is no setting that
/// can suppress it (`dashboard`: *Unconditional Progress Layer*).
#[tokio::test]
async fn dashboard_is_callable_headless_with_no_workspaces() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let data = svc.dashboard().await.expect("dashboard assembles headless");

    assert!(data.repos.is_empty(), "no repos registered");
    assert!(
        !data.progress.heatmap.is_empty(),
        "the heatmap window is always computed, with no opt-in"
    );
}

/// The `get_identity` payload carries the saved config and the detected
/// candidates, and **nothing else** — the contributor roster it used to carry
/// is gone (`developer-identity`: *Named People Roster* removed).
///
/// This is the only test of `identity_info` anywhere. Its sole mutant,
/// `Ok(Default::default())`, is unviable only because `IdentityInfo` derives no
/// `Default`, so without an assertion here the mutation gate reports the
/// function as covered when nothing exercises it at all. The serialised shape
/// is asserted rather than the struct, because `src/types.ts` mirrors this by
/// hand and a silent field change is invisible to both compilers.
#[tokio::test]
async fn identity_payload_is_config_and_candidates_only() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    svc.settings
        .set_display_name(Some("Ada".to_string()))
        .unwrap();

    let info = svc.identity_info().expect("identity payload assembles");
    assert_eq!(info.config.display_name.as_deref(), Some("Ada"));

    let json = serde_json::to_value(&info).expect("payload serialises");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("payload is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["candidates", "config"],
        "the get_identity payload gained or lost a field; src/types.ts mirrors \
         it by hand and nothing else would catch the drift"
    );
}

/// The progress layer is computed on a fresh config where no setting was ever
/// written, and does not drift between reads.
#[tokio::test]
async fn progress_layer_is_present_without_opt_in_and_stable_across_reads() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let first = svc.dashboard().await.expect("first read");
    let second = svc.dashboard().await.expect("second read");

    let a = serde_json::to_value(&first.progress).unwrap();
    let b = serde_json::to_value(&second.progress).unwrap();
    assert_eq!(a, b, "progress layer drifted between dashboard reads");
}

// --- Lifecycle-cache mining (cache-change-lifecycle, tasks 5.1 / 5.2) -------
//
// These exercise the *real* `change_lifecycle` git invocation through
// `AppService::dashboard()`, counted via `openspec_core::git::invocation_log`
// (the same test-only instrumentation `openspec-core`'s own watcher/repo
// monitor tests use to bound git spawn counts). Wall-clock is deliberately
// not asserted — see design.md's "Verification strategy".

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git repo carrying an `openspec/changes/` tree with one committed active
/// change, so `change_lifecycle` has a real add-event to mine. An empty repo
/// would yield zero `A` rows regardless of whether mining ran at all, which
/// would make an invocation-count-only assertion the test's entire signal.
fn init_repo_with_a_change(root: &Path) -> PathBuf {
    let change_dir = root.join("openspec").join("changes").join("add-x");
    std::fs::create_dir_all(&change_dir).unwrap();
    std::fs::write(change_dir.join("proposal.md"), "# X").unwrap();
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    git(&["add", "."], root);
    git(&["commit", "-m", "add x"], root);
    root.canonicalize().unwrap()
}

fn register(svc: &AppService, path: &Path) {
    svc.registry
        .lock()
        .unwrap()
        .register(path.to_path_buf())
        .unwrap();
}

/// Count real `change_lifecycle`/`change_lifecycle_checked` git invocations
/// issued for `repo_root` since `mark`. `--diff-filter=A` uniquely
/// discriminates this specific git call — no other invocation in this
/// codebase passes that flag — so filtering on it (plus the anchor path, to
/// stay isolated from any other test's git calls landing in the same
/// process-global log — see `invocation_log`'s module doc) counts exactly
/// the lifecycle-mining spawns.
fn lifecycle_mining_calls_for(mark: usize, repo_root: &Path) -> usize {
    let repo_root_canonical = repo_root.canonicalize().unwrap();
    invocation_log::recorded_since(mark)
        .into_iter()
        .filter(|inv| {
            inv.anchor.starts_with(&repo_root_canonical)
                && inv.args.iter().any(|a| a == "--diff-filter=A")
        })
        .count()
}

/// The commit garden is computed unconditionally: a registered repository with
/// a commit landed today yields a live (non-dormant) plot on a fresh config
/// where no setting was ever written (`commit-garden`: *Per-Workspace Commit
/// Graphs at the Dashboard Bottom*, scenario *Section needs no opt-in*).
///
/// This is the assertion that kills the `commit_garden -> Ok(vec![])` mutant.
/// Before this change an opt-in guard made exactly that substitution the
/// function's default behaviour, so "always empty" has to be a detectable
/// regression rather than a plausible one.
///
/// The garden is *today*-scoped and git stamps the commit from the wall clock,
/// while `compute_garden` reads its own `local_today()` later — so a run that
/// crosses local midnight in between would legitimately see zero commits for
/// "today" and fail. Pinning the commit date does not close that window (the
/// comparison is against a clock read *after* the commit either way), so the
/// boundary is instead *detected* and retried once: an unavoidable, precisely
/// identified race is handled rather than left to read as a flake.
#[tokio::test]
async fn commit_garden_is_computed_without_opt_in() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));
    register(&svc, &repo);
    svc.populate().await;

    let day_at_commit = openspec_core::local_today();
    let mut plots = svc.commit_garden().await.expect("garden computes");

    if openspec_core::local_today() != day_at_commit {
        // Midnight landed mid-test: the seed commit belongs to yesterday now.
        // Land a fresh commit on the new day and re-read, once.
        std::fs::write(repo.join("openspec/changes/add-x/notes.md"), "y").unwrap();
        git(&["add", "-A"], &repo);
        git(&["commit", "-m", "post-midnight"], &repo);
        plots = svc.commit_garden().await.expect("garden recomputes");
    }

    assert!(
        !plots.is_empty(),
        "a registered repository must yield a plot with no setting consulted"
    );
    let live: Vec<_> = plots.iter().filter(|p| !p.dormant).collect();
    assert_eq!(
        live.len(),
        1,
        "the repository committed to today, so its plot is live: {plots:?}"
    );
    assert!(
        !live[0].commits.is_empty(),
        "the live plot carries today's commits"
    );
}

// --- The garden's plot order and caption annotation ------------------------
//
// The garden is the Dashboard's only per-repository surface now that the
// analytics band is gone, so the order it returns is what the reader sees and
// the active count it carries is the one figure the removed breakdown
// contributed (`commit-garden`: *Deterministic Plot Order*, *Plot Caption*).
// Both live in `AppService::commit_garden`, inside the mutation gate.

/// Every fixture below seeds its commits from the wall clock and reads them
/// back against a later `local_today()`, so a run that crosses local midnight
/// in between would legitimately see zero commits today and every plot dormant.
/// The window is the fixture's own setup — a few hundred milliseconds — and is
/// detected rather than left to read as a flake, matching how
/// `commit_garden_is_computed_without_opt_in` handles the same boundary.
const CROSSED_MIDNIGHT: &str =
    "the test crossed local midnight between seeding its commits and reading \
     the garden, so the seeds belong to yesterday — re-run";

/// A git repository carrying `changes` active OpenSpec changes and `commits`
/// commits landed now: the two axes the garden reads, independently. The
/// directory's basename becomes the plot's label, no display-name override
/// being configured.
fn init_repo_with(root: &Path, changes: usize, commits: usize) -> PathBuf {
    let changes_dir = root.join("openspec").join("changes");
    std::fs::create_dir_all(&changes_dir).unwrap();
    for i in 0..changes {
        let change_dir = changes_dir.join(format!("c{i}"));
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# C").unwrap();
    }
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    for j in 0..commits {
        std::fs::write(root.join(format!("n{j}.md")), "x").unwrap();
        git(&["add", "-A"], root);
        git(&["commit", "-m", &format!("commit {j}")], root);
    }
    root.canonicalize().unwrap()
}

/// Task 1.4: the plant carries the entry's own registry-wide active-change
/// count, from the `WorkspaceView` the service already holds — `r.active.len()`
/// for a repository group and `changes.len()` for a flat workspace. A flat
/// workspace is always dormant and so never rendered, but its count is filled
/// anyway rather than left at a zero that would later read as data
/// (`commit-garden`: *Plot Caption*).
#[tokio::test]
async fn garden_plants_carry_the_entry_active_change_count() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let day = openspec_core::local_today();
    let repo = init_repo_with(&ws.path().join("repo"), 3, 1);
    register(&svc, &repo);
    register(&svc, &flat_workspace_with_changes(ws.path(), "flat", 2));
    svc.populate().await;

    let plots = svc.commit_garden().await.expect("garden computes");
    assert_eq!(openspec_core::local_today(), day, "{CROSSED_MIDNIGHT}");

    let repo_plot = plots
        .iter()
        .find(|p| p.label == "repo")
        .expect("the registered repository has a plot");
    assert!(!repo_plot.dormant, "the repository committed today");
    assert_eq!(
        repo_plot.active_count, 3,
        "the plot carries the repository's three active changes, not its \
         commit count and not zero"
    );

    let flat_plot = plots
        .iter()
        .find(|p| p.label == "flat")
        .expect("the registered flat workspace has a plot");
    assert!(flat_plot.dormant, "a flat workspace is always dormant");
    assert_eq!(
        flat_plot.active_count, 2,
        "a dormant flat plant reports its own change count rather than zero"
    );
}

/// Tasks 2.1-2.4: the service *applies* the presentation order to plants that
/// really carry today's commits.
///
/// The comparator itself is unit-tested over pure data in
/// `openspec_core::garden` — the leading key, the label tiebreak with an
/// anti-correlated active-count fixture, duplicate labels, and totality. Those
/// tests need no repository on disk, no runtime and no midnight guard, and
/// because `plot_order` is a named function the mutation gate generates real
/// mutants for it; a closure inside this crate's `async fn` produced none.
///
/// What only an integration test can show is the wiring, so this one asserts
/// the commit counts and non-dormancy alongside the order. Without those two
/// assertions it would pass unchanged if every plant came back dormant and
/// empty: they would all tie at zero on the leading key and the label tiebreak
/// would reproduce the expected labels exactly — so a regression that stopped
/// commits reaching the plants at all would go unnoticed
/// (`commit-garden`: *Deterministic Plot Order*).
#[tokio::test]
async fn garden_plots_are_returned_in_presentation_order() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    let day = openspec_core::local_today();
    // Registered bravo, alpha, charlie — reversed: charlie, alpha, bravo.
    // Expected: alpha, charlie, bravo. Distinct from both.
    for (name, commits) in [("bravo", 1), ("alpha", 3), ("charlie", 2)] {
        register(&svc, &init_repo_with(&ws.path().join(name), 1, commits));
    }
    svc.populate().await;

    let plots = svc.commit_garden().await.expect("garden computes");
    assert_eq!(openspec_core::local_today(), day, "{CROSSED_MIDNIGHT}");

    let ordered: Vec<(&str, usize)> = plots
        .iter()
        .map(|p| (p.label.as_str(), p.commits.len()))
        .collect();
    assert_eq!(
        ordered,
        vec![("alpha", 3), ("charlie", 2), ("bravo", 1)],
        "plots order by today's commit count descending: {plots:?}"
    );
    assert!(
        plots.iter().all(|p| !p.dormant),
        "every plot actually carries today's commits: {plots:?}"
    );
    assert!(
        plots.iter().all(|p| !p.entry_key.is_empty()),
        "the service fills each plant's stable entry key: {plots:?}"
    );
}

/// Count real year-long commit-activity walks issued for `repo_root` since
/// `mark`. The `%aI<US>%an<US>%ae` pretty-format is unique to
/// `commit_activity_with_authors` — the lifecycle mine also carries `%an`, but
/// with `%at` and a `--diff-filter=A`, so matching the whole format string
/// (rather than just `%an`) is what discriminates the two.
fn commit_activity_calls_for(mark: usize, repo_root: &Path) -> usize {
    let repo_root_canonical = repo_root.canonicalize().unwrap();
    invocation_log::recorded_since(mark)
        .into_iter()
        .filter(|inv| {
            inv.anchor.starts_with(&repo_root_canonical)
                && inv
                    .args
                    .iter()
                    .any(|a| a == "--pretty=format:%aI\u{1f}%an\u{1f}%ae")
        })
        .count()
}

/// The year-long commit walk that backs the heatmap and streak is cached per
/// repository, so a second Dashboard fetch with no intervening history change
/// does not re-walk it.
///
/// This walk used to sit behind the gamification opt-in, which defaulted to
/// off; making the progress layer unconditional would otherwise have imposed a
/// ~30-40ms `git log` per registered repository on EVERY fetch for every user.
#[tokio::test]
async fn commit_activity_is_walked_once_across_two_fetches() {
    invocation_log::enable();
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));
    register(&svc, &repo);

    // Marked before `populate()` for the same reason as the lifecycle test:
    // populate starts a fire-and-forget warm that would otherwise race the mark.
    let mark = invocation_log::mark();
    svc.populate().await;
    svc.dashboard().await.expect("first fetch");
    svc.dashboard().await.expect("second fetch");

    assert_eq!(
        commit_activity_calls_for(mark, &repo),
        1,
        "two fetches with no intervening GraphChanged must walk the year once"
    );
}

/// Task 5.1: two consecutive `dashboard()` fetches with no intervening
/// `GraphChanged` mine a registered repository exactly once, and both reflect
/// the same derived lifecycles.
#[tokio::test]
async fn unchanged_repository_is_mined_at_most_once_across_two_fetches() {
    invocation_log::enable();
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));

    // Archive a change dated today, so the ships feed the equivalence assertion
    // reads is NON-EMPTY. `init_repo_with_a_change` creates no
    // `openspec/changes/archive/` tree at all, and `repo_ships` derives
    // membership from a dated archive directory — so without this the
    // assertion below compares two empty vectors and can never fail, however
    // badly the lifecycle cache is broken.
    let day = openspec_core::local_today();
    let archived = repo
        .join("openspec")
        .join("changes")
        .join("archive")
        .join(format!("{day}-add-x"));
    std::fs::create_dir_all(&archived).unwrap();
    std::fs::write(archived.join("proposal.md"), "# X").unwrap();
    git(&["add", "."], &repo);
    git(&["commit", "-m", "archive x"], &repo);

    register(&svc, &repo);

    // Marked BEFORE `populate()`, not after: `populate` starts a
    // fire-and-forget background warm (see `AppService::populate`), and
    // marking afterwards races it — if the warm wins, both `dashboard()`
    // fetches below would observe zero *new* mining calls instead of one.
    // Marking first brings the warm's own mining invocation inside the
    // counted window, so the count is deterministically 1 either way:
    // single-flight guarantees exactly one real invocation happens for this
    // repo, whether it's the warm, the first fetch, or both racing into one.
    let mark = invocation_log::mark();
    svc.populate().await;
    let first = svc.dashboard().await.expect("first fetch");
    let second = svc
        .dashboard()
        .await
        .expect("second fetch, no intervening event");

    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo),
        1,
        "two fetches with no intervening GraphChanged must mine the repo exactly once"
    );
    // Both fetches must reflect the same derived lifecycles. Today's ships
    // feed is what the mining now dates — the aggregate metrics it also fed
    // are gone — so it is what the equivalence is asserted on
    // (`dashboard`: *Change Lifecycle Mining*, scenario *Concurrent fetches
    // mine once*).
    //
    // Assert the feed is populated FIRST. The equivalence below is vacuous
    // against two empty vectors, and an empty feed is exactly what a fixture
    // regression (or a midnight rollover past the archive directory's date)
    // would produce — so the emptiness check is what keeps the real assertion
    // honest rather than silently self-satisfying.
    assert_eq!(openspec_core::local_today(), day, "{CROSSED_MIDNIGHT}");
    assert_eq!(
        first.todays_ships.len(),
        1,
        "the fixture must ship today, else the equivalence below is [] == []: {:?}",
        first.todays_ships
    );
    assert!(
        first.todays_ships[0].archived_at.is_some(),
        "the ship carries a git-recovered archival instant, which is the part \
         only the mining can supply"
    );
    assert_eq!(
        serde_json::to_value(&first.todays_ships).unwrap(),
        serde_json::to_value(&second.todays_ships).unwrap(),
        "the mined lifecycles must reach both fetches identically"
    );
}

/// Task 5.2: a `GraphChanged` for repository A causes the *next* fetch to
/// re-mine A only — repository B, untouched by the event, stays cached.
/// The invalidation is processed by a background subscriber
/// (`AppService::spawn_lifecycle_cache_invalidator`), so this polls
/// `dashboard()` until the re-mine is observed rather than assuming a fixed
/// delay (a fixed sleep here would flake under CI load — see the project's
/// established "wait for activity, not a timer" testing convention).
#[tokio::test]
async fn graph_changed_re_mines_only_the_affected_repository() {
    invocation_log::enable();
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo_a = init_repo_with_a_change(&roots.path().join("a"));
    let repo_b = init_repo_with_a_change(&roots.path().join("b"));
    register(&svc, &repo_a);
    register(&svc, &repo_b);
    svc.populate().await;

    // Warm both repositories via one real fetch.
    svc.dashboard().await.expect("warm fetch");

    let repo_a_id = openspec_core::git_common_dir(&repo_a).expect("repo A has a git dir");

    let mark = invocation_log::mark();
    svc.watcher.emit(openspec_core::CacheEvent::GraphChanged {
        repo_id: repo_a_id.into_path_buf(),
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        svc.dashboard().await.expect("poll fetch");
        if lifecycle_mining_calls_for(mark, &repo_a) >= 1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "GraphChanged for repo A was never reflected in a re-mine within the timeout"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo_a),
        1,
        "repo A must be re-mined exactly once after its GraphChanged"
    );
    assert_eq!(
        lifecycle_mining_calls_for(mark, &repo_b),
        0,
        "repo B must not be re-mined by repo A's GraphChanged"
    );
}

/// Regression test for finding M2 (adversarial review of `cache-change-lifecycle`):
/// unregistering a workspace must evict its repository's lifecycle-cache
/// entry, not just tear down its watcher/monitor. Without this, a repo's
/// `RepoMonitor` (and with it, the only source of `GraphChanged` — the
/// cache's sole invalidation signal) is torn down on unregister; a commit
/// landing while the repo is unregistered would then go unnoticed, and on
/// re-registration the Dashboard would keep serving the pre-removal snapshot
/// until some later, unrelated commit happened to invalidate it.
///
/// The observable is today's ships feed, which is what the mined lifecycles now
/// date: membership comes from the dated archive directory alone (no git), so
/// it is the entry's `archived_at` — recoverable only from the archive commit —
/// that a stale, pre-removal cache entry could not supply.
#[tokio::test]
async fn unregistering_a_workspace_evicts_its_lifecycle_cache_entry() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let roots = tempdir().unwrap();
    let repo = init_repo_with_a_change(&roots.path().join("repo"));
    register(&svc, &repo);
    svc.populate().await;

    // Warm the cache with a real fetch: nothing archived yet.
    let before = svc.dashboard().await.expect("warm fetch");
    assert!(
        before.todays_ships.is_empty(),
        "precondition: nothing archived before the repo is unregistered"
    );

    // Unregister — this must evict the cached entry, not just the watcher.
    svc.remove_workspace(repo.clone())
        .await
        .expect("unregister");

    // A commit lands while the repo is unregistered — no monitor is
    // watching it, so no `GraphChanged` is possible for this change. Archive
    // the change that `init_repo_with_a_change` created, dated today so the
    // ships feed carries it.
    let day = openspec_core::local_today();
    let archive_dir = repo.join(format!("openspec/changes/archive/{day}-add-x"));
    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::rename(
        repo.join("openspec/changes/add-x/proposal.md"),
        archive_dir.join("proposal.md"),
    )
    .unwrap();
    std::fs::remove_dir_all(repo.join("openspec/changes/add-x")).unwrap();
    git(&["add", "-A"], &repo);
    git(&["commit", "-m", "archive x"], &repo);

    // Re-register and refetch: the Dashboard must reflect the new history
    // immediately — not the pre-removal cached lifecycle, which (absent the
    // M2 fix) carries no archival instant for this change at all.
    register(&svc, &repo);
    svc.populate().await;
    let after = svc.dashboard().await.expect("post-re-register fetch");
    assert_eq!(openspec_core::local_today(), day, "{CROSSED_MIDNIGHT}");

    assert_eq!(
        after.todays_ships.len(),
        1,
        "the change archived while unregistered ships today: {:?}",
        after.todays_ships
    );
    assert!(
        after.todays_ships[0].archived_at.is_some(),
        "the archive that landed while unregistered must be dated from the \
         re-mined history, not served from a stale pre-removal cache entry"
    );
}

/// A flat (non-git) OpenSpec workspace under `tmp` carrying `change_count`
/// active changes, with no git involved. Flat rows report
/// `archived_count = 0` by construction.
fn flat_workspace_with_changes(tmp: &Path, name: &str, change_count: usize) -> PathBuf {
    let root = tmp.join(name);
    std::fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
    for i in 0..change_count {
        let change_dir = root.join("openspec").join("changes").join(format!("c{i}"));
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# C").unwrap();
    }
    root
}

/// Disabling is an attention control, not an existence control: a parked row
/// still contributes its counts to the Dashboard's cross-workspace data, so the
/// summary line's registry-wide totals stay complete (`dashboard`: *Dashboard
/// Includes Disabled Workspaces*, *Cross-Workspace Summary Metrics*).
///
/// Membership and counts, not order — the vector carries no ordering any more.
/// The asymmetry this pins is real - `AppService::workspace_views` filters
/// parked rows out for the tree, while `dashboard()` reads the watcher's
/// unfiltered views - so a future refactor routing the Dashboard through the
/// filtered accessor would drop the row entirely and fail here.
#[tokio::test]
async fn a_disabled_workspace_keeps_the_counts_it_contributes() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());

    svc.add_workspace(flat_workspace_with_changes(ws.path(), "quiet", 1))
        .await
        .expect("add quiet");
    let busy = svc
        .add_workspace(flat_workspace_with_changes(ws.path(), "busy", 2))
        .await
        .expect("add busy");
    svc.populate().await;

    let active_for = |data: &openspec_core::DashboardData, label: &str| -> Option<usize> {
        data.repos
            .iter()
            .find(|r| r.label == label)
            .map(|r| r.active_count)
    };

    let before = svc.dashboard().await.expect("dashboard");
    assert_eq!(active_for(&before, "busy"), Some(2));
    assert_eq!(active_for(&before, "quiet"), Some(1));

    svc.set_workspace_disabled(busy.uri.clone(), None, true)
        .await
        .expect("park busy");
    svc.populate().await;

    let after = svc.dashboard().await.expect("dashboard after parking");
    assert_eq!(
        active_for(&after, "busy"),
        Some(2),
        "a parked row keeps its counts; the Dashboard is the unfiltered record"
    );
    assert_eq!(
        active_for(&after, "quiet"),
        Some(1),
        "parking one row must not disturb another"
    );

    // ...while the tree, which the same parking DOES silence, has dropped it.
    assert_eq!(
        svc.workspace_views().len(),
        1,
        "the tree drops the parked row even as the Dashboard keeps it"
    );
}
