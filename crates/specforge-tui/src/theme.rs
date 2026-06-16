//! Terminal-capability detection and the colour/glyph vocabulary.
//!
//! Resolved once at startup from the environment and shared read-only. Every
//! colour the TUI paints — workspace tints, commit-graph lanes, treatment
//! rarities, per-person garden nodes — is chosen at truecolor fidelity and
//! downsampled here to whatever the terminal can actually show. An SSH session
//! into a 256/16-colour or `NO_COLOR`/`dumb` terminal therefore degrades
//! cleanly (named colours, then no colour at all) instead of emitting escape
//! codes it can't honour, and emoji/box-drawing glyphs fall back to ASCII.

use std::sync::OnceLock;

use openspec_core::{PaletteColor, Rarity};
use ratatui::style::{Color, Modifier, Style};

/// How much colour the terminal can render, narrowest last.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
    Ansi16,
    Mono,
}

/// The resolved, process-wide terminal capabilities.
pub struct Theme {
    pub depth: ColorDepth,
    pub emoji: bool,
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// The active theme, resolved from the environment on first use (idempotent).
pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::from_env)
}

/// The eight workspace-tint hues, as the desktop's scheme-stable swatch RGB.
/// Order matches [`PaletteColor`]'s declaration so the index lines up.
const PALETTE_RGB: [(u8, u8, u8); 8] = [
    (0x59, 0x52, 0xE0), // Indigo
    (0x36, 0x92, 0xE2), // Blue
    (0x2E, 0xB8, 0xAA), // Teal
    (0x34, 0xB2, 0x6A), // Green
    (0xEE, 0xB7, 0x2B), // Amber
    (0xEE, 0x72, 0x2B), // Orange
    (0xE6, 0x4C, 0x66), // Rose
    (0xAD, 0x5C, 0xD6), // Purple
];

/// The eight commit-graph lane colours (mirrors the desktop `LANE_COLORS`).
const LANE_RGB: [(u8, u8, u8); 8] = [
    (0x7c, 0x9c, 0xff),
    (0x5c, 0xc6, 0xb0),
    (0x69, 0xc2, 0x67),
    (0xd9, 0xa4, 0x41),
    (0xe0, 0x79, 0x5b),
    (0xc6, 0x78, 0xb4),
    (0x56, 0xb6, 0xe0),
    (0xb5, 0x8d, 0xf0),
];

/// A stable spread of named colours for per-person garden attribution. Named
/// (not RGB) so they survive on 16-colour terminals; keyed by a hash of the
/// person so a developer always draws in the same hue.
const PERSON_COLORS: [Color; 10] = [
    Color::Green,
    Color::Magenta,
    Color::Yellow,
    Color::Blue,
    Color::Red,
    Color::LightGreen,
    Color::LightMagenta,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightRed,
];

impl Theme {
    fn from_env() -> Self {
        let env = |k: &str| std::env::var(k).ok();
        let term = env("TERM").unwrap_or_default();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let dumb = term == "dumb" || term.is_empty();

        let depth = if no_color || dumb {
            ColorDepth::Mono
        } else if matches!(
            env("COLORTERM").as_deref(),
            Some("truecolor") | Some("24bit")
        ) {
            ColorDepth::TrueColor
        } else if term.contains("256color") {
            ColorDepth::Ansi256
        } else {
            ColorDepth::Ansi16
        };

        // Emoji/box-drawing only when the locale advertises UTF-8 and the
        // terminal isn't degraded.
        let unicode = ["LC_ALL", "LC_CTYPE", "LANG"]
            .iter()
            .filter_map(|k| env(k))
            .any(|v| v.to_ascii_lowercase().contains("utf"));
        let emoji = unicode && !dumb && depth != ColorDepth::Mono;

        Theme { depth, emoji }
    }

    /// Downsample an RGB triple to the terminal's depth, using `ansi16` as the
    /// nearest named colour and `Reset` (the default ink) when colour is off.
    pub fn rgb(&self, (r, g, b): (u8, u8, u8), ansi16: Color) -> Color {
        match self.depth {
            ColorDepth::TrueColor => Color::Rgb(r, g, b),
            ColorDepth::Ansi256 => Color::Indexed(rgb_to_256(r, g, b)),
            ColorDepth::Ansi16 => ansi16,
            ColorDepth::Mono => Color::Reset,
        }
    }

    /// Foreground colour for a workspace tint.
    pub fn palette_fg(&self, c: PaletteColor) -> Color {
        let i = palette_index(c);
        self.rgb(PALETTE_RGB[i], PALETTE_ANSI16[i])
    }

    /// A bold header style, tinted when a colour is set and colour is available.
    pub fn header_style(&self, color: Option<PaletteColor>) -> Style {
        let base = Style::default().add_modifier(Modifier::BOLD);
        match color {
            Some(c) if self.depth != ColorDepth::Mono => base.fg(self.palette_fg(c)),
            _ => base,
        }
    }

    /// Colour for commit-graph lane `col` (cycles every 8 lanes).
    pub fn lane(&self, col: usize) -> Color {
        let i = col % LANE_RGB.len();
        // Lane fallbacks reuse the person spread for a reasonable 16-colour read.
        self.rgb(LANE_RGB[i], PERSON_COLORS[i % PERSON_COLORS.len()])
    }

    /// A stable colour for a garden node's person, or the accent for "me".
    pub fn person(&self, key: &str, is_me: bool, accent: Color) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        if is_me {
            return accent;
        }
        PERSON_COLORS[(fnv1a(&key.to_ascii_lowercase()) as usize) % PERSON_COLORS.len()]
    }

    /// Colour for a treatment rarity.
    pub fn rarity(&self, r: Rarity) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        match r {
            Rarity::Common => Color::Gray,
            Rarity::Rare => Color::Cyan,
            Rarity::Epic => Color::Magenta,
            Rarity::Legendary => Color::Yellow,
        }
    }

    /// Pick an emoji/unicode glyph or its ASCII fallback by capability.
    pub fn glyph<'a>(&self, fancy: &'a str, ascii: &'a str) -> &'a str {
        if self.emoji {
            fancy
        } else {
            ascii
        }
    }
}

/// Nearest base-16 names for the palette hues, parallel to `PALETTE_RGB`.
const PALETTE_ANSI16: [Color; 8] = [
    Color::Blue,         // Indigo
    Color::LightBlue,    // Blue
    Color::Cyan,         // Teal
    Color::Green,        // Green
    Color::Yellow,       // Amber
    Color::LightRed,     // Orange
    Color::LightMagenta, // Rose
    Color::Magenta,      // Purple
];

fn palette_index(c: PaletteColor) -> usize {
    match c {
        PaletteColor::Indigo => 0,
        PaletteColor::Blue => 1,
        PaletteColor::Teal => 2,
        PaletteColor::Green => 3,
        PaletteColor::Amber => 4,
        PaletteColor::Orange => 5,
        PaletteColor::Rose => 6,
        PaletteColor::Purple => 7,
    }
}

/// Map an 8-bit RGB triple to the nearest xterm-256 6×6×6 colour-cube index.
fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    let q = |v: u8| (v as u16 * 5 / 255) as u8;
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

/// FNV-1a 32-bit, matching the desktop garden's person-colour seed.
fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for byte in s.bytes() {
        h ^= byte as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
