//! The diff model, its two parsers, and the line and byte budgets that decide
//! which files arrive with their hunks (`diff-view`: *Diff Model*, *Line and
//! Byte Budgets With On-Request Loading*).
//!
//! One model whatever the source, so commit detail and the pull-request viewer
//! render the same thing:
//!
//! - [`parse_diff`] reads a full unified diff, as git writes it and as
//!   BitBucket serves it, and [`parse_diff_with_spans`] also reports where
//!   each file's patch text lies in that input;
//! - [`parse_hunks`] reads a header-less per-file patch, such as GitHub's
//!   `patch` field. Its caller builds the [`DiffFile`] from the provider's own
//!   status, paths and counts.
//!
//! Both split on `\n` alone and decode each line on its own with
//! `String::from_utf8_lossy`, so a file in another encoding shows replacement
//! characters in its own lines and no other file is affected, and a CRLF line
//! keeps its `\r`. The model carries no layout: the frontend's unified and
//! side-by-side layouts both render the same [`Hunk::lines`].
//!
//! The budgets are pure functions over per-file inputs in the model's file
//! order. They depend neither on git nor on how the text was read, so a
//! commit's streamed read and a pull request's in-memory detail apply the
//! same decision.

use serde::{Deserialize, Serialize};
use similar::{Algorithm, TextDiff};
use std::borrow::Cow;
use std::ops::Range;
use std::time::Duration;

// ----------------------------------------------------------------- the model

/// One file of a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
    /// `None` when the file was added.
    pub old_path: Option<String>,
    /// `None` when the file was deleted.
    pub new_path: Option<String>,
    /// git's octal mode (`100644`), `None` when the source gives none.
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub status: FileStatus,
    /// Added lines, `None` when the source gives no count (a binary file).
    pub additions: Option<u32>,
    /// Removed lines, `None` when the source gives no count.
    pub deletions: Option<u32>,
    pub content: DiffContent,
}

/// What a diff did to one file.
// `rename_all` on an enum renames the VARIANTS (`ModeChanged` ->
// `modeChanged`); it does not touch the fields inside a struct variant.
// `rename_all_fields` is what would carry a multi-word field to camelCase, as
// `ArchiveScope` needs it to. `similarity` is one word, so today it is
// trap-removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    /// `similarity` is git's percentage, `None` when the source gives none
    /// (GitHub's file list).
    Renamed {
        similarity: Option<u8>,
    },
    Copied {
        similarity: Option<u8>,
    },
    /// The mode alone changed. A file whose content changed as well is
    /// `Modified`, with both modes carried.
    ModeChanged,
    /// A change between a regular file, a symlink and a submodule, which git
    /// writes as a deletion section followed by a creation section.
    TypeChanged,
}

/// A file's content, in exactly one of four states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiffContent {
    /// Its hunks, none when the file has no textual change.
    Hunks {
        hunks: Vec<Hunk>,
    },
    /// Held back by the budgets, and loadable on request.
    Withheld,
    /// Past a read ceiling, or its provider omitted its patch for size.
    TooLarge,
    Binary,
}

/// One `@@ -a,b +c,d @@ heading` hunk and its lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    /// The heading after the second `@@`, `None` when it is empty.
    pub section: Option<String>,
    pub lines: Vec<Line>,
}

/// One line of a hunk, numbered from the hunk's ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub kind: LineKind,
    /// Set on a context or removed line.
    pub old_no: Option<u32>,
    /// Set on a context or added line.
    pub new_no: Option<u32>,
    /// The line without its marker, decoded on its own.
    pub text: String,
    /// The line ends its file without a newline: git's `\ No newline at end
    /// of file`, folded into the line it qualifies so no layout has to look
    /// backwards for it. Skipped when false, so an unflagged line carries no
    /// `noNewline` key at all.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

// ------------------------------------------------------------ the hunk reader

