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

use crate::theme::theme;

const ACCENT: Color = Color::Cyan;
/// Hard cap on rendered lanes so a pathological fan never eats the subject.
const MAX_LANES: usize = 12;

/// One styled line per commit (plus a trailing notice when truncated). The
/// `selected` row index is drawn reversed; callers own the surrounding block
/// and scroll offset.
pub fn commit_rail(graph: &CommitGraph, selected: usize) -> Vec<Line<'static>> {
    if graph.commits.is_empty() {
        return vec![dim("No commits")];
    }
    let th = theme();
    let width = graph.lane_count.min(MAX_LANES);
    let mut lines = Vec::with_capacity(graph.commits.len() + 1);

    for c in &graph.commits {
        let r = c.row;
        let mut cells = vec![' '; width];
        let mut colors = vec![Color::Reset; width];

        // 1. Straight verticals crossing this row (a lane passing through).
        for e in &graph.edges {
            if e.from_column == e.to_column
                && e.from_column < width
                && (e.band == r || e.band + 1 == r)
            {
                cells[e.from_column] = '│';
                colors[e.from_column] = th.lane(e.from_column);
            }
        }
        // 2. Diagonal edges arriving at this row from above (band + 1 == r),
        //    folded into an elbow toward the destination lane.
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

        let mut spans: Vec<Span<'static>> = Vec::with_capacity(width + 4);
        for (ch, col) in cells.into_iter().zip(colors) {
            spans.push(Span::styled(ch.to_string(), Style::default().fg(col)));
        }
        spans.push(Span::raw(" "));
        for rf in &c.refs {
            spans.push(Span::styled(
                format!("{} ", rf.name),
                Style::default()
                    .fg(ref_color(&rf.kind))
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

fn set(cells: &mut [char], colors: &mut [Color], i: usize, ch: char, col: Color, width: usize) {
    if i < width {
        cells[i] = ch;
        colors[i] = col;
    }
}

/// Chip colour for a ref decoration, shared with the garden renderer.
pub fn ref_color(kind: &RefKind) -> Color {
    match kind {
        RefKind::Head => ACCENT,
        RefKind::LocalBranch => Color::Green,
        RefKind::RemoteBranch => Color::Blue,
        RefKind::Tag => Color::Yellow,
    }
}

fn dim(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().add_modifier(Modifier::DIM),
    ))
}
