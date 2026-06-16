//! `view`: a pure function of `Model`. Immediate-mode rendering with ratatui;
//! the terminal diffs frames, so redraw-on-event stays cheap.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;
use serde_json::Value;

use crate::app::{Focus, Model, Screen, TreeRow};
use crate::markdown;

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
    let hints = match model.screen {
        Screen::Browse => "Tab pane · j/k move · Enter open · 2 dash · 3 season · ? help · q quit",
        _ => "j/k scroll · 1 browse · 2 dash · 3 season · Esc back · ? help · q quit",
    };
    let _ = model;
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {hints}"),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ))),
        area,
    );
}

fn browse(f: &mut Frame, area: Rect, model: &Model) {
    if area.width >= TWO_PANE_MIN_WIDTH {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);
        tree_pane(f, cols[0], model);
        detail_pane(f, cols[1], model);
    } else {
        // Single-pane fallback: show whichever pane has focus.
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

    let lines: Vec<Line> = model
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| row_line(r, i == model.selected))
        .collect();

    // Keep the selection in view with a simple scroll offset.
    let offset = model
        .selected
        .saturating_sub(inner_h.saturating_sub(1))
        .min(model.rows.len().saturating_sub(inner_h.max(1))) as u16;

    f.render_widget(Paragraph::new(lines).block(block).scroll((offset, 0)), area);
}

fn row_line(r: &TreeRow, selected: bool) -> Line<'static> {
    let indent = "  ".repeat(r.depth as usize);
    let mut spans = Vec::new();
    if r.is_header {
        spans.push(Span::styled(
            format!("▾ {}", r.label),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    } else {
        let glyph = match r.progress {
            Some((c, t)) if t > 0 && c >= t => "● ",
            Some((c, _)) if c > 0 => "◐ ",
            _ => "○ ",
        };
        let mut text = format!("{indent}{glyph}{}", r.label);
        if let Some((c, t)) = r.progress {
            text.push_str(&format!("  {}", progress_bar(c, t)));
        }
        spans.push(Span::raw(text));
    }
    let mut line = Line::from(spans);
    if selected {
        line = line.style(Style::default().add_modifier(Modifier::REVERSED));
    }
    line
}

/// A 7-cell progress bar plus the raw count, e.g. `▓▓▓▓▓░░ 5/7`.
fn progress_bar(completed: usize, total: usize) -> String {
    const WIDTH: usize = 7;
    if total == 0 {
        return "— 0/0".to_string();
    }
    let filled = (completed * WIDTH).div_ceil(total).min(WIDTH);
    let bar: String = "▓".repeat(filled) + &"░".repeat(WIDTH - filled);
    format!("{bar} {completed}/{total}")
}

fn detail_pane(f: &mut Frame, area: Rect, model: &Model) {
    let focused = matches!(model.focus, Focus::Detail);
    let title = if model.detail_title.is_empty() {
        "Detail".to_string()
    } else {
        format!("{} — proposal.md", model.detail_title)
    };
    let block = pane_block(&title, focused);
    let lines = markdown::render(&model.detail_md);
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((model.detail_scroll, 0)),
        area,
    );
}

fn dashboard(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Dashboard", true);
    let mut lines: Vec<Line> = Vec::new();
    match &model.dashboard {
        None => lines.push(dim("Assembling dashboard… (press 2 to refresh)")),
        Some(v) => {
            let gam = v
                .get("gamificationEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            lines.push(Line::from(Span::styled(
                format!("Gamification: {}", if gam { "on" } else { "off" }),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if let Some(summary) = v.get("summary") {
                lines.push(section("Summary"));
                lines.extend(kv_lines(summary));
                lines.push(Line::from(""));
            }
            if let Some(ships) = v.get("todaysShips").and_then(Value::as_array) {
                lines.push(section(&format!("Ships today ({})", ships.len())));
                for s in ships {
                    let label = s
                        .get("title")
                        .or_else(|| s.get("changeId"))
                        .or_else(|| s.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("(change)");
                    lines.push(Line::from(format!("  ✓ {label}")));
                }
                if ships.is_empty() {
                    lines.push(dim("  none yet today"));
                }
                lines.push(Line::from(""));
            }
            if let Some(season) = v.get("season") {
                if !season.is_null() {
                    lines.push(section("Season"));
                    lines.extend(kv_lines(season));
                    lines.push(dim("  (press 3 for the full season screen)"));
                }
            }
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((model.dash_scroll, 0)),
        area,
    );
}

fn season(f: &mut Frame, area: Rect, model: &Model) {
    let block = pane_block("Season", true);
    let mut lines: Vec<Line> = Vec::new();
    match model.dashboard.as_ref().and_then(|v| v.get("season")) {
        Some(season) if !season.is_null() => {
            lines.push(section("Standing"));
            lines.extend(kv_lines(season));
            if let Some(v) = &model.dashboard {
                if let Some(locker) = v.get("locker").and_then(Value::as_array) {
                    lines.push(Line::from(""));
                    lines.push(section(&format!("Unlocked treatments ({})", locker.len())));
                }
            }
        }
        _ => {
            lines.push(dim("No active season standing."));
            lines.push(dim(
                "Enable gamification in SpecForge settings, then press 2.",
            ));
        }
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((model.dash_scroll, 0)),
        area,
    );
}

/// Render a JSON object's scalar fields as `key: value` lines (one level deep).
fn kv_lines(v: &Value) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            let rendered = match val {
                Value::Null => continue,
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.clone(),
                Value::Array(a) => format!("[{} items]", a.len()),
                Value::Object(_) => "{…}".to_string(),
            };
            out.push(Line::from(vec![
                Span::styled(format!("  {k}: "), Style::default().fg(ACCENT)),
                Span::raw(rendered),
            ]));
        }
    } else {
        out.push(Line::from(format!("  {v}")));
    }
    out
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
    let area = centered_rect(60, 60, f.area());
    let text = vec![
        Line::from(Span::styled(
            "SpecForge TUI — keys",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  1 / 2 / 3    Browse / Dashboard / Season"),
        Line::from("  Esc          back to Browse (or close this)"),
        Line::from("  Tab          switch tree ⇄ detail (Browse)"),
        Line::from("  j / k        move / scroll"),
        Line::from("  Enter / l    open the selected change"),
        Line::from("  h            back to the tree"),
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
