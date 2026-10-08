//! What one version of a changed image file is, decided from its bytes alone
//! (`diff-view`: *Image Comparison*). Pure, with no I/O: the commit read and
//! the pull-request image read both hand it bytes, and only a version it
//! answers `Image` for ever reaches a web view's decoder.
//!
//! A version is an image only when its bytes begin with one of seven
//! signatures, never because of its name. Its width and height come from its
//! header alone, so a few kilobytes declaring a huge canvas are refused
//! before anything decodes them. Every other header a decoder may size its
//! canvas by is counted too, so a file cannot declare a small image to this
//! check and a huge one to the web view.

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

/// The eight bytes every PNG begins with.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

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
    let Some(canvases) = decoder_canvases(kind, bytes) else {
        return ImageCheck::Refused(ImageRefusal::NotImage);
    };
    let declared = (size.width as u64, size.height as u64);
    if std::iter::once(declared)
        .chain(canvases)
        .any(|(width, height)| {
            u128::from(width) * u128::from(height) > u128::from(IMAGE_PIXELS_LIMIT)
        })
    {
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
    if bytes.starts_with(PNG_SIGNATURE) {
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

/// Every canvas a web view's decoder may size a version of `kind` by that
/// `imagesize` does not read, which a crafted file can make disagree with the
/// one it does: a GIF's screen grown to hold its frames, each of an icon's
/// images by its own header, and each frame header a JPEG decoder finds.
/// `None` when the bytes are not laid out as their signature says: a PNG, or
/// a PNG inside an icon, whose first chunk is not `IHDR`, which `imagesize`
/// would measure all the same.
fn decoder_canvases(kind: ImageType, bytes: &[u8]) -> Option<Vec<(u64, u64)>> {
    match kind {
        ImageType::Png => png_size(bytes).map(|size| vec![size]),
        ImageType::Gif => Some(vec![gif_canvas(bytes)]),
        ImageType::Ico => icon_images(bytes),
        ImageType::Jpeg => Some(jpeg_frames(bytes)),
        _ => Some(Vec::new()),
    }
}

/// The `N` bytes at `at`, when there are that many.
fn field<const N: usize>(bytes: &[u8], at: usize) -> Option<[u8; N]> {
    bytes.get(at..at.checked_add(N)?)?.try_into().ok()
}

/// A PNG's width and height, from its `IHDR`, which must be its first chunk
/// as a decoder requires. Apple's `CgBI` PNGs, which put a chunk before it,
/// are not images here.
fn png_size(png: &[u8]) -> Option<(u64, u64)> {
    if png.get(12..16) != Some(&b"IHDR"[..]) {
        return None;
    }
    Some((
        u32::from_be_bytes(field(png, 16)?).into(),
        u32::from_be_bytes(field(png, 20)?).into(),
    ))
}

/// A GIF's canvas as a browser sizes it: its logical screen, grown to hold
/// each frame at its offset. A stray byte between blocks is read past, as a
/// GIF87a decoder may, and the walk ends at the trailer or where the bytes
/// run out; a frame whose descriptor is whole counts though its data is cut
/// short.
fn gif_canvas(gif: &[u8]) -> (u64, u64) {
    let word = |at: usize| field(gif, at).map_or(0, |pair| u64::from(u16::from_le_bytes(pair)));
    // A colour table's length, from the flags that say whether there is one.
    let table = |flags: u8| {
        if flags & 0x80 == 0 {
            0
        } else {
            3usize << ((flags & 7) + 1)
        }
    };
    let mut canvas = (word(6), word(8));
    let mut at = 13 + gif.get(10).map_or(0, |&flags| table(flags));
    // Each block moves the walk on by a byte at least, so it takes no more
    // steps than there are bytes.
    for _ in 0..gif.len() {
        let Some(&block) = gif.get(at) else {
            break;
        };
        let blocks = match block {
            // An extension: its label, then its sub-blocks.
            0x21 => at + 2,
            // A frame: its descriptor, its own colour table, its LZW code
            // size, then its sub-blocks.
            0x2C => {
                let Some(&flags) = gif.get(at + 9) else {
                    break;
                };
                canvas.0 = canvas.0.max(word(at + 1) + word(at + 5));
                canvas.1 = canvas.1.max(word(at + 3) + word(at + 7));
                at + 10 + table(flags) + 1
            }
            0x3B => break,
            _ => {
                at += 1;
                continue;
            }
        };
        let Some(next) = past_sub_blocks(gif, blocks) else {
            break;
        };
        at = next;
    }
    canvas
}

/// Where the GIF sub-blocks that start at `at` end, past their empty
/// terminator; `None` when the bytes run out first.
fn past_sub_blocks(gif: &[u8], mut at: usize) -> Option<usize> {
    // Each sub-block is a byte at least.
    for _ in 0..gif.len() {
        let len = usize::from(*gif.get(at)?);
        at += 1 + len;
        if len == 0 {
            return Some(at);
        }
    }
    None
}

/// Each of an icon's images, sized by its own header as a decoder sizes it
/// rather than by the directory's one-byte sizes: an embedded PNG by its
/// `IHDR`, a bitmap by its info header, whose height also counts its mask.
/// An entry whose image lies past the bytes is skipped, as a decoder fails
/// it. `None` when an embedded PNG's first chunk is not `IHDR`.
fn icon_images(icon: &[u8]) -> Option<Vec<(u64, u64)>> {
    let count = field(icon, 4).map_or(0, u16::from_le_bytes);
    let mut images = Vec::new();
    for entry in 0..usize::from(count) {
        let Some(offset) = field(icon, 6 + 16 * entry + 12).map(u32::from_le_bytes) else {
            break;
        };
        let Some(image) = icon.get(offset as usize..) else {
            continue;
        };
        if image.starts_with(PNG_SIGNATURE) {
            images.push(png_size(image)?);
        } else if let Some(size) = bitmap_size(image) {
            images.push(size);
        }
    }
    Some(images)
}

/// A bitmap's width and height from its info header: two 16-bit fields in
/// the 12-byte core header, two signed 32-bit fields in every later one,
/// whose height is negative for a top-down bitmap.
fn bitmap_size(dib: &[u8]) -> Option<(u64, u64)> {
    if u32::from_le_bytes(field(dib, 0)?) == 12 {
        return Some((
            u16::from_le_bytes(field(dib, 4)?).into(),
            u16::from_le_bytes(field(dib, 6)?).into(),
        ));
    }
    Some((
        i32::from_le_bytes(field(dib, 4)?).unsigned_abs().into(),
        i32::from_le_bytes(field(dib, 8)?).unsigned_abs().into(),
    ))
}

/// The width and height of each frame header a JPEG decoder may size the
/// image by, found as libjpeg finds markers: past any bytes that are not
/// `0xFF` and any run of `0xFF` fill bytes, which `imagesize` does not skip.
/// A segment is skipped by its length and a standalone marker alone. The
/// walk ends at the start of the scan, at the end of the image, at a second
/// start of image, or where the bytes run out.
fn jpeg_frames(jpeg: &[u8]) -> Vec<(u64, u64)> {
    let mut frames = Vec::new();
    let mut at = 2;
    // Each marker moves the walk on by a byte at least, so it takes no more
    // steps than there are bytes.
    for _ in 0..jpeg.len() {
        let Some((marker, after)) = next_marker(jpeg, at) else {
            break;
        };
        at = after;
        match marker {
            // A second start of image, the end of image, the start of scan.
            0xD8..=0xDA => break,
            // `TEM` and `RST0` to `RST7`, which stand alone.
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let Some(length) = field(jpeg, at).map(u16::from_be_bytes) else {
            break;
        };
        if is_frame_marker(marker) {
            if let (Some(height), Some(width)) = (field(jpeg, at + 3), field(jpeg, at + 5)) {
                frames.push((
                    u16::from_be_bytes(width).into(),
                    u16::from_be_bytes(height).into(),
                ));
            }
        }
        at += usize::from(length);
    }
    frames
}

/// The marker libjpeg reads next from `at`, past any bytes that are not
/// `0xFF` and then past the run of `0xFF` fill bytes, and where the bytes
/// after it start.
fn next_marker(jpeg: &[u8], at: usize) -> Option<(u8, usize)> {
    let rest = jpeg.get(at..)?;
    let fill = rest.iter().position(|&byte| byte == 0xFF)?;
    let marker = fill + rest[fill..].iter().position(|&byte| byte != 0xFF)?;
    Some((rest[marker], at + marker + 1))
}

/// Whether `marker` starts a frame: `SOF0` to `SOF15`, less `DHT`, `JPG` and
/// `DAC`, which share their range.
fn is_frame_marker(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
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

    // ------------------------------------------------- what a decoder sizes

    /// The byte that starts a GIF frame, filling every colour table and every
    /// frame's data below, so a walk that loses its place finds a frame there.
    const FRAME: u8 = 0x2C;

    /// A GIF89a of a `width` × `height` screen, with the global colour table
    /// `flags` call for, then `body`, then the trailer.
    fn gif((width, height): (u16, u16), flags: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend(width.to_le_bytes());
        bytes.extend(height.to_le_bytes());
        bytes.extend([flags, 0, 0]);
        bytes.extend(colour_table(flags));
        bytes.extend(body);
        bytes.push(0x3B);
        bytes
    }

    fn colour_table(flags: u8) -> Vec<u8> {
        let len = if flags & 0x80 == 0 {
            0
        } else {
            3 << ((flags & 7) + 1)
        };
        vec![FRAME; len]
    }

    /// One frame of a GIF: its descriptor, the local colour table its `flags`
    /// call for, and one sub-block of data.
    fn frame(left: u16, top: u16, width: u16, height: u16, flags: u8) -> Vec<u8> {
        let mut bytes = vec![FRAME];
        for field in [left, top, width, height] {
            bytes.extend(field.to_le_bytes());
        }
        bytes.push(flags);
        bytes.extend(colour_table(flags));
        bytes.extend([2, 2, FRAME, FRAME, 0]);
        bytes
    }

    /// A graphic control extension, its fields all the frame byte.
    const GRAPHIC_CONTROL: &[u8] = &[0x21, 0xF9, 4, FRAME, FRAME, FRAME, FRAME, 0];

    /// A browser grows a GIF's screen to hold its frames, so each frame's
    /// offset and size count, past any colour table and extension, and a
    /// screen and a frame together can pass the ceiling neither passes alone.
    #[test]
    fn a_gif_is_measured_by_its_screen_grown_to_its_frames() {
        assert_eq!(gif_canvas(GIF87A), (3, 2));
        assert_eq!(gif_canvas(GIF89A), (3, 2));

        // A 3 × 2 screen whose frame declares 30,000 × 30,000.
        let bomb = gif((3, 2), 0, &frame(0, 0, 30_000, 30_000, 0));
        assert_eq!(imagesize_reads(ImageType::Gif, &bomb), Some((3, 2)));
        assert_eq!(gif_canvas(&bomb), (30_000, 30_000));
        assert_eq!(inspect(&bomb), refused(ImageRefusal::TooManyPixels));

        // A frame's offset counts with its size.
        let offset = gif((1, 1_000), 0, &frame(39_999, 1_000, 1, 1, 0));
        assert_eq!(gif_canvas(&offset), (40_000, 1_001));
        assert_eq!(inspect(&offset), refused(ImageRefusal::TooManyPixels));

        // A wide screen and a tall frame make a canvas neither declares.
        let together = gif((40_000, 1), 0, &frame(0, 0, 1, 40_000, 0));
        assert_eq!(gif_canvas(&together), (40_000, 40_000));
        assert_eq!(inspect(&together), refused(ImageRefusal::TooManyPixels));

        // Past a global table of 4 colours, an extension, and a first frame
        // with a local table of 8, the second frame still counts.
        let body = [
            GRAPHIC_CONTROL,
            &frame(0, 0, 3, 2, 0x82),
            &frame(1, 2, 8_999, 4_998, 0),
        ]
        .concat();
        let tables = gif((3, 2), 0x81, &body);
        assert_eq!(gif_canvas(&tables), (9_000, 5_000));
        assert_eq!(inspect(&tables), refused(ImageRefusal::TooManyPixels));
    }

    /// A whole descriptor counts though its data is cut short, and a stray
    /// byte is read past; the trailer, or a descriptor cut short, ends it.
    #[test]
    fn a_gif_walk_ends_at_its_trailer_or_its_last_byte() {
        let descriptor = &frame(0, 0, 30_000, 30_000, 0)[..10];
        let screen = &gif((3, 2), 0, &[])[..13];
        assert_eq!(gif_canvas(&[screen, descriptor].concat()), (30_000, 30_000));
        assert_eq!(gif_canvas(&[screen, &descriptor[..9]].concat()), (3, 2));

        let bomb = frame(0, 0, 30_000, 30_000, 0);
        let stray = gif((3, 2), 0, &[&[0x00][..], &bomb].concat());
        assert_eq!(gif_canvas(&stray), (30_000, 30_000));
        let trailed = gif((3, 2), 0, &[&[0x3B][..], &bomb].concat());
        assert_eq!(gif_canvas(&trailed), (3, 2));
        let unterminated = gif((3, 2), 0, &[&[0x21, 0xFE, 4, FRAME][..], &bomb].concat());
        assert_eq!(gif_canvas(&unterminated[..unterminated.len() - 1]), (3, 2));
    }

    /// What an extension or a frame's data holds is skipped whole, however
    /// many sub-blocks it takes, though one holds the bytes of a frame; and
    /// an extension right after the screen leads on to the frame after it.
    #[test]
    fn a_gif_walk_skips_what_its_blocks_hold() {
        let held = &frame(0, 0, 30_000, 30_000, 0)[..10];
        let comment = [&[0x21, 0xFE, 1, 0, 10][..], held, &[0]].concat();
        assert_eq!(gif_canvas(&gif((3, 2), 0, &comment)), (3, 2));
        let data = [&frame(0, 0, 3, 2, 0)[..10], &[2, 1, 0, 10], held, &[0]].concat();
        assert_eq!(gif_canvas(&gif((3, 2), 0, &data)), (3, 2));

        let after = [GRAPHIC_CONTROL, &frame(0, 0, 30_000, 30_000, 0)].concat();
        assert_eq!(gif_canvas(&gif((3, 2), 0, &after)), (30_000, 30_000));
    }

    /// An icon whose directory says 16 × 16 for each of `images`, which
    /// follow it in order.
    fn icon(images: &[&[u8]]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 1, 0];
        bytes.extend(u16::try_from(images.len()).unwrap().to_le_bytes());
        let mut offset = 6 + 16 * images.len();
        for image in images {
            bytes.extend([16, 16, 0, 0, 1, 0, 32, 0]);
            bytes.extend(u32::try_from(image.len()).unwrap().to_le_bytes());
            bytes.extend(u32::try_from(offset).unwrap().to_le_bytes());
            offset += image.len();
        }
        for image in images {
            bytes.extend_from_slice(image);
        }
        bytes
    }

    /// A bitmap info header of `header` bytes: the 12-byte core header's
    /// 16-bit width and height, or a later header's signed 32-bit ones.
    fn dib(header: u32, width: i32, height: i32) -> Vec<u8> {
        let mut bytes = header.to_le_bytes().to_vec();
        if header == 12 {
            bytes.extend(u16::try_from(width).unwrap().to_le_bytes());
            bytes.extend(u16::try_from(height).unwrap().to_le_bytes());
        } else {
            bytes.extend(width.to_le_bytes());
            bytes.extend(height.to_le_bytes());
        }
        bytes.extend([1, 0, 32, 0]);
        bytes.resize(header as usize, 0);
        bytes
    }

    /// A decoder sizes an icon's image by the image's own header, not by the
    /// directory's one-byte sizes, so each embedded PNG and bitmap counts.
    #[test]
    fn an_icon_is_measured_by_each_image_it_holds() {
        assert_eq!(icon_images(ICO), Some(vec![(16, 16), (32, 32), (256, 256)]));

        let bomb = icon(&[&png_header(3, 2, 33), &png_header(30_000, 30_000, 33)]);
        assert_eq!(imagesize_reads(ImageType::Ico, &bomb), Some((16, 16)));
        assert_eq!(icon_images(&bomb), Some(vec![(3, 2), (30_000, 30_000)]));
        assert_eq!(inspect(&bomb), refused(ImageRefusal::TooManyPixels));

        // An info header, a top-down one, a core header and a later one.
        let bitmaps = icon(&[
            &dib(40, 3, 4),
            &dib(40, 9_000, -10_000),
            &dib(12, 7, 8),
            &dib(124, -5, 6),
        ]);
        assert_eq!(
            icon_images(&bitmaps),
            Some(vec![(3, 4), (9_000, 10_000), (7, 8), (5, 6)])
        );
        assert_eq!(inspect(&bitmaps), refused(ImageRefusal::TooManyPixels));
        let core = icon(&[&dib(12, 9_000, 9_000)]);
        assert_eq!(inspect(&core), refused(ImageRefusal::TooManyPixels));

        // An entry whose image lies past the bytes, or is cut short of its
        // header, is skipped.
        let mut past = icon(&[&png_header(30_000, 30_000, 33), &dib(40, 3, 4)]);
        past[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(icon_images(&past), Some(vec![(3, 4)]));
        assert_eq!(inspect(&past), image("image/x-icon", 16, 16));
        let cut = icon(&[&dib(40, 3, 4), &dib(40, 30_000, 30_000)[..8]]);
        assert_eq!(icon_images(&cut), Some(vec![(3, 4)]));
    }

    /// A PNG must open with its `IHDR`, as a decoder requires, alone or
    /// inside an icon: Apple's `CgBI` chunk before it would hand `imagesize`
    /// that chunk's bytes as a size.
    #[test]
    fn a_png_must_open_with_its_header_chunk() {
        let mut cgbi = b"\x89PNG\r\n\x1a\n\0\0\0\x04CgBI\0\0\0\x03\0\0\0\x02".to_vec();
        cgbi.extend(&png_header(30_000, 30_000, 33)[8..]);
        assert_eq!(imagesize_reads(ImageType::Png, &cgbi), Some((3, 2)));
        assert_eq!(png_size(&cgbi), None);
        assert_eq!(inspect(&cgbi), refused(ImageRefusal::NotImage));
        assert_eq!(png_size(PNG), Some((3, 2)));
        assert_eq!(png_size(&png_header(30_000, 1, 33)), Some((30_000, 1)));

        let inside = icon(&[&png_header(3, 2, 33), &cgbi]);
        assert_eq!(icon_images(&inside), None);
        assert_eq!(inspect(&inside), refused(ImageRefusal::NotImage));
    }

    /// A JPEG frame header (`marker`) declaring `width` × `height`, of three
    /// components.
    fn sof(marker: u8, width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, marker, 0, 17, 8];
        bytes.extend(height.to_be_bytes());
        bytes.extend(width.to_be_bytes());
        bytes.extend([3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        bytes
    }

    /// What `imagesize` alone reads of `bytes` as `kind`.
    fn imagesize_reads(kind: ImageType, bytes: &[u8]) -> Option<(usize, usize)> {
        let size = kind.reader_size(&mut Cursor::new(bytes)).ok()?;
        Some((size.width, size.height))
    }

    /// `head`, a real frame header declaring 30,000 × 30,000, then at byte
    /// `decoy` a frame header declaring 3 × 2.
    fn decoyed(head: &[u8], decoy: usize) -> Vec<u8> {
        let mut bytes = [head, &sof(0xC0, 30_000, 30_000)].concat();
        bytes.resize(decoy, 0);
        bytes.extend(sof(0xC0, 3, 2));
        bytes
    }

    /// libjpeg reads past fill bytes, a standalone marker and bytes that are
    /// not `0xFF`, where `imagesize` takes the bytes after a marker for its
    /// length and lands on a decoy; each frame header either finds counts.
    #[test]
    fn a_jpeg_counts_each_frame_header_a_decoder_finds() {
        assert_eq!(jpeg_frames(JPEG), vec![(3, 2)]);

        // `imagesize` takes the fill byte for a marker, and `TEM` with the
        // byte after it for a length of 0x100.
        let filled = decoyed(&[0xFF, 0xD8, 0xFF, 0xFF, 0x01, 0x00], 260);
        assert_eq!(imagesize_reads(ImageType::Jpeg, &filled), Some((3, 2)));
        assert_eq!(jpeg_frames(&filled), vec![(30_000, 30_000), (3, 2)]);
        assert_eq!(inspect(&filled), refused(ImageRefusal::TooManyPixels));

        // `TEM` and `RST0` to `RST7` stand alone, where `imagesize` reads
        // the two bytes after one as a length of 0x20.
        for standalone in [0x01, 0xD0, 0xD7] {
            let alone = decoyed(&[0xFF, 0xD8, 0xFF, standalone, 0x00, 0x20], 36);
            assert_eq!(imagesize_reads(ImageType::Jpeg, &alone), Some((3, 2)));
            assert_eq!(
                jpeg_frames(&alone),
                vec![(30_000, 30_000), (3, 2)],
                "{standalone:#x}"
            );
            assert_eq!(inspect(&alone), refused(ImageRefusal::TooManyPixels));
        }

        // Bytes that are not `0xFF` are read past.
        let junk = [&b"\xFF\xD8junk"[..], &sof(0xC2, 5, 4)].concat();
        assert_eq!(jpeg_frames(&junk), vec![(5, 4)]);
    }

    /// Every `SOFn` is a frame header and `DHT`, `JPG` and `DAC` are not; the
    /// walk ends at the start of the scan, the end of the image, a second
    /// start of image, or a frame header cut short.
    #[test]
    fn a_jpeg_walk_reads_frame_headers_up_to_its_scan() {
        let after = |segment: &[u8]| [&[0xFF, 0xD8][..], segment, &sof(0xC1, 5, 4)].concat();
        for marker in [
            0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
        ] {
            assert_eq!(
                jpeg_frames(&after(&sof(marker, 30_000, 30_000))),
                vec![(30_000, 30_000), (5, 4)],
                "{marker:#x}"
            );
        }
        for marker in [0xC4, 0xC8, 0xCC] {
            assert_eq!(
                jpeg_frames(&after(&sof(marker, 30_000, 30_000))),
                vec![(5, 4)],
                "{marker:#x}"
            );
        }
        for end in [0xD8, 0xD9, 0xDA] {
            assert_eq!(
                jpeg_frames(&after(&[0xFF, end, 0, 4, 0, 0])),
                vec![],
                "{end:#x}"
            );
        }
        assert_eq!(jpeg_frames(&after(&[]).as_slice()[..10]), vec![]);
    }
}
