use openspec_core::{
    archive_dir_date, archive_dir_logical_id, list_active_changes, list_archived_stubs,
    list_archived_summaries, parse_all_changes, parse_artifact_status, parse_change,
    parse_proposal_title, parse_tasks_md, WorkspaceFolder,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn workspace_a_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace-a")
}

fn workspace_a_folder() -> WorkspaceFolder {
    let root = workspace_a_root().canonicalize().unwrap();
    WorkspaceFolder::from_path(root)
}

fn change_dir(name: &str) -> PathBuf {
    workspace_a_root().join("openspec/changes").join(name)
}

fn write_tasks_md(content: &str) -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("tasks.md");
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    (tmp, path)
}

// -------------------------------------------------------------------------
// list_active_changes
// -------------------------------------------------------------------------

#[test]
fn list_active_changes_excludes_archive_and_sorts() {
    let ids = list_active_changes(&workspace_a_root()).unwrap();
    assert_eq!(
        ids,
        vec![
            "add-source-aliases".to_string(),
            "empty-tasks".to_string(),
            "many-specs".to_string(),
            "no-tasks".to_string(),
        ]
    );
}

#[test]
fn list_active_changes_returns_empty_when_dir_absent() {
    let tmp = TempDir::new().unwrap();
    // tmp has no openspec/changes inside it
    let ids = list_active_changes(tmp.path()).unwrap();
    assert!(ids.is_empty());
}

// -------------------------------------------------------------------------
// parse_artifact_status
// -------------------------------------------------------------------------

#[test]
fn parse_artifact_status_full_change_has_everything() {
    let status = parse_artifact_status(&change_dir("add-source-aliases"));
    assert!(status.proposal);
    assert!(status.design);
    assert!(status.tasks);
    assert_eq!(status.specs, vec!["alias-flag".to_string()]);
}

#[test]
fn parse_artifact_status_proposal_only() {
    let status = parse_artifact_status(&change_dir("no-tasks"));
    assert!(status.proposal);
    assert!(!status.design);
    assert!(!status.tasks);
    assert!(status.specs.is_empty());
}

#[test]
fn parse_artifact_status_returns_sorted_capability_names() {
    let status = parse_artifact_status(&change_dir("many-specs"));
    assert!(status.proposal);
    assert!(!status.tasks);
    assert_eq!(
        status.specs,
        vec!["feature-a", "feature-b", "feature-c"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
    );
}

#[test]
fn parse_artifact_status_empty_directory_reports_nothing() {
    let tmp = TempDir::new().unwrap();
    let status = parse_artifact_status(tmp.path());
    assert!(!status.proposal);
    assert!(!status.design);
    assert!(!status.tasks);
    assert!(status.specs.is_empty());
}

#[test]
fn parse_artifact_status_ignores_capability_dir_without_spec_md() {
    let tmp = TempDir::new().unwrap();
    let bad_capability = tmp.path().join("specs/no-spec-here");
    fs::create_dir_all(&bad_capability).unwrap();
    // no spec.md inside

    let good_capability = tmp.path().join("specs/has-spec");
    fs::create_dir_all(&good_capability).unwrap();
    fs::write(good_capability.join("spec.md"), "# has-spec\n").unwrap();

    let status = parse_artifact_status(tmp.path());
    assert_eq!(status.specs, vec!["has-spec".to_string()]);
}

// -------------------------------------------------------------------------
// parse_tasks_md
// -------------------------------------------------------------------------

#[test]
fn parse_tasks_md_returns_empty_for_missing_file() {
    let tmp = TempDir::new().unwrap();
    let parsed = parse_tasks_md(&tmp.path().join("does-not-exist.md")).unwrap();
    assert_eq!(parsed.sections.len(), 0);
    assert_eq!(parsed.total_tasks, 0);
    assert_eq!(parsed.completed_tasks, 0);
}

#[test]
fn parse_tasks_md_handles_sectioned_mixed_completion_with_orphan() {
    let parsed = parse_tasks_md(&change_dir("add-source-aliases").join("tasks.md")).unwrap();

    // 4 sections, with task counts [3, 2, 2, 2]
    let titles: Vec<_> = parsed.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "1. Parser changes",
            "2. Help text",
            "3. Tests",
            "4. Verification"
        ]
    );

    let task_counts: Vec<_> = parsed.sections.iter().map(|s| s.tasks.len()).collect();
    assert_eq!(task_counts, vec![3, 2, 2, 2]);

    // Orphan task before any section is counted, but does not appear in sections.
    // Orphan: 1 task, 1 completed
    // Section 1: 1.1 [x], 1.2 [x], 1.2.1 [ ] → 2 completed of 3
    // Section 2: 2.1 [x], 2.2 [ ] → 1 completed of 2
    // Section 3: 3.1 [x], 3.2 [x] → 2 completed of 2
    // Section 4: 4.1 [ ], 4.2 [ ] → 0 completed of 2
    assert_eq!(parsed.total_tasks, 10);
    assert_eq!(parsed.completed_tasks, 6);
}

