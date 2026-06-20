//! The Elm-style core: `Model`, `Msg`, and `update`. The view is in `ui`.
//!
//! `update` owns all state transitions and, for anything asynchronous (loading
//! an artifact, assembling the dashboard, mining a commit graph), spawns a task
//! that posts a `Msg` back through the channel — so the render loop never
//! blocks. All payloads are the headless core's own typed structs (no parallel
//! cache, no JSON scraping); the TUI links `openspec_core` directly.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::{AppService, ClaudeQuotaState};
use openspec_core::{
    ArtifactStatus, CommitGraph, DashboardData, PaletteColor, WorkspaceGarden, WorkspaceOrigin,
    WorkspaceView,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{prefs, theme};

/// Default commit-graph window; bumped by `m` when more history exists.
const GRAPH_PAGE: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Dashboard,
    Season,
    Garden,
    History,
    Settings,
}

/// The two toggle rows the Settings screen leads with, in display order. They
/// occupy cursor indices `0..SETTINGS_TOGGLE_COUNT`; the add-workspace action
/// and the per-workspace rows follow, so the cursor's upper bound is dynamic
/// (see [`settings_row_count`]).
pub const SETTINGS_TOGGLE_COUNT: usize = 2;

/// A user-registered workspace as the Settings screen manages it. Mirrored onto
/// `Model` (the view is a pure function of `Model` and never sees the service)
/// and rebuilt from `AppService::list_workspaces` whenever the registry changes.
pub struct SettingsWorkspace {
    pub uri: PathBuf,
    /// The default name (folder basename), shown when no display name is set.
    pub name: String,
    pub display_name: Option<String>,
    pub color: Option<PaletteColor>,
    pub repo_id: Option<PathBuf>,
    pub is_missing: bool,
}

/// A modal overlay on the Settings screen: a text prompt (add / rename) or a
/// yes/no confirm (remove). While one is open it swallows all key input.
pub enum Overlay {
    Prompt {
        kind: PromptKind,
        title: String,
        input: String,
        error: Option<String>,
    },
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
    },
}

/// Which text-prompt flow an [`Overlay::Prompt`] is driving.
pub enum PromptKind {
    AddWorkspace,
    RenameWorkspace {
        uri: PathBuf,
        repo_id: Option<PathBuf>,
        /// The workspace's current colour, preserved across a rename so setting
        /// a name doesn't clear the swatch.
        color: Option<PaletteColor>,
    },
}

/// The action a confirm overlay commits on `y`/Enter.
pub enum ConfirmAction {
    RemoveWorkspace { uri: PathBuf },
}

/// What the Settings cursor currently points at, derived from its index: the
/// two toggles, then the add-workspace action, then one row per workspace.
pub enum SettingsRow {
    Toggle,
    /// The colour-scheme picker row.
    Appearance,
    AddWorkspace,
    Workspace(usize),
}

/// Total selectable rows on the Settings screen.
pub fn settings_row_count(model: &Model) -> usize {
    // toggles + the Appearance row + the add action + one row per workspace.
    SETTINGS_TOGGLE_COUNT + 2 + model.settings_workspaces.len()
}

