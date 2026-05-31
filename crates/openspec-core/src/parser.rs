use crate::types::{ArtifactStatus, ChangeData, Section, Task, WorkspaceFolder};
use std::fs;
use std::io;
use std::path::Path;

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

/// Extract a title from the first line of a `proposal.md` file.
///
/// Strips leading `#` characters, then any whitespace, then an optional
/// case-insensitive `Proposal:` prefix, then trims the result. Returns `None`
/// if the file does not exist, cannot be read, or the resulting title is empty.
pub fn parse_proposal_title(path: &Path) -> Option<String> {
    const PROPOSAL_PREFIX: &str = "Proposal:";

    let text = fs::read_to_string(path).ok()?;
    let first_line = text.lines().next()?;

    let without_hash = first_line.trim_start_matches('#').trim_start();
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