/// The hunks of a header-less per-file patch that starts at its first `@@`,
/// such as GitHub's `patch` field. Anything before the first hunk header is
/// ignored.
pub fn parse_hunks(patch: &[u8]) -> Vec<Hunk> {
    read_hunks(split_lines(patch))
}

/// `text`'s lines, split on `\n` alone: a CRLF line keeps its `\r`, so a copy
/// stays faithful. A final `\n` ends the last line rather than opening an
/// empty one.
fn split_lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    lines_and_ends(text).map(|(line, _)| line)
}

/// [`split_lines`], each line beside where it ends in `text`: just past its
/// `\n`, or at the end of `text` for a last line without one.
fn lines_and_ends(text: &[u8]) -> impl Iterator<Item = (&[u8], usize)> {
    text.split_inclusive(|&byte| byte == b'\n')
        .scan(0, |end, line| {
            *end += line.len();
            Some((line.strip_suffix(b"\n").unwrap_or(line), *end))
        })
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The hunk reader `parse_hunks` and `parse_diff` share.
///
/// A hunk takes lines until its header's counts are used up, so a line after
/// a complete hunk (a provider's trailing blank line) is never read into it,
/// and a hunk cut off early keeps the lines it read. Any line that cannot
/// belong to a hunk ends the one being read.
fn read_hunks<'a>(lines: impl IntoIterator<Item = &'a [u8]>) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    // Where the last hunk stands; `None` once it is complete or abandoned.
    let mut cursor: Option<Cursor> = None;
    for line in lines {
        if line.starts_with(b"@@") {
            let hunk = parse_hunk_header(line);
            cursor = hunk.as_ref().map(Cursor::at_start);
            hunks.extend(hunk);
        } else if line.starts_with(b"\\") {
            // `\ No newline at end of file` qualifies the line before it,
            // whichever side that line is on, and is never a line itself.
            if let Some(last) = hunks.last_mut().and_then(|hunk| hunk.lines.last_mut()) {
                last.no_newline = true;
            }
        } else if let (Some(at), Some(hunk)) = (cursor.as_mut(), hunks.last_mut()) {
            match at.read(line) {
                Some(read) => hunk.lines.push(read),
                None => cursor = None,
            }
        }
    }
    hunks
}

/// `@@ -a[,b] +c[,d] @@ heading`, with no lines yet. An omitted count is 1.
fn parse_hunk_header(line: &[u8]) -> Option<Hunk> {
    let rest = line.strip_prefix(b"@@ -")?;
    let (old_start, old_lines, rest) = parse_range(rest)?;
    let rest = rest.strip_prefix(b" +")?;
    let (new_start, new_lines, rest) = parse_range(rest)?;
    let rest = rest.strip_prefix(b" @@")?;
    let heading = rest.strip_prefix(b" ").unwrap_or(rest);
    Some(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        section: (!heading.is_empty()).then(|| decode(heading)),
        lines: Vec::new(),
    })
}

/// `start[,count]` and what follows it.
fn parse_range(text: &[u8]) -> Option<(u32, u32, &[u8])> {
    let (start, rest) = parse_number(text)?;
    match rest.strip_prefix(b",") {
        Some(rest) => {
            let (count, rest) = parse_number(rest)?;
            Some((start, count, rest))
        }
        None => Some((start, 1, rest)),
    }
}

/// The run of ASCII digits `text` starts with, and what follows it.
fn parse_number(text: &[u8]) -> Option<(u32, &[u8])> {
    let digits = text.iter().take_while(|byte| byte.is_ascii_digit()).count();
    let (number, rest) = text.split_at(digits);
    Some((std::str::from_utf8(number).ok()?.parse().ok()?, rest))
}

/// The next line number on each side, and how many lines each side of the
/// hunk still expects.
struct Cursor {
    old_no: u32,
    new_no: u32,
    old_left: u32,
    new_left: u32,
}