/// Map a Settings cursor index onto the row it addresses.
pub fn settings_row_at(idx: usize) -> SettingsRow {
    if idx < SETTINGS_TOGGLE_COUNT {
        SettingsRow::Toggle
    } else if idx == SETTINGS_TOGGLE_COUNT {
        SettingsRow::Appearance
    } else if idx == SETTINGS_TOGGLE_COUNT + 1 {
        SettingsRow::AddWorkspace
    } else {
        SettingsRow::Workspace(idx - SETTINGS_TOGGLE_COUNT - 2)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Detail,
}

/// One artifact tab in the Browse detail pane. Only tabs whose file exists for
/// the selected change are shown.
#[derive(Clone, PartialEq, Eq)]
pub enum ArtifactTab {
    Proposal,
    Design,
    Tasks,
    Spec(String),
}

impl ArtifactTab {
    /// Short label for the tab strip.
    pub fn label(&self) -> String {
        match self {
            ArtifactTab::Proposal => "proposal".to_string(),
            ArtifactTab::Design => "design".to_string(),
            ArtifactTab::Tasks => "tasks".to_string(),
            ArtifactTab::Spec(c) => format!("spec:{c}"),
        }
    }

    /// The on-disk filename, for the pane title.
    pub fn filename(&self) -> String {
        match self {
            ArtifactTab::Proposal => "proposal.md".to_string(),
            ArtifactTab::Design => "design.md".to_string(),
            ArtifactTab::Tasks => "tasks.md".to_string(),
            ArtifactTab::Spec(c) => format!("specs/{c}/spec.md"),
        }
    }

    /// The `(kind, capability)` pair `AppService::read_artifact` expects — note
    /// the singular `"spec"` at the service boundary.
    fn read_target(&self) -> (&'static str, Option<String>) {
        match self {
            ArtifactTab::Proposal => ("proposal", None),
            ArtifactTab::Design => ("design", None),
            ArtifactTab::Tasks => ("tasks", None),
            ArtifactTab::Spec(c) => ("spec", Some(c.clone())),
        }
    }
}

/// Build the present-only tab list for a change, in a stable reading order.
fn tabs_for(a: &ArtifactStatus) -> Vec<ArtifactTab> {
    let mut tabs = Vec::new();
    if a.proposal {
        tabs.push(ArtifactTab::Proposal);
    }
    if a.design {
        tabs.push(ArtifactTab::Design);
    }
    if a.tasks {
        tabs.push(ArtifactTab::Tasks);
    }
    for cap in &a.specs {
        tabs.push(ArtifactTab::Spec(cap.clone()));
    }
    tabs
}

/// Messages that drive `update`. Async results arrive as the lower variants.
pub enum Msg {
    Key(KeyEvent),
    Resize,
    Cache,
    Tick,
    Artifact {
        gen: u64,
        title: String,
        body: String,
    },
    Dashboard(Box<DashboardData>),
    Garden(Vec<WorkspaceGarden>),
    Graph(Box<CommitGraph>),
    /// The quota poller refreshed its snapshot; re-read it from the service.
    Quota,
    /// An async add-workspace finished: `Ok` closes the prompt and refreshes the
    /// Settings list; `Err` shows the validation message inline in the prompt.
    AddResult(Result<(), String>),
}

/// One flattened tree row: a workspace header or a change beneath it.
pub struct TreeRow {
    pub depth: u8,
    pub label: String,
    pub progress: Option<(usize, usize)>,
    /// `(workspace_uri, change_id)` when this row is a loadable change.
    pub change: Option<(PathBuf, String)>,
    pub is_header: bool,
    /// Header tint from the presentation store.
    pub color: Option<PaletteColor>,
    /// The repo identity to mine history from (set on header and change rows).
    pub repo: Option<PathBuf>,
    /// Which artifacts the change has, for the detail tab bar.
    pub artifacts: Option<ArtifactStatus>,
}

pub struct Model {
    pub screen: Screen,
    pub focus: Focus,
    /// The full flattened tree; `visible` holds the indices passing the filter.
    pub rows: Vec<TreeRow>,
    pub visible: Vec<usize>,
    /// Selection index into `visible`.
    pub selected: usize,
    /// `Some` while a `/` filter is active; `filter_editing` is the typing state.
    pub filter: Option<String>,
    pub filter_editing: bool,

    pub tabs: Vec<ArtifactTab>,
    pub active_tab: usize,
    pub detail_title: String,
    pub detail_md: String,
    pub detail_scroll: u16,
    /// Monotonic token so a slow artifact read can't clobber a newer selection.
    pub artifact_gen: u64,

    pub dashboard: Option<DashboardData>,
    pub dash_scroll: u16,
    /// Signed nudge around the ladder's auto-centred current tier (Season only).
    pub season_scroll: i32,

    pub garden: Option<Vec<WorkspaceGarden>>,
    pub garden_scroll: u16,

    pub graph: Option<CommitGraph>,
    pub graph_repo: Option<PathBuf>,
    pub graph_selected: usize,
    pub graph_limit: usize,

    pub status: String,
    /// Latest opt-in Claude usage-quota snapshot, rendered in the title bar.
    /// `Disabled` until the poller runs with the feature enabled.
    pub quota: ClaudeQuotaState,

    /// Row cursor on the Settings screen (`0..settings_row_count`).
    pub settings_selected: usize,
    /// The Settings screen renders from `Model` alone (the view never sees the
    /// service), so the two toggles are mirrored here. Re-read from the store
    /// whenever the screen is opened and after each flip, so the panel always
    /// shows what was last written.
    pub gamification_on: bool,
    pub quota_on: bool,
    /// The user-registered workspaces the Settings screen manages, mirrored from
    /// the service. Rebuilt when the screen is opened, on each registry change,
    /// and after a rename/recolour.
    pub settings_workspaces: Vec<SettingsWorkspace>,
    /// The active Settings modal (add / rename prompt or remove confirm), if any.
    /// While `Some`, all key input is routed to it.
    pub overlay: Option<Overlay>,

    /// The SpecForge config directory, used to persist the TUI colour scheme.
    /// `None` in tests that bootstrap a `Model` without a real config dir.
    pub config_dir: Option<PathBuf>,

    pub show_help: bool,
    pub should_quit: bool,
}

impl Model {
    pub fn new(svc: &AppService) -> Self {
        let mut m = Model {
            screen: Screen::Browse,
            focus: Focus::Tree,
            rows: Vec::new(),
            visible: Vec::new(),
            selected: 0,
            filter: None,
            filter_editing: false,
            tabs: Vec::new(),
            active_tab: 0,
            detail_title: String::new(),
            detail_md: "Select a change to read its proposal.".to_string(),
            detail_scroll: 0,
            artifact_gen: 0,
            dashboard: None,
            dash_scroll: 0,
            season_scroll: 0,
            garden: None,
            garden_scroll: 0,
            graph: None,
            graph_repo: None,
            graph_selected: 0,
            graph_limit: GRAPH_PAGE,
            status: String::new(),
            quota: svc.claude_quota(),
            settings_selected: 0,
            gamification_on: svc.settings.gamification_enabled(),
            quota_on: svc.settings.claude_quota_enabled(),
            settings_workspaces: Vec::new(),
            overlay: None,
            config_dir: None,
            show_help: false,
            should_quit: false,
        };
        m.refresh(svc);
        m.refresh_settings_workspaces(svc);
        m
    }

    /// Re-read the user-registered workspace list into the Settings mirror and
    /// clamp the row cursor. Cheap (one registry + presentation read), so it runs
    /// whenever the registry may have changed.
    pub fn refresh_settings_workspaces(&mut self, svc: &AppService) {
        self.settings_workspaces = svc
            .list_workspaces()
            .unwrap_or_default()
            .into_iter()
            .map(|w| SettingsWorkspace {
                uri: w.uri,
                name: w.name,
                display_name: w.display_name,
                color: w.color,
                repo_id: w.repo_id,
                is_missing: w.is_missing,
            })
            .collect();
        let max = settings_row_count(self).saturating_sub(1);
        if self.settings_selected > max {
            self.settings_selected = max;
        }
    }

    /// Re-read the aggregated view from the service (never a parallel cache).
    /// Selection follows the previously-selected *change* by identity, so a
    /// watcher refresh that reorders/inserts rows doesn't silently point the
    /// highlight (and detail pane) at a different change.
    pub fn refresh(&mut self, svc: &AppService) {
        let prev = self.selected_change();
        self.rows = flatten(&svc.workspace_views());
        self.recompute_visible();
        if let Some(prev) = prev {
            if let Some(vi) = self
                .visible
                .iter()
                .position(|&i| self.rows[i].change.as_ref() == Some(&prev))
            {
                self.selected = vi;
            }
        }
        let active = svc.active_count();
        let ws = svc.list_workspaces().map(|w| w.len()).unwrap_or(0);
        self.status = format!("{ws} workspaces · {active} open changes");
    }

    /// Rebuild `visible` from `rows` + the current filter and clamp `selected`.
    fn recompute_visible(&mut self) {
        self.visible = compute_visible(&self.rows, &self.filter);
        if self.selected >= self.visible.len() {
            self.selected = self.visible.len().saturating_sub(1);
        }
    }

    pub fn selected_row(&self) -> Option<&TreeRow> {
        self.visible
            .get(self.selected)
            .and_then(|&i| self.rows.get(i))
    }

    pub fn selected_change(&self) -> Option<(PathBuf, String)> {
        self.selected_row().and_then(|r| r.change.clone())
    }
}

/// Flatten the repo/flat views into indented rows the tree pane renders.
pub fn flatten(views: &[WorkspaceView]) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    for view in views {
        match view {
            WorkspaceView::Repo(r) => {
                let name = r.display_name.clone().unwrap_or_else(|| r.name.clone());
                rows.push(TreeRow {
                    depth: 0,
                    label: format!("{name}  ({} active)", r.active.len()),
                    progress: None,
                    change: None,
                    is_header: true,
                    color: r.color,
                    repo: Some(r.repo_id.clone()),
                    artifacts: None,
                });
                for lc in &r.active {
                    let inst = lc
                        .instances
                        .iter()
                        .find(|i| i.is_main_worktree)
                        .or_else(|| lc.instances.first());
                    if let Some(inst) = inst {
                        let cd = &inst.change;
                        rows.push(TreeRow {
                            depth: 1,
                            label: cd.title.clone().unwrap_or_else(|| lc.name.clone()),
                            progress: Some((cd.completed_tasks, cd.total_tasks)),
                            change: Some((cd.workspace.uri.clone(), cd.change_id.clone())),
                            is_header: false,
                            color: None,
                            repo: Some(r.repo_id.clone()),
                            artifacts: Some(cd.artifacts.clone()),
                        });
                    }
                }
            }
            WorkspaceView::Flat {
                workspace,
                changes,
                display_name,
                color,
            } => {
                let name = display_name
                    .clone()
                    .unwrap_or_else(|| workspace.name.clone());
                rows.push(TreeRow {
                    depth: 0,
                    label: format!("{name}  ({} active)", changes.len()),
                    progress: None,
                    change: None,
                    is_header: true,
                    color: *color,
                    // A Flat workspace is the non-git case — no repo to mine.
                    repo: None,
                    artifacts: None,
                });
                for cd in changes {
                    rows.push(TreeRow {
                        depth: 1,
                        label: cd.title.clone().unwrap_or_else(|| cd.change_id.clone()),
                        progress: Some((cd.completed_tasks, cd.total_tasks)),
                        change: Some((cd.workspace.uri.clone(), cd.change_id.clone())),
                        is_header: false,
                        color: None,
                        repo: None,
                        artifacts: Some(cd.artifacts.clone()),
                    });
                }
            }
        }
    }
    rows
}