#[test]
fn parse_tasks_md_captures_indent_and_line_numbers() {
    let (_tmp, path) = write_tasks_md(
        "## Section\n\
         \n\
         - [ ] zero indent\n\
         \x20\x20- [x] two-space indent\n\
         \x20\x20\x20\x20- [ ] four-space indent\n",
    );

    let parsed = parse_tasks_md(&path).unwrap();
    let tasks = &parsed.sections[0].tasks;
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].indent, 0);
    assert_eq!(tasks[0].line_number, 3);
    assert_eq!(tasks[1].indent, 2);
    assert_eq!(tasks[1].line_number, 4);
    assert_eq!(tasks[2].indent, 4);
    assert_eq!(tasks[2].line_number, 5);
}

#[test]
fn parse_tasks_md_recognises_uppercase_checkbox() {
    let (_tmp, path) = write_tasks_md("## S\n\n- [X] done\n- [x] also done\n- [ ] todo\n");
    let parsed = parse_tasks_md(&path).unwrap();
    assert_eq!(parsed.total_tasks, 3);
    assert_eq!(parsed.completed_tasks, 2);
}

#[test]
fn parse_tasks_md_ignores_malformed_checkbox() {
    let (_tmp, path) = write_tasks_md(
        "## S\n\n\
         - [?] unrecognised checkbox\n\
         - [] missing space\n\
         -[ ] missing leading space\n\
         - [ ] valid task\n",
    );
    let parsed = parse_tasks_md(&path).unwrap();
    assert_eq!(parsed.total_tasks, 1);
    assert_eq!(parsed.sections[0].tasks[0].text, "valid task");
}

#[test]
fn parse_tasks_md_returns_no_sections_when_file_has_no_headings() {
    let parsed = parse_tasks_md(&change_dir("empty-tasks").join("tasks.md")).unwrap();
    assert!(parsed.sections.is_empty());
    assert_eq!(parsed.total_tasks, 0);
    assert_eq!(parsed.completed_tasks, 0);
}

// -------------------------------------------------------------------------
// parse_proposal_title
// -------------------------------------------------------------------------

#[test]
fn parse_proposal_title_strips_hash_and_whitespace() {
    let title = parse_proposal_title(&change_dir("add-source-aliases").join("proposal.md"));
    assert_eq!(
        title,
        Some("Add `--source`/`--destination` aliases for `-s`/`-d` flags".to_string())
    );
}

#[test]
fn parse_proposal_title_strips_proposal_prefix() {
    // no-tasks fixture: `# Proposal: Bare proposal change`
    let title = parse_proposal_title(&change_dir("no-tasks").join("proposal.md"));
    assert_eq!(title, Some("Bare proposal change".to_string()));
}

#[test]
fn parse_proposal_title_handles_case_insensitive_prefix() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("proposal.md");
    fs::write(&path, "# proposal: lowercase prefix\n").unwrap();
    assert_eq!(
        parse_proposal_title(&path),
        Some("lowercase prefix".to_string())
    );
}

#[test]
fn parse_proposal_title_returns_none_for_missing_file() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(parse_proposal_title(&tmp.path().join("missing.md")), None);
}

#[test]
fn parse_proposal_title_returns_none_for_empty_or_hash_only() {
    let tmp = TempDir::new().unwrap();

    let empty = tmp.path().join("empty.md");
    fs::write(&empty, "").unwrap();
    assert_eq!(parse_proposal_title(&empty), None);

    let hash_only = tmp.path().join("hash.md");
    fs::write(&hash_only, "#\n").unwrap();
    assert_eq!(parse_proposal_title(&hash_only), None);
}