impl Cursor {
    fn at_start(hunk: &Hunk) -> Self {
        Self {
            old_no: hunk.old_start,
            new_no: hunk.new_start,
            old_left: hunk.old_lines,
            new_left: hunk.new_lines,
        }
    }

    /// Reads `line` into the hunk: `None` once both sides are complete, or
    /// when `line` is not a hunk line.
    fn read(&mut self, line: &[u8]) -> Option<Line> {
        if self.old_left == 0 && self.new_left == 0 {
            return None;
        }
        let (kind, text) = match line.split_first() {
            Some((b' ', text)) => (LineKind::Context, text),
            Some((b'-', text)) => (LineKind::Removed, text),
            Some((b'+', text)) => (LineKind::Added, text),
            // An empty context line whose space was trimmed, as GNU diff's
            // `--suppress-blank-empty` writes it and `git apply` reads it.
            None => (LineKind::Context, line),
            Some(_) => return None,
        };
        let old_no = (kind != LineKind::Added).then(|| take(&mut self.old_no, &mut self.old_left));
        let new_no =
            (kind != LineKind::Removed).then(|| take(&mut self.new_no, &mut self.new_left));
        Some(Line {
            kind,
            old_no,
            new_no,
            text: decode(text),
            no_newline: false,
        })
    }
}

/// Takes one side's next line number, and counts the line off that side.
/// Saturating, so a hostile header near `u32::MAX` cannot overflow.
fn take(number: &mut u32, left: &mut u32) -> u32 {
    *left = left.saturating_sub(1);
    let taken = *number;
    *number = number.saturating_add(1);
    taken
}

// ------------------------------------------------------------ the diff reader

/// The files of a full unified diff, as git writes it and as BitBucket serves
/// it, split into sections at each `diff --git` line. Anything before the
/// first section is ignored, and a section cut off mid-way keeps what it read
/// without disturbing the files before it.
///
/// Each section is named by the provider-text rule, never by guessing: from
/// its `rename` or `copy` lines, else its `---` and `+++` lines, else its
/// `diff --git` line where both halves name the same path. A deletion
/// directly followed by a creation of the same path in another file type
/// folds into one `TypeChanged` file.
pub fn parse_diff(text: &[u8]) -> Vec<DiffFile> {
    parse_diff_with_spans(text)
        .into_iter()
        .map(|parsed| parsed.file)
        .collect()
}

/// One file [`parse_diff_with_spans`] read, beside where its patch text lies
/// in the input. Never on the wire: the spans index the caller's own bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedFile {
    pub file: DiffFile,
    /// The byte ranges of the input holding the file's patch text, in order:
    /// its section, from its `diff --git` line up to the next one or the end
    /// of the input, or both sections of a folded type change.
    pub spans: Vec<Range<usize>>,
}

impl SpannedFile {
    /// The length of the file's patch text, its spans together: the size the
    /// byte limits measure, as a streamed read measures the same sections.
    pub fn patch_bytes(&self) -> usize {
        self.spans.iter().map(|span| span.end - span.start).sum()
    }
}

/// [`parse_diff`], reporting beside each file the raw byte spans of its patch
/// text as found in `text`. A provider's patch can then be taken as received:
/// hashed byte for byte, whatever its encoding, and measured by the byte
/// limits as a commit's streamed read measures it (`pull-request-viewer`:
/// *Review Progress*). Text before the first section is in no span.
pub fn parse_diff_with_spans(text: &[u8]) -> Vec<SpannedFile> {
    let mut sections: Vec<Section> = Vec::new();
    let mut start = 0;
    for (line, end) in lines_and_ends(text) {
        if let Some(names) = line.strip_prefix(b"diff --git ") {
            sections.push(Section {
                names,
                lines: Vec::new(),
                span: start..end,
            });
        } else if let Some(section) = sections.last_mut() {
            section.lines.push(line);
            section.span.end = end;
        }
        start = end;
    }
    fold_type_changes(sections.into_iter().map(|section| SpannedFile {
        file: read_section(section.names, &section.lines),
        spans: vec![section.span],
    }))
}