/// Indices into `rows` passing the filter. A header survives when it matches or
/// any of its children match; a child survives when it (or its header) matches.
fn compute_visible(rows: &[TreeRow], filter: &Option<String>) -> Vec<usize> {
    let q = match filter {
        Some(q) if !q.trim().is_empty() => q.trim().to_lowercase(),
        _ => return (0..rows.len()).collect(),
    };
    let hit = |label: &str| label.to_lowercase().contains(&q);
    let mut out = Vec::new();
    let n = rows.len();
    let mut i = 0;
    while i < n {
        if rows[i].is_header {
            let mut j = i + 1;
            while j < n && !rows[j].is_header {
                j += 1;
            }
            let header_hit = hit(&rows[i].label);
            let kids: Vec<usize> = (i + 1..j).filter(|&k| hit(&rows[k].label)).collect();
            if header_hit {
                out.extend(i..j); // header matches: show the whole group
            } else if !kids.is_empty() {
                out.push(i);
                out.extend(kids);
            }
            i = j;
        } else {
            if hit(&rows[i].label) {
                out.push(i);
            }
            i += 1;
        }
    }
    out
}

/// Apply a message. `svc` and `tx` let async work be spawned without blocking.
pub fn update(model: &mut Model, msg: Msg, svc: &AppService, tx: &UnboundedSender<Msg>) {
    match msg {
        Msg::Key(key) => handle_key(model, key, svc, tx),
        Msg::Resize | Msg::Tick => {}
        Msg::Cache => {
            let before = model.selected_change();
            model.refresh(svc);
            model.refresh_settings_workspaces(svc);
            reconcile_detail(model, svc, tx, &before);
            if model.screen == Screen::History && model.graph_repo.is_some() {
                reload_graph(model, svc, tx);
            }
        }
        Msg::Artifact { gen, title, body } => {
            // Drop replies for a selection/tab the user has already moved past.
            if gen == model.artifact_gen {
                model.detail_title = title;
                model.detail_md = body;
                model.detail_scroll = 0;
            }
        }
        Msg::Dashboard(data) => {
            model.dashboard = Some(*data);
            model.dash_scroll = 0;
            model.season_scroll = 0;
        }
        Msg::Garden(plots) => {
            model.garden = Some(plots);
            model.garden_scroll = 0;
        }
        Msg::Graph(graph) => {
            model.graph = Some(*graph);
            if let Some(g) = &model.graph {
                if model.graph_selected >= g.commits.len() {
                    model.graph_selected = g.commits.len().saturating_sub(1);
                }
            }
        }
        Msg::Quota => {
            model.quota = svc.claude_quota();
        }
        Msg::AddResult(res) => match res {
            // The tree refreshes via the `CacheEvent` `add_workspace` emitted;
            // here we just close the prompt and refresh the Settings list.
            Ok(()) => {
                model.overlay = None;
                model.refresh_settings_workspaces(svc);
            }
            Err(e) => set_prompt_error(model, e),
        },
    }
}

