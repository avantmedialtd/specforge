//! The diff model's two parsers and its line and byte budgets, over byte
//! fixtures (`diff-view`: *Diff Model*, *Line and Byte Budgets With
//! On-Request Loading*).
//!
//! Every fixture is a byte-string literal, or is built in code where only its
//! size matters, so an editor that trims trailing whitespace or re-encodes
//! this file cannot change what a test reads. Most are raw (`br#"…"#`), where
//! `\ No newline at end of file` and git's `\303\251` quoting read as git
//! writes them. A fixture that needs a byte a raw literal cannot pin — a
//! trailing space, a tab, a byte outside ASCII — is an ordinary `b"…"`
//! literal that escapes it (`\x20`, `\t`, `\xe9`).
//!
//! The budget boundaries are written out by hand. The mutation gate cannot be
//! trusted to mutate a comparator, so these cases are what pin `<=` against
//! `<`.

use openspec_core::diff::{
    eager_by_lines, eager_files, parse_diff, parse_hunks, withhold_files, ByteBudget, DiffContent,
    DiffFile, FileStatus, Hunk, Line, LineKind, PatchSize, EAGER_LINES_LIMIT,
    EAGER_PATCH_BYTES_LIMIT, FILE_LINES_LIMIT, FILE_PATCH_BYTES_LIMIT, REQUESTED_FILE_BYTES_LIMIT,
    STREAMED_READ_BYTES_LIMIT,
};

// ------------------------------------------------------------------- helpers

fn context(old_no: u32, new_no: u32, text: &str) -> Line {
    Line {
        kind: LineKind::Context,
        old_no: Some(old_no),
        new_no: Some(new_no),
        text: text.to_string(),
        no_newline: false,
    }
}

fn removed(old_no: u32, text: &str) -> Line {
    Line {
        kind: LineKind::Removed,
        old_no: Some(old_no),
        new_no: None,
        text: text.to_string(),
        no_newline: false,
    }
}

fn added(new_no: u32, text: &str) -> Line {
    Line {
        kind: LineKind::Added,
        old_no: None,
        new_no: Some(new_no),
        text: text.to_string(),
        no_newline: false,
    }
}

/// `line`, flagged as ending its file without a newline.
fn at_eof(line: Line) -> Line {
    Line {
        no_newline: true,
        ..line
    }
}

/// A hunk with no section heading.
fn hunk(old: (u32, u32), new: (u32, u32), lines: Vec<Line>) -> Hunk {
    Hunk {
        old_start: old.0,
        old_lines: old.1,
        new_start: new.0,
        new_lines: new.1,
        section: None,
        lines,
    }
}

/// An owned path or mode.
fn some(text: &str) -> Option<String> {
    Some(text.to_string())
}

fn hunks_of(file: &DiffFile) -> &[Hunk] {
    match &file.content {
        DiffContent::Hunks { hunks } => hunks,
        other => panic!("expected hunks, found {other:?}"),
    }
}

// ------------------------------------------------------- parse_hunks fixtures

/// `@@ -10,3 +10,4 @@ fn main()` holding a context line, a removed line, two
/// added lines and a context line.
const MAIN_HUNK: &[u8] = br#"@@ -10,3 +10,4 @@ fn main()
 let a = 1;
-let b = 2;
+let b = 3;
+let c = 4;
 let d = 5;
"#;

/// A GitHub `patch` field: no file headers, and no final newline.
const GITHUB_PATCH: &[u8] = b"@@ -1,2 +1,3 @@\n # Title\n+A new line.\n Body.";

/// A change block whose last removed and last added lines both end their
/// file without a newline.
const MARKER_AFTER_EACH_SIDE: &[u8] = br#"@@ -1,2 +1,2 @@
 keep
-old end
\ No newline at end of file
+new end
\ No newline at end of file
"#;

/// The same, with more than one line on each side of the block.
const MARKERS_ON_BOTH_SIDES: &[u8] = br#"@@ -1,3 +1,3 @@
 a
-b
-c
\ No newline at end of file
+B
+C
\ No newline at end of file
"#;

/// A change that only adds the newline missing after the file's last line.
const FINAL_NEWLINE_ADDED: &[u8] = br#"@@ -1 +1 @@
-last line
\ No newline at end of file
+last line
"#;

/// A file whose last line lacks a newline, changed above that line.
const MARKER_AFTER_CONTEXT: &[u8] = br#"@@ -1,2 +1,2 @@
-first
+First
 end
\ No newline at end of file
"#;

/// An empty context line, as git writes it: a single space.
const EMPTY_CONTEXT_LINE: &[u8] = b"@@ -1,3 +1,3 @@
 a
\x20
-b
+c
";

/// The same line with its space trimmed, as GNU diff's
/// `--suppress-blank-empty` writes it and `git apply` reads it.
const TRIMMED_EMPTY_CONTEXT_LINE: &[u8] = b"@@ -1,3 +1,3 @@\n a\n\n-b\n+c\n";

const TWO_HUNKS: &[u8] = br#"@@ -1,2 +1,2 @@ fn first()
-a
+A
 b
@@ -20,2 +20,3 @@ fn second()
 x
+y
 z
"#;

// --------------------------------------------------------- parse_hunks tests

#[test]
fn line_numbers_follow_the_hunk_ranges() {
    assert_eq!(
        parse_hunks(MAIN_HUNK),
        vec![Hunk {
            old_start: 10,
            old_lines: 3,
            new_start: 10,
            new_lines: 4,
            section: Some("fn main()".to_string()),
            lines: vec![
                context(10, 10, "let a = 1;"),
                removed(11, "let b = 2;"),
                added(11, "let b = 3;"),
                added(12, "let c = 4;"),
                context(12, 13, "let d = 5;"),
            ],
        }]
    );
}

/// `parse_hunks` yields the hunks alone. The caller builds the file from the
/// provider's own status, paths and counts, so nothing in the patch can
/// override them.
#[test]
fn a_header_less_patch_parses_to_hunks() {
    assert_eq!(
        parse_hunks(GITHUB_PATCH),
        vec![hunk(
            (1, 2),
            (1, 3),
            vec![
                context(1, 1, "# Title"),
                added(2, "A new line."),
                context(2, 3, "Body."),
            ],
        )]
    );
}

