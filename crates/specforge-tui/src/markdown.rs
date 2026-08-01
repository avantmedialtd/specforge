//! Render OpenSpec artifact markdown into styled terminal lines.
//!
//! Intentionally modest: headings, paragraphs, list items, emphasis, inline
//! code, and fenced code blocks. Tables degrade to their cell text and images
//! to their alt text — nothing is dropped silently. This is the one rendering
//! subsystem with no equivalent in the headless core (the desktop frontend
//! renders markdown in TypeScript).
//!
//! Links render their destination inline (OSC 8 hyperlink or, on a terminal
//! not known to support it, a trailing `(destination)`) — this module never
//! spawns a process or otherwise "opens" anything; any opening is the
//! terminal emulator's own click-through behaviour on the emitted escape.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::{Slot, Theme};

/// Convert markdown source into owned, styled lines for a `Paragraph`. `th`
/// is injected (rather than read from the process-global `theme()`) so link
/// rendering — the one thing here with two capability-dependent outputs — is
/// unit-testable against a pinned `Theme` (see `theme::Theme::for_test`); the
/// caller passes the live `theme()` singleton.
pub fn render(src: &str, th: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style = Style::default();
    let mut in_code_block = false;
    let mut list_depth: usize = 0;
    // Strikethrough/dim applied to the body of a completed `- [x]` task line.
    let mut task_mods = Modifier::empty();
    // The link currently open: its destination and the index in `spans`
    // where its visible text begins, so `End(Link)` can wrap everything
    // pushed since with an OSC 8 escape (or, unsupported, append the
    // destination as trailing text) regardless of how many separate events
    // contributed to the link's inline content. `spans` only ever grows
    // between a link's Start and End (links can't cross paragraph/block
    // boundaries in CommonMark, so no `flush` intervenes), so `start` stays
    // a valid insertion point throughout.
    let mut pending_link: Option<(usize, String)> = None;

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
            // A link's destination is always discoverable — never swallowed
            // with an escape sequence the terminal doesn't render (the
            // *Artifact Markdown Rendering* requirement's textual fallback).
            // Underline marks the text as a link regardless of which path End
            // takes below; on `End` we remember where its visible text began
            // so the destination can be wrapped in or appended to it.
            Event::Start(Tag::Link { dest_url, .. }) => {
                pending_link = Some((spans.len(), dest_url.to_string()));
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            Event::End(TagEnd::Link) => {
                style = style.remove_modifier(Modifier::UNDERLINED);
                if let Some((start, dest)) = pending_link.take() {
                    if th.hyperlinks {
                        spans.insert(start, Span::raw(osc8_open(&dest)));
                        spans.push(Span::raw(OSC8_CLOSE));
                    } else {
                        spans.push(Span::styled(
                            format!(" ({dest})"),
                            Style::default().fg(th.slot(Slot::TextDim)),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut spans, &mut lines);
    lines
}

/// The OSC 8 terminal-hyperlink open escape (BEL-terminated — shorter than
/// `ESC \` and at least as widely supported in practice). Emitted only when
/// `Theme::hyperlinks` has allow-listed the hosting terminal.
fn osc8_open(uri: &str) -> String {
    format!("\x1b]8;;{uri}\x07")
}

/// The matching OSC 8 close escape — an empty URI parameter ends the link.
const OSC8_CLOSE: &str = "\x1b]8;;\x07";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorDepth;

    fn rendered_text(src: &str, th: &Theme) -> String {
        render(src, th)
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn link_destination_is_textual_when_hyperlinks_are_unsupported() {
        let th = Theme::for_test(ColorDepth::TrueColor, false, false);
        let out = rendered_text("[mockup](./mockups/login.html)", &th);
        assert!(
            out.contains("mockup (./mockups/login.html)"),
            "destination must be shown textually: {out:?}"
        );
        assert!(
            !out.contains('\u{1b}'),
            "no escape sequence must be emitted on an unsupported terminal: {out:?}"
        );
    }

    #[test]
    fn link_destination_is_osc8_wrapped_when_hyperlinks_are_supported() {
        let th = Theme::for_test(ColorDepth::TrueColor, false, true);
        let out = rendered_text("[mockup](./mockups/login.html)", &th);
        let expected = format!("{}mockup{OSC8_CLOSE}", osc8_open("./mockups/login.html"));
        assert!(
            out.contains(&expected),
            "link text must be wrapped in the OSC 8 escape: {out:?}"
        );
        // The textual fallback's parenthesised destination must not *also*
        // appear — the two presentations are mutually exclusive.
        assert!(!out.contains("(./mockups/login.html)"));
    }

    #[test]
    fn external_link_destination_survives_either_presentation() {
        for hyperlinks in [false, true] {
            let th = Theme::for_test(ColorDepth::TrueColor, false, hyperlinks);
            let out = rendered_text("[docs](https://example.com/x)", &th);
            assert!(
                out.contains("https://example.com/x"),
                "destination must be discoverable with hyperlinks={hyperlinks}: {out:?}"
            );
        }
    }

    #[test]
    fn plain_text_is_unaffected_by_link_handling() {
        let th = Theme::for_test(ColorDepth::TrueColor, false, false);
        let out = rendered_text("just a paragraph, no links here", &th);
        assert_eq!(out.trim(), "just a paragraph, no links here");
    }
}