/// One section of a diff as [`parse_diff_with_spans`] splits its input.
struct Section<'a> {
    /// What follows its `diff --git`.
    names: &'a [u8],
    /// Every line after that one.
    lines: Vec<&'a [u8]>,
    /// The bytes of the input it spans, its `diff --git` line included.
    span: Range<usize>,
}

/// The extended headers of one section, as read before its first hunk.
#[derive(Default)]
struct Headers<'a> {
    old_mode: Option<&'a [u8]>,
    new_mode: Option<&'a [u8]>,
    deleted_file_mode: Option<&'a [u8]>,
    new_file_mode: Option<&'a [u8]>,
    /// The mode an `index <old>..<new> <mode>` line carries when it did not
    /// change.
    index_mode: Option<&'a [u8]>,
    similarity: Option<u8>,
    rename_from: Option<&'a [u8]>,
    rename_to: Option<&'a [u8]>,
    copy_from: Option<&'a [u8]>,
    copy_to: Option<&'a [u8]>,
    minus: Option<&'a [u8]>,
    plus: Option<&'a [u8]>,
    binary: bool,
}

impl<'a> Headers<'a> {
    fn read(&mut self, line: &'a [u8]) {
        if let Some(mode) = line.strip_prefix(b"old mode ") {
            self.old_mode = Some(mode);
        } else if let Some(mode) = line.strip_prefix(b"new mode ") {
            self.new_mode = Some(mode);
        } else if let Some(mode) = line.strip_prefix(b"deleted file mode ") {
            self.deleted_file_mode = Some(mode);
        } else if let Some(mode) = line.strip_prefix(b"new file mode ") {
            self.new_file_mode = Some(mode);
        } else if let Some(ids) = line.strip_prefix(b"index ") {
            self.index_mode = ids.split(|&byte| byte == b' ').nth(1);
        } else if let Some(score) = line.strip_prefix(b"similarity index ") {
            self.similarity = parse_percent(score);
        } else if let Some(name) = line.strip_prefix(b"rename from ") {
            self.rename_from = Some(name);
        } else if let Some(name) = line.strip_prefix(b"rename to ") {
            self.rename_to = Some(name);
        } else if let Some(name) = line.strip_prefix(b"copy from ") {
            self.copy_from = Some(name);
        } else if let Some(name) = line.strip_prefix(b"copy to ") {
            self.copy_to = Some(name);
        } else if let Some(label) = line.strip_prefix(b"--- ") {
            self.minus = Some(label);
        } else if let Some(label) = line.strip_prefix(b"+++ ") {
            self.plus = Some(label);
        } else if line.starts_with(b"Binary files ") {
            self.binary = true;
        }
    }
}

/// `92%` as 92.
fn parse_percent(score: &[u8]) -> Option<u8> {
    std::str::from_utf8(score.strip_suffix(b"%")?)
        .ok()?
        .parse()
        .ok()
}

