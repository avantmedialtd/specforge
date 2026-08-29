//! Path canonicalisation helper.
//!
//! Wraps [`dunce::canonicalize`] so the whole crate resolves paths the same
//! way. On Windows `dunce` strips the verbatim extended-length prefix (`\\?\`,
//! `\\?\UNC\…`) whenever the simplified form is equivalent, so a repository
//! reached through differently-shaped-but-equivalent paths always yields one
//! canonical representation — and therefore one `RepoId`. On other platforms it
//! is exactly `std::fs::canonicalize`. Routing every canonicalisation through
//! here is what keeps WSL repos from being split into two identities.
use std::io;
use std::path::{Path, PathBuf};

/// Canonicalise `path`, normalising Windows verbatim/UNC forms to a single
/// simplified representation. See the module docs for why this matters.
pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
}

/// The deepest ancestor of `dir` (including `dir` itself) that exists as a
/// directory. Falls back to `dir` when nothing in the chain does, so a caller
/// always gets a path to attempt and the attempt fails loudly rather than this
/// guessing.
///
/// Driven by `PathBuf::pop`, which strictly shortens the path and reports when
/// there is nothing left to remove, so termination is a property of the loop.
pub fn deepest_existing_dir(dir: &Path) -> PathBuf {
    let mut candidate = dir.to_path_buf();
    while !candidate.is_dir() {
        if !candidate.pop() {
            return dir.to_path_buf();
        }
    }
    candidate
}

/// Canonicalise as much of `path` as exists on disk, then re-append the
/// components that do not.
///
/// `canonicalize` fails outright on a path whose final component is missing,
/// which is right for a read and wrong for anything that has to reason about a
/// path that is not there *yet* — a watch on a document that may come back, or
/// a containment check for one that has gone. Resolving the existing prefix
/// keeps such a check honest, because every symlink that actually exists is
/// still followed and still has to land inside the root.
///
/// Shared deliberately: the workspace read guard and the document watch both
/// answer this question about the same paths, and two independently-written
/// resolvers would be free to disagree about symlinks or UNC forms while both
/// looking correct.
pub fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    if let Ok(canonical) = canonicalize(path) {
        return canonical;
    }
    let existing = deepest_existing_dir(path);
    let Ok(base) = canonicalize(&existing) else {
        return path.to_path_buf();
    };
    match path.strip_prefix(&existing) {
        Ok(tail) if !tail.as_os_str().is_empty() => base.join(tail),
        _ => base,
    }
}
