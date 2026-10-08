//! What one version of a changed image file is, decided from its bytes alone
//! (`diff-view`: *Image Comparison*). Pure, with no I/O: the commit read and
//! the pull-request image read both hand it bytes, and only a version it
//! answers `Image` for ever reaches a web view's decoder.
//!
//! A version is an image only when its bytes begin with one of seven
//! signatures, never because of its name. Its width and height come from its
//! header alone, so a few kilobytes declaring a huge canvas are refused
//! before anything decodes them.

use crate::diff::REQUESTED_FILE_BYTES_LIMIT;
use imagesize::{Compression, ImageType};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// The most pixels a version may declare, its width times its height. It
/// admits an 8K screenshot (7680 × 4320) and refuses the classic bomb, a
/// small PNG declaring 30,000 × 30,000.
pub const IMAGE_PIXELS_LIMIT: u64 = 40_000_000;

/// A Git LFS pointer is shorter than this.
const LFS_POINTER_BYTES_LIMIT: usize = 1024;

/// A Git LFS pointer's first line.
const LFS_VERSION_LINE: &str = "version https://git-lfs.github.com/spec/v1";

/// Why a version is not shown as an image, the first that applies in this
/// order: a Git LFS pointer is no image of its own, a version past the 8 MiB
/// ceiling is never inspected, and only an image has pixels to count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageRefusal {
    Lfs,
    TooLarge,
    NotImage,
    TooManyPixels,
}

/// What [`inspect`] found a version to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageCheck {
    /// An image the web view may decode: its type, sniffed from its
    /// signature, and its dimensions, read from its header.
    Image {
        mime: &'static str,
        width: u32,
        height: u32,
    },
    Refused(ImageRefusal),
}

/// Decides what one version's bytes are.
pub fn inspect(bytes: &[u8]) -> ImageCheck {
    if is_lfs_pointer(bytes) {
        return ImageCheck::Refused(ImageRefusal::Lfs);
    }
    if bytes.len() > REQUESTED_FILE_BYTES_LIMIT {
        return ImageCheck::Refused(ImageRefusal::TooLarge);
    }
    let Some((mime, kind)) = sniff(bytes) else {
        return ImageCheck::Refused(ImageRefusal::NotImage);
    };
    // Measured as the type the signature named, so the header is read the
    // way the web view will read it. The reader starts where `imagesize`'s
    // own type check leaves it, past the 12-byte header: its WebP reader
    // reads the chunk tag from there, and every other reader seeks.
    let mut header = Cursor::new(bytes);
    header.set_position(12);
    let Ok(size) = kind.reader_size(&mut header) else {
        return ImageCheck::Refused(ImageRefusal::NotImage);
    };
    if size.width == 0 || size.height == 0 {
        return ImageCheck::Refused(ImageRefusal::NotImage);
    }
    if size.width as u128 * size.height as u128 > u128::from(IMAGE_PIXELS_LIMIT) {
        return ImageCheck::Refused(ImageRefusal::TooManyPixels);
    }
    // Within the pixel ceiling, neither side can pass `u32::MAX`.
    ImageCheck::Image {
        mime,
        width: size.width as u32,
        height: size.height as u32,
    }
}

/// The type whose signature `bytes` begin with, as a MIME type and as the
/// `imagesize` type that measures it. AVIF is the one HEIF that qualifies;
/// HEIC and every other brand are not images here.
fn sniff(bytes: &[u8]) -> Option<(&'static str, ImageType)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", ImageType::Png))
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        Some(("image/jpeg", ImageType::Jpeg))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", ImageType::Gif))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(("image/webp", ImageType::Webp))
    } else if bytes.starts_with(b"BM") {
        Some(("image/bmp", ImageType::Bmp))
    } else if bytes.starts_with(b"\0\0\x01\0") {
        Some(("image/x-icon", ImageType::Ico))
    } else if let Ok(avif @ ImageType::Heif(Compression::Av1)) = imagesize::image_type(bytes) {
        Some(("image/avif", avif))
    } else {
        None
    }
}

