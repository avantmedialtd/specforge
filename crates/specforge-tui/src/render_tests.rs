//! Headless render smoke tests. The interactive TUI can't be exercised against
//! a real terminal in CI, but `ui::view` is a pure function of `Model`, so we
//! can drive it through ratatui's `TestBackend` and assert it never panics —
//! the real risk in the index-heavy widget code (graph elbows, the heatmap
//! grid, the 30-tier ladder's auto-scroll math).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::AppService;
use openspec_core::{CommitGraph, EdgeSegment, LaidOutCommit};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::tempdir;
use tokio::sync::mpsc;

use crate::app::{update, Model, Msg, Screen};
use crate::{graph, ui};

const SCREENS: [Screen; 6] = [
    Screen::Browse,
    Screen::Dashboard,
    Screen::Season,
    Screen::Garden,
    Screen::History,
    Screen::Settings,
];

/// A bootstrapped service over an empty config dir (no registered workspaces).
fn service() -> AppService {
    let dir = tempdir().unwrap();
    AppService::bootstrap(dir.path().to_path_buf())
}

fn draw_every_screen(model: &mut Model, width: u16, height: u16) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    for screen in SCREENS {
        model.screen = screen;
        terminal.draw(|f| ui::view(f, model)).unwrap();
    }
    model.show_help = true;
    terminal.draw(|f| ui::view(f, model)).unwrap();
    model.show_help = false;
}

#[test]
fn renders_empty_model_at_many_sizes() {
    let svc = service();
    let mut model = Model::new(&svc);
    // Two-pane, single-pane fallback, and a degenerate tiny terminal.
    draw_every_screen(&mut model, 120, 40);
    draw_every_screen(&mut model, 60, 20);
    draw_every_screen(&mut model, 8, 3);
}

#[test]
fn renders_with_an_active_filter() {
    let svc = service();
    let mut model = Model::new(&svc);
    model.filter = Some("nothing-matches-this".to_string());
    model.filter_editing = true;
    draw_every_screen(&mut model, 100, 30);
}

/// The gamified Dashboard and Season screens, fed a *real* assembled standing
/// (gamification on) so the 30-tier ladder and heatmap render with live data.
#[tokio::test]
async fn renders_gamified_screens_with_real_dashboard() {
    let svc = service();
    svc.settings
        .set_gamification_enabled(true)
        .expect("enable gamification");
    let data = svc.dashboard().await.expect("assemble dashboard");

    let mut model = Model::new(&svc);
    model.dashboard = Some(data);
    for (w, h) in [(120, 40), (40, 12)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        for screen in [Screen::Dashboard, Screen::Season] {
            model.screen = screen;
            terminal.draw(|f| ui::view(f, &model)).unwrap();
        }
    }
}

/// Ctrl-C must quit from anywhere, including the search field, and must not be
/// swallowed as a query character. A bare `q`, by contrast, is a valid query
/// character while editing and must not quit.
#[test]
fn ctrl_c_quits_during_search_but_q_is_a_query_char() {
    let svc = service();
    let (tx, _rx) = mpsc::unbounded_channel();

    let mut model = Model::new(&svc);
    model.filter = Some(String::new());
    model.filter_editing = true;
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    update(&mut model, Msg::Key(ctrl_c), &svc, &tx);
    assert!(model.should_quit, "Ctrl-C must quit during search");
    assert_eq!(
        model.filter.as_deref(),
        Some(""),
        "Ctrl-C must not append 'c' to the query"
    );

    let mut model = Model::new(&svc);
    model.filter = Some(String::new());
    model.filter_editing = true;
    let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    update(&mut model, Msg::Key(q), &svc, &tx);
    assert!(!model.should_quit, "bare q is a query char, not a quit");
    assert_eq!(model.filter.as_deref(), Some("q"));
}

/// A hand-built merge graph exercises the box-drawing elbow code for every
/// selection index without needing a real repository.
#[test]
fn commit_rail_renders_a_merge_without_panicking() {
    let commit = |row: usize, column: usize, parents: &[&str], subject: &str| LaidOutCommit {
        id: format!("sha{row}"),
        parents: parents.iter().map(|s| s.to_string()).collect(),
        author: "Dev".to_string(),
        date: "2026-06-16T00:00:00Z".to_string(),
        subject: subject.to_string(),
        refs: Vec::new(),
        trailers: Vec::new(),
        row,
        column,
    };
    let graph = CommitGraph {
        commits: vec![
            commit(0, 0, &["sha1", "sha2"], "merge branch"),
            commit(1, 0, &["sha2"], "mainline"),
            commit(2, 1, &[], "feature tip"),
        ],
        edges: vec![
            EdgeSegment {
                band: 0,
                from_column: 0,
                to_column: 0,
            },
            EdgeSegment {
                band: 0,
                from_column: 0,
                to_column: 1,
            },
            EdgeSegment {
                band: 1,
                from_column: 1,
                to_column: 1,
            },
        ],
        lane_count: 2,
        truncated: true,
    };

    for selected in 0..graph.commits.len() {
        let lines = graph::commit_rail(&graph, selected);
        // One line per commit, plus the truncation notice.
        assert_eq!(lines.len(), graph.commits.len() + 1);
    }
}

