//! Integration coverage of [`openspec_core::markdown_files`] against a real
//! git repository, verifying gitignore semantics end-to-end. Unit tests for
//! the non-git-root fallback (`walk_markdown_files`) live directly in
//! `openspec_core::files`.

use openspec_core::markdown_files;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git invocation");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_repo(root: &Path) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    git(&["init", "-b", "main"], root);
    git(&["config", "user.email", "t@t"], root);
    git(&["config", "user.name", "t"], root);
    root.canonicalize().unwrap()
}

#[test]
fn markdown_files_respects_gitignore_and_extension() {
    let tmp = TempDir::new().unwrap();
    let root = init_repo(&tmp.path().join("repo"));

    // A gitignored directory containing markdown — must never appear, and
    // ls-files should never need to look inside it.
    fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
    fs::create_dir_all(root.join("ignored")).unwrap();
    fs::write(root.join("ignored/notes.md"), "ignored").unwrap();

    // A tracked (committed) markdown file.
    fs::write(root.join("tracked.md"), "tracked").unwrap();
    git(&["add", ".gitignore", "tracked.md"], &root);
    git(&["commit", "-m", "init"], &root);

    // An untracked-but-not-ignored markdown draft.
    fs::write(root.join("draft.md"), "draft").unwrap();

    // A tracked non-markdown file — must never appear.
    fs::write(root.join("readme.txt"), "text").unwrap();
    git(&["add", "readme.txt"], &root);
    git(&["commit", "-m", "add text file"], &root);

    let files = markdown_files(&root).expect("git ls-files should succeed");

    assert_eq!(
        files,
        vec!["draft.md".to_string(), "tracked.md".to_string()],
        "expected only the tracked + untracked markdown, sorted, excluding the \
         gitignored and non-markdown paths"
    );
}
