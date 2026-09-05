//! `view`: a pure function of `Model`. Immediate-mode rendering with ratatui;
//! the terminal diffs frames, so redraw-on-event stays cheap.
//!
//! Four screens — Browse, Dashboard, Garden, History — share a title bar and
//! key bar. The progress screens render the headless core's typed payloads
//! directly (no JSON scraping).

use std::collections::HashSet;

use openspec_app::{QuotaStatus, QuotaWindow};
use openspec_core::{GardenCommit, HeatmapCell, WorkspaceGarden};
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

    // The opt-in quota gauge sits flush-right; the dim status truncates first
    // if the row is tight. Groups render in priority order (Claude, then
    // ChatGPT), and `fit_gauge_groups` drops whole trailing groups until the
    // remainder fits — enabling ChatGPT can never hide an otherwise-visible
    // Claude gauge (design.md, Decision 7). Only split when at least the
    // first group fits — otherwise the (ambient) gauge yields entirely to the
    // screen title.
    let groups: Vec<Vec<Span<'static>>> = [claude_gauge_group(model), chatgpt_gauge_group(model)]
        .into_iter()
        .flatten()
        .collect();
    if let Some(spans) = fit_gauge_groups(&groups, area.width) {
        let gauge_w = spans_width(&spans);
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
/// Fallback window lengths (seconds) for the ChatGPT gauge, used only when the
/// usage response omits `limit_window_seconds` — mirrors the desktop/web
/// `ChatGptQuotaPill`'s identical fallback.
const FALLBACK_PRIMARY_SECS: u64 = 5 * 3600;
const FALLBACK_SECONDARY_SECS: u64 = 7 * 24 * 3600;

/// One provider's gauge-group spans, or `None` when there's nothing to show
/// for it (feature disabled, or an `Ok` snapshot with no windows at all).
/// Carries no leading pad — [`join_groups`] adds the gap between groups (and
/// before the first one, separating the gauge from the status text).
fn claude_gauge_group(model: &Model) -> Option<Vec<Span<'static>>> {
    let q = &model.quota;
    match q.status {
        QuotaStatus::Disabled => None,
        QuotaStatus::Unauthenticated => Some(vec![Span::styled(
            "Claude: sign in".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Unavailable => Some(vec![Span::styled(
            "Claude: quota n/a".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Ok => {
            let mut spans: Vec<Span<'static>> = Vec::new();
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
            // Per-model scoped weekly windows (e.g. Fable), labeled by model.
            // Same weekly grammar as `wk`; appended last so they are the first
            // content clipped when the flush-right gauge runs out of room.
            for scoped in &q.scoped {
                if wrote {
                    spans.push(Span::raw("  ".to_string()));
                }
                let win = QuotaWindow {
                    utilization: scoped.utilization,
                    resets_at_unix: scoped.resets_at_unix,
                };
                spans.extend(window_spans(
                    &scoped.model,
                    &win,
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

/// ChatGPT's gauge-group spans — a structural twin of [`claude_gauge_group`].
/// Each window's segment count and axis length derive from the
/// server-reported `window_secs` (hours up to 24h, else days) via
/// [`chatgpt_window_axis`], falling back to a fixed 5h primary / 7d secondary
/// only when the response omits it — unlike Claude's permanently-fixed
/// windows (design.md, Decision 5).
fn chatgpt_gauge_group(model: &Model) -> Option<Vec<Span<'static>>> {
    let q = &model.chatgpt_quota;
    match q.status {
        QuotaStatus::Disabled => None,
        QuotaStatus::Unauthenticated => Some(vec![Span::styled(
            "ChatGPT: sign in".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Unavailable => Some(vec![Span::styled(
            "ChatGPT: quota n/a".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )]),
        QuotaStatus::Ok => {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut wrote = false;
            if let Some(w) = &q.primary {
                let (label, cells, secs) =
                    chatgpt_window_axis(w.window_secs, FALLBACK_PRIMARY_SECS);
                let win = QuotaWindow {
                    utilization: w.utilization,
                    resets_at_unix: w.resets_at_unix,
                };
                spans.extend(window_spans(&label, &win, q.stale, cells, secs));
                wrote = true;
            }
            if let Some(w) = &q.secondary {
                if wrote {
                    spans.push(Span::raw("  ".to_string()));
                }
                let (label, cells, secs) =
                    chatgpt_window_axis(w.window_secs, FALLBACK_SECONDARY_SECS);
                let win = QuotaWindow {
                    utilization: w.utilization,
                    resets_at_unix: w.resets_at_unix,
                };
                spans.extend(window_spans(&label, &win, q.stale, cells, secs));
                wrote = true;
            }
            wrote.then_some(spans)
        }
    }
}

/// Segment count, axis length, and a terse label ("5h", "7d", ...) for one
/// ChatGPT window: hours for a window length up to 24h, days beyond, rounded
/// to the nearest whole unit and floored at 1 so a very short/zero length
/// still renders a bar instead of dividing by zero. `window_secs` is `None`
/// when the response omitted `limit_window_seconds`. Mirrors the desktop/web
/// `ChatGptQuotaPill`'s `axisFor`.
fn chatgpt_window_axis(window_secs: Option<u64>, fallback_secs: u64) -> (String, usize, u64) {
    let secs = window_secs.unwrap_or(fallback_secs).max(1);
    if secs <= 24 * 3600 {
        let hours = ((secs as f64 / 3600.0).round() as usize).max(1);
        (chatgpt_window_label(secs, format!("{hours}h")), hours, secs)
    } else {
        let days = ((secs as f64 / 86400.0).round() as usize).max(1);
        (chatgpt_window_label(secs, format!("{days}d")), days, secs)
    }
}

/// The standard window lengths borrow the Claude gauge's vocabulary — `wk` for
/// a week, `5h` for five hours — so two provider rows in one title bar name the
/// same period the same way. Matched within a tolerance because the endpoint's
/// `limit_window_seconds` need not be exactly 604800 / 18000. Any other length
/// keeps the label derived from its duration.
fn chatgpt_window_label(secs: u64, derived: String) -> String {
    const WEEK: u64 = 7 * 86400;
    const FIVE_HOURS: u64 = 5 * 3600;
    if secs.abs_diff(WEEK) <= 3600 {
        "wk".to_string()
    } else if secs.abs_diff(FIVE_HOURS) <= 600 {
        "5h".to_string()
    } else {
        derived
    }
}

/// Total display width (terminal columns) of a span list.
fn spans_width(spans: &[Span<'static>]) -> u16 {
    spans.iter().map(|s| s.content.chars().count() as u16).sum()
}

/// Join provider groups with a two-space gap before each one — including the
/// first, separating the gauge from the status text, exactly as the
/// single-provider gauge always has.
fn join_groups(groups: &[Vec<Span<'static>>]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for g in groups {
        spans.push(Span::raw("  ".to_string()));
        spans.extend(g.iter().cloned());
    }
    spans
}

/// Drop whole trailing provider groups (lowest priority last) until the
/// combined width satisfies the gauge's width guard — unchanged from the
/// single-provider gauge's own `area_width >= width + 16` — preferring to
/// keep earlier groups. `groups` is priority order (Claude, then ChatGPT).
/// `None` when there are no groups to show, or even the first alone doesn't
/// fit.
fn fit_gauge_groups(groups: &[Vec<Span<'static>>], area_width: u16) -> Option<Vec<Span<'static>>> {
    for k in (1..=groups.len()).rev() {
        let joined = join_groups(&groups[..k]);
        if area_width >= spans_width(&joined) + 16 {
            return Some(joined);
        }
    }
    None
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
                    " Tab pane · j/k move · Enter open · / search · 2-5 screens · ? help · q quit"
                        .to_string()
                }
                Focus::Detail => {
                    " [ ] tabs · j/k scroll · h tree · 2-5 screens · ? help · q quit".to_string()
                }
            },
            Screen::History => {
                " j/k move · m more · Esc back · 1-5 screens · ? help · q quit".to_string()
            }
            Screen::Settings => settings_footer(model),
            _ => " j/k scroll · Esc back · 1-5 screens · ? help · q quit".to_string(),
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
        Paragraph::new(markdown::render(&model.detail_md, theme()))
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
    // The Dashboard is the unfiltered record while the Browse tree hides parked
    // rows (`dashboard`: *Dashboard Includes Disabled Workspaces*), so these
    // totals legitimately exceed what the tree reaches. Say so, in the same
    // words the desktop Dashboard uses, next to the totals being qualified.
    if model.disabled_row_count > 0 {
        let n = model.disabled_row_count;
        lines.push(dim(&format!(
            "  includes {n} disabled workspace{}",
            if n == 1 { "" } else { "s" }
        )));
    }
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

// --- Garden ----------------------------------------------------------------

fn garden(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Today's commits", true);
    let th = theme();
    let mut lines: Vec<Line> = Vec::new();
    let Some(plots) = &model.garden else {
        lines.push(dim("Loading commit garden… (press 3 to refresh)"));
        render_scroll(f, area, block, lines, model.garden_scroll);
        return;
    };

    let active: Vec<&WorkspaceGarden> = plots
        .iter()
        .filter(|p| !p.dormant && !p.commits.is_empty())
        .collect();
    if active.is_empty() {
        lines.push(dim("No commits yet today."));
        render_scroll(f, area, block, lines, model.garden_scroll);
        return;
    }

    for p in active {
        let authors: HashSet<&str> = p.commits.iter().map(|c| c.author_key.as_str()).collect();
        let plural = if p.commits.len() == 1 { "" } else { "s" };
        let suffix = if authors.len() > 1 {
            format!("  ·  {} authors", authors.len())
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

/// One garden commit row: an author-coloured node in a lane gutter (diagonals
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
        colors[c.column] = th.author(&c.author_key, c.is_me, theme().accent());
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
            "Select a change in Browse, then press 4 for its repository history.",
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
        ("Claude quota gauge", model.quota_on),
        ("ChatGPT quota gauge", model.chatgpt_quota_on),
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
    let appearance_idx = SETTINGS_TOGGLE_COUNT; // after the toggle rows
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
/// `(missing)` / `(disabled)` flags. A parked row keeps its place in this list —
/// it is the only surface from which it can be brought back — but its name is
/// dimmed to echo the Browse tree it has left.
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
    } else if ws.disabled {
        Style::default().add_modifier(Modifier::DIM)
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
    if ws.disabled {
        spans.push(Span::styled(
            "  (disabled)".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

/// Context-sensitive footer for the Settings screen, keyed to the focused row.
fn settings_footer(model: &Model) -> String {
    match settings_row_at(model.settings_selected) {
        SettingsRow::Toggle => {
            " j/k move · Space toggle · a add · Esc back · 1-5 · ? · q".to_string()
        }
        SettingsRow::Appearance => {
            " j/k move · Space/→ cycle scheme · a add · Esc back · 1-5 · ? · q".to_string()
        }
        SettingsRow::AddWorkspace => " j/k move · Enter add · Esc back · 1-5 · ? · q".to_string(),
        SettingsRow::Workspace(_) => {
            " j/k move · Space on/off · x remove · r rename · c colour · a add · Esc back"
                .to_string()
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
        Line::from("  1 / 2        Browse / Dashboard"),
        Line::from("  3 / 4 / 5    Garden / History / Settings"),
        Line::from("  Esc          back to Browse (or clear search / close help)"),
        Line::from("  Tab          switch tree ⇄ detail (Browse)"),
        Line::from("  j / k        move / scroll"),
        Line::from("  Enter / l    open the selected change"),
        Line::from("  Space        toggle a setting or a workspace (Settings)"),
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
        chatgpt_window_axis, elapsed_fraction, fit_gauge_groups, marker_cell, quota_fill_cells,
        quota_severity, window_spans, Span, FIVE_HOUR_CELLS, FIVE_HOUR_SECS, SEVEN_DAY_CELLS,
        SEVEN_DAY_SECS,
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
    fn scoped_window_spans_are_labeled_and_seven_cells() {
        use openspec_app::QuotaWindow;
        // A per-model scoped weekly window renders with the model name as its
        // label and one cell per day (7), like the pooled weekly gauge.
        let w = QuotaWindow {
            utilization: 59,
            resets_at_unix: None,
        };
        let spans = window_spans("Fable", &w, false, SEVEN_DAY_CELLS, SEVEN_DAY_SECS);
        // Layout: [label] + [one glyph per day cell] + [value].
        assert_eq!(spans.len(), 1 + SEVEN_DAY_CELLS + 1);
        assert_eq!(spans[0].content.as_ref(), "Fable ");
        assert!(
            spans.last().unwrap().content.contains("59%"),
            "value span shows the percent"
        );
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

    // ---- group-level degradation (`fit_gauge_groups`) ----
    //
    // Two synthetic 10-char-wide groups (contents 'A'* / 'B'* so the test can
    // tell which survived): each group costs 2 (leading pad) + 10 (content) =
    // 12 columns. Two groups joined = 24, so the two-group guard trips at
    // `area_width >= 24 + 16 = 40`; one group alone trips at
    // `area_width >= 12 + 16 = 28`.

    fn synthetic_groups() -> Vec<Vec<Span<'static>>> {
        vec![
            vec![Span::raw("A".repeat(10))],
            vec![Span::raw("B".repeat(10))],
        ]
    }

    #[test]
    fn both_groups_render_when_the_width_allows() {
        let spans = fit_gauge_groups(&synthetic_groups(), 40).expect("both groups should fit");
        let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains('A'), "Claude group renders");
        assert!(joined.contains('B'), "ChatGPT group renders too");
    }

    #[test]
    fn chatgpt_group_is_dropped_before_claude_when_only_one_fits() {
        // One column under the two-group threshold: only the first (Claude)
        // group's own, narrower threshold is satisfied.
        let spans =
            fit_gauge_groups(&synthetic_groups(), 39).expect("the Claude group alone should fit");
        let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains('A'), "Claude group still renders");
        assert!(
            !joined.contains('B'),
            "ChatGPT group is dropped, not Claude's"
        );
    }

    #[test]
    fn no_gauge_renders_when_not_even_one_group_fits() {
        // One column under the single-group threshold.
        assert!(fit_gauge_groups(&synthetic_groups(), 27).is_none());
    }

    #[test]
    fn standard_window_lengths_use_the_claude_labels() {
        // A week and five hours borrow `wk` / `5h` so the ChatGPT rows name the
        // same periods the Claude rows do, instead of reading `7d` / `5h`.
        let (label, segments, _) = chatgpt_window_axis(Some(7 * 86400), 1);
        assert_eq!(label, "wk");
        assert_eq!(segments, 7, "segment count still derives from the length");
        let (label, segments, _) = chatgpt_window_axis(Some(5 * 3600), 1);
        assert_eq!(label, "5h");
        assert_eq!(segments, 5);
    }

    #[test]
    fn near_standard_lengths_still_match_within_tolerance() {
        // `limit_window_seconds` need not be exactly 604800 / 18000.
        assert_eq!(chatgpt_window_axis(Some(7 * 86400 - 900), 1).0, "wk");
        assert_eq!(chatgpt_window_axis(Some(5 * 3600 + 300), 1).0, "5h");
    }

    #[test]
    fn non_standard_lengths_keep_the_derived_label() {
        // Well outside either tolerance: fall back to the duration-derived form.
        assert_eq!(chatgpt_window_axis(Some(3 * 3600), 1).0, "3h");
        assert_eq!(chatgpt_window_axis(Some(30 * 86400), 1).0, "30d");
    }

    #[test]
    fn a_missing_length_labels_from_the_fallback() {
        // No reported length → the caller's fallback drives both label and axis.
        assert_eq!(chatgpt_window_axis(None, 7 * 86400).0, "wk");
        assert_eq!(chatgpt_window_axis(None, 5 * 3600).0, "5h");
    }
}
