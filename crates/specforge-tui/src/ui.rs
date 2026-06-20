//! `view`: a pure function of `Model`. Immediate-mode rendering with ratatui;
//! the terminal diffs frames, so redraw-on-event stays cheap.
//!
//! Five screens — Browse, Dashboard, Season, Garden, History — share a title
//! bar and key bar. The gamified screens render the headless core's typed
//! payloads directly (no JSON scraping), and the season ladder reconstructs all
//! thirty tiers by calling `openspec_core::treatment` per tier.

use std::collections::HashSet;

use openspec_app::{QuotaStatus, QuotaWindow};
use openspec_core::{GardenCommit, HeatmapCell, LeaderboardEntry, Rarity, WorkspaceGarden};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    settings_row_at, Focus, Model, Overlay, Screen, SettingsRow, SettingsWorkspace, TreeRow,
    SETTINGS_TOGGLE_COUNT,
};
use crate::theme::{self, theme, Slot};
use crate::{graph, markdown};

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
        Screen::Settings => settings(f, chunks[1], model),
    }
    key_bar(f, chunks[2], model);

    if let Some(overlay) = &model.overlay {
        overlay_view(f, overlay);
    }
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
        Screen::Settings => "Settings",
    };
    let line = Line::from(vec![
        Span::styled(
            " SpecForge ",
            Style::default()
                .fg(theme().slot(Slot::OnAccent))
                .bg(theme().accent())
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

    // The opt-in quota gauge sits flush-right; the dim status truncates first if
    // the row is tight. Only split when there's room for both — otherwise the
    // (ambient) gauge yields to the screen title.
    if let Some(spans) = quota_gauge(model) {
        let gauge_w: u16 = spans.iter().map(|s| s.content.chars().count() as u16).sum();
        if area.width >= gauge_w + 16 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(gauge_w)])
                .split(area);
            f.render_widget(Paragraph::new(line), cols[0]);
            f.render_widget(
                Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
                cols[1],
            );
            return;
        }
    }
    f.render_widget(Paragraph::new(line), area);
}

/// Segment cells per window in the title-bar gauge: the 5-hour window is one
/// cell per hour, the weekly window one cell per day. Each cell is a time
/// segment — the fill shows utilization across them and the "now" marker
/// underlines the segment the clock currently sits in.
const FIVE_HOUR_CELLS: usize = 5;
const SEVEN_DAY_CELLS: usize = 7;
/// The windows' fixed durations, used to place the "now" marker from a reset
/// instant (the field names *are* the lengths: `five_hour` / `seven_day`).
const FIVE_HOUR_SECS: u64 = 5 * 3600;
const SEVEN_DAY_SECS: u64 = 7 * 24 * 3600;

