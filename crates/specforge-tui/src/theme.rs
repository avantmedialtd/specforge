//! Terminal-capability detection and the colour/glyph vocabulary.
//!
//! Resolved once at startup from the environment and shared read-only — except
//! the *active colour scheme*, a runtime-switchable `AtomicU8` index the Settings
//! screen can flip. Every colour the TUI paints — chrome accents, workspace
//! tints, commit-graph lanes, treatment rarities, per-person garden nodes — is
//! chosen by the active [`Scheme`] at truecolor fidelity and downsampled here to
//! whatever the terminal can actually show. An SSH session into a 256/16-colour
//! or `NO_COLOR`/`dumb` terminal therefore degrades cleanly (named colours, then
//! no colour at all) regardless of scheme, and emoji/box-drawing glyphs fall back
//! to ASCII.

use std::sync::atomic::{AtomicU8, Ordering};
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

/// A semantic colour role the renderer paints against, so a [`Scheme`] — not an
/// inline literal — owns the look. Chrome only; data hues (workspace tints, git
/// lanes, person nodes) are resolved by the dedicated `palette_fg`/`lane`/`person`
/// methods.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Focused borders, title-bar background, key hints, selection cursor.
    Accent,
    /// Foreground painted *on top of* the accent (title/key bar text).
    OnAccent,
    /// Secondary / muted text rendered with an explicit colour (not the DIM
    /// modifier).
    TextDim,
    /// Error text and markers.
    Error,
    /// Warning / "equipped" emphasis.
    Warn,
    /// Success / "unlocked" emphasis.
    Success,
}

/// A user-selectable colour scheme. Order is the cycle order and the persisted
/// index — keep it stable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    /// Today's desktop-matched brand look.
    Default,
    /// Brighter, bolder named colours for low-contrast terminals.
    HighContrast,
    /// No colour at all — distinctions via glyph, weight and reverse-video.
    Mono,
    Nord,
    Gruvbox,
    /// Defer to the terminal's own ANSI palette: never emit imposed RGB.
    Native,
}

impl Scheme {
    /// Cycle / persistence order. Index into this is the stored value.
    pub const ALL: [Scheme; 6] = [
        Scheme::Default,
        Scheme::HighContrast,
        Scheme::Mono,
        Scheme::Nord,
        Scheme::Gruvbox,
        Scheme::Native,
    ];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0) as u8
    }

    pub fn from_index(i: u8) -> Scheme {
        Self::ALL
            .get(i as usize)
            .copied()
            .unwrap_or(Scheme::Default)
    }

    /// The next scheme in cycle order (wraps), for the Settings picker.
    pub fn next(self) -> Scheme {
        Self::from_index((self.index() + 1) % Self::ALL.len() as u8)
    }

    /// Human-facing name shown in the Settings Appearance row.
    pub fn name(self) -> &'static str {
        match self {
            Scheme::Default => "Default",
            Scheme::HighContrast => "High contrast",
            Scheme::Mono => "Monochrome",
            Scheme::Nord => "Nord",
            Scheme::Gruvbox => "Gruvbox",
            Scheme::Native => "Terminal-native",
        }
    }

    /// Stable kebab key for on-disk persistence (decoupled from the index so the
    /// stored file stays readable and order changes don't silently remap).
    pub fn key(self) -> &'static str {
        match self {
            Scheme::Default => "default",
            Scheme::HighContrast => "high-contrast",
            Scheme::Mono => "mono",
            Scheme::Nord => "nord",
            Scheme::Gruvbox => "gruvbox",
            Scheme::Native => "native",
        }
    }

    pub fn from_key(k: &str) -> Option<Scheme> {
        Self::ALL.iter().copied().find(|s| s.key() == k)
    }

    /// Foreground for a semantic [`Slot`] under this scheme. Pure (no global
    /// reads) so scheme correctness is unit-testable without touching the active
    /// scheme. `th` is consulted only for RGB downsampling.
    pub fn slot(self, slot: Slot, th: &Theme) -> Color {
        match self {
            Scheme::Mono => Color::Reset,
            // Default and Native share the named chrome — Native's distinction is
            // that its *data* hues never upgrade to RGB (see `palette_fg`/`lane`).
            Scheme::Default | Scheme::Native => default_named(slot),
            Scheme::HighContrast => high_contrast_named(slot),
            Scheme::Nord => th.rgb(NORD_SLOTS[slot_index(slot)], default_named(slot)),
            Scheme::Gruvbox => th.rgb(GRUVBOX_SLOTS[slot_index(slot)], default_named(slot)),
        }
    }
}

