//! The non-interactive run modes: `--status` (a snapshot, pipeable) and
//! `--line` (a single ambient status line for a prompt / tmux status bar). Both
//! reuse the same `AppService` reads the interactive UI renders.

use openspec_app::AppService;
use openspec_core::WorkspaceView;

/// One glanceable line — the terminal twin of the desktop tray badge.
pub fn line(svc: &AppService) {
    let ws = svc.list_workspaces().map(|w| w.len()).unwrap_or(0);
    let active = svc.active_count();
    println!("SpecForge · {ws} workspaces · {active} open changes");
}

/// A printed snapshot of every workspace and its active changes.
pub fn status(svc: &AppService) {
    let views = svc.workspace_views();
    println!(
        "SpecForge — {} workspaces · {} open changes\n",
        views.len(),
        svc.active_count()
    );
    for view in &views {
        match view {
            WorkspaceView::Repo(r) => {
                let name = r.display_name.clone().unwrap_or_else(|| r.name.clone());
                println!("  {name}  ({} active)", r.active.len());
                for lc in &r.active {
                    let title = lc
                        .instances
                        .iter()
                        .find(|i| i.is_main_worktree)
                        .or_else(|| lc.instances.first())
                        .and_then(|i| i.change.title.clone())
                        .unwrap_or_else(|| lc.name.clone());
                    println!("    ○ {title}");
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
                println!("  {name}  ({} active)", changes.len());
                for cd in changes {
                    let title = cd.title.clone().unwrap_or_else(|| cd.change_id.clone());
                    println!("    ○ {title}");
                }
            }
        }
    }
}