fn handle_key(model: &mut Model, key: KeyEvent, svc: &AppService, tx: &UnboundedSender<Msg>) {
    // Ctrl-c quits from anywhere, including while editing the search filter.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        model.should_quit = true;
        return;
    }
    // The search input swallows everything else while editing.
    if model.filter_editing {
        handle_filter_key(model, key, svc, tx);
        return;
    }
    // A Settings modal (add / rename prompt, remove confirm) swallows everything
    // else while open — including screen-switch and quit keys.
    if model.overlay.is_some() {
        handle_overlay_key(model, key, svc, tx);
        return;
    }
    match key.code {
        KeyCode::Char('q') => {
            model.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            model.show_help = !model.show_help;
            return;
        }
        KeyCode::Esc => {
            if model.show_help {
                model.show_help = false;
            } else if model.filter.is_some() {
                model.filter = None;
                model.recompute_visible();
            } else {
                model.screen = Screen::Browse;
            }
            return;
        }
        KeyCode::Char('1') => {
            model.screen = Screen::Browse;
            return;
        }
        KeyCode::Char('2') => {
            model.screen = Screen::Dashboard;
            load_dashboard(svc, tx);
            return;
        }
        KeyCode::Char('3') => {
            model.screen = Screen::Season;
            load_dashboard(svc, tx);
            return;
        }
        KeyCode::Char('4') => {
            model.screen = Screen::Garden;
            load_garden(svc, tx);
            return;
        }
        KeyCode::Char('5') => {
            model.screen = Screen::History;
            load_graph(model, svc, tx);
            return;
        }
        KeyCode::Char('6') => {
            model.screen = Screen::Settings;
            // Re-read the stores so the panel reflects the current values, even
            // if another process (the desktop app) changed them since startup.
            model.gamification_on = svc.settings.gamification_enabled();
            model.quota_on = svc.settings.claude_quota_enabled();
            model.refresh_settings_workspaces(svc);
            return;
        }
        _ => {}
    }

    match model.screen {
        Screen::Browse => handle_browse_key(model, key, svc, tx),
        Screen::Dashboard => scroll_key(&mut model.dash_scroll, key),
        // The ladder auto-centres the current tier, so its nudge is signed:
        // `k` walks above the centre, `j` below.
        Screen::Season => match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                model.season_scroll = model.season_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                model.season_scroll = model.season_scroll.saturating_sub(1)
            }
            _ => {}
        },
        Screen::Garden => scroll_key(&mut model.garden_scroll, key),
        Screen::History => handle_history_key(model, key, svc, tx),
        Screen::Settings => handle_settings_key(model, key, svc, tx),
    }
}