/// The resolved, process-wide terminal capabilities plus the active scheme.
pub struct Theme {
    pub depth: ColorDepth,
    pub emoji: bool,
    /// Active [`Scheme`] index. Interior-mutable so the Settings screen can flip
    /// it through the shared `&'static Theme`.
    active: AtomicU8,
}

static THEME: OnceLock<Theme> = OnceLock::new();

/// The active theme, resolved from the environment on first use (idempotent).
pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::from_env)
}

/// Set the process-wide active scheme (Settings picker; persistence layer on
/// startup). Takes effect on the next redraw.
pub fn set_scheme(s: Scheme) {
    theme().set_scheme(s);
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

/// Nord workspace tints (frost + aurora spread), parallel to `PALETTE_RGB`.
const NORD_TINTS: [(u8, u8, u8); 8] = [
    (0x5E, 0x81, 0xAC), // Indigo  -> nord10
    (0x81, 0xA1, 0xC1), // Blue    -> nord9
    (0x88, 0xC0, 0xD0), // Teal    -> nord8
    (0xA3, 0xBE, 0x8C), // Green   -> nord14
    (0xEB, 0xCB, 0x8B), // Amber   -> nord13
    (0xD0, 0x87, 0x70), // Orange  -> nord12
    (0xBF, 0x61, 0x6A), // Rose    -> nord11
    (0xB4, 0x8E, 0xAD), // Purple  -> nord15
];

/// Nord commit-graph lanes (a readable spread of the same palette).
const NORD_LANES: [(u8, u8, u8); 8] = [
    (0x88, 0xC0, 0xD0),
    (0xA3, 0xBE, 0x8C),
    (0x81, 0xA1, 0xC1),
    (0xEB, 0xCB, 0x8B),
    (0xD0, 0x87, 0x70),
    (0xB4, 0x8E, 0xAD),
    (0x8F, 0xBC, 0xBB),
    (0x5E, 0x81, 0xAC),
];

/// Gruvbox (dark) workspace tints, parallel to `PALETTE_RGB`.
const GRUVBOX_TINTS: [(u8, u8, u8); 8] = [
    (0x83, 0xA5, 0x98), // Indigo  -> blue
    (0x45, 0x85, 0x88), // Blue    -> dim blue
    (0x8E, 0xC0, 0x7C), // Teal    -> aqua
    (0xB8, 0xBB, 0x26), // Green
    (0xFA, 0xBD, 0x2F), // Amber   -> yellow
    (0xFE, 0x80, 0x19), // Orange
    (0xFB, 0x49, 0x34), // Rose    -> red
    (0xD3, 0x86, 0x9B), // Purple
];

/// Gruvbox commit-graph lanes.
const GRUVBOX_LANES: [(u8, u8, u8); 8] = [
    (0x83, 0xA5, 0x98),
    (0x8E, 0xC0, 0x7C),
    (0xB8, 0xBB, 0x26),
    (0xFA, 0xBD, 0x2F),
    (0xFE, 0x80, 0x19),
    (0xD3, 0x86, 0x9B),
    (0x45, 0x85, 0x88),
    (0xFB, 0x49, 0x34),
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

        Theme {
            depth,
            emoji,
            active: AtomicU8::new(Scheme::Default.index()),
        }
    }

    /// The active colour scheme.
    pub fn active_scheme(&self) -> Scheme {
        Scheme::from_index(self.active.load(Ordering::Relaxed))
    }

    /// Switch the active scheme. Read on the next redraw.
    pub fn set_scheme(&self, s: Scheme) {
        self.active.store(s.index(), Ordering::Relaxed);
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

    /// Foreground for a semantic [`Slot`] under the active scheme. A terminal
    /// that disables colour (`NO_COLOR`, `dumb`) collapses every slot to the
    /// default ink, overriding the scheme.
    pub fn slot(&self, slot: Slot) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        self.active_scheme().slot(slot, self)
    }

    /// The accent — the most-used slot. Shorthand for `slot(Slot::Accent)`.
    pub fn accent(&self) -> Color {
        self.slot(Slot::Accent)
    }

    /// Foreground colour for a workspace tint, under the active scheme.
    pub fn palette_fg(&self, c: PaletteColor) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        let i = palette_index(c);
        match self.active_scheme() {
            Scheme::Mono => Color::Reset,
            // Defer to the terminal's own ANSI palette — never upgrade to RGB.
            Scheme::Native => PALETTE_ANSI16[i],
            Scheme::Nord => self.rgb(NORD_TINTS[i], PALETTE_ANSI16[i]),
            Scheme::Gruvbox => self.rgb(GRUVBOX_TINTS[i], PALETTE_ANSI16[i]),
            Scheme::Default | Scheme::HighContrast => self.rgb(PALETTE_RGB[i], PALETTE_ANSI16[i]),
        }
    }

    /// A bold header style, tinted when a colour is set and colour is available.
    pub fn header_style(&self, color: Option<PaletteColor>) -> Style {
        let base = Style::default().add_modifier(Modifier::BOLD);
        match color {
            Some(c) if self.depth != ColorDepth::Mono => base.fg(self.palette_fg(c)),
            _ => base,
        }
    }

    /// Colour for commit-graph lane `col` (cycles every 8 lanes), under the
    /// active scheme.
    pub fn lane(&self, col: usize) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        let i = col % LANE_RGB.len();
        // Lane fallbacks reuse the person spread for a reasonable 16-colour read.
        let floor = PERSON_COLORS[i % PERSON_COLORS.len()];
        match self.active_scheme() {
            Scheme::Mono => Color::Reset,
            Scheme::Native => floor,
            Scheme::Nord => self.rgb(NORD_LANES[i], floor),
            Scheme::Gruvbox => self.rgb(GRUVBOX_LANES[i], floor),
            Scheme::Default | Scheme::HighContrast => self.rgb(LANE_RGB[i], floor),
        }
    }

    /// A stable colour for a garden node's person, or the accent for "me".
    pub fn person(&self, key: &str, is_me: bool, accent: Color) -> Color {
        if self.depth == ColorDepth::Mono || self.active_scheme() == Scheme::Mono {
            return Color::Reset;
        }
        if is_me {
            return accent;
        }
        PERSON_COLORS[(fnv1a(&key.to_ascii_lowercase()) as usize) % PERSON_COLORS.len()]
    }

    /// Colour for a treatment rarity, under the active scheme.
    pub fn rarity(&self, r: Rarity) -> Color {
        if self.depth == ColorDepth::Mono || self.active_scheme() == Scheme::Mono {
            return Color::Reset;
        }
        match r {
            Rarity::Common => Color::Gray,
            Rarity::Rare => Color::Cyan,
            Rarity::Epic => Color::Magenta,
            Rarity::Legendary => Color::Yellow,
        }
    }

    /// Title-bar quota-gauge colour by severity (0 ok, 1 warn, 2 critical), under
    /// the active scheme.
    pub fn quota(&self, severity: u8) -> Color {
        if self.depth == ColorDepth::Mono {
            return Color::Reset;
        }
        match self.active_scheme() {
            Scheme::Mono => Color::Reset,
            Scheme::Native => match severity {
                2 => Color::Red,
                1 => Color::Yellow,
                _ => Color::Green,
            },
            _ => match severity {
                2 => self.rgb((255, 59, 48), Color::Red),
                1 => self.rgb((255, 159, 10), Color::Yellow),
                _ => self.rgb((52, 199, 89), Color::Green),
            },
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

/// Default / Terminal-native named colours per slot. These are exactly the
/// inline colours the renderer used before the slot refactor, so the Default
/// scheme is pixel-identical to the pre-scheme TUI.
fn default_named(slot: Slot) -> Color {
    match slot {
        Slot::Accent => Color::Cyan,
        Slot::OnAccent => Color::Black,
        Slot::TextDim => Color::DarkGray,
        Slot::Error => Color::Red,
        Slot::Warn => Color::Yellow,
        Slot::Success => Color::Green,
    }
}

/// High-contrast named colours — brighter accents and lighter dim text.
fn high_contrast_named(slot: Slot) -> Color {
    match slot {
        Slot::Accent => Color::White,
        Slot::OnAccent => Color::Black,
        Slot::TextDim => Color::Gray,
        Slot::Error => Color::LightRed,
        Slot::Warn => Color::LightYellow,
        Slot::Success => Color::LightGreen,
    }
}

fn slot_index(slot: Slot) -> usize {
    match slot {
        Slot::Accent => 0,
        Slot::OnAccent => 1,
        Slot::TextDim => 2,
        Slot::Error => 3,
        Slot::Warn => 4,
        Slot::Success => 5,
    }
}

/// Nord slot RGB, parallel to [`slot_index`].
const NORD_SLOTS: [(u8, u8, u8); 6] = [
    (0x88, 0xC0, 0xD0), // Accent   -> frost
    (0x2E, 0x34, 0x40), // OnAccent -> polar night
    (0x4C, 0x56, 0x6A), // TextDim
    (0xBF, 0x61, 0x6A), // Error
    (0xEB, 0xCB, 0x8B), // Warn
    (0xA3, 0xBE, 0x8C), // Success
];

/// Gruvbox slot RGB, parallel to [`slot_index`].
const GRUVBOX_SLOTS: [(u8, u8, u8); 6] = [
    (0x8E, 0xC0, 0x7C), // Accent   -> aqua
    (0x28, 0x28, 0x28), // OnAccent -> bg
    (0x92, 0x83, 0x74), // TextDim  -> gray
    (0xFB, 0x49, 0x34), // Error
    (0xFA, 0xBD, 0x2F), // Warn
    (0xB8, 0xBB, 0x26), // Success
];

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A theme pinned to a given depth with the default scheme, for resolution
    /// tests that must not depend on the ambient terminal.
    fn theme_at(depth: ColorDepth) -> Theme {
        Theme {
            depth,
            emoji: false,
            active: AtomicU8::new(Scheme::Default.index()),
        }
    }

    #[test]
    fn default_scheme_slots_are_the_legacy_named_colors() {
        let th = theme_at(ColorDepth::TrueColor);
        assert_eq!(th.slot(Slot::Accent), Color::Cyan);
        assert_eq!(th.accent(), Color::Cyan);
        assert_eq!(th.slot(Slot::OnAccent), Color::Black);
        assert_eq!(th.slot(Slot::TextDim), Color::DarkGray);
        assert_eq!(th.slot(Slot::Error), Color::Red);
        assert_eq!(th.slot(Slot::Warn), Color::Yellow);
        assert_eq!(th.slot(Slot::Success), Color::Green);
    }

    #[test]
    fn default_data_palettes_upgrade_to_rgb_at_truecolor() {
        let th = theme_at(ColorDepth::TrueColor);
        // Pixel-identity: Default still emits the brand RGB tints.
        assert_eq!(
            th.palette_fg(PaletteColor::Indigo),
            Color::Rgb(0x59, 0x52, 0xE0)
        );
        assert_eq!(th.lane(0), Color::Rgb(0x7c, 0x9c, 0xff));
        assert_eq!(th.quota(2), Color::Rgb(255, 59, 48));
    }

    #[test]
    fn native_scheme_never_emits_rgb_even_at_truecolor() {
        let th = theme_at(ColorDepth::TrueColor);
        th.set_scheme(Scheme::Native);
        // Chrome stays named; data hues stay on the ANSI-16 floor.
        assert_eq!(th.accent(), Color::Cyan);
        for c in [
            PaletteColor::Indigo,
            PaletteColor::Teal,
            PaletteColor::Rose,
            PaletteColor::Purple,
        ] {
            assert!(
                !matches!(th.palette_fg(c), Color::Rgb(..) | Color::Indexed(..)),
                "native palette_fg must be a named ANSI colour"
            );
        }
        for col in 0..8 {
            assert!(
                !matches!(th.lane(col), Color::Rgb(..) | Color::Indexed(..)),
                "native lane must be a named ANSI colour"
            );
        }
        assert!(!matches!(th.quota(0), Color::Rgb(..)));
    }

    #[test]
    fn mono_scheme_resolves_everything_to_reset() {
        let th = theme_at(ColorDepth::TrueColor);
        th.set_scheme(Scheme::Mono);
        for slot in [
            Slot::Accent,
            Slot::OnAccent,
            Slot::TextDim,
            Slot::Error,
            Slot::Warn,
            Slot::Success,
        ] {
            assert_eq!(th.slot(slot), Color::Reset);
        }
        assert_eq!(th.palette_fg(PaletteColor::Teal), Color::Reset);
        assert_eq!(th.lane(3), Color::Reset);
        assert_eq!(th.rarity(Rarity::Epic), Color::Reset);
        assert_eq!(th.quota(1), Color::Reset);
        assert_eq!(th.person("ada", false, Color::Cyan), Color::Reset);
    }

    #[test]
    fn no_color_terminal_overrides_any_scheme() {
        let th = theme_at(ColorDepth::Mono);
        for s in Scheme::ALL {
            th.set_scheme(s);
            assert_eq!(
                th.accent(),
                Color::Reset,
                "{s:?} must be colourless on a mono terminal"
            );
            assert_eq!(th.palette_fg(PaletteColor::Rose), Color::Reset);
        }
    }

    #[test]
    fn scheme_index_roundtrips_and_cycles_through_all() {
        let mut seen = Vec::new();
        let mut s = Scheme::Default;
        for _ in 0..Scheme::ALL.len() {
            assert_eq!(Scheme::from_index(s.index()), s);
            assert_eq!(Scheme::from_key(s.key()), Some(s));
            seen.push(s);
            s = s.next();
        }
        assert_eq!(s, Scheme::Default, "cycle wraps back to the start");
        assert_eq!(seen.len(), Scheme::ALL.len());
        assert_eq!(
            Scheme::from_index(99),
            Scheme::Default,
            "out-of-range falls back"
        );
        assert_eq!(Scheme::from_key("bogus"), None);
    }
}
