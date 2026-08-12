//! Headless render smoke tests. The interactive TUI can't be exercised against
//! a real terminal in CI, but `ui::view` is a pure function of `Model`, so we
//! can drive it through ratatui's `TestBackend` and assert it never panics —
//! the real risk in the index-heavy widget code (graph elbows, the heatmap
//! grid, the 30-tier ladder's auto-scroll math).

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::AppService;
use openspec_core::{CommitGraph, EdgeSegment, LaidOutCommit, PaletteColor};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tempfile::{tempdir, TempDir};
use tokio::sync::mpsc;

use crate::app::{update, ConfirmAction, Model, Msg, Overlay, PromptKind, Screen};
use crate::{graph, theme, ui};

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

/// The Browse tree pane is width-capped: it tracks the terminal width but stays
/// within [28, 44] columns, so on wide terminals the surplus goes to the detail
/// pane instead of the tree growing without bound.
#[test]
fn browse_tree_pane_width_is_capped() {
    // Floors at the two-pane threshold, reaches and holds the cap when wide.
    assert_eq!(ui::tree_pane_width(90), 28, "floors at the narrow end");
    assert_eq!(ui::tree_pane_width(140), 44, "reaches the cap");
    assert_eq!(ui::tree_pane_width(220), 44, "stays capped when wide");

    // The cap means the detail pane takes the surplus on a wide terminal.
    let wide = 220u16;
    assert!(
        wide - ui::tree_pane_width(wide) >= 170,
        "detail pane gets the surplus width on a wide terminal"
    );

    // Never below the floor, never above the cap, across the two-pane range
    // (the layout is single-pane below 90, so that's where the sweep starts).
    for w in 90..=400u16 {
        let t = ui::tree_pane_width(w);
        assert!(
            (28..=44).contains(&t),
            "width {w} -> tree {t} out of bounds"
        );
    }

    // And Browse actually renders at those representative widths.
    let svc = service();
    let mut model = Model::new(&svc);
    model.screen = Screen::Browse;
    for (w, h) in [(90, 30), (140, 40), (220, 50)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::view(f, &model)).unwrap();
    }
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
    use openspec_app::{ClaudeQuotaState, QuotaStatus, QuotaWindow, ScopedQuotaWindow};

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
            scoped: vec![],
        },
        // Exhausted 5-hour window → reset countdown; weekly past the warn line.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(100, Some(9_999_999_999))),
            seven_day: Some(win(72, None)),
            scoped: vec![],
        },
        // Stale snapshot (de-emphasized), single window present.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: true,
            five_hour: Some(win(95, None)),
            seven_day: None,
            scoped: vec![],
        },
        // Time markers present across both widths: far-future reset pins the
        // marker to the first segment, an already-past reset to the last.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(40, Some(9_999_999_999))),
            seven_day: Some(win(55, Some(9_999_999_999))),
            scoped: vec![],
        },
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(80, Some(1))),
            seven_day: Some(win(20, Some(1))),
            scoped: vec![],
        },
        // A per-model scoped weekly window (Fable) appended after the pooled
        // windows — the third gauge exercises the scoped render path.
        ClaudeQuotaState {
            status: QuotaStatus::Ok,
            stale: false,
            five_hour: Some(win(5, Some(9_999_999_999))),
            seven_day: Some(win(39, Some(9_999_999_999))),
            scoped: vec![ScopedQuotaWindow {
                model: "Fable".to_string(),
                utilization: 59,
                resets_at_unix: Some(9_999_999_999),
            }],
        },
        ClaudeQuotaState {
            status: QuotaStatus::Unauthenticated,
            stale: false,
            five_hour: None,
            seven_day: None,
            scoped: vec![],
        },
        ClaudeQuotaState {
            status: QuotaStatus::Unavailable,
            stale: false,
            five_hour: None,
            seven_day: None,
            scoped: vec![],
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
            for chatgpt_quota_on in [false, true] {
                // 0, 1, 2 = toggles; 3 = Appearance; 4 = the add-workspace
                // action row.
                for cursor in 0..5 {
                    model.gamification_on = gamification_on;
                    model.quota_on = quota_on;
                    model.chatgpt_quota_on = chatgpt_quota_on;
                    model.settings_selected = cursor;
                    for (w, h) in [(120, 40), (40, 12)] {
                        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                        terminal.draw(|f| ui::view(f, &model)).unwrap();
                    }
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

    // Move to row 1 = Claude quota; enable, then disable. Disabling clears
    // the gauge.
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

    // Move to row 2 = ChatGPT quota; same enable/disable/gauge-clear
    // behavior as the Claude row, toggled independently.
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(model.settings_selected, 2);
    press(&mut model, KeyCode::Char(' '));
    assert!(model.chatgpt_quota_on);
    assert!(svc.settings.chatgpt_quota_enabled());
    press(&mut model, KeyCode::Char(' '));
    assert!(!model.chatgpt_quota_on);
    assert!(!svc.settings.chatgpt_quota_enabled());
    assert!(
        matches!(model.chatgpt_quota.status, QuotaStatus::Disabled),
        "disabling the ChatGPT quota opt-in clears its title-bar gauge"
    );

    // Past the toggles the cursor steps onto the Appearance row, then the
    // add-workspace row (the last row — no workspaces registered), then clamps.
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(
        model.settings_selected, 3,
        "cursor reaches the Appearance row"
    );
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(
        model.settings_selected, 4,
        "cursor reaches the add-workspace row"
    );
    press(&mut model, KeyCode::Char('j'));
    assert_eq!(model.settings_selected, 4, "cursor clamps at the last row");
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

/// The Settings Appearance row cycles the active colour scheme on Space, applies
/// it live (the renderer reads the active scheme each frame), and persists it to
/// the TUI preference file so it survives a restart.
#[tokio::test]
async fn settings_appearance_cycles_and_persists_scheme() {
    let dir = tempdir().unwrap();
    let svc = AppService::bootstrap(dir.path().to_path_buf());
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);
    model.config_dir = Some(dir.path().to_path_buf());

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    model.settings_selected = 3; // the Appearance row

    let before = theme::theme().active_scheme();
    key(&mut model, &svc, &tx, KeyCode::Char(' '));
    let after = theme::theme().active_scheme();
    assert_ne!(before, after, "Space cycles to the next scheme");
    assert_eq!(
        crate::prefs::load_scheme(dir.path()),
        Some(after),
        "the chosen scheme is persisted and reloadable"
    );

    // Restore the global so a non-default scheme doesn't leak into other tests.
    theme::set_scheme(theme::Scheme::Default);
}

/// Create a flat OpenSpec workspace (a folder with `openspec/changes/`) under a
/// tempdir — what registration requires — and return its path.
fn make_workspace(tmp: &TempDir, name: &str) -> PathBuf {
    let root = tmp.path().join(name);
    fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
    root
}

/// Dispatch one key press through `update`, as the event loop would.
fn key(model: &mut Model, svc: &AppService, tx: &mpsc::UnboundedSender<Msg>, code: KeyCode) {
    update(
        model,
        Msg::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        svc,
        tx,
    );
}

/// Draw one frame at the given size and return it as text, one terminal row per
/// line — so a test can assert on what the user actually reads, markers and
/// footer hints included, rather than on the `Model` behind it.
fn frame_text(model: &Model, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::view(f, model)).unwrap();
    let buf = terminal.backend().buffer();
    buf.content
        .chunks(buf.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run a git command in `cwd`, asserting it succeeded.
fn git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git")
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

/// A git repo carrying an `openspec/changes/` tree and one commit; returns its
/// canonical root.
fn init_openspec_repo(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("openspec").join("changes")).unwrap();
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    git(&["commit", "--allow-empty", "-m", "init"], root);
    openspec_core::canonicalize(root).unwrap()
}

/// The Settings Workspaces section renders a registered workspace (with a colour
/// swatch and display name) at the cursor, and each modal overlay — the add and
/// rename prompts (including an inline error) and the remove confirm — draws over
/// it without panicking, at a wide and a narrow width. The parked variant of the
/// row, which carries an extra marker, is drawn at both widths too.
#[tokio::test]
async fn renders_settings_workspaces_and_overlays() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    let folder = make_workspace(&ws, "acme");
    svc.add_workspace(folder).await.expect("register");
    let listed = svc.list_workspaces().unwrap();
    svc.set_workspace_presentation(
        listed[0].uri.clone(),
        listed[0].repo_id.clone(),
        Some("Acme".to_string()),
        Some(PaletteColor::Teal),
    )
    .expect("set presentation");

    let mut model = Model::new(&svc);
    model.screen = Screen::Settings;
    assert_eq!(model.settings_workspaces.len(), 1);

    // Cursor on the add row (4) and the workspace row (5).
    for cursor in [4usize, 5] {
        model.settings_selected = cursor;
        for (w, h) in [(120, 40), (40, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| ui::view(f, &model)).unwrap();
        }
    }

    let overlays = [
        Overlay::Prompt {
            kind: PromptKind::AddWorkspace,
            title: "Add workspace".to_string(),
            input: "/tmp/not-a-ws".to_string(),
            error: Some("not an OpenSpec workspace (no `openspec/` subdirectory)".to_string()),
        },
        Overlay::Prompt {
            kind: PromptKind::RenameWorkspace {
                uri: PathBuf::from("/x"),
                repo_id: None,
                color: Some(PaletteColor::Teal),
            },
            title: "Rename acme".to_string(),
            input: "Acme".to_string(),
            error: None,
        },
        Overlay::Confirm {
            title: "Remove workspace".to_string(),
            message: "Remove \u{201c}Acme\u{201d}?".to_string(),
            action: ConfirmAction::RemoveWorkspace {
                uri: PathBuf::from("/x"),
            },
        },
    ];
    for ov in overlays {
        model.overlay = Some(ov);
        for (w, h) in [(120, 40), (40, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| ui::view(f, &model)).unwrap();
        }
    }

    // Parked, the row keeps its place in the list and gains a marker — the
    // narrow render has to absorb it alongside the swatch, name and path.
    model.overlay = None;
    svc.set_workspace_disabled(listed[0].uri.clone(), listed[0].repo_id.clone(), true)
        .await
        .expect("park");
    model.refresh_settings_workspaces(&svc);
    assert!(model.settings_workspaces[0].disabled);
    for (w, h) in [(120, 40), (40, 12)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::view(f, &model)).unwrap();
    }
}

