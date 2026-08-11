//! Desktop-notification dispatcher.
//!
//! Subscribes to [`openspec_core::CacheEvent`] and fires a desktop
//! notification only on structural transitions — new active change or
//! a change moving into `archive/`. Plain `Updated` events (file edits
//! within existing changes) never notify, per spec.
//!
//! The dispatcher honours the in-app notifications-enabled toggle. If
//! the user turns notifications off in settings, events still flow but
//! no OS notification is shown.

use openspec_app::{row_key_for_workspace, SettingsStore};
use openspec_core::presentation::{PresentationKey, WorkspacePresentationStore};
use openspec_core::{CacheEvent, WatcherManager, WorkspaceRegistry};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::broadcast;

/// Spawn the dispatcher task. Returns immediately.
///
/// `registry` and `presentation` are needed only to decide whether the row an
/// event belongs to has been parked by the user; see [`dispatch`].
pub fn spawn_notification_dispatcher(
    app: AppHandle,
    watcher: &WatcherManager,
    settings: Arc<SettingsStore>,
    registry: Arc<Mutex<WorkspaceRegistry>>,
    presentation: Arc<Mutex<WorkspacePresentationStore>>,
) {
    let mut rx = watcher.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => dispatch(&app, &settings, &registry, &presentation, event),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

fn dispatch(
    app: &AppHandle,
    settings: &SettingsStore,
    registry: &Mutex<WorkspaceRegistry>,
    presentation: &Mutex<WorkspacePresentationStore>,
    event: CacheEvent,
) {
    let Some((title, body)) = notification_for(event, registry, presentation) else {
        return;
    };

    // Check setting last so the lock is held only briefly and never across the
    // OS notification call.
    if !settings.snapshot().notifications_enabled {
        return;
    }

    let _ = app.notification().builder().title(title).body(&body).show();
}

/// The notification `event` should produce, or `None` when it must be silent —
/// either because the event is not a logical transition, or because the row it
/// belongs to has been parked by the user.
///
/// Split out from [`dispatch`] so the suppression rules are unit-testable
/// without a Tauri `AppHandle`.
fn notification_for(
    event: CacheEvent,
    registry: &Mutex<WorkspaceRegistry>,
    presentation: &Mutex<WorkspacePresentationStore>,
) -> Option<(&'static str, String)> {
    // Notifications are scoped to *logical* transitions only — a new change
    // appearing anywhere in a repository, or every instance of a change
    // being archived. Per-instance churn (the harness opening or closing
    // ephemeral worktrees) is intentionally silent.
    let (title, body, row) = match event {
        CacheEvent::LogicalChangeAdded {
            repo_id,
            change_name,
        } => (
            "New change",
            format!("{} · {}", display_name(&repo_id), change_name),
            PresentationKey::Repo(repo_id),
        ),
        CacheEvent::LogicalChangeArchived {
            repo_id,
            change_name,
        } => (
            "Change archived",
            format!("{} · {}", display_name(&repo_id), change_name),
            PresentationKey::Repo(repo_id),
        ),
        // Non-git workspaces still emit the original instance-grained events;
        // forward those so users with workspaces outside any git repo continue
        // to get notifications.
        CacheEvent::ChangeAdded {
            workspace,
            change_id,
        } => {
            let row = row_key_for_workspace(registry, &workspace);
            (
                "New change",
                format!("{} · {}", display_name(&workspace), change_id),
                row,
            )
        }
        CacheEvent::ChangeArchived {
            workspace,
            change_id,
        } => {
            let row = row_key_for_workspace(registry, &workspace);
            (
                "Change archived",
                format!("{} · {}", display_name(&workspace), change_id),
                row,
            )
        }
        // All other events are silent by design — including GraphChanged,
        // which is a pure git-history signal with no OpenSpec transition.
        CacheEvent::Updated { .. }
        | CacheEvent::WorkspaceRemoved { .. }
        | CacheEvent::InstanceAdded { .. }
        | CacheEvent::InstanceRemoved { .. }
        | CacheEvent::GraphChanged { .. }
        | CacheEvent::QuotaUpdated => return None,
    };

    // A parked row is silent. Suppression is dispatch-only and deliberately so:
    // the event has already flowed on the broadcast channel and the achievement
    // is already in the activity log, so un-parking replays nothing and the
    // Dashboard's shipped haul is unaffected.
    let parked = presentation
        .lock()
        .map(|store| store.is_disabled(&row))
        .unwrap_or(false);
    if parked {
        return None;
    }

    Some((title, body))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openspec_core::presentation::PresentationKey;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// An empty registry: every event resolves to a `Flat` row key, which is the
    /// shape the non-git notification path uses.
    fn fixtures(tmp: &TempDir) -> (Mutex<WorkspaceRegistry>, Mutex<WorkspacePresentationStore>) {
        (
            Mutex::new(WorkspaceRegistry::new(tmp.path().join("workspaces.json"))),
            Mutex::new(WorkspacePresentationStore::new(
                tmp.path().join("presentation.json"),
            )),
        )
    }

    fn logical_added(repo: &str) -> CacheEvent {
        CacheEvent::LogicalChangeAdded {
            repo_id: PathBuf::from(repo),
            change_name: "foo".into(),
        }
    }

    fn logical_archived(repo: &str) -> CacheEvent {
        CacheEvent::LogicalChangeArchived {
            repo_id: PathBuf::from(repo),
            change_name: "foo".into(),
        }
    }

    #[test]
    fn an_enabled_repository_still_notifies() {
        let tmp = TempDir::new().unwrap();
        let (reg, pres) = fixtures(&tmp);
        assert_eq!(
            notification_for(logical_added("/r/.git"), &reg, &pres),
            Some(("New change", ".git · foo".to_string()))
        );
        assert_eq!(
            notification_for(logical_archived("/r/.git"), &reg, &pres).map(|(t, _)| t),
            Some("Change archived")
        );
    }

    #[test]
    fn a_parked_repository_is_silent_for_both_transitions() {
        let tmp = TempDir::new().unwrap();
        let (reg, pres) = fixtures(&tmp);
        pres.lock()
            .unwrap()
            .set_disabled(PresentationKey::repo("/r/.git"), true)
            .unwrap();

        assert_eq!(
            notification_for(logical_added("/r/.git"), &reg, &pres),
            None,
            "a new change in a parked repository must not notify"
        );
        assert_eq!(
            notification_for(logical_archived("/r/.git"), &reg, &pres),
            None,
            "an archive transition in a parked repository must not notify"
        );
        // Control: parking one repository must not silence another.
        assert!(
            notification_for(logical_added("/other/.git"), &reg, &pres).is_some(),
            "an unrelated repository must still notify"
        );
    }

    #[test]
    fn a_parked_flat_workspace_is_silent() {
        let tmp = TempDir::new().unwrap();
        let (reg, pres) = fixtures(&tmp);
        let ws = tmp.path().join("flat");
        pres.lock()
            .unwrap()
            .set_disabled(PresentationKey::flat(ws.clone()), true)
            .unwrap();

        let added = CacheEvent::ChangeAdded {
            workspace: ws.clone(),
            change_id: "foo".into(),
        };
        let archived = CacheEvent::ChangeArchived {
            workspace: ws,
            change_id: "foo".into(),
        };
        assert_eq!(notification_for(added, &reg, &pres), None);
        assert_eq!(notification_for(archived, &reg, &pres), None);

        // Control: a different flat workspace is unaffected.
        let other = CacheEvent::ChangeAdded {
            workspace: tmp.path().join("other"),
            change_id: "foo".into(),
        };
        assert!(notification_for(other, &reg, &pres).is_some());
    }

    #[test]
    fn non_transition_events_stay_silent_regardless() {
        let tmp = TempDir::new().unwrap();
        let (reg, pres) = fixtures(&tmp);
        for event in [
            CacheEvent::Updated {
                workspace: PathBuf::from("/r"),
            },
            CacheEvent::QuotaUpdated,
        ] {
            assert_eq!(notification_for(event, &reg, &pres), None);
        }
    }
}
