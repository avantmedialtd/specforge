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
