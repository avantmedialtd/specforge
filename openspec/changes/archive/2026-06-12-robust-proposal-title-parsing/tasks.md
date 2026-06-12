# Tasks: Robust Proposal-Title Extraction

## 1. Parser

- [x] 1.1 Rewrite `parse_proposal_title` in `crates/openspec-core/src/parser.rs`: skip ignorable preamble (blank lines, one leading `---` YAML frontmatter block, HTML comment blocks), then examine exactly one content line — accept it as the title only when it is a single `#` followed by whitespace and non-empty text; keep the case-insensitive `Proposal:` prefix strip and trim; return `None` otherwise (including unterminated preamble blocks). Update the doc comment to state the new contract.

## 2. Tests

- [x] 2.1 Extend the title tests (`crates/openspec-core/tests/parser.rs`) to cover: line-1 `# Title` (regression), legacy `# Proposal: X`, title below blank lines / frontmatter / single- and multi-line HTML comments / combinations, `## Why` first → `None`, body text first → `None`, `#Title` without space → `None`, `#` with empty text → `None`, unterminated frontmatter or comment → `None`, h1 only inside a later fenced code block → `None`, missing file → `None`, indented `   # Title` → accepted.

## 3. Sidebar

- [x] 3.1 In `src/components/WorkspaceTree.tsx`, label `InstanceNode`'s flattened-singleton row with `instance.change.title` (via `stripInlineMarkdown`, falling back to `changeName`), and set the row's `title` tooltip attribute to the logical change name when the proposal title is displayed. Leave the multi-instance disclosure parent and child rows untouched.

## 4. Verification

- [x] 4.1 `cargo test` (workspace), `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all --check` pass; confirm no call sites of `parse_proposal_title` needed changes.
- [x] 4.2 `bun run build` passes (tsc strict). Visually verify in the dev app (`bun run wt:dev`) that this change's own worktree row shows "Robust Proposal-Title Extraction" on line 1 with the branch chip below, and that hovering reveals the kebab change name. *(Confirmed by the user in the slot-1 dev app.)*