#[test]
fn parse_proposal_title_returns_none_for_plain_text_first_line() {
    // Pre-h1-only behaviour displayed any first line as the title; body text
    // now yields no title and the UI falls back to the change id.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("plain.md");
    fs::write(&path, "Just a plain line\n\nbody\n").unwrap();
    assert_eq!(parse_proposal_title(&path), None);
}

#[test]
fn parse_proposal_title_returns_none_for_deeper_headings() {
    let tmp = TempDir::new().unwrap();

    // The spec-driven template opens with `## Why` — never a title.
    let template = tmp.path().join("template.md");
    fs::write(&template, "## Why\n\nBecause reasons.\n").unwrap();
    assert_eq!(parse_proposal_title(&template), None);

    let deep = tmp.path().join("deep.md");
    fs::write(&deep, "#### Deep heading\n").unwrap();
    assert_eq!(parse_proposal_title(&deep), None);
}

#[test]
fn parse_proposal_title_requires_whitespace_after_hash() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nospace.md");
    fs::write(&path, "#Title\n").unwrap();
    assert_eq!(parse_proposal_title(&path), None);
}

#[test]
fn parse_proposal_title_accepts_indented_h1() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("indented.md");
    fs::write(&path, "   # Indented Title\n").unwrap();
    assert_eq!(
        parse_proposal_title(&path),
        Some("Indented Title".to_string())
    );
}

#[test]
fn parse_proposal_title_finds_title_below_blank_lines() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("blanks.md");
    fs::write(&path, "\n\n   \n# After Blanks\n").unwrap();
    assert_eq!(
        parse_proposal_title(&path),
        Some("After Blanks".to_string())
    );
}

#[test]
fn parse_proposal_title_finds_title_below_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("frontmatter.md");
    fs::write(
        &path,
        "---\nstatus: draft\nowner: someone\n---\n\n# After Frontmatter\n",
    )
    .unwrap();
    assert_eq!(
        parse_proposal_title(&path),
        Some("After Frontmatter".to_string())
    );
}

#[test]
fn parse_proposal_title_finds_title_below_html_comments() {
    let tmp = TempDir::new().unwrap();

    let single = tmp.path().join("single.md");
    fs::write(&single, "<!-- template note -->\n# After Comment\n").unwrap();
    assert_eq!(
        parse_proposal_title(&single),
        Some("After Comment".to_string())
    );

    let multi = tmp.path().join("multi.md");
    fs::write(
        &multi,
        "<!-- a comment\nspanning several\nlines -->\n\n# After Block Comment\n",
    )
    .unwrap();
    assert_eq!(
        parse_proposal_title(&multi),
        Some("After Block Comment".to_string())
    );
}

#[test]
fn parse_proposal_title_skips_combined_preamble() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("combined.md");
    fs::write(
        &path,
        "\n---\nstatus: draft\n---\n\n<!-- one -->\n<!-- two\n-->\n\n# Proposal: Combined\n",
    )
    .unwrap();
    assert_eq!(parse_proposal_title(&path), Some("Combined".to_string()));
}

#[test]
fn parse_proposal_title_returns_none_for_unterminated_blocks() {
    let tmp = TempDir::new().unwrap();

    let frontmatter = tmp.path().join("open-frontmatter.md");
    fs::write(&frontmatter, "---\nstatus: draft\n# Not A Title\n").unwrap();
    assert_eq!(parse_proposal_title(&frontmatter), None);

    let comment = tmp.path().join("open-comment.md");
    fs::write(&comment, "<!-- never closed\n# Not A Title\n").unwrap();
    assert_eq!(parse_proposal_title(&comment), None);
}

#[test]
fn parse_proposal_title_never_scans_into_the_body() {
    // An h1 after the first content line — here inside a fenced code block —
    // must not be mistaken for the title.
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("body-h1.md");
    fs::write(
        &path,
        "## Why\n\nSome text.\n\n```sh\n# a shell comment, not a heading\n```\n# Late H1\n",
    )
    .unwrap();
    assert_eq!(parse_proposal_title(&path), None);
}

// -------------------------------------------------------------------------
// parse_change
// -------------------------------------------------------------------------

