//! The review skip patterns (`pull-request-viewer`: *Review Skip Patterns*;
//! design D4 to D7 of `review-skip-patterns`): one global list naming the
//! files of a pull request the reader does not intend to review, tests above
//! all. It is empty until the reader stores one, so nothing is skipped
//! before then, and no list ships with SpecForge (D6).
//!
//! A pattern is a glob, or a regular expression written between slashes:
//!
//! matches(p, q) = regex(p) finds a match in q, when p is `/r/` of at least
//! three characters; else glob(p) matches q, when p holds a `/`; else glob(p)
//! matches basename(q).
//!
//! Globs compile with `literal_separator`, so `*` and `?` never cross a `/`,
//! while `**` matches any number of whole segments, none included. A regular
//! expression is in the `regex` crate's own syntax, which matches in linear
//! time and bounds its compile. Matching is case-sensitive. A file is matched
//! by its new path and, renamed or deleted, by its old path too
//! ([`match_paths`]), and is named by the first pattern of the list that
//! matches it ([`SkipRule::first_match`]).
//!
//! [`validate`] is the setter's rule. [`SkipRule::compile`] is the reader's: it
//! applies the same rule to each pattern on its own and leaves out the ones a
//! list would not be accepted with, so a hand-edited settings file degrades
//! rather than failing. A rule of at most 64 short patterns compiles in
//! microseconds, so it is compiled again for every progress read and every
//! write, and nothing caches it (D5).

use globset::{GlobBuilder, GlobMatcher};
use openspec_core::{DiffFile, FileStatus};
use regex::Regex;
use serde::Serialize;

use crate::events::ReviewSkipPatternsChangedPayload;

/// The most patterns a list holds.
pub const MAX_PATTERNS: usize = 64;

/// The longest a pattern is once its surrounding whitespace is removed, in
/// bytes.
pub const MAX_PATTERN_BYTES: usize = 256;

// ---- the wire ----
//
// Camel case on the wire and hand-mirrored in `src/types.ts`;
// `tests/wire_shape.rs` pins the keys.

/// A pattern a list would not be accepted with, and why: one that
/// `set_review_skip_patterns` refuses, or one a hand-edited settings file
/// holds, which matching leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternError {
    /// Its place in the list, counted from zero.
    pub index: usize,
    /// The pattern as the list holds it.
    pub pattern: String,
    /// Why, in the words the Settings view shows on its field.
    pub reason: String,
}

/// What `get_review_skip_patterns` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSkipPatterns {
    /// The stored list, empty until the reader stores one.
    pub patterns: Vec<String>,
    /// Each pattern of the stored list that a list would not be accepted
    /// with, and why. Only a hand-edited settings file holds one.
    pub errors: Vec<PatternError>,
}

/// What `set_review_skip_patterns` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SkipPatternsOutcome {
    /// The list was stored, the empty one included: the list now stored,
    /// which the command also emits as `review-skip-patterns-changed`.
    Stored(ReviewSkipPatternsChangedPayload),
    /// The list was not accepted, so nothing was stored: every refused
    /// pattern, and why.
    Refused { refused: Vec<PatternError> },
}

// ---- matching ----

/// How one accepted pattern matches a path.
#[derive(Debug, Clone)]
enum Matcher {
    /// `/…/`: searched anywhere in the path.
    Regex(Regex),
    /// A glob holding a `/`: the whole path, from the repository root.
    Path(GlobMatcher),
    /// A glob without one: the path's last segment, the file's name.
    Name(GlobMatcher),
}

impl Matcher {
    fn matches(&self, path: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(path),
            Self::Path(glob) => glob.is_match(path),
            Self::Name(glob) => glob.is_match(path.rsplit('/').next().unwrap_or(path)),
        }
    }
}

/// The body of a pattern written as a regular expression: one of at least
/// three characters that starts and ends with `/`. `None` for a glob, which
/// `//` therefore is. A string that starts and ends with `/` is at least
/// three characters long exactly when it is at least three bytes long.
fn regex_body(pattern: &str) -> Option<&str> {
    if pattern.len() < 3 {
        return None;
    }
    pattern.strip_prefix('/')?.strip_suffix('/')
}

/// A regular expression's compile error on one line: the `regex` crate's own
/// last line (`unclosed group`), without the caret diagram above it.
fn regex_reason(error: &regex::Error) -> String {
    let message = error.to_string();
    let last = message.lines().last().unwrap_or_default();
    last.strip_prefix("error: ").unwrap_or(last).to_string()
}