/// The title-bar quota gauge spans, or `None` when there's nothing to show
/// (feature disabled). Leads with two spaces to separate from the status text.
fn quota_gauge(model: &Model) -> Option<Vec<Span<'static>>> {
    let q = &model.quota;
    match q.status {
        QuotaStatus::Disabled => None,
        QuotaStatus::Unauthenticated => Some(vec![Span::styled(
            "  Claude: sign in".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Unavailable => Some(vec![Span::styled(
            "  Claude: quota n/a".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Ok => {
            let mut spans: Vec<Span<'static>> = vec![Span::raw("  ".to_string())];
            let mut wrote = false;
            if let Some(w) = &q.five_hour {
                spans.extend(window_spans(
                    "5h",
                    w,
                    q.stale,
                    FIVE_HOUR_CELLS,
                    FIVE_HOUR_SECS,
                ));
                wrote = true;
            }
            if let Some(w) = &q.seven_day {
                if wrote {
                    spans.push(Span::raw("  ".to_string()));
                }
                spans.extend(window_spans(
                    "wk",
                    w,
                    q.stale,
                    SEVEN_DAY_CELLS,
                    SEVEN_DAY_SECS,
                ));
                wrote = true;
            }
            wrote.then_some(spans)
        }
    }
}

/// `label ▓▓▓░░ NN%` for one window, coloured by threshold, with one cell per
/// time segment (hours / days). The fill shows utilization across the segments
/// and the segment the clock currently sits in is underlined as a live "now"
/// marker (absent — a plain bar — when the reset time is unknown). A spent
/// window (100%) shows a reset countdown in place of the percentage; a stale
/// snapshot is dimmed.
fn window_spans(
    label: &str,
    w: &QuotaWindow,
    stale: bool,
    cells: usize,
    length_secs: u64,
) -> Vec<Span<'static>> {
    let mut style = Style::default().fg(quota_color(w.utilization));
    if stale {
        style = style.add_modifier(Modifier::DIM);
    }
    let value = if w.utilization >= 100 {
        w.resets_at_unix
            .and_then(countdown)
            .unwrap_or_else(|| "full".to_string())
    } else {
        format!("{}%", w.utilization)
    };

    // The "now" marker: which segment cell the window's clock currently sits in,
    // or none when the reset time is unknown (then the bar is unsegmented).
    let marker = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
        .and_then(|now| elapsed_fraction(now, w.resets_at_unix, length_secs))
        .map(|frac| marker_cell(frac, cells));

    let th = theme();
    let fill = th.glyph("▓", "#");
    let empty = th.glyph("░", ".");
    let filled = quota_fill_cells(w.utilization, cells);

    let mut spans = vec![Span::styled(
        format!("{label} "),
        Style::default().add_modifier(Modifier::DIM),
    )];
    for i in 0..cells {
        let glyph = if i < filled { fill } else { empty };
        let cell_style = if marker == Some(i) {
            style.add_modifier(Modifier::UNDERLINED)
        } else {
            style
        };
        spans.push(Span::styled(glyph.to_string(), cell_style));
    }
    spans.push(Span::styled(format!(" {value}"), style));
    spans
}

/// Filled-cell count for `util` across a `cells`-wide bar: ceil so any non-zero
/// usage shows at least one cell, capped at full.
fn quota_fill_cells(util: u8, cells: usize) -> usize {
    (util as usize * cells).div_ceil(100).min(cells)
}

/// Fraction of a fixed-length window that has elapsed, in `0.0..=1.0`, from the
/// current time and the window's reset instant. `None` when the reset time is
/// unknown. Pure in `(now, reset)` so the marker placement is unit-testable.
fn elapsed_fraction(now: u64, reset_unix: Option<u64>, length_secs: u64) -> Option<f64> {
    let reset = reset_unix?;
    if length_secs == 0 {
        return Some(1.0);
    }
    // Clamp the remaining time to the window so a far-future reset reads as "just
    // started" and an already-past reset reads as "fully elapsed".
    let remaining = reset.saturating_sub(now).min(length_secs);
    let elapsed = length_secs - remaining;
    Some(elapsed as f64 / length_secs as f64)
}

/// The segment cell (`0..cells`) a `0.0..=1.0` elapsed fraction falls in, with a
/// full window pinned to the last cell rather than overflowing the bar.
fn marker_cell(frac: f64, cells: usize) -> usize {
    ((frac.clamp(0.0, 1.0) * cells as f64) as usize).min(cells.saturating_sub(1))
}

/// Severity by utilization: 0 = nominal (green), 1 = warn (orange, ≥70%),
/// 2 = critical (red, ≥90%).
fn quota_severity(util: u8) -> u8 {
    if util >= 90 {
        2
    } else if util >= 70 {
        1
    } else {
        0
    }
}

/// Green / orange / red by utilization, under the active scheme (and downsampled
/// to the terminal's depth).
fn quota_color(util: u8) -> Color {
    theme().quota(quota_severity(util))
}

/// `h:mm` until `reset_unix` (or `Nd` beyond 48h), computed live so the
/// countdown ticks between polls. `None` only if the system clock is unreadable.
fn countdown(reset_unix: u64) -> Option<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mins = reset_unix.saturating_sub(now) / 60;
    Some(if mins >= 48 * 60 {
        format!("{}d", mins / 1440)
    } else {
        format!("{}:{:02}", mins / 60, mins % 60)
    })
}

fn key_bar(f: &mut Frame, area: Rect, model: &Model) {
    let (hint, bg) = if model.filter_editing {
        let q = model.filter.clone().unwrap_or_default();
        (
            format!(" /{q}   (Enter apply · Esc cancel)"),
            theme().accent(),
        )
    } else if let Some(overlay) = &model.overlay {
        let h = match overlay {
            Overlay::Prompt { input, .. } => {
                format!(" {input}_   (Enter confirm · Esc cancel)")
            }
            Overlay::Confirm { .. } => " y confirm · n / Esc cancel".to_string(),
        };
        (h, theme().accent())
    } else {
        let h: String = match model.screen {
            Screen::Browse => match model.focus {
                Focus::Tree => {
                    " Tab pane · j/k move · Enter open · / search · 2-6 screens · ? help · q quit"
                        .to_string()
                }
                Focus::Detail => {
                    " [ ] tabs · j/k scroll · h tree · 2-6 screens · ? help · q quit".to_string()
                }
            },
            Screen::History => {
                " j/k move · m more · Esc back · 1-6 screens · ? help · q quit".to_string()
            }
            Screen::Settings => settings_footer(model),
            _ => " j/k scroll · Esc back · 1-6 screens · ? help · q quit".to_string(),
        };
        (h, theme().slot(Slot::TextDim))
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(theme().slot(Slot::OnAccent)).bg(bg),
        ))),
        area,
    );
}