#[test]
fn parse_change_aggregates_all_artifacts() {
    let workspace = workspace_a_folder();
    let dir = change_dir("add-source-aliases");
    let data = parse_change(&dir, "add-source-aliases", &workspace).unwrap();

    assert_eq!(data.change_id, "add-source-aliases");
    assert_eq!(
        data.title.as_deref(),
        Some("Add `--source`/`--destination` aliases for `-s`/`-d` flags")
    );
    assert_eq!(data.sections.len(), 4);
    assert_eq!(data.total_tasks, 10);
    assert_eq!(data.completed_tasks, 6);
    assert!(data.artifacts.proposal);
    assert!(data.artifacts.design);
    assert!(data.artifacts.tasks);
    assert_eq!(data.artifacts.specs, vec!["alias-flag".to_string()]);
    assert_eq!(data.workspace.uri, workspace.uri);
}

#[test]
fn parse_change_handles_proposal_only_change() {
    let workspace = workspace_a_folder();
    let dir = change_dir("no-tasks");
    let data = parse_change(&dir, "no-tasks", &workspace).unwrap();

    assert_eq!(data.change_id, "no-tasks");
    assert_eq!(data.title.as_deref(), Some("Bare proposal change"));
    assert!(data.sections.is_empty());
    assert_eq!(data.total_tasks, 0);
    assert_eq!(data.completed_tasks, 0);
    assert!(data.artifacts.proposal);
    assert!(!data.artifacts.tasks);
}

#[test]
fn parse_change_handles_missing_proposal() {
    let workspace = workspace_a_folder();
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("tasks.md"), "## S\n- [x] done\n").unwrap();

    let data = parse_change(tmp.path(), "synthetic", &workspace).unwrap();
    assert_eq!(data.title, None);
    assert_eq!(data.total_tasks, 1);
    assert_eq!(data.completed_tasks, 1);
}

// -------------------------------------------------------------------------
// parse_all_changes
// -------------------------------------------------------------------------

#[test]
fn parse_all_changes_returns_four_actives_for_workspace_a() {
    let workspace = workspace_a_folder();
    let changes = parse_all_changes(&workspace).unwrap();
    let ids: Vec<_> = changes.iter().map(|c| c.change_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "add-source-aliases",
            "empty-tasks",
            "many-specs",
            "no-tasks"
        ]
    );

    // archive/done-feature must not appear
    assert!(!ids.contains(&"archive"));
    assert!(!ids.contains(&"done-feature"));
}

#[test]
fn parse_all_changes_returns_empty_when_changes_dir_absent() {
    let tmp = TempDir::new().unwrap();
    let workspace = WorkspaceFolder::from_path(tmp.path().to_path_buf());
    let changes = parse_all_changes(&workspace).unwrap();
    assert!(changes.is_empty());
}

// -------------------------------------------------------------------------
// integration: parse_all_changes wires together everything
// -------------------------------------------------------------------------

#[test]
fn parse_all_changes_wires_titles_and_artifacts_through() {
    let workspace = workspace_a_folder();
    let changes = parse_all_changes(&workspace).unwrap();

    let by_id: std::collections::HashMap<&str, &openspec_core::ChangeData> =
        changes.iter().map(|c| (c.change_id.as_str(), c)).collect();

    let full = by_id["add-source-aliases"];
    assert!(full.artifacts.tasks);
    assert_eq!(full.completed_tasks, 6);

    let proposal_only = by_id["no-tasks"];
    assert_eq!(proposal_only.title.as_deref(), Some("Bare proposal change"));

    let many_specs = by_id["many-specs"];
    assert_eq!(many_specs.artifacts.specs.len(), 3);

    let empty_tasks = by_id["empty-tasks"];
    assert!(empty_tasks.artifacts.tasks);
    assert_eq!(empty_tasks.total_tasks, 0);
}

// Silence dead-code lint when the helper is unused in some configurations.
#[allow(dead_code)]
fn _silence(_p: &Path) {}

