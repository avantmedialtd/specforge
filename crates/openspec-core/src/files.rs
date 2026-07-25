//! Bounded filesystem walk for enumerating markdown files in a workspace with
//! no `.gitignore` semantics to consult (a non-git root). Git repositories use
//! [`crate::git::markdown_files`] instead, which reads the index rather than
//! walking the working tree.

use std::path::{Component, Path};

/// Directory names skipped wherever they occur — common dependency/build
/// output that would otherwise be walked in full, since a non-git root has no
/// `.gitignore` to exclude them.
const JUNK_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    "__pycache__",
];

/// Defensive recursion cap: deep enough for any real project layout, shallow
/// enough to bound a pathological or cyclic-looking tree. A directory at this
/// depth is still read; its subdirectories are not descended into.
const MAX_DEPTH: u32 = 16;

/// Recursively enumerate `.md` files (case-insensitive) beneath `root`, for a
/// non-git workspace. Skips dot-prefixed entries, never follows symlinks
/// (directory or file — distinguishing the two safely would mean following
/// the link anyway, which is exactly the traversal a symlink guard exists to
/// avoid), and skips [`JUNK_DIR_NAMES`]. Returns sorted, forward-slash
/// relative paths. Best-effort: an unreadable directory is silently skipped
/// rather than failing the whole walk, mirroring `git::markdown_files`
/// degrading to an empty/partial result instead of propagating I/O errors.
pub fn walk_markdown_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    walk(root, root, 0, &mut out);
    out.sort();
    out
}

fn walk(root: &Path, dir: &Path, depth: u32, out: &mut Vec<String>) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        // `symlink_metadata` never follows the link, so a symlink (to a file
        // OR a directory) reports as neither `is_dir()` nor `is_file()` —
        // checking `is_symlink()` first is what makes "real directory" and
        // "real file" mutually exclusive below.
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if JUNK_DIR_NAMES.contains(&name_str.as_ref()) {
                continue;
            }
            walk(root, &path, depth + 1, out);
        } else if meta.is_file() && has_md_extension(&path) {
            if let Some(rel) = relative_forward_slash(root, &path) {
                out.push(rel);
            }
        }
    }
}

fn has_md_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// `path`'s location relative to `root`, joined with `/` regardless of host
/// platform. `None` if `path` isn't under `root`, or a component along the
/// way isn't a plain name — neither should happen for a walk that only ever
/// descends from `root` via `read_dir`.
fn relative_forward_slash(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "content").unwrap();
    }

    #[test]
    fn junk_directories_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "node_modules/pkg/readme.md");
        write(tmp.path(), "target/debug/notes.md");
        write(tmp.path(), "docs/guide.md");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(files, vec!["docs/guide.md".to_string()]);
    }

    #[test]
    fn dot_directories_are_skipped() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), ".hidden/secret.md");
        write(tmp.path(), ".git/COMMIT_EDITMSG.md");
        write(tmp.path(), "visible.md");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(files, vec!["visible.md".to_string()]);
    }

    #[test]
    fn nested_markdown_is_found() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "a/b/c/deep.md");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(files, vec!["a/b/c/deep.md".to_string()]);
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "README.MD");
        write(tmp.path(), "Notes.Md");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(files, vec!["Notes.Md".to_string(), "README.MD".to_string()]);
    }

    #[test]
    fn non_markdown_files_are_excluded() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "notes.md");
        write(tmp.path(), "image.png");
        write(tmp.path(), "script.rs");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(files, vec!["notes.md".to_string()]);
    }

    #[test]
    fn output_uses_forward_slashes_and_is_sorted() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "zebra/z.md");
        write(tmp.path(), "alpha/a.md");
        let files = walk_markdown_files(tmp.path());
        assert_eq!(
            files,
            vec!["alpha/a.md".to_string(), "zebra/z.md".to_string()]
        );
        assert!(files.iter().all(|f| !f.contains('\\')));
    }
}
