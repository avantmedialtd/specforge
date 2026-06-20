//! Terminal-only preferences (the colour scheme), persisted beside the shared
//! app config as `tui.json`. Kept out of the desktop's `AppSettings` because the
//! desktop never reads a terminal scheme. Best-effort: a read/write failure
//! silently falls back to the default rather than disrupting the TUI, and only
//! the config directory is ever written — never a workspace.

use std::path::Path;

use crate::theme::Scheme;

/// Read the persisted colour scheme from `<config_dir>/tui.json`, or `None` when
/// the file is absent/unreadable or names an unknown scheme (caller keeps the
/// default).
pub fn load_scheme(config_dir: &Path) -> Option<Scheme> {
    let raw = std::fs::read_to_string(config_dir.join("tui.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Scheme::from_key(value.get("colorScheme")?.as_str()?)
}

/// Persist the chosen colour scheme to `<config_dir>/tui.json`. Best-effort.
pub fn save_scheme(config_dir: &Path, scheme: Scheme) {
    let _ = std::fs::create_dir_all(config_dir);
    let body = serde_json::json!({ "colorScheme": scheme.key() });
    if let Ok(raw) = serde_json::to_string_pretty(&body) {
        let _ = std::fs::write(config_dir.join("tui.json"), raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scheme_round_trips_through_the_pref_file() {
        let dir = tempdir().unwrap();
        assert_eq!(load_scheme(dir.path()), None, "absent file → default");
        save_scheme(dir.path(), Scheme::Nord);
        assert_eq!(load_scheme(dir.path()), Some(Scheme::Nord));
        save_scheme(dir.path(), Scheme::Native);
        assert_eq!(load_scheme(dir.path()), Some(Scheme::Native));
    }

    #[test]
    fn unknown_key_falls_back_to_default() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("tui.json"), r#"{"colorScheme":"bogus"}"#).unwrap();
        assert_eq!(load_scheme(dir.path()), None);
    }
}