/// `written`, the list's `index`th pattern counted from zero, compiled once
/// its surrounding whitespace is removed; or why a list would not be accepted
/// with it: past the 64th, empty, longer than 256 bytes, not compiling, or a
/// glob starting with `/`, since paths are relative to the repository root.
fn compile_one(index: usize, written: &str) -> Result<Matcher, String> {
    if index >= MAX_PATTERNS {
        return Err(format!("a list holds at most {MAX_PATTERNS} patterns"));
    }
    let pattern = written.trim();
    if pattern.is_empty() {
        return Err("the pattern is empty".to_string());
    }
    if pattern.len() > MAX_PATTERN_BYTES {
        return Err(format!(
            "a pattern is at most {MAX_PATTERN_BYTES} bytes long"
        ));
    }
    if let Some(body) = regex_body(pattern) {
        return Regex::new(body).map(Matcher::Regex).map_err(|error| {
            format!(
                "the regular expression does not compile: {}",
                regex_reason(&error)
            )
        });
    }
    if pattern.starts_with('/') {
        return Err(
            "a glob cannot start with a slash, since paths are relative to the repository root"
                .to_string(),
        );
    }
    // Backslash escapes are named rather than left to globset's default,
    // which differs on Windows, so a list matches alike on every platform.
    let glob = GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .map_err(|error| format!("the glob does not compile: {}", error.kind()))?
        .compile_matcher();
    Ok(if pattern.contains('/') {
        Matcher::Path(glob)
    } else {
        Matcher::Name(glob)
    })
}

/// The compiled skip patterns, in list order.
#[derive(Debug, Clone, Default)]
pub struct SkipRule {
    /// Each accepted pattern as the list holds it, beside its matcher.
    rules: Vec<(String, Matcher)>,
}

impl SkipRule {
    /// Compiles `patterns`, each on its own: the rule of every pattern a list
    /// would be accepted with, and why each of the others would not be, so a
    /// pattern a hand-edited settings file holds leaves the rest working.
    pub fn compile(patterns: &[String]) -> (Self, Vec<PatternError>) {
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for (index, pattern) in patterns.iter().enumerate() {
            match compile_one(index, pattern) {
                Ok(matcher) => rules.push((pattern.clone(), matcher)),
                Err(reason) => errors.push(PatternError {
                    index,
                    pattern: pattern.clone(),
                    reason,
                }),
            }
        }
        (Self { rules }, errors)
    }

    /// The first pattern of the list that matches any of `paths`, as the list
    /// holds it:
    ///
    /// match(f) = the first p ∈ P such that ∃ q ∈ paths(f): matches(p, q)
    pub fn first_match(&self, paths: &[&str]) -> Option<&str> {
        self.rules
            .iter()
            .find(|(_, matcher)| paths.iter().any(|path| matcher.matches(path)))
            .map(|(pattern, _)| pattern.as_str())
    }
}

/// The list as it is stored, each pattern without its surrounding
/// whitespace, when a list is accepted with every one of them; else every
/// pattern it is not accepted with, and why:
///
/// accepted(P) ⇔ |P| ≤ 64 ∧ ∀ p ∈ P: 1 ≤ |trim(p)| ≤ 256 ∧ compiles(p), and no
/// glob starts with `/`.
///
/// The empty list is accepted, and skips nothing.
pub fn validate(patterns: &[String]) -> Result<Vec<String>, Vec<PatternError>> {
    let (_, errors) = SkipRule::compile(patterns);
    if errors.is_empty() {
        Ok(patterns
            .iter()
            .map(|pattern| pattern.trim().to_string())
            .collect())
    } else {
        Err(errors)
    }
}

