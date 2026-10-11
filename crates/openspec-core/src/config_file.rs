//! Crash-safe persistence for SpecForge's JSON config stores.
//!
//! Every store in the shared config directory — `workspaces.json`
//! ([`crate::WorkspaceRegistry`]), `presentation.json`
//! ([`crate::WorkspacePresentationStore`]), and `openspec-app`'s
//! `settings.json` and `review-progress.json` — saves through
//! [`write_atomic`] and has a damaged file moved aside by
//! [`preserve_corrupt_config`], so the recipe exists exactly once. The
//! presentation, settings and review-progress stores also read through
//! [`read_if_present`]; the registry still does its own missing-file check.
//! Use all three for any new config store:
//!
//! - [`write_atomic`] replaces a file so that no crash, failed write or
//!   concurrent writer can ever leave it truncated or half-written.
//! - [`read_if_present`] tells a missing file apart from one that exists but
//!   cannot be read, which a store needs before it decides whether starting
//!   from defaults may later save over the file.
//! - [`preserve_corrupt_config`] moves a file that was read but does not parse
//!   aside to a `.corrupt-<n>` sibling, so the next save cannot destroy it.
//!
//! A store that loads its file typically combines the last two:
//!
//! ```no_run
//! use openspec_core::config_file::{preserve_corrupt_config, read_if_present};
//! # let path = std::path::Path::new("settings.json");
//! match read_if_present(path) {
//!     Ok(None) => { /* missing: start from defaults, create nothing */ }
//!     Ok(Some(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
//!         Ok(_value) => { /* loaded */ }
//!         Err(_) => match preserve_corrupt_config(path) {
//!             Some(_backup) => { /* moved aside: defaults, saves allowed */ }
//!             None => { /* still in place: refuse to save over it */ }
//!         },
//!     },
//!     Err(_) => { /* unreadable: leave it alone and refuse to save over it */ }
//! }
//! ```

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Replaces the file at `path` with `bytes`, atomically.
///
/// The bytes are staged in a uniquely named temporary file in `path`'s own
/// directory — the same filesystem, so the rename is atomic, and a unique name,
/// so two processes never collide on a fixed `.tmp`. Its name starts with
/// `.<file name>.`, so a stray left by a crash says which store it belonged
/// to. The staged file is synced to disk
/// and then renamed over `path`. The directory is synced afterwards, best
/// effort, so the rename itself survives a crash. A reader, a crash or a
/// concurrent writer therefore only ever sees the previous complete file or the
/// new complete file.
///
/// The parent directory is created if it is missing. On any error the previous
/// file is left exactly as it was and the staged file is removed, and the error
/// is returned for the caller to surface: a store must never swallow it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, bytes, || Ok(()))
}