/// Settings screen: move the row cursor and act on the focused row. Toggles flip
/// on Space/Enter; the add row and `a` open the add prompt; a workspace row takes
/// `x` (remove), `r`/Enter (rename), and `c` (cycle colour). `Esc` is handled by
/// the global key router (back to Browse), so it never reaches here.
fn handle_settings_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let max = settings_row_count(model).saturating_sub(1);
            model.settings_selected = (model.settings_selected + 1).min(max);
            return;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            model.settings_selected = model.settings_selected.saturating_sub(1);
            return;
        }
        // Add from anywhere on the screen, not just the add row.
        KeyCode::Char('a') => {
            open_add_prompt(model);
            return;
        }
        _ => {}
    }

    match settings_row_at(model.settings_selected) {
        SettingsRow::Toggle => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                toggle_focused_setting(model, svc, tx);
            }
        }
        SettingsRow::Appearance => {
            if matches!(
                key.code,
                KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')
            ) {
                cycle_scheme(model);
            }
        }
        SettingsRow::AddWorkspace => {
            if key.code == KeyCode::Enter {
                open_add_prompt(model);
            }
        }
        SettingsRow::Workspace(i) => match key.code {
            KeyCode::Char('x') => open_remove_confirm(model, svc, i),
            KeyCode::Char('r') | KeyCode::Enter => open_rename_prompt(model, i),
            KeyCode::Char('c') => cycle_workspace_color(model, svc, i),
            _ => {}
        },
    }
}

/// Open the add-workspace text prompt.
fn open_add_prompt(model: &mut Model) {
    model.overlay = Some(Overlay::Prompt {
        kind: PromptKind::AddWorkspace,
        title: "Add workspace — type or paste a folder path".to_string(),
        input: String::new(),
        error: None,
    });
}

/// Open the rename prompt for workspace `i`, prefilled with its current display
/// name (empty when it has none — clearing it reverts to the default basename).
fn open_rename_prompt(model: &mut Model, i: usize) {
    let Some(ws) = model.settings_workspaces.get(i) else {
        return;
    };
    let input = ws.display_name.clone().unwrap_or_default();
    let title = format!("Rename {} — empty clears to default", ws.name);
    let kind = PromptKind::RenameWorkspace {
        uri: ws.uri.clone(),
        repo_id: ws.repo_id.clone(),
        color: ws.color,
    };
    model.overlay = Some(Overlay::Prompt {
        kind,
        title,
        input,
        error: None,
    });
}

/// Open the remove confirm for workspace `i`, naming how many discovered
/// worktrees of its repository the cascade will also drop.
fn open_remove_confirm(model: &mut Model, svc: &AppService, i: usize) {
    let Some(ws) = model.settings_workspaces.get(i) else {
        return;
    };
    let label = ws.display_name.clone().unwrap_or_else(|| ws.name.clone());
    let uri = ws.uri.clone();
    let cascade = cascade_count(svc, ws);
    let message = if cascade > 0 {
        format!(
            "Remove \u{201c}{label}\u{201d}? This also unregisters {cascade} discovered worktree{} of its repository.",
            if cascade == 1 { "" } else { "s" }
        )
    } else {
        format!("Remove workspace \u{201c}{label}\u{201d}?")
    };
    model.overlay = Some(Overlay::Confirm {
        title: "Remove workspace".to_string(),
        message,
        action: ConfirmAction::RemoveWorkspace { uri },
    });
}

