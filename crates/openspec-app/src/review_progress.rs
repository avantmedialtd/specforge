//! The pull-request viewer's review progress (`pull-request-viewer`: *Review
//! Progress*; design D9 of `pull-request-viewer`, D1–D6 of
//! `review-hunks-viewed`, D1–D3 of `review-skip-patterns`): which files, and
//! which hunks of them, of which pull request the reader marked viewed, and
//! which files the skip patterns match that the reader included in the
//! review, kept on this machine only and sent to neither host.
//!
//! Marks live in `review-progress.json` in the shared configuration
//! directory, owned by `AppService` as the activity log is and created by the
//! first mark or inclusion. Entries are keyed by the canonical reference, each
//! one `{ lastMarkedHead, files: { path: key }, hunks: { path: [key] },
//! included: [path], touchedAt }`, `hunks` and `included` left out while
//! empty, and `lastMarkedHead` until a mark. Every write reads the file
//! afresh and replaces it atomically, so a standalone `specforge-serve`
//! beside the desktop app — the documented second writer, as for
//! `activity.json` — loses a mark only when both write at once, and neither
//! sees the other's marks until it reads the file again.
//!
//! A file's key and its hunks' keys are computed here, from the cached
//! detail, and never taken from a caller ([`FileKey`], [`hunk_keys`]). A hunk
//! is viewed when its file's stored key is its current one or its own key is
//! stored, and a file is viewed exactly when every hunk is ([`progress`]);
//! every write keeps that so by one rule ([`MarkWrite::apply`]). A file with
//! nothing stored that a skip pattern matches is skipped unless its path is
//! included: worked out on every read, never stored, so no push brings it
//! back (`review-skip-patterns` design D1). An entry
//! untouched for [`PRUNE_AFTER_SECS`] whose provider is
//! enabled and no longer lists its pull request is pruned, but only while
//! that provider's list is complete, and once every enabled provider's list
//! has arrived in this run ([`Lists`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use openspec_core::{DiffFile, FileStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::bitbucket::BitbucketPullRequestsState;
use crate::events::PullRequestProvider;
use crate::github::GithubPullRequestsState;
use crate::pull_request_detail::{
    file_path, listed_row, CachedFile, PullRequestDetail, PullRequestKey, PullRequestReference,
};
use crate::pull_requests::PullRequestsStatus;
use crate::review_skip::{match_paths, SkipRule};

/// An entry untouched this long is pruned once its provider's list no longer
/// holds its pull request: 90 days.
pub const PRUNE_AFTER_SECS: u64 = 90 * 24 * 60 * 60;

// ---- the wire ----
//
// Camel case on the wire and hand-mirrored in `src/types.ts`;
// `tests/wire_shape.rs` pins the keys and each state's value.

/// A file's review state, derived from its keys, its inclusion and the skip
/// patterns alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileReviewState {
    /// Its stored key equals its current one, or every one of its known
    /// hunks is viewed.
    Viewed,
    /// Its stored key differs from its current one, and some hunk is not
    /// viewed: a push or a retarget changed what the file shows since it was
    /// marked.
    ChangedSinceViewed,
    /// No key is stored for it, and some of its hunks are viewed, or, while
    /// its hunks are not known, some hunk keys are stored for it.
    PartlyViewed,
    /// No mark is stored for it, a skip pattern matches it, and its path is
    /// not included: the reader does not intend to review it. Never stored,
    /// so no push or retarget makes it changed since viewed.
    Skipped,
    /// Nothing that holds is stored for it.
    Unviewed,
}

/// One file of a pull request's review progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReviewProgress {
    /// The path the view keys the file by: its new path, or its old one once
    /// deleted.
    pub path: String,
    pub state: FileReviewState,
    /// The file is keyed by the head commit and the base branch, for want of
    /// patch text or a blob `sha`, so any push or retarget makes it changed
    /// since viewed, and the view says why.
    pub keyed_by_head: bool,
    /// Whether each of its hunks is viewed, in the order the view renders
    /// them; `None` while its hunks are not known.
    pub hunks: Option<Vec<bool>>,
    /// The first skip pattern that matches the file, as the list holds it,
    /// whether or not the file is skipped; `None` when none does.
    pub matched: Option<String>,
    /// The reader included the file's path in the review.
    pub included: bool,
}

/// What `get_review_progress` answers for one pull request: each file of its
/// cached detail with its state, and the counts the header shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProgress {
    /// In the detail's file order.
    pub files: Vec<FileReviewProgress>,
    /// Files viewed.
    pub viewed: usize,
    /// Files changed since viewed. A partly viewed file counts in neither.
    pub changed_since_viewed: usize,
    /// Files skipped, which the header leaves out of the files it counts,
    /// rather than counting them viewed.
    pub skipped: usize,
    /// Every file of the cached detail, the skipped ones included.
    pub total: usize,
    /// The head commit at the last mark, which only dates the changed count;
    /// `None` before any mark.
    pub last_marked_head: Option<String>,
    /// The cached detail's head commit. The hunk states are positional, so
    /// they hold only for the detail these two commits name.
    pub head_commit: String,
    /// The cached detail's base commit.
    pub base_commit: String,
}

// ---- keys ----

/// What a file's viewed mark is keyed by (design D9), stored in
/// `review-progress.json` as written. The service computes it from the
/// cached detail, so no caller can supply one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(crate) enum FileKey {
    /// A file with patch text, withheld or not: `sha256:` and the hex SHA-256
    /// of its patch bytes as received. Never a short or non-cryptographic
    /// hash: the author controls the patch, and the key outlives the run and
    /// the toolchain.
    Patch(String),
    /// A file without patch text that GitHub gives a blob `sha`: the `sha`,
    /// with the file's status, its previous filename and the base branch's
    /// name.
    Blob {
        sha: String,
        status: FileStatus,
        previous: Option<String>,
        base_branch: String,
    },
    /// Any other file, every BitBucket file without patch text among them:
    /// the head commit and the base branch's name, which any push or retarget
    /// changes.
    HeadCommit { commit: String, base_branch: String },
}

/// Lowercase hex, two digits a byte.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The key of `file` of `detail`, from what the cache keeps beside it:
///
/// key(f) = SHA-256(patch(f)) when f has patch text; else (sha(f), status(f),
/// previous(f), base branch) when GitHub gives f a blob sha; else (head
/// commit, base branch).
pub(crate) fn file_key(
    file: &DiffFile,
    cached: &CachedFile,
    detail: &PullRequestDetail,
) -> FileKey {
    if let Some(patch) = cached.patch {
        return FileKey::Patch(format!("sha256:{}", hex(&patch.sha256)));
    }
    let base_branch = detail.base_branch.clone();
    match &cached.blob_sha {
        Some(sha) => FileKey::Blob {
            sha: sha.clone(),
            status: file.status,
            previous: file.old_path.clone(),
            base_branch,
        },
        None => FileKey::HeadCommit {
            commit: detail.head_commit.clone(),
            base_branch,
        },
    }
}

/// The key of each of a file's hunks, in the order the view renders them,
/// from what the cache keeps beside it (design D1):
///
/// key(h_i) = `sha256:` hex(SHA-256(body(h_i))) `#` n_i, where n_i counts the
/// hunks j ≤ i whose body equals h_i's,
///
/// so identical hunks of one file are numbered apart. Never a short hash, for
/// the file key's reasons. `None` while the file's hunks are not known: no
/// digests are cached for it, or it has no hunk.
pub(crate) fn hunk_keys(cached: &CachedFile) -> Option<Vec<String>> {
    let digests = cached
        .hunks
        .as_ref()
        .filter(|digests| !digests.is_empty())?;
    let mut seen: HashMap<&[u8; 32], u32> = HashMap::new();
    Some(
        digests
            .iter()
            .map(|digest| {
                let n = seen.entry(digest).or_insert(0);
                *n += 1;
                format!("sha256:{}#{n}", hex(digest))
            })
            .collect(),
    )
}

/// The file of the cached `detail` at `path`, beside what the cache keeps of
/// it, checked as both marking commands check: refused when either commit
/// differs from the detail's, as a push or a retarget read since makes them,
/// and when no file of the detail has that path.
fn marked_file<'a>(
    detail: &'a PullRequestDetail,
    files: &'a [CachedFile],
    path: &str,
    head: &str,
    base: &str,
) -> Result<(&'a DiffFile, &'a CachedFile), String> {
    if detail.head_commit != head || detail.base_commit != base {
        return Err("the pull request has changed since it was read".to_string());
    }
    detail
        .files
        .iter()
        .zip(files)
        .find(|(file, _)| file_path(file) == Some(path))
        .ok_or_else(|| "not a file of this pull request".to_string())
}

/// Whether a skip pattern of `rule` matches `file`.
fn matched(rule: &SkipRule, file: &DiffFile) -> bool {
    rule.first_match(&match_paths(file)).is_some()
}

/// What `set_file_viewed` writes for `path`, its keys computed from the
/// cached detail, never taken from a caller. A mark or unmark of a file a
/// skip pattern of `rule` matches includes it as well (design D3).
pub(crate) fn file_write(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    path: &str,
    viewed: bool,
    head: &str,
    base: &str,
    rule: &SkipRule,
) -> Result<ReviewWrite, String> {
    let (file, cached) = marked_file(detail, files, path, head, base)?;
    let write = if viewed {
        MarkWrite::MarkFile {
            file: file_key(file, cached, detail),
            hunks: hunk_keys(cached),
        }
    } else {
        MarkWrite::UnmarkFile
    };
    Ok(ReviewWrite::Marks {
        write,
        include: matched(rule, file),
    })
}

/// What `set_hunk_viewed` writes for hunk `hunk` of `path`, counted from zero
/// in the order the view renders them. Refused, beside `set_file_viewed`'s
/// refusals, while the file's hunks are not known, and for an index past its
/// last hunk. Includes a file a skip pattern of `rule` matches, as a file's
/// write does.
// The request's own commits beside the rule they are judged by: bundling
// any two of the eight would only rename the list.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hunk_write(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    path: &str,
    hunk: usize,
    viewed: bool,
    head: &str,
    base: &str,
    rule: &SkipRule,
) -> Result<ReviewWrite, String> {
    let (file, cached) = marked_file(detail, files, path, head, base)?;
    let hunks =
        hunk_keys(cached).ok_or_else(|| "the file's hunks have not been read".to_string())?;
    let key = hunks
        .get(hunk)
        .cloned()
        .ok_or_else(|| "not a hunk of this file".to_string())?;
    let include = matched(rule, file);
    let file = file_key(file, cached, detail);
    let write = if viewed {
        MarkWrite::MarkHunk { key, file, hunks }
    } else {
        MarkWrite::UnmarkHunk { key, file, hunks }
    };
    Ok(ReviewWrite::Marks { write, include })
}

/// What `set_file_included` writes for `path`: its inclusion in the review,
/// or its exclusion. Refused as `set_file_viewed` is, and never for want of a
/// matching pattern: including a file no pattern matches changes no state.
pub(crate) fn include_write(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    path: &str,
    included: bool,
    head: &str,
    base: &str,
) -> Result<ReviewWrite, String> {
    marked_file(detail, files, path, head, base)?;
    Ok(ReviewWrite::Include(included))
}

