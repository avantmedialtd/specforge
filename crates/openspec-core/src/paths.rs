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
    // The tail is non-empty whenever this line is reached: `existing` is a
    // strict ancestor of `path` unless `path` itself is a directory, and a
    // directory that exists was already answered by the early return above.
    // Guarding for an empty tail would therefore be dead code — and
    // indistinguishable dead code, since `join("")` compares equal to the
    // path it is joined to.
    let tail = path.strip_prefix(&existing).unwrap_or(Path::new(""));
    base.join(tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path that exists resolves exactly as `canonicalize` would — the
    /// prefix logic must not change the answer for the ordinary case.
    #[test]
    fn an_existing_path_resolves_to_its_canonical_form() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.md"), "# a").unwrap();

        assert_eq!(
            canonicalize_existing_prefix(&root.join("a.md")),
            canonicalize(&root.join("a.md")).unwrap()
        );
    }

    /// The case `canonicalize` cannot answer: the file is not there. The
    /// existing prefix still resolves, and the missing tail is re-appended.
    #[test]
    fn a_missing_file_keeps_its_name_under_the_resolved_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        assert_eq!(
            canonicalize_existing_prefix(&root.join("gone.md")),
            root.join("gone.md")
        );
    }

    /// Several missing components, not just one — the tail is a path, not a
    /// file name.
    #[test]
    fn a_missing_subtree_keeps_every_missing_component() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();

        assert_eq!(
            canonicalize_existing_prefix(&root.join("a/b/c.md")),
            root.join("a").join("b").join("c.md")
        );
    }

    /// An existing DIRECTORY has no tail to re-append; the result must be the
    /// directory itself rather than the directory joined with nothing.
    #[test]
    fn an_existing_directory_resolves_to_itself() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();

        assert_eq!(
            canonicalize_existing_prefix(&root.join("sub")),
            root.join("sub")
        );
    }

    /// The containment check this feeds depends on symlinks being FOLLOWED for
    /// the part of the path that exists — otherwise a link out of a workspace
    /// would pass a guard that compares against the canonical root.
    #[cfg(unix)]
    #[test]
    fn an_existing_symlink_is_resolved_so_containment_can_see_through_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("real")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

        // Through the link, naming a file that does not exist yet.
        assert_eq!(
            canonicalize_existing_prefix(&root.join("link/new.md")),
            root.join("real").join("new.md")
        );
    }

    #[test]
    fn deepest_existing_dir_walks_up_to_what_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("a")).unwrap();

        assert_eq!(deepest_existing_dir(&root.join("a")), root.join("a"));
        assert_eq!(deepest_existing_dir(&root.join("a/b/c")), root.join("a"));
        assert_eq!(deepest_existing_dir(&root), root);
    }

    /// A path with nothing existing anywhere in its chain comes back unchanged,
    /// so the caller's attempt fails loudly rather than against a guess.
    #[test]
    fn a_wholly_absent_relative_path_is_returned_unchanged() {
        let path = Path::new("no/such/place.md");
        assert_eq!(deepest_existing_dir(path), path);
    }
}
