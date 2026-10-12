//! Render a laid-out [`CommitGraph`] into a vertical box-drawing rail.
//!
//! Topology is computed in `openspec_core::graph` — this module only maps
//! `(row, column, band)` to terminal cells, one text line per commit. Each
//! [`EdgeSegment`](openspec_core::EdgeSegment) spanning `band → band+1` is
//! drawn on the lower row as a straight vertical (`│`) or an elbow folding into
//! its destination lane, painted in that lane's colour (matching the desktop
//! rail's "edge takes the colour it flows into" rule). Diagonals collapse to
//! single-row elbows because rows are unit-height; the node `●` is stamped last
//! so it always wins its own cell.

use openspec_core::{CommitGraph, RefKind};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::{theme, Slot, Theme};

/// Hard cap on rendered lanes so a pathological fan never eats the subject.
const MAX_LANES: usize = 12;

/// One styled line per commit (plus a trailing notice when truncated), in
/// the process-wide theme. The `selected` row index is drawn reversed;
/// callers own the surrounding block and scroll offset.
///
/// The composition of [`rail_lines`] and [`highlight`]: the History screen
/// caches the first and applies the second per frame, so this whole-rail
/// form remains only as the reference its tests compare against.
#[cfg(test)]
pub fn commit_rail(graph: &CommitGraph, selected: usize) -> Vec<Line<'static>> {
    commit_rail_with(graph, selected, theme())
}

/// [`commit_rail`] under an explicit theme, so a test can pin the scheme
/// rather than share the process-wide one with tests that flip it.
#[cfg(test)]
pub fn commit_rail_with(graph: &CommitGraph, selected: usize, th: &Theme) -> Vec<Line<'static>> {
    let mut lines = rail_lines(graph, th);
    if selected < graph.commits.len() {
        if let Some(line) = lines.get_mut(selected) {
            *line = highlight(std::mem::take(line));
        }
    }
    lines
}

/// The selected row's presentation, applied at draw time so moving the
/// selection never rebuilds the rail.
pub fn highlight(line: Line<'_>) -> Line<'_> {
    line.style(Style::default().add_modifier(Modifier::REVERSED))
}

/// One styled line per commit with no row selected (plus a trailing notice
/// when truncated) — what the History screen caches per loaded graph and
/// scheme. `th` supplies the lane and chip colours.
///
/// Linear in commits plus edges: edge indices are bucketed by `band` first,
/// so a row visits only the edges that touch it (`band == row` or
/// `band + 1 == row`). Each bucket keeps `graph.edges` order, which the
/// diagonal pass relies on — the last edge to cross a cell owns it. Runs of
/// cells sharing a colour become one span rather than one `String` per cell.
pub fn rail_lines(graph: &CommitGraph, th: &Theme) -> Vec<Line<'static>> {
    if graph.commits.is_empty() {
        return vec![dim("No commits")];
    }
    let width = graph.lane_count.min(MAX_LANES);
    let mut lines = Vec::with_capacity(graph.commits.len() + 1);

    let rows = graph.commits.iter().map(|c| c.row + 1).max().unwrap_or(0);
    let mut by_band: Vec<Vec<usize>> = vec![Vec::new(); rows];
    for (i, e) in graph.edges.iter().enumerate() {
        if let Some(bucket) = by_band.get_mut(e.band) {
            bucket.push(i);
        }
    }
    let no_edges: &[usize] = &[];

    let mut cells = vec![' '; width];
    let mut colors = vec![Color::Reset; width];
    for c in &graph.commits {
        let r = c.row;
        cells.fill(' ');
        colors.fill(Color::Reset);
        // Edges spanning `r → r+1` (this row is their upper end) and
        // `r-1 → r` (this row is their lower end).
        let starting = by_band.get(r).map_or(no_edges, Vec::as_slice);
        let arriving = r
            .checked_sub(1)
            .and_then(|b| by_band.get(b))
            .map_or(no_edges, Vec::as_slice);

        // 1. Straight verticals crossing this row (a lane passing through).
        //    Every edge writes the same glyph and lane colour for its column,
        //    so visiting the two buckets in turn gives the same cells.
        for &i in starting.iter().chain(arriving) {
            let e = &graph.edges[i];
            if e.from_column == e.to_column && e.from_column < width {
                cells[e.from_column] = '│';
                colors[e.from_column] = th.lane(e.from_column);
            }
        }
        // 2. Diagonal edges arriving at this row from above (band + 1 == r),
        //    folded into an elbow toward the destination lane.
        for &i in arriving {
            let e = &graph.edges[i];
            if e.from_column == e.to_column {
                continue;
            }
            let (from, to) = (e.from_column, e.to_column);
            let col = th.lane(to);
            let (lo, hi) = (from.min(to), from.max(to));
            for x in (lo..=hi).take_while(|&x| x < width) {
                cells[x] = if cells[x] == '│' { '┼' } else { '─' };
                colors[x] = col;
            }
            // The edge drops from `from` (above) into `to` (below).
            if to < from {
                set(&mut cells, &mut colors, to, '╭', col, width);
                set(&mut cells, &mut colors, from, '╯', col, width);
            } else {
                set(&mut cells, &mut colors, from, '╰', col, width);
                set(&mut cells, &mut colors, to, '╮', col, width);
            }
        }
        // 3. The node, stamped last so it owns its cell.
        if c.column < width {
            cells[c.column] = '●';
            colors[c.column] = th.lane(c.column);
        }

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(c.refs.len() + 4);
        let mut run = String::new();
        let mut run_color = Color::Reset;
        for (&ch, &col) in cells.iter().zip(&colors) {
            if col != run_color && !run.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut run),
                    Style::default().fg(run_color),
                ));
            }
            run_color = col;
            run.push(ch);
        }
        if !run.is_empty() {
            spans.push(Span::styled(run, Style::default().fg(run_color)));
        }
        spans.push(Span::raw(" "));
        for rf in &c.refs {
            spans.push(Span::styled(
                format!("{} ", rf.name),
                Style::default()
                    .fg(ref_color_with(&rf.kind, th))
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::raw(c.subject.clone()));
        lines.push(Line::from(spans));
    }

    if graph.truncated {
        lines.push(dim("  … older history not shown"));
    }
    lines
}