#[test]
fn anything_before_the_first_hunk_header_is_ignored() {
    let patch = b"--- a/x\n+++ b/x\n-not a line\n@@ -1 +1 @@\n-a\n+b\n";
    assert_eq!(
        parse_hunks(patch),
        vec![hunk((1, 1), (1, 1), vec![removed(1, "a"), added(1, "b")])]
    );
}

#[test]
fn the_no_newline_marker_qualifies_the_line_before_it() {
    assert_eq!(
        parse_hunks(MARKER_AFTER_EACH_SIDE),
        vec![hunk(
            (1, 2),
            (1, 2),
            vec![
                context(1, 1, "keep"),
                at_eof(removed(2, "old end")),
                at_eof(added(2, "new end")),
            ],
        )]
    );
}

#[test]
fn markers_on_both_sides_flag_only_the_last_line_of_each() {
    assert_eq!(
        parse_hunks(MARKERS_ON_BOTH_SIDES),
        vec![hunk(
            (1, 3),
            (1, 3),
            vec![
                context(1, 1, "a"),
                removed(2, "b"),
                at_eof(removed(3, "c")),
                added(2, "B"),
                at_eof(added(3, "C")),
            ],
        )]
    );
}

#[test]
fn adding_only_the_final_newline_flags_the_old_line_alone() {
    assert_eq!(
        parse_hunks(FINAL_NEWLINE_ADDED),
        vec![hunk(
            (1, 1),
            (1, 1),
            vec![at_eof(removed(1, "last line")), added(1, "last line")],
        )]
    );
}

#[test]
fn a_context_line_can_carry_the_flag() {
    assert_eq!(
        parse_hunks(MARKER_AFTER_CONTEXT),
        vec![hunk(
            (1, 2),
            (1, 2),
            vec![
                removed(1, "first"),
                added(1, "First"),
                at_eof(context(2, 2, "end")),
            ],
        )]
    );
}

#[test]
fn an_omitted_count_is_one() {
    assert_eq!(
        parse_hunks(b"@@ -1 +1 @@\n-a\n+b\n"),
        vec![hunk((1, 1), (1, 1), vec![removed(1, "a"), added(1, "b")])]
    );
}

#[test]
fn an_added_files_hunk_numbers_its_new_side_alone() {
    assert_eq!(
        parse_hunks(b"@@ -0,0 +1,3 @@\n+one\n+two\n+three\n"),
        vec![hunk(
            (0, 0),
            (1, 3),
            vec![added(1, "one"), added(2, "two"), added(3, "three")],
        )]
    );
}

#[test]
fn a_deleted_files_hunk_numbers_its_old_side_alone() {
    assert_eq!(
        parse_hunks(b"@@ -1,2 +0,0 @@\n-one\n-two\n"),
        vec![hunk(
            (1, 2),
            (0, 0),
            vec![removed(1, "one"), removed(2, "two")]
        )]
    );
}

#[test]
fn an_empty_context_line_is_numbered_on_both_sides() {
    let expected = vec![hunk(
        (1, 3),
        (1, 3),
        vec![
            context(1, 1, "a"),
            context(2, 2, ""),
            removed(3, "b"),
            added(3, "c"),
        ],
    )];
    assert_eq!(parse_hunks(EMPTY_CONTEXT_LINE), expected);
    assert_eq!(parse_hunks(TRIMMED_EMPTY_CONTEXT_LINE), expected);
}

#[test]
fn several_hunks_are_numbered_from_their_own_ranges() {
    assert_eq!(
        parse_hunks(TWO_HUNKS),
        vec![
            Hunk {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 2,
                section: Some("fn first()".to_string()),
                lines: vec![removed(1, "a"), added(1, "A"), context(2, 2, "b")],
            },
            Hunk {
                old_start: 20,
                old_lines: 2,
                new_start: 20,
                new_lines: 3,
                section: Some("fn second()".to_string()),
                lines: vec![context(20, 20, "x"), added(21, "y"), context(21, 22, "z")],
            },
        ]
    );
}

#[test]
fn a_cut_off_hunk_keeps_the_lines_it_read() {
    assert_eq!(
        parse_hunks(b"@@ -1,5 +1,5 @@\n a\n-b\n+c"),
        vec![hunk(
            (1, 5),
            (1, 5),
            vec![context(1, 1, "a"), removed(2, "b"), added(2, "c")],
        )]
    );
}

/// A provider's trailing blank line is not an empty context line: the hunk's
/// counts were already used up.
#[test]
fn a_line_after_a_complete_hunk_is_not_read_into_it() {
    assert_eq!(
        parse_hunks(b"@@ -1 +1 @@\n-a\n+b\n\n"),
        vec![hunk((1, 1), (1, 1), vec![removed(1, "a"), added(1, "b")])]
    );
}

#[test]
fn a_line_that_cannot_belong_to_a_hunk_ends_it() {
    assert_eq!(
        parse_hunks(b"@@ -1,3 +1,3 @@\n a\nnot a hunk line\n b\n"),
        vec![hunk((1, 3), (1, 3), vec![context(1, 1, "a")])]
    );
}

#[test]
fn a_malformed_hunk_header_is_no_hunk_and_ends_the_one_before_it() {
    assert_eq!(
        parse_hunks(b"@@ -1,3 +1,3 @@\n a\n@@ -x +1 @@\n b\n@@ -9 +9 @@\n-c\n+d\n"),
        vec![
            hunk((1, 3), (1, 3), vec![context(1, 1, "a")]),
            hunk((9, 1), (9, 1), vec![removed(9, "c"), added(9, "d")]),
        ]
    );
}

#[test]
fn a_crlf_line_keeps_its_carriage_return() {
    assert_eq!(
        parse_hunks(b"@@ -1 +1 @@\n-a\r\n+b\r\n"),
        vec![hunk(
            (1, 1),
            (1, 1),
            vec![removed(1, "a\r"), added(1, "b\r")]
        )]
    );
}

#[test]
fn a_one_line_change_to_a_very_long_line() {
    let old = "x".repeat(256 * 1024);
    let new = format!("{old}y");
    let patch = format!("@@ -1 +1 @@\n-{old}\n+{new}\n");
    assert_eq!(
        parse_hunks(patch.as_bytes()),
        vec![hunk((1, 1), (1, 1), vec![removed(1, &old), added(1, &new)])]
    );
}

