//! Bounded filesystem walk for enumerating markdown files in a workspace with
//! no `.gitignore` semantics to consult (a non-git root). Git repositories use
//! [`crate::git::markdown_files`] instead, which reads the index rather than
//! walking the working tree.

use crate::types::{WorkspaceFileCopy, WorkspaceFileRow};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hasher;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

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

/// Pool per-worktree markdown listings into one row per **root-relative path**.
///
/// Input is one `(worktree path, that worktree's markdown enumeration)` pair
/// per tracked worktree of a top-level row; output is the de-duplicated union
/// the file browser lists (`workspace-file-browser`: *Union Markdown Listing
/// Across a Repository's Worktrees*). Pure — no I/O, no clock, no filesystem;
/// [`mark_divergent_rows`] adds the `differs` marker afterwards, which is what
/// lets the tree render from the path list before divergence is known.
///
/// **The de-duplication key is the root-relative path** (design D1). It is what
/// the enumeration already returns and what the file address already carries,
/// so one path is one row however many worktrees hold it. Identity by content
/// would split a path that two branches disagree about into two rows, which is
/// precisely the shape a folder tree cannot express; identity by absolute path
/// would collapse nothing at all.
///
/// Ordering is a deterministic **total** order at both levels, so the listing
/// never reorders between two reads of unchanged content:
///
/// - rows: by path ascending, matching the sorted listing a single browse root
///   already returns (ids are unique per row, so the order is total);
/// - copies: by worktree path ascending, which is total because a worktree
///   contributes each path at most once — the per-worktree enumerations are
///   themselves de-duplicated, and the set below removes a repeat should a
///   caller pass one worktree twice.
///
/// Both orders come from ordered containers rather than a sort comparator, so
/// there is no ordering closure to get wrong and none to leave untested.
pub fn group_workspace_file_rows(listings: Vec<(PathBuf, Vec<String>)>) -> Vec<WorkspaceFileRow> {
    let mut by_path: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    for (worktree_path, paths) in listings {
        for path in paths {
            by_path
                .entry(path)
                .or_default()
                .insert(worktree_path.clone());
        }
    }
    by_path
        .into_iter()
        .map(|(path, worktrees)| WorkspaceFileRow {
            path,
            copies: worktrees
                .into_iter()
                .map(|worktree_path| WorkspaceFileCopy { worktree_path })
                .collect(),
            // Nothing has been read yet. A row is marked only by
            // `mark_divergent_rows`, and only after a difference is observed.
            differs: false,
        })
        .collect()
}

/// Chunk size for the streaming content hash below (64 KiB). Big enough that a
/// typical markdown file is one read, small enough that a pathologically large
/// one never lands in memory whole.
///
/// Written as a literal rather than `64 * 1024` deliberately. The value is a
/// read-buffer size, so it cannot change any row's `differs` verdict — one
/// `DefaultHasher` is fed the whole byte stream, and every copy compared in a
/// run uses the same constant, so chunking changes only the number of read
/// syscalls. An arithmetic expression here therefore mints a mutant
/// (`64 * 1024` -> `64 + 1024`) that no test can kill without asserting the
/// constant's own value, which would test the constant rather than the
/// product. A literal has nothing to mutate, which is a better answer than
/// suppressing the mutant by line number.
const DIVERGENCE_CHUNK_BYTES: usize = 65_536;

/// Set `differs` on every row whose copies are not all byte-identical
/// (`workspace-file-browser`: *A file differing between worktrees is marked*).
///
/// # Why content, and not size+mtime (tasks.md 1.4)
///
/// Measured against this repository (666 markdown files, three tracked
/// worktrees) before choosing:
///
/// | work                                        | cost   |
/// |---------------------------------------------|--------|
/// | 3 × `git ls-files` (the enumeration itself)  | 157 ms |
/// | read + hash 3 × 666 files (14.5 MB)         | 141 ms |
/// | `stat` 3 × 666 files                        |  10 ms |
///
/// So the exact answer costs *less than the listing it accompanies*, measured
/// in a slower language than the one that runs it. "A content hash may dominate
/// the listing" turned out not to hold at this repository's scale, which
/// removes the only argument for the cheap-and-wrong option.
///
/// And the cheap option is wrong in the direction that matters. `mtime` is
/// checkout time for every file in a freshly created worktree, so size+mtime
/// would mark the whole tree as divergent the day a worktree is added and stay
/// wrong until each file is touched. This marker is *the signal* the file
/// browser exists to surface (design D2) — a marker that fires on every row
/// after a `git checkout` is worse than no marker, because it teaches the
/// reader to ignore it. Size alone has the opposite failure: a same-length edit
/// (the common one-word fix) reads as identical.
///
/// Size is still used, as a **fast reject**: two copies of different lengths
/// differ, and no bytes need to be read to know it. That is the case the marker
/// exists for — branches that actually drifted — so the exact path is also the
/// cheap one whenever the answer is "yes".
///
/// A copy that cannot be read contributes no evidence and the row stays
/// unmarked: the marker asserts that a difference *was observed*, never that
/// one could not be ruled out. That also keeps this consistent with the
/// listing's own stance on an unreadable worktree, which contributes nothing
/// rather than failing.
pub fn mark_divergent_rows(rows: &mut [WorkspaceFileRow]) {
    for row in rows.iter_mut() {
        if row.copies.len() < 2 {
            // Single-copy rows are never marked — there is nothing to differ
            // from — and skipping them here is also what keeps the cost
            // proportional to the pooled part of the listing rather than to
            // all of it.
            continue;
        }
        let paths: Vec<PathBuf> = row
            .copies
            .iter()
            .map(|c| c.worktree_path.join(&row.path))
            .collect();
        row.differs = copies_differ(&paths);
    }
}

