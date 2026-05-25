//! Render the tray glyph from its SVG source.
//!
//! The SVG is bundled at compile time and rasterized at the active monitor's
//! pixel density. macOS template rendering requires the output to be pure
//! black + alpha, so a debug-only sanity check walks the buffer and panics on
//! any non-zero R/G/B component.

use resvg::tiny_skia::{Pixmap, Transform};
use tauri::image::Image;
use usvg::Tree;

/// The tray glyph, bundled at compile time. Must be a solid-black silhouette
/// for macOS template rendering — guarded by a debug-build pixel check in
/// [`rasterize`].
pub const SVG: &[u8] = include_bytes!("../icons/tray-icon.svg");

/// Logical (point) edge length of the tray glyph. macOS menu bar is ~22pt;
/// other platforms get the same size (slight upsize on Windows/Linux tray
/// areas, accepted per design until we measure it).
pub const LOGICAL_SIZE: u32 = 22;

/// Rasterize the bundled SVG at [`LOGICAL_SIZE`] for the given scale factor.
pub fn rasterize_glyph(scale: f64) -> Image<'static> {
    rasterize(SVG, LOGICAL_SIZE, scale)
}

/// Rasterize an SVG to an `Image` sized for the given logical size and
/// display scale factor. Output is square, `(logical_size * scale).round()`
/// pixels per side.
///
/// Panics on debug builds if any output pixel has non-zero R/G/B; release
/// builds skip the check.
pub fn rasterize(svg_bytes: &[u8], logical_size: u32, scale: f64) -> Image<'static> {
    let pixel_size = ((logical_size as f64) * scale).round().max(1.0) as u32;

    let tree =
        Tree::from_data(svg_bytes, &usvg::Options::default()).expect("tray-icon SVG must parse");

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
    fn rasterizes_at_multiple_scales() {
        for scale in [1.0_f64, 2.0, 3.0] {
            let logical_size = 22u32;
            let img = rasterize(SVG, logical_size, scale);
            let expected_side = (logical_size as f64 * scale).round() as u32;
            assert_eq!(img.width(), expected_side, "scale {scale}: width");
            assert_eq!(img.height(), expected_side, "scale {scale}: height");
            assert_eq!(
                img.rgba().len(),
                (expected_side * expected_side * 4) as usize,
                "scale {scale}: buffer dims = side² × 4",
            );
        }
    }
}
