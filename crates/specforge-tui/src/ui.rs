//! `view`: a pure function of `Model`. Immediate-mode rendering with ratatui;
//! the terminal diffs frames, so redraw-on-event stays cheap.
//!
//! Five screens — Browse, Dashboard, Season, Garden, History — share a title
//! bar and key bar. The gamified screens render the headless core's typed
//! payloads directly (no JSON scraping), and the season ladder reconstructs all
//! thirty tiers by calling `openspec_core::treatment` per tier.

use std::collections::HashSet;

use openspec_core::{GardenCommit, HeatmapCell, LeaderboardEntry, Rarity, WorkspaceGarden};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{Focus, Model, Screen, TreeRow};
use crate::theme::{self, theme};
use crate::{graph, markdown};

const ACCENT: Color = Color::Cyan;
/// Below this width the two-pane Browse layout collapses to a single pane.
const TWO_PANE_MIN_WIDTH: u16 = 90;

pub fn view(f: &mut Frame, model: &Model) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    title_bar(f, chunks[0], model);
    match model.screen {
        Screen::Browse => browse(f, chunks[1], model),
        Screen::Dashboard => dashboard(f, chunks[1], model),
        Screen::Season => season(f, chunks[1], model),
        Screen::Garden => garden(f, chunks[1], model),
        Screen::History => history(f, chunks[1], model),
    }
    key_bar(f, chunks[2], model);

    if model.show_help {
        help_overlay(f);
    }
}

fn title_bar(f: &mut Frame, area: Rect, model: &Model) {
    let screen = match model.screen {
        Screen::Browse => "Browse",
        Screen::Dashboard => "Dashboard",
        Screen::Season => "Season",
        Screen::Garden => "Garden",
        Screen::History => "History",
    };
    let line = Line::from(vec![
        Span::styled(
            " SpecForge ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{screen}]"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            model.status.clone(),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn key_bar(f: &mut Frame, area: Rect, model: &Model) {
    let (hint, bg) = if model.filter_editing {
        let q = model.filter.clone().unwrap_or_default();
        (format!(" /{q}   (Enter apply · Esc cancel)"), ACCENT)
    } else {
        let h = match model.screen {
            Screen::Browse => match model.focus {
                Focus::Tree => {
                    " Tab pane · j/k move · Enter open · / search · 2-5 screens · ? help · q quit"
                }
                Focus::Detail => " [ ] tabs · j/k scroll · h tree · 2-5 screens · ? help · q quit",
            },
            Screen::History => " j/k move · m more · Esc back · 1-5 screens · ? help · q quit",
            _ => " j/k scroll · Esc back · 1-5 screens · ? help · q quit",
        };
        (h.to_string(), Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::Black).bg(bg),
        ))),
        area,
    );
}

// --- Browse ----------------------------------------------------------------

fn browse(f: &mut Frame, area: Rect, model: &Model) {
    if area.width >= TWO_PANE_MIN_WIDTH {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);
        tree_pane(f, cols[0], model);
        detail_pane(f, cols[1], model);
    } else {
        match model.focus {
            Focus::Tree => tree_pane(f, area, model),
            Focus::Detail => detail_pane(f, area, model),
        }
    }
}

fn tree_pane(f: &mut Frame, area: Rect, model: &Model) {
    let focused = matches!(model.focus, Focus::Tree);
    let block = pane_block("Workspaces", focused);
    let inner_h = area.height.saturating_sub(2) as usize;
    let query = model
        .filter
        .as_ref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    let lines: Vec<Line> = if model.visible.is_empty() {
        vec![dim("  no changes match the filter")]
    } else {
        model
            .visible
            .iter()
            .enumerate()
            .map(|(vi, &ri)| row_line(&model.rows[ri], vi == model.selected, query.as_deref()))
            .collect()
    };

    let offset = model
        .selected
        .saturating_sub(inner_h.saturating_sub(1))
        .min(model.visible.len().saturating_sub(inner_h.max(1))) as u16;

    f.render_widget(Paragraph::new(lines).block(block).scroll((offset, 0)), area);
}

