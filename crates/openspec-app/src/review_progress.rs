//! The pull-request viewer's review progress (`pull-request-viewer`: *Review
//! Progress*; design D9): which files of which pull request the reader marked
//! viewed, kept on this machine only and sent to neither host.
//!
//! Marks live in `review-progress.json` in the shared configuration
//! directory, owned by `AppService` as the activity log is and created by the
//! first mark. Entries are keyed by the canonical reference, each one
//! `{ lastMarkedHead, files: { path: key }, touchedAt }`. Every write reads
//! the file afresh and replaces it atomically, so a standalone
//! `specforge-serve` beside the desktop app — the documented second writer,
//! as for `activity.json` — loses a mark only when both write at once, and
//! neither sees the other's marks until it reads the file again.
//!
//! A file's key is computed here, from the cached detail, and never taken
//! from a caller ([`FileKey`]); its state is its stored key against its
//! current one. An entry untouched for [`PRUNE_AFTER_SECS`] whose provider is
//! enabled and no longer lists its pull request is pruned, but only while
//! that provider's list is complete, and once every enabled provider's list
//! has arrived in this run ([`Lists`]).

use std::collections::BTreeMap;
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

/// An entry untouched this long is pruned once its provider's list no longer
/// holds its pull request: 90 days.
pub const PRUNE_AFTER_SECS: u64 = 90 * 24 * 60 * 60;

// ---- the wire ----
//
// Camel case on the wire and hand-mirrored in `src/types.ts`;
// `tests/wire_shape.rs` pins the keys and each state's value.

/// A file's review state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileReviewState {
    /// Its stored key equals its current one.
    Viewed,
    /// Its stored key differs from its current one: a push or a retarget
    /// changed what the file shows since it was marked.
    ChangedSinceViewed,
    /// Nothing is stored for it.
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
    /// Files changed since viewed.
    pub changed_since_viewed: usize,
    /// Every file of the cached detail.
    pub total: usize,
    /// The head commit at the last mark, which only dates the changed count;
    /// `None` before any mark.
    pub last_marked_head: Option<String>,
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

/// The key `path` is marked or unmarked with, checked against the cached
/// `detail` and its `files` as `set_file_viewed` checks: refused when either
/// commit differs from the detail's, as a push or a retarget read since makes
/// them, and when no file of the detail has that path.
pub(crate) fn mark_key(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    path: &str,
    head: &str,
    base: &str,
) -> Result<FileKey, String> {
    if detail.head_commit != head || detail.base_commit != base {
        return Err("the pull request has changed since it was read".to_string());
    }
    detail
        .files
        .iter()
        .zip(files)
        .find(|(file, _)| file_path(file) == Some(path))
        .map(|(file, cached)| file_key(file, cached, detail))
        .ok_or_else(|| "not a file of this pull request".to_string())
}

/// The review progress of `detail`'s files, each by its current key against
/// the one `entry` stores for its path: viewed when they are equal, changed
/// since viewed when they differ, unviewed when none is stored. The counts
/// come from the keys alone.
pub(crate) fn progress(
    detail: &PullRequestDetail,
    files: &[CachedFile],
    entry: Option<&Entry>,
) -> ReviewProgress {
    let files: Vec<FileReviewProgress> = detail
        .files
        .iter()
        .zip(files)
        .filter_map(|(file, cached)| {
            let path = file_path(file)?;
            let key = file_key(file, cached, detail);
            let state = match entry.and_then(|entry| entry.files.get(path)) {
                None => FileReviewState::Unviewed,
                Some(stored) if *stored == key => FileReviewState::Viewed,
                Some(_) => FileReviewState::ChangedSinceViewed,
            };
            Some(FileReviewProgress {
                path: path.to_string(),
                state,
                keyed_by_head: matches!(key, FileKey::HeadCommit { .. }),
            })
        })
        .collect();
    let count = |state| files.iter().filter(|file| file.state == state).count();
    ReviewProgress {
        viewed: count(FileReviewState::Viewed),
        changed_since_viewed: count(FileReviewState::ChangedSinceViewed),
        total: files.len(),
        last_marked_head: entry.map(|entry| entry.last_marked_head.clone()),
        files,
    }
}

// ---- the store ----

