//! The Elm-style core: `Model`, `Msg`, and `update`. The view is in `ui`.
//!
//! `update` owns all state transitions and, for anything asynchronous (loading
//! an artifact, assembling the dashboard, mining a commit graph), spawns a task
//! that posts a `Msg` back through the channel — so the render loop never
//! blocks. All payloads are the headless core's own typed structs (no parallel
//! cache, no JSON scraping); the TUI links `openspec_core` directly.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openspec_app::{AppService, ChatGptQuotaState, ClaudeQuotaState};
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

/// The three toggle rows the Settings screen leads with, in display order.
/// They occupy cursor indices `0..SETTINGS_TOGGLE_COUNT`; the add-workspace
/// action and the per-workspace rows follow, so the cursor's upper bound is
/// dynamic (see [`settings_row_count`]).
pub const SETTINGS_TOGGLE_COUNT: usize = 3;

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
    /// True when the user has parked this row: it keeps being watched and keeps
    /// feeding the Dashboard, but leaves the Browse tree. Always re-read from
    /// the listing rather than flipped optimistically, so a persist that failed
    /// shows the truth instead of a lie.
    pub disabled: bool,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Why an artifact body is being loaded.
///
/// A user-initiated load returns the reader to the top of the new artifact and
/// surfaces a read failure in place of the body, as it always has. A re-read
/// driven by the filesystem watcher targets what is *already* on screen, so it
/// must leave both the scroll offset and — when the read fails — the existing
/// body alone (`terminal-ui`: *Live Updates From the Watcher*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadTrigger {
    Select,
    Watch,
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
        /// `Err` carries the message a user-initiated load renders in place of
        /// the body; a watcher-driven load discards it and keeps what it has.
        body: Result<String, String>,
        trigger: LoadTrigger,
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
    /// Trigger of the artifact load currently outstanding, `None` once its
    /// reply lands. A watcher-driven re-read issued while a user-initiated
    /// load is still in flight inherits `Select` from it, so superseding the
    /// user's load never costs them the jump to the top of what they chose.
    pub pending_trigger: Option<LoadTrigger>,

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
    /// How many top-level rows the tree filter dropped because the user parked
    /// them. Counted from the *unfiltered* snapshot, one per top-level row — so
    /// a repository registered at two worktrees counts once, not twice. The
    /// Dashboard reads that same unfiltered record (`dashboard`: *Dashboard
    /// Unaffected by Workspace Disable*), so its totals exceed the tree's
    /// whenever this is non-zero and the Dashboard footnote says so.
    pub disabled_row_count: usize,
    /// Latest opt-in Claude usage-quota snapshot, rendered in the title bar.
    /// `Disabled` until the poller runs with the feature enabled.
    pub quota: ClaudeQuotaState,
    /// Latest opt-in ChatGPT usage-quota snapshot, rendered in the title bar
    /// beside `quota` (Claude). `Disabled` until the poller runs with the
    /// feature enabled. Refreshed on the same `Msg::Quota` as `quota` — the
    /// ChatGPT poller emits the identical `CacheEvent::QuotaUpdated`.
    pub chatgpt_quota: ChatGptQuotaState,

    /// Row cursor on the Settings screen (`0..settings_row_count`).
    pub settings_selected: usize,
    /// The Settings screen renders from `Model` alone (the view never sees the
    /// service), so the three toggles are mirrored here. Re-read from the store
    /// whenever the screen is opened and after each flip, so the panel always
    /// shows what was last written.
    pub gamification_on: bool,
    pub quota_on: bool,
    pub chatgpt_quota_on: bool,
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
            pending_trigger: None,
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
            disabled_row_count: 0,
            quota: svc.claude_quota(),
            chatgpt_quota: svc.chatgpt_quota(),
            settings_selected: 0,
            gamification_on: svc.settings.gamification_enabled(),
            quota_on: svc.settings.claude_quota_enabled(),
            chatgpt_quota_on: svc.settings.chatgpt_quota_enabled(),
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
                disabled: w.disabled,
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
        // The rows the filter took away, counted off the *unfiltered* snapshot
        // the Dashboard reads — `AppService::workspace_views` is exactly this
        // list with `retain(!is_disabled)` applied, and the aggregator emits one
        // entry per top-level row (one per repo group, one per flat workspace),
        // so a repository registered at several worktrees counts once.
        self.disabled_row_count = svc
            .watcher
            .workspace_views()
            .iter()
            .filter(|v| v.is_disabled())
            .count();
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
                // Disabled rows never reach a frontend — `get_workspace_views`
                // drops them — so the TUI has nothing to render differently.
                disabled: _,
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
            // `reconcile_detail` only acts when the selection moved, but the
            // batch that woke us may have rewritten the artifact already on
            // screen. Re-read it so the detail pane matches disk, holding the
            // reader's place (`terminal-ui`: *Live Updates From the Watcher*).
            // This lives here rather than inside `reconcile_detail` because
            // that helper also runs on filter and cursor keys, where an
            // unchanged selection means nothing has changed on disk.
            if model.selected_change() == before && before.is_some() {
                // The artifact set can change without the selection moving —
                // an agent writes `design.md` into a change that had only a
                // proposal. Rebuild the strip keeping the reader's tab; if
                // that tab's file is gone, the body about to load is a
                // different artifact, so it starts at the top rather than
                // inheriting an offset into a file the reader was never in.
                let tab_changed = refresh_tabs_preserving_active(model);
                let trigger = if tab_changed {
                    LoadTrigger::Select
                } else {
                    LoadTrigger::Watch
                };
                load_selected_artifact(model, svc, tx, trigger);
            }
            if model.screen == Screen::History && model.graph_repo.is_some() {
                reload_graph(model, svc, tx);
            }
        }
        Msg::Artifact {
            gen,
            title,
            body,
            trigger,
        } => {
            // Drop replies for a selection/tab the user has already moved past.
            if gen == model.artifact_gen {
                model.pending_trigger = None;
                match body {
                    Ok(body) => {
                        model.detail_title = title;
                        // Only a load the user asked for returns them to the
                        // top; a watcher-driven re-read holds their place —
                        // but never past the end of a file that shrank under
                        // them. `Paragraph::scroll` renders blank beyond the
                        // last line and the only way back is one `k` per line,
                        // so an unclamped offset silently empties the pane.
                        if trigger == LoadTrigger::Select {
                            model.detail_scroll = 0;
                        } else {
                            model.detail_scroll = model.detail_scroll.min(max_scroll(&body));
                        }
                        model.detail_md = body;
                    }
                    // A failed re-read of what is already on screen leaves the
                    // reader with the content they had, rather than replacing
                    // it with an error they did not provoke.
                    Err(message) => {
                        if trigger == LoadTrigger::Select {
                            model.detail_title = title;
                            model.detail_md = message;
                            model.detail_scroll = 0;
                        }
                    }
                }
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
            model.chatgpt_quota = svc.chatgpt_quota();
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
            model.chatgpt_quota_on = svc.settings.chatgpt_quota_enabled();
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
/// Space (enable/disable), `x` (remove), `r`/Enter (rename), and `c` (cycle
/// colour). `Esc` is handled by the global key router (back to Browse), so it
/// never reaches here.
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
            KeyCode::Char(' ') => toggle_workspace_disabled(model, svc, tx, i),
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

/// Park or un-park the focused workspace's top-level row. Reversible from the
/// same row, so — like the colour cycle — it applies at once with no confirm.
///
/// The service call is async (un-parking re-runs the git sweep the row skipped
/// while parked), so it is spawned like the remove flow and a `Msg::Cache` nudge
/// re-reads the tree, the Settings mirror and the Dashboard footnote when it
/// lands. Nothing is flipped locally: the mirror comes back from
/// `list_workspaces`, so a persist that failed self-corrects instead of lying.
fn toggle_workspace_disabled(
    model: &mut Model,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
    i: usize,
) {
    let Some(ws) = model.settings_workspaces.get(i) else {
        return;
    };
    let uri = ws.uri.clone();
    let repo_id = ws.repo_id.clone();
    let next = !ws.disabled;
    let svc = svc.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = svc.set_workspace_disabled(uri, repo_id, next).await;
        let _ = tx.send(Msg::Cache);
    });
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
/// surfaces; disabling a quota opt-in clears its title-bar gauge at once
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
        2 => {
            let next = !model.chatgpt_quota_on;
            if let Err(e) = svc.settings.set_chatgpt_quota_enabled(next) {
                model.status = format!("Could not save settings: {e}");
                return;
            }
            model.chatgpt_quota_on = next;
            if !next {
                model.chatgpt_quota = ChatGptQuotaState::disabled();
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
            load_selected_artifact(model, svc, tx, LoadTrigger::Select);
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
                load_selected_artifact(model, svc, tx, LoadTrigger::Select);
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

/// Rebuild the tab strip for the *same* change, keeping the reader on the tab
/// they were reading. Used by the watcher path, where the artifact set can grow
/// (an agent writes `design.md`) or shrink while the selection never moves —
/// [`refresh_tabs`] would drop them back to the first tab on every batch.
/// Returns true when the active tab's artifact is gone, so the caller knows the
/// body it is about to read belongs to a different tab than before.
fn refresh_tabs_preserving_active(model: &mut Model) -> bool {
    let active = model.tabs.get(model.active_tab).cloned();
    model.tabs = model
        .selected_row()
        .and_then(|r| r.artifacts.as_ref())
        .map(tabs_for)
        .unwrap_or_default();
    match active.and_then(|tab| model.tabs.iter().position(|t| *t == tab)) {
        Some(idx) => {
            model.active_tab = idx;
            false
        }
        None => {
            model.active_tab = 0;
            true
        }
    }
}

fn cycle_tab(model: &mut Model, delta: i32, svc: &AppService, tx: &UnboundedSender<Msg>) {
    if model.tabs.is_empty() {
        return;
    }
    let n = model.tabs.len() as i32;
    let next = (model.active_tab as i32 + delta).rem_euclid(n) as usize;
    if next != model.active_tab {
        model.active_tab = next;
        load_selected_artifact(model, svc, tx, LoadTrigger::Select);
    }
}

/// Largest scroll offset that still shows a line of `body`.
///
/// Counted in *source* lines, a lower bound on the rendered line count once
/// `Paragraph`'s wrapping is applied — so this clamps at least as eagerly as
/// strictly necessary, never less. That is the safe direction: the reader may
/// be nudged up a line, but is never left facing a blank pane.
fn max_scroll(body: &str) -> u16 {
    u16::try_from(body.lines().count().saturating_sub(1)).unwrap_or(u16::MAX)
}

fn load_selected_artifact(
    model: &mut Model,
    svc: &AppService,
    tx: &UnboundedSender<Msg>,
    trigger: LoadTrigger,
) {
    let Some((workspace, change_id)) = model.selected_change() else {
        return;
    };
    // Bumping the generation below cancels whatever is in flight. If that was
    // a load the user asked for, this re-read has to finish the job on its
    // behalf, or the selection they just made never lands at the top.
    let trigger = match (trigger, model.pending_trigger) {
        (LoadTrigger::Watch, Some(LoadTrigger::Select)) => LoadTrigger::Select,
        _ => trigger,
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
    model.pending_trigger = Some(trigger);
    let gen = model.artifact_gen;
    let svc = svc.clone();
    let tx = tx.clone();
    let filename = tab.filename();
    tokio::spawn(async move {
        let body = svc
            .read_artifact(&workspace, &change_id, kind, capability.as_deref())
            .await
            .map_err(|e| format!("Could not read {filename}: {e}"));
        let _ = tx.send(Msg::Artifact {
            gen,
            title,
            body,
            trigger,
        });
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::{tempdir, TempDir};
    use tokio::sync::mpsc;

    use super::*;

    /// A service over an empty config dir, plus a channel the spawned reads
    /// post back through. Tests assert on the model, not on the replies.
    fn harness(cfg: &TempDir) -> (AppService, mpsc::UnboundedSender<Msg>) {
        let svc = AppService::bootstrap(cfg.path().to_path_buf());
        let (tx, rx) = mpsc::unbounded_channel();
        // Keep the receiver alive for the test's duration so `tx.send` in a
        // spawned read never fails; nothing here inspects what it carries.
        std::mem::forget(rx);
        (svc, tx)
    }

    /// A flat OpenSpec workspace holding one change with a readable proposal.
    fn workspace_with_change(tmp: &TempDir) -> PathBuf {
        let root = tmp.path().join("acme");
        let change = root.join("openspec").join("changes").join("demo");
        fs::create_dir_all(&change).unwrap();
        fs::write(change.join("proposal.md"), "# Demo\n\n## Why\n\nBecause.\n").unwrap();
        root
    }

    /// Register one workspace and park the tree cursor on its change row.
    async fn model_on_a_change(svc: &AppService, ws: &TempDir) -> Model {
        svc.add_workspace(workspace_with_change(ws))
            .await
            .expect("register");
        let mut model = Model::new(svc);
        let idx = model
            .visible
            .iter()
            .position(|&i| model.rows[i].change.is_some())
            .expect("the registered change appears in the tree");
        model.selected = idx;
        assert!(model.selected_change().is_some());
        // Selecting a change populates the tab strip (via `reconcile_detail`
        // or Enter); the watcher path relies on that having happened, so the
        // fixture mirrors it rather than leaving `tabs` empty.
        refresh_tabs(&mut model);
        model
    }

    fn reply(model: &Model, body: Result<String, String>, trigger: LoadTrigger) -> Msg {
        Msg::Artifact {
            gen: model.artifact_gen,
            title: "demo — proposal.md".to_string(),
            body,
            trigger,
        }
    }

    #[test]
    fn select_reply_returns_the_reader_to_the_top() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_scroll = 9;

        let msg = reply(&model, Ok("fresh".to_string()), LoadTrigger::Select);
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_md, "fresh");
        assert_eq!(model.detail_scroll, 0);
    }

    /// A body of `n` lines, so scroll offsets in these tests are meaningful.
    fn body_of(n: usize, tag: &str) -> String {
        (0..n)
            .map(|i| format!("{tag} line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn watch_reply_holds_the_readers_place() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_scroll = 9;
        let fresh = body_of(80, "fresh");

        let msg = reply(&model, Ok(fresh.clone()), LoadTrigger::Watch);
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_md, fresh, "the new bytes are shown");
        assert_eq!(model.detail_scroll, 9, "the reader is not moved");
    }

    #[test]
    fn watch_reply_clamps_the_offset_to_a_shrunken_body() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_scroll = 150;
        let pruned = body_of(20, "pruned");

        let msg = reply(&model, Ok(pruned.clone()), LoadTrigger::Watch);
        update(&mut model, msg, &svc, &tx);

        // Without the clamp `Paragraph::scroll(150)` renders nothing at all
        // and `k` is the only way back, one line per press.
        assert_eq!(model.detail_md, pruned);
        assert_eq!(model.detail_scroll, 19);
    }

    #[test]
    fn watch_reply_leaves_an_in_range_offset_alone() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_scroll = 10;

        let msg = reply(&model, Ok(body_of(40, "same")), LoadTrigger::Watch);
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_scroll, 10, "clamping only bites when it must");
    }

    #[test]
    fn watch_failure_keeps_the_body_already_on_screen() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_md = "what the reader was reading".to_string();
        model.detail_scroll = 4;

        let msg = reply(
            &model,
            Err("Could not read proposal.md".to_string()),
            LoadTrigger::Watch,
        );
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_md, "what the reader was reading");
        assert_eq!(model.detail_scroll, 4);
    }

    #[test]
    fn select_failure_replaces_the_body_with_the_message() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_md = "stale".to_string();
        model.detail_scroll = 4;

        let msg = reply(
            &model,
            Err("Could not read proposal.md".to_string()),
            LoadTrigger::Select,
        );
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_md, "Could not read proposal.md");
        assert_eq!(model.detail_scroll, 0);
    }

    #[test]
    fn stale_reply_is_discarded() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        model.detail_md = "current".to_string();
        model.detail_scroll = 3;
        model.artifact_gen = 7;

        let msg = Msg::Artifact {
            gen: 6,
            title: "old".to_string(),
            body: Ok("overtaken".to_string()),
            trigger: LoadTrigger::Select,
        };
        update(&mut model, msg, &svc, &tx);

        assert_eq!(model.detail_md, "current");
        assert_eq!(model.detail_scroll, 3);
    }

    #[tokio::test]
    async fn cache_event_rereads_the_open_artifact() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;
        let before = model.artifact_gen;

        update(&mut model, Msg::Cache, &svc, &tx);

        assert_eq!(
            model.artifact_gen,
            before + 1,
            "an unchanged selection still re-reads the open artifact"
        );
        assert_eq!(model.pending_trigger, Some(LoadTrigger::Watch));
    }

    #[test]
    fn cache_event_without_a_selection_reads_nothing() {
        let cfg = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = Model::new(&svc);
        assert!(model.selected_change().is_none());

        update(&mut model, Msg::Cache, &svc, &tx);

        assert_eq!(model.artifact_gen, 0);
        assert_eq!(model.pending_trigger, None);
    }

    #[tokio::test]
    async fn reconcile_detail_does_not_reread_on_an_unchanged_selection() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;
        let before = model.selected_change();
        let gen = model.artifact_gen;

        // Filter and cursor keys funnel through here too, where an unchanged
        // selection means nothing on disk has moved and a read would be waste.
        reconcile_detail(&mut model, &svc, &tx, &before);

        assert_eq!(model.artifact_gen, gen);
    }

    /// Overwrite the parsed artifact set of the row the cursor is on, standing
    /// in for the watcher re-parsing a change whose files changed on disk.
    fn set_artifacts(model: &mut Model, artifacts: ArtifactStatus) {
        let row = model.visible[model.selected];
        model.rows[row].artifacts = Some(artifacts);
    }

    fn artifacts(proposal: bool, design: bool, tasks: bool) -> ArtifactStatus {
        ArtifactStatus {
            proposal,
            design,
            tasks,
            specs: Vec::new(),
        }
    }

    #[tokio::test]
    async fn watcher_reread_picks_up_an_artifact_that_appeared() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, _tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;
        assert_eq!(model.tabs, vec![ArtifactTab::Proposal]);

        // An agent writes design.md and tasks.md into the open change.
        set_artifacts(&mut model, artifacts(true, true, true));
        let tab_changed = refresh_tabs_preserving_active(&mut model);

        assert_eq!(
            model.tabs,
            vec![
                ArtifactTab::Proposal,
                ArtifactTab::Design,
                ArtifactTab::Tasks
            ],
            "the new artifacts are reachable without moving the tree cursor"
        );
        assert_eq!(model.active_tab, 0);
        assert!(!tab_changed);
    }

    #[tokio::test]
    async fn watcher_reread_keeps_the_reader_on_their_tab() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, _tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;
        set_artifacts(&mut model, artifacts(true, true, false));
        refresh_tabs_preserving_active(&mut model);
        model.active_tab = 1; // the reader moves to Design

        set_artifacts(&mut model, artifacts(true, true, true));
        let tab_changed = refresh_tabs_preserving_active(&mut model);

        assert_eq!(model.tabs[model.active_tab], ArtifactTab::Design);
        assert!(!tab_changed, "the reader's tab still exists");
    }

    #[tokio::test]
    async fn watcher_reread_reports_a_tab_that_disappeared() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, _tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;
        set_artifacts(&mut model, artifacts(true, true, false));
        refresh_tabs_preserving_active(&mut model);
        model.active_tab = 1; // reading Design

        set_artifacts(&mut model, artifacts(true, false, false)); // design.md deleted
        let tab_changed = refresh_tabs_preserving_active(&mut model);

        assert_eq!(model.tabs, vec![ArtifactTab::Proposal]);
        assert_eq!(model.active_tab, 0);
        assert!(
            tab_changed,
            "the caller must start the replacement body at the top"
        );
    }

    #[tokio::test]
    async fn watch_load_inherits_an_outstanding_select() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;

        load_selected_artifact(&mut model, &svc, &tx, LoadTrigger::Select);
        assert_eq!(model.pending_trigger, Some(LoadTrigger::Select));

        // The watcher fires before the user's read comes back. Bumping the
        // generation cancels that read, so this one has to finish its job.
        load_selected_artifact(&mut model, &svc, &tx, LoadTrigger::Watch);

        assert_eq!(model.pending_trigger, Some(LoadTrigger::Select));
    }

    #[tokio::test]
    async fn watch_load_stays_watch_when_nothing_is_outstanding() {
        let cfg = tempdir().unwrap();
        let ws = tempdir().unwrap();
        let (svc, tx) = harness(&cfg);
        let mut model = model_on_a_change(&svc, &ws).await;

        load_selected_artifact(&mut model, &svc, &tx, LoadTrigger::Select);
        let settled = reply(&model, Ok("settled".to_string()), LoadTrigger::Select);
        update(&mut model, settled, &svc, &tx);
        assert_eq!(model.pending_trigger, None, "the reply settles the load");

        load_selected_artifact(&mut model, &svc, &tx, LoadTrigger::Watch);

        assert_eq!(model.pending_trigger, Some(LoadTrigger::Watch));
    }
}
