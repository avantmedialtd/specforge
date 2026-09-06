use openspec_core::{
    archive_dir_date, archive_dir_logical_id, group_archived_rows, list_active_changes,
    list_archived_stubs, list_archived_summaries, parse_all_changes, parse_artifact_status,
    parse_change, parse_proposal_title, parse_tasks_md, ArchivedChangeSummary, WorkspaceFolder,
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
fn the_date_strip_is_applied_exactly_once() {
    // A change id that itself begins with a date-shaped prefix. The strip is a
    // single anchored match by design, so exactly one `YYYY-MM-DD-` comes off:
    // strip twice and `2026-06-05-add-thing` and `add-thing` would collapse
    // into one row that is really two different changes.
    const DIR: &str = "2026-06-04-2026-06-05-add-thing";
    assert_eq!(archive_dir_date(DIR), Some("2026-06-04"));
    assert_eq!(archive_dir_logical_id(DIR), "2026-06-05-add-thing");

    // Stripping once is exactly reversible, so the pair round-trips back to
    // the on-disk name — which is what makes `dir_name` a check on the split
    // rather than a second opinion about it.
    assert_eq!(
        format!(
            "{}-{}",
            archive_dir_date(DIR).unwrap(),
            archive_dir_logical_id(DIR)
        ),
        DIR
    );
}

#[test]
fn list_archived_summaries_carries_the_directory_name_verbatim() {
    let tmp = TempDir::new().unwrap();
    let archive = tmp.path().join("openspec/changes/archive");
    // Both shapes the union has to address on disk: a date-headed id, and a
    // legacy un-dated directory.
    for dir in ["2026-06-04-2026-06-05-add-thing", "legacy-gamma"] {
        fs::create_dir_all(archive.join(dir)).unwrap();
    }
    let summaries = list_archived_summaries(tmp.path()).unwrap();

    let dated = summaries
        .iter()
        .find(|s| s.id == "2026-06-05-add-thing")
        .expect("the date-headed id keeps its own date-shaped head");
    assert_eq!(dated.date.as_deref(), Some("2026-06-04"));
    assert_eq!(dated.dir_name, "2026-06-04-2026-06-05-add-thing");

    let legacy = summaries
        .iter()
        .find(|s| s.id == "legacy-gamma")
        .expect("legacy entry present");
    assert_eq!(legacy.date, None);
    assert_eq!(legacy.dir_name, "legacy-gamma");
}

// -------------------------------------------------------------------------
// group_archived_rows — the union's de-duplication, dating and ordering
// -------------------------------------------------------------------------

/// One archive listing entry as `list_archived_summaries` would build it from
/// `dir_name`, so these fixtures cannot drift from the real split.
fn summary(dir_name: &str, title: Option<&str>) -> ArchivedChangeSummary {
    ArchivedChangeSummary {
        id: archive_dir_logical_id(dir_name).to_string(),
        date: archive_dir_date(dir_name).map(str::to_string),
        title: title.map(str::to_string),
        dir_name: dir_name.to_string(),
    }
}

fn listing(worktree: &str, dirs: &[&str]) -> (PathBuf, Vec<ArchivedChangeSummary>) {
    (
        PathBuf::from(worktree),
        dirs.iter().map(|d| summary(d, None)).collect(),
    )
}

#[test]
fn group_collapses_differing_date_prefixes_and_dates_the_row_by_the_newer() {
    // The case that forces the key to be the bare logical id: two worktrees
    // archived one change on different days. Keying on the directory name
    // would render two rows — the duplication a union exists to remove.
    let rows = group_archived_rows(vec![
        listing("/wt/a", &["2026-06-04-add-thing"]),
        listing("/wt/b", &["2026-06-05-add-thing"]),
    ]);

    assert_eq!(rows.len(), 1, "one logical change is one row: {rows:?}");
    assert_eq!(rows[0].id, "add-thing");
    // Newest-date-wins, NOT first-seen and NOT oldest: the copy listed first
    // is deliberately the older one, so a `min`/first-wins rule would report
    // `2026-06-04` here.
    assert_eq!(rows[0].date.as_deref(), Some("2026-06-05"));
    // Both copies survive, each addressable by its own directory name, and
    // the newest copy leads so it is the one that opens first.
    assert_eq!(
        rows[0]
            .copies
            .iter()
            .map(|c| (c.worktree_path.to_str().unwrap(), c.archive_dir.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("/wt/b", "2026-06-05-add-thing"),
            ("/wt/a", "2026-06-04-add-thing"),
        ]
    );
}

#[test]
fn group_collapses_a_legacy_undated_directory_with_its_dated_twin() {
    let rows = group_archived_rows(vec![
        listing("/wt/legacy", &["add-thing"]),
        listing("/wt/dated", &["2026-06-04-add-thing"]),
    ]);

    assert_eq!(rows.len(), 1, "both are the same logical change: {rows:?}");
    assert_eq!(rows[0].id, "add-thing");
    // A `None` date must never displace a `Some`: the row is dated by its
    // dated copy even though the un-dated one was listed first.
    assert_eq!(rows[0].date.as_deref(), Some("2026-06-04"));
    assert_eq!(rows[0].copies.len(), 2);
    // Dated copy first; the un-dated legacy copy sorts last but stays openable.
    assert_eq!(rows[0].copies[0].archive_dir, "2026-06-04-add-thing");
    assert_eq!(rows[0].copies[0].date.as_deref(), Some("2026-06-04"));
    assert_eq!(rows[0].copies[1].archive_dir, "add-thing");
    assert_eq!(rows[0].copies[1].date, None);
}

#[test]
fn group_reports_no_date_for_a_row_whose_copies_are_all_undated() {
    let rows = group_archived_rows(vec![
        listing("/wt/a", &["legacy-thing"]),
        listing("/wt/b", &["legacy-thing"]),
    ]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].date, None, "no copy carries a date to inherit");
    // The worktree path is the total tie-break once date and directory name
    // are equal, so the copy order is still deterministic.
    assert_eq!(
        rows[0]
            .copies
            .iter()
            .map(|c| c.worktree_path.to_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["/wt/a", "/wt/b"]
    );
}

#[test]
fn group_breaks_a_date_tie_on_the_id_and_sorts_undated_rows_last() {
    // `zulu` is listed FIRST and `alpha` second, both on the same date, so a
    // missing (or reversed) tie-break shows up as `zulu` leading.
    let rows = group_archived_rows(vec![
        listing("/wt/a", &["2026-06-04-zulu", "no-date-thing"]),
        listing("/wt/b", &["2026-06-04-alpha", "2026-06-09-newest"]),
    ]);

    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["newest", "alpha", "zulu", "no-date-thing"],
        "newest date first, ties by id ascending, un-dated rows last"
    );
}

#[test]
fn group_prefers_the_title_of_the_copy_that_opens_first() {
    // Titles can differ between copies (the archive is read from the working
    // tree, so a post-archival correction can live in one worktree only). The
    // row shows the title of the copy the reader opens by default — the
    // newest — regardless of which worktree the caller listed first.
    let rows = group_archived_rows(vec![
        (
            PathBuf::from("/wt/old"),
            vec![summary("2026-06-04-add-thing", Some("Stale title"))],
        ),
        (
            PathBuf::from("/wt/new"),
            vec![summary("2026-06-05-add-thing", Some("Corrected title"))],
        ),
    ]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title.as_deref(), Some("Corrected title"));
}

#[test]
fn group_keeps_two_copies_from_one_worktree_apart() {
    // One worktree CAN hold a dated directory and its legacy un-dated twin at
    // once (`glyph_predicate.rs` builds exactly that), so the worktree path
    // alone is not a total tie-break for copies.
    let rows = group_archived_rows(vec![listing(
        "/wt/a",
        &["add-thing", "2026-06-04-add-thing"],
    )]);

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]
            .copies
            .iter()
            .map(|c| c.archive_dir.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-06-04-add-thing", "add-thing"]
    );
}

#[test]
fn group_of_no_listings_is_empty() {
    assert!(group_archived_rows(vec![]).is_empty());
    assert!(group_archived_rows(vec![listing("/wt/a", &[])]).is_empty());
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