fn set(cells: &mut [char], colors: &mut [Color], i: usize, ch: char, col: Color, width: usize) {
    if i < width {
        cells[i] = ch;
        colors[i] = col;
    }
}

/// Chip colour for a ref decoration, shared with the garden renderer. Routed
/// through the active scheme so HEAD tracks the accent and the colourless path
/// (Monochrome scheme / `NO_COLOR`) drops every chip to the default ink.
pub fn ref_color(kind: &RefKind) -> Color {
    ref_color_with(kind, theme())
}

/// [`ref_color`] under an explicit theme.
fn ref_color_with(kind: &RefKind, th: &Theme) -> Color {
    match kind {
        RefKind::Head => th.accent(),
        RefKind::LocalBranch => th.slot(Slot::Success),
        RefKind::Tag => th.slot(Slot::Warn),
        // No dedicated blue slot; show ANSI blue, but honour the colourless path
        // exactly as the slots do.
        RefKind::RemoteBranch => {
            if th.slot(Slot::Accent) == Color::Reset {
                Color::Reset
            } else {
                Color::Blue
            }
        }
    }
}

fn dim(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().add_modifier(Modifier::DIM),
    ))
}

#[cfg(test)]
mod tests {
    use openspec_core::{EdgeSegment, LaidOutCommit, RefKind};

    use super::*;
    use crate::theme::{ColorDepth, Scheme};