// -------------------------------------------------------- parse_diff fixtures

const RENAME: &[u8] = br#"diff --git a/src/old.ts b/src/new.ts
similarity index 92%
rename from src/old.ts
rename to src/new.ts
index 1111111..2222222 100644
--- a/src/old.ts
+++ b/src/new.ts
@@ -1,3 +1,3 @@
 export const a = 1;
-export const b = 2;
+export const b = 3;
 export const c = 4;
"#;

/// A pure rename: no `---`/`+++` lines, and a `diff --git` line whose halves
/// name different paths.
const PURE_RENAME: &[u8] = br#"diff --git a/notes/old name.md "b/notes/caf\303\251.md"
similarity index 100%
rename from notes/old name.md
rename to "notes/caf\303\251.md"
"#;

const COPY: &[u8] = br#"diff --git a/a.txt b/b.txt
similarity index 80%
copy from a.txt
copy to b.txt
index 1111111..2222222 100644
--- a/a.txt
+++ b/b.txt
@@ -1,2 +1,2 @@
 same
-old
+new
"#;

/// The regular file `bin/tool` replaced by a symlink of the same name.
const TYPE_CHANGE: &[u8] = br#"diff --git a/bin/tool b/bin/tool
deleted file mode 100755
index 1111111..0000000
--- a/bin/tool
+++ /dev/null
@@ -1,2 +0,0 @@
-#!/bin/sh
-exec ../lib/tool
diff --git a/bin/tool b/bin/tool
new file mode 120000
index 0000000..2222222
--- /dev/null
+++ b/bin/tool
@@ -0,0 +1 @@
+../lib/tool
\ No newline at end of file
"#;

const MODIFIED_README: &[u8] = br#"diff --git a/README.md b/README.md
index 3333333..4444444 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-old
+new
"#;

const MODE_ONLY: &[u8] = br#"diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
"#;

const MODE_AND_EDIT: &[u8] = br#"diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
index 1111111..2222222
--- a/run.sh
+++ b/run.sh
@@ -1 +1 @@
-echo hi
+echo hello
"#;

const QUOTED_NAME: &[u8] = br#"diff --git "a/caf\303\251.md" "b/caf\303\251.md"
index 1111111..2222222 100644
--- "a/caf\303\251.md"
+++ "b/caf\303\251.md"
@@ -1 +1 @@
-a
+b
"#;

/// git appends a tab to a `---`/`+++` name that contains a space.
const SPACE_IN_NAME: &[u8] = b"diff --git a/my notes.md b/my notes.md
index 1111111..2222222 100644
--- a/my notes.md\t
+++ b/my notes.md\t
@@ -1 +1 @@
-a
+b
";

/// A mode-only change to the path `x b/y`, named only by its `diff --git`
/// line.
const AMBIGUOUS_DIFF_LINE: &[u8] = br#"diff --git a/x b/y b/x b/y
old mode 100644
new mode 100755
"#;

/// Three files, the second one's changed lines encoded in Latin-1.
const LATIN1_BETWEEN_UTF8: &[u8] = b"diff --git a/one.txt b/one.txt
index 1111111..2222222 100644
--- a/one.txt
+++ b/one.txt
@@ -1 +1 @@
-caf\xc3\xa9 one
+caf\xc3\xa9 uno
diff --git a/two.txt b/two.txt
index 3333333..4444444 100644
--- a/two.txt
+++ b/two.txt
@@ -1 +1 @@
-caf\xe9 two
+caf\xe9 dos
diff --git a/three.txt b/three.txt
index 5555555..6666666 100644
--- a/three.txt
+++ b/three.txt
@@ -1 +1 @@
-caf\xc3\xa9 three
+caf\xc3\xa9 tres
";

const SUBMODULE: &[u8] = br#"diff --git a/vendor/lib b/vendor/lib
index 1111111..2222222 160000
--- a/vendor/lib
+++ b/vendor/lib
@@ -1 +1 @@
-Subproject commit 1111111111111111111111111111111111111111
+Subproject commit 2222222222222222222222222222222222222222
"#;

const BINARY: &[u8] = br#"diff --git a/logo.png b/logo.png
index 1111111..2222222 100644
Binary files a/logo.png and b/logo.png differ
"#;

/// A binary file added under a name git quotes in its `diff --git` line.
const QUOTED_BINARY: &[u8] = br#"diff --git "a/caf\303\251.png" "b/caf\303\251.png"
new file mode 100644
index 0000000..1111111
Binary files /dev/null and "b/caf\303\251.png" differ
"#;

const EMPTY_ADDED: &[u8] = br#"diff --git a/empty.txt b/empty.txt
new file mode 100644
index 0000000..e69de29
"#;

const DELETED: &[u8] = br#"diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 1111111..0000000
--- a/gone.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-one
-two
"#;

/// Three files, the last one named in git's C-quoted form and ending without
/// a newline, so that cutting it short crosses every kind of line.
const THREE_FILES: &[u8] = br#"diff --git a/one.txt b/one.txt
index 1111111..2222222 100644
--- a/one.txt
+++ b/one.txt
@@ -1,2 +1,2 @@
 one
-two
+deux
diff --git a/two.txt b/two.txt
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/two.txt
@@ -0,0 +1 @@
+fresh
diff --git "a/caf\303\251.txt" "b/caf\303\251.txt"
index 4444444..5555555 100644
--- "a/caf\303\251.txt"
+++ "b/caf\303\251.txt"
@@ -1,3 +1,3 @@ heading
 alpha
-beta
+gamma
 end
\ No newline at end of file
"#;

// ----------------------------------------------------------- parse_diff tests

#[test]
fn a_rename_is_one_file() {
    assert_eq!(
        parse_diff(RENAME),
        vec![DiffFile {
            old_path: some("src/old.ts"),
            new_path: some("src/new.ts"),
            old_mode: some("100644"),
            new_mode: some("100644"),
            status: FileStatus::Renamed {
                similarity: Some(92),
            },
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks {
                hunks: vec![hunk(
                    (1, 3),
                    (1, 3),
                    vec![
                        context(1, 1, "export const a = 1;"),
                        removed(2, "export const b = 2;"),
                        added(2, "export const b = 3;"),
                        context(3, 3, "export const c = 4;"),
                    ],
                )],
            },
        }]
    );
}

