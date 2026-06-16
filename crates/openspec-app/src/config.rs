//! Resolution of the shared application configuration directory.
//!
//! Both frontends MUST read and write the same on-disk stores (registry,
//! settings, presentation, activity log), so the path is resolved here, once,
//! to exactly what the Tauri shell historically used: the platform config
//! directory joined with the bundle identifier. This is the same computation
//! Tauri v2's `app_config_dir()` performs (`dirs::config_dir()` + identifier),
//! so switching the shell to this resolver is path-preserving while giving the
//! terminal frontend — which has no Tauri — the identical location.
//!
//! This deliberately uses the low-level `dirs::config_dir()` rather than
//! `directories::ProjectDirs`, whose `qualifier.org.app` scheme diverges from
//! the identifier path on Linux (`~/.config/specforge` vs
//! `~/.config/com.avantmedia.specforge`).

use std::path::PathBuf;

/// The application bundle identifier — the leaf of the config directory.
/// Mirrors `tauri.conf.json`'s `identifier`; keep the two in sync.
pub const APP_IDENTIFIER: &str = "com.avantmedia.specforge";

/// The shared application configuration directory:
/// `dirs::config_dir()/com.avantmedia.specforge`.
///
/// - macOS: `~/Library/Application Support/com.avantmedia.specforge`
/// - Linux: `${XDG_CONFIG_HOME:-~/.config}/com.avantmedia.specforge`
/// - Windows: `%APPDATA%\com.avantmedia.specforge`
///
/// Returns `None` only when the platform config directory cannot be determined
/// (e.g. no `$HOME`).
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|base| base.join(APP_IDENTIFIER))
}
