//! Render OpenSpec artifact markdown into styled terminal lines.
//!
//! Intentionally modest: headings, paragraphs, list items, emphasis, inline
//! code, and fenced code blocks. Tables degrade to their cell text and images
//! to their alt text — nothing is dropped silently. This is the one rendering
//! subsystem with no equivalent in the headless core (the desktop frontend
//! renders markdown in TypeScript).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::theme;

/// Convert markdown source into owned, styled lines for a `Paragraph`.
pub fn render(src: &str) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style = Style::default();
    let mut in_code_block = false;
    let mut list_depth: usize = 0;
    // Strikethrough/dim applied to the body of a completed `- [x]` task line.
    let mut task_mods = Modifier::empty();

    fn flush(spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
        if !spans.is_empty() {
            lines.push(Line::from(std::mem::take(spans)));
        }
    }

    for ev in Parser::new_ext(src, Options::ENABLE_TASKLISTS) {
        match ev {
            Event::Start(Tag::Heading { .. }) => {
                flush(&mut spans, &mut lines);
                if !lines.is_empty() {
                    lines.push(Line::from(""));
                }
                style = Style::default().add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(&mut spans, &mut lines);
                lines.push(Line::from(""));
                style = Style::default();
            }
            Event::End(TagEnd::Paragraph) => {
                flush(&mut spans, &mut lines);
                lines.push(Line::from(""));
            }
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush(&mut spans, &mut lines);
            }
            Event::Start(Tag::Item) => {
                flush(&mut spans, &mut lines);
                // Reset task styling so a nested or sibling item doesn't inherit
                // a completed parent's strikethrough until its own marker sets it.
                task_mods = Modifier::empty();
                let indent = "  ".repeat(list_depth.saturating_sub(1));
                spans.push(Span::raw(format!("{indent}• ")));
            }
            // A `- [ ]` / `- [x]` checkbox; replaces the bullet just pushed,
            // keeping the depth indent so nested task items stay nested.
            Event::TaskListMarker(checked) => {
                let glyph = if checked {
                    th.glyph("☑ ", "[x] ")
                } else {
                    th.glyph("☐ ", "[ ] ")
                };
                let style = if checked {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                };
                if let Some(last) = spans.last_mut() {
                    let indent = "  ".repeat(list_depth.saturating_sub(1));
                    *last = Span::styled(format!("{indent}{glyph}"), style);
                }
                task_mods = if checked {
                    Modifier::DIM | Modifier::CROSSED_OUT
                } else {
                    Modifier::empty()
                };
            }
            Event::End(TagEnd::Item) => {
                flush(&mut spans, &mut lines);
                task_mods = Modifier::empty();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut spans, &mut lines);
                in_code_block = true;
                style = Style::default().add_modifier(Modifier::DIM);
            }
            Event::End(TagEnd::CodeBlock) => {
                flush(&mut spans, &mut lines);
                in_code_block = false;
                style = Style::default();
            }
            Event::Start(Tag::Emphasis) => style = style.add_modifier(Modifier::ITALIC),
            Event::End(TagEnd::Emphasis) => style = style.remove_modifier(Modifier::ITALIC),
            Event::Start(Tag::Strong) => style = style.add_modifier(Modifier::BOLD),
            Event::End(TagEnd::Strong) => style = style.remove_modifier(Modifier::BOLD),
            Event::Text(t) => {
                let style = style.add_modifier(task_mods);
                if in_code_block {
                    for (i, part) in t.split('\n').enumerate() {
                        if i > 0 {
                            flush(&mut spans, &mut lines);
                        }
                        spans.push(Span::styled(part.to_string(), style));
                    }
                } else {
                    spans.push(Span::styled(t.to_string(), style));
                }
            }
            Event::Code(c) => spans.push(Span::styled(
                format!("`{c}`"),
                Style::default().add_modifier(Modifier::DIM),
            )),
            Event::SoftBreak => spans.push(Span::raw(" ")),
            Event::HardBreak => flush(&mut spans, &mut lines),
            Event::Start(Tag::Image { dest_url, .. }) => {
                spans.push(Span::raw(format!("[image: {dest_url}]")));
            }
            _ => {}
        }
    }
    flush(&mut spans, &mut lines);
    lines
}