#[test]
fn a_pure_rename_is_named_by_its_unquoted_rename_lines() {
    assert_eq!(
        parse_diff(PURE_RENAME),
        vec![DiffFile {
            old_path: some("notes/old name.md"),
            new_path: some("notes/caf\u{e9}.md"),
            old_mode: None,
            new_mode: None,
            status: FileStatus::Renamed {
                similarity: Some(100),
            },
            additions: Some(0),
            deletions: Some(0),
            content: DiffContent::Hunks { hunks: Vec::new() },
        }]
    );
}

#[test]
fn a_copy_is_one_file() {
    assert_eq!(
        parse_diff(COPY),
        vec![DiffFile {
            old_path: some("a.txt"),
            new_path: some("b.txt"),
            old_mode: some("100644"),
            new_mode: some("100644"),
            status: FileStatus::Copied {
                similarity: Some(80),
            },
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks {
                hunks: vec![hunk(
                    (1, 2),
                    (1, 2),
                    vec![context(1, 1, "same"), removed(2, "old"), added(2, "new")],
                )],
            },
        }]
    );
}

fn tool_type_change() -> DiffFile {
    DiffFile {
        old_path: some("bin/tool"),
        new_path: some("bin/tool"),
        old_mode: some("100755"),
        new_mode: some("120000"),
        status: FileStatus::TypeChanged,
        additions: Some(1),
        deletions: Some(2),
        content: DiffContent::Hunks {
            hunks: vec![
                hunk(
                    (1, 2),
                    (0, 0),
                    vec![removed(1, "#!/bin/sh"), removed(2, "exec ../lib/tool")],
                ),
                hunk((0, 0), (1, 1), vec![at_eof(added(1, "../lib/tool"))]),
            ],
        },
    }
}

#[test]
fn a_type_change_is_one_file() {
    assert_eq!(parse_diff(TYPE_CHANGE), vec![tool_type_change()]);
}

#[test]
fn a_type_change_does_not_swallow_the_file_after_it() {
    let text = [TYPE_CHANGE, MODIFIED_README].concat();
    let files = parse_diff(&text);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0], tool_type_change());
    assert_eq!(files[1], parse_diff(MODIFIED_README)[0]);
    assert_eq!(files[1].status, FileStatus::Modified);
}

#[test]
fn a_type_change_folds_only_the_section_directly_after_the_deletion() {
    let text = [TYPE_CHANGE, EMPTY_ADDED].concat();
    let files = parse_diff(&text);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0], tool_type_change());
    assert_eq!(files[1].status, FileStatus::Added);
    assert_eq!(files[1].new_path, some("empty.txt"));
}

#[test]
fn two_sections_of_one_file_type_stay_apart() {
    let text = br#"diff --git a/run b/run
deleted file mode 100644
index 1111111..0000000
--- a/run
+++ /dev/null
@@ -1 +0,0 @@
-old
diff --git a/run b/run
new file mode 100755
index 0000000..2222222
--- /dev/null
+++ b/run
@@ -0,0 +1 @@
+new
"#;
    let files = parse_diff(text);
    let statuses: Vec<FileStatus> = files.iter().map(|file| file.status).collect();
    assert_eq!(statuses, [FileStatus::Deleted, FileStatus::Added]);
}

#[test]
fn a_type_change_needs_a_deletion_then_a_creation_of_the_same_path() {
    // A creation before the deletion.
    let created_first = br#"diff --git a/tool b/tool
new file mode 120000
index 0000000..1111111
--- /dev/null
+++ b/tool
@@ -0,0 +1 @@
+target
diff --git a/tool b/tool
deleted file mode 100755
index 2222222..0000000
--- a/tool
+++ /dev/null
@@ -1 +0,0 @@
-script
"#;
    // An edit, not a deletion, before the creation.
    let edited_first = br#"diff --git a/tool b/tool
index 1111111..2222222 100755
--- a/tool
+++ b/tool
@@ -1 +1 @@
-a
+b
diff --git a/tool b/tool
new file mode 120000
index 0000000..3333333
--- /dev/null
+++ b/tool
@@ -0,0 +1 @@
+target
"#;
    // A deletion and a creation of different paths.
    let other_path = br#"diff --git a/tool b/tool
deleted file mode 100755
index 1111111..0000000
--- a/tool
+++ /dev/null
@@ -1 +0,0 @@
-script
diff --git a/link b/link
new file mode 120000
index 0000000..2222222
--- /dev/null
+++ b/link
@@ -0,0 +1 @@
+target
"#;
    // Sections no header names: they are not known to share a path.
    let unnamed = br#"diff --git a/x b/y
deleted file mode 100644
diff --git a/x b/y
new file mode 120000
"#;
    for (label, text, statuses) in [
        (
            "created first",
            &created_first[..],
            [FileStatus::Added, FileStatus::Deleted],
        ),
        (
            "edited first",
            &edited_first[..],
            [FileStatus::Modified, FileStatus::Added],
        ),
        (
            "other path",
            &other_path[..],
            [FileStatus::Deleted, FileStatus::Added],
        ),
        (
            "unnamed",
            &unnamed[..],
            [FileStatus::Deleted, FileStatus::Added],
        ),
    ] {
        let found: Vec<FileStatus> = parse_diff(text).iter().map(|file| file.status).collect();
        assert_eq!(found, statuses, "{label}");
    }
}

#[test]
fn a_type_change_with_a_binary_side_is_binary() {
    let text = br#"diff --git a/icon b/icon
deleted file mode 100644
index 1111111..0000000
Binary files a/icon and /dev/null differ
diff --git a/icon b/icon
new file mode 120000
index 0000000..2222222
--- /dev/null
+++ b/icon
@@ -0,0 +1 @@
+icons/app.png
"#;
    assert_eq!(
        parse_diff(text),
        vec![DiffFile {
            old_path: some("icon"),
            new_path: some("icon"),
            old_mode: some("100644"),
            new_mode: some("120000"),
            status: FileStatus::TypeChanged,
            additions: None,
            deletions: None,
            content: DiffContent::Binary,
        }]
    );
}