// --- Browse ----------------------------------------------------------------

fn browse(f: &mut Frame, area: Rect, model: &Model) {
    if area.width >= TWO_PANE_MIN_WIDTH {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(tree_pane_width(area.width)),
                Constraint::Min(0),
            ])
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

/// Width allotted to the Browse tree pane in two-pane mode. Roughly a third of
/// the terminal width, but clamped: it stays wide enough to read change names on
/// small terminals and stops growing on wide ones so the surplus width goes to
/// the detail pane rather than an ever-wider tree.
pub(crate) fn tree_pane_width(total: u16) -> u16 {
    const MIN: u16 = 28;
    const MAX: u16 = 44;
    ((total as u32 * 32 / 100) as u16).clamp(MIN, MAX)
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
                .fg(theme().accent())
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
        Span::styled(format!("  {key}: "), Style::default().fg(theme().accent())),
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
                    theme().slot(Slot::TextDim)
                } else {
                    theme().accent()
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
            Style::default()
                .fg(theme().accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(e.display.clone(), name_style));
        if e.is_me {
            spans.push(Span::styled(
                " (you)",
                Style::default().fg(theme().accent()),
            ));
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
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD),
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
            theme().slot(Slot::Success)
        } else {
            theme().slot(Slot::TextDim)
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
                    .fg(theme().slot(Slot::Warn))
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
                    colors[col] = theme().slot(Slot::TextDim);
                }
            }
        }
    }
    if c.column < width {
        cells[c.column] = '●';
        colors[c.column] = th.person(&c.person_key, c.is_me, theme().accent());
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

// --- Settings --------------------------------------------------------------

/// The Settings screen: the two toggle rows, then a Workspaces section with an
/// add action and one row per user-registered workspace (name, path, colour
/// swatch, missing indicator). The focused row is marked and accented, and the
/// list scrolls to keep the cursor in view. The view reads only `Model`, so the
/// values come from the mirrors the key handler keeps current.
fn settings(f: &mut Frame, area: Rect, model: &Model) {
    let th = theme();
    let mut lines: Vec<Line> = vec![Line::from("")];
    // Track the rendered line index of the focused row so the list can scroll to
    // keep it visible (cursor indices don't map 1:1 to lines — blanks/headers).
    let mut focused_line = 0usize;

    let toggles = [
        ("Gamification", model.gamification_on),
        ("Claude quota gauge", model.quota_on),
    ];
    for (i, (label, on)) in toggles.iter().enumerate() {
        let focused = model.settings_selected == i;
        if focused {
            focused_line = lines.len();
        }
        lines.push(settings_toggle_line(label, *on, focused));
    }

    lines.push(Line::from(""));
    lines.push(section("Appearance"));
    let appearance_idx = SETTINGS_TOGGLE_COUNT; // after the two toggles
    let focused = model.settings_selected == appearance_idx;
    if focused {
        focused_line = lines.len();
    }
    lines.push(settings_scheme_line(theme().active_scheme(), focused));

    lines.push(Line::from(""));
    lines.push(section("Workspaces"));

    let add_idx = SETTINGS_TOGGLE_COUNT + 1; // after the toggles and the Appearance row
    let focused = model.settings_selected == add_idx;
    if focused {
        focused_line = lines.len();
    }
    let add_style = if focused {
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme().accent())
    };
    let add_marker = if focused { " ▸ " } else { "   " };
    lines.push(Line::from(Span::styled(
        format!("{add_marker}+ Add workspace"),
        add_style,
    )));

    if model.settings_workspaces.is_empty() {
        lines.push(dim(
            "     No workspaces registered yet — press a to add one.",
        ));
    } else {
        for (i, ws) in model.settings_workspaces.iter().enumerate() {
            let focused = model.settings_selected == add_idx + 1 + i;
            if focused {
                focused_line = lines.len();
            }
            lines.push(settings_workspace_line(th, ws, focused));
        }
    }

    lines.push(Line::from(""));
    lines.push(dim(
        "   Writes only SpecForge's config (registry + presentation) — never workspace files.",
    ));

    let inner_h = area.height.saturating_sub(2) as usize;
    let max_off = lines.len().saturating_sub(inner_h);
    let scroll = focused_line
        .saturating_sub(inner_h.saturating_sub(1))
        .min(max_off) as u16;
    let block = pane_block("Settings", true);
    render_scroll(f, area, block, lines, scroll);
}