/// One pull request's stored progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    /// The head commit at the last mark. An unmark leaves it.
    pub(crate) last_marked_head: String,
    /// Each marked file's key, by its path.
    pub(crate) files: BTreeMap<String, FileKey>,
    /// When a mark or an unmark last touched the entry, as Unix epoch
    /// seconds.
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

    /// Stores `file_key` as the mark of `path` in the entry of `key`,
    /// creating the entry, advancing its `lastMarkedHead` to `head` and
    /// touching it at `now`.
    pub(crate) fn mark(
        &self,
        key: &PullRequestKey,
        path: &str,
        file_key: FileKey,
        head: &str,
        now: u64,
    ) -> io::Result<()> {
        let _writing = self.writing.lock().unwrap();
        let mut entries = self.read_for_write()?;
        let name = entry_name(key);
        let mut files = entries
            .get(&name)
            .and_then(Entry::read)
            .map(|entry| entry.files)
            .unwrap_or_default();
        files.insert(path.to_string(), file_key);
        let entry = Entry {
            last_marked_head: head.to_string(),
            files,
            touched_at: now,
        };
        entries.insert(name, serde_json::to_value(entry)?);
        self.write(&entries)
    }

    /// Removes the mark of `path` from the entry of `key`, touching it at
    /// `now`. With no entry it creates none and stores nothing. Returns
    /// whether it stored the unmark.
    pub(crate) fn unmark(&self, key: &PullRequestKey, path: &str, now: u64) -> io::Result<bool> {
        let _writing = self.writing.lock().unwrap();
        let mut entries = self.read_for_write()?;
        let name = entry_name(key);
        let Some(mut entry) = entries.get(&name).and_then(Entry::read) else {
            return Ok(false);
        };
        entry.files.remove(path);
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

    /// An entry marked at `HEAD` with `files`.
    fn entry(files: impl IntoIterator<Item = (&'static str, FileKey)>) -> Entry {
        Entry {
            last_marked_head: HEAD.to_string(),
            files: files
                .into_iter()
                .map(|(path, key)| (path.to_string(), key))
                .collect(),
            touched_at: NOW,
        }
    }

    /// The state of the one file `path` in a detail read at `head` with
    /// `cached` beside it, against `stored`.
    fn state(head: &str, path: &str, cached: CachedFile, stored: &Entry) -> FileReviewState {
        let read = detail(head, &[file(path)]);
        progress(&read, &[cached], Some(stored)).files[0].state
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
        let read = progress(&pushed, &[patched(b"+two\n")], Some(&stored));
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
        );
        assert_eq!(pushed.files[0].state, FileReviewState::ChangedSinceViewed);
        assert!(pushed.files[0].keyed_by_head);

        let unmoved = progress(&detail(HEAD, &[file("logo.png")]), &[bare()], Some(&stored));
        assert_eq!(unmoved.files[0].state, FileReviewState::Viewed);
        let retargeted = PullRequestDetail {
            base_branch: "release".to_string(),
            ..detail(HEAD, &[file("logo.png")])
        };
        let read = progress(&retargeted, &[bare()], Some(&stored));
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
            progress(read, &[cached], Some(&stored)).files[0].clone()
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
    /// file goes by its old path.
    #[test]
    fn a_mark_is_checked_against_the_cached_commits_and_paths() {
        let mut gone = file("gone.rs");
        gone.new_path = None;
        let read = detail(HEAD, &[file("a.rs"), gone]);
        let files = [patched(b"a"), patched(b"gone")];
        assert_eq!(
            mark_key(&read, &files, "a.rs", HEAD, BASE),
            Ok(file_key(&read.files[0], &files[0], &read))
        );
        assert_eq!(
            mark_key(&read, &files, "gone.rs", HEAD, BASE),
            Ok(file_key(&read.files[1], &files[1], &read))
        );
        for (path, head, base) in [
            ("a.rs", PUSHED, BASE),
            ("a.rs", HEAD, PUSHED),
            ("b.rs", HEAD, BASE),
        ] {
            assert!(
                mark_key(&read, &files, path, head, base).is_err(),
                "{path} at {head}..{base}"
            );
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
        let mut stored = Entry {
            last_marked_head: HEAD.to_string(),
            files: BTreeMap::new(),
            touched_at: NOW,
        };
        for n in 0..4 {
            stored
                .files
                .insert(paths[n].clone(), file_key(&files[n], &cached[n], &read));
        }
        stored.files.insert(
            paths[4].clone(),
            file_key(&files[4], &patched(b"before the push"), &read),
        );
        let counted = progress(&read, &cached, Some(&stored));
        assert_eq!(
            (counted.viewed, counted.changed_since_viewed, counted.total),
            (4, 1, 10)
        );
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
        let counted = progress(&read, &[patched(b"a"), bare()], None);
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
        assert_eq!(stored.last_marked_head, PUSHED);
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
        assert_eq!(stored.last_marked_head, PUSHED);
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
}