/// Space on a workspace row parks it and pressing it again brings it back: the
/// flag reaches the presentation store, the Settings mirror re-reads it, and the
/// row leaves (and rejoins) the Browse tree — the terminal's half of the
/// workspace-disable feature, driven entirely through the keys and the async
/// nudge the real event loop delivers.
#[tokio::test]
async fn settings_space_parks_and_unparks_a_workspace_via_keys() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    svc.add_workspace(make_workspace(&ws, "acme"))
        .await
        .expect("register");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    model.settings_selected = 5; // the workspace row
    assert!(!model.settings_workspaces[0].disabled);
    assert_eq!(headers(&model), 1, "the row starts in the tree");
    assert_eq!(model.disabled_row_count, 0);

    key(&mut model, &svc, &tx, KeyCode::Char(' '));
    let msg = rx.recv().await.expect("the park nudge");
    update(&mut model, msg, &svc, &tx);

    assert!(
        svc.list_workspaces().unwrap()[0].disabled,
        "the flag went through the service to the presentation store"
    );
    assert!(
        model.settings_workspaces[0].disabled,
        "the Settings mirror re-read it"
    );
    assert_eq!(headers(&model), 0, "its top-level row left the tree");
    assert_eq!(model.disabled_row_count, 1);
    assert!(
        !model.status.contains("Could not save"),
        "a park that persisted reports nothing: {}",
        model.status
    );

    // The same key is the way back — a one-way toggle would strand the user.
    key(&mut model, &svc, &tx, KeyCode::Char(' '));
    let msg = rx.recv().await.expect("the un-park nudge");
    update(&mut model, msg, &svc, &tx);

    assert!(!svc.list_workspaces().unwrap()[0].disabled);
    assert!(!model.settings_workspaces[0].disabled);
    assert_eq!(headers(&model), 1, "the row is back in the tree");
    assert_eq!(model.disabled_row_count, 0);
}