/// How many discovered worktrees would cascade-drop if this user-registered
/// workspace were removed — non-zero only when it's the last user-registered
/// workspace for its repository (otherwise the registry keeps the discoveries).
fn cascade_count(svc: &AppService, ws: &SettingsWorkspace) -> usize {
    let Some(repo) = ws.repo_id.as_deref() else {
        return 0;
    };
    let Ok(reg) = svc.registry.lock() else {
        return 0;
    };
    let entries = reg.entries();
    let other_user = entries.iter().any(|e| {
        matches!(e.origin, WorkspaceOrigin::UserRegistered)
            && e.folder.uri != ws.uri
            && e.repo_id.as_ref().map(|r| r.as_path()) == Some(repo)
    });
    if other_user {
        return 0;
    }
    entries
        .iter()
        .filter(|e| {
            matches!(e.origin, WorkspaceOrigin::Discovered { .. })
                && e.repo_id.as_ref().map(|r| r.as_path()) == Some(repo)
        })
        .count()
}

/// Advance a workspace's colour one step through the curated palette plus
/// "none", persisting immediately and re-tinting the tree and the row.
fn cycle_workspace_color(model: &mut Model, svc: &AppService, i: usize) {
    let Some(ws) = model.settings_workspaces.get(i) else {
        return;
    };
    let uri = ws.uri.clone();
    let repo_id = ws.repo_id.clone();
    let name = ws.display_name.clone();
    let next = next_color(ws.color);
    if let Err(e) = svc.set_workspace_presentation(uri, repo_id, name, next) {
        model.status = format!("Could not save: {e}");
        return;
    }
    model.refresh(svc);
    model.refresh_settings_workspaces(svc);
}

/// Advance the active colour scheme one step and persist the choice to the TUI
/// preference file. Live on the next redraw (the renderer reads the active scheme
/// each frame); persistence is best-effort and silent on failure.
fn cycle_scheme(model: &mut Model) {
    let next = theme::theme().active_scheme().next();
    theme::set_scheme(next);
    if let Some(dir) = &model.config_dir {
        prefs::save_scheme(dir, next);
    }
}

/// The next colour in the cycle `none → indigo → … → purple → none`.
fn next_color(c: Option<PaletteColor>) -> Option<PaletteColor> {
    use PaletteColor::*;
    match c {
        None => Some(Indigo),
        Some(Indigo) => Some(Blue),
        Some(Blue) => Some(Teal),
        Some(Teal) => Some(Green),
        Some(Green) => Some(Amber),
        Some(Amber) => Some(Orange),
        Some(Orange) => Some(Rose),
        Some(Rose) => Some(Purple),
        Some(Purple) => None,
    }
}

/// Route a key to the open Settings modal. Esc cancels; for a confirm, `y`/Enter
/// commits and `n`/Esc cancels; for a prompt, Enter submits and typed characters
/// edit the buffer (clearing any inline error).
fn handle_overlay_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    let is_confirm = matches!(model.overlay, Some(Overlay::Confirm { .. }));
    match key.code {
        KeyCode::Esc => model.overlay = None,
        KeyCode::Char('n') if is_confirm => model.overlay = None,
        KeyCode::Char('y') if is_confirm => confirm_overlay(model, svc, tx),
        KeyCode::Enter => {
            if is_confirm {
                confirm_overlay(model, svc, tx);
            } else {
                submit_prompt(model, svc, tx);
            }
        }
        KeyCode::Backspace => {
            if let Some(Overlay::Prompt { input, error, .. }) = &mut model.overlay {
                input.pop();
                *error = None;
            }
        }
        KeyCode::Char(c) => {
            if let Some(Overlay::Prompt { input, error, .. }) = &mut model.overlay {
                input.push(c);
                *error = None;
            }
        }
        _ => {}
    }
}

/// An owned description of a prompt's pending action, extracted so the borrow of
/// `model.overlay` ends before the model is mutated.
enum PromptAction {
    Add(String),
    Rename {
        uri: PathBuf,
        repo_id: Option<PathBuf>,
        color: Option<PaletteColor>,
        input: String,
    },
}