#[test]
fn a_mode_only_change_has_its_own_status() {
    assert_eq!(
        parse_diff(MODE_ONLY),
        vec![DiffFile {
            old_path: some("run.sh"),
            new_path: some("run.sh"),
            old_mode: some("100644"),
            new_mode: some("100755"),
            status: FileStatus::ModeChanged,
            additions: Some(0),
            deletions: Some(0),
            content: DiffContent::Hunks { hunks: Vec::new() },
        }]
    );
}

#[test]
fn an_edited_file_whose_mode_changed_is_modified() {
    assert_eq!(
        parse_diff(MODE_AND_EDIT),
        vec![DiffFile {
            old_path: some("run.sh"),
            new_path: some("run.sh"),
            old_mode: some("100644"),
            new_mode: some("100755"),
            status: FileStatus::Modified,
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks {
                hunks: vec![hunk(
                    (1, 1),
                    (1, 1),
                    vec![removed(1, "echo hi"), added(1, "echo hello")],
                )],
            },
        }]
    );
}

#[test]
fn a_binary_file_whose_mode_changed_is_modified() {
    let text = br#"diff --git a/tool.bin b/tool.bin
old mode 100644
new mode 100755
index 1111111..2222222
Binary files a/tool.bin and b/tool.bin differ
"#;
    let files = parse_diff(text);
    assert_eq!(files[0].status, FileStatus::Modified);
    assert_eq!(files[0].content, DiffContent::Binary);
}

#[test]
fn a_section_with_no_textual_or_mode_change_is_modified() {
    let files = parse_diff(b"diff --git a/x b/x\nindex 1111111..2222222 100644\n");
    assert_eq!(files[0].status, FileStatus::Modified);
    assert_eq!(files[0].old_mode, some("100644"));
    assert_eq!(files[0].new_mode, some("100644"));
}

#[test]
fn a_c_quoted_path_is_unquoted() {
    let files = parse_diff(QUOTED_NAME);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].old_path, some("caf\u{e9}.md"));
    assert_eq!(files[0].new_path, some("caf\u{e9}.md"));
    assert_eq!(files[0].status, FileStatus::Modified);
}

#[test]
fn a_name_containing_a_space_keeps_its_exact_path() {
    let files = parse_diff(SPACE_IN_NAME);
    assert_eq!(files[0].old_path, some("my notes.md"));
    assert_eq!(files[0].new_path, some("my notes.md"));
}

#[test]
fn an_ambiguous_diff_line_is_split_on_its_matching_halves() {
    let files = parse_diff(AMBIGUOUS_DIFF_LINE);
    assert_eq!(files[0].old_path, some("x b/y"));
    assert_eq!(files[0].new_path, some("x b/y"));
    assert_eq!(files[0].status, FileStatus::ModeChanged);
}

/// Paths are never guessed: a `diff --git` line whose halves name different
/// paths, or that does not split at a space, names nothing.
#[test]
fn a_diff_line_whose_halves_differ_names_no_path() {
    for text in [
        &b"diff --git a/x.bin b/y.bin\nindex 1111111..2222222 100644\nBinary files a/x.bin and b/y.bin differ\n"[..],
        &b"diff --git a/x_b/x\nold mode 100644\nnew mode 100755\n"[..],
        &b"diff --git c/x d/x\nold mode 100644\nnew mode 100755\n"[..],
        &b"diff --git \"a/x\"\nold mode 100644\nnew mode 100755\n"[..],
    ] {
        let files = parse_diff(text);
        assert_eq!(
            (files[0].old_path.as_deref(), files[0].new_path.as_deref()),
            (None, None),
            "{}",
            String::from_utf8_lossy(text)
        );
    }
}

/// Each source is used only where the one before it is missing, so headers
/// that disagree show which one won.
#[test]
fn paths_come_from_the_first_header_that_names_them() {
    let rename_over_labels = br#"diff --git a/one b/two
similarity index 90%
rename from from.txt
rename to to.txt
--- a/minus.txt
+++ b/plus.txt
"#;
    let labels_over_diff_line = br#"diff --git a/x b/x
index 1111111..2222222 100644
--- a/minus.txt
+++ b/plus.txt
"#;
    let files = parse_diff(rename_over_labels);
    assert_eq!(files[0].old_path, some("from.txt"));
    assert_eq!(files[0].new_path, some("to.txt"));
    let files = parse_diff(labels_over_diff_line);
    assert_eq!(files[0].old_path, some("minus.txt"));
    assert_eq!(files[0].new_path, some("plus.txt"));
}

