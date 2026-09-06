use crate::types::{
    ArchivedChangeCopy, ArchivedChangeRow, ArchivedChangeSummary, ArtifactStatus, ChangeData,
    Section, Task, WorkspaceFolder,
};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Parsed contents of a `tasks.md` file. Counts include tasks that appear
/// before any `## Heading` (so-called "orphan" tasks). Orphan tasks do not
/// appear in any section.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedTasks {
    pub sections: Vec<Section>,
    pub total_tasks: usize,
    pub completed_tasks: usize,
}

/// Parse a `tasks.md` file into its sections and task counts.
///
/// Format mirrored from `artifex/vscode-extension/src/taskParser.ts`:
/// - `## <title>` lines start a new section (the rest of the line is the title).
/// - `- [ ] <text>` / `- [x] <text>` / `- [X] <text>` (with arbitrary leading
///   whitespace as indent) match a task.
/// - Tasks before any section header are counted in `total_tasks` and
///   `completed_tasks` but are not surfaced in `sections`.
///
/// Returns an empty `ParsedTasks` if the file does not exist. Other IO errors
/// (permission denied, etc.) propagate.
pub fn parse_tasks_md(path: &Path) -> io::Result<ParsedTasks> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ParsedTasks::default()),
        Err(e) => return Err(e),
    };

    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    let mut total_tasks = 0usize;
    let mut completed_tasks = 0usize;

    for (idx, raw_line) in text.lines().enumerate() {
        let line_number = idx + 1;

        if let Some(rest) = raw_line.strip_prefix("## ") {
            if let Some(prev) = current.take() {
                sections.push(prev);
            }
            current = Some(Section {
                title: rest.trim().to_string(),
                tasks: Vec::new(),
            });
            continue;
        }

        if let Some(task) = parse_task_line(raw_line, line_number) {
            if task.completed {
                completed_tasks += 1;
            }
            total_tasks += 1;
            if let Some(section) = current.as_mut() {
                section.tasks.push(task);
            }
        }
    }

    if let Some(last) = current {
        sections.push(last);
    }

    Ok(ParsedTasks {
        sections,
        total_tasks,
        completed_tasks,
    })
}

/// Count completed task checkboxes in raw `tasks.md` text. Used by the git
/// backfill to read a change's completed-count at a past commit without
/// touching the working tree.
pub fn count_completed_in_text(text: &str) -> usize {
    text.lines()
        .filter_map(|line| parse_task_line(line, 0))
        .filter(|t| t.completed)
        .count()
}