/// One toggle row: ` ▸ Label                 [ on ]`.
fn settings_toggle_line(label: &str, on: bool, focused: bool) -> Line<'static> {
    let marker = if focused { " ▸ " } else { "   " };
    let state = if on { "[ on ]" } else { "[ off ]" };
    let style = if focused {
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(Span::styled(format!("{marker}{label:<22}{state}"), style))
}

/// The colour-scheme row: ` ▸ Colour scheme        Nord`. Space/Enter/→ cycles.
fn settings_scheme_line(scheme: theme::Scheme, focused: bool) -> Line<'static> {
    let marker = if focused { " ▸ " } else { "   " };
    let style = if focused {
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(Span::styled(
        format!("{marker}{:<22}{}", "Colour scheme", scheme.name()),
        style,
    ))
}

/// One workspace row: marker, colour swatch (or empty ring), name, dim path, and
/// a `(missing)` flag when the folder is gone.
fn settings_workspace_line(
    th: &theme::Theme,
    ws: &SettingsWorkspace,
    focused: bool,
) -> Line<'static> {
    let marker = if focused { " ▸ " } else { "   " };
    let marker_style = if focused {
        Style::default().fg(theme().accent())
    } else {
        Style::default()
    };
    let mut spans = vec![Span::styled(marker.to_string(), marker_style)];

    match ws.color {
        Some(c) => spans.push(Span::styled(
            th.glyph("● ", "* ").to_string(),
            Style::default().fg(th.palette_fg(c)),
        )),
        None => spans.push(Span::styled(
            th.glyph("○ ", "- ").to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )),
    }

    let name = ws.display_name.clone().unwrap_or_else(|| ws.name.clone());
    let name_style = if focused {
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    spans.push(Span::styled(name, name_style));
    spans.push(Span::styled(
        format!("  {}", ws.uri.display()),
        Style::default().add_modifier(Modifier::DIM),
    ));
    if ws.is_missing {
        spans.push(Span::styled(
            "  (missing)".to_string(),
            Style::default()
                .fg(theme().slot(Slot::Error))
                .add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

/// Context-sensitive footer for the Settings screen, keyed to the focused row.
fn settings_footer(model: &Model) -> String {
    match settings_row_at(model.settings_selected) {
        SettingsRow::Toggle => {
            " j/k move · Space toggle · a add · Esc back · 1-6 · ? · q".to_string()
        }
        SettingsRow::Appearance => {
            " j/k move · Space/→ cycle scheme · a add · Esc back · 1-6 · ? · q".to_string()
        }
        SettingsRow::AddWorkspace => " j/k move · Enter add · Esc back · 1-6 · ? · q".to_string(),
        SettingsRow::Workspace(_) => {
            " j/k move · x remove · r rename · c colour · a add · Esc back".to_string()
        }
    }
}

/// The centred Settings modal: an add/rename text prompt (with inline error) or
/// a remove confirm.
fn overlay_view(f: &mut Frame, overlay: &Overlay) {
    let (title, lines) = match overlay {
        Overlay::Prompt {
            title,
            input,
            error,
            ..
        } => {
            let mut v = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!(" {input}_"),
                    Style::default().fg(theme().accent()),
                )),
                Line::from(""),
            ];
            if let Some(e) = error {
                v.push(Line::from(Span::styled(
                    format!(" ✗ {e}"),
                    Style::default().fg(theme().slot(Slot::Error)),
                )));
            }
            v.push(dim(" Enter confirm · Esc cancel"));
            (title.clone(), v)
        }
        Overlay::Confirm { title, message, .. } => {
            let v = vec![
                Line::from(""),
                Line::from(format!(" {message}")),
                Line::from(""),
                dim(" y confirm · n / Esc cancel"),
            ];
            (title.clone(), v)
        }
    };
    let area = centered_rect(72, 34, f.area());
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme().accent()))
                .title(format!(" {title} ")),
        ),
        area,
    );
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
        Style::default()
            .fg(theme().accent())
            .add_modifier(Modifier::BOLD),
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
        Style::default().fg(theme().accent())
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
        Line::from("  4 / 5 / 6    Garden / History / Settings"),
        Line::from("  Esc          back to Browse (or clear search / close help)"),
        Line::from("  Tab          switch tree ⇄ detail (Browse)"),
        Line::from("  j / k        move / scroll"),
        Line::from("  Enter / l    open the selected change"),
        Line::from("  Space        toggle the focused setting (Settings)"),
        Line::from("  a / x        add / remove a workspace (Settings)"),
        Line::from("  r / c        rename / recolour a workspace (Settings)"),
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
                .border_style(Style::default().fg(theme().accent()))
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