/// Whether the files at `paths` are not all byte-identical. `false` when any of
/// them cannot be read (no evidence either way) or when there are fewer than
/// two.
fn copies_differ(paths: &[PathBuf]) -> bool {
    let mut sizes = Vec::with_capacity(paths.len());
    for path in paths {
        match std::fs::metadata(path) {
            Ok(m) => sizes.push(m.len()),
            Err(_) => return false,
        }
    }
    // Fast reject: different lengths is different content, with nothing read.
    if sizes.windows(2).any(|w| w[0] != w[1]) {
        return true;
    }
    let mut digests = Vec::with_capacity(paths.len());
    for path in paths {
        match content_digest(path) {
            Some(d) => digests.push(d),
            None => return false,
        }
    }
    digests.windows(2).any(|w| w[0] != w[1])
}

/// A digest of the file's bytes, streamed so a large file never lands in memory
/// whole. `None` when the file cannot be read.
///
/// The hash needs no cryptographic strength and gets no new dependency: it is
/// only ever compared against digests computed in the same process, over the
/// handful of copies of one path, so a collision would need two same-length
/// files of one path to collide in 64 bits within a single listing.
fn content_digest(path: &Path) -> Option<u64> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = DefaultHasher::new();
    let mut buf = vec![0u8; DIVERGENCE_CHUNK_BYTES];
    loop {
        let read = file.read(&mut buf).ok()?;
        if read == 0 {
            return Some(hasher.finish());
        }
        hasher.write(&buf[..read]);
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

    // ---- group_workspace_file_rows (the pooled union) -------------------
    //
    // Every assertion below pins the EXACT sequence — rows and copies — not a
    // length or a membership test. The mutation gate replaces whole functions,
    // so a grouping that returned the right rows in the wrong order, or the
    // right copies in insertion order, has to be caught by an ordering
    // assertion or it is not caught at all.

    fn listing(worktree: &str, paths: &[&str]) -> (PathBuf, Vec<String>) {
        (
            PathBuf::from(worktree),
            paths.iter().map(|p| p.to_string()).collect(),
        )
    }

    /// `(path, copies)` in output order, for comparing a whole listing at once.
    fn shape(rows: &[WorkspaceFileRow]) -> Vec<(String, Vec<String>)> {
        rows.iter()
            .map(|r| {
                (
                    r.path.clone(),
                    r.copies
                        .iter()
                        .map(|c| c.worktree_path.to_string_lossy().into_owned())
                        .collect(),
                )
            })
            .collect()
    }

    /// A path in exactly one worktree is a row like any other, and a path in
    /// all three collapses to one row carrying all three copies
    /// (`workspace-file-browser`: *A file in only one worktree is listed*,
    /// *The same path in several worktrees is one row*).
    #[test]
    fn union_collapses_shared_paths_and_keeps_solitary_ones() {
        let rows = group_workspace_file_rows(vec![
            listing("/wt/b", &["docs/guide.md", "only-b.md"]),
            listing("/wt/a", &["docs/guide.md"]),
            listing("/wt/c", &["docs/guide.md", "only-c.md"]),
        ]);
        assert_eq!(
            shape(&rows),
            vec![
                (
                    "docs/guide.md".to_string(),
                    vec![
                        "/wt/a".to_string(),
                        "/wt/b".to_string(),
                        "/wt/c".to_string()
                    ]
                ),
                ("only-b.md".to_string(), vec!["/wt/b".to_string()]),
                ("only-c.md".to_string(), vec!["/wt/c".to_string()]),
            ]
        );
        // Nothing has been read, so nothing is marked.
        assert!(rows.iter().all(|r| !r.differs));
    }

    /// Two worktrees with no path in common: every row is a single-copy row,
    /// and the rows interleave by path rather than by which worktree was
    /// enumerated first.
    #[test]
    fn disjoint_worktrees_interleave_by_path() {
        let rows = group_workspace_file_rows(vec![
            listing("/wt/second", &["b.md", "d.md"]),
            listing("/wt/first", &["a.md", "c.md"]),
        ]);
        assert_eq!(
            shape(&rows),
            vec![
                ("a.md".to_string(), vec!["/wt/first".to_string()]),
                ("b.md".to_string(), vec!["/wt/second".to_string()]),
                ("c.md".to_string(), vec!["/wt/first".to_string()]),
                ("d.md".to_string(), vec!["/wt/second".to_string()]),
            ]
        );
    }

    /// The orders are TOTAL, not merely stable: both the listings and the paths
    /// within them arrive in exactly the reverse of the output order, so a
    /// grouping that preserved insertion order — or that sorted only one of the
    /// two levels — produces a different sequence here.
    #[test]
    fn row_and_copy_order_are_independent_of_input_order() {
        let rows = group_workspace_file_rows(vec![
            listing("/wt/zulu", &["z.md", "m.md", "a.md"]),
            listing("/wt/mike", &["z.md", "m.md", "a.md"]),
            listing("/wt/alpha", &["z.md", "m.md", "a.md"]),
        ]);
        let expected_copies = vec![
            "/wt/alpha".to_string(),
            "/wt/mike".to_string(),
            "/wt/zulu".to_string(),
        ];
        assert_eq!(
            shape(&rows),
            vec![
                ("a.md".to_string(), expected_copies.clone()),
                ("m.md".to_string(), expected_copies.clone()),
                ("z.md".to_string(), expected_copies),
            ]
        );
    }

    /// One worktree listed twice — or listing one path twice — is one copy.
    /// The tie the copy order would otherwise have to break is removed rather
    /// than resolved, which is what makes "by worktree path ascending" a total
    /// order instead of a stable one.
    #[test]
    fn a_repeated_worktree_contributes_one_copy() {
        let rows = group_workspace_file_rows(vec![
            listing("/wt/a", &["dup.md", "dup.md"]),
            listing("/wt/a", &["dup.md"]),
        ]);
        assert_eq!(
            shape(&rows),
            vec![("dup.md".to_string(), vec!["/wt/a".to_string()])]
        );
    }

    /// The degenerate cases: no worktrees at all, and a worktree with no
    /// markdown, both yield an empty listing rather than an empty-path row.
    #[test]
    fn empty_input_yields_no_rows() {
        assert!(group_workspace_file_rows(vec![]).is_empty());
        assert!(group_workspace_file_rows(vec![listing("/wt/a", &[])]).is_empty());
    }

    // ---- mark_divergent_rows (the divergence marker) --------------------

    /// Build rows over `n` real worktree directories under `tmp`, each holding
    /// `rel` with the given content — `None` for a worktree that does not hold
    /// the file at all.
    fn rows_for(tmp: &Path, rel: &str, contents: &[Option<&str>]) -> Vec<WorkspaceFileRow> {
        let mut listings = Vec::new();
        for (i, content) in contents.iter().enumerate() {
            let wt = tmp.join(format!("wt{i}"));
            fs::create_dir_all(wt.join(rel).parent().unwrap()).unwrap();
            match content {
                Some(body) => {
                    fs::write(wt.join(rel), body).unwrap();
                    listings.push((wt, vec![rel.to_string()]));
                }
                None => listings.push((wt, Vec::new())),
            }
        }
        group_workspace_file_rows(listings)
    }

    #[test]
    fn identical_copies_are_not_marked() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(tmp.path(), "docs/guide.md", &[Some("same"), Some("same")]);
        mark_divergent_rows(&mut rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].copies.len(), 2);
        assert!(!rows[0].differs);
    }

    /// The case a size-only comparison would miss: the copies differ by one
    /// character and are therefore exactly as long as each other.
    #[test]
    fn same_length_differing_copies_are_marked() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(tmp.path(), "docs/guide.md", &[Some("cat"), Some("bat")]);
        mark_divergent_rows(&mut rows);
        assert!(rows[0].differs);
    }

    /// The fast-reject path: different lengths is different content.
    #[test]
    fn differently_sized_copies_are_marked() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(
            tmp.path(),
            "docs/guide.md",
            &[Some("short"), Some("longer!")],
        );
        mark_divergent_rows(&mut rows);
        assert!(rows[0].differs);
    }

    /// Three copies where only the third disagrees — a comparison that stopped
    /// after the first pair would call this identical.
    #[test]
    fn a_third_copy_that_disagrees_marks_the_row() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(
            tmp.path(),
            "docs/guide.md",
            &[Some("same"), Some("same"), Some("diff")],
        );
        mark_divergent_rows(&mut rows);
        assert_eq!(rows[0].copies.len(), 3);
        assert!(rows[0].differs);
    }

    /// A single-copy row is never marked, even when other worktrees exist and
    /// hold entirely different files — there is nothing for it to differ from.
    #[test]
    fn a_single_copy_row_is_never_marked() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(tmp.path(), "solo.md", &[Some("only here"), None]);
        mark_divergent_rows(&mut rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].copies.len(), 1);
        assert!(!rows[0].differs);
    }

    /// A copy that cannot be read is no evidence of a difference, so the row
    /// stays unmarked rather than asserting a divergence nobody observed.
    #[test]
    fn an_unreadable_copy_leaves_the_row_unmarked() {
        let tmp = TempDir::new().unwrap();
        let mut rows = rows_for(tmp.path(), "docs/guide.md", &[Some("here"), Some("there!")]);
        // Delete the second copy out from under the listing: `differs` would be
        // true for these two bodies if both were readable.
        fs::remove_file(tmp.path().join("wt1").join("docs/guide.md")).unwrap();
        mark_divergent_rows(&mut rows);
        assert!(!rows[0].differs);
    }
}