/// The paths `file` is matched by: its new path and, when it was renamed or
/// deleted, its old path too, so moving a test file does not bring it into
/// the review. A copied file is matched by its new path alone.
pub(crate) fn match_paths(file: &DiffFile) -> Vec<&str> {
    let moved = matches!(
        file.status,
        FileStatus::Renamed { .. } | FileStatus::Deleted
    );
    file.new_path
        .iter()
        .chain(file.old_path.iter().filter(|_| moved))
        .map(String::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openspec_core::DiffContent;

    fn list(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|pattern| pattern.to_string()).collect()
    }

    /// The rule of `patterns`, every one of which must compile.
    fn rule(patterns: &[&str]) -> SkipRule {
        let (rule, errors) = SkipRule::compile(&list(patterns));
        assert_eq!(errors, [], "{patterns:?}");
        rule
    }

    /// The pattern of `patterns` that names `path`.
    fn named<'a>(patterns: &'a SkipRule, path: &str) -> Option<&'a str> {
        patterns.first_match(&[path])
    }

    /// Why `pattern` alone is refused.
    fn refusal(pattern: &str) -> String {
        let refused = validate(&list(&[pattern])).expect_err(pattern);
        assert_eq!(refused.len(), 1, "{pattern}");
        assert_eq!(
            (refused[0].index, refused[0].pattern.as_str()),
            (0, pattern)
        );
        refused[0].reason.clone()
    }

    fn file(old: Option<&str>, new: Option<&str>, status: FileStatus) -> DiffFile {
        DiffFile {
            old_path: old.map(str::to_string),
            new_path: new.map(str::to_string),
            old_mode: None,
            new_mode: None,
            status,
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks { hunks: Vec::new() },
        }
    }

    // ------------------------------------------------ the spec's scenarios

    /// *A glob without a slash matches the file's name anywhere*.
    #[test]
    fn a_glob_without_a_slash_matches_the_files_name_anywhere() {
        let go = rule(&["*_test.go"]);
        assert_eq!(named(&go, "pkg/api/server_test.go"), Some("*_test.go"));
        assert_eq!(named(&go, "server_test.go"), Some("*_test.go"));
        assert_eq!(named(&go, "pkg/server_test.go/main.go"), None, "a name");
        // A whole name too, wherever it is.
        let name = rule(&["parse.rs"]);
        assert_eq!(named(&name, "crates/core/tests/parse.rs"), Some("parse.rs"));
    }

    /// *A glob with a slash matches from the root*.
    #[test]
    fn a_glob_with_a_slash_matches_the_whole_path_from_the_root() {
        let tests = rule(&["tests/**"]);
        assert_eq!(named(&tests, "tests/parse.rs"), Some("tests/**"));
        assert_eq!(named(&tests, "crates/core/tests/parse.rs"), None);
        let whole = rule(&["core/tests/parse.rs"]);
        assert_eq!(named(&whole, "crates/core/tests/parse.rs"), None);
        assert_eq!(
            named(&whole, "core/tests/parse.rs"),
            Some("core/tests/parse.rs")
        );
    }

    /// *A star stays within its segment*, and so does `?`.
    #[test]
    fn a_star_stays_within_its_segment() {
        let star = rule(&["src/*.test.ts"]);
        assert_eq!(named(&star, "src/ui/button.test.ts"), None);
        assert_eq!(named(&star, "src/button.test.ts"), Some("src/*.test.ts"));
        let one = rule(&["src?a/x.rs"]);
        assert_eq!(named(&one, "src/a/x.rs"), None);
        assert_eq!(named(&one, "srcza/x.rs"), Some("src?a/x.rs"));
    }

    /// `**` matches any number of whole segments, none included.
    #[test]
    fn a_double_star_matches_any_number_of_segments_none_included() {
        let deep = rule(&["src/**/a.rs"]);
        for path in ["src/a.rs", "src/x/a.rs", "src/x/y/a.rs"] {
            assert_eq!(named(&deep, path), Some("src/**/a.rs"), "{path}");
        }
        assert_eq!(named(&deep, "src/xa.rs"), None);
        let tests = rule(&["**/tests/**"]);
        for path in ["tests/parse.rs", "crates/core/tests/parse.rs"] {
            assert_eq!(named(&tests, path), Some("**/tests/**"), "{path}");
        }
        assert_eq!(named(&tests, "crates/core/contests/parse.rs"), None);
    }

    /// *A regular expression is searched in the path*, named as written.
    #[test]
    fn a_regular_expression_is_searched_in_the_path() {
        let fixtures = rule(&["/(^|/)fixtures?//"]);
        assert_eq!(
            named(&fixtures, "spec/fixtures/user.json"),
            Some("/(^|/)fixtures?//")
        );
        assert_eq!(
            named(&fixtures, "fixture/user.json"),
            Some("/(^|/)fixtures?//")
        );
        assert_eq!(named(&fixtures, "spec/myfixtures/user.json"), None);
        // Unanchored, unless `^` anchors it at the repository root.
        let anywhere = rule(&["/generated/"]);
        assert_eq!(named(&anywhere, "src/generated.rs"), Some("/generated/"));
        let rooted = rule(&["/^docs//"]);
        assert_eq!(named(&rooted, "docs/guide.md"), Some("/^docs//"));
        assert_eq!(named(&rooted, "site/docs/guide.md"), None);
    }

    /// *The first matching pattern is named*: in list order, whatever the
    /// order the patterns would match in.
    #[test]
    fn the_first_matching_pattern_of_the_list_is_named() {
        let path = "crates/core/tests/parse.rs";
        assert_eq!(
            named(&rule(&["**/tests/**", "*.rs"]), path),
            Some("**/tests/**")
        );
        assert_eq!(named(&rule(&["*.rs", "**/tests/**"]), path), Some("*.rs"));
        assert_eq!(
            named(&rule(&["*.md", "**/tests/**", "*.rs"]), path),
            Some("**/tests/**")
        );
        // A later path of the file does not outrank an earlier pattern.
        assert_eq!(
            rule(&["old/**", "new/**"]).first_match(&["new/a.rs", "old/a.rs"]),
            Some("old/**")
        );
        assert_eq!(rule(&["*.md"]).first_match(&[path]), None);
        assert_eq!(rule(&[]).first_match(&[path]), None);
    }

    /// *A renamed test file is still matched*, by its old path, and a deleted
    /// one by the only path it has; a copied one by its new path alone.
    #[test]
    fn a_file_is_matched_by_its_new_path_and_a_moved_ones_old_path() {
        let tests = rule(&["**/tests/**"]);
        let renamed = file(
            Some("tests/parse.rs"),
            Some("checks/parse.rs"),
            FileStatus::Renamed { similarity: None },
        );
        assert_eq!(match_paths(&renamed), ["checks/parse.rs", "tests/parse.rs"]);
        assert_eq!(
            tests.first_match(&match_paths(&renamed)),
            Some("**/tests/**")
        );
        let deleted = file(Some("tests/parse.rs"), None, FileStatus::Deleted);
        assert_eq!(match_paths(&deleted), ["tests/parse.rs"]);
        assert_eq!(
            tests.first_match(&match_paths(&deleted)),
            Some("**/tests/**")
        );
        let copied = file(
            Some("tests/fixture.rs"),
            Some("src/fixture.rs"),
            FileStatus::Copied { similarity: None },
        );
        assert_eq!(match_paths(&copied), ["src/fixture.rs"]);
        assert_eq!(tests.first_match(&match_paths(&copied)), None);
        let added = file(None, Some("tests/new.rs"), FileStatus::Added);
        assert_eq!(match_paths(&added), ["tests/new.rs"]);
        let modified = file(Some("tests/a.rs"), Some("tests/a.rs"), FileStatus::Modified);
        assert_eq!(match_paths(&modified), ["tests/a.rs"]);
    }

    /// *Matching is case-sensitive*, for a glob and for a regular
    /// expression.
    #[test]
    fn matching_is_case_sensitive() {
        assert_eq!(named(&rule(&["**/tests/**"]), "Tests/Parse.rs"), None);
        assert_eq!(named(&rule(&["*.RS"]), "a.rs"), None);
        assert_eq!(named(&rule(&["/Tests/"]), "src/tests/a.rs"), None);
        assert_eq!(
            named(&rule(&["/Tests/"]), "src/Tests/a.rs"),
            Some("/Tests/")
        );
    }

    /// *A pattern that does not compile is refused on its field*: the
    /// refusal says why, in one line.
    #[test]
    fn a_regular_expression_that_does_not_compile_is_refused_saying_why() {
        assert_eq!(
            refusal("/(unclosed/"),
            "the regular expression does not compile: unclosed group"
        );
        assert_eq!(
            refusal("/(?=x)/"),
            "the regular expression does not compile: look-around, including look-ahead and look-behind, is not supported"
        );
        assert_eq!(
            refusal("{a"),
            "the glob does not compile: unclosed alternate group; missing '}' (maybe escape '{' with '[{]'?)"
        );
        assert_eq!(
            refusal("src/[z-a].rs"),
            "the glob does not compile: invalid range; 'z' > 'a'"
        );
    }

    /// *A glob with a leading slash is refused*, naming it.
    #[test]
    fn a_glob_with_a_leading_slash_is_refused() {
        assert_eq!(
            refusal("/build/**"),
            "a glob cannot start with a slash, since paths are relative to the repository root"
        );
        // A slash alone, and two, are globs too.
        for pattern in ["/", "//"] {
            assert!(refusal(pattern).starts_with("a glob cannot start"));
        }
    }

    /// `/` + `/` is a glob, and three characters between and including the
    /// slashes are a regular expression.
    #[test]
    fn a_regular_expression_is_three_characters_or_more_between_slashes() {
        assert_eq!(regex_body("//"), None);
        assert_eq!(regex_body("/a/"), Some("a"));
        assert_eq!(regex_body("///"), Some("/"));
        assert_eq!(regex_body("/é/"), Some("é"));
        assert_eq!(regex_body("/ab"), None);
        assert_eq!(regex_body("ab/"), None);
        assert_eq!(regex_body("a/b"), None);
        // `/a/` searches for an `a`, and `///` for a slash.
        assert_eq!(named(&rule(&["/a/"]), "data.json"), Some("/a/"));
        assert_eq!(named(&rule(&["/a/"]), "lib.rs"), None);
        assert_eq!(named(&rule(&["///"]), "src/lib.rs"), Some("///"));
        assert_eq!(named(&rule(&["///"]), "lib.rs"), None);
    }

    /// *Too many patterns are refused*: 64 are accepted and 65 are not, the
    /// 65th named; reading 65, the first 64 still match.
    #[test]
    fn sixty_four_patterns_are_accepted_and_sixty_five_are_not() {
        let many: Vec<String> = (0..65).map(|n| format!("p{n}.rs")).collect();
        assert_eq!(validate(&many[..64]), Ok(many[..64].to_vec()));
        let refused = validate(&many).unwrap_err();
        assert_eq!(
            refused,
            [PatternError {
                index: 64,
                pattern: "p64.rs".to_string(),
                reason: "a list holds at most 64 patterns".to_string(),
            }]
        );
        let (read, errors) = SkipRule::compile(&many);
        assert_eq!(errors, refused);
        assert_eq!(read.first_match(&["p63.rs"]), Some("p63.rs"));
        assert_eq!(read.first_match(&["p64.rs"]), None);
        assert_eq!(MAX_PATTERNS, 64);
    }

    /// 256 bytes once trimmed are accepted and 257 are not, counted in bytes
    /// rather than characters.
    #[test]
    fn a_pattern_of_256_bytes_is_accepted_and_one_of_257_is_not() {
        let at_cap = format!("{}.rs", "a".repeat(253));
        assert_eq!(at_cap.len(), 256);
        assert_eq!(
            validate(&[format!("  {at_cap}  ")]),
            Ok(vec![at_cap.clone()])
        );
        let past = format!("{}.rs", "a".repeat(254));
        assert_eq!(refusal(&past), "a pattern is at most 256 bytes long");
        // 128 two-byte characters are 256 bytes, and one more is past.
        assert!(validate(&["é".repeat(128)]).is_ok());
        assert!(validate(&[format!("{}a", "é".repeat(128))]).is_err());
        assert_eq!(MAX_PATTERN_BYTES, 256);
    }

    /// A pattern is stored without its surrounding whitespace, and an empty
    /// one is refused; a hand-edited one is matched trimmed and named as the
    /// list holds it.
    #[test]
    fn patterns_are_trimmed_and_an_empty_one_is_refused() {
        assert_eq!(
            validate(&list(&["  **/tests/**\t", "*.md "])),
            Ok(list(&["**/tests/**", "*.md"]))
        );
        assert_eq!(refusal(""), "the pattern is empty");
        assert_eq!(refusal("   "), "the pattern is empty");
        let spaced = rule(&[" docs/** "]);
        assert_eq!(named(&spaced, "docs/guide.md"), Some(" docs/** "));
        // Inside the slashes, a space is the expression's own.
        assert_eq!(named(&rule(&["/ /"]), "a b.md"), Some("/ /"));
    }

    /// *An empty list skips nothing*: it is accepted, and matches no path.
    #[test]
    fn an_empty_list_is_accepted_and_skips_nothing() {
        assert_eq!(validate(&[]), Ok(Vec::new()));
        let (empty, errors) = SkipRule::compile(&[]);
        assert_eq!(errors, []);
        assert_eq!(empty.first_match(&["tests/parse.rs"]), None);
        assert_eq!(SkipRule::default().first_match(&["tests/parse.rs"]), None);
    }

    /// *A hand-edited pattern that does not compile is ignored*: reading
    /// skips it and applies the others, and says why on its own place, while
    /// the setter refuses the same list whole.
    #[test]
    fn a_bad_pattern_is_left_out_by_compile_and_refused_by_validate() {
        let held = list(&["/(unclosed/", "**/tests/**", "/abs/**"]);
        let (read, errors) = SkipRule::compile(&held);
        assert_eq!(read.first_match(&["tests/parse.rs"]), Some("**/tests/**"));
        assert_eq!(read.first_match(&["src/(unclosed.rs"]), None);
        assert_eq!(
            errors
                .iter()
                .map(|error| (error.index, error.pattern.as_str()))
                .collect::<Vec<_>>(),
            [(0, "/(unclosed/"), (2, "/abs/**")]
        );
        assert_eq!(validate(&held), Err(errors));
    }
}