/// Every escape git's `quote_c_style` writes, and malformed quoting, which is
/// taken verbatim.
#[test]
fn quoted_names_are_unquoted_as_git_quotes_them() {
    for (label, expected) in [
        (&br#""a/caf\303\251.md""#[..], "caf\u{e9}.md"),
        (
            &br#""a/\a\b\f\n\r\t\v\\\"\101\177""#[..],
            "\u{7}\u{8}\u{c}\n\r\t\u{b}\\\"A\u{7f}",
        ),
        (&br#""a/\377""#[..], "\u{fffd}"),
        (&br#""a/bad\q""#[..], r#""a/bad\q""#),
        (&br#""a/bad\3x1""#[..], r#""a/bad\3x1""#),
        (&br#""a/bad\31x""#[..], r#""a/bad\31x""#),
        (&br#""a/bad\400""#[..], r#""a/bad\400""#),
        (&br#""a/cut\30"#[..], r#""a/cut\30"#),
        (&br#""a/cut\"#[..], r#""a/cut\"#),
        (&br#""a/unterminated"#[..], r#""a/unterminated"#),
    ] {
        let mut text = b"diff --git a/x b/x\n--- ".to_vec();
        text.extend_from_slice(label);
        text.extend_from_slice(b"\n+++ b/x\n");
        assert_eq!(
            parse_diff(&text)[0].old_path.as_deref(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(label)
        );
    }
}

#[test]
fn one_file_in_another_encoding_affects_only_itself() {
    let files = parse_diff(LATIN1_BETWEEN_UTF8);
    let paths: Vec<Option<&str>> = files.iter().map(|f| f.new_path.as_deref()).collect();
    assert_eq!(paths, [Some("one.txt"), Some("two.txt"), Some("three.txt")]);
    assert_eq!(
        hunks_of(&files[0])[0].lines,
        [removed(1, "caf\u{e9} one"), added(1, "caf\u{e9} uno")]
    );
    assert_eq!(
        hunks_of(&files[1])[0].lines,
        [removed(1, "caf\u{fffd} two"), added(1, "caf\u{fffd} dos")]
    );
    assert_eq!(
        hunks_of(&files[2])[0].lines,
        [removed(1, "caf\u{e9} three"), added(1, "caf\u{e9} tres")]
    );
}

#[test]
fn a_submodule_change_is_read_as_ordinary_hunk_lines() {
    assert_eq!(
        parse_diff(SUBMODULE),
        vec![DiffFile {
            old_path: some("vendor/lib"),
            new_path: some("vendor/lib"),
            old_mode: some("160000"),
            new_mode: some("160000"),
            status: FileStatus::Modified,
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks {
                hunks: vec![hunk(
                    (1, 1),
                    (1, 1),
                    vec![
                        removed(
                            1,
                            "Subproject commit 1111111111111111111111111111111111111111",
                        ),
                        added(
                            1,
                            "Subproject commit 2222222222222222222222222222222222222222",
                        ),
                    ],
                )],
            },
        }]
    );
}

#[test]
fn a_binary_marker_makes_a_binary_file_without_counts() {
    assert_eq!(
        parse_diff(BINARY),
        vec![DiffFile {
            old_path: some("logo.png"),
            new_path: some("logo.png"),
            old_mode: some("100644"),
            new_mode: some("100644"),
            status: FileStatus::Modified,
            additions: None,
            deletions: None,
            content: DiffContent::Binary,
        }]
    );
}

#[test]
fn a_binary_file_is_named_by_its_quoted_diff_line() {
    assert_eq!(
        parse_diff(QUOTED_BINARY),
        vec![DiffFile {
            old_path: None,
            new_path: some("caf\u{e9}.png"),
            old_mode: None,
            new_mode: some("100644"),
            status: FileStatus::Added,
            additions: None,
            deletions: None,
            content: DiffContent::Binary,
        }]
    );
}

#[test]
fn an_empty_added_file_has_no_hunks() {
    assert_eq!(
        parse_diff(EMPTY_ADDED),
        vec![DiffFile {
            old_path: None,
            new_path: some("empty.txt"),
            old_mode: None,
            new_mode: some("100644"),
            status: FileStatus::Added,
            additions: Some(0),
            deletions: Some(0),
            content: DiffContent::Hunks { hunks: Vec::new() },
        }]
    );
}

#[test]
fn an_empty_deleted_file_has_no_new_path() {
    let files = parse_diff(b"diff --git a/empty.txt b/empty.txt\ndeleted file mode 100644\n");
    assert_eq!(files[0].old_path, some("empty.txt"));
    assert_eq!(files[0].new_path, None);
    assert_eq!(files[0].status, FileStatus::Deleted);
}

#[test]
fn a_deleted_file_carries_its_old_side_alone() {
    assert_eq!(
        parse_diff(DELETED),
        vec![DiffFile {
            old_path: some("gone.txt"),
            new_path: None,
            old_mode: some("100644"),
            new_mode: None,
            status: FileStatus::Deleted,
            additions: Some(0),
            deletions: Some(2),
            content: DiffContent::Hunks {
                hunks: vec![hunk(
                    (1, 2),
                    (0, 0),
                    vec![removed(1, "one"), removed(2, "two")],
                )],
            },
        }]
    );
}

#[test]
fn an_empty_diff_has_no_files() {
    assert_eq!(parse_diff(b""), Vec::new());
    // A commit header before the first section, as `git show` prints one.
    assert_eq!(
        parse_diff(b"commit 1111111\nAuthor: A <a@example.com>\n\n    subject\n"),
        Vec::new()
    );
}

#[test]
fn a_diff_cut_mid_section_keeps_the_files_before_it() {
    let whole = parse_diff(THREE_FILES);
    assert_eq!(whole.len(), 3);
    assert_eq!(whole[2].new_path, some("caf\u{e9}.txt"));
    let last_section = THREE_FILES
        .windows(b"diff --git ".len())
        .rposition(|window| window == b"diff --git ")
        .expect("the fixture has a third section");
    // Every cut, header, name, hunk header and marker included, parses
    // without a panic; once the cut is in the last section, the files before
    // it are exactly as they were.
    for cut in 0..=THREE_FILES.len() {
        let files = parse_diff(&THREE_FILES[..cut]);
        assert!(files.len() <= 3, "cut at byte {cut}");
        if cut >= last_section {
            assert_eq!(files[..2], whole[..2], "cut at byte {cut}");
        }
    }
    // Cut after `+gamma`, the last file keeps the lines it read.
    let cut = THREE_FILES
        .windows(b"+gamma\n".len())
        .position(|window| window == b"+gamma\n")
        .expect("the fixture adds gamma")
        + b"+gamma\n".len();
    assert_eq!(
        parse_diff(&THREE_FILES[..cut])[2].content,
        DiffContent::Hunks {
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 3,
                new_start: 1,
                new_lines: 3,
                section: Some("heading".to_string()),
                lines: vec![
                    context(1, 1, "alpha"),
                    removed(2, "beta"),
                    added(2, "gamma"),
                ],
            }],
        }
    );
}

#[test]
fn many_files_with_long_lines_each_keep_their_own() {
    let old = |index: usize| format!("{index:04}-").repeat(800);
    let new = |index: usize| format!("{index:04}+").repeat(800);
    let mut text = Vec::new();
    for index in 0..300 {
        text.extend_from_slice(
            format!(
                "diff --git a/f{index}.txt b/f{index}.txt\n\
                 index 1111111..2222222 100644\n\
                 --- a/f{index}.txt\n\
                 +++ b/f{index}.txt\n\
                 @@ -1 +1 @@\n\
                 -{}\n\
                 +{}\n",
                old(index),
                new(index)
            )
            .as_bytes(),
        );
    }
    let files = parse_diff(&text);
    assert_eq!(files.len(), 300);
    for (index, file) in files.iter().enumerate() {
        assert_eq!(file.new_path, Some(format!("f{index}.txt")));
        assert_eq!(
            hunks_of(file),
            [hunk(
                (1, 1),
                (1, 1),
                vec![removed(1, &old(index)), added(1, &new(index))],
            )]
        );
    }
}

// --------------------------------------------------------------- the budgets

fn patched(sizes: &[u32]) -> Vec<Option<u32>> {
    sizes.iter().copied().map(Some).collect()
}

/// A patched file with `changed_lines` lines and `bytes` of patch text.
fn size(changed_lines: u32, bytes: usize) -> Option<PatchSize> {
    Some(PatchSize {
        changed_lines,
        bytes,
    })
}

/// The spec's numbers. The two read ceilings are applied by the reads in
/// `git.rs`, so this is their only pin here.
#[test]
fn the_limits_are_the_specified_constants() {
    assert_eq!(FILE_LINES_LIMIT, 500);
    assert_eq!(EAGER_LINES_LIMIT, 3_000);
    assert_eq!(FILE_PATCH_BYTES_LIMIT, 64 * 1024);
    assert_eq!(EAGER_PATCH_BYTES_LIMIT, 1024 * 1024);
    assert_eq!(STREAMED_READ_BYTES_LIMIT, 8 * 1024 * 1024);
    assert_eq!(REQUESTED_FILE_BYTES_LIMIT, 8 * 1024 * 1024);
}

#[test]
fn files_are_eager_in_order_while_the_total_allows() {
    // The third is over 500 lines; the eighth would take the total to 3,040;
    // the ninth takes it to exactly 3,000.
    assert_eq!(
        eager_by_lines(patched(&[500, 480, 2_000, 490, 490, 490, 490, 100, 60])),
        [true, true, false, true, true, true, true, false, true]
    );
}

#[test]
fn a_file_of_500_changed_lines_is_eager_and_one_of_501_is_withheld() {
    assert_eq!(eager_by_lines(patched(&[500])), [true]);
    assert_eq!(eager_by_lines(patched(&[501])), [false]);
    assert_eq!(eager_files([size(500, 10)]), [true]);
    assert_eq!(eager_files([size(501, 10)]), [false]);
}

#[test]
fn the_running_total_may_reach_3000_but_not_3001() {
    // 2,999 eager, then 2 would make 3,001 and is withheld, while 1 still
    // makes exactly 3,000: a withheld file adds nothing to the total.
    let lines = [500, 500, 500, 500, 500, 499, 2, 1];
    let expected = [true, true, true, true, true, true, false, true];
    assert_eq!(eager_by_lines(patched(&lines)), expected);
    assert_eq!(
        eager_files(lines.iter().map(|&lines| size(lines, 10))),
        expected
    );
}

/// The model's `additions` and `deletions` count changed lines alone, so a
/// file of 500 changed lines and 60 context lines is eager.
#[test]
fn context_lines_are_not_counted() {
    let mut patch = String::from("@@ -1,310 +1,310 @@\n");
    for n in 0..30 {
        patch.push_str(&format!(" before {n}\n"));
    }
    for n in 0..250 {
        patch.push_str(&format!("-old {n}\n"));
    }
    for n in 0..250 {
        patch.push_str(&format!("+new {n}\n"));
    }
    for n in 0..30 {
        patch.push_str(&format!(" after {n}\n"));
    }
    let text = format!("diff --git a/big.rs b/big.rs\nindex 1111111..2222222 100644\n--- a/big.rs\n+++ b/big.rs\n{patch}");
    let file = &parse_diff(text.as_bytes())[0];
    let hunk_lines = hunks_of(file)[0].lines.len();
    assert_eq!(hunk_lines, 560);
    let changed = file.additions.unwrap() + file.deletions.unwrap();
    assert_eq!(changed, 500);
    assert_eq!(eager_files([size(changed, text.len())]), [true]);
}

/// A file with no patch to show keeps its state: it is never eager and never
/// withheld, and adds nothing to the total, even when its provider counted
/// thousands of lines.
#[test]
fn binary_and_too_large_files_keep_their_own_state() {
    let file = |content: DiffContent, changed: u32| DiffFile {
        old_path: some("f"),
        new_path: some("f"),
        old_mode: None,
        new_mode: None,
        status: FileStatus::Modified,
        additions: Some(changed),
        deletions: Some(0),
        content,
    };
    let text = |changed: u32| {
        file(
            DiffContent::Hunks {
                hunks: vec![hunk((1, 0), (1, changed), Vec::new())],
            },
            changed,
        )
    };
    let model = vec![
        text(500),
        text(500),
        text(500),
        text(500),
        text(500),
        file(DiffContent::Binary, 0),
        file(DiffContent::TooLarge, 10_000),
        text(500),
        text(1),
    ];
    // As a host maps its model: only a file with hunks is patched.
    let sizes = model.iter().map(|file| match file.content {
        DiffContent::Hunks { .. } => size(file.additions.unwrap(), 100),
        _ => None,
    });
    let eager = eager_files(sizes);
    assert_eq!(
        eager,
        [true, true, true, true, true, false, false, true, false]
    );
    let payload = withhold_files(model, &eager);
    let kinds: Vec<&str> = payload
        .iter()
        .map(|file| match file.content {
            DiffContent::Hunks { .. } => "hunks",
            DiffContent::Withheld => "withheld",
            DiffContent::TooLarge => "tooLarge",
            DiffContent::Binary => "binary",
        })
        .collect();
    assert_eq!(
        kinds,
        ["hunks", "hunks", "hunks", "hunks", "hunks", "binary", "tooLarge", "hunks", "withheld"]
    );
    assert_eq!(
        eager_by_lines([None, Some(500), None]),
        [false, true, false]
    );
}

#[test]
fn a_byte_limit_only_shrinks_the_eager_set() {
    // The first file's 70 KiB withholds it, but its 300 lines stay in the
    // total: the seventh is still withheld and the eighth still eager.
    let files = [
        size(300, 70 * 1024),
        size(500, 1_000),
        size(500, 1_000),
        size(500, 1_000),
        size(500, 1_000),
        size(500, 1_000),
        size(500, 1_000),
        size(200, 1_000),
    ];
    assert_eq!(
        eager_files(files),
        [false, true, true, true, true, true, false, true]
    );
}

#[test]
fn exactly_64_kib_of_patch_text_is_eager_and_one_byte_more_is_withheld() {
    assert!(ByteBudget::default().admit(64 * 1024));
    assert!(!ByteBudget::default().admit(64 * 1024 + 1));
    // The oversized file is withheld, and the one after it is still decided.
    assert_eq!(
        eager_files([size(1, 64 * 1024 + 1), size(1, 64 * 1024), size(1, 10)]),
        [false, true, true]
    );
}

#[test]
fn the_eager_files_text_stops_at_1_mib() {
    // Sixteen eager files of 64 KiB total exactly 1 MiB: every later file is
    // withheld, and a reader reads no further.
    let mut files = vec![size(1, 64 * 1024); 16];
    files.extend([size(1, 10), size(1, 10)]);
    let mut expected = vec![true; 16];
    expected.extend([false, false]);
    assert_eq!(eager_files(files), expected);

    let mut budget = ByteBudget::default();
    for _ in 0..16 {
        assert!(!budget.is_spent());
        assert!(budget.admit(64 * 1024));
    }
    assert!(budget.is_spent());
    assert!(!budget.admit(1));
}

#[test]
fn an_eager_total_one_byte_short_of_1_mib_admits_one_more_file() {
    // 1 MiB less one byte has not reached the limit, so the next file is
    // eager, and the one after it, with the total past 1 MiB, is not.
    let mut files = vec![size(1, 64 * 1024); 15];
    files.extend([size(1, 64 * 1024 - 1), size(1, 10), size(1, 10)]);
    let mut expected = vec![true; 16];
    expected.extend([true, false]);
    assert_eq!(eager_files(files), expected);

    let mut budget = ByteBudget::default();
    assert!(budget.admit(64 * 1024 - 1));
    for _ in 0..15 {
        assert!(budget.admit(64 * 1024));
    }
    assert!(!budget.is_spent());
    assert!(budget.admit(1));
    assert!(budget.is_spent());
}

/// A streamed read decides the line rule from the file list, then drives the
/// byte budget over the eager sections it reaches: the same decision as
/// `eager_files` over the same sizes.
#[test]
fn a_streamed_read_and_an_in_memory_caller_decide_alike() {
    let files = [
        size(300, 70 * 1024),
        None,
        size(500, 64 * 1024),
        size(600, 10),
        size(200, 64 * 1024),
        size(10, 60 * 1024),
    ];
    let by_lines = eager_by_lines(files.iter().map(|file| file.map(|s| s.changed_lines)));
    let mut budget = ByteBudget::default();
    let streamed: Vec<bool> = by_lines
        .iter()
        .zip(&files)
        .map(|(&eager, file)| eager && budget.admit(file.unwrap().bytes))
        .collect();
    assert_eq!(streamed, eager_files(files));
    assert_eq!(streamed, [false, false, true, false, true, true]);
}

#[test]
fn the_payload_withholds_every_patched_file_that_is_not_eager() {
    let model = parse_diff(&[MODIFIED_README, BINARY, DELETED, MODE_ONLY].concat());
    let payload = withhold_files(model.clone(), &[true, false, false, false]);
    assert_eq!(payload[0], model[0]);
    assert_eq!(payload[1].content, DiffContent::Binary);
    assert_eq!(payload[2].content, DiffContent::Withheld);
    assert_eq!(payload[3].content, DiffContent::Withheld);
    // A withheld file keeps everything but its hunks.
    assert_eq!(
        DiffFile {
            content: DiffContent::Withheld,
            ..model[2].clone()
        },
        payload[2]
    );
}

#[test]
fn a_file_past_the_end_of_the_decision_is_withheld() {
    let model = parse_diff(&[MODIFIED_README, DELETED].concat());
    let payload = withhold_files(model.clone(), &[true]);
    assert_eq!(payload[0], model[0]);
    assert_eq!(payload[1].content, DiffContent::Withheld);
}

/// The budget driven over an in-memory model, as `openspec-app` drives it for
/// a pull request: GitHub's per-file `patch` texts, each parsed with
/// `parse_hunks`, measured by their own length.
#[test]
fn a_pull_requests_in_memory_files_are_budgeted_as_a_commits_are() {
    let wide = format!(
        "@@ -1 +1 @@\n-{}\n+{}",
        "x".repeat(40 * 1024),
        "y".repeat(40 * 1024)
    );
    let provider: [(&str, Option<&[u8]>); 4] = [
        ("small.rs", Some(&b"@@ -1 +1 @@\n-a\n+b"[..])),
        ("logo.png", None),
        ("wide.rs", Some(wide.as_bytes())),
        ("after.rs", Some(&b"@@ -3,2 +3,2 @@\n-c\n+d\n e"[..])),
    ];
    let model: Vec<DiffFile> = provider
        .iter()
        .map(|(name, patch)| DiffFile {
            old_path: some(name),
            new_path: some(name),
            old_mode: None,
            new_mode: None,
            status: FileStatus::Modified,
            additions: patch.map(|_| 1),
            deletions: patch.map(|_| 1),
            content: match patch {
                Some(patch) => DiffContent::Hunks {
                    hunks: parse_hunks(patch),
                },
                None => DiffContent::Binary,
            },
        })
        .collect();
    let sizes = model.iter().zip(&provider).map(|(file, (_, patch))| {
        patch.map(|patch| PatchSize {
            changed_lines: file.additions.unwrap() + file.deletions.unwrap(),
            bytes: patch.len(),
        })
    });
    let eager = eager_files(sizes);
    assert_eq!(eager, [true, false, false, true]);

    let payload = withhold_files(model, &eager);
    assert_eq!(
        payload[0].content,
        DiffContent::Hunks {
            hunks: vec![hunk((1, 1), (1, 1), vec![removed(1, "a"), added(1, "b")])],
        }
    );
    assert_eq!(payload[1].content, DiffContent::Binary);
    assert_eq!(payload[2].content, DiffContent::Withheld);
    assert_eq!(payload[2].additions, Some(1));
    assert_eq!(
        payload[3].content,
        DiffContent::Hunks {
            hunks: vec![hunk(
                (3, 2),
                (3, 2),
                vec![removed(3, "c"), added(3, "d"), context(4, 4, "e")],
            )],
        }
    );
}