fn parse_task_line(line: &str, line_number: usize) -> Option<Task> {
    let trimmed = line.trim_start();
    let prefixed = trimmed.strip_prefix("- [")?;
    let mut chars = prefixed.chars();
    let checkbox = chars.next()?;
    let completed = match checkbox {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    let after_checkbox = &prefixed[checkbox.len_utf8()..];
    let rest = after_checkbox.strip_prefix("] ")?;
    let indent = line.len() - trimmed.len();
    Some(Task {
        text: rest.trim().to_string(),
        completed,
        indent,
        line_number,
    })
}

/// Extract a title from a `proposal.md` file.
///
/// Skips ignorable preamble at the top of the document — blank lines, one
/// leading `---` YAML frontmatter block, and HTML comment blocks — then
/// examines exactly one content line. That line yields a title only when it
/// is a level-1 Markdown heading: a single `#` followed by whitespace and
/// non-empty text (leading whitespace before the `#` is tolerated). An
/// optional case-insensitive `Proposal:` prefix is stripped from the heading
/// text. Anything else — a deeper heading such as the spec-driven template's
/// `## Why`, body text, or an unterminated preamble block — yields `None`;
/// later lines are never considered, so a `#` inside a fenced code block can
/// never be mistaken for the title. A missing or unreadable file yields
/// `None`.
pub fn parse_proposal_title(path: &Path) -> Option<String> {
    const PROPOSAL_PREFIX: &str = "Proposal:";

    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();

    // Skip the preamble. Frontmatter only counts when its opening `---` is
    // the document's first content line; an unterminated frontmatter or
    // comment block consumes the rest of the file, so the `?`s yield `None`.
    let mut at_document_start = true;
    let first_content = loop {
        let trimmed = lines.next()?.trim();
        if trimmed.is_empty() {
            continue;
        }
        if at_document_start && trimmed == "---" {
            at_document_start = false;
            while lines.next()?.trim() != "---" {}
            continue;
        }
        if trimmed.starts_with("<!--") {
            at_document_start = false;
            let mut line = trimmed;
            while !line.contains("-->") {
                line = lines.next()?;
            }
            continue;
        }
        break trimmed;
    };

    // Only a true h1 titles the document: one `#`, then whitespace, then text.
    let rest = first_content.strip_prefix('#')?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let without_hash = rest.trim_start();
    let stripped = match without_hash.get(0..PROPOSAL_PREFIX.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(PROPOSAL_PREFIX) => {
            without_hash[PROPOSAL_PREFIX.len()..].trim_start()
        }
        _ => without_hash,
    };

    let title = stripped.trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

/// Inspect a change directory and report which artifacts are present.
/// Capability specs are sorted alphabetically for stable output.
pub fn parse_artifact_status(change_dir: &Path) -> ArtifactStatus {
    ArtifactStatus {
        proposal: change_dir.join("proposal.md").is_file(),
        design: change_dir.join("design.md").is_file(),
        tasks: change_dir.join("tasks.md").is_file(),
        specs: collect_capability_specs(&change_dir.join("specs")),
    }
}

fn collect_capability_specs(specs_dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let Ok(entries) = fs::read_dir(specs_dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        if !path.join("spec.md").is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            names.push(name.to_string());
        }
    }
    names.sort();
    names
}

/// Parse a single change directory into its `ChangeData`. The `change_id` is
/// the directory name as it appears under `openspec/changes/`.
pub fn parse_change(
    change_dir: &Path,
    change_id: &str,
    workspace: &WorkspaceFolder,
) -> io::Result<ChangeData> {
    let artifacts = parse_artifact_status(change_dir);

    let (sections, total_tasks, completed_tasks) = if artifacts.tasks {
        let parsed = parse_tasks_md(&change_dir.join("tasks.md"))?;
        (parsed.sections, parsed.total_tasks, parsed.completed_tasks)
    } else {
        (Vec::new(), 0, 0)
    };

    let title = if artifacts.proposal {
        parse_proposal_title(&change_dir.join("proposal.md"))
    } else {
        None
    };

    Ok(ChangeData {
        change_id: change_id.to_string(),
        title,
        sections,
        total_tasks,
        completed_tasks,
        artifacts,
        workspace: workspace.clone(),
    })
}

/// List the change IDs in a workspace's `openspec/changes/` directory.
/// Excludes the `archive/` subdirectory. Result is sorted for stable
/// ordering. A missing `openspec/changes/` directory returns an empty vec.
pub fn list_active_changes(workspace_root: &Path) -> io::Result<Vec<String>> {
    let changes_dir = workspace_root.join("openspec").join("changes");
    let entries = match fs::read_dir(&changes_dir) {
        Ok(it) => it,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut ids = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        if name == "archive" {
            continue;
        }
        ids.push(name);
    }
    ids.sort();
    Ok(ids)
}

/// Parse every active (non-archived) change in a workspace.
pub fn parse_all_changes(workspace: &WorkspaceFolder) -> io::Result<Vec<ChangeData>> {
    let workspace_root = &workspace.uri;
    let ids = list_active_changes(workspace_root)?;
    let mut changes = Vec::with_capacity(ids.len());
    for id in ids {
        let change_dir = workspace_root.join("openspec").join("changes").join(&id);
        let data = parse_change(&change_dir, &id, workspace)?;
        changes.push(data);
    }
    Ok(changes)
}

/// List the change IDs in a workspace's `openspec/changes/archive/`
/// directory. A missing `archive/` directory returns an empty vec.
pub fn list_archived_changes(workspace_root: &Path) -> io::Result<Vec<String>> {
    let archive_dir = workspace_root
        .join("openspec")
        .join("changes")
        .join("archive");
    let entries = match fs::read_dir(&archive_dir) {
        Ok(it) => it,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut ids = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        ids.push(name);
    }
    ids.sort();
    Ok(ids)
}

/// Recover the bare logical change id from an `openspec/changes/archive/`
/// directory name. The archive tooling moves a change to
/// `archive/<YYYY-MM-DD>-<id>/`, so strip a leading `YYYY-MM-DD-` date prefix
/// when (and only when) those leading characters form a valid date; any name
/// without such a prefix passes through unchanged.
///
/// The match is anchored: the prefix is `YYYY-MM-DD-` exactly (digits and
/// hyphens at fixed positions), and the remainder is returned verbatim. So
/// `2026-06-04-foo` → `foo`, but `2026-06-04-x-foo` → `x-foo` (not `foo`),
/// which keeps an unrelated archive entry from matching a shorter id.
pub fn archive_dir_logical_id(dir_name: &str) -> &str {
    let b = dir_name.as_bytes();
    // `YYYY-MM-DD-` is 11 bytes; require at least one byte of id after it.
    let dated = b.len() > 11
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-';
    if dated {
        // Byte 10 is an ASCII '-', so byte 11 is a char boundary.
        &dir_name[11..]
    } else {
        dir_name
    }
}

/// Parse every archived change in a workspace. Mirrors [`parse_all_changes`]
/// but reads from `openspec/changes/archive/<id>/`. Returns an empty list
/// if no archive directory exists.
pub fn parse_all_archived(workspace: &WorkspaceFolder) -> io::Result<Vec<ChangeData>> {
    let workspace_root = &workspace.uri;
    let ids = list_archived_changes(workspace_root)?;
    let mut changes = Vec::with_capacity(ids.len());
    for id in ids {
        let change_dir = workspace_root
            .join("openspec")
            .join("changes")
            .join("archive")
            .join(&id);
        let data = parse_change(&change_dir, &id, workspace)?;
        changes.push(data);
    }
    Ok(changes)
}

/// Extract the `YYYY-MM-DD` date prefix from an archive directory name, when
/// present. Returns `None` for a name without the dated prefix — mirrors the
/// anchored matching in [`archive_dir_logical_id`].
pub fn archive_dir_date(dir_name: &str) -> Option<&str> {
    let b = dir_name.as_bytes();
    let dated = b.len() > 11
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-';
    // Bytes 0..10 are ASCII digits and hyphens, so `[0..10]` is the
    // `YYYY-MM-DD` date on a char boundary.
    dated.then(|| &dir_name[0..10])
}

/// Lightweight listing of a workspace's archived changes for the Archive
/// browser: one [`ArchivedChangeSummary`] per archive directory, ordered
/// newest-first by date. The id and date come from the directory name; the
/// title is a heading-only read of `proposal.md`. No `tasks.md` or `specs/`
/// is read — this never does a full [`parse_change`].
pub fn list_archived_summaries(workspace_root: &Path) -> io::Result<Vec<ArchivedChangeSummary>> {
    let archive_dir = workspace_root
        .join("openspec")
        .join("changes")
        .join("archive");
    let dir_names = list_archived_changes(workspace_root)?;
    let mut out = Vec::with_capacity(dir_names.len());
    for name in dir_names {
        let title = parse_proposal_title(&archive_dir.join(&name).join("proposal.md"));
        out.push(ArchivedChangeSummary {
            id: archive_dir_logical_id(&name).to_string(),
            date: archive_dir_date(&name).map(str::to_string),
            title,
            dir_name: name,
        });
    }
    // Newest-first by date, with a stable tiebreak on id. Entries with no date
    // prefix sort last (a `None` date orders before any `Some`).
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

/// Pool per-worktree archive listings into one row per **logical** change.
///
/// Input is one `(worktree path, that worktree's [`list_archived_summaries`])`
/// pair per tracked worktree of a top-level row; output is the de-duplicated
/// union the Archive browser lists (`archive-browser`: *Union Archive Listing
/// Across a Repository's Worktrees*). Pure — no I/O, no clock, no filesystem.
///
/// **The de-duplication key is the bare logical id**, never the raw archive
/// directory name. The date prefix records the day `openspec archive` ran *in
/// that worktree*, so two worktrees that archived one change on different days
/// (`2026-06-04-add-thing` and `2026-06-05-add-thing`) — or one that predates
/// the dated naming (`add-thing`) — would otherwise render as two or three
/// rows, which is precisely the duplication a union exists to remove.
///
/// Ordering is a deterministic **total** order at both levels, so the listing
/// never reorders between two reads of unchanged content:
///
/// - rows: newest date first, then id ascending (ids are unique per row, so
///   the tie-break is total);
/// - copies: newest date first, then archive directory name, then worktree
///   path (one worktree CAN hold two copies of a logical change — a dated
///   directory alongside a legacy un-dated one — so the worktree alone is not
///   a total tie-break).
///
/// A row's `date` is the NEWEST across its copies; `None` never displaces a
/// `Some`, so a legacy un-dated copy cannot erase its dated twin's date, and a
/// row is dateless only when every copy is.
pub fn group_archived_rows(
    listings: Vec<(PathBuf, Vec<ArchivedChangeSummary>)>,
) -> Vec<ArchivedChangeRow> {
    // Each entry carries its copy alongside that copy's own title, so the row's
    // title can be chosen AFTER the copies are ordered — making it the title of
    // the copy that opens first rather than of whichever worktree the caller
    // happened to list first.
    let mut by_id: BTreeMap<String, Vec<(ArchivedChangeCopy, Option<String>)>> = BTreeMap::new();
    for (worktree_path, summaries) in listings {
        for summary in summaries {
            by_id.entry(summary.id).or_default().push((
                ArchivedChangeCopy {
                    worktree_path: worktree_path.clone(),
                    archive_dir: summary.dir_name,
                    date: summary.date,
                },
                summary.title,
            ));
        }
    }

    let mut rows: Vec<ArchivedChangeRow> = by_id
        .into_iter()
        .map(|(id, mut entries)| {
            entries.sort_by(|(a, _), (b, _)| {
                b.date
                    .cmp(&a.date)
                    .then_with(|| a.archive_dir.cmp(&b.archive_dir))
                    .then_with(|| a.worktree_path.cmp(&b.worktree_path))
            });
            let title = entries.iter().find_map(|(_, t)| t.clone());
            let copies: Vec<ArchivedChangeCopy> = entries.into_iter().map(|(c, _)| c).collect();
            // `Option<String>`'s ordering puts `None` below every `Some`, so
            // `max` over the present dates is the newest one and a row of only
            // un-dated copies yields `None`.
            let date = copies.iter().filter_map(|c| c.date.as_ref()).max().cloned();
            ArchivedChangeRow {
                id,
                date,
                title,
                copies,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.id.cmp(&b.id)));
    rows
}

/// Cheap archived-change snapshot for the aggregator: one stub [`ChangeData`]
/// per archive directory, carrying only the (dated) directory name as
/// `change_id` so [`crate::repo_view::diff_views`] can key logical changes and
/// detect archive transitions — WITHOUT reading any change's proposal, tasks,
/// or spec files. Replaces the former eager [`parse_all_archived`] call on the
/// watcher hot path; the archive's content is loaded lazily and per-workspace
/// by the Archive browser instead (see [`list_archived_summaries`]).
pub fn list_archived_stubs(workspace: &WorkspaceFolder) -> io::Result<Vec<ChangeData>> {
    let ids = list_archived_changes(&workspace.uri)?;
    Ok(ids
        .into_iter()
        .map(|id| ChangeData {
            change_id: id,
            title: None,
            sections: Vec::new(),
            total_tasks: 0,
            completed_tasks: 0,
            artifacts: ArtifactStatus::default(),
            workspace: workspace.clone(),
        })
        .collect())
}