/// One section as a file, before the type-change fold: `names` is what
/// follows its `diff --git`, and `lines` everything after that line.
fn read_section(names: &[u8], lines: &[&[u8]]) -> DiffFile {
    let first_hunk = lines
        .iter()
        .position(|line| line.starts_with(b"@@"))
        .unwrap_or(lines.len());
    let mut headers = Headers::default();
    for line in &lines[..first_hunk] {
        headers.read(line);
    }
    let hunks = read_hunks(lines[first_hunk..].iter().copied());

    let added = headers.new_file_mode.is_some();
    let deleted = headers.deleted_file_mode.is_some();
    let old_mode = headers
        .old_mode
        .or(headers.deleted_file_mode)
        .or(headers.index_mode)
        .map(decode);
    let new_mode = headers
        .new_mode
        .or(headers.new_file_mode)
        .or(headers.index_mode)
        .map(decode);
    let (old_path, new_path) = section_paths(&headers, names);

    let changed = !hunks.is_empty() || headers.binary;
    let status = if added {
        FileStatus::Added
    } else if deleted {
        FileStatus::Deleted
    } else if headers.rename_from.is_some() {
        FileStatus::Renamed {
            similarity: headers.similarity,
        }
    } else if headers.copy_from.is_some() {
        FileStatus::Copied {
            similarity: headers.similarity,
        }
    } else if old_mode != new_mode && !changed {
        FileStatus::ModeChanged
    } else {
        FileStatus::Modified
    };
    let (additions, deletions, content) = if headers.binary {
        (None, None, DiffContent::Binary)
    } else {
        (
            Some(count_lines(&hunks, LineKind::Added)),
            Some(count_lines(&hunks, LineKind::Removed)),
            DiffContent::Hunks { hunks },
        )
    };
    DiffFile {
        old_path: old_path.filter(|_| !added),
        new_path: new_path.filter(|_| !deleted),
        old_mode,
        new_mode,
        status,
        additions,
        deletions,
        content,
    }
}

fn count_lines(hunks: &[Hunk], kind: LineKind) -> u32 {
    let count = hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == kind)
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// A section's old and new paths, by the provider-text rule:
///
/// 1. `rename from`/`rename to`, or `copy from`/`copy to`;
/// 2. else the `---` and `+++` lines;
/// 3. only for a section with neither (a mode-only change, an empty added or
///    deleted file, a binary file), its `diff --git` line, split where its two
///    halves name the same path.
///
/// Never guessed: `-z` does not apply to patch text, and git leaves a space
/// unquoted in the `diff --git` line, so `a/x b/y b/x b/y` is read only
/// because its halves match.
fn section_paths(headers: &Headers, names: &[u8]) -> (Option<String>, Option<String>) {
    let renamed = headers.rename_from.zip(headers.rename_to);
    if let Some((from, to)) = renamed.or(headers.copy_from.zip(headers.copy_to)) {
        return (Some(decode(&unquote(from))), Some(decode(&unquote(to))));
    }
    if let Some((minus, plus)) = headers.minus.zip(headers.plus) {
        return (label_path(minus, b"a/"), label_path(plus, b"b/"));
    }
    let path = git_line_path(names);
    (path.clone(), path)
}

/// A `---` or `+++` line's path: `None` for `/dev/null`, else unquoted and
/// without its `a/` or `b/` prefix.
fn label_path(label: &[u8], prefix: &[u8]) -> Option<String> {
    // git appends a tab after a name containing a space. A raw tab is never
    // part of a name, which git would have quoted, so the name ends at the
    // first one.
    let label = label.split(|&byte| byte == b'\t').next().unwrap_or(label);
    if label == b"/dev/null" {
        return None;
    }
    let name = unquote(label);
    Some(decode(name.strip_prefix(prefix).unwrap_or(&name)))
}

/// The one path a `diff --git a/<path> b/<path>` line names, `None` unless
/// its two halves name the same path.
fn git_line_path(names: &[u8]) -> Option<String> {
    let (old, new) = match unquote_c_style(names) {
        Some((old, rest)) => (Cow::Owned(old), unquote(rest.strip_prefix(b" ")?)),
        None => {
            // Unquoted, `a/<path> b/<path>` has two halves of one length,
            // so they can only meet at its middle byte.
            let middle = names.len() / 2;
            if names.get(middle) != Some(&b' ') {
                return None;
            }
            (
                Cow::Borrowed(&names[..middle]),
                Cow::Borrowed(&names[middle + 1..]),
            )
        }
    };
    let old = old.strip_prefix(b"a/")?;
    (Some(old) == new.strip_prefix(b"b/")).then(|| decode(old))
}