/// [`write_atomic`] with a hook run after the bytes are staged and synced and
/// before the rename. `write_atomic` passes a no-op; tests pass a failure, the
/// only deterministic way to show that a write failing at that point leaves the
/// previous file intact (file permissions cannot, since root ignores them).
fn write_atomic_with(
    path: &Path,
    bytes: &[u8],
    before_persist: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let dir = staging_dir(path);
    fs::create_dir_all(dir)?;
    // Dropping `staged` on any early return deletes the temporary file, so a
    // failed write leaves nothing behind.
    let mut staged = tempfile::Builder::new()
        .prefix(&staging_prefix(path))
        .suffix(".tmp")
        .tempfile_in(dir)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    before_persist()?;
    staged.persist(path).map_err(|e| e.error)?;
    // Best effort: the new contents are already in place and durable; this only
    // makes the directory entry durable too, and not every platform allows a
    // directory to be opened for syncing.
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// The directory a write to `path` is staged in: `path`'s parent, or the
/// current directory for a bare file name (whose parent is the empty path).
fn staging_dir(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// The staged file's name prefix: `.<file name>.`, or `.config.` for a path
/// with no file name.
fn staging_prefix(path: &Path) -> String {
    let name = path
        .file_name()
        .map_or_else(|| "config".into(), |name| name.to_string_lossy());
    format!(".{name}.")
}

/// Reads the whole file at `path`: `Ok(None)` when there is no such file,
/// `Ok(Some(bytes))` when it was read, and the error for anything else (a
/// permission error, an I/O error, a directory at `path`).
///
/// Bytes rather than a `String`, so that invalid UTF-8 is the caller's parse
/// failure — a damaged file to move aside — rather than a read failure.
pub fn read_if_present(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Moves a config file that does not parse aside to the first free
/// `<name>.corrupt-<n>` sibling, so its data stays recoverable and the next
/// save writes a new file instead of overwriting it. Used for all four stores:
/// `workspaces.json`, `presentation.json`, `settings.json` and
/// `review-progress.json`.
///
/// Returns the backup's path when the file was moved, and `None` when there was
/// nothing to move or the rename failed. A caller that gets `None` while the
/// damaged file is still at `path` must not save over it.
#[must_use = "a damaged file that could not be moved aside must not be saved over"]
pub fn preserve_corrupt_config(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    for n in 0u32..10_000 {
        let backup = sibling(path, &format!(".corrupt-{n}"));
        if !backup.exists() {
            return fs::rename(path, &backup).ok().map(|()| backup);
        }
    }
    // Numeric range exhausted (astronomically unlikely): fall back to a
    // high-entropy suffix so the corrupt file is still moved aside and never
    // left in place to be overwritten by a later save.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let backup = sibling(path, &format!(".corrupt-{nanos}"));
    fs::rename(path, &backup).ok().map(|()| backup)
}

/// `path` with `suffix` appended to its file name.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every entry in `dir`, by file name, sorted.
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn writing_into_a_missing_parent_creates_it_and_the_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("settings.json");

        write_atomic(&path, b"{\"x\":1}").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"{\"x\":1}");
        assert_eq!(entries(path.parent().unwrap()), ["settings.json"]);
    }

    #[test]
    fn a_shorter_payload_leaves_exactly_the_new_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "a much longer previous document").unwrap();

        write_atomic(&path, b"short").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"short");
        assert_eq!(entries(dir.path()), ["settings.json"]);
    }

    #[test]
    fn a_failure_before_the_rename_keeps_the_previous_file_and_no_staged_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "previous").unwrap();

        let err =
            write_atomic_with(&path, b"next", || Err(io::Error::other("injected"))).unwrap_err();

        assert_eq!(err.to_string(), "injected");
        assert_eq!(fs::read(&path).unwrap(), b"previous");
        assert_eq!(entries(dir.path()), ["settings.json"]);
    }

    #[test]
    fn persisting_onto_a_directory_fails_and_leaves_no_staged_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("inside"), "kept").unwrap();

        assert!(write_atomic(&path, b"next").is_err());

        assert!(path.is_dir());
        assert_eq!(fs::read(path.join("inside")).unwrap(), b"kept");
        assert_eq!(entries(dir.path()), ["settings.json"]);
    }

    #[test]
    fn the_staged_file_is_named_after_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut staged_names = Vec::new();
        write_atomic_with(&path, b"next", || {
            staged_names = entries(dir.path());
            Ok(())
        })
        .unwrap();
        assert_eq!(staged_names.len(), 1, "{staged_names:?}");
        assert!(
            staged_names[0].starts_with(".settings.json.")
                && Path::new(&staged_names[0]).extension() == Some("tmp".as_ref()),
            "{staged_names:?}"
        );
        assert_eq!(entries(dir.path()), ["settings.json"]);
        assert_eq!(staging_prefix(Path::new("/")), ".config.");
    }

    #[test]
    fn the_staging_dir_is_the_parent_or_the_current_directory() {
        assert_eq!(staging_dir(Path::new("a/b.json")), Path::new("a"));
        assert_eq!(staging_dir(Path::new("b.json")), Path::new("."));
    }

    #[test]
    fn read_if_present_splits_missing_present_and_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(read_if_present(&path).unwrap(), None);

        fs::write(&path, [0xff, 0xfe]).unwrap();
        assert_eq!(read_if_present(&path).unwrap(), Some(vec![0xff, 0xfe]));

        assert!(read_if_present(dir.path()).is_err());
    }

    #[test]
    fn preserve_corrupt_config_moves_file_aside_without_losing_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspaces.json");
        fs::write(&path, "{ corrupt").unwrap();

        let backup = preserve_corrupt_config(&path);

        // The corrupt file is moved aside (not deleted), so its data stays
        // recoverable and a later save cannot overwrite the original.
        let expected = dir.path().join("workspaces.json.corrupt-0");
        assert_eq!(backup.as_deref(), Some(expected.as_path()));
        assert!(
            !path.exists(),
            "corrupt file should be moved out of the way"
        );
        assert_eq!(fs::read_to_string(&expected).unwrap(), "{ corrupt");
    }

    #[test]
    fn preserve_corrupt_config_is_a_noop_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspaces.json");
        assert_eq!(preserve_corrupt_config(&path), None);
        assert!(!path.exists());
        assert_eq!(entries(dir.path()), Vec::<String>::new());
    }

    /// A file name so long that every `.corrupt-<n>` sibling exceeds the
    /// platform's file-name limit (255 bytes on macOS and Linux), so the move
    /// fails for real, with no permission games a root runner would ignore.
    const UNMOVABLE_NAME_LEN: usize = 250;

    #[test]
    fn preserve_corrupt_config_reports_a_move_that_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x".repeat(UNMOVABLE_NAME_LEN));
        fs::write(&path, "{ corrupt").unwrap();

        assert_eq!(preserve_corrupt_config(&path), None);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ corrupt");
    }

    #[test]
    fn preserve_corrupt_config_skips_an_existing_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "second damage").unwrap();
        fs::write(dir.path().join("settings.json.corrupt-0"), "first damage").unwrap();

        let backup = preserve_corrupt_config(&path);

        assert_eq!(backup, Some(dir.path().join("settings.json.corrupt-1")));
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json.corrupt-0")).unwrap(),
            "first damage"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.json.corrupt-1")).unwrap(),
            "second damage"
        );
        assert!(!path.exists());
    }
}