/// The review progress of `detail`'s files, each derived from its keys, its
/// inclusion and `rule` alone (design D4; `review-skip-patterns` D1), and the
/// counts. A hunk is viewed when its file's stored key is its current one, or
/// its own key is stored. A file is:
///
/// - viewed when its stored key is its current one, or its hunks are known
///   and every one is viewed;
/// - else changed since viewed when a key is stored for it;
/// - else partly viewed when one of its known hunks is viewed, or, while its
///   hunks are not known, some hunk keys are stored for it;
/// - else skipped when a skip pattern matches it, by its new path or a moved
///   file's old one, and its path is not included;
/// - else unviewed.
///
/// So the reader's marks always come first, and a skipped file has nothing
/// stored that a push could make stale.
pub(crate) fn progress(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    entry: Option<&Entry>,
    rule: &SkipRule,
) -> ReviewProgress {
    let files: Vec<FileReviewProgress> = detail
        .files
        .iter()
        .zip(files)
        .filter_map(|(file, cached)| {
            let path = file_path(file)?;
            let key = file_key(file, cached, detail);
            let stored_file = entry.and_then(|entry| entry.files.get(path));
            let stored_hunks = entry.and_then(|entry| entry.hunks.get(path));
            let included = entry.is_some_and(|entry| entry.included.contains(path));
            let matched = rule.first_match(&match_paths(file)).map(str::to_string);
            let whole = stored_file == Some(&key);
            let hunks: Option<Vec<bool>> = hunk_keys(cached).map(|keys| {
                keys.iter()
                    .map(|hunk| whole || stored_hunks.is_some_and(|stored| stored.contains(hunk)))
                    .collect()
            });
            let state = match &hunks {
                _ if whole => FileReviewState::Viewed,
                Some(viewed) if viewed.iter().all(|&viewed| viewed) => FileReviewState::Viewed,
                _ if stored_file.is_some() => FileReviewState::ChangedSinceViewed,
                Some(viewed) if viewed.contains(&true) => FileReviewState::PartlyViewed,
                None if stored_hunks.is_some_and(|stored| !stored.is_empty()) => {
                    FileReviewState::PartlyViewed
                }
                _ if matched.is_some() && !included => FileReviewState::Skipped,
                _ => FileReviewState::Unviewed,
            };
            Some(FileReviewProgress {
                path: path.to_string(),
                state,
                keyed_by_head: matches!(key, FileKey::HeadCommit { .. }),
                hunks,
                matched,
                included,
            })
        })
        .collect();
    let count = |state| files.iter().filter(|file| file.state == state).count();
    ReviewProgress {
        viewed: count(FileReviewState::Viewed),
        changed_since_viewed: count(FileReviewState::ChangedSinceViewed),
        skipped: count(FileReviewState::Skipped),
        total: files.len(),
        last_marked_head: entry.and_then(|entry| entry.last_marked_head.clone()),
        head_commit: detail.head_commit.clone(),
        base_commit: detail.base_commit.clone(),
        files,
    }
}

// ---- the writes ----

/// What one file has stored: its key, and its viewed hunks' keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Marks {
    pub(crate) file: Option<FileKey>,
    pub(crate) hunks: BTreeSet<String>,
}

/// One write to a file's stored review: to its marks, or to its inclusion
/// ([`file_write`], [`hunk_write`], [`include_write`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewWrite {
    /// A mark or an unmark of the file or of one of its hunks. `include`
    /// adds the file's path to `included` too, for a file a skip pattern
    /// matches: a reader who marked or unmarked it took it into the review,
    /// so an unmark leaves it unviewed rather than skipped again
    /// (`review-skip-patterns` design D3).
    Marks { write: MarkWrite, include: bool },
    /// Including the file's path in the review, or excluding it. Kept by
    /// path, never by key, so no push or retarget undoes it (design D2).
    Include(bool),
}

impl ReviewWrite {
    /// Whether the write may create the entry: a mark, or an inclusion. An
    /// unmark and an exclusion never do.
    fn creates(&self) -> bool {
        match self {
            Self::Marks { write, .. } => write.marks(),
            Self::Include(included) => *included,
        }
    }
}

/// One write to a file's marks, with the keys the service computed from the
/// cached detail ([`file_write`], [`hunk_write`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarkWrite {
    /// Mark the file: its key, and its current hunks' keys while they are
    /// known.
    MarkFile {
        file: FileKey,
        hunks: Option<Vec<String>>,
    },
    UnmarkFile,
    /// Mark one hunk: its key, its file's key, and every current hunk's key.
    MarkHunk {
        key: String,
        file: FileKey,
        hunks: Vec<String>,
    },
    /// Unmark one hunk, with the same three.
    UnmarkHunk {
        key: String,
        file: FileKey,
        hunks: Vec<String>,
    },
}

impl MarkWrite {
    /// Whether the write marks, which alone advances `lastMarkedHead` and,
    /// beside an inclusion, alone may create an entry.
    fn marks(&self) -> bool {
        matches!(self, Self::MarkFile { .. } | Self::MarkHunk { .. })
    }

    /// The file's marks after the write, from `stored` (design D3). With `V`
    /// the keys of the current hunks viewed now:
    ///
    /// | write          | hunk keys after             | file key after                      |
    /// |----------------|-----------------------------|-------------------------------------|
    /// | mark the file  | every current one, if known | its key                             |
    /// | unmark it      | none                        | none                                |
    /// | mark hunk `h`  | `V ∪ {h}`                   | its key once that covers every hunk |
    /// | unmark `h`     | `V ∖ {h}`                   | none                                |
    ///
    /// So a file whose hunks are known keeps no key matching none of them,
    /// unmarking one hunk of a file marked whole keeps its other hunks
    /// viewed, and a stale file key survives hunk marks, still saying the
    /// file changed since viewed, until the last one replaces it.
    pub(crate) fn apply(&self, stored: Marks) -> Marks {
        // The current hunks viewed now, as `progress` derives them.
        let viewed = |file: &FileKey, hunks: &[String]| -> BTreeSet<String> {
            let whole = stored.file.as_ref() == Some(file);
            hunks
                .iter()
                .filter(|hunk| whole || stored.hunks.contains(*hunk))
                .cloned()
                .collect()
        };
        match self {
            Self::MarkFile { file, hunks } => Marks {
                file: Some(file.clone()),
                hunks: match hunks {
                    Some(hunks) => hunks.iter().cloned().collect(),
                    None => stored.hunks.clone(),
                },
            },
            Self::UnmarkFile => Marks::default(),
            Self::MarkHunk { key, file, hunks } => {
                let mut kept = viewed(file, hunks);
                kept.insert(key.clone());
                let covered = hunks.iter().all(|hunk| kept.contains(hunk));
                Marks {
                    file: if covered {
                        Some(file.clone())
                    } else {
                        stored.file.clone()
                    },
                    hunks: kept,
                }
            }
            Self::UnmarkHunk { key, file, hunks } => {
                let mut kept = viewed(file, hunks);
                kept.remove(key);
                Marks {
                    file: None,
                    hunks: kept,
                }
            }
        }
    }
}

// ---- the store ----

/// One pull request's stored progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    /// The head commit at the last mark; `None` until one, as in an entry
    /// an inclusion created, since including never advances it. An unmark
    /// leaves it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_marked_head: Option<String>,
    /// Each marked file's key, by its path.
    pub(crate) files: BTreeMap<String, FileKey>,
    /// Each file's viewed hunks' keys, by its path; a path is left out while
    /// none is stored, and the whole map while it is empty, so an entry from
    /// before hunk marks reads as one with none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) hunks: BTreeMap<String, BTreeSet<String>>,
    /// The paths of the files the reader included in the review; left out
    /// while empty, so an entry from before skip patterns reads as one that
    /// includes nothing and an entry that includes nothing is stored as it
    /// was before them.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) included: BTreeSet<String>,
    /// When a mark, an unmark, an inclusion or an exclusion last touched the
    /// entry, as Unix epoch seconds.
    pub(crate) touched_at: u64,
}

impl Entry {
    /// An entry as stored, when it is one this version reads.
    fn read(value: &Value) -> Option<Self> {
        Self::deserialize(value).ok()
    }
}

/// The name an entry is stored under: its canonical reference, as
/// `<provider>/<owner>/<repo>/<number>`, owner and repository in lowercase.
fn entry_name(key: &PullRequestKey) -> String {
    let provider = match key.provider {
        PullRequestProvider::Github => "github",
        PullRequestProvider::Bitbucket => "bitbucket",
    };
    format!("{provider}/{}/{}/{}", key.owner, key.repo, key.number)
}

/// The reference an entry's name spells, the inverse of [`entry_name`];
/// `None` for a name this version does not write. An owner holds no `/`,
/// since a row's repository splits at its first one, so the repository is
/// everything from there to the number.
fn reference_of(name: &str) -> Option<PullRequestReference> {
    let (provider, rest) = name.split_once('/')?;
    let provider = match provider {
        "github" => PullRequestProvider::Github,
        "bitbucket" => PullRequestProvider::Bitbucket,
        _ => return None,
    };
    let (path, number) = rest.rsplit_once('/')?;
    let (owner, repo) = path.split_once('/')?;
    Some(PullRequestReference {
        provider,
        owner: owner.to_string(),
        repo: repo.to_string(),
        number: number.parse().ok()?,
    })
}

/// Every entry as stored, by name. An entry this version cannot read stays
/// exactly as it is, so a write never drops another version's marks.
type Entries = Map<String, Value>;

/// `review-progress.json`, read afresh by every call and replaced atomically
/// by every write (`pull-request-viewer`: *Review Progress*). `AppService`
/// owns it and opens it in `bootstrap`; nothing is read until a call asks.
#[derive(Debug)]
pub(crate) struct ReviewProgressStore {
    path: PathBuf,
    /// Held across each read, change and write, so this process's own writes
    /// never interleave.
    writing: Mutex<()>,
}

impl ReviewProgressStore {
    pub(crate) fn open(path: PathBuf) -> Self {
        Self {
            path,
            writing: Mutex::new(()),
        }
    }

    /// The stored entry of `key`, read from the file now; `None` when nothing
    /// this version reads is stored for it.
    pub(crate) fn entry(&self, key: &PullRequestKey) -> io::Result<Option<Entry>> {
        Ok(self
            .read()?
            .and_then(|entries| entries.get(&entry_name(key)).and_then(Entry::read)))
    }

    /// Applies `write` to the marks or the inclusion of `path` in the entry
    /// of `key`, read afresh, and touches the entry at `now`. A mark creates
    /// the entry, and advances its `lastMarkedHead` to `head`; an unmark
    /// leaves that, and with no entry creates none and stores nothing. An
    /// inclusion creates the entry too, and neither it nor an exclusion
    /// advances `lastMarkedHead`; an exclusion with no entry creates none
    /// and stores nothing. Returns whether it stored the write.
    pub(crate) fn write_marks(
        &self,
        key: &PullRequestKey,
        path: &str,
        write: &ReviewWrite,
        head: &str,
        now: u64,
    ) -> io::Result<bool> {
        let _writing = self.writing.lock().unwrap();
        let mut entries = self.read_for_write()?;
        let name = entry_name(key);
        let mut entry = match entries.get(&name).and_then(Entry::read) {
            Some(entry) => entry,
            None if write.creates() => Entry {
                last_marked_head: None,
                files: BTreeMap::new(),
                hunks: BTreeMap::new(),
                included: BTreeSet::new(),
                touched_at: now,
            },
            None => return Ok(false),
        };
        match write {
            ReviewWrite::Marks { write, include } => {
                let stored = Marks {
                    file: entry.files.remove(path),
                    hunks: entry.hunks.remove(path).unwrap_or_default(),
                };
                let marks = write.apply(stored);
                if let Some(file) = marks.file {
                    entry.files.insert(path.to_string(), file);
                }
                if !marks.hunks.is_empty() {
                    entry.hunks.insert(path.to_string(), marks.hunks);
                }
                if *include {
                    entry.included.insert(path.to_string());
                }
                if write.marks() {
                    entry.last_marked_head = Some(head.to_string());
                }
            }
            ReviewWrite::Include(true) => {
                entry.included.insert(path.to_string());
            }
            ReviewWrite::Include(false) => {
                entry.included.remove(path);
            }
        }
        entry.touched_at = now;
        entries.insert(name, serde_json::to_value(entry)?);
        self.write(&entries)?;
        Ok(true)
    }

    /// Removes every entry `pruned` names by its reference and its
    /// `touchedAt`, and returns their references. An entry whose name or
    /// `touchedAt` cannot be read is kept. The file is written only when an
    /// entry was removed.
    pub(crate) fn prune(
        &self,
        pruned: impl Fn(&PullRequestReference, u64) -> bool,
    ) -> io::Result<Vec<PullRequestReference>> {
        let _writing = self.writing.lock().unwrap();
        let Some(mut entries) = self.read()? else {
            return Ok(Vec::new());
        };
        let mut removed = Vec::new();
        entries.retain(|name, entry| {
            let touched_at = entry.get("touchedAt").and_then(Value::as_u64);
            match (reference_of(name), touched_at) {
                (Some(reference), Some(touched_at)) if pruned(&reference, touched_at) => {
                    removed.push(reference);
                    false
                }
                _ => true,
            }
        });
        if !removed.is_empty() {
            self.write(&entries)?;
        }
        Ok(removed)
    }

    /// The stored entries, read from the file now: none when there is no
    /// file, and `None` when the file is not a JSON object at all.
    fn read(&self) -> io::Result<Option<Entries>> {
        match fs::read(&self.path) {
            Ok(raw) => Ok(serde_json::from_slice(&raw).ok()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Some(Entries::new())),
            Err(error) => Err(error),
        }
    }

    /// The stored entries a write starts from. A file that is not a JSON
    /// object is moved aside to a `.corrupt-<n>` sibling first, as an
    /// unreadable registry is, so the write that follows never destroys it.
    fn read_for_write(&self) -> io::Result<Entries> {
        match self.read()? {
            Some(entries) => Ok(entries),
            None => {
                crate::service::preserve_corrupt_config(&self.path);
                Ok(Entries::new())
            }
        }
    }

