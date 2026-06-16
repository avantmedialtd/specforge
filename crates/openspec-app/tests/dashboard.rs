//! The dashboard assembly used to live behind `#[tauri::command]` in the Tauri
//! shell, so it could not be exercised from `cargo test`. After the extraction
//! into `openspec-app::AppService` it is plain, in-process Rust — these tests
//! are the regression net that the extraction unlocked.

use openspec_app::AppService;
use tempfile::tempdir;

/// The assembly is callable with no Tauri and no registered workspaces, and
/// returns the analytics-only payload (gamification is off by default).
#[tokio::test]
async fn dashboard_is_callable_headless_with_no_workspaces() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());

    let data = svc.dashboard().await.expect("dashboard assembles headless");

    assert!(
        !data.gamification_enabled,
        "gamification is off by default, so the gamified layer is skipped"
    );
    assert!(
        data.season.is_none(),
        "no season standing without gamification"
    );
    assert!(data.repos.is_empty(), "no repos registered");
}

/// The season completion target is paced from the developer's *entry* baseline
/// and held fixed for the season — it must not drift between reads. This is the
/// regression the entry-baseline pacing fixed; before the extraction it could
/// not be asserted at the assembly level at all.
#[tokio::test]
async fn season_target_is_stable_across_reads() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());
    svc.settings
        .set_gamification_enabled(true)
        .expect("enable gamification");

    let first = svc.dashboard().await.expect("first read");
    let second = svc.dashboard().await.expect("second read");

    assert!(first.gamification_enabled && second.gamification_enabled);
    assert!(
        first.season.is_some(),
        "gamified dashboard has a season standing"
    );

    let a = serde_json::to_value(&first.season).unwrap();
    let b = serde_json::to_value(&second.season).unwrap();
    assert_eq!(a, b, "season standing drifted between dashboard reads");
}
