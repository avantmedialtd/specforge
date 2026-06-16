//! The Elm-style core: `Model`, `Msg`, and `update`. The view is in `ui`.
//!
//! `update` owns all state transitions and, for anything asynchronous (loading
//! an artifact, assembling the dashboard), spawns a task that posts a `Msg`
//! back through the channel — so the render loop never blocks.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::AppService;
use openspec_core::WorkspaceView;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Dashboard,
    Season,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Detail,
}

/// Messages that drive `update`. Async results arrive as `Artifact`/`Dashboard`.
pub enum Msg {
    Key(KeyEvent),
    Resize,
    Cache,
    Tick,
    Artifact { title: String, body: String },
    Dashboard(Box<Value>),
}

/// One flattened tree row: a workspace header or a change beneath it.
pub struct TreeRow {
    pub depth: u8,
    pub label: String,
    pub progress: Option<(usize, usize)>,
    /// `(workspace_uri, change_id)` when this row is a loadable change.
    pub change: Option<(PathBuf, String)>,
    pub is_header: bool,
}

pub struct Model {
    pub screen: Screen,
    pub focus: Focus,
    pub rows: Vec<TreeRow>,
    pub selected: usize,
    pub detail_title: String,
    pub detail_md: String,
    pub detail_scroll: u16,
    pub dashboard: Option<Value>,
    pub dash_scroll: u16,
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
            selected: 0,
            detail_title: String::new(),
            detail_md: "Select a change to read its proposal.".to_string(),
            detail_scroll: 0,
            dashboard: None,
            dash_scroll: 0,
            status: String::new(),
            show_help: false,
            should_quit: false,
        };
        m.refresh(svc);
        m
    }

    /// Re-read the aggregated view from the service (never a parallel cache).
    pub fn refresh(&mut self, svc: &AppService) {
        self.rows = flatten(&svc.workspace_views());
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        let active = svc.active_count();
        let ws = svc.list_workspaces().map(|w| w.len()).unwrap_or(0);
        self.status = format!("{ws} workspaces · {active} open changes");
    }

    pub fn selected_change(&self) -> Option<(PathBuf, String)> {
        self.rows.get(self.selected).and_then(|r| r.change.clone())
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
                        });
                    }
                }
            }
            WorkspaceView::Flat {
                workspace,
                changes,
                display_name,
                ..
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
                });
                for cd in changes {
                    rows.push(TreeRow {
                        depth: 1,
                        label: cd.title.clone().unwrap_or_else(|| cd.change_id.clone()),
                        progress: Some((cd.completed_tasks, cd.total_tasks)),
                        change: Some((cd.workspace.uri.clone(), cd.change_id.clone())),
                        is_header: false,
                    });
                }
            }
        }
    }
    rows
}

/// Apply a message. `svc` and `tx` let async work be spawned without blocking.
pub fn update(model: &mut Model, msg: Msg, svc: &AppService, tx: &UnboundedSender<Msg>) {
    match msg {
        Msg::Key(key) => handle_key(model, key, svc, tx),
        Msg::Resize | Msg::Tick => {}
        Msg::Cache => model.refresh(svc),
        Msg::Artifact { title, body } => {
            model.detail_title = title;
            model.detail_md = body;
            model.detail_scroll = 0;
        }
        Msg::Dashboard(value) => {
            model.dashboard = Some(*value);
            model.dash_scroll = 0;
        }
    }
}

fn handle_key(model: &mut Model, key: KeyEvent, svc: &AppService, tx: &UnboundedSender<Msg>) {
    // Global bindings first.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        model.should_quit = true;
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
        _ => {}
    }

    match model.screen {
        Screen::Browse => handle_browse_key(model, key, svc, tx),
        Screen::Dashboard => scroll_key(&mut model.dash_scroll, key),
        Screen::Season => scroll_key(&mut model.dash_scroll, key),
    }
}

fn handle_browse_key(
    model: &mut Model,
    key: KeyEvent,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
) {
    match key.code {
        KeyCode::Tab => {
            model.focus = match model.focus {
                Focus::Tree => Focus::Detail,
                Focus::Detail => Focus::Tree,
            };
        }
        _ => match model.focus {
            Focus::Tree => match key.code {
                KeyCode::Down | KeyCode::Char('j') => move_selection(model, 1, svc, tx),
                KeyCode::Up | KeyCode::Char('k') => move_selection(model, -1, svc, tx),
                KeyCode::Enter | KeyCode::Char('l') if model.selected_change().is_some() => {
                    model.focus = Focus::Detail;
                    load_selected_artifact(model, svc, tx);
                }
                _ => {}
            },
            Focus::Detail => match key.code {
                KeyCode::Char('h') => model.focus = Focus::Tree,
                KeyCode::Down | KeyCode::Char('j') => {
                    model.detail_scroll = model.detail_scroll.saturating_add(1)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    model.detail_scroll = model.detail_scroll.saturating_sub(1)
                }
                _ => {}
            },
        },
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
    if model.rows.is_empty() {
        return;
    }
    let max = model.rows.len() as i32 - 1;
    let next = (model.selected as i32 + delta).clamp(0, max) as usize;
    if next != model.selected {
        model.selected = next;
        if model.selected_change().is_some() {
            load_selected_artifact(model, svc, tx);
        }
    }
}

fn load_selected_artifact(model: &mut Model, svc: &AppService, tx: &UnboundedSender<Msg>) {
    let Some((workspace, change_id)) = model.selected_change() else {
        return;
    };
    model.detail_title = change_id.clone();
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let body = svc
            .read_artifact(&workspace, &change_id, "proposal", None)
            .await
            .unwrap_or_else(|e| format!("Could not read proposal.md: {e}"));
        let _ = tx.send(Msg::Artifact {
            title: change_id,
            body,
        });
    });
}

fn load_dashboard(svc: &AppService, tx: &UnboundedSender<Msg>) {
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if let Ok(data) = svc.dashboard().await {
            if let Ok(value) = serde_json::to_value(&data) {
                let _ = tx.send(Msg::Dashboard(Box::new(value)));
            }
        }
    });
}