/// Top-level (workspace) rows currently in the Browse tree.
fn headers(model: &Model) -> usize {
    model.rows.iter().filter(|r| r.is_header).count()
}

/// A park that cannot be written says so on screen. The terminal frontend has no
/// console the user can see and the alternate screen swallows stderr, so the
/// status line is the only channel there is — and the Settings mirror cannot
/// stand in for it, because the presentation store flips its in-memory entry
/// before `save()` runs and `list_workspaces` serves that same map. Without the
/// report the row would claim to be parked, nothing would be on disk, and the
/// flag would be gone at the next launch with no explanation.
#[tokio::test]
async fn a_park_that_cannot_be_persisted_is_reported_in_the_status_line() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    svc.add_workspace(make_workspace(&ws, "acme"))
        .await
        .expect("register");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    model.settings_selected = 5; // the workspace row

    // Make the store unwritable the way a read-only config dir or a full disk
    // would: `save()`'s `fs::write` cannot overwrite a directory. The store was
    // loaded at bootstrap, when this path did not exist, so only the write
    // fails — exactly the shape of the real failure.
    let store_path = cfg.path().join("presentation.json");
    fs::create_dir(&store_path).unwrap();

    key(&mut model, &svc, &tx, KeyCode::Char(' '));
    update(
        &mut model,
        rx.recv().await.expect("the park result"),
        &svc,
        &tx,
    );

    assert!(
        model.status.starts_with("Could not save: "),
        "the failed persist is reported like every other Settings-row write: {}",
        model.status
    );
    let frame = frame_text(&model, 160, 40);
    assert!(
        frame.contains("Could not save"),
        "and it reaches the screen, not just the model: {frame}"
    );

    // The row keeps showing the STORED state, not the attempted one
    // (`terminal-ui`: *A disable that cannot be persisted is reported, not
    // swallowed*). `WorkspacePresentationStore` rolls its in-memory entry back
    // when `save` fails, so the mirror and the file agree — which is precisely
    // why the status line has to carry the failure: nothing about the row
    // itself now looks any different from the user never having pressed the
    // key.
    assert!(
        !model.settings_workspaces[0].disabled,
        "the mirror must report the stored state, not the attempted one"
    );
    assert!(
        !store_path.is_file(),
        "nothing was persisted, so there is no park to survive a restart"
    );
}

