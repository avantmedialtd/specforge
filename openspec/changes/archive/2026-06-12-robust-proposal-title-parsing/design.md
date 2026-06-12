# Design: Robust Proposal-Title Extraction

## Context

`parse_proposal_title` (`crates/openspec-core/src/parser.rs:113`) is the single source of every change title in the app — `parse_change` (sidebar rows), the lightweight archive listing, and the dashboard's ships feed all call it. Its current contract is a relic of the legacy proposal format (`# Proposal: <x>` always on line 1): it reads only `lines().next()`, strips *all* leading `#`s regardless of heading level, strips an optional `Proposal:` prefix, and returns whatever is left.

The OpenSpec spec-driven template broke both assumptions: scaffolds carry no `proposal.md`, and the template itself opens with `## Why` (no title line). Result: template-faithful proposals are titled "Why", titles below a blank line or frontmatter are missed, and any junk first line becomes the displayed title.

Independently of the parser, git-singleton (worktree) rows never display the title: *Two-Line Sole-Change-Row Layout* pins their line 1 to the logical change name, and `InstanceNode` (`src/components/WorkspaceTree.tsx:1052`) renders `changeName` even though the parsed `instance.change.title` is already in the row's data. Since this project's own changes are always browsed through worktree discovery, they show kebab-case regardless of how well the proposal is titled.

## Goals / Non-Goals

**Goals:**
- A proposal's title is found when (and only when) the document opens with a true h1, optionally preceded by ignorable preamble.
- The template's `## Why` (and any non-h1 first content line) yields `None`, letting the existing change-ID fallback do its job.
- Existing well-formed proposals (`# Title` on line 1, legacy `# Proposal: X`) parse identically to today.
- A titled change reads as its title everywhere it appears as a sole row — including the flattened singleton worktree row.

**Non-Goals:**
- No humanized rendering of the kebab-case ID fallback (explicitly deferred — the UI keeps showing the raw change ID when no title exists).
- No changes to which files are read or when (no new I/O, no caching changes).
- No IPC-type changes.
- No title display on multi-instance disclosure parents or their child rows.
- No upstream template fix (the `openspec` CLI is external).

## Decisions

### D1: h1-only — a title must be a single `#` heading

Accept a line as the title only if, after trimming leading whitespace, it is `#` followed by at least one space or tab and non-empty text. Deeper headings (`##`+) and bare text never become titles.

Rationale: the document's h1 *is* its title by Markdown convention; an h2-opening document is simply titleless. The current strip-all-`#`s behaviour is what manufactures "Why" out of `## Why`. Requiring the space matches CommonMark (`#Title` is not a heading).

### D2: skip ignorable preamble, then examine exactly one content line

Before looking for the h1, skip in order: blank/whitespace-only lines, one leading YAML frontmatter block (only when the first content line is exactly `---`; skipped through its closing `---`), and HTML comment blocks (`<!--` through `-->`, single- or multi-line, repeated as needed). The first remaining content line is examined; if it is not an h1, return `None` immediately.

Rationale: agents and tools commonly put frontmatter or template comments above the heading, so a strict line-1 read misses real titles. But scanning the *whole document* for an h1 is dangerous — `#` lines inside fenced code blocks (shell comments) would false-positive. Examining exactly one content line after deterministic preamble-skipping recovers the real cases without ever reading into the body. An unterminated frontmatter or comment block consumes the rest of the file and yields `None`.

### D3: keep the `Proposal:` prefix strip and the `Option<String>` contract

After matching the h1, strip the optional case-insensitive `Proposal:` prefix and trim, returning `None` for an empty result — exactly today's post-match behaviour. The function signature (`&Path -> Option<String>`) and all call sites stay untouched; missing/unreadable files still return `None`.

Rationale: legacy workspaces still display correctly, and the change stays invisible to every consumer except in what title it yields.

### D4: singleton worktree rows show `title ?? changeName`; multi-instance parents keep the name

`InstanceNode`'s flattened-singleton branch labels line 1 with `instance.change.title` (stripped of inline Markdown, like `FlatChangeNode` does) and falls back to `changeName`. When the title is shown, the row's `title` tooltip attribute carries the logical change name — the `Row` component already supports this (it's how workspace rows surface their path). The multi-instance disclosure parent and its child rows are untouched.

Rationale: the singleton row is the sole place the change appears, so it should read like the flat-workspace row already does — title first, ID recoverable. On a multi-instance parent the kebab name is the cross-worktree join key (the thing instances are grouped by), and two worktrees can transiently disagree on title mid-edit; the name is the honest label there. No backend work is needed — `title` already crosses the IPC boundary on every `ChangeData`.

## Risks / Trade-offs

- **Behaviour change for junk titles**: documents that today display their first line (e.g. `---`, `**Status:** Draft`, `## Why` → "Why") will now show the change ID instead. This is the intended fix, but a user who somehow relied on a non-h1 first line as their "title" loses it. Mitigation: that pattern produces garbage today; the spec scenarios document the new contract explicitly.
- **Preamble heuristics are bounded, not exhaustive**: only blank lines, one frontmatter block, and HTML comments are skipped. A title below, say, a Markdown reference definition still yields `None`. Acceptable — the goal is the formats tools actually emit, and `None` degrades gracefully to the ID fallback.
- **No new dependencies**: the skip logic is a small hand-rolled line scanner; no Markdown parser crate is introduced for what is a three-rule preamble.
- **The kebab name leaves the singleton row's visible surface**: with a title on line 1 and the branch chip on line 2, the directory name appears only in the hover tooltip. Acceptable — the flat-workspace row made the same trade (title on line 1, ID demoted to line 2) and for worktree rows the branch name usually echoes the change name anyway.
