//! The pull-request window's title, as the desktop shell may set it
//! (`pull-request-viewer`: *Pull-Request Window Title*; design D3, D10).
//!
//! A pull request's title is written by a stranger, and a titlebar renders
//! what it is given: a right-to-left override would reorder it, and a
//! zero-width or control character would hide in it. The shell sets the
//! title when it builds the window and again from the page's title-change
//! hook, each time through [`sanitize_window_title`], so the page's own
//! `pullRequestTitle` (`src/pullRequestOpen.ts`) is never the only guard.
//!
//! It lives here rather than in the shell so the mutation gate reaches it.

/// The longest title a pull-request window carries, in Unicode scalar
/// values. `src/pullRequestOpen.ts`'s `PULL_REQUEST_TITLE_CAP` is the same
/// number and cuts by code point too, so the title the page sets is the
/// title the titlebar shows.
pub const WINDOW_TITLE_CAP: usize = 200;

/// Unicode's `Default_Ignorable_Code_Point` property, as ranges, from
/// `DerivedCoreProperties.txt` of Unicode 16.0.0: the characters that render
/// as nothing when a font has no glyph for them. `std` exposes no such
/// table. Ascending and disjoint.
const DEFAULT_IGNORABLE: [(char, char); 17] = [
    // SOFT HYPHEN
    ('\u{ad}', '\u{ad}'),
    // COMBINING GRAPHEME JOINER
    ('\u{34f}', '\u{34f}'),
    // ARABIC LETTER MARK
    ('\u{61c}', '\u{61c}'),
    // HANGUL CHOSEONG FILLER..HANGUL JUNGSEONG FILLER
    ('\u{115f}', '\u{1160}'),
    // KHMER VOWEL INHERENT AQ..KHMER VOWEL INHERENT AA
    ('\u{17b4}', '\u{17b5}'),
    // MONGOLIAN FREE VARIATION SELECTOR ONE..FOUR, VOWEL SEPARATOR
    ('\u{180b}', '\u{180f}'),
    // ZERO WIDTH SPACE..RIGHT-TO-LEFT MARK
    ('\u{200b}', '\u{200f}'),
    // LEFT-TO-RIGHT EMBEDDING..RIGHT-TO-LEFT OVERRIDE
    ('\u{202a}', '\u{202e}'),
    // WORD JOINER..NOMINAL DIGIT SHAPES, the isolates among them
    ('\u{2060}', '\u{206f}'),
    // HANGUL FILLER
    ('\u{3164}', '\u{3164}'),
    // VARIATION SELECTOR-1..VARIATION SELECTOR-16
    ('\u{fe00}', '\u{fe0f}'),
    // ZERO WIDTH NO-BREAK SPACE
    ('\u{feff}', '\u{feff}'),
    // HALFWIDTH HANGUL FILLER
    ('\u{ffa0}', '\u{ffa0}'),
    // <reserved-FFF0>..<reserved-FFF8>
    ('\u{fff0}', '\u{fff8}'),
    // SHORTHAND FORMAT LETTER OVERLAP..SHORTHAND FORMAT UP STEP
    ('\u{1bca0}', '\u{1bca3}'),
    // MUSICAL SYMBOL BEGIN BEAM..MUSICAL SYMBOL END PHRASE
    ('\u{1d173}', '\u{1d17a}'),
    // The tags, VARIATION SELECTOR-17..256 and their reserved neighbours
    ('\u{e0000}', '\u{e0fff}'),
];

/// Whether `c` is a Unicode default-ignorable code point.
pub(crate) fn is_default_ignorable(c: char) -> bool {
    DEFAULT_IGNORABLE
        .iter()
        .any(|&(first, last)| first <= c && c <= last)
}

/// `title` as a pull-request window may show it: every default-ignorable
/// character and every control character removed, then cut to
/// [`WINDOW_TITLE_CAP`] scalar values, never inside one.
pub fn sanitize_window_title(title: &str) -> String {
    title
        .chars()
        .filter(|&c| !c.is_control() && !is_default_ignorable(c))
        .take(WINDOW_TITLE_CAP)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pull-request-viewer`: *Hidden and control characters never reach the
    /// title*: a right-to-left override, a zero-width space and a newline.
    #[test]
    fn an_override_a_zero_width_space_and_a_newline_never_reach_the_title() {
        assert_eq!(
            sanitize_window_title("#42 Add\u{202e} rate\u{200b} limits\n — acme/api"),
            "#42 Add rate limits — acme/api"
        );
        let controls: String = ('\u{0}'..='\u{1f}').chain('\u{7f}'..='\u{9f}').collect();
        assert_eq!(sanitize_window_title(&format!("a{controls}b")), "ab");
    }

    /// Each range's first and last character are removed, and the one just
    /// outside it on either side is kept: none of those neighbours is
    /// default-ignorable or a control character.
    #[test]
    fn each_range_is_removed_to_its_edges_and_no_further() {
        let neighbour =
            |c: char, step: i64| char::from_u32((c as i64 + step) as u32).expect("a scalar value");
        for (first, last) in DEFAULT_IGNORABLE {
            for edge in [first, last] {
                assert_eq!(
                    sanitize_window_title(&format!("a{edge}b")),
                    "ab",
                    "{edge:?}"
                );
            }
            for kept in [neighbour(first, -1), neighbour(last, 1)] {
                assert!(!is_default_ignorable(kept), "{kept:?}");
                assert_eq!(
                    sanitize_window_title(&format!("a{kept}b")),
                    format!("a{kept}b"),
                    "{kept:?}"
                );
            }
        }
    }

    /// The table is ascending and disjoint, as `DerivedCoreProperties.txt`
    /// lists it, so no range hides inside another.
    #[test]
    fn the_ranges_are_ascending_and_disjoint() {
        for (first, last) in DEFAULT_IGNORABLE {
            assert!(first <= last, "{first:?}..{last:?}");
        }
        for pair in DEFAULT_IGNORABLE.windows(2) {
            assert!(pair[0].1 < pair[1].0, "{pair:?}");
        }
    }

    /// `pull-request-viewer`: *An overlong title is capped*: several thousand
    /// characters are cut to the cap, counted in scalar values, so a title of
    /// characters outside the Basic Multilingual Plane is never cut inside
    /// one. A title of exactly the cap is kept whole.
    #[test]
    fn an_overlong_title_is_cut_to_the_cap() {
        assert_eq!(WINDOW_TITLE_CAP, 200);
        let long = sanitize_window_title(&"x".repeat(5_000));
        assert_eq!(long, "x".repeat(WINDOW_TITLE_CAP));
        let wide = sanitize_window_title(&"\u{1f600}".repeat(3_000));
        assert_eq!(wide.chars().count(), WINDOW_TITLE_CAP);
        assert!(wide.chars().all(|c| c == '\u{1f600}'));
        let exact = "y".repeat(WINDOW_TITLE_CAP);
        assert_eq!(sanitize_window_title(&exact), exact);
    }

    /// The cap counts what is kept: removed characters spend none of it.
    #[test]
    fn removed_characters_spend_none_of_the_cap() {
        let padded = format!("{}{}", "\u{200b}".repeat(500), "z".repeat(300));
        assert_eq!(sanitize_window_title(&padded), "z".repeat(WINDOW_TITLE_CAP));
    }
}