/// A parked row is legible as parked from the Settings screen, and the key that
/// parks it is advertised in the footer — without both, a terminal-only user
/// watches a workspace vanish from the tree with no marker and no way back.
#[tokio::test]
async fn parked_workspace_row_is_marked_and_the_key_is_advertised() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    svc.add_workspace(make_workspace(&ws, "acme"))
        .await
        .expect("register");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    model.settings_selected = 5; // the workspace row

    let before = frame_text(&model, 160, 40);
    assert!(
        !before.contains("(disabled)"),
        "an enabled row carries no marker"
    );
    assert!(
        before.contains("Space on/off"),
        "the footer advertises the toggle key on a workspace row: {before}"
    );

    key(&mut model, &svc, &tx, KeyCode::Char(' '));
    update(
        &mut model,
        rx.recv().await.expect("the park nudge"),
        &svc,
        &tx,
    );

    let after = frame_text(&model, 160, 40);
    assert!(
        after.contains("(disabled)"),
        "a parked row is marked in the Settings list: {after}"
    );

    // And the key overlay names it too, so it is reachable without the footer.
    key(&mut model, &svc, &tx, KeyCode::Char('?'));
    let help = frame_text(&model, 160, 40);
    assert!(
        help.contains("toggle a setting or a workspace"),
        "the help overlay covers the workspace toggle: {help}"
    );
}

