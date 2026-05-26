//! macOS Dock-tile badge.
//!
//! Mirrors the tray-indicator numeric badge onto the macOS Dock tile (which
//! is the same surface rendered by the CMD+Tab application switcher), driven
//! by the same [`WatcherManager`] event stream as the tray badge so the two
//! indicators never drift apart.
//!
//! Every Dock-badge write is funnelled through [`set_dock_badge`]. If a
//! future Tauri upgrade regresses [`WebviewWindow::set_badge_count`] (see
//! `tauri-apps/tauri#13905`), or if `set_badge_count(None)` is ever found to
//! leave a stale digit on the tile, the workaround belongs inside that
//! helper — never duplicated at call sites.
//!
//! The whole module is excluded from non-macOS builds at the `mod`
//! declaration in `lib.rs`; the file-level `#![cfg(target_os = "macos")]`
//! below is belt-and-braces so accidental inclusion elsewhere still compiles
//! out.

#![cfg(target_os = "macos")]

use openspec_core::WatcherManager;
use tauri::WebviewWindow;
use tokio::sync::broadcast;

/// Apply the current active-change count to the macOS Dock tile.
///
/// `None` or `Some(0)` hides the badge; the `Some(0) → None` collapse
/// happens here so callers can pass the raw count without filtering. Returns
/// the underlying Tauri result so callers can surface set-failures, though
/// in practice the spawned updater swallows them (see
/// [`spawn_dock_badge_updater`]).
pub fn set_dock_badge(window: &WebviewWindow, count: Option<u32>) -> tauri::Result<()> {
    let count = count.filter(|&n| n > 0);
    window.set_badge_count(count.map(|n| n as i64))
}

/// Spawn the Dock-badge updater: applies the current count once, then
/// re-applies on every [`openspec_core::CacheEvent`].
///
/// Structurally mirrors `tray::spawn_badge_updater` so the two updaters read
/// as siblings. Lagged broadcasts are ignored (the next event will refresh
/// the value anyway); a closed channel ends the task.
pub fn spawn_dock_badge_updater(window: WebviewWindow, watcher: WatcherManager) {
    tauri::async_runtime::spawn(async move {
        let _ = set_dock_badge(
            &window,
            Some(watcher.total_active_logical_count() as u32),
        );

        let mut rx = watcher.subscribe();
        loop {
            match rx.recv().await {
                Ok(_) => {
                    let _ = set_dock_badge(
                        &window,
                        Some(watcher.total_active_logical_count() as u32),
                    );
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}