/// `name` with git's C-style quoting undone, or verbatim when it is not
/// quoted or its quoting is malformed.
fn unquote(name: &[u8]) -> Cow<'_, [u8]> {
    match unquote_c_style(name) {
        Some((bytes, _)) => Cow::Owned(bytes),
        None => Cow::Borrowed(name),
    }
}

/// The bytes of a C-quoted name at the start of `quoted`, and what follows
/// its closing quote, as git's own `unquote_c_style` reads them: the
/// backslash escapes `\a \b \f \n \r \t \v \\ \"`, and three-digit octal
/// escapes (`\303\251` is the UTF-8 for `é`) of at most `\377`.
fn unquote_c_style(quoted: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    let mut rest = quoted.strip_prefix(b"\"")?;
    let mut bytes = Vec::new();
    loop {
        let (&byte, tail) = rest.split_first()?;
        rest = tail;
        match byte {
            b'"' => return Some((bytes, rest)),
            b'\\' => {
                let (&escape, tail) = rest.split_first()?;
                rest = tail;
                bytes.push(match escape {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b'f' => 0x0c,
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'v' => 0x0b,
                    b'\\' | b'"' => escape,
                    b'0'..=b'3' => {
                        let [second, third, tail @ ..] = rest else {
                            return None;
                        };
                        rest = tail;
                        (escape - b'0') * 64 + octal_digit(*second)? * 8 + octal_digit(*third)?
                    }
                    _ => return None,
                });
            }
            _ => bytes.push(byte),
        }
    }
}

fn octal_digit(byte: u8) -> Option<u8> {
    (b'0'..=b'7').contains(&byte).then(|| byte - b'0')
}

/// Folds each type change into one file. git writes a change between a
/// regular file, a symlink and a submodule as a section deleting the path
/// followed directly by a section creating it. The pair becomes one
/// `TypeChanged` file carrying both modes, both sections' hunks in order,
/// their summed counts and both their spans. Two sections of one file type
/// stay apart.
fn fold_type_changes(files: impl IntoIterator<Item = SpannedFile>) -> Vec<SpannedFile> {
    let mut files = files.into_iter().peekable();
    let mut folded = Vec::new();
    while let Some(parsed) = files.next() {
        match files.next_if(|next| is_type_change(&parsed.file, &next.file)) {
            Some(created) => folded.push(SpannedFile {
                file: type_change(parsed.file, created.file),
                spans: [parsed.spans, created.spans].concat(),
            }),
            None => folded.push(parsed),
        }
    }
    folded
}

fn is_type_change(deleted: &DiffFile, created: &DiffFile) -> bool {
    deleted.status == FileStatus::Deleted
        && created.status == FileStatus::Added
        && deleted
            .old_path
            .as_ref()
            .is_some_and(|path| created.new_path.as_ref() == Some(path))
        && differ_in_type(deleted.old_mode.as_deref(), created.new_mode.as_deref())
}

/// True when both are git modes of different file types: regular (`100644`,
/// `100755`), symlink (`120000`) or gitlink (`160000`).
fn differ_in_type(old: Option<&str>, new: Option<&str>) -> bool {
    let file_type = |mode: Option<&str>| {
        u32::from_str_radix(mode?, 8)
            .ok()
            .map(|mode| mode & 0o170000)
    };
    matches!((file_type(old), file_type(new)), (Some(old), Some(new)) if old != new)
}

fn type_change(deleted: DiffFile, created: DiffFile) -> DiffFile {
    let content = match (deleted.content, created.content) {
        (DiffContent::Hunks { hunks: mut first }, DiffContent::Hunks { hunks: second }) => {
            first.extend(second);
            DiffContent::Hunks { hunks: first }
        }
        // Text on one side only, such as a binary file replaced by a symlink.
        _ => DiffContent::Binary,
    };
    DiffFile {
        old_path: deleted.old_path,
        new_path: created.new_path,
        old_mode: deleted.old_mode,
        new_mode: created.new_mode,
        status: FileStatus::TypeChanged,
        additions: add_counts(deleted.additions, created.additions),
        deletions: add_counts(deleted.deletions, created.deletions),
        content,
    }
}