/// Resolve the focused prompt's Enter: spawn an async add (kept open until the
/// `AddResult` lands), or apply a synchronous rename and close on success.
fn submit_prompt(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let action = match &model.overlay {
        Some(Overlay::Prompt { kind, input, .. }) => {
            let input = input.clone();
            match kind {
                PromptKind::AddWorkspace => PromptAction::Add(input),
                PromptKind::RenameWorkspace {
                    uri,
                    repo_id,
                    color,
                } => PromptAction::Rename {
                    uri: uri.clone(),
                    repo_id: repo_id.clone(),
                    color: *color,
                    input,
                },
            }
        }
        _ => return,
    };
    match action {
        PromptAction::Add(path) => {
            if path.trim().is_empty() {
                set_prompt_error(model, "Enter a folder path.".to_string());
                return;
            }
            submit_add(svc, tx, path);
        }
        PromptAction::Rename {
            uri,
            repo_id,
            color,
            input,
        } => {
            let name = if input.trim().is_empty() {
                None
            } else {
                Some(input)
            };
            match svc.set_workspace_presentation(uri, repo_id, name, color) {
                Ok(()) => {
                    model.overlay = None;
                    model.refresh(svc);
                    model.refresh_settings_workspaces(svc);
                }
                Err(e) => set_prompt_error(model, e),
            }
        }
    }
}

/// Spawn the async registration; the result returns as [`Msg::AddResult`].
fn submit_add(svc: &AppService, tx: &UnboundedSender<Msg>, path: String) {
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let res = svc.add_workspace(PathBuf::from(path)).await.map(|_| ());
        let _ = tx.send(Msg::AddResult(res));
    });
}

/// Set the inline error on the open prompt (no-op for a confirm overlay).
fn set_prompt_error(model: &mut Model, err: String) {
    if let Some(Overlay::Prompt { error, .. }) = &mut model.overlay {
        *error = Some(err);
    }
}

/// Commit a confirm overlay's action. Removal is spawned async and the overlay
/// closes optimistically; the watcher's `CacheEvent` refreshes the views, and a
/// nudge covers the case where nothing was actually removed.
fn confirm_overlay(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let uri = match &model.overlay {
        Some(Overlay::Confirm { action, .. }) => match action {
            ConfirmAction::RemoveWorkspace { uri } => uri.clone(),
        },
        _ => return,
    };
    model.overlay = None;
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = svc.remove_workspace(uri).await;
        let _ = tx.send(Msg::Cache);
    });
}

/// Flip the setting the cursor is on, persist it immediately, and make the
/// change visible in the running TUI: gamification re-fetches the gamified
/// surfaces; disabling the quota opt-in clears the title-bar gauge at once
/// (enabling it lets the always-running poller surface the gauge on its next
/// refresh). A persist failure is surfaced in the status line and leaves the
/// mirrored value untouched.
fn toggle_focused_setting(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    match model.settings_selected {
        0 => {
            let next = !model.gamification_on;
            if let Err(e) = svc.settings.set_gamification_enabled(next) {
                model.status = format!("Could not save settings: {e}");
                return;
            }
            model.gamification_on = next;
            // The gamified surfaces read the flag when their data is *built*, so
            // re-dispatch the same fetches the 2/3/4 keys use to reflect the new
            // state without a restart.
            load_dashboard(svc, tx);
            load_garden(svc, tx);
        }
        1 => {
            let next = !model.quota_on;
            if let Err(e) = svc.settings.set_claude_quota_enabled(next) {
                model.status = format!("Could not save settings: {e}");
                return;
            }
            model.quota_on = next;
            if !next {
                model.quota = ClaudeQuotaState::disabled();
            }
        }
        _ => {}
    }
}

/// Bring the detail pane and tab strip back in sync after the selection may have
/// moved (cache refresh, filter edit, navigation). Reloads when the selected
/// change differs from `before`, or resets to the placeholder on a header row.
fn reconcile_detail(
    model: &mut Model,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
    before: &Option<(PathBuf, String)>,
) {
    let after = model.selected_change();
    if &after == before {
        return;
    }
    match after {
        Some(_) => {
            refresh_tabs(model);
            load_selected_artifact(model, svc, tx);
        }
        None => {
            model.tabs.clear();
            model.active_tab = 0;
            model.detail_title.clear();
            model.detail_md = "Select a change to read its proposal.".to_string();
            model.detail_scroll = 0;
        }
    }
}

fn handle_filter_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    let before = model.selected_change();
    match key.code {
        KeyCode::Esc => {
            model.filter = None;
            model.filter_editing = false;
            model.recompute_visible();
        }
        KeyCode::Enter => model.filter_editing = false, // keep the filter applied
        KeyCode::Backspace => {
            if let Some(q) = model.filter.as_mut() {
                q.pop();
            }
            model.recompute_visible();
        }
        KeyCode::Char(c) => {
            if let Some(q) = model.filter.as_mut() {
                q.push(c);
            }
            model.recompute_visible();
        }
        _ => {}
    }
    reconcile_detail(model, svc, tx, &before);
}