#[cfg(test)]
mod quota_tests {
    use super::{
        elapsed_fraction, marker_cell, quota_fill_cells, quota_severity, FIVE_HOUR_CELLS,
        FIVE_HOUR_SECS, SEVEN_DAY_CELLS,
    };

    #[test]
    fn bar_fill_is_ceil_and_capped() {
        // 5-cell (5-hour) bar.
        assert_eq!(quota_fill_cells(0, FIVE_HOUR_CELLS), 0);
        assert_eq!(quota_fill_cells(1, FIVE_HOUR_CELLS), 1); // any usage shows a cell
        assert_eq!(quota_fill_cells(20, FIVE_HOUR_CELLS), 1);
        assert_eq!(quota_fill_cells(21, FIVE_HOUR_CELLS), 2); // ceil past the boundary
        assert_eq!(quota_fill_cells(80, FIVE_HOUR_CELLS), 4);
        assert_eq!(quota_fill_cells(100, FIVE_HOUR_CELLS), 5);
        assert_eq!(quota_fill_cells(255, FIVE_HOUR_CELLS), 5); // capped — never overflows
                                                               // 7-cell (weekly) bar.
        assert_eq!(quota_fill_cells(0, SEVEN_DAY_CELLS), 0);
        assert_eq!(quota_fill_cells(1, SEVEN_DAY_CELLS), 1);
        assert_eq!(quota_fill_cells(50, SEVEN_DAY_CELLS), 4); // 3.5 → ceil 4
        assert_eq!(quota_fill_cells(100, SEVEN_DAY_CELLS), 7);
    }

    #[test]
    fn severity_thresholds_at_70_and_90() {
        assert_eq!(quota_severity(0), 0);
        assert_eq!(quota_severity(69), 0);
        assert_eq!(quota_severity(70), 1); // orange at exactly 70%
        assert_eq!(quota_severity(89), 1);
        assert_eq!(quota_severity(90), 2); // red at exactly 90%
        assert_eq!(quota_severity(100), 2);
    }

    #[test]
    fn elapsed_fraction_clamps_to_the_window() {
        let now = 1_000_000;
        // No reset time → no fraction (→ no marker).
        assert_eq!(elapsed_fraction(now, None, FIVE_HOUR_SECS), None);
        // Reset further out than a window length → just started (0.0).
        assert_eq!(
            elapsed_fraction(now, Some(now + FIVE_HOUR_SECS * 2), FIVE_HOUR_SECS),
            Some(0.0)
        );
        // Reset exactly one length out → start of the window (0.0).
        assert_eq!(
            elapsed_fraction(now, Some(now + FIVE_HOUR_SECS), FIVE_HOUR_SECS),
            Some(0.0)
        );
        // Halfway through → 0.5.
        assert_eq!(
            elapsed_fraction(now, Some(now + FIVE_HOUR_SECS / 2), FIVE_HOUR_SECS),
            Some(0.5)
        );
        // Reset already in the past → fully elapsed (1.0).
        assert_eq!(
            elapsed_fraction(now, Some(now - 10), FIVE_HOUR_SECS),
            Some(1.0)
        );
    }

    #[test]
    fn marker_cell_maps_fraction_to_segment() {
        // 5-hour, 5 cells.
        assert_eq!(marker_cell(0.0, FIVE_HOUR_CELLS), 0);
        assert_eq!(marker_cell(0.5, FIVE_HOUR_CELLS), 2); // mid-window → hour 3 (index 2)
        assert_eq!(marker_cell(1.0, FIVE_HOUR_CELLS), 4); // full → last cell, no overflow
                                                          // Weekly, 7 cells.
        assert_eq!(marker_cell(0.0, SEVEN_DAY_CELLS), 0);
        assert_eq!(marker_cell(0.5, SEVEN_DAY_CELLS), 3); // mid-week → index 3
        assert_eq!(marker_cell(1.0, SEVEN_DAY_CELLS), 6);
    }
}