/// The TUI Dashboard reads the unfiltered record while the tree hides parked
/// rows, so its totals legitimately exceed what the tree reaches. It says so —
/// and only when something is actually parked.
#[tokio::test]
async fn dashboard_notes_disabled_workspaces() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    svc.add_workspace(make_workspace(&ws, "alpha"))
        .await
        .expect("register alpha");
    svc.add_workspace(make_workspace(&ws, "beta"))
        .await
        .expect("register beta");

    let mut model = Model::new(&svc);
    model.screen = Screen::Dashboard;
    model.dashboard = Some(svc.dashboard().await.expect("assemble dashboard"));
    assert!(
        !frame_text(&model, 160, 40).contains("disabled workspace"),
        "with nothing parked there is no discrepancy to explain"
    );

    let listed = svc.list_workspaces().unwrap();
    svc.set_workspace_disabled(listed[0].uri.clone(), listed[0].repo_id.clone(), true)
        .await
        .expect("park alpha");
    model.refresh(&svc);
    let one = frame_text(&model, 160, 40);
    assert!(one.contains("includes 1 disabled workspace"), "{one}");
    assert!(
        !one.contains("disabled workspaces"),
        "one parked row reads as singular: {one}"
    );

    svc.set_workspace_disabled(listed[1].uri.clone(), listed[1].repo_id.clone(), true)
        .await
        .expect("park beta");
    model.refresh(&svc);
    let two = frame_text(&model, 160, 40);
    assert!(two.contains("includes 2 disabled workspaces"), "{two}");

    // The number is defined as the top-level rows the tree lost, so it stays
    // equal to that difference however the rows are keyed.
    assert_eq!(
        model.disabled_row_count,
        svc.watcher.workspace_views().len() - headers(&model),
        "the footnote counts the top-level rows the tree lost"
    );
}

/// One repository registered at two worktrees is *one* top-level row, so parking
/// it costs the tree one row and the footnote must say one — counting registered
/// entries carrying the flag would say two.
#[tokio::test]
async fn two_worktrees_of_one_repository_count_as_one_disabled_row() {
    let cfg = tempdir().unwrap();
    let roots = tempdir().unwrap();
    let main = init_openspec_repo(&roots.path().join("main"));
    let sibling = roots.path().join("sibling");
    git(
        &[
            "worktree",
            "add",
            "-b",
            "feature",
            sibling.to_str().unwrap(),
        ],
        &main,
    );
    fs::create_dir_all(sibling.join("openspec").join("changes")).unwrap();
    let sibling = openspec_core::canonicalize(&sibling).unwrap();

    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    let registered = svc
        .add_workspace(main.clone())
        .await
        .expect("register main");
    // Both worktrees user-registered, so Settings lists two rows for one repo.
    // Registered through the registry directly because `add_workspace` cannot
    // promote a worktree the first registration already discovered — the same
    // route `openspec-app`'s own sibling-worktree test takes.
    svc.registry.lock().unwrap().register(sibling).unwrap();
    svc.watcher.sync_repos();
    svc.watcher.aggregate_and_emit();

    let mut model = Model::new(&svc);
    assert_eq!(
        model.settings_workspaces.len(),
        2,
        "two registered entries for one repository"
    );
    assert_eq!(headers(&model), 1, "which the tree shows as one row");

    svc.set_workspace_disabled(main, registered.repo_id.clone(), true)
        .await
        .expect("park the repository");
    model.refresh(&svc);
    model.refresh_settings_workspaces(&svc);

    assert!(
        model.settings_workspaces.iter().all(|w| w.disabled),
        "both entries report the group's single state"
    );
    assert_eq!(headers(&model), 0, "the whole group left the tree");
    assert_eq!(
        model.disabled_row_count, 1,
        "one row lost, not one per registered worktree"
    );

    model.screen = Screen::Dashboard;
    model.dashboard = Some(svc.dashboard().await.expect("assemble dashboard"));
    let text = frame_text(&model, 160, 40);
    assert!(text.contains("includes 1 disabled workspace"), "{text}");
    assert!(!text.contains("disabled workspaces"), "{text}");
}

