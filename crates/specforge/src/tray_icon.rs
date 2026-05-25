//! Render the tray glyph from its SVG source.
//!
//! Two glyph variants are bundled at compile time: a default and a
//! spec-activity variant. `TrayGlyphState` carries the current variant
//! between the updater task that flips it and the scale-change handler
//! that needs to know which SVG to re-rasterize.
//!
//! macOS template rendering requires the output to be pure black + alpha,
//! so a debug-only sanity check walks the buffer and panics on any
//! non-zero R/G/B component.

use resvg::tiny_skia::{Pixmap, Transform};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::image::Image;
use usvg::Tree;

/// Default tray glyph. Pure-black silhouette required for macOS template
/// rendering — guarded by a debug-build pixel check in [`rasterize`].
pub const SVG_DEFAULT: &[u8] = include_bytes!("../icons/tray-icon.svg");

/// Spec-activity tray glyph, shown when any active change in any
/// registered workspace has a non-empty `ArtifactStatus.specs`.
pub const SVG_SPECS: &[u8] = include_bytes!("../icons/tray-specs.svg");

/// Logical (point) edge length of the tray glyph. macOS menu bar is ~22pt;
/// other platforms get the same size (slight upsize on Windows/Linux tray
/// areas, accepted per design until we measure it).
pub const LOGICAL_SIZE: u32 = 22;

/// Which glyph variant the tray is currently showing.
///
/// Discriminants are explicit so the value round-trips losslessly through
/// the `AtomicU8` backing [`TrayGlyphState`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TrayGlyph {
    Default = 0,
    Specs = 1,
}

impl TrayGlyph {
    /// SVG bytes for this variant.
    pub fn svg(self) -> &'static [u8] {
        match self {
            TrayGlyph::Default => SVG_DEFAULT,
            TrayGlyph::Specs => SVG_SPECS,
        }
    }
}

impl From<TrayGlyph> for u8 {
    fn from(g: TrayGlyph) -> u8 {
        g as u8
    }
}

impl TryFrom<u8> for TrayGlyph {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0 => Ok(TrayGlyph::Default),
            1 => Ok(TrayGlyph::Specs),
            other => Err(other),
        }
    }
}

/// Shared, lock-free cell holding the current tray glyph variant. Cloneable;
/// every clone references the same underlying atomic. The glyph-updater task
/// writes; the scale-change handler only reads.
#[derive(Clone, Debug)]
pub struct TrayGlyphState(Arc<AtomicU8>);

impl TrayGlyphState {
    /// Seed the state with an initial variant.
    pub fn new(initial: TrayGlyph) -> Self {
        Self(Arc::new(AtomicU8::new(initial as u8)))
    }

    pub fn load(&self) -> TrayGlyph {
        let raw = self.0.load(Ordering::Relaxed);
        TrayGlyph::try_from(raw).unwrap_or(TrayGlyph::Default)
    }

    pub fn store(&self, glyph: TrayGlyph) {
        self.0.store(glyph as u8, Ordering::Relaxed);
    }
}

/// Rasterize the given variant at [`LOGICAL_SIZE`] for the given scale factor.
pub fn rasterize_glyph(variant: TrayGlyph, scale: f64) -> Image<'static> {
    rasterize(variant.svg(), LOGICAL_SIZE, scale)
}

/// Rasterize an SVG to an `Image` sized for the given logical size and
/// display scale factor. Output is square, `(logical_size * scale).round()`
/// pixels per side.
///
/// Panics on debug builds if any output pixel has non-zero R/G/B; release
/// builds skip the check.
pub fn rasterize(svg_bytes: &[u8], logical_size: u32, scale: f64) -> Image<'static> {
    let pixel_size = ((logical_size as f64) * scale).round().max(1.0) as u32;

    let tree = Tree::from_data(svg_bytes, &usvg::Options::default())
        .expect("tray-icon SVG must parse");

    let mut pixmap = Pixmap::new(pixel_size, pixel_size).expect("pixel_size > 0");

    let svg_size = tree.size();
    let transform = Transform::from_scale(
        pixel_size as f32 / svg_size.width(),
        pixel_size as f32 / svg_size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny_skia stores premultiplied RGBA. For a pure-black source (R=G=B=0)
    // premultiplied == straight, so we can hand the buffer to Tauri as-is.
    // The debug assertion below enforces the pure-black invariant.
    let rgba = pixmap.take();

    #[cfg(debug_assertions)]
    assert_template_safe(&rgba);

    Image::new_owned(rgba, pixel_size, pixel_size)
}

/// Every pixel's R/G/B must be zero so macOS template rendering can recolour
/// from the alpha channel alone.
#[cfg(debug_assertions)]
fn assert_template_safe(rgba: &[u8]) {
    for (i, chunk) in rgba.chunks_exact(4).enumerate() {
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        assert!(
            r == 0 && g == 0 && b == 0,
            "tray icon pixel {i} has non-black RGB (r={r}, g={g}, b={b}); \
             macOS template rendering requires pure-black + alpha",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_both_variants_at_multiple_scales() {
        for variant in [TrayGlyph::Default, TrayGlyph::Specs] {
            for scale in [1.0_f64, 2.0, 3.0] {
                let img = rasterize_glyph(variant, scale);
                let expected_side = (LOGICAL_SIZE as f64 * scale).round() as u32;
                assert_eq!(img.width(), expected_side, "{variant:?} @ {scale}: width");
                assert_eq!(img.height(), expected_side, "{variant:?} @ {scale}: height");
                assert_eq!(
                    img.rgba().len(),
                    (expected_side * expected_side * 4) as usize,
                    "{variant:?} @ {scale}: buffer dims = side² × 4",
                );
            }
        }
    }

    #[test]
    fn tray_glyph_round_trips_through_u8() {
        for v in [TrayGlyph::Default, TrayGlyph::Specs] {
            let raw: u8 = v.into();
            assert_eq!(TrayGlyph::try_from(raw).unwrap(), v);
        }
        assert!(TrayGlyph::try_from(2).is_err());
    }

    #[test]
    fn tray_glyph_state_stores_and_loads() {
        let state = TrayGlyphState::new(TrayGlyph::Default);
        assert_eq!(state.load(), TrayGlyph::Default);
        state.store(TrayGlyph::Specs);
        assert_eq!(state.load(), TrayGlyph::Specs);
        let clone = state.clone();
        clone.store(TrayGlyph::Default);
        assert_eq!(state.load(), TrayGlyph::Default);
    }
}