fn row_line(r: &TreeRow, selected: bool, query: Option<&str>) -> Line<'static> {
    let th = theme();
    let mut spans = Vec::new();
    if r.is_header {
        let style = th.header_style(r.color);
        spans.push(Span::styled(th.glyph("▾ ", "v ").to_string(), style));
        spans.extend(highlight(&r.label, query, style));
    } else {
        let indent = "  ".repeat(r.depth as usize);
        let glyph = match r.progress {
            Some((c, t)) if t > 0 && c >= t => th.glyph("● ", "* "),
            Some((c, _)) if c > 0 => th.glyph("◐ ", "o "),
            _ => th.glyph("○ ", "- "),
        };
        spans.push(Span::raw(format!("{indent}{glyph}")));
        spans.extend(highlight(&r.label, query, Style::default()));
        if let Some((c, t)) = r.progress {
            spans.push(Span::raw(format!("  {}", progress_bar(c, t))));
        }
    }
    let mut line = Line::from(spans);
    if selected {
        line = line.style(Style::default().add_modifier(Modifier::REVERSED));
    }
    line
}

/// Split a label so the matched filter substring is underlined; falls back to a
/// single plain span when there's no query or a non-ASCII boundary intervenes.
fn highlight(label: &str, query: Option<&str>, base: Style) -> Vec<Span<'static>> {
    if let Some(q) = query {
        let lower = label.to_lowercase();
        if let Some(pos) = lower.find(q) {
            let end = pos + q.len();
            if label.is_char_boundary(pos) && label.is_char_boundary(end) {
                return vec![
                    Span::styled(label[..pos].to_string(), base),
                    Span::styled(
                        label[pos..end].to_string(),
                        base.add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                    ),
                    Span::styled(label[end..].to_string(), base),
                ];
            }
        }
    }
    vec![Span::styled(label.to_string(), base)]
}

/// A 7-cell progress bar plus the raw count, e.g. `▓▓▓▓▓░░ 5/7`.
fn progress_bar(completed: usize, total: usize) -> String {
    const WIDTH: usize = 7;
    let th = theme();
    if total == 0 {
        return format!("{} 0/0", th.glyph("—", "-"));
    }
    let filled = (completed * WIDTH).div_ceil(total).min(WIDTH);
    let bar = th.glyph("▓", "#").repeat(filled) + &th.glyph("░", ".").repeat(WIDTH - filled);
    format!("{bar} {completed}/{total}")
}