/// Driving the keys end to end: open Settings, open the add prompt, type a valid
/// folder path, submit → the async add registers it; then select the row, press
/// `x`, confirm with `y` → the async remove unregisters it. The mirrored Settings
/// list reflects both, matching the spec's add/remove scenarios.
#[tokio::test]
async fn settings_add_then_remove_workspace_via_keys() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let folder = make_workspace(&ws, "acme");
    let path = folder.to_str().unwrap().to_string();

    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    assert!(model.screen == Screen::Settings);
    key(&mut model, &svc, &tx, KeyCode::Char('a'));
    assert!(model.overlay.is_some(), "add prompt opens");

    for c in path.chars() {
        key(&mut model, &svc, &tx, KeyCode::Char(c));
    }
    key(&mut model, &svc, &tx, KeyCode::Enter);

    // The async add posts its result back through the channel.
    let msg = rx.recv().await.expect("add result");
    update(&mut model, msg, &svc, &tx);
    assert!(
        model.overlay.is_none(),
        "a successful add closes the prompt"
    );
    assert_eq!(
        model.settings_workspaces.len(),
        1,
        "the workspace registered"
    );

    // Select the workspace row (index 5 = 3 toggles + Appearance + add + ws).
    model.settings_selected = 5;
    key(&mut model, &svc, &tx, KeyCode::Char('x'));
    assert!(matches!(model.overlay, Some(Overlay::Confirm { .. })));
    key(&mut model, &svc, &tx, KeyCode::Char('y'));
    assert!(model.overlay.is_none(), "confirming closes the modal");

    let msg = rx.recv().await.expect("remove refresh nudge");
    update(&mut model, msg, &svc, &tx);
    assert!(
        model.settings_workspaces.is_empty(),
        "the workspace was unregistered"
    );
}

/// An invalid path keeps the add prompt open with an inline error and registers
/// nothing — the spec's invalid-path-rejected scenario.
#[tokio::test]
async fn settings_add_rejects_invalid_path() {
    let cfg = tempdir().unwrap();
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    key(&mut model, &svc, &tx, KeyCode::Char('a'));
    for c in "/no/such/openspec/here".chars() {
        key(&mut model, &svc, &tx, KeyCode::Char(c));
    }
    key(&mut model, &svc, &tx, KeyCode::Enter);

    let msg = rx.recv().await.expect("add result");
    update(&mut model, msg, &svc, &tx);
    match &model.overlay {
        Some(Overlay::Prompt { error, .. }) => {
            assert!(error.is_some(), "an invalid path shows an inline error")
        }
        _ => panic!("the add prompt should stay open after an invalid path"),
    }
    assert!(
        model.settings_workspaces.is_empty(),
        "nothing is registered on an invalid path"
    );
}

/// Renaming a workspace (and clearing the name back to default) and cycling its
/// colour persist immediately to the presentation store and update the mirror —
/// the spec's rename and colour scenarios. Clearing the name preserves the
/// colour.
#[tokio::test]
async fn settings_rename_and_color_workspace_via_keys() {
    let cfg = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let folder = make_workspace(&ws, "acme");
    let svc = AppService::bootstrap(cfg.path().to_path_buf());
    svc.add_workspace(folder).await.expect("register");
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut model = Model::new(&svc);

    key(&mut model, &svc, &tx, KeyCode::Char('6'));
    model.settings_selected = 5; // the workspace row

    // Rename → "Renamed".
    key(&mut model, &svc, &tx, KeyCode::Char('r'));
    for c in "Renamed".chars() {
        key(&mut model, &svc, &tx, KeyCode::Char(c));
    }
    key(&mut model, &svc, &tx, KeyCode::Enter);
    assert!(
        model.overlay.is_none(),
        "rename applies synchronously and closes"
    );
    assert_eq!(
        svc.list_workspaces().unwrap()[0].display_name.as_deref(),
        Some("Renamed")
    );
    assert_eq!(
        model.settings_workspaces[0].display_name.as_deref(),
        Some("Renamed")
    );

    // Colour cycle: none → indigo.
    key(&mut model, &svc, &tx, KeyCode::Char('c'));
    assert_eq!(
        svc.list_workspaces().unwrap()[0].color,
        Some(PaletteColor::Indigo)
    );
    assert_eq!(
        model.settings_workspaces[0].color,
        Some(PaletteColor::Indigo)
    );

    // Clear the name (empty rename) → reverts to default; colour is preserved.
    key(&mut model, &svc, &tx, KeyCode::Char('r'));
    for _ in 0.."Renamed".len() {
        key(&mut model, &svc, &tx, KeyCode::Backspace);
    }
    key(&mut model, &svc, &tx, KeyCode::Enter);
    let listed = svc.list_workspaces().unwrap();
    assert!(
        listed[0].display_name.is_none(),
        "an empty rename clears to the default name"
    );
    assert_eq!(
        listed[0].color,
        Some(PaletteColor::Indigo),
        "clearing the name keeps the colour"
    );
}