    /// Replaces the file with `entries`, atomically, as `WorkspaceRegistry`
    /// saves `workspaces.json`: a uniquely named temporary file in the same
    /// directory, so the rename stays on one filesystem and two processes
    /// never collide on a fixed name, synced and renamed over the target. A
    /// crash or the other process therefore never sees a truncated store.
    fn write(&self, entries: &Entries) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let raw = serde_json::to_vec_pretty(entries)?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        staged.write_all(&raw)?;
        staged.as_file().sync_all()?;
        staged.persist(&self.path).map_err(|error| error.error)?;
        // Best effort, as the registry's: the rename itself survives a crash.
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

// ---- pruning ----

/// What the pruning rule reads of the two providers when it is applied: each
/// one's enabled flag, and its snapshot as it is now.
pub(crate) struct Lists<'a> {
    pub(crate) github_enabled: bool,
    pub(crate) bitbucket_enabled: bool,
    pub(crate) github: &'a GithubPullRequestsState,
    pub(crate) bitbucket: &'a BitbucketPullRequestsState,
}

impl Lists<'_> {
    fn enabled(&self, provider: PullRequestProvider) -> bool {
        match provider {
            PullRequestProvider::Github => self.github_enabled,
            PullRequestProvider::Bitbucket => self.bitbucket_enabled,
        }
    }

    /// Whether `provider`'s snapshot is a successful, non-stale refresh.
    /// Snapshots live in memory only and start disabled, so such a snapshot
    /// is one refreshed in this run; a stale one, or a fresh `unavailable`
    /// one with no rows, proves nothing absent.
    fn refreshed(&self, provider: PullRequestProvider) -> bool {
        let (status, stale) = match provider {
            PullRequestProvider::Github => (self.github.status, self.github.stale),
            PullRequestProvider::Bitbucket => (self.bitbucket.status, self.bitbucket.stale),
        };
        status == PullRequestsStatus::Ok && !stale
    }

    /// Whether `provider`'s list holds every pull request its provider would
    /// list: GitHub's when it withheld no result, as it withholds an
    /// organisation blocked by single sign-on, and BitBucket's when it
    /// skipped no workspace. An incomplete list proves no pull request
    /// absent, any more than a disabled provider's empty one does.
    fn complete(&self, provider: PullRequestProvider) -> bool {
        match provider {
            PullRequestProvider::Github => self.github.withheld == 0,
            PullRequestProvider::Bitbucket => self.bitbucket.skipped_workspaces.is_empty(),
        }
    }

    /// Every enabled provider's list has arrived, by a successful, non-stale
    /// refresh in this run. With no provider enabled this holds, and prunes
    /// nothing all the same: no entry's provider is enabled.
    fn arrived(&self) -> bool {
        [PullRequestProvider::Github, PullRequestProvider::Bitbucket]
            .into_iter()
            .all(|provider| !self.enabled(provider) || self.refreshed(provider))
    }

    /// Whether the entry of `reference`, last touched at `touched_at`, is
    /// pruned at `now`:
    ///
    /// prune(e) ⇔ now − touchedAt(e) ≥ 90 days ∧ p(e) ∈ enabled ∧
    /// complete(S_p(e)) ∧ e ∉ S_p(e) ∧ ∀ p ∈ enabled: refreshedThisRun(p)
    ///
    /// A disabled provider's entries are kept, since its empty list proves
    /// nothing, and so are an incomplete list's provider's, whatever the
    /// other provider's list says.
    pub(crate) fn prunes(
        &self,
        reference: &PullRequestReference,
        touched_at: u64,
        now: u64,
    ) -> bool {
        now.saturating_sub(touched_at) >= PRUNE_AFTER_SECS
            && self.enabled(reference.provider)
            && self.complete(reference.provider)
            && listed_row(reference, self.bitbucket, self.github).is_none()
            && self.arrived()
    }

    /// Applies the rule to `store` at `now`, and returns the references it
    /// pruned. Until every enabled provider's list has arrived nothing can be
    /// pruned, and the store is not read.
    pub(crate) fn prune(
        &self,
        store: &ReviewProgressStore,
        now: u64,
    ) -> io::Result<Vec<PullRequestReference>> {
        if !self.arrived() {
            return Ok(Vec::new());
        }
        store.prune(|reference, touched_at| self.prunes(reference, touched_at, now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_request_detail::PatchDigest;
    use crate::pull_requests::PullRequestSummary;
    use openspec_core::{DiffContent, Hunk};
    use serde_json::json;

    const NOW: u64 = 1_800_000_000;
    const DAY: u64 = 24 * 60 * 60;
    const HEAD: &str = "1111111111111111111111111111111111111111";
    const PUSHED: &str = "3333333333333333333333333333333333333333";
    const BASE: &str = "2222222222222222222222222222222222222222";

    fn reference(provider: PullRequestProvider, number: u64) -> PullRequestReference {
        PullRequestReference {
            provider,
            owner: "acme".to_string(),
            repo: "api".to_string(),
            number,
        }
    }

    fn github(number: u64) -> PullRequestReference {
        reference(PullRequestProvider::Github, number)
    }

    fn bitbucket(number: u64) -> PullRequestReference {
        reference(PullRequestProvider::Bitbucket, number)
    }

    fn row(number: u64) -> PullRequestSummary {
        PullRequestSummary {
            id: number,
            title: format!("PR {number}"),
            repo_full_name: "acme/api".to_string(),
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            url: format!("https://github.com/acme/api/pull/{number}"),
            draft: false,
            updated_at_unix: 1_700_000_000,
            review: None,
            open_tasks: 0,
            author: None,
            checks: None,
            conflicting: false,
            unresolved_threads: 0,
            source_repo_full_name: "acme/api".to_string(),
        }
    }

    fn file(path: &str) -> DiffFile {
        DiffFile {
            old_path: Some(path.to_string()),
            new_path: Some(path.to_string()),
            old_mode: None,
            new_mode: None,
            status: FileStatus::Modified,
            additions: Some(1),
            deletions: Some(1),
            content: DiffContent::Hunks { hunks: Vec::new() },
        }
    }

    /// What the cache keeps of a file with patch text, and GitHub's blob
    /// `sha` beside it, as GitHub gives one for every file.
    fn patched(patch: &[u8]) -> CachedFile {
        CachedFile {
            withheld: None,
            patch: Some(PatchDigest::of([patch])),
            blob_sha: Some("blob-sha".to_string()),
            ..Default::default()
        }
    }

    /// What the cache keeps of a GitHub file without a `patch`.
    fn blob(sha: &str) -> CachedFile {
        CachedFile {
            withheld: None,
            patch: None,
            blob_sha: Some(sha.to_string()),
            ..Default::default()
        }
    }

    /// What the cache keeps of a file with neither, as of every BitBucket
    /// file without patch text.
    fn bare() -> CachedFile {
        CachedFile {
            withheld: None,
            patch: None,
            blob_sha: None,
            ..Default::default()
        }
    }

    /// A detail read at `head` against `BASE` on `main`, with `files`.
    fn detail(head: &str, files: &[DiffFile]) -> PullRequestDetail {
        PullRequestDetail {
            reference: github(42),
            row: row(42),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            head_commit: head.to_string(),
            base_commit: BASE.to_string(),
            author: None,
            description: String::new(),
            conversation: Vec::new(),
            checks: Vec::new(),
            threads: Vec::new(),
            files: files.to_vec(),
            unlisted_files: 0,
            read_at_unix: NOW,
            no_longer_listed: false,
        }
    }

    /// The key `path` is marked with, read at `head` with `cached` beside it.
    fn key_at(head: &str, path: &str, cached: &CachedFile) -> FileKey {
        file_key(&file(path), cached, &detail(head, &[file(path)]))
    }

    /// The current key of the file of `read` at `path`, `files` beside it.
    fn key_of(read: &PullRequestDetail, files: &[CachedFile], path: &str) -> FileKey {
        let at = read
            .files
            .iter()
            .position(|file| file_path(file) == Some(path))
            .expect("a file of the detail");
        file_key(&read.files[at], &files[at], read)
    }

    /// No skip pattern at all.
    fn no_rule() -> SkipRule {
        SkipRule::default()
    }

    /// The rule of `patterns`.
    fn skipping(patterns: &[&str]) -> SkipRule {
        let list: Vec<String> = patterns.iter().map(|pattern| pattern.to_string()).collect();
        let (rule, errors) = SkipRule::compile(&list);
        assert_eq!(errors, [], "{patterns:?}");
        rule
    }

    /// `write`, of a file no skip pattern matches.
    fn marks(write: MarkWrite) -> ReviewWrite {
        ReviewWrite::Marks {
            write,
            include: false,
        }
    }

    /// The whole-file mark and unmark the store tests write, as
    /// `set_file_viewed` writes them for a file whose hunks are not known
    /// and that no skip pattern matches.
    impl ReviewProgressStore {
        fn mark(
            &self,
            key: &PullRequestKey,
            path: &str,
            file: FileKey,
            head: &str,
            now: u64,
        ) -> io::Result<()> {
            let write = marks(MarkWrite::MarkFile { file, hunks: None });
            self.write_marks(key, path, &write, head, now).map(|_| ())
        }

        fn unmark(&self, key: &PullRequestKey, path: &str, now: u64) -> io::Result<bool> {
            self.write_marks(key, path, &marks(MarkWrite::UnmarkFile), "unused", now)
        }
    }

    /// An entry marked at `HEAD` with `files`.
    fn entry(files: impl IntoIterator<Item = (&'static str, FileKey)>) -> Entry {
        Entry {
            last_marked_head: Some(HEAD.to_string()),
            files: files
                .into_iter()
                .map(|(path, key)| (path.to_string(), key))
                .collect(),
            hunks: BTreeMap::new(),
            included: BTreeSet::new(),
            touched_at: NOW,
        }
    }

    /// The state of the one file `path` in a detail read at `head` with
    /// `cached` beside it, against `stored`.
    fn state(head: &str, path: &str, cached: CachedFile, stored: &Entry) -> FileReviewState {
        let read = detail(head, &[file(path)]);
        progress(&read, &[cached], Some(stored), &no_rule()).files[0].state
    }

    // ------------------------------------------------------------ keys

    /// A known SHA-256 vector: a shortened, truncated or non-cryptographic
    /// hash of the same bytes would not spell it.
    #[test]
    fn a_patch_is_keyed_by_the_full_hex_sha256_of_its_bytes() {
        assert_eq!(
            key_at(HEAD, "a.rs", &patched(b"abc")),
            FileKey::Patch(
                "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .to_string()
            )
        );
        assert_eq!(
            key_at(HEAD, "a.rs", &patched(b"")),
            FileKey::Patch(
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string()
            )
        );
        assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }

    /// GitHub gives every file a blob `sha`; one with patch text is keyed by
    /// the patch all the same, and so is a withheld one, whose hunks the
    /// cache holds.
    #[test]
    fn patch_text_outranks_a_blob_and_a_withheld_file_keeps_its_patch_key() {
        let withheld = CachedFile {
            withheld: Some(vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
                section: None,
                lines: Vec::new(),
            }]),
            ..patched(b"abc")
        };
        assert_eq!(
            key_at(HEAD, "a.rs", &withheld),
            key_at(HEAD, "a.rs", &patched(b"abc"))
        );
        assert!(matches!(
            key_at(HEAD, "a.rs", &patched(b"abc")),
            FileKey::Patch(_)
        ));
    }

    /// `pull-request-viewer`: *A push that changes a file flags it*: the file
    /// is changed since viewed, and the header counts it.
    #[test]
    fn a_patch_changed_by_a_push_makes_its_file_changed_since_viewed_and_counted() {
        let stored = entry([("a.rs", key_at(HEAD, "a.rs", &patched(b"+one\n")))]);
        let pushed = detail(PUSHED, &[file("a.rs")]);
        let read = progress(&pushed, &[patched(b"+two\n")], Some(&stored), &no_rule());
        assert_eq!(read.files[0].state, FileReviewState::ChangedSinceViewed);
        assert!(!read.files[0].keyed_by_head);
        assert_eq!((read.viewed, read.changed_since_viewed), (0, 1));
    }

    /// `pull-request-viewer`: *A push that leaves a file alone keeps its
    /// mark*: a byte-identical patch at a new head is still viewed.
    #[test]
    fn a_byte_identical_patch_after_a_push_keeps_its_mark() {
        let stored = entry([("a.rs", key_at(HEAD, "a.rs", &patched(b"+one\n")))]);
        assert_eq!(
            state(PUSHED, "a.rs", patched(b"+one\n"), &stored),
            FileReviewState::Viewed
        );
    }

    /// The bytes are keyed as received: `é` in Latin-1 (0xE9) and `è`
    /// (0xE8) are each invalid UTF-8, and would decode alike, yet their
    /// patches key apart.
    #[test]
    fn one_latin1_byte_keys_a_patch_apart() {
        let stored = entry([("README", key_at(HEAD, "README", &patched(b"+caf\xe9\n")))]);
        assert_eq!(
            String::from_utf8_lossy(b"+caf\xe9\n"),
            String::from_utf8_lossy(b"+caf\xe8\n"),
            "alike once decoded"
        );
        assert_eq!(
            state(PUSHED, "README", patched(b"+caf\xe8\n"), &stored),
            FileReviewState::ChangedSinceViewed
        );
    }

    /// `pull-request-viewer`: *A file keyed by the head commit says why it
    /// changed*: a file with neither patch text nor a blob is keyed by the
    /// head commit and the base branch, so a push flags it and says why, and
    /// so does a retarget.
    #[test]
    fn a_file_keyed_by_the_head_is_flagged_by_a_push_or_a_retarget_and_says_why() {
        assert_eq!(
            key_at(HEAD, "logo.png", &bare()),
            FileKey::HeadCommit {
                commit: HEAD.to_string(),
                base_branch: "main".to_string(),
            }
        );
        let stored = entry([("logo.png", key_at(HEAD, "logo.png", &bare()))]);
        let pushed = progress(
            &detail(PUSHED, &[file("logo.png")]),
            &[bare()],
            Some(&stored),
            &no_rule(),
        );
        assert_eq!(pushed.files[0].state, FileReviewState::ChangedSinceViewed);
        assert!(pushed.files[0].keyed_by_head);

        let unmoved = progress(
            &detail(HEAD, &[file("logo.png")]),
            &[bare()],
            Some(&stored),
            &no_rule(),
        );
        assert_eq!(unmoved.files[0].state, FileReviewState::Viewed);
        let retargeted = PullRequestDetail {
            base_branch: "release".to_string(),
            ..detail(HEAD, &[file("logo.png")])
        };
        let read = progress(&retargeted, &[bare()], Some(&stored), &no_rule());
        assert_eq!(read.files[0].state, FileReviewState::ChangedSinceViewed);
    }

    /// `pull-request-viewer`: *A file without a patch is keyed by GitHub's
    /// blob*: while its `sha`, status, previous name and base branch hold, a
    /// push leaves it viewed; a change to any one of them flags it.
    #[test]
    fn a_patchless_file_is_keyed_by_its_blob_status_previous_name_and_base_branch() {
        let marked = detail(HEAD, &[file("logo.png")]);
        let stored = entry([(
            "logo.png",
            file_key(&file("logo.png"), &blob("sha-1"), &marked),
        )]);
        assert_eq!(
            stored.files["logo.png"],
            FileKey::Blob {
                sha: "sha-1".to_string(),
                status: FileStatus::Modified,
                previous: Some("logo.png".to_string()),
                base_branch: "main".to_string(),
            }
        );
        let at = |read: &PullRequestDetail, cached: CachedFile| {
            progress(read, &[cached], Some(&stored), &no_rule()).files[0].clone()
        };
        let pushed = detail(PUSHED, &[file("logo.png")]);
        let kept = at(&pushed, blob("sha-1"));
        assert_eq!(kept.state, FileReviewState::Viewed);
        assert!(!kept.keyed_by_head);

        let moved = DiffFile {
            old_path: Some("old-logo.png".to_string()),
            ..file("logo.png")
        };
        let retargeted = PullRequestDetail {
            base_branch: "release".to_string(),
            ..pushed.clone()
        };
        // Each case changes one of the four, and only it.
        for (read, cached) in [
            (pushed.clone(), blob("sha-2")),
            (detail(PUSHED, &[moved]), blob("sha-1")),
            (
                detail(
                    PUSHED,
                    &[DiffFile {
                        status: FileStatus::TypeChanged,
                        ..file("logo.png")
                    }],
                ),
                blob("sha-1"),
            ),
            (retargeted, blob("sha-1")),
        ] {
            assert_eq!(
                at(&read, cached).state,
                FileReviewState::ChangedSinceViewed,
                "{:?}",
                read.files[0]
            );
        }
    }

    /// A mark is refused against another head or base, a push or a
    /// retarget read since, and for a path not among the files; a deleted
    /// file goes by its old path. A file's mark carries its key, and its
    /// unmark nothing at all.
    #[test]
    fn a_mark_is_checked_against_the_cached_commits_and_paths() {
        let mut gone = file("gone.rs");
        gone.new_path = None;
        let read = detail(HEAD, &[file("a.rs"), gone]);
        let files = [patched(b"a"), patched(b"gone")];
        let marking = |path: &str| {
            marks(MarkWrite::MarkFile {
                file: key_of(&read, &files, path),
                hunks: None,
            })
        };
        let rule = no_rule();
        assert_eq!(
            file_write(&read, &files, "a.rs", true, HEAD, BASE, &rule),
            Ok(marking("a.rs"))
        );
        assert_eq!(
            file_write(&read, &files, "gone.rs", true, HEAD, BASE, &rule),
            Ok(marking("gone.rs"))
        );
        assert_eq!(
            file_write(&read, &files, "a.rs", false, HEAD, BASE, &rule),
            Ok(marks(MarkWrite::UnmarkFile))
        );
        for (path, head, base) in [
            ("a.rs", PUSHED, BASE),
            ("a.rs", HEAD, PUSHED),
            ("b.rs", HEAD, BASE),
        ] {
            for viewed in [true, false] {
                assert!(
                    file_write(&read, &files, path, viewed, head, base, &rule).is_err(),
                    "{path} at {head}..{base}"
                );
            }
        }
    }

    // ------------------------------------------------------------ states

    /// `pull-request-viewer`: *The header counts progress*: ten files, four
    /// viewed and one changed since viewed, count four of ten with one
    /// changed; the last mark's head dates the count.
    #[test]
    fn ten_files_with_four_viewed_and_one_changed_count_four_of_ten_with_one_changed() {
        let paths: Vec<String> = (0..10).map(|n| format!("f{n}.rs")).collect();
        let files: Vec<DiffFile> = paths.iter().map(|path| file(path)).collect();
        let cached: Vec<CachedFile> = paths.iter().map(|path| patched(path.as_bytes())).collect();
        let read = detail(PUSHED, &files);
        let mut stored = entry([]);
        for n in 0..4 {
            stored
                .files
                .insert(paths[n].clone(), file_key(&files[n], &cached[n], &read));
        }
        stored.files.insert(
            paths[4].clone(),
            file_key(&files[4], &patched(b"before the push"), &read),
        );
        let counted = progress(&read, &cached, Some(&stored), &no_rule());
        assert_eq!(
            (counted.viewed, counted.changed_since_viewed, counted.total),
            (4, 1, 10)
        );
        assert_eq!(counted.skipped, 0, "no skip pattern matches");
        assert_eq!(counted.last_marked_head.as_deref(), Some(HEAD));
        let states: Vec<FileReviewState> = counted.files.iter().map(|file| file.state).collect();
        let mut expected = vec![FileReviewState::Viewed; 4];
        expected.push(FileReviewState::ChangedSinceViewed);
        expected.extend([FileReviewState::Unviewed; 5]);
        assert_eq!(states, expected);
        assert_eq!(counted.files[9].path, "f9.rs");
    }

    #[test]
    fn with_nothing_stored_every_file_is_unviewed_and_nothing_dates_the_count() {
        let read = detail(HEAD, &[file("a.rs"), file("b.rs")]);
        let counted = progress(&read, &[patched(b"a"), bare()], None, &no_rule());
        assert_eq!(
            (counted.viewed, counted.changed_since_viewed, counted.total),
            (0, 0, 2)
        );
        assert_eq!(counted.last_marked_head, None);
        assert!(counted
            .files
            .iter()
            .all(|file| file.state == FileReviewState::Unviewed));
        assert!(counted.files[1].keyed_by_head);
    }

    // ------------------------------------------------------------ the store

    fn store_in(dir: &tempfile::TempDir) -> ReviewProgressStore {
        ReviewProgressStore::open(dir.path().join("review-progress.json"))
    }

    fn patch_key(text: &str) -> FileKey {
        FileKey::Patch(format!("sha256:{text}"))
    }

    /// A mark round-trips through disk, read back by a store opened afresh,
    /// in the shape the file is documented to hold.
    #[test]
    fn a_mark_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let acme = github(42).key();
        store_in(&dir)
            .mark(&acme, "src/a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        let reopened = store_in(&dir);
        assert_eq!(
            reopened.entry(&acme).unwrap(),
            Some(entry([("src/a.rs", patch_key("aa"))]))
        );
        assert_eq!(reopened.entry(&github(7).key()).unwrap(), None);
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("review-progress.json")).unwrap())
                .unwrap();
        assert_eq!(
            raw,
            json!({ "github/acme/api/42": {
                "lastMarkedHead": HEAD,
                "files": { "src/a.rs": { "patch": "sha256:aa" } },
                "touchedAt": NOW,
            } })
        );
    }

    /// The other two keys' stored shapes, which a later version must still
    /// read.
    #[test]
    fn every_key_is_stored_in_a_shape_a_later_version_reads() {
        let blob = FileKey::Blob {
            sha: "sha-1".to_string(),
            status: FileStatus::Renamed { similarity: None },
            previous: Some("old.png".to_string()),
            base_branch: "main".to_string(),
        };
        let head = FileKey::HeadCommit {
            commit: HEAD.to_string(),
            base_branch: "main".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&blob).unwrap(),
            json!({ "blob": {
                "sha": "sha-1",
                "status": { "kind": "renamed", "similarity": null },
                "previous": "old.png",
                "baseBranch": "main",
            } })
        );
        assert_eq!(
            serde_json::to_value(&head).unwrap(),
            json!({ "headCommit": { "commit": HEAD, "baseBranch": "main" } })
        );
        for key in [blob, head, patch_key("aa")] {
            let stored = serde_json::to_value(&key).unwrap();
            assert_eq!(FileKey::deserialize(&stored).unwrap(), key);
        }
    }

    /// `pull-request-viewer`: *A second process's earlier mark is kept*: a
    /// store re-reads the file before it writes, so another store's mark,
    /// made since, survives its own.
    #[test]
    fn a_second_stores_earlier_mark_is_kept_when_the_first_marks_another_file() {
        let dir = tempfile::tempdir().unwrap();
        let (desktop, served) = (store_in(&dir), store_in(&dir));
        let acme = github(42).key();
        desktop
            .mark(&acme, "a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        served
            .mark(&acme, "b.rs", patch_key("bb"), HEAD, NOW + 1)
            .unwrap();
        desktop
            .mark(&acme, "c.rs", patch_key("cc"), PUSHED, NOW + 2)
            .unwrap();
        let stored = desktop.entry(&acme).unwrap().unwrap();
        assert_eq!(
            stored.files.keys().collect::<Vec<_>>(),
            ["a.rs", "b.rs", "c.rs"]
        );
        assert_eq!(stored.last_marked_head.as_deref(), Some(PUSHED));
        assert_eq!(stored.touched_at, NOW + 2);
    }

    /// `pull-request-viewer`: *Unmarking creates nothing*. An unmark of a
    /// stored entry removes its file and touches it, and leaves the last
    /// mark's head, which only a mark advances.
    #[test]
    fn an_unmark_never_creates_an_entry_and_leaves_the_last_marked_head() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let acme = github(42).key();
        assert!(!store.unmark(&acme, "a.rs", NOW).unwrap());
        assert!(
            !dir.path().join("review-progress.json").exists(),
            "nothing stored"
        );

        store
            .mark(&acme, "a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        store
            .mark(&acme, "b.rs", patch_key("bb"), PUSHED, NOW + 1)
            .unwrap();
        assert!(store.unmark(&acme, "a.rs", NOW + 2).unwrap());
        let stored = store.entry(&acme).unwrap().unwrap();
        assert_eq!(stored.files.keys().collect::<Vec<_>>(), ["b.rs"]);
        assert_eq!(stored.last_marked_head.as_deref(), Some(PUSHED));
        assert_eq!(stored.touched_at, NOW + 2);
        assert!(!store.unmark(&github(7).key(), "a.rs", NOW).unwrap());
        assert_eq!(store.entry(&github(7).key()).unwrap(), None);
    }

    /// Each write is staged beside the store and renamed over it, leaving
    /// nothing else behind.
    #[test]
    fn a_write_leaves_only_the_store_in_its_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("config");
        let store = ReviewProgressStore::open(nested.join("review-progress.json"));
        let acme = github(42).key();
        store
            .mark(&acme, "a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        store
            .mark(&acme, "b.rs", patch_key("bb"), HEAD, NOW)
            .unwrap();
        let names: Vec<String> = fs::read_dir(&nested)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["review-progress.json"]);
    }

    /// A store that is not a JSON object reads as no marks, and the first
    /// write moves it aside, intact, before replacing it.
    #[test]
    fn an_unreadable_store_is_moved_aside_before_it_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review-progress.json");
        fs::write(&path, "not json").unwrap();
        let store = store_in(&dir);
        let acme = github(42).key();
        assert_eq!(store.entry(&acme).unwrap(), None);
        assert_eq!(store.prune(|_, _| true).unwrap(), Vec::new());
        assert_eq!(fs::read_to_string(&path).unwrap(), "not json", "untouched");

        store
            .mark(&acme, "a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("review-progress.json.corrupt-0")).unwrap(),
            "not json"
        );
        assert!(store.entry(&acme).unwrap().is_some());
    }

    /// An entry this version cannot read stays exactly as stored through
    /// another entry's write, and reads as nothing stored.
    #[test]
    fn an_entry_this_version_cannot_read_is_kept_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review-progress.json");
        let foreign = json!({ "lastMarkedHead": HEAD, "files": { "a.rs": { "future": 1 } }, "touchedAt": NOW });
        fs::write(&path, json!({ "github/acme/web/1": foreign }).to_string()).unwrap();
        let store = store_in(&dir);
        let web = PullRequestReference {
            repo: "web".to_string(),
            ..github(1)
        }
        .key();
        assert_eq!(store.entry(&web).unwrap(), None);
        assert!(!store.unmark(&web, "a.rs", NOW).unwrap());
        store
            .mark(&github(42).key(), "a.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["github/acme/web/1"], foreign);
    }

    /// A store that cannot be read at all, a directory in its place, is an
    /// error rather than no marks, and nothing is written over it.
    #[test]
    fn a_store_that_cannot_be_read_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("review-progress.json")).unwrap();
        let store = store_in(&dir);
        let acme = github(42).key();
        assert!(store.entry(&acme).is_err());
        assert!(store
            .mark(&acme, "a.rs", patch_key("aa"), HEAD, NOW)
            .is_err());
        assert!(store.unmark(&acme, "a.rs", NOW).is_err());
        assert!(store.prune(|_, _| true).is_err());
    }

    #[test]
    fn entry_names_spell_the_canonical_reference_and_read_back() {
        let spelt = PullRequestReference {
            owner: "ACME".to_string(),
            repo: "Api".to_string(),
            ..bitbucket(7)
        };
        assert_eq!(entry_name(&spelt.key()), "bitbucket/acme/api/7");
        assert_eq!(entry_name(&github(42).key()), "github/acme/api/42");
        for name in [
            "github/acme/api/42",
            "bitbucket/acme/api/7",
            "github/acme/a/b/9",
        ] {
            let read = reference_of(name).unwrap();
            assert_eq!(entry_name(&read.key()), name);
        }
        assert_eq!(reference_of("github/acme/a/b/9").unwrap().repo, "a/b");
        for foreign in [
            "gitlab/acme/api/1",
            "github/acme/api/x",
            "github/acme/1",
            "github/1",
            "github",
            "",
        ] {
            assert_eq!(reference_of(foreign), None, "{foreign}");
        }
    }

    // ------------------------------------------------------------ hunks

    /// What the cache keeps of a file with patch text whose hunks digest to
    /// `[d; 32]` for each `d` of `digests`, in order.
    fn hunked(patch: &[u8], digests: &[u8]) -> CachedFile {
        CachedFile {
            hunks: Some(digests.iter().map(|&digest| [digest; 32]).collect()),
            ..patched(patch)
        }
    }

    /// The key of the `n`th hunk (from one) whose body digests to `[d; 32]`.
    fn hunk(digest: u8, n: u32) -> String {
        format!("sha256:{}#{n}", hex(&[digest; 32]))
    }

    fn set(keys: &[String]) -> BTreeSet<String> {
        keys.iter().cloned().collect()
    }

    /// The progress of the one file `a.rs`, read at `head` with `cached`
    /// beside it, against `stored`.
    fn one(head: &str, cached: CachedFile, stored: Option<&Entry>) -> FileReviewProgress {
        progress(
            &detail(head, &[file("a.rs")]),
            &[cached],
            stored,
            &no_rule(),
        )
        .files[0]
            .clone()
    }

    /// An entry holding `file` as the key of `a.rs`, when there is one, and
    /// `hunks` as its hunk keys.
    fn marked(file: Option<FileKey>, hunks: &[String]) -> Entry {
        let mut stored = entry(file.map(|key| ("a.rs", key)));
        if !hunks.is_empty() {
            stored.hunks.insert("a.rs".to_string(), set(hunks));
        }
        stored
    }

    /// `pull-request-viewer`: *Identical hunks are marked apart*: the full
    /// hex digest, numbered among the file's identical bodies in order. A
    /// file with no digests, or none at all, has no known hunks.
    #[test]
    fn hunk_keys_number_identical_bodies_apart_and_need_a_hunk() {
        assert_eq!(
            hunk_keys(&hunked(b"p", &[1, 2, 1, 1])),
            Some(vec![hunk(1, 1), hunk(2, 1), hunk(1, 2), hunk(1, 3)])
        );
        assert_eq!(
            hunk(0xab, 1),
            format!("sha256:{}#1", "ab".repeat(32)),
            "64 hex digits"
        );
        assert_eq!(hunk_keys(&hunked(b"p", &[])), None);
        assert_eq!(hunk_keys(&patched(b"p")), None);
    }

    /// D3, one row each, from a file with three hunks and nothing stored, or
    /// as stated.
    #[test]
    fn each_write_replaces_a_files_marks_by_its_row() {
        let (k, hunks) = (patch_key("now"), vec![hunk(1, 1), hunk(2, 1), hunk(3, 1)]);
        let nothing = Marks::default();
        let mark_file = MarkWrite::MarkFile {
            file: k.clone(),
            hunks: Some(hunks.clone()),
        };
        assert_eq!(
            mark_file.apply(nothing.clone()),
            Marks {
                file: Some(k.clone()),
                hunks: set(&hunks)
            }
        );
        // Hunks not known: the stored ones stay as they are.
        let unknown = MarkWrite::MarkFile {
            file: k.clone(),
            hunks: None,
        };
        let earlier = Marks {
            file: None,
            hunks: set(&[hunk(9, 1)]),
        };
        assert_eq!(
            unknown.apply(earlier.clone()),
            Marks {
                file: Some(k.clone()),
                hunks: earlier.hunks.clone()
            }
        );
        let whole = Marks {
            file: Some(k.clone()),
            hunks: set(&hunks),
        };
        assert_eq!(MarkWrite::UnmarkFile.apply(whole.clone()), nothing);

        let mark = |key: &String| MarkWrite::MarkHunk {
            key: key.clone(),
            file: k.clone(),
            hunks: hunks.clone(),
        };
        let unmark = |key: &String| MarkWrite::UnmarkHunk {
            key: key.clone(),
            file: k.clone(),
            hunks: hunks.clone(),
        };
        let first = mark(&hunks[1]).apply(nothing.clone());
        assert_eq!(
            first,
            Marks {
                file: None,
                hunks: set(&hunks[1..2])
            }
        );
        let second = mark(&hunks[0]).apply(first);
        assert_eq!(second.file, None, "the third is still unviewed");
        assert_eq!(
            mark(&hunks[2]).apply(second),
            whole,
            "the last one marks the file"
        );
        assert_eq!(
            unmark(&hunks[1]).apply(whole),
            Marks {
                file: None,
                hunks: set(&[hunks[0].clone(), hunks[2].clone()])
            }
        );
    }

    /// `pull-request-viewer`: *Unmarking a hunk keeps the others viewed*: a
    /// file marked whole before hunk marks existed stores its key and no
    /// hunk keys; unmarking one hunk writes the other hunks out first.
    #[test]
    fn unmarking_a_hunk_of_a_file_marked_whole_keeps_its_other_hunks() {
        let (k, hunks) = (
            patch_key("now"),
            vec![hunk(1, 1), hunk(2, 1), hunk(3, 1), hunk(4, 1)],
        );
        let before_hunk_marks = Marks {
            file: Some(k.clone()),
            hunks: BTreeSet::new(),
        };
        let after = MarkWrite::UnmarkHunk {
            key: hunks[1].clone(),
            file: k,
            hunks: hunks.clone(),
        }
        .apply(before_hunk_marks);
        assert_eq!(
            after,
            Marks {
                file: None,
                hunks: set(&[hunks[0].clone(), hunks[2].clone(), hunks[3].clone()])
            }
        );
    }

    /// A stale file key, a push since, stays through hunk marks, still saying
    /// changed since viewed, until the last hunk replaces it with the current
    /// key; and a stored key of a body the push changed is pruned by the
    /// first write (*Stored hunk keys follow the file's hunks*).
    #[test]
    fn a_stale_file_key_survives_hunk_marks_until_the_last_and_stale_hunk_keys_go() {
        let (stale, k) = (patch_key("before"), patch_key("now"));
        let hunks = vec![hunk(1, 1), hunk(2, 1), hunk(3, 1)];
        let pushed = Marks {
            file: Some(stale.clone()),
            hunks: set(&[hunks[0].clone(), hunk(7, 1)]),
        };
        let mark = |key: &String| MarkWrite::MarkHunk {
            key: key.clone(),
            file: k.clone(),
            hunks: hunks.clone(),
        };
        let once = mark(&hunks[1]).apply(pushed);
        assert_eq!(
            once,
            Marks {
                file: Some(stale),
                hunks: set(&hunks[..2])
            }
        );
        assert_eq!(
            mark(&hunks[2]).apply(once),
            Marks {
                file: Some(k),
                hunks: set(&hunks)
            }
        );
    }

    /// D4, case by case, for one file of three hunks.
    #[test]
    fn a_files_state_is_derived_from_its_keys_alone() {
        let cached = || hunked(b"now", &[1, 2, 3]);
        let k = key_at(HEAD, "a.rs", &cached());
        let all = [hunk(1, 1), hunk(2, 1), hunk(3, 1)];
        let at = |stored: Entry| one(HEAD, cached(), Some(&stored));

        let whole = at(marked(Some(k.clone()), &[]));
        assert_eq!(whole.state, FileReviewState::Viewed);
        assert_eq!(whole.hunks, Some(vec![true; 3]), "every hunk, by the file");

        let partly = at(marked(None, &all[1..2]));
        assert_eq!(partly.state, FileReviewState::PartlyViewed);
        assert_eq!(partly.hunks, Some(vec![false, true, false]));

        let changed = at(marked(Some(patch_key("before")), &all[..1]));
        assert_eq!(changed.state, FileReviewState::ChangedSinceViewed);
        assert_eq!(changed.hunks, Some(vec![true, false, false]));
        let all_changed = at(marked(Some(patch_key("before")), &[]));
        assert_eq!(all_changed.state, FileReviewState::ChangedSinceViewed);

        // Marked hunks whose bodies all changed hold nothing any more.
        let gone = at(marked(None, &[hunk(8, 1), hunk(9, 1)]));
        assert_eq!(gone.state, FileReviewState::Unviewed);
        assert_eq!(gone.hunks, Some(vec![false; 3]));

        let nothing = one(HEAD, cached(), None);
        assert_eq!(nothing.state, FileReviewState::Unviewed);
        assert_eq!(nothing.hunks, Some(vec![false; 3]));
    }

    /// `pull-request-viewer`: *A rebase that only moves a file's hunks keeps
    /// it viewed*: the moved headers change the patch, so the stored file
    /// key is stale, yet every body is one marked, so the file is viewed.
    #[test]
    fn a_rebase_that_only_moves_hunks_keeps_the_file_viewed() {
        let marked_at = hunked(b"@@ -1,2 +1,2 @@ body", &[1, 2]);
        let moved = hunked(b"@@ -40,2 +41,2 @@ body", &[1, 2]);
        let stored = marked(
            Some(key_at(HEAD, "a.rs", &marked_at)),
            &[hunk(1, 1), hunk(2, 1)],
        );
        assert_ne!(stored.files["a.rs"], key_at(PUSHED, "a.rs", &moved));
        let read = one(PUSHED, moved, Some(&stored));
        assert_eq!(read.state, FileReviewState::Viewed);
        assert_eq!(read.hunks, Some(vec![true, true]));
    }

    /// `pull-request-viewer`: *Identical hunks are marked apart*: the first
    /// of two identical hunks marked, the second is not viewed.
    #[test]
    fn of_two_identical_hunks_marking_the_first_leaves_the_second() {
        let read = one(
            HEAD,
            hunked(b"p", &[5, 5]),
            Some(&marked(None, &[hunk(5, 1)])),
        );
        assert_eq!(read.hunks, Some(vec![true, false]));
        assert_eq!(read.state, FileReviewState::PartlyViewed);
    }

    /// `pull-request-viewer`: *An unloaded file with hunk marks is partly
    /// viewed*, *A file without hunks keeps a file mark only*: with its
    /// hunks not known, a file's stored hunk keys make it partly viewed and
    /// its key alone viewed, and it carries no hunk states.
    #[test]
    fn a_file_whose_hunks_are_not_known_carries_no_hunk_states() {
        let unloaded = || blob("sha-1");
        let k = key_at(HEAD, "a.rs", &unloaded());
        let partly = one(HEAD, unloaded(), Some(&marked(None, &[hunk(1, 1)])));
        assert_eq!(
            (partly.state, partly.hunks),
            (FileReviewState::PartlyViewed, None)
        );
        let whole = one(HEAD, unloaded(), Some(&marked(Some(k), &[])));
        assert_eq!((whole.state, whole.hunks), (FileReviewState::Viewed, None));
        let nothing = one(HEAD, unloaded(), Some(&marked(None, &[])));
        assert_eq!(nothing.state, FileReviewState::Unviewed);
    }

    /// A partly viewed file counts as neither viewed nor changed, and the
    /// answer names the commits its hunk states belong to (*Hunk states name
    /// their detail*).
    #[test]
    fn the_counts_leave_a_partly_viewed_file_out_and_the_commits_are_named() {
        let read = detail(HEAD, &[file("a.rs"), file("b.rs")]);
        let cached = [hunked(b"a", &[1, 2]), hunked(b"b", &[3])];
        let mut stored = marked(None, &[hunk(1, 1)]);
        stored
            .files
            .insert("b.rs".to_string(), key_of(&read, &cached, "b.rs"));
        let counted = progress(&read, &cached, Some(&stored), &no_rule());
        assert_eq!(
            counted
                .files
                .iter()
                .map(|file| file.state)
                .collect::<Vec<_>>(),
            [FileReviewState::PartlyViewed, FileReviewState::Viewed]
        );
        assert_eq!(
            (counted.viewed, counted.changed_since_viewed, counted.total),
            (1, 0, 2)
        );
        assert_eq!(
            (counted.head_commit.as_str(), counted.base_commit.as_str()),
            (HEAD, BASE)
        );
    }

    /// `pull-request-viewer`: *A hunk mark needs the file's hunks*, *A hunk
    /// past the last is refused*: beside a file mark's refusals, a hunk mark
    /// needs known hunks and an index among them; it carries its key, its
    /// file's and every hunk's.
    #[test]
    fn a_hunk_write_is_checked_and_carries_its_keys() {
        let mut gone = file("gone.rs");
        gone.new_path = None;
        let read = detail(HEAD, &[file("a.rs"), file("logo.png"), gone]);
        let files = [hunked(b"a", &[1, 2]), blob("sha-1"), hunked(b"gone", &[3])];
        let hunks = vec![hunk(1, 1), hunk(2, 1)];
        let rule = no_rule();
        assert_eq!(
            hunk_write(&read, &files, "a.rs", 1, true, HEAD, BASE, &rule),
            Ok(marks(MarkWrite::MarkHunk {
                key: hunk(2, 1),
                file: key_of(&read, &files, "a.rs"),
                hunks: hunks.clone(),
            }))
        );
        assert_eq!(
            hunk_write(&read, &files, "a.rs", 0, false, HEAD, BASE, &rule),
            Ok(marks(MarkWrite::UnmarkHunk {
                key: hunk(1, 1),
                file: key_of(&read, &files, "a.rs"),
                hunks,
            }))
        );
        assert!(hunk_write(&read, &files, "gone.rs", 0, true, HEAD, BASE, &rule).is_ok());
        for (path, at, head, base) in [
            ("a.rs", 2, HEAD, BASE),
            ("logo.png", 0, HEAD, BASE),
            ("a.rs", 0, PUSHED, BASE),
            ("a.rs", 0, HEAD, PUSHED),
            ("b.rs", 0, HEAD, BASE),
        ] {
            for viewed in [true, false] {
                assert!(
                    hunk_write(&read, &files, path, at, viewed, head, base, &rule).is_err(),
                    "{path} hunk {at} at {head}..{base}"
                );
            }
        }
        // A file mark carries every hunk's key once they are known.
        assert_eq!(
            file_write(&read, &files, "a.rs", true, HEAD, BASE, &rule),
            Ok(marks(MarkWrite::MarkFile {
                file: key_of(&read, &files, "a.rs"),
                hunks: Some(vec![hunk(1, 1), hunk(2, 1)]),
            }))
        );
    }

    /// Hunk marks round-trip through disk under `hunks`, by path; a hunk mark
    /// advances `lastMarkedHead` and a hunk unmark does not, nor creates an
    /// entry; a path whose hunks are all gone leaves the map, and an empty
    /// map leaves the entry.
    #[test]
    fn hunk_marks_round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let acme = github(42).key();
        let hunks = vec![hunk(1, 1), hunk(2, 1)];
        let write = |key: &String, viewed: bool| {
            let (key, file, hunks) = (key.clone(), patch_key("aa"), hunks.clone());
            marks(if viewed {
                MarkWrite::MarkHunk { key, file, hunks }
            } else {
                MarkWrite::UnmarkHunk { key, file, hunks }
            })
        };
        assert!(!store
            .write_marks(&acme, "a.rs", &write(&hunks[0], false), HEAD, NOW)
            .unwrap());
        assert!(!dir.path().join("review-progress.json").exists());

        assert!(store
            .write_marks(&acme, "a.rs", &write(&hunks[0], true), HEAD, NOW)
            .unwrap());
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("review-progress.json")).unwrap())
                .unwrap();
        assert_eq!(
            raw,
            json!({ "github/acme/api/42": {
                "lastMarkedHead": HEAD,
                "files": {},
                "hunks": { "a.rs": [hunk(1, 1)] },
                "touchedAt": NOW,
            } })
        );
        assert!(store
            .write_marks(&acme, "a.rs", &write(&hunks[0], false), PUSHED, NOW + 1)
            .unwrap());
        let stored = store.entry(&acme).unwrap().unwrap();
        assert_eq!(
            stored.last_marked_head.as_deref(),
            Some(HEAD),
            "an unmark leaves it"
        );
        assert_eq!(stored.touched_at, NOW + 1);
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("review-progress.json")).unwrap())
                .unwrap();
        assert_eq!(raw["github/acme/api/42"].get("hunks"), None);
    }

    /// `pull-request-viewer`: *An entry from before hunk marks reads
    /// unchanged*: an entry without `hunks` reads as one with none, and its
    /// files keep the states they had.
    #[test]
    fn an_entry_from_before_hunk_marks_reads_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let cached = hunked(b"now", &[1, 2]);
        let k = key_at(HEAD, "a.rs", &cached);
        fs::write(
            dir.path().join("review-progress.json"),
            json!({ "github/acme/api/42": {
                "lastMarkedHead": HEAD,
                "files": { "a.rs": k },
                "touchedAt": NOW,
            } })
            .to_string(),
        )
        .unwrap();
        let stored = store_in(&dir).entry(&github(42).key()).unwrap().unwrap();
        assert!(stored.hunks.is_empty());
        let read = one(HEAD, cached, Some(&stored));
        assert_eq!(
            (read.state, read.hunks),
            (FileReviewState::Viewed, Some(vec![true, true]))
        );
    }

    // ------------------------------------------------------------ pruning

    fn github_list(
        status: PullRequestsStatus,
        stale: bool,
        listed: &[u64],
    ) -> GithubPullRequestsState {
        GithubPullRequestsState {
            status,
            stale,
            fetched_at_unix: Some(NOW),
            authored: listed.iter().map(|&number| row(number)).collect(),
            review_requested: Vec::new(),
            withheld: 0,
        }
    }

    fn bitbucket_list(
        status: PullRequestsStatus,
        stale: bool,
        listed: &[u64],
    ) -> BitbucketPullRequestsState {
        BitbucketPullRequestsState {
            status,
            stale,
            fetched_at_unix: Some(NOW),
            pull_requests: listed
                .iter()
                .map(|&number| PullRequestSummary {
                    url: format!("https://bitbucket.org/acme/api/pull-requests/{number}"),
                    ..row(number)
                })
                .collect(),
            skipped_workspaces: Vec::new(),
        }
    }

    fn refreshed_github(listed: &[u64]) -> GithubPullRequestsState {
        github_list(PullRequestsStatus::Ok, false, listed)
    }

    fn refreshed_bitbucket(listed: &[u64]) -> BitbucketPullRequestsState {
        bitbucket_list(PullRequestsStatus::Ok, false, listed)
    }

    fn lists<'a>(
        github: (bool, &'a GithubPullRequestsState),
        bitbucket: (bool, &'a BitbucketPullRequestsState),
    ) -> Lists<'a> {
        Lists {
            github_enabled: github.0,
            bitbucket_enabled: bitbucket.0,
            github: github.1,
            bitbucket: bitbucket.1,
        }
    }

    /// `pull-request-viewer`: *Pruning waits for the lists*. At load every
    /// snapshot is still disabled, so a 120-day entry survives; once the
    /// lists arrive without it, it is pruned, and a listed one survives.
    #[test]
    fn an_entry_untouched_for_120_days_survives_load_and_goes_once_the_lists_arrive_without_it() {
        let old = NOW - 120 * DAY;
        let (loading, off) = (
            GithubPullRequestsState::disabled(),
            BitbucketPullRequestsState::disabled(),
        );
        let at_load = lists((true, &loading), (true, &off));
        assert!(!at_load.prunes(&github(42), old, NOW));
        assert!(!at_load.arrived());

        let (github_rows, bitbucket_rows) = (refreshed_github(&[7]), refreshed_bitbucket(&[9]));
        let arrived = lists((true, &github_rows), (true, &bitbucket_rows));
        assert!(arrived.arrived());
        assert!(arrived.prunes(&github(42), old, NOW));
        assert!(arrived.prunes(&bitbucket(42), old, NOW));
        assert!(!arrived.prunes(&github(7), old, NOW), "listed");
        assert!(!arrived.prunes(&bitbucket(9), old, NOW), "listed");
    }

    /// The age to the minute: exactly 90 days is pruned, a minute less is
    /// kept.
    #[test]
    fn an_entry_untouched_exactly_90_days_is_pruned_and_one_a_minute_younger_is_kept() {
        assert_eq!(PRUNE_AFTER_SECS, 90 * DAY);
        let (github_rows, bitbucket_rows) = (refreshed_github(&[]), refreshed_bitbucket(&[]));
        let arrived = lists((true, &github_rows), (true, &bitbucket_rows));
        assert!(arrived.prunes(&github(42), NOW - 90 * DAY, NOW));
        assert!(!arrived.prunes(&github(42), NOW - 90 * DAY + 60, NOW));
        assert!(!arrived.prunes(&github(42), NOW + 60, NOW), "touched later");
    }

    /// `pull-request-viewer`: *Switching every provider off prunes nothing*.
    #[test]
    fn with_both_providers_off_a_120_day_entry_is_kept() {
        let (github_rows, bitbucket_rows) = (
            GithubPullRequestsState::disabled(),
            BitbucketPullRequestsState::disabled(),
        );
        let off = lists((false, &github_rows), (false, &bitbucket_rows));
        for reference in [github(42), bitbucket(42)] {
            assert!(
                !off.prunes(&reference, NOW - 120 * DAY, NOW),
                "{reference:?}"
            );
        }
    }

    /// `pull-request-viewer`: *A disabled provider's entries are kept*:
    /// BitBucket off and GitHub refreshed, a 120-day BitBucket entry is kept
    /// while an unlisted GitHub one goes.
    #[test]
    fn a_disabled_providers_entry_is_kept_while_the_others_list_prunes() {
        let (github_rows, bitbucket_rows) = (
            refreshed_github(&[]),
            BitbucketPullRequestsState::disabled(),
        );
        let bitbucket_off = lists((true, &github_rows), (false, &bitbucket_rows));
        assert!(!bitbucket_off.prunes(&bitbucket(42), NOW - 120 * DAY, NOW));
        assert!(bitbucket_off.prunes(&github(42), NOW - 120 * DAY, NOW));

        let (github_rows, bitbucket_rows) = (
            GithubPullRequestsState::disabled(),
            refreshed_bitbucket(&[]),
        );
        let github_off = lists((false, &github_rows), (true, &bitbucket_rows));
        assert!(!github_off.prunes(&github(42), NOW - 120 * DAY, NOW));
        assert!(github_off.prunes(&bitbucket(42), NOW - 120 * DAY, NOW));
    }

    /// Only a successful, non-stale refresh counts as a list: a stale one,
    /// a fresh `unavailable` one with no rows, or one provider's list still
    /// missing holds every entry back, its own provider's included.
    #[test]
    fn a_list_that_is_stale_unavailable_or_missing_prunes_nothing() {
        let old = NOW - 120 * DAY;
        let refreshed = refreshed_bitbucket(&[]);
        for github_rows in [
            github_list(PullRequestsStatus::Ok, true, &[]),
            github_list(PullRequestsStatus::Unavailable, false, &[]),
            github_list(PullRequestsStatus::Unauthenticated, false, &[]),
        ] {
            let held = lists((true, &github_rows), (true, &refreshed));
            assert!(!held.arrived(), "{github_rows:?}");
            assert!(!held.prunes(&github(42), old, NOW), "{github_rows:?}");
            assert!(!held.prunes(&bitbucket(42), old, NOW), "{github_rows:?}");
        }
        let github_rows = refreshed_github(&[]);
        for bitbucket_rows in [
            bitbucket_list(PullRequestsStatus::Ok, true, &[]),
            bitbucket_list(PullRequestsStatus::Unavailable, false, &[]),
        ] {
            let held = lists((true, &github_rows), (true, &bitbucket_rows));
            assert!(!held.prunes(&github(42), old, NOW), "{bitbucket_rows:?}");
        }
    }

    /// `pull-request-viewer`: *An incomplete list prunes nothing of its
    /// provider*. A GitHub refresh that withheld results, as from an
    /// organisation blocked by single sign-on, keeps a 120-day GitHub entry
    /// it does not list, until a refresh that withholds nothing leaves it
    /// out; a single withheld result is enough to hold it. It holds GitHub's
    /// entries only: BitBucket's complete list prunes as before.
    #[test]
    fn a_github_list_with_results_withheld_prunes_nothing_of_github() {
        let old = NOW - 120 * DAY;
        let bitbucket_rows = refreshed_bitbucket(&[]);
        let withholding = GithubPullRequestsState {
            withheld: 1,
            ..refreshed_github(&[7])
        };
        let incomplete = lists((true, &withholding), (true, &bitbucket_rows));
        assert!(!incomplete.prunes(&github(42), old, NOW));
        assert!(
            incomplete.prunes(&bitbucket(42), old, NOW),
            "the other provider's own list is complete"
        );

        let withholding_nothing = refreshed_github(&[7]);
        assert_eq!(withholding_nothing.withheld, 0);
        let complete = lists((true, &withholding_nothing), (true, &bitbucket_rows));
        assert!(complete.prunes(&github(42), old, NOW));
        assert!(!complete.prunes(&github(7), old, NOW), "listed");
    }

    /// The BitBucket twin: a refresh that skipped a workspace keeps every
    /// unlisted BitBucket entry, and holds back none of GitHub's; one that
    /// skipped none prunes as before.
    #[test]
    fn a_bitbucket_list_with_a_skipped_workspace_prunes_nothing_of_bitbucket() {
        let old = NOW - 120 * DAY;
        let github_rows = refreshed_github(&[]);
        let skipping = BitbucketPullRequestsState {
            skipped_workspaces: vec!["locked-out".to_string()],
            ..refreshed_bitbucket(&[9])
        };
        let incomplete = lists((true, &github_rows), (true, &skipping));
        assert!(!incomplete.prunes(&bitbucket(42), old, NOW));
        assert!(
            incomplete.prunes(&github(42), old, NOW),
            "the other provider's own list is complete"
        );

        let skipping_none = refreshed_bitbucket(&[9]);
        assert!(skipping_none.skipped_workspaces.is_empty());
        let complete = lists((true, &github_rows), (true, &skipping_none));
        assert!(complete.prunes(&bitbucket(42), old, NOW));
        assert!(!complete.prunes(&bitbucket(9), old, NOW), "listed");
    }

    /// The store prunes what the rule names, keeps an entry whose name or
    /// time it cannot read, returns the references it pruned, and writes
    /// only when it pruned one.
    #[test]
    fn the_store_prunes_what_the_rule_names_and_writes_only_then() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review-progress.json");
        let store = store_in(&dir);
        assert_eq!(store.prune(|_, _| true).unwrap(), Vec::new());
        assert!(!path.exists(), "nothing to prune, nothing written");

        for (number, touched) in [(1, NOW - 120 * DAY), (2, NOW), (3, NOW - 120 * DAY)] {
            store
                .mark(
                    &github(number).key(),
                    "a.rs",
                    patch_key("aa"),
                    HEAD,
                    touched,
                )
                .unwrap();
        }
        let mut raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        raw["not a reference"] = json!({ "touchedAt": 0 });
        raw["github/acme/api/4"] = json!({ "files": {} });
        fs::write(&path, raw.to_string()).unwrap();

        let pruned = store
            .prune(|reference, touched| reference.number != 3 && touched <= NOW - 90 * DAY)
            .unwrap();
        assert_eq!(pruned, [github(1)]);
        let kept: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mut names: Vec<&String> = kept.as_object().unwrap().keys().collect();
        names.sort();
        assert_eq!(
            names,
            [
                "github/acme/api/2",
                "github/acme/api/3",
                "github/acme/api/4",
                "not a reference"
            ]
        );

        // Nothing named: the file is not rewritten.
        fs::write(&path, "{ \"github/acme/api/2\": { \"touchedAt\": 1 } }").unwrap();
        assert_eq!(store.prune(|_, _| false).unwrap(), Vec::new());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{ \"github/acme/api/2\": { \"touchedAt\": 1 } }"
        );
    }

    /// Through the rule: an incomplete set of lists prunes nothing, and a
    /// complete one prunes the unlisted, untouched entry only.
    #[test]
    fn the_rule_prunes_the_store_once_the_lists_have_arrived() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        for number in [1, 2] {
            store
                .mark(
                    &github(number).key(),
                    "a.rs",
                    patch_key("aa"),
                    HEAD,
                    NOW - 120 * DAY,
                )
                .unwrap();
        }
        let (loading, listed, off) = (
            GithubPullRequestsState::disabled(),
            refreshed_github(&[2]),
            BitbucketPullRequestsState::disabled(),
        );
        let at_load = lists((true, &loading), (false, &off));
        assert_eq!(at_load.prune(&store, NOW).unwrap(), Vec::new());
        assert!(store.entry(&github(1).key()).unwrap().is_some());

        let withholding = GithubPullRequestsState {
            withheld: 2,
            ..listed.clone()
        };
        let incomplete = lists((true, &withholding), (false, &off));
        assert_eq!(incomplete.prune(&store, NOW).unwrap(), Vec::new());
        assert!(store.entry(&github(1).key()).unwrap().is_some());

        let arrived = lists((true, &listed), (false, &off));
        assert_eq!(arrived.prune(&store, NOW).unwrap(), [github(1)]);
        assert_eq!(store.entry(&github(1).key()).unwrap(), None);
        assert!(store.entry(&github(2).key()).unwrap().is_some());
    }

    // ------------------------------------------------------------ skip patterns

    /// A test file, under a directory `**/tests/**` matches.
    const TEST: &str = "crates/core/tests/parse.rs";

    /// The progress of the one file `a.rs`, read at `HEAD` with `cached`
    /// beside it, against `stored`, under `rule`.
    fn one_under(
        rule: &SkipRule,
        cached: CachedFile,
        stored: Option<&Entry>,
    ) -> FileReviewProgress {
        progress(&detail(HEAD, &[file("a.rs")]), &[cached], stored, rule).files[0].clone()
    }

    /// A file's state, matching pattern and inclusion, as one tuple.
    fn seen(file: &FileReviewProgress) -> (FileReviewState, Option<&str>, bool) {
        (file.state, file.matched.as_deref(), file.included)
    }

    /// `pull-request-viewer`: *A matching file with nothing stored is
    /// skipped*: it names its pattern and is not included, whether there is
    /// no entry or one that stores nothing of it; a file no pattern matches
    /// is unviewed and names none.
    #[test]
    fn a_matching_file_with_nothing_stored_is_skipped_naming_its_pattern() {
        let rule = skipping(&["**/tests/**"]);
        let read = detail(HEAD, &[file(TEST), file("src/lib.rs")]);
        let cached = [patched(b"test"), patched(b"lib")];
        let empty = entry([]);
        for stored in [None, Some(&empty)] {
            let counted = progress(&read, &cached, stored, &rule);
            assert_eq!(
                seen(&counted.files[0]),
                (FileReviewState::Skipped, Some("**/tests/**"), false)
            );
            assert_eq!(
                seen(&counted.files[1]),
                (FileReviewState::Unviewed, None, false)
            );
            assert_eq!((counted.viewed, counted.skipped, counted.total), (0, 1, 2));
        }
    }

    /// D4 under a pattern that matches, case by case for one file of three
    /// hunks: the reader's marks give the state they give, a file with
    /// nothing that holds is skipped, and an included one is unviewed. A
    /// file no pattern matches is unviewed, included or not.
    #[test]
    fn each_mark_outranks_the_pattern_and_an_inclusion_unskips() {
        let rule = skipping(&["*.rs"]);
        let cached = || hunked(b"now", &[1, 2, 3]);
        let k = key_at(HEAD, "a.rs", &cached());
        let at = |stored: &Entry| one_under(&rule, cached(), Some(stored));

        let viewed = at(&marked(Some(k), &[]));
        assert_eq!(
            seen(&viewed),
            (FileReviewState::Viewed, Some("*.rs"), false)
        );
        let changed = at(&marked(Some(patch_key("before")), &[]));
        assert_eq!(changed.state, FileReviewState::ChangedSinceViewed);
        let partly = at(&marked(None, &[hunk(2, 1)]));
        assert_eq!(partly.state, FileReviewState::PartlyViewed);
        let unloaded = one_under(&rule, blob("sha-1"), Some(&marked(None, &[hunk(1, 1)])));
        assert_eq!(
            unloaded.state,
            FileReviewState::PartlyViewed,
            "hunks not known"
        );

        let skipped = at(&marked(None, &[]));
        assert_eq!(
            seen(&skipped),
            (FileReviewState::Skipped, Some("*.rs"), false)
        );
        assert_eq!(
            skipped.hunks,
            Some(vec![false; 3]),
            "its hunks still carry states"
        );
        // Hunk keys whose bodies all changed hold nothing, as for any file.
        let gone = at(&marked(None, &[hunk(8, 1)]));
        assert_eq!(gone.state, FileReviewState::Skipped);

        let mut including = marked(None, &[]);
        including.included.insert("a.rs".to_string());
        assert_eq!(
            seen(&at(&including)),
            (FileReviewState::Unviewed, Some("*.rs"), true)
        );
        assert_eq!(
            seen(&one_under(&no_rule(), cached(), Some(&including))),
            (FileReviewState::Unviewed, None, true)
        );
        // Another path's inclusion is not this one's.
        let mut elsewhere = marked(None, &[]);
        elsewhere.included.insert("b.rs".to_string());
        assert_eq!(at(&elsewhere).state, FileReviewState::Skipped);
    }

    /// `pull-request-viewer`: *An older entry's marks outrank the patterns*:
    /// an entry from before skip patterns keeps `src/app.test.ts` viewed
    /// while its key holds, and changed since viewed, never skipped, once a
    /// push changes it.
    #[test]
    fn an_older_entrys_key_outranks_the_pattern_before_and_after_a_push() {
        let rule = skipping(&["*.test.{js,jsx,ts,tsx,mjs,cjs}"]);
        let path = "src/app.test.ts";
        let stored = entry([(path, key_at(HEAD, path, &patched(b"+one\n")))]);
        let viewed = progress(
            &detail(HEAD, &[file(path)]),
            &[patched(b"+one\n")],
            Some(&stored),
            &rule,
        );
        assert_eq!(viewed.files[0].state, FileReviewState::Viewed);
        let pushed = progress(
            &detail(PUSHED, &[file(path)]),
            &[patched(b"+two\n")],
            Some(&stored),
            &rule,
        );
        assert_eq!(
            seen(&pushed.files[0]),
            (
                FileReviewState::ChangedSinceViewed,
                Some("*.test.{js,jsx,ts,tsx,mjs,cjs}"),
                false
            )
        );
        assert_eq!((pushed.changed_since_viewed, pushed.skipped), (1, 0));
    }

    /// `pull-request-viewer`: *A push never brings a skipped file back*: with
    /// nothing stored of it, a push that changes its patch leaves it skipped
    /// and out of the changed count.
    #[test]
    fn a_push_leaves_a_skipped_file_skipped_and_uncounted() {
        let rule = skipping(&["**/tests/**"]);
        let stored = entry([("src/lib.rs", key_at(HEAD, "src/lib.rs", &patched(b"lib")))]);
        for (head, patch) in [(HEAD, b"+one\n"), (PUSHED, b"+two\n")] {
            let read = progress(
                &detail(head, &[file(TEST), file("src/lib.rs")]),
                &[patched(patch), patched(b"lib")],
                Some(&stored),
                &rule,
            );
            assert_eq!(read.files[0].state, FileReviewState::Skipped, "at {head}");
            assert_eq!(
                (read.viewed, read.changed_since_viewed, read.skipped),
                (1, 0, 1)
            );
        }
    }

    /// `pull-request-viewer`: *Skipped files are counted apart*: ten files,
    /// three skipped and four viewed, count four viewed and three skipped of
    /// ten, none changed.
    #[test]
    fn ten_files_with_three_skipped_and_four_viewed_count_them_apart() {
        let rule = skipping(&["**/tests/**"]);
        let paths: Vec<String> = (0..10)
            .map(|n| match n {
                0..=2 => format!("tests/t{n}.rs"),
                _ => format!("src/f{n}.rs"),
            })
            .collect();
        let files: Vec<DiffFile> = paths.iter().map(|path| file(path)).collect();
        let cached: Vec<CachedFile> = paths.iter().map(|path| patched(path.as_bytes())).collect();
        let read = detail(HEAD, &files);
        let mut stored = entry([]);
        for n in 3..7 {
            stored
                .files
                .insert(paths[n].clone(), file_key(&files[n], &cached[n], &read));
        }
        let counted = progress(&read, &cached, Some(&stored), &rule);
        assert_eq!(
            (
                counted.viewed,
                counted.changed_since_viewed,
                counted.skipped,
                counted.total
            ),
            (4, 0, 3, 10)
        );
        let skipped: Vec<&str> = counted
            .files
            .iter()
            .filter(|file| file.state == FileReviewState::Skipped)
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(skipped, ["tests/t0.rs", "tests/t1.rs", "tests/t2.rs"]);
    }

    /// `pull-request-viewer`: *A renamed test file is still matched*: by its
    /// old path, and named by its new one, its key path.
    #[test]
    fn a_renamed_test_file_is_skipped_by_its_old_path() {
        let rule = skipping(&["**/tests/**"]);
        let renamed = DiffFile {
            old_path: Some("tests/parse.rs".to_string()),
            status: FileStatus::Renamed { similarity: None },
            ..file("checks/parse.rs")
        };
        let read = progress(&detail(HEAD, &[renamed]), &[patched(b"r")], None, &rule);
        assert_eq!(read.files[0].path, "checks/parse.rs");
        assert_eq!(
            seen(&read.files[0]),
            (FileReviewState::Skipped, Some("**/tests/**"), false)
        );
    }

    /// `pull-request-viewer`: *Including a skipped file*, *Inclusion survives
    /// a push*: included by path, the file is unviewed and still names its
    /// pattern, at the head it was included at and after a push changes its
    /// patch.
    #[test]
    fn an_included_file_is_unviewed_naming_its_pattern_across_a_push() {
        let rule = skipping(&["**/tests/**"]);
        let mut stored = entry([]);
        stored.included.insert(TEST.to_string());
        for (head, patch) in [(HEAD, b"+one\n"), (PUSHED, b"+two\n")] {
            let read = progress(
                &detail(head, &[file(TEST)]),
                &[patched(patch)],
                Some(&stored),
                &rule,
            );
            assert_eq!(
                seen(&read.files[0]),
                (FileReviewState::Unviewed, Some("**/tests/**"), true),
                "at {head}"
            );
            assert_eq!(read.skipped, 0);
        }
    }

    /// An inclusion is refused in every case a file's mark is, and carries
    /// only whether it includes, whatever the patterns.
    #[test]
    fn an_inclusion_is_checked_against_the_cached_commits_and_paths() {
        let mut gone = file("gone.rs");
        gone.new_path = None;
        let read = detail(HEAD, &[file("a.rs"), gone]);
        let files = [patched(b"a"), patched(b"gone")];
        assert_eq!(
            include_write(&read, &files, "a.rs", true, HEAD, BASE),
            Ok(ReviewWrite::Include(true))
        );
        assert_eq!(
            include_write(&read, &files, "gone.rs", false, HEAD, BASE),
            Ok(ReviewWrite::Include(false))
        );
        for (path, head, base) in [
            ("a.rs", PUSHED, BASE),
            ("a.rs", HEAD, PUSHED),
            ("b.rs", HEAD, BASE),
        ] {
            for included in [true, false] {
                assert!(
                    include_write(&read, &files, path, included, head, base).is_err(),
                    "{path} at {head}..{base}"
                );
            }
        }
    }

    /// Design D3: a mark or an unmark, of the file or of one of its hunks,
    /// includes a file a pattern matches, a renamed one by its old path, and
    /// no other.
    #[test]
    fn a_mark_or_unmark_of_a_matching_file_includes_it() {
        let rule = skipping(&["**/tests/**"]);
        let renamed = DiffFile {
            old_path: Some("tests/old.rs".to_string()),
            status: FileStatus::Renamed { similarity: None },
            ..file("checks/new.rs")
        };
        let read = detail(HEAD, &[file(TEST), file("src/lib.rs"), renamed]);
        let files = [hunked(b"t", &[1]), hunked(b"l", &[2]), patched(b"r")];
        let includes = |write: Result<ReviewWrite, String>| match write.unwrap() {
            ReviewWrite::Marks { include, .. } => include,
            other => panic!("a marks write, got {other:?}"),
        };
        for viewed in [true, false] {
            assert!(includes(file_write(
                &read, &files, TEST, viewed, HEAD, BASE, &rule
            )));
            assert!(includes(hunk_write(
                &read, &files, TEST, 0, viewed, HEAD, BASE, &rule
            )));
            assert!(
                includes(file_write(
                    &read,
                    &files,
                    "checks/new.rs",
                    viewed,
                    HEAD,
                    BASE,
                    &rule
                )),
                "by its old path"
            );
            assert!(!includes(file_write(
                &read,
                &files,
                "src/lib.rs",
                viewed,
                HEAD,
                BASE,
                &rule
            )));
            assert!(!includes(hunk_write(
                &read,
                &files,
                "src/lib.rs",
                0,
                viewed,
                HEAD,
                BASE,
                &rule
            )));
            assert!(!includes(file_write(
                &read,
                &files,
                TEST,
                viewed,
                HEAD,
                BASE,
                &no_rule()
            )));
        }
    }

    /// `included` round-trips through disk by path and is left out while
    /// empty; an inclusion alone stores no `lastMarkedHead`; and an entry
    /// from before skip patterns, with no `included`, reads as including
    /// nothing (*Two writers*).
    #[test]
    fn inclusions_round_trip_through_disk_and_an_older_entry_includes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("review-progress.json");
        let store = store_in(&dir);
        let acme = github(42).key();
        let raw = || -> Value { serde_json::from_slice(&fs::read(&path).unwrap()).unwrap() };

        assert!(store
            .write_marks(&acme, TEST, &ReviewWrite::Include(true), HEAD, NOW)
            .unwrap());
        assert_eq!(
            raw(),
            json!({ "github/acme/api/42": {
                "files": {},
                "included": [TEST],
                "touchedAt": NOW,
            } })
        );
        let stored = store.entry(&acme).unwrap().unwrap();
        assert_eq!(stored.last_marked_head, None);
        assert_eq!(stored.included, BTreeSet::from([TEST.to_string()]));

        assert!(store
            .write_marks(&acme, TEST, &ReviewWrite::Include(false), HEAD, NOW + 1)
            .unwrap());
        assert_eq!(raw()["github/acme/api/42"].get("included"), None);
        assert_eq!(raw()["github/acme/api/42"]["touchedAt"], NOW + 1);

        fs::write(
            &path,
            json!({ "github/acme/api/42": {
                "lastMarkedHead": HEAD,
                "files": {},
                "touchedAt": NOW,
            } })
            .to_string(),
        )
        .unwrap();
        let older = store.entry(&acme).unwrap().unwrap();
        assert!(older.included.is_empty());
        assert_eq!(older.last_marked_head.as_deref(), Some(HEAD));
        assert_eq!(
            progress(
                &detail(HEAD, &[file(TEST)]),
                &[patched(b"t")],
                Some(&older),
                &skipping(&["**/tests/**"])
            )
            .files[0]
                .state,
            FileReviewState::Skipped
        );
    }

    /// `pull-request-viewer`: *Excluding creates nothing*, *Unmarking creates
    /// nothing*: neither an exclusion nor an unmark of a matching file
    /// creates an entry; and an inclusion into an existing entry leaves its
    /// `lastMarkedHead` and every key as they are, while touching it.
    #[test]
    fn an_exclusion_or_unmark_creates_nothing_and_an_inclusion_dates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let acme = github(42).key();
        let unmark = ReviewWrite::Marks {
            write: MarkWrite::UnmarkFile,
            include: true,
        };
        assert!(!store.write_marks(&acme, TEST, &unmark, HEAD, NOW).unwrap());
        assert!(!store
            .write_marks(&acme, TEST, &ReviewWrite::Include(false), HEAD, NOW)
            .unwrap());
        assert!(
            !dir.path().join("review-progress.json").exists(),
            "nothing stored"
        );

        store
            .mark(&acme, "src/lib.rs", patch_key("aa"), HEAD, NOW)
            .unwrap();
        assert!(store
            .write_marks(&acme, TEST, &ReviewWrite::Include(true), PUSHED, NOW + 1)
            .unwrap());
        let stored = store.entry(&acme).unwrap().unwrap();
        assert_eq!(stored.last_marked_head.as_deref(), Some(HEAD), "undated");
        assert_eq!(stored.files.keys().collect::<Vec<_>>(), ["src/lib.rs"]);
        assert!(stored.hunks.is_empty());
        assert_eq!(stored.included, BTreeSet::from([TEST.to_string()]));
        assert_eq!(stored.touched_at, NOW + 1);
    }

    /// `pull-request-viewer`: *Marking a matching file includes it*: viewed
    /// after the mark, unviewed rather than skipped after the unmark, and
    /// included after both; the mark alone dates the count. The same by a
    /// hunk.
    #[test]
    fn marking_then_unmarking_a_skipped_file_leaves_it_unviewed_and_included() {
        let rule = skipping(&["**/tests/**"]);
        let read = detail(HEAD, &[file(TEST)]);
        let files = [hunked(b"t", &[1, 2])];
        for by_hunk in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let store = store_in(&dir);
            let acme = github(42).key();
            let now = || {
                let stored = store.entry(&acme).unwrap();
                progress(&read, &files, stored.as_ref(), &rule).files[0].clone()
            };
            let write = |viewed: bool| {
                if by_hunk {
                    hunk_write(&read, &files, TEST, 0, viewed, HEAD, BASE, &rule)
                } else {
                    file_write(&read, &files, TEST, viewed, HEAD, BASE, &rule)
                }
                .unwrap()
            };
            assert_eq!(now().state, FileReviewState::Skipped);

            assert!(store
                .write_marks(&acme, TEST, &write(true), HEAD, NOW)
                .unwrap());
            let marked = now();
            let expected = if by_hunk {
                FileReviewState::PartlyViewed
            } else {
                FileReviewState::Viewed
            };
            assert_eq!(seen(&marked), (expected, Some("**/tests/**"), true));
            let entry = store.entry(&acme).unwrap().unwrap();
            assert_eq!(entry.last_marked_head.as_deref(), Some(HEAD));

            assert!(store
                .write_marks(&acme, TEST, &write(false), HEAD, NOW + 1)
                .unwrap());
            assert_eq!(
                seen(&now()),
                (FileReviewState::Unviewed, Some("**/tests/**"), true),
                "by hunk: {by_hunk}"
            );
        }
    }
}