/// The opt-in quota gauge in the title bar renders across every state without
/// panicking — the live `Ok` windows (including an exhausted-window countdown
/// and a stale snapshot) and the terse `Unauthenticated` / `Unavailable`
/// markers — at both a wide width (gauge shown) and a narrow one (gauge yields
/// to the screen title).
#[test]
fn renders_quota_gauge_states() {
    use openspec_app::{ClaudeQuotaState, QuotaStatus, QuotaWindow};

    let svc = service();
    let win = |utilization: u8, resets_at_unix: Option<u64>| QuotaWindow {
        utilization,
        resets_at_unix,
    };
    let states = [
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(62, None)),
            seven_day: Some(win(18, None)),
        },
        // Exhausted 5-hour window → reset countdown; weekly past the warn line.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(100, Some(9_999_999_999))),
            seven_day: Some(win(72, None)),
        },
        // Stale snapshot (de-emphasized), single window present.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: true,
            five_hour: Some(win(95, None)),
            seven_day: None,
        },
        // Time markers present across both widths: far-future reset pins the
        // marker to the first segment, an already-past reset to the last.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(40, Some(9_999_999_999))),
            seven_day: Some(win(55, Some(9_999_999_999))),
        },
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(80, Some(1))),
            seven_day: Some(win(20, Some(1))),
        },
        ClaudeQuotaState {
            status: QuotaStatus::Unauthenticated,
            stale: false,
            five_hour: None,
            seven_day: None,
        },
        ClaudeQuotaState {
            status: QuotaStatus::Unavailable,
            stale: false,
            five_hour: None,
            seven_day: None,
        },
    ];

    for state in states {
        let mut model = Model::new(&svc);
        model.quota = state;
        draw_every_screen(&mut model, 120, 40);
        draw_every_screen(&mut model, 40, 12);
    }
}

/// The Settings screen renders in both on/off states for each toggle, with the
/// row cursor on each row, at a wide and a narrow width.
#[test]
fn renders_settings_screen() {
    let svc = service();
    let mut model = Model::new(&svc);
    model.screen = Screen::Settings;
    for gamification_on in [false, true] {
        for quota_on in [false, true] {
            for cursor in 0..2 {
                model.gamification_on = gamification_on;
                model.quota_on = quota_on;
                model.settings_selected = cursor;
                for (w, h) in [(120, 40), (40, 12)] {
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal.draw(|f| ui::view(f, &model)).unwrap();
                }
            }
        }
    }
}

/// Flipping a toggle on the Settings screen updates the mirrored value and
/// persists it to the shared store; disabling the quota opt-in clears the
/// title-bar gauge at once. The gamification flip re-dispatches the gamified
/// fetches, so this needs a runtime (`tokio::spawn`).
#[tokio::test]
async fn settings_toggles_persist_and_take_effect() {
    use openspec_app::QuotaStatus;

    let svc = service();
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    let press = |model: &mut Model, code: KeyCode| {
        update(
            model,
            Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            &svc,
            &tx,
        );
    };

    // Open Settings (key 6).
    press(&mut model, KeyCode::Char('6'));
    assert!(model.screen == Screen::Settings);

    // Row 0 = gamification: off → on, mirrored and persisted.
    assert!(!model.gamification_on);
    press(&mut model, KeyCode::Char(' '));
    assert!(model.gamification_on);
    assert!(
        svc.settings.gamification_enabled(),
        "gamification toggle persisted to the store"
    );

    // Move to row 1 = quota; enable, then disable. Disabling clears the gauge.
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(model.settings_selected, 1);
    press(&mut model, KeyCode::Char(' '));
    assert!(model.quota_on);
    assert!(svc.settings.claude_quota_enabled());
    press(&mut model, KeyCode::Char(' '));
    assert!(!model.quota_on);
    assert!(!svc.settings.claude_quota_enabled());
    assert!(
        matches!(model.quota.status, QuotaStatus::Disabled),
        "disabling the quota opt-in clears the title-bar gauge"
    );

    // The cursor never leaves the row range.
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(model.settings_selected, 1, "cursor clamps at the last row");
}

/// A toggle flipped on the Settings screen is written to the shared settings
/// file, so a fresh service over the same config dir (a "restart") sees it — and
/// a `Model` opened over that service mirrors the persisted value.
#[tokio::test]
async fn settings_toggle_survives_restart() {
    let dir = tempdir().unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();

    {
        let svc = AppService::bootstrap(dir.path().to_path_buf());
        let mut model = Model::new(&svc);
        // Open Settings and flip gamification (row 0) on.
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE)),
            &svc,
            &tx,
        );
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            &svc,
            &tx,
        );
        assert!(svc.settings.gamification_enabled());
    }

    // "Restart": a new service over the same config dir loads the persisted value.
    let svc2 = AppService::bootstrap(dir.path().to_path_buf());
    assert!(
        svc2.settings.gamification_enabled(),
        "the toggle persisted across a restart"
    );
    assert!(
        Model::new(&svc2).gamification_on,
        "the panel mirrors the persisted value when reopened"
    );
}
