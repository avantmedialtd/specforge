//! WSL (Windows Subsystem for Linux) workspace support — pure path logic.
//!
//! A Windows host reaches files inside a WSL2 distribution through the 9P
//! share at `\\wsl.localhost\<distro>\…` (or the legacy `\\wsl$\<distro>\…`).
//! This module recognises those paths, translates between the Windows UNC form
//! and the in-distro Linux form, and builds the argument vector used to run the
//! distribution's native `git` via `wsl.exe`.
//!
//! Everything here is **pure** — no Windows API, no process execution — so it
//! compiles and is unit-tested on every platform. Off Windows it is never
//! selected at runtime (a WSL path cannot occur there), hence the
//! module-level `allow(dead_code)`; see the `Windows-Scoped WSL Backend`
//! requirement.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::path::{Path, PathBuf};

/// The two UNC hosts the WSL 9P share is exposed under: the modern
/// `wsl.localhost` and the legacy `wsl$`.
const WSL_HOSTS: [&str; 2] = ["wsl.localhost", "wsl$"];

/// A parsed WSL path: the distribution it lives in and the absolute Linux path
/// inside that distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslPath {
    /// Distribution name, e.g. `Ubuntu` — the first UNC segment after the host.
    pub distro: String,
    /// Absolute Linux path inside the distribution, always starting with `/`.
    pub linux_path: String,
}

/// Which filesystem-watching backend a workspace path requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchStrategy {
    /// Event-driven OS watcher — local drive-letter / native paths.
    Native,
    /// Periodic stat sweep — required for the WSL 9P share, where the Windows
    /// directory-change API (`ReadDirectoryChangesW`) delivers no events.
    Poll,
}

/// Recognise `path` as WSL-hosted and decompose it into `(distro, linux_path)`.
///
/// Accepts the plain forms `\\wsl$\<distro>\…` and `\\wsl.localhost\<distro>\…`
/// as well as the verbatim extended-length forms `\\?\UNC\wsl…` that
/// `canonicalize` can produce. Returns `None` for any non-WSL path (including
/// other UNC shares and local drive-letter paths). The function is total — an
/// unparseable path yields `None` rather than an error.
pub fn parse_wsl_path(path: &Path) -> Option<WslPath> {
    // Work on the string form with separators normalised to `/`. This keeps
    // parsing identical on every OS — crucially, on non-Windows `Path` does not
    // treat `\` as a separator, so structural parsing must be string-based.
    let normalised = path.to_str()?.replace('\\', "/");

    // Strip an optional verbatim UNC prefix (`\\?\UNC\` → `//?/UNC/`), otherwise
    // the plain UNC lead (`//`). After this, the remainder begins with the host.
    let after_lead = normalised
        .strip_prefix("//?/UNC/")
        .or_else(|| normalised.strip_prefix("//"))?;

    for host in WSL_HOSTS {
        let Some(after_host) = after_lead.strip_prefix(host) else {
            continue;
        };
        // The host must be a whole segment: the next char is a separator.
        let Some(tail) = after_host.strip_prefix('/') else {
            continue;
        };
        if tail.is_empty() {
            return None; // host but no distribution segment
        }
        let (distro, linux_rel) = match tail.split_once('/') {
            Some((distro, rest)) => (distro, rest),
            None => (tail, ""), // `\\wsl.localhost\Ubuntu` → distro root
        };
        if distro.is_empty() {
            return None;
        }
        let trimmed = linux_rel.trim_end_matches('/');
        let linux_path = if trimmed.is_empty() {
            "/".to_string()
        } else {
            format!("/{trimmed}")
        };
        return Some(WslPath {
            distro: distro.to_string(),
            linux_path,
        });
    }
    None
}

/// Whether `path` is a WSL-hosted path.
pub fn is_wsl_path(path: &Path) -> bool {
    parse_wsl_path(path).is_some()
}

/// Rebuild the Windows UNC path for a `(distro, linux_path)` pair, using the
/// modern `\\wsl.localhost\…` host. The inverse of [`parse_wsl_path`] for the
/// distribution + Linux path it extracts.
pub fn wsl_to_unc(distro: &str, linux_path: &str) -> PathBuf {
    let relative = linux_path.trim_start_matches('/').replace('/', "\\");
    let mut unc = format!("\\\\wsl.localhost\\{distro}");
    if !relative.is_empty() {
        unc.push('\\');
        unc.push_str(&relative);
    }
    PathBuf::from(unc)
}

/// The watcher backend `path` requires: [`WatchStrategy::Poll`] for WSL paths
/// (the 9P share is deaf to OS change events), [`WatchStrategy::Native`]
/// otherwise.
pub fn watch_strategy(path: &Path) -> WatchStrategy {
    if is_wsl_path(path) {
        WatchStrategy::Poll
    } else {
        WatchStrategy::Native
    }
}