fn handle_browse_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    if key.code == KeyCode::Tab {
        model.focus = match model.focus {
            Focus::Tree => Focus::Detail,
            Focus::Detail => Focus::Tree,
        };
        return;
    }
    match model.focus {
        Focus::Tree => match key.code {
            KeyCode::Char('/') => {
                model.filter = Some(String::new());
                model.filter_editing = true;
            }
            KeyCode::Down | KeyCode::Char('j') => move_selection(model, 1, svc, tx),
            KeyCode::Up | KeyCode::Char('k') => move_selection(model, -1, svc, tx),
            KeyCode::Enter | KeyCode::Char('l') if model.selected_change().is_some() => {
                model.focus = Focus::Detail;
                refresh_tabs(model);
                load_selected_artifact(model, svc, tx);
            }
            _ => {}
        },
        Focus::Detail => match key.code {
            KeyCode::Char('h') => model.focus = Focus::Tree,
            KeyCode::Char('[') => cycle_tab(model, -1, svc, tx),
            KeyCode::Char(']') => cycle_tab(model, 1, svc, tx),
            KeyCode::Down | KeyCode::Char('j') => {
                model.detail_scroll = model.detail_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                model.detail_scroll = model.detail_scroll.saturating_sub(1)
            }
            _ => {}
        },
    }
}

fn handle_history_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let max = model
                .graph
                .as_ref()
                .map(|g| g.commits.len().saturating_sub(1))
                .unwrap_or(0);
            model.graph_selected = (model.graph_selected + 1).min(max);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            model.graph_selected = model.graph_selected.saturating_sub(1);
        }
        // Load more history when the window was truncated.
        KeyCode::Char('m') if model.graph.as_ref().is_some_and(|g| g.truncated) => {
            model.graph_limit = model.graph_limit.saturating_add(GRAPH_PAGE);
            reload_graph(model, svc, tx);
        }
        _ => {}
    }
}

fn scroll_key(scroll: &mut u16, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
        KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
        _ => {}
    }
}

fn move_selection(model: &mut Model, delta: i32, svc: &AppService, tx: &UnboundedSender<Msg>) {
    if model.visible.is_empty() {
        return;
    }
    let before = model.selected_change();
    let max = model.visible.len() as i32 - 1;
    let next = (model.selected as i32 + delta).clamp(0, max) as usize;
    if next != model.selected {
        model.selected = next;
        reconcile_detail(model, svc, tx, &before);
    }
}

/// Recompute the tab list for the current selection and reset to the first tab.
fn refresh_tabs(model: &mut Model) {
    model.tabs = model
        .selected_row()
        .and_then(|r| r.artifacts.as_ref())
        .map(tabs_for)
        .unwrap_or_default();
    model.active_tab = 0;
}

fn cycle_tab(model: &mut Model, delta: i32, svc: &AppService, tx: &UnboundedSender<Msg>) {
    if model.tabs.is_empty() {
        return;
    }
    let n = model.tabs.len() as i32;
    let next = (model.active_tab as i32 + delta).rem_euclid(n) as usize;
    if next != model.active_tab {
        model.active_tab = next;
        load_selected_artifact(model, svc, tx);
    }
}

fn load_selected_artifact(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let Some((workspace, change_id)) = model.selected_change() else {
        return;
    };
    let tab = model
        .tabs
        .get(model.active_tab)
        .cloned()
        .unwrap_or(ArtifactTab::Proposal);
    let (kind, capability) = tab.read_target();
    let title = format!("{} — {}", change_id, tab.filename());
    model.detail_title = title.clone();
    model.artifact_gen = model.artifact_gen.wrapping_add(1);
    let gen = model.artifact_gen;
    let svc = svc.clone();
    let tx = tx.clone();
    let filename = tab.filename();
    tokio::spawn(async move {
        let body = svc
            .read_artifact(&workspace, &change_id, kind, capability.as_deref())
            .await
            .unwrap_or_else(|e| format!("Could not read {filename}: {e}"));
        let _ = tx.send(Msg::Artifact { gen, title, body });
    });
}

fn load_dashboard(svc: &AppService, tx: &UnboundedSender<Msg>) {
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if let Ok(data) = svc.dashboard().await {
            let _ = tx.send(Msg::Dashboard(Box::new(data)));
        }
    });
}

fn load_garden(svc: &AppService, tx: &UnboundedSender<Msg>) {
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if let Ok(plots) = svc.commit_garden().await {
            let _ = tx.send(Msg::Garden(plots));
        }
    });
}

fn load_graph(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let Some(repo) = model.selected_row().and_then(|r| r.repo.clone()) else {
        return;
    };
    model.graph_repo = Some(repo);
    model.graph_selected = 0;
    reload_graph(model, svc, tx);
}

fn reload_graph(model: &Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let Some(repo) = model.graph_repo.clone() else {
        return;
    };
    let limit = model.graph_limit;
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if let Ok(graph) = svc.commit_graph(repo, limit).await {
            let _ = tx.send(Msg::Graph(Box::new(graph)));
        }
    });
}