fn add_counts(first: Option<u32>, second: Option<u32>) -> Option<u32> {
    Some(first? + second?)
}

// --------------------------------------------------------------- the budgets
//
// None of these limits is a setting, and none depends on the layout: the
// budgets are decided before any layout, so both layouts withhold the same
// files.

/// A file whose own changed lines pass this is withheld.
pub const FILE_LINES_LIMIT: u32 = 500;
/// The eager files' changed lines never pass this in all.
pub const EAGER_LINES_LIMIT: u32 = 3_000;
/// A file whose own patch text passes this many bytes is withheld.
pub const FILE_PATCH_BYTES_LIMIT: usize = 64 * 1024;
/// Once the eager files' patch text reaches this many bytes, every remaining
/// file is withheld and a reader stops. The file that reaches it keeps its
/// hunks, so the eager text stays under this plus one file's limit.
pub const EAGER_PATCH_BYTES_LIMIT: usize = 1024 * 1024;
/// A streamed read gives up once it has read this many bytes of patch text in
/// all, eager or not, withholding every file it has not reached.
pub const STREAMED_READ_BYTES_LIMIT: usize = 8 * 1024 * 1024;
/// A file read on request whose diff text passes this many bytes is too large
/// to preview.
pub const REQUESTED_FILE_BYTES_LIMIT: usize = 8 * 1024 * 1024;

/// A patched file's size as the budgets measure it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchSize {
    /// Its added plus removed lines. Context lines are not counted.
    pub changed_lines: u32,
    /// The length of its patch text, in bytes.
    pub bytes: usize,
}

/// The line rule, decided before any patch text is read: whether each file is
/// eager, in the model's file order.
///
/// Each entry is a patched file's changed lines, or `None` for a file with no
/// patch to show (a `Binary` or `TooLarge` one), which keeps its own state,
/// adds nothing to the total and is never withheld. A patched file is eager
/// when its own changed lines are at most [`FILE_LINES_LIMIT`] and, added to
/// those of the eager files before it, at most [`EAGER_LINES_LIMIT`].
pub fn eager_by_lines(changed_lines: impl IntoIterator<Item = Option<u32>>) -> Vec<bool> {
    let mut lines = LineBudget::default();
    changed_lines
        .into_iter()
        .map(|changed| changed.is_some_and(|changed| lines.admit(changed)))
        .collect()
}

/// The whole decision for files whose patch text is already in memory, as a
/// pull request's is: the line rule, then the byte limits over the files it
/// made eager. Equal to driving [`eager_by_lines`] and then a [`ByteBudget`]
/// over its eager files, as a streamed read does.
pub fn eager_files(files: impl IntoIterator<Item = Option<PatchSize>>) -> Vec<bool> {
    let mut lines = LineBudget::default();
    let mut bytes = ByteBudget::default();
    files
        .into_iter()
        .map(|file| {
            file.is_some_and(|size| lines.admit(size.changed_lines) && bytes.admit(size.bytes))
        })
        .collect()
}

/// The eager files' changed lines so far.
#[derive(Default)]
struct LineBudget {
    eager_lines: u32,
}

impl LineBudget {
    fn admit(&mut self, changed_lines: u32) -> bool {
        let eager = changed_lines <= FILE_LINES_LIMIT
            && self.eager_lines + changed_lines <= EAGER_LINES_LIMIT;
        if eager {
            self.eager_lines += changed_lines;
        }
        eager
    }
}

/// The byte limits, decided one line-eager file at a time in the model's
/// order, the same way by a streamed reader and an in-memory caller. They
/// apply after the line rule and only ever withhold more: a file they withhold
/// keeps its lines in the line total.
#[derive(Debug, Clone, Default)]
pub struct ByteBudget {
    eager_bytes: usize,
}