/// Build the argument vector for invoking the distribution's native `git`
/// through `wsl.exe`: `wsl.exe -d <distro> git <git_args…>`. `git_args` are the
/// arguments that would otherwise be passed to a native `git` — any path
/// arguments among them must already be in Linux form (see [`wsl_to_unc`]'s
/// inverse). Pure and testable; the actual spawn lives behind `cfg(windows)`.
pub fn wsl_git_args<'a>(distro: &'a str, git_args: &'a [&'a str]) -> Vec<String> {
    let mut args = Vec::with_capacity(git_args.len() + 3);
    args.push("-d".to_string());
    args.push(distro.to_string());
    args.push("git".to_string());
    args.extend(git_args.iter().map(|a| a.to_string()));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_modern_localhost_share() {
        let p = Path::new(r"\\wsl.localhost\Ubuntu\home\dev\project");
        let parsed = parse_wsl_path(p).expect("should parse");
        assert_eq!(parsed.distro, "Ubuntu");
        assert_eq!(parsed.linux_path, "/home/dev/project");
        assert!(is_wsl_path(p));
        assert_eq!(watch_strategy(p), WatchStrategy::Poll);
    }

    #[test]
    fn detects_legacy_wsl_dollar_share() {
        let p = Path::new(r"\\wsl$\Ubuntu\home\dev\project");
        let parsed = parse_wsl_path(p).expect("should parse");
        assert_eq!(parsed.distro, "Ubuntu");
        assert_eq!(parsed.linux_path, "/home/dev/project");
    }

    #[test]
    fn detects_verbatim_unc_form() {
        let p = Path::new(r"\\?\UNC\wsl.localhost\Ubuntu\home\dev\project");
        let parsed = parse_wsl_path(p).expect("should parse");
        assert_eq!(parsed.distro, "Ubuntu");
        assert_eq!(parsed.linux_path, "/home/dev/project");
    }

    #[test]
    fn detects_verbatim_legacy_form() {
        let p = Path::new(r"\\?\UNC\wsl$\Debian\srv\code");
        let parsed = parse_wsl_path(p).expect("should parse");
        assert_eq!(parsed.distro, "Debian");
        assert_eq!(parsed.linux_path, "/srv/code");
    }

    #[test]
    fn distro_root_has_slash_linux_path() {
        let parsed = parse_wsl_path(Path::new(r"\\wsl.localhost\Ubuntu")).expect("parse");
        assert_eq!(parsed.distro, "Ubuntu");
        assert_eq!(parsed.linux_path, "/");
    }

    #[test]
    fn local_drive_letter_is_not_wsl() {
        let p = Path::new(r"C:\Users\dev\project");
        assert!(parse_wsl_path(p).is_none());
        assert!(!is_wsl_path(p));
        assert_eq!(watch_strategy(p), WatchStrategy::Native);
    }

    #[test]
    fn other_unc_share_is_not_wsl() {
        // A real SMB share must keep the native backend — WSL-hosts-only.
        let p = Path::new(r"\\fileserver\share\project");
        assert!(parse_wsl_path(p).is_none());
        assert_eq!(watch_strategy(p), WatchStrategy::Native);
    }

    #[test]
    fn unix_path_is_not_wsl() {
        let p = Path::new("/Users/dev/project");
        assert!(parse_wsl_path(p).is_none());
        assert_eq!(watch_strategy(p), WatchStrategy::Native);
    }

    #[test]
    fn host_without_distro_is_rejected() {
        assert!(parse_wsl_path(Path::new(r"\\wsl.localhost")).is_none());
        assert!(parse_wsl_path(Path::new(r"\\wsl.localhost\")).is_none());
    }

    #[test]
    fn unc_to_linux_translation() {
        let parsed = parse_wsl_path(Path::new(r"\\wsl.localhost\Ubuntu\home\dev\project")).unwrap();
        assert_eq!(parsed.linux_path, "/home/dev/project");
        assert_eq!(parsed.distro, "Ubuntu");
    }

    #[test]
    fn linux_to_unc_translation() {
        let unc = wsl_to_unc("Ubuntu", "/home/dev/project/.git/worktrees/feature");
        assert_eq!(
            unc,
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\dev\project\.git\worktrees\feature")
        );
    }

    #[test]
    fn translation_round_trips() {
        let original = Path::new(r"\\wsl.localhost\Ubuntu\home\dev\project");
        let parsed = parse_wsl_path(original).unwrap();
        let rebuilt = wsl_to_unc(&parsed.distro, &parsed.linux_path);
        // The rebuilt UNC re-parses to the same distro + Linux path.
        let reparsed = parse_wsl_path(&rebuilt).unwrap();
        assert_eq!(parsed, reparsed);
        assert_eq!(rebuilt, original.to_path_buf());
    }

    #[test]
    fn verbatim_round_trips_to_simplified() {
        let verbatim = Path::new(r"\\?\UNC\wsl.localhost\Ubuntu\home\dev\project");
        let parsed = parse_wsl_path(verbatim).unwrap();
        let rebuilt = wsl_to_unc(&parsed.distro, &parsed.linux_path);
        // Verbatim simplifies to the plain UNC form, identifying the same place.
        assert_eq!(
            rebuilt,
            PathBuf::from(r"\\wsl.localhost\Ubuntu\home\dev\project")
        );
    }

    #[test]
    fn builds_wsl_git_argv() {
        let args = wsl_git_args("Ubuntu", &["-C", "/home/dev/project", "worktree", "list"]);
        assert_eq!(
            args,
            vec!["-d", "Ubuntu", "git", "-C", "/home/dev/project", "worktree", "list"]
        );
    }
}