    /// The rail algorithm as it stood before band indexing and span merging,
    /// kept verbatim (theme injected) as the oracle `rail_lines` must match.
    fn reference_rail(graph: &CommitGraph, selected: usize, th: &Theme) -> Vec<Line<'static>> {
        if graph.commits.is_empty() {
            return vec![dim("No commits")];
        }
        let width = graph.lane_count.min(MAX_LANES);
        let mut lines = Vec::with_capacity(graph.commits.len() + 1);
        for c in &graph.commits {
            let r = c.row;
            let mut cells = vec![' '; width];
            let mut colors = vec![Color::Reset; width];
            for e in &graph.edges {
                if e.from_column == e.to_column
                    && e.from_column < width
                    && (e.band == r || e.band + 1 == r)
                {
                    cells[e.from_column] = '│';
                    colors[e.from_column] = th.lane(e.from_column);
                }
            }
            for e in &graph.edges {
                if e.from_column == e.to_column || e.band + 1 != r {
                    continue;
                }
                let (from, to) = (e.from_column, e.to_column);
                let col = th.lane(to);
                let (lo, hi) = (from.min(to), from.max(to));
                for x in lo..=hi {
                    if x >= width {
                        continue;
                    }
                    cells[x] = if cells[x] == '│' { '┼' } else { '─' };
                    colors[x] = col;
                }
                if to < from {
                    set(&mut cells, &mut colors, to, '╭', col, width);
                    set(&mut cells, &mut colors, from, '╯', col, width);
                } else {
                    set(&mut cells, &mut colors, from, '╰', col, width);
                    set(&mut cells, &mut colors, to, '╮', col, width);
                }
            }
            if c.column < width {
                cells[c.column] = '●';
                colors[c.column] = th.lane(c.column);
            }
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(width + 4);
            for (ch, col) in cells.into_iter().zip(colors) {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(col)));
            }
            spans.push(Span::raw(" "));
            for rf in &c.refs {
                spans.push(Span::styled(
                    format!("{} ", rf.name),
                    Style::default()
                        .fg(ref_color_with(&rf.kind, th))
                        .add_modifier(Modifier::BOLD),
                ));
            }
            spans.push(Span::raw(c.subject.clone()));
            let mut line = Line::from(spans);
            if r == selected {
                line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            lines.push(line);
        }
        if graph.truncated {
            lines.push(dim("  … older history not shown"));
        }
        lines
    }

    /// Each line as the terminal would paint it: one `(symbol, style)` per
    /// character, the line's style patched under each span's. Comparing cells
    /// rather than spans is what lets merged runs equal per-cell spans.
    fn cells(lines: &[Line<'_>]) -> Vec<Vec<(char, Style)>> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .flat_map(|span| {
                        let style = line.style.patch(span.style);
                        span.content.chars().map(move |ch| (ch, style))
                    })
                    .collect()
            })
            .collect()
    }

    /// A deterministic pseudo-random sequence (64-bit LCG), so the fixture is
    /// large and irregular without depending on a crate or the clock.
    struct Lcg(u64);
    impl Lcg {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    /// Several hundred commits over more lanes than the rail draws, with
    /// verticals and diagonals in every band — several diagonals crossing the
    /// same cells of one band — and a few refs.
    fn tangled_graph(commits: usize, lanes: usize) -> CommitGraph {
        let mut rng = Lcg(0x5eed);
        let commits_v: Vec<LaidOutCommit> = (0..commits)
            .map(|row| LaidOutCommit {
                id: format!("sha{row}"),
                parents: Vec::new(),
                author: "Dev".to_string(),
                date: "2026-06-16T00:00:00Z".to_string(),
                subject: format!("commit {row}"),
                refs: if row % 37 == 0 {
                    vec![openspec_core::CommitRef {
                        name: format!("v{row}"),
                        kind: if row % 2 == 0 {
                            RefKind::Tag
                        } else {
                            RefKind::LocalBranch
                        },
                    }]
                } else {
                    Vec::new()
                },
                trailers: Vec::new(),
                row,
                column: rng.below(lanes),
            })
            .collect();
        let mut edges = Vec::new();
        for band in 0..commits {
            for _ in 0..rng.below(6) {
                let from = rng.below(lanes);
                let to = if rng.below(3) == 0 {
                    rng.below(lanes)
                } else {
                    from
                };
                edges.push(EdgeSegment {
                    band,
                    from_column: from,
                    to_column: to,
                });
            }
            // Two diagonals over the same span in opposite directions, so the
            // last one in edge order must own the shared cells.
            if band % 5 == 0 {
                edges.push(EdgeSegment {
                    band,
                    from_column: 1,
                    to_column: 6,
                });
                edges.push(EdgeSegment {
                    band,
                    from_column: 6,
                    to_column: 2,
                });
            }
        }
        // Edges whose band lies past the last row are never drawn.
        edges.push(EdgeSegment {
            band: commits + 3,
            from_column: 0,
            to_column: 1,
        });
        CommitGraph {
            commits: commits_v,
            edges,
            lane_count: lanes,
            truncated: true,
        }
    }

    fn pinned_theme(scheme: Scheme) -> Theme {
        let th = Theme::for_test(ColorDepth::TrueColor, false, false);
        th.set_scheme(scheme);
        th
    }

    #[test]
    fn the_band_indexed_rail_matches_the_reference_algorithm() {
        for lanes in [3, 8, MAX_LANES + 4] {
            let graph = tangled_graph(400, lanes);
            for scheme in [Scheme::Default, Scheme::Nord, Scheme::Mono] {
                let th = pinned_theme(scheme);
                let unselected = rail_lines(&graph, &th);
                assert_eq!(
                    cells(&unselected),
                    cells(&reference_rail(&graph, usize::MAX, &th)),
                    "lanes {lanes}, {scheme:?}, nothing selected"
                );
                for selected in [0, 1, 199, 399] {
                    assert_eq!(
                        cells(&commit_rail_with(&graph, selected, &th)),
                        cells(&reference_rail(&graph, selected, &th)),
                        "lanes {lanes}, {scheme:?}, row {selected} selected"
                    );
                }
            }
        }
    }

    #[test]
    fn merged_runs_use_fewer_spans_than_cells() {
        let graph = tangled_graph(50, 8);
        let th = pinned_theme(Scheme::Default);
        let ours: usize = rail_lines(&graph, &th).iter().map(|l| l.spans.len()).sum();
        let reference: usize = reference_rail(&graph, usize::MAX, &th)
            .iter()
            .map(|l| l.spans.len())
            .sum();
        assert!(ours < reference, "{ours} spans vs {reference}");
    }

    #[test]
    fn an_empty_graph_says_so_and_is_never_highlighted() {
        let graph = CommitGraph {
            commits: Vec::new(),
            edges: Vec::new(),
            lane_count: 0,
            truncated: false,
        };
        let th = pinned_theme(Scheme::Default);
        let lines = commit_rail_with(&graph, 0, &th);
        assert_eq!(cells(&lines), cells(&reference_rail(&graph, 0, &th)));
        assert!(!lines[0].style.add_modifier.contains(Modifier::REVERSED));
    }
}