fn detail_pane(f: &mut Frame, area: Rect, model: &Model) {
    let focused = matches!(model.focus, Focus::Detail);
    let title = if model.detail_title.is_empty() {
        "Detail".to_string()
    } else {
        model.detail_title.clone()
    };
    let block = pane_block(&title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    f.render_widget(Paragraph::new(tab_strip(model)), rows[0]);
    f.render_widget(
        Paragraph::new(markdown::render(&model.detail_md))
            .wrap(Wrap { trim: false })
            .scroll((model.detail_scroll, 0)),
        rows[1],
    );
}

fn tab_strip(model: &Model) -> Line<'static> {
    if model.tabs.is_empty() {
        return dim("  (no artifacts)");
    }
    let mut spans = Vec::new();
    for (i, tab) in model.tabs.iter().enumerate() {
        let style = if i == model.active_tab {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        spans.push(Span::styled(format!(" {} ", tab.label()), style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

// --- Dashboard -------------------------------------------------------------

fn dashboard(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Dashboard", true);
    let th = theme();
    let mut lines: Vec<Line> = Vec::new();
    let Some(d) = &model.dashboard else {
        lines.push(dim("Assembling dashboard… (press 2 to refresh)"));
        render_scroll(f, area, block, lines, model.dash_scroll);
        return;
    };

    lines.push(Line::from(Span::styled(
        format!(
            "Gamification: {}",
            if d.gamification_enabled { "on" } else { "off" }
        ),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    lines.push(section("Summary"));
    let s = &d.summary;
    lines.push(kv("active changes", &s.active_changes.to_string()));
    lines.push(kv(
        "tasks",
        &format!(
            "{}/{} ({}%)",
            s.completed_tasks, s.total_tasks, s.task_percent
        ),
    ));
    lines.push(kv("specs touched", &s.specs_touching.to_string()));
    lines.push(kv("repos", &s.repo_count.to_string()));
    lines.push(kv("worktrees", &s.worktree_count.to_string()));
    lines.push(Line::from(""));

    lines.push(section(&format!("Ships today ({})", d.todays_ships.len())));
    if d.todays_ships.is_empty() {
        lines.push(dim("  none yet today"));
    } else {
        for ship in &d.todays_ships {
            let label = ship.title.clone().unwrap_or_else(|| ship.change_id.clone());
            lines.push(Line::from(format!("  {} {label}", th.glyph("✓", "*"))));
        }
    }
    lines.push(Line::from(""));

    if d.gamification_enabled {
        let active_days = d.progress.heatmap.iter().filter(|c| c.count > 0).count();
        lines.push(section(&format!(
            "Activity · {} days · {} active",
            d.progress.heatmap.len(),
            active_days
        )));
        lines.extend(heatmap_lines(
            &d.progress.heatmap,
            area.width.saturating_sub(4),
        ));
        lines.push(dim(&format!(
            "  streak: {} (best {})",
            d.progress.streak.current, d.progress.streak.longest
        )));
        lines.push(Line::from(""));

        lines.extend(leaderboard_lines(
            "Leaderboard · last year",
            &d.leaderboard,
            th,
        ));
        lines.extend(leaderboard_lines(
            "Leaderboard · this season",
            &d.season_leaderboard,
            th,
        ));

        if let Some(st) = &d.season {
            lines.push(section("Season"));
            lines.push(Line::from(format!(
                "  {} · {}",
                st.season.name, st.ladder.label
            )));
            lines.push(dim("  (press 3 for the full season ladder)"));
        }
    } else {
        lines.push(dim(
            "Enable gamification in SpecForge for streaks, heatmap, leaderboards, and seasons.",
        ));
    }

    render_scroll(f, area, block, lines, model.dash_scroll);
}

fn kv(key: &str, val: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key}: "), Style::default().fg(ACCENT)),
        Span::raw(val.to_string()),
    ])
}

/// A GitHub-style contribution grid: 7 day-rows × week-columns, intensity
/// relative to the busiest day, cropped to the most recent weeks that fit.
fn heatmap_lines(cells: &[HeatmapCell], width: u16) -> Vec<Line<'static>> {
    if cells.is_empty() {
        return vec![dim("  (no activity recorded)")];
    }
    let th = theme();
    let max = cells.iter().map(|c| c.count).max().unwrap_or(1).max(1) as u64;
    let total_weeks = cells.len().div_ceil(7);
    let fit = (width as usize).max(1);
    let start = total_weeks.saturating_sub(fit);
    let glyphs: [&str; 5] = if th.emoji {
        ["·", "░", "▒", "▓", "█"]
    } else {
        [" ", ".", ":", "+", "#"]
    };
    let mut rows: Vec<Vec<Span<'static>>> = (0..7).map(|_| Vec::new()).collect();
    for w in start..total_weeks {
        for (r, row) in rows.iter_mut().enumerate() {
            let idx = w * 7 + r;
            if idx < cells.len() {
                let c = cells[idx].count as u64;
                let lvl = if c == 0 {
                    0
                } else {
                    (c * 4).div_ceil(max).min(4) as usize
                };
                let mut style = Style::default().fg(if lvl == 0 {
                    Color::DarkGray
                } else {
                    Color::Cyan
                });
                if lvl >= 3 {
                    style = style.add_modifier(Modifier::BOLD);
                }
                row.push(Span::styled(glyphs[lvl].to_string(), style));
            } else {
                row.push(Span::raw(" "));
            }
        }
    }
    rows.into_iter()
        .map(|spans| {
            let mut v = vec![Span::raw("  ")];
            v.extend(spans);
            Line::from(v)
        })
        .collect()
}

/// A ranked author list; rendered only for a genuine multi-author contest
/// (matching the desktop's `entries.length <= 1` guard).
fn leaderboard_lines(
    title: &str,
    entries: &[LeaderboardEntry],
    th: &theme::Theme,
) -> Vec<Line<'static>> {
    if entries.len() <= 1 {
        return Vec::new();
    }
    let mut out = vec![section(title)];
    for (i, e) in entries.iter().enumerate() {
        let mut spans = vec![Span::styled(
            format!("  {:>2}. ", i + 1),
            Style::default().add_modifier(Modifier::DIM),
        )];
        let name_style = if e.is_me {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(e.display.clone(), name_style));
        if e.is_me {
            spans.push(Span::styled(" (you)", Style::default().fg(ACCENT)));
        }
        spans.push(Span::styled(
            format!(
                "   {} {}  {} {}  {} {}",
                th.glyph("🏆", "ships"),
                e.ships,
                th.glyph("✔", "tasks"),
                e.tasks,
                th.glyph("⎇", "commits"),
                e.commits
            ),
            Style::default().add_modifier(Modifier::DIM),
        ));
        out.push(Line::from(spans));
    }
    out.push(Line::from(""));
    out
}

// --- Season ----------------------------------------------------------------

fn season(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Season", true);
    let th = theme();
    let inner_h = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();

    let Some(d) = &model.dashboard else {
        lines.push(dim("Assembling season… (press 3 to refresh)"));
        render_scroll(f, area, block, lines, model.dash_scroll);
        return;
    };
    let Some(st) = &d.season else {
        lines.push(dim("No active season standing."));
        lines.push(dim(
            "Enable gamification in SpecForge settings, then press 3.",
        ));
        render_scroll(f, area, block, lines, model.dash_scroll);
        return;
    };

    lines.push(section(&format!(
        "Season {} · {}",
        st.season.number, st.season.name
    )));
    lines.push(Line::from(Span::styled(
        format!("  {}", st.ladder.label),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));
    lines.push(dim(&format!(
        "  Career: {} · {} shipped",
        st.career.label, st.career.ships
    )));
    if !st.ladder.overflow {
        lines.push(dim(&format!(
            "  {} pts → tier {}",
            st.ladder.gap_to_next,
            st.ladder.tier + 1
        )));
    } else {
        lines.push(dim(&format!(
            "  Master ∞+{} — all tiers cleared",
            st.ladder.tier - openspec_core::seasons::TIER_COUNT
        )));
    }
    lines.push(Line::from(""));
    let header_len = lines.len();

    let tiers = openspec_core::seasons::TIER_COUNT;
    for i in 1..=tiers {
        let unlocked = i <= st.ladder.tier;
        let current = i == st.ladder.tier || (st.ladder.overflow && i == tiers);
        let t = openspec_core::treatment(st.season.index, i);
        let threshold = i.saturating_mul(st.ladder.per_tier);
        let glyph = if current {
            th.glyph("▸ ", "> ")
        } else if unlocked {
            th.glyph("● ", "* ")
        } else {
            th.glyph("○ ", "- ")
        };
        let glyph_color = if unlocked {
            Color::Green
        } else {
            Color::DarkGray
        };
        let mut spans = vec![
            Span::styled(
                format!("{i:>2} "),
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(glyph.to_string(), Style::default().fg(glyph_color)),
            Span::styled(
                format!("{threshold:>6} pts  "),
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!("{} · {}", t.effect, rarity_word(t.rarity)),
                rarity_style(th, t.rarity, unlocked),
            ),
        ];
        let equipped = d
            .equipped
            .as_ref()
            .is_some_and(|e| e.season_index == st.season.index && e.tier_index == i);
        if equipped {
            spans.push(Span::styled(
                format!("  {} equipped", th.glyph("★", "*")),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if current && !st.ladder.overflow {
            spans.push(Span::styled(
                format!("   {} pts → next", st.ladder.gap_to_next),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
        let mut line = Line::from(spans);
        if current {
            line = line.style(Style::default().add_modifier(Modifier::REVERSED));
        }
        lines.push(line);
    }

    lines.push(Line::from(""));
    let unlocked_count = st.ladder.tier.min(tiers);
    lines.push(section(&format!(
        "Treatments ({unlocked_count}/{tiers} unlocked)"
    )));
    match &d.equipped {
        Some(e) => lines.push(Line::from(vec![
            Span::raw("  Equipped: "),
            Span::styled(
                format!("{} · {}", e.effect, rarity_word(e.rarity)),
                rarity_style(th, e.rarity, true),
            ),
        ])),
        None => lines.push(dim("  Equipped: none")),
    }

    // Auto-centre the current tier; `season_scroll` is a signed nudge that can
    // walk above the centre (up to the top) or below it (down to the footer).
    let cur_row = header_len + st.ladder.tier.min(tiers).saturating_sub(1) as usize;
    let auto = cur_row.saturating_sub(inner_h / 2) as i64;
    let max_off = lines.len().saturating_sub(inner_h) as i64;
    let offset = (auto + model.season_scroll as i64).clamp(0, max_off) as u16;
    // No wrap: tier rows are single-line, so the logical line count behind
    // `max_off` matches the rendered height and every row stays reachable.
    f.render_widget(Paragraph::new(lines).block(block).scroll((offset, 0)), area);
}

fn rarity_word(r: Rarity) -> &'static str {
    match r {
        Rarity::Common => "common",
        Rarity::Rare => "rare",
        Rarity::Epic => "epic",
        Rarity::Legendary => "legendary",
    }
}

fn rarity_style(th: &theme::Theme, r: Rarity, unlocked: bool) -> Style {
    let mut s = Style::default().fg(th.rarity(r));
    if matches!(r, Rarity::Epic | Rarity::Legendary) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if !unlocked {
        s = s.add_modifier(Modifier::DIM);
    }
    s
}

// --- Garden ----------------------------------------------------------------

fn garden(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Today's commits", true);
    let th = theme();
    let mut lines: Vec<Line> = Vec::new();
    let Some(plots) = &model.garden else {
        lines.push(dim("Loading commit garden… (press 4 to refresh)"));
        render_scroll(f, area, block, lines, model.garden_scroll);
        return;
    };

    let active: Vec<&WorkspaceGarden> = plots
        .iter()
        .filter(|p| !p.dormant && !p.commits.is_empty())
        .collect();
    if active.is_empty() {
        lines.push(dim("No commits yet today."));
        lines.push(dim(
            "Enable gamification in SpecForge settings, then press 4.",
        ));
        render_scroll(f, area, block, lines, model.garden_scroll);
        return;
    }

    for p in active {
        let people: HashSet<&str> = p.commits.iter().map(|c| c.person_key.as_str()).collect();
        let plural = if p.commits.len() == 1 { "" } else { "s" };
        let suffix = if people.len() > 1 {
            format!("  ·  {} people", people.len())
        } else {
            String::new()
        };
        lines.push(section(&format!(
            "{}  ·  {} commit{plural}{suffix}",
            p.label,
            p.commits.len()
        )));
        for c in &p.commits {
            lines.push(garden_row(p, c, th));
        }
        lines.push(Line::from(""));
    }

    render_scroll(f, area, block, lines, model.garden_scroll);
}

/// One garden commit row: a person-coloured node in a lane gutter (diagonals
/// approximated as verticals — a deliberate simplification of the desktop's
/// bezier rail) followed by ref chips and the subject.
fn garden_row(plot: &WorkspaceGarden, c: &GardenCommit, th: &theme::Theme) -> Line<'static> {
    const MAX_LANES: usize = 10;
    let width = plot.lane_count.min(MAX_LANES);
    let mut cells = vec![' '; width];
    let mut colors = vec![Color::Reset; width];
    for e in &plot.edges {
        if e.band == c.row || e.band + 1 == c.row {
            for col in [e.from_column, e.to_column] {
                if col < width && cells[col] == ' ' {
                    cells[col] = '│';
                    colors[col] = Color::DarkGray;
                }
            }
        }
    }
    if c.column < width {
        cells[c.column] = '●';
        colors[c.column] = th.person(&c.person_key, c.is_me, ACCENT);
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
                .fg(graph::ref_color(&rf.kind))
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::raw(c.subject.clone()));
    Line::from(spans)
}

// --- History ---------------------------------------------------------------

fn history(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("History", true);
    let inner_h = area.height.saturating_sub(2) as usize;
    let lines = match &model.graph {
        None => vec![dim(
            "Select a change in Browse, then press 5 for its repository history.",
        )],
        Some(g) => graph::commit_rail(g, model.graph_selected),
    };
    let total = lines.len();
    let offset = model
        .graph_selected
        .saturating_sub(inner_h.saturating_sub(1))
        .min(total.saturating_sub(inner_h.max(1))) as u16;
    f.render_widget(Paragraph::new(lines).block(block).scroll((offset, 0)), area);
}

// --- shared helpers --------------------------------------------------------

fn render_scroll(f: &mut Frame, area: Rect, block: Block<'static>, lines: Vec<Line>, scroll: u16) {
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
}

fn dim(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_string(),
        Style::default().add_modifier(Modifier::DIM),
    ))
}

fn pane_block(title: &str, focused: bool) -> Block<'static> {
    let border = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(format!(" {title} "))
}

fn help_overlay(f: &mut Frame) {
    let area = centered_rect(64, 70, f.area());
    let text = vec![
        Line::from(Span::styled(
            "SpecForge TUI — keys",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  1 / 2 / 3    Browse / Dashboard / Season"),
        Line::from("  4 / 5        Garden / History"),
        Line::from("  Esc          back to Browse (or clear search / close help)"),
        Line::from("  Tab          switch tree ⇄ detail (Browse)"),
        Line::from("  j / k        move / scroll"),
        Line::from("  Enter / l    open the selected change"),
        Line::from("  [ / ]        previous / next artifact tab"),
        Line::from("  /            filter the tree (Enter applies, Esc clears)"),
        Line::from("  h            back to the tree"),
        Line::from("  m            load more history (History screen)"),
        Line::from("  ?            toggle this help"),
        Line::from("  q / Ctrl-c   quit"),
    ];
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title(" Help "),
        ),
        area,
    );
}

fn centered_rect(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vert[1])[1]
}