impl ByteBudget {
    /// True once the eager files' patch text has reached
    /// [`EAGER_PATCH_BYTES_LIMIT`]: every remaining file is withheld, and a
    /// reader reads no further.
    pub fn is_spent(&self) -> bool {
        self.eager_bytes >= EAGER_PATCH_BYTES_LIMIT
    }

    /// Whether a file the line rule made eager keeps its hunks, given the
    /// length of its patch text. A file whose text passes
    /// [`FILE_PATCH_BYTES_LIMIT`] is withheld and the files after it are
    /// still decided; once the budget [`is_spent`](Self::is_spent), none is
    /// admitted.
    pub fn admit(&mut self, patch_bytes: usize) -> bool {
        if self.is_spent() || patch_bytes > FILE_PATCH_BYTES_LIMIT {
            return false;
        }
        self.eager_bytes += patch_bytes;
        true
    }
}

/// The payload a model and its budget decision make: every patched file that
/// is not eager becomes `Withheld`, and every other file keeps its content. A
/// `Binary` or `TooLarge` file is never withheld, and a file past the end of
/// `eager` counts as not eager.
pub fn withhold_files(mut files: Vec<DiffFile>, eager: &[bool]) -> Vec<DiffFile> {
    for (index, file) in files.iter_mut().enumerate() {
        let patched = matches!(file.content, DiffContent::Hunks { .. });
        if patched && eager.get(index) != Some(&true) {
            file.content = DiffContent::Withheld;
        }
    }
    files
}

// ---- two versions, diffed locally ----

/// How long the local diff of two versions may search for the shortest edit
/// before it settles for a coarser one, so a pathological pair of versions
/// cannot hold a blocking thread.
pub const VERSIONS_DIFF_TIMEOUT: Duration = Duration::from_secs(2);

/// The content of a file whose provider sent its two versions rather than its
/// patch, as `git diff` would show it: an added file is diffed against an
/// empty old version, and a deleted one against an empty new version
/// (`pull-request-viewer`: *GitHub Detail Reads*). Decided in this order:
///
/// 1. a version longer than [`REQUESTED_FILE_BYTES_LIMIT`] is too large;
/// 2. a version holding a NUL byte, or one that is not valid UTF-8, is binary;
/// 3. otherwise the versions are diffed line by line, with three lines of
///    context, and a diff text longer than [`REQUESTED_FILE_BYTES_LIMIT`] is
///    too large too.
///
/// The hunks come from [`parse_hunks`], so a version without a final newline
/// carries the no-newline flag on its last line, as a provider's patch would.
pub fn diff_versions(old: Option<&[u8]>, new: Option<&[u8]>) -> DiffContent {
    let (old, new) = (old.unwrap_or_default(), new.unwrap_or_default());
    if old.len() > REQUESTED_FILE_BYTES_LIMIT || new.len() > REQUESTED_FILE_BYTES_LIMIT {
        return DiffContent::TooLarge;
    }
    /// A version as text: `None` when it holds a NUL byte or is not UTF-8.
    fn text(bytes: &[u8]) -> Option<&str> {
        (!bytes.contains(&0))
            .then(|| std::str::from_utf8(bytes).ok())
            .flatten()
    }
    let (Some(old), Some(new)) = (text(old), text(new)) else {
        return DiffContent::Binary;
    };
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(VERSIONS_DIFF_TIMEOUT)
        .diff_lines(old, new);
    let mut unified = diff.unified_diff();
    unified.context_radius(3).missing_newline_hint(true);
    let patch = unified.to_string();
    if patch.len() > REQUESTED_FILE_BYTES_LIMIT {
        return DiffContent::TooLarge;
    }
    DiffContent::Hunks {
        hunks: parse_hunks(patch.as_bytes()),
    }
}