/// Whether `bytes` are a Git LFS pointer: text shorter than 1,024 bytes whose
/// first line names the pointer format, with an `oid sha256:` line of 64
/// hexadecimal digits, a `size` line of decimal digits, and nothing else but
/// the `ext-` lines Git LFS allows.
pub fn is_lfs_pointer(bytes: &[u8]) -> bool {
    if bytes.len() >= LFS_POINTER_BYTES_LIMIT {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text.lines();
    if lines.next() != Some(LFS_VERSION_LINE) {
        return false;
    }
    let (mut oid, mut size) = (false, false);
    for line in lines {
        if let Some(hex) = line.strip_prefix("oid sha256:") {
            oid = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        } else if let Some(digits) = line.strip_prefix("size ") {
            size = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
        } else if !line.starts_with("ext-") {
            return false;
        }
    }
    oid && size
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real files, 3 × 2 pixels each so a swapped width and height fails,
    // written by Pillow (and `heif-enc` for the HEIC).
    const PNG: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.png");
    const JPEG: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.jpg");
    const GIF87A: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two-87a.gif");
    const GIF89A: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two-89a.gif");
    const WEBP: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.webp");
    const BMP: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.bmp");
    const AVIF: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.avif");
    const HEIC: &[u8] = include_bytes!("../tests/fixtures/images/three-by-two.heic");
    /// Entries of 16, 32 and 256 pixels.
    const ICO: &[u8] = include_bytes!("../tests/fixtures/images/sixteen-to-256.ico");

    const OID: &str = "4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393";

    fn image(mime: &'static str, width: u32, height: u32) -> ImageCheck {
        ImageCheck::Image {
            mime,
            width,
            height,
        }
    }

    fn refused(reason: ImageRefusal) -> ImageCheck {
        ImageCheck::Refused(reason)
    }

    /// A PNG signature and `IHDR` declaring `width` × `height`, which is all
    /// a header read sees, padded with zeros to `len` bytes.
    fn png_header(width: u32, height: u32, len: usize) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        bytes.resize(len.max(bytes.len()), 0);
        bytes
    }

    fn pointer(lines: &[&str]) -> Vec<u8> {
        lines
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>()
            .into_bytes()
    }

    #[test]
    fn each_signature_is_an_image_of_its_header_size() {
        assert_eq!(inspect(PNG), image("image/png", 3, 2));
        assert_eq!(inspect(JPEG), image("image/jpeg", 3, 2));
        assert_eq!(inspect(GIF87A), image("image/gif", 3, 2));
        assert_eq!(inspect(GIF89A), image("image/gif", 3, 2));
        assert_eq!(inspect(WEBP), image("image/webp", 3, 2));
        assert_eq!(inspect(BMP), image("image/bmp", 3, 2));
        assert_eq!(inspect(AVIF), image("image/avif", 3, 2));
    }

    #[test]
    fn heic_is_refused_while_avif_is_an_image() {
        assert_eq!(inspect(HEIC), refused(ImageRefusal::NotImage));
        assert_eq!(inspect(AVIF), image("image/avif", 3, 2));
    }

    #[test]
    fn an_icon_measures_its_largest_entry() {
        assert_eq!(inspect(ICO), image("image/x-icon", 256, 256));
    }

    #[test]
    fn the_bytes_decide_and_never_the_name() {
        // Text, markup, and signatures `imagesize` would accept but these
        // seven do not: a PNG's first four bytes alone, a GIF of no version,
        // and a RIFF holding sound.
        for bytes in [
            &b""[..],
            b"just some text that is long enough",
            b"<!DOCTYPE html><html><body>Not found</body></html>",
            b"\x89PNG\0\0\0\0\0\0\0\x0dIHDR\0\0\0\x03\0\0\0\x02",
            b"GIF88a\x03\x00\x02\x00\x81\x00",
            b"RIFF\x1e\x00\x00\x00WAVEfmt ",
        ] {
            assert_eq!(inspect(bytes), refused(ImageRefusal::NotImage), "{bytes:?}");
        }
        // A signature whose header cannot be read.
        assert_eq!(inspect(&PNG[..12]), refused(ImageRefusal::NotImage));
        assert_eq!(inspect(&ICO[..6]), refused(ImageRefusal::NotImage));
        // A WebP needs both halves of its signature: a real one whose RIFF is
        // spelt otherwise still has a readable header, and is still refused.
        let mut not_riff = WEBP.to_vec();
        not_riff[..4].copy_from_slice(b"RIFX");
        assert_eq!(inspect(&not_riff), refused(ImageRefusal::NotImage));
    }

    #[test]
    fn a_header_declaring_no_pixels_is_not_an_image() {
        assert_eq!(
            inspect(&png_header(0, 2, 64)),
            refused(ImageRefusal::NotImage)
        );
        assert_eq!(
            inspect(&png_header(3, 0, 64)),
            refused(ImageRefusal::NotImage)
        );
        assert_eq!(inspect(&png_header(3, 2, 64)), image("image/png", 3, 2));
    }

    #[test]
    fn the_byte_ceiling_is_exact() {
        let at = png_header(3, 2, REQUESTED_FILE_BYTES_LIMIT);
        assert_eq!(at.len(), 8 * 1024 * 1024);
        assert_eq!(inspect(&at), image("image/png", 3, 2));
        let past = png_header(3, 2, REQUESTED_FILE_BYTES_LIMIT + 1);
        assert_eq!(inspect(&past), refused(ImageRefusal::TooLarge));
    }

    #[test]
    fn the_pixel_ceiling_is_exact() {
        // 8,000 × 5,000 is the ceiling itself; 53 × 754,717 is one more.
        assert_eq!(
            inspect(&png_header(8_000, 5_000, 64)),
            image("image/png", 8_000, 5_000)
        );
        assert_eq!(53 * 754_717, IMAGE_PIXELS_LIMIT + 1);
        assert_eq!(
            inspect(&png_header(53, 754_717, 64)),
            refused(ImageRefusal::TooManyPixels)
        );
        // The bomb: a few bytes declaring 30,000 × 30,000.
        assert_eq!(
            inspect(&png_header(30_000, 30_000, 64)),
            refused(ImageRefusal::TooManyPixels)
        );
        // No product of two `u32`s overflows the count.
        assert_eq!(
            inspect(&png_header(u32::MAX, u32::MAX, 64)),
            refused(ImageRefusal::TooManyPixels)
        );
    }

    #[test]
    fn refusals_apply_in_order() {
        // Past the byte ceiling before anything is sniffed or measured.
        let nine_mib = vec![b'x'; 9 * 1024 * 1024];
        assert_eq!(inspect(&nine_mib), refused(ImageRefusal::TooLarge));
        let big_bomb = png_header(30_000, 30_000, 9 * 1024 * 1024);
        assert_eq!(inspect(&big_bomb), refused(ImageRefusal::TooLarge));
        // A pointer is a pointer, though no signature matches it.
        let lfs = pointer(&[LFS_VERSION_LINE, &format!("oid sha256:{OID}"), "size 812"]);
        assert_eq!(inspect(&lfs), refused(ImageRefusal::Lfs));
    }

    #[test]
    fn a_git_lfs_pointer_follows_its_grammar() {
        let oid = format!("oid sha256:{OID}");
        let valid = pointer(&[LFS_VERSION_LINE, &oid, "size 12345"]);
        assert!(is_lfs_pointer(&valid));
        // Without its final newline, and with the extension lines LFS allows.
        assert!(is_lfs_pointer(valid.strip_suffix(b"\n").unwrap()));
        assert!(is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            "ext-0-foo sha256:0000",
            &oid,
            "size 12345",
        ])));

        // Each line is required, and each must be whole.
        assert!(!is_lfs_pointer(&pointer(&[&oid, "size 12345"])));
        assert!(!is_lfs_pointer(&pointer(&[LFS_VERSION_LINE, "size 12345"])));
        assert!(!is_lfs_pointer(&pointer(&[LFS_VERSION_LINE, &oid])));
        assert!(!is_lfs_pointer(&pointer(&[
            "version https://git-lfs.github.com/spec/v2",
            &oid,
            "size 12345",
        ])));
        let short = format!("oid sha256:{}", &OID[..63]);
        assert!(!is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            &short,
            "size 12345"
        ])));
        let not_hex = format!("oid sha256:{}g", &OID[..63]);
        assert!(!is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            &not_hex,
            "size 12345"
        ])));
        assert!(!is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            &oid,
            "size "
        ])));
        assert!(!is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            &oid,
            "size 12a"
        ])));
        assert!(!is_lfs_pointer(&pointer(&[
            LFS_VERSION_LINE,
            &oid,
            "size 1",
            "note"
        ])));
        assert!(!is_lfs_pointer(&[0xff, 0xfe]));

        // Shorter than 1,024 bytes: an `ext-` line pads one to the edge.
        let padded = |len: usize| {
            let mut bytes = valid.clone();
            let ext = format!("ext-0-pad {}\n", "p".repeat(len - valid.len() - 11));
            bytes.extend_from_slice(ext.as_bytes());
            assert_eq!(bytes.len(), len);
            bytes
        };
        assert!(is_lfs_pointer(&padded(1023)));
        assert!(!is_lfs_pointer(&padded(1024)));
    }
}
