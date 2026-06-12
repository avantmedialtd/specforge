# Robust Proposal-Title Extraction

## Why

Change rows across the app (sidebar, archive browser, dashboard ships feed) label themselves with the title parsed from `proposal.md`. Today `parse_proposal_title` reads **only the first line** of the file and strips **any number** of leading `#` characters. That contract was built for the legacy proposal format, whose line 1 was always `# Proposal: <x>` — and it silently broke when the OpenSpec spec-driven schema arrived:

- The current `openspec new change` scaffold creates no `proposal.md` at all, and the spec-driven proposal template has **no title line** — it opens with `## Why`. A template-faithful proposal therefore parses to the absurd title **"Why"** (the `##` is stripped like an h1).
- A real `# Title` preceded by a blank line, YAML frontmatter, or an HTML comment is invisible, because parsing never looks past line 1.
- *Any* first line becomes the title — a frontmatter `---` fence or a `**Status:** Draft` line is happily displayed as the change's name.

This is why proposal titles stopped appearing in the sidebar for workspaces using the new schema: their proposals either don't exist yet or carry no line-1 h1, and the rows fall back to kebab-case change IDs.

There is a second, independent suppressor: **git-singleton (worktree) rows never display the title at all**. The *Two-Line Sole-Change-Row Layout* requirement pins their line 1 to the logical change name, and `InstanceNode` (`src/components/WorkspaceTree.tsx:1052`) renders `changeName` even though `instance.change.title` is already present in the data. So a change browsed through worktree discovery shows kebab-case even when its proposal has a perfect `# Title`.

## What Changes

**1. Rewrite the title-extraction contract** in `parse_proposal_title` (`crates/openspec-core/src/parser.rs`):

- **Skip ignorable preamble** before looking for a title: blank lines, a leading YAML frontmatter block (`---` … `---`), and HTML comments (`<!-- … -->`, single- or multi-line).
- **Only a true h1 counts**: the first content line after the preamble yields a title only if it is a single `#` followed by whitespace. `## Why`, deeper headings, and arbitrary text yield no title (the UI keeps its existing change-ID fallback).
- **Never scan past the first content line** — a `#` inside a fenced code block or later in the body can never be mistaken for the title.
- The legacy case-insensitive `Proposal:` prefix strip is preserved, as is the existing fast path (line-1 `# Title` proposals parse exactly as before).

**2. Show the title on git-singleton rows**: the flattened singleton worktree row's line 1 becomes the proposal title, falling back to the logical change name; the kebab name stays recoverable via the row's hover tooltip. Multi-instance disclosure parents keep the logical name — there the name is the cross-worktree join key, and instances could momentarily disagree on title mid-edit. No backend work: `instance.change.title` already crosses the IPC boundary.

Every other consumer (`parse_change`, the archive listing, the dashboard ships feed) already treats the title as `Option<String>` with an ID fallback and needs no changes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `spec-browser`: adds a requirement defining how the proposal title is extracted from `proposal.md` (h1-only, preamble-tolerant, legacy `Proposal:` prefix stripped) — the archive browser and dashboard inherit the same extraction since they call the same parser; and modifies *Two-Line Sole-Change-Row Layout* so a git singleton's line 1 shows the proposal title (falling back to the logical change name) instead of always the change name.

## Impact

- `crates/openspec-core/src/parser.rs` — `parse_proposal_title` body; signature and all call sites unchanged.
- `crates/openspec-core/tests/parser.rs` — new unit coverage for the extraction rules.
- `src/components/WorkspaceTree.tsx` — `InstanceNode`'s singleton branch label and tooltip; no type or IPC changes.
- Behaviour change (parser): a proposal whose first content line is not an h1 (e.g. the template's `## Why`, or junk like `---`) now yields **no title** instead of a garbage title; rows fall back to the change ID as they already do for missing proposals.
- Behaviour change (UI): worktree singleton rows show the proposal title when one exists; rows without a title are unchanged.
- No platform-specific code.