#[test]
fn archive_dir_logical_id_strips_only_a_valid_date_prefix() {
    // A dated archive name resolves to the bare logical id.
    assert_eq!(archive_dir_logical_id("2026-06-04-foo"), "foo");
    // Hyphenated ids keep every segment after the date: an unrelated entry
    // like `2026-06-04-x-foo` must NOT collapse to `foo`, or a deletion of
    // `foo` would be misread as the archival of `x-foo` (and vice versa).
    assert_eq!(archive_dir_logical_id("2026-06-04-x-foo"), "x-foo");
    // Names without a valid leading date pass through unchanged.
    assert_eq!(archive_dir_logical_id("foo"), "foo");
    assert_eq!(archive_dir_logical_id("not-a-date-foo"), "not-a-date-foo");
    // A bare date with no id after it is not a strippable prefix.
    assert_eq!(archive_dir_logical_id("2026-06-04-"), "2026-06-04-");
}

#[test]
fn archive_dir_date_extracts_only_a_valid_date_prefix() {
    assert_eq!(archive_dir_date("2026-06-07-beta"), Some("2026-06-07"));
    assert_eq!(archive_dir_date("2026-06-07-x-foo"), Some("2026-06-07"));
    // No date prefix → None.
    assert_eq!(archive_dir_date("legacy-gamma"), None);
    assert_eq!(archive_dir_date("not-a-date-foo"), None);
    // A bare date with no id after it is not a valid prefix.
    assert_eq!(archive_dir_date("2026-06-04-"), None);
}

/// Build a temp workspace with three archive directories: two dated and one
/// legacy (un-dated), the newest carrying a real `tasks.md` so we can prove the
/// lightweight paths never parse it.
fn temp_archive_workspace() -> (TempDir, WorkspaceFolder) {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("openspec/changes/archive");

    let beta = archive.join("2026-06-07-beta");
    fs::create_dir_all(&beta).unwrap();
    fs::write(beta.join("proposal.md"), "# Beta change\n\nbody\n").unwrap();
    fs::write(
        beta.join("tasks.md"),
        "## 1. G\n\n- [x] 1.1 a\n- [ ] 1.2 b\n",
    )
    .unwrap();

    let alpha = archive.join("2026-06-05-alpha");
    fs::create_dir_all(&alpha).unwrap();
    fs::write(alpha.join("proposal.md"), "# Alpha change\n").unwrap();

    let gamma = archive.join("legacy-gamma");
    fs::create_dir_all(&gamma).unwrap();
    fs::write(gamma.join("proposal.md"), "# Gamma change\n").unwrap();

    let ws = WorkspaceFolder::from_path(tmp.path().to_path_buf());
    (tmp, ws)
}

#[test]
fn list_archived_summaries_strips_date_reads_title_newest_first() {
    let (_tmp, ws) = temp_archive_workspace();
    let summaries = list_archived_summaries(&ws.uri).unwrap();

    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
    // Newest-first by date; the un-dated legacy entry sorts last.
    assert_eq!(ids, vec!["beta", "alpha", "legacy-gamma"]);

    assert_eq!(summaries[0].date.as_deref(), Some("2026-06-07"));
    assert_eq!(summaries[0].title.as_deref(), Some("Beta change"));
    assert_eq!(summaries[1].date.as_deref(), Some("2026-06-05"));
    assert_eq!(summaries[1].title.as_deref(), Some("Alpha change"));
    // Legacy directory: no date, but its title still comes through.
    assert_eq!(summaries[2].date, None);
    assert_eq!(summaries[2].title.as_deref(), Some("Gamma change"));
}

#[test]
fn list_archived_summaries_empty_when_no_archive_dir() {
    let tmp = TempDir::new().unwrap();
    assert!(list_archived_summaries(tmp.path()).unwrap().is_empty());
}

#[test]
fn list_archived_stubs_carry_no_parsed_content() {
    let (_tmp, ws) = temp_archive_workspace();
    let stubs = list_archived_stubs(&ws).unwrap();

    // One stub per archive directory, keyed by the (dated) directory name so
    // the aggregator's logical diff can detect archive transitions.
    let beta = stubs
        .iter()
        .find(|c| c.change_id == "2026-06-07-beta")
        .expect("beta stub present");

    // Despite beta having a real proposal.md and tasks.md on disk, the stub is
    // empty: the aggregation path reads no change content — only the directory
    // listing. This is what keeps the archive off the watcher hot path.
    assert_eq!(beta.title, None);
    assert_eq!(beta.total_tasks, 0);
    assert_eq!(beta.completed_tasks, 0);
    assert!(beta.sections.is_empty());
    assert!(!beta.artifacts.tasks);
    assert!(!beta.artifacts.proposal);
}
