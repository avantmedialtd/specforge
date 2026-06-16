//! The Elm-style core: `Model`, `Msg`, and `update`. The view is in `ui`.
//!
//! `update` owns all state transitions and, for anything asynchronous (loading
//! an artifact, assembling the dashboard, mining a commit graph), spawns a task
//! that posts a `Msg` back through the channel — so the render loop never
//! blocks. All payloads are the headless core's own typed structs (no parallel
//! cache, no JSON scraping); the TUI links `openspec_core` directly.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::AppService;
use openspec_core::{
    ArtifactStatus, CommitGraph, DashboardData, PaletteColor, WorkspaceGarden, WorkspaceView,
};
use tokio::sync::mpsc::UnboundedSender;

/// Default commit-graph window; bumped by `m` when more history exists.
const GRAPH_PAGE: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Dashboard,
    Season,
    Garden,
    History,
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
            show_help: false,
            should_quit: false,
        };
        m.refresh(svc);
        m
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
