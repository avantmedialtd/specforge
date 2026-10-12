//! The pull-request viewer's detail cache (`pull-request-viewer`: *Detail
//! Reads Are Scoped to the Snapshot*; design D5).
//!
//! The last detail of each pull request is kept in memory only, keyed by its
//! canonical reference, holding at most [`CAPACITY`] and dropping the least
//! recently used. It serves four uses: a reopened view paints at once; a pull
//! request that leaves its list keeps its last detail, marked no longer
//! listed; a withheld file loads from it, with no request; and a review mark
//! is keyed against it.
//!
//! A read is answered from it, with no request, while its entry is under
//! [`FRESH_SECS`] old and its row unchanged, unless a manual refresh asks and
//! no manual refresh of that pull request sent a read in the last
//! [`MANUAL_REFRESH_SECS`]. At most one read of a pull request is in flight:
//! a caller asking for one whose read is in flight receives that read's
//! outcome rather than starting another.
//!
//! Each provider has a credential generation, which disabling it or saving
//! its credential advances, dropping its entries at once. A read is stored
//! only while the generation it started under is still current, checked
//! under the same lock that advances it, so a read in flight across a
//! credential change never lands in the cache: an entry only ever exists for
//! its provider's current generation. Every time is Unix epoch seconds,
//! passed in.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use openspec_core::diff::REQUESTED_FILE_BYTES_LIMIT;
use openspec_core::{DiffContent, DiffFile};
use tokio::sync::watch;

use crate::events::PullRequestProvider;
use crate::pull_request_detail::{
    file_path, CachedFile, FetchPaths, Fetched, PullRequestDetail, PullRequestDetailOutcome,
    PullRequestKey, ReadLinks, ReusedFiles,
};
use crate::pull_requests::{ChecksState, PullRequestSummary};

/// The most details the cache holds.
pub const CAPACITY: usize = 32;
/// A cached detail younger than this is answered with no request while its
/// row is unchanged.
pub const FRESH_SECS: u64 = 60;
/// A manual refresh reads past the freshness rule unless a manual refresh of
/// the same pull request sent a read this recently.
pub const MANUAL_REFRESH_SECS: u64 = 30;

/// What a row says that its cached detail goes stale with: its updated time
/// and, on GitHub, its checks and its unresolved conversations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowSignature {
    updated_at_unix: u64,
    checks: Option<ChecksState>,
    unresolved_threads: u32,
}

impl RowSignature {
    pub(crate) fn of(row: &PullRequestSummary) -> Self {
        Self {
            updated_at_unix: row.updated_at_unix,
            checks: row.checks,
            unresolved_threads: row.unresolved_threads,
        }
    }
}

/// The freshness rule, for a cached detail `age` seconds old whose row is
/// unchanged or not, when the last manual refresh of it that sent a read was
/// `since_manual_read` seconds ago, if ever:
///
/// fromCache ⇔ age < 60 ∧ row unchanged ∧ ¬(manual ∧ now − lastManualRead ≥ 30)
fn answer_from_cache(
    age: u64,
    row_unchanged: bool,
    manual: bool,
    since_manual_read: Option<u64>,
) -> bool {
    let fresh = age < FRESH_SECS && row_unchanged;
    let manual_reads = manual && since_manual_read.is_none_or(|since| since >= MANUAL_REFRESH_SECS);
    fresh && !manual_reads
}

/// One cached detail, and beside each of its files what the cache keeps of
/// it.
#[derive(Debug, Clone)]
pub(crate) struct CachedDetail {
    pub(crate) detail: PullRequestDetail,
    /// In the order of `detail.files`.
    pub(crate) files: Vec<CachedFile>,
    /// The row it was read through.
    signature: RowSignature,
    /// When a manual refresh of it last sent a read.
    last_manual_read: Option<u64>,
    /// The merge base a file read or an image read learned, kept so the next
    /// one of this detail sends no compare. A new read of the pull request
    /// keeps it while that read reports the same head and base commits, and
    /// drops it otherwise.
    merge_base: Option<String>,
    /// The BitBucket links it was read by; `None` on GitHub.
    links: Option<ReadLinks>,
}

/// A file of the cached detail, as `get_pull_request_file` asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileAnswer {
    /// The file, from the cache.
    Ready(DiffFile),
    /// A file sent without its patch and not read yet: a file read fetches it.
    Fetch(FileFetch),
}

/// What a file read needs, all from the cached detail: the file as the
/// detail carries it, the paths to fetch, the commits, and the merge base
/// when a file read of this detail has learned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileFetch {
    pub(crate) file: DiffFile,
    pub(crate) paths: FetchPaths,
    pub(crate) head: String,
    pub(crate) base: String,
    pub(crate) merge_base: Option<String>,
}

/// What an image read needs, all from the cached detail (`pull-request-viewer`:
/// *Pull-Request Image Reads*): the paths of the file's two versions, none on
/// a side it does not have, the commits, and the merge base when a file read
/// or an image read of this detail has learned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageFetch {
    pub(crate) paths: FetchPaths,
    pub(crate) head: String,
    pub(crate) base: String,
    pub(crate) merge_base: Option<String>,
}

/// Why the cache has no file to give: nothing cached, a commit that differs
/// from the cached detail's, or a path that is none of its files. The view
/// reads the pull request again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileChanged;

/// A read's result, as it asks to be stored.
pub(crate) struct ReadDetail {
    pub(crate) key: PullRequestKey,
    pub(crate) detail: PullRequestDetail,
    pub(crate) files: Vec<CachedFile>,
    pub(crate) signature: RowSignature,
    /// The credential generation it started under.
    pub(crate) generation: u64,
    /// When it was sent, if a manual refresh asked for it.
    pub(crate) manual_at: Option<u64>,
    /// Whether it reused the cached detail's files rather than list them.
    pub(crate) reused: bool,
    /// The merge base its reused files carried; `None` for a listed read.
    pub(crate) merge_base: Option<String>,
    /// The BitBucket links it was read by; `None` on GitHub.
    pub(crate) links: Option<ReadLinks>,
}

#[derive(Debug, Default)]
struct Cache {
    /// The least recently used first. A use moves an entry to the end, so
    /// the order is the order of use, however many uses one second holds.
    entries: Vec<(PullRequestKey, CachedDetail)>,
    github_generation: u64,
    bitbucket_generation: u64,
}

impl Cache {
    fn generation(&self, provider: PullRequestProvider) -> u64 {
        match provider {
            PullRequestProvider::Github => self.github_generation,
            PullRequestProvider::Bitbucket => self.bitbucket_generation,
        }
    }

    fn position(&self, key: &PullRequestKey) -> Option<usize> {
        self.entries.iter().position(|(cached, _)| cached == key)
    }

    /// The entry of `key`, without counting as a use of it.
    fn get(&self, key: &PullRequestKey) -> Option<&CachedDetail> {
        self.position(key).map(|at| &self.entries[at].1)
    }

    /// The entry of `key`, to change, without counting as a use of it.
    fn get_mut(&mut self, key: &PullRequestKey) -> Option<&mut CachedDetail> {
        let at = self.position(key)?;
        Some(&mut self.entries[at].1)
    }

    /// The entry of `key`, moved to the most recently used end.
    fn touch(&mut self, key: &PullRequestKey) -> Option<&mut CachedDetail> {
        let entry = self.entries.remove(self.position(key)?);
        self.entries.push(entry);
        self.entries.last_mut().map(|(_, entry)| entry)
    }

    /// Stores `entry` under `key` as the most recently used, replacing the
    /// one it had, or first dropping the least recently used entry when the
    /// cache is full and `key` is new to it.
    fn insert(&mut self, key: PullRequestKey, entry: CachedDetail) {
        match self.position(&key) {
            Some(at) => {
                self.entries.remove(at);
            }
            None if self.entries.len() >= CAPACITY => {
                self.entries.remove(0);
            }
            None => {}
        }
        self.entries.push((key, entry));
    }

    /// The entry of `key`, moved to the most recently used end, while its
    /// commits are still `head` and `base`.
    fn touch_at(
        &mut self,
        key: &PullRequestKey,
        head: &str,
        base: &str,
    ) -> Option<&mut CachedDetail> {
        self.touch(key)
            .filter(|entry| entry.detail.head_commit == head && entry.detail.base_commit == base)
    }

    /// The entry of `key` a read may keep what it found in: its provider
    /// still enabled, its credential generation still `generation`, and its
    /// commits still `head` and `base`.
    fn keepable(
        &mut self,
        key: &PullRequestKey,
        head: &str,
        base: &str,
        generation: u64,
        enabled: impl Fn() -> bool,
    ) -> Option<&mut CachedDetail> {
        if !enabled() || self.generation(key.provider) != generation {
            return None;
        }
        self.touch_at(key, head, base)
    }

    /// Advances `provider`'s credential generation and drops its entries.
    fn forget(&mut self, provider: PullRequestProvider) {
        match provider {
            PullRequestProvider::Github => self.github_generation += 1,
            PullRequestProvider::Bitbucket => self.bitbucket_generation += 1,
        }
        self.entries.retain(|(key, _)| key.provider != provider);
    }
}

/// The reads in flight, by pull request, each with the channel its outcome
/// is published on.
type InFlight =
    Arc<Mutex<HashMap<PullRequestKey, Arc<watch::Sender<Option<PullRequestDetailOutcome>>>>>>;

/// The detail cache and the reads in flight. Cheap to clone; clones share
/// one state, as the snapshot handles do.
#[derive(Debug, Clone, Default)]
pub(crate) struct PullRequestDetails {
    cache: Arc<Mutex<Cache>>,
    in_flight: InFlight,
}

impl PullRequestDetails {
    fn lock(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap()
    }

    /// `provider`'s credential generation, which a read records when it
    /// starts.
    pub(crate) fn generation(&self, provider: PullRequestProvider) -> u64 {
        self.lock().generation(provider)
    }

    /// Drops `provider`'s cached details at once and advances its credential
    /// generation, as disabling it or saving its credential must.
    pub(crate) fn forget(&self, provider: PullRequestProvider) {
        self.lock().forget(provider);
    }

    /// The cached detail of `key`, marked no longer listed as `not_listed`
    /// says, whatever its age; `None` when nothing is cached.
    pub(crate) fn cached(
        &self,
        key: &PullRequestKey,
        not_listed: bool,
    ) -> Option<PullRequestDetail> {
        let mut detail = self.lock().touch(key)?.detail.clone();
        detail.no_longer_listed = not_listed;
        Some(detail)
    }

    /// The cached detail of `key`, when the freshness rule answers a read of
    /// it from the cache at `now`, through `row` as its snapshot lists it
    /// now; `None` when the read goes to the provider.
    pub(crate) fn fresh(
        &self,
        key: &PullRequestKey,
        row: &PullRequestSummary,
        manual: bool,
        now: u64,
    ) -> Option<PullRequestDetail> {
        let mut cache = self.lock();
        let answered = {
            let entry = cache.get(key)?;
            answer_from_cache(
                now.saturating_sub(entry.detail.read_at_unix),
                entry.signature == RowSignature::of(row),
                manual,
                entry.last_manual_read.map(|at| now.saturating_sub(at)),
            )
        };
        if !answered {
            return None;
        }
        cache.touch(key).map(|entry| entry.detail.clone())
    }

    /// The cached detail's files, for a read of `key` that started under
    /// `generation` and whose first reply reports `head`, `base` and, on
    /// BitBucket, `links` (`pull-request-viewer`: *Detail Re-reads Reuse
    /// Unchanged Files*). `None` unless an entry exists under that
    /// generation with those commits and links, and `None` for a reply that
    /// named no head or no base. Probing is no use of the entry: the order
    /// of use is left as it was.
    pub(crate) fn reusable(
        &self,
        key: &PullRequestKey,
        generation: u64,
        head: &str,
        base: &str,
        links: Option<&ReadLinks>,
    ) -> Option<ReusedFiles> {
        if head.is_empty() || base.is_empty() {
            return None;
        }
        let cache = self.lock();
        if cache.generation(key.provider) != generation {
            return None;
        }
        let entry = cache.get(key)?;
        let same = entry.detail.head_commit == head
            && entry.detail.base_commit == base
            && entry.links.as_ref() == links;
        same.then(|| ReusedFiles {
            files: entry.detail.files.clone(),
            cached: entry.files.clone(),
            unlisted_files: entry.detail.unlisted_files,
            merge_base: entry.merge_base.clone(),
        })
    }

    /// Stores a read's detail, unless its provider is no longer enabled or
    /// its credential generation has moved on since the read started.
    /// Checked under the lock [`Self::forget`] takes, so a read in flight
    /// across a credential change is never stored. A manual refresh that sent
    /// it dates the manual bound; any other read keeps the bound it found.
    ///
    /// It merges against the entry it replaces, under the same lock
    /// (`pull-request-viewer`: *Detail Re-reads Reuse Unchanged Files*).
    /// While that entry has the read's head and base commits, its merge base
    /// is kept, and a read that reused the files takes that entry's files as
    /// they are now, so a file read or an image read kept since the reuse
    /// probe is not lost. Otherwise a reused read stores the files it probed,
    /// and a listed read stores its own with no merge base. Returns whether
    /// it was stored.
    pub(crate) fn store(&self, read: ReadDetail, enabled: impl Fn() -> bool) -> bool {
        let mut cache = self.lock();
        if !enabled() || cache.generation(read.key.provider) != read.generation {
            return false;
        }
        // The live entry is replaced below, so what is kept of it is moved
        // out rather than cloned.
        let live = cache.get_mut(&read.key);
        let last_manual_read = read
            .manual_at
            .or_else(|| live.as_ref().and_then(|entry| entry.last_manual_read));
        let mut detail = read.detail;
        let mut files = read.files;
        let mut merge_base = read.merge_base;
        let same_commits = live.filter(|entry| {
            entry.detail.head_commit == detail.head_commit
                && entry.detail.base_commit == detail.base_commit
        });
        if let Some(live) = same_commits {
            merge_base = live.merge_base.take();
            if read.reused {
                detail.files = std::mem::take(&mut live.detail.files);
                detail.unlisted_files = live.detail.unlisted_files;
                files = std::mem::take(&mut live.files);
            }
        }
        let entry = CachedDetail {
            detail,
            files,
            signature: read.signature,
            last_manual_read,
            merge_base,
            links: read.links,
        };
        cache.insert(read.key, entry);
        true
    }

    /// One file of the cached detail of `key`, as a withheld file's load asks
    /// for it (`pull-request-viewer`: *Detail Reads Are Scoped to the
    /// Snapshot*). `path` names it as the view keys it, and `head` and `base`
    /// are the commits the view rendered.
    ///
    /// [`FileChanged`] with nothing cached, when either commit differs from
    /// the cached detail's, as a push or a retarget read since makes them,
    /// and when no file has that path. A file the budgets withheld comes with
    /// its hunks, or too large past the per-file ceiling of `diff-view`'s
    /// *Line and Byte Budgets With On-Request Loading*, with no request. A
    /// file sent without its patch comes as a file read left it, or as what a
    /// file read needs while none has. Any other file comes as the detail
    /// carries it.
    pub(crate) fn file(
        &self,
        key: &PullRequestKey,
        path: &str,
        head: &str,
        base: &str,
    ) -> Result<FileAnswer, FileChanged> {
        let mut cache = self.lock();
        let entry = cache.touch_at(key, head, base).ok_or(FileChanged)?;
        let (file, cached) = entry
            .detail
            .files
            .iter()
            .zip(&entry.files)
            .find(|(file, _)| file_path(file) == Some(path))
            .ok_or(FileChanged)?;
        let patch_bytes = cached.patch.map_or(0, |patch| patch.len);
        let content = match (&cached.withheld, &cached.fetch, &cached.fetched) {
            (Some(_), _, _) if patch_bytes > REQUESTED_FILE_BYTES_LIMIT => DiffContent::TooLarge,
            (Some(hunks), _, _) => DiffContent::Hunks {
                hunks: hunks.clone(),
            },
            (None, Some(_), Some(fetched)) => fetched.clone(),
            (None, Some(paths), None) => {
                return Ok(FileAnswer::Fetch(FileFetch {
                    file: file.clone(),
                    paths: paths.clone(),
                    head: head.to_string(),
                    base: base.to_string(),
                    merge_base: entry.merge_base.clone(),
                }));
            }
            (None, None, _) => file.content.clone(),
        };
        Ok(FileAnswer::Ready(DiffFile {
            content,
            ..file.clone()
        }))
    }

    /// Keeps what a file read found for the file at `path`, its hunks' body
    /// digests among it, and the merge base it read by, unless the provider
    /// is no longer enabled, its
    /// credential generation has moved on since the file read started, or
    /// the cached detail is no longer the one the file read was asked of
    /// (its commits differ, or it is gone). Checked under the lock
    /// [`Self::forget`] takes, as [`Self::store`] checks a detail. Returns
    /// whether it was kept.
    pub(crate) fn keep_fetched(
        &self,
        key: &PullRequestKey,
        fetch: &FileFetch,
        merge_base: &str,
        fetched: Fetched,
        generation: u64,
        enabled: impl Fn() -> bool,
    ) -> bool {
        let mut cache = self.lock();
        let Some(entry) = cache.keepable(key, &fetch.head, &fetch.base, generation, enabled) else {
            return false;
        };
        let path = file_path(&fetch.file);
        let Some(cached) = entry
            .detail
            .files
            .iter()
            .zip(entry.files.iter_mut())
            .find(|(file, _)| file_path(file) == path)
            .map(|(_, cached)| cached)
        else {
            return false;
        };
        cached.fetched = Some(fetched.content);
        cached.hunks = fetched.hunks;
        entry.merge_base = Some(merge_base.to_string());
        true
    }

    /// What an image read of the file at `path` needs, from the cached detail
    /// of `key` (`pull-request-viewer`: *Pull-Request Image Reads*). `head`
    /// and `base` are the commits the view rendered. [`FileChanged`] exactly
    /// when [`Self::file`] answers it: nothing cached, either commit differs
    /// from the cached detail's, or no file has that path.
    pub(crate) fn image_fetch(
        &self,
        key: &PullRequestKey,
        path: &str,
        head: &str,
        base: &str,
    ) -> Result<ImageFetch, FileChanged> {
        let mut cache = self.lock();
        let entry = cache.touch_at(key, head, base).ok_or(FileChanged)?;
        let file = entry
            .detail
            .files
            .iter()
            .find(|file| file_path(file) == Some(path))
            .ok_or(FileChanged)?;
        Ok(ImageFetch {
            paths: FetchPaths {
                old: file.old_path.clone(),
                new: file.new_path.clone(),
            },
            head: head.to_string(),
            base: base.to_string(),
            merge_base: entry.merge_base.clone(),
        })
    }

    /// Keeps the merge base an image read read by, on the terms
    /// [`Self::keep_fetched`] keeps a file read's: unless the provider is no
    /// longer enabled, its credential generation has moved on since the read
    /// started, or the cached detail is no longer the one the read was asked
    /// of. Returns whether it was kept.
    pub(crate) fn keep_merge_base(
        &self,
        key: &PullRequestKey,
        fetch: &ImageFetch,
        merge_base: &str,
        generation: u64,
        enabled: impl Fn() -> bool,
    ) -> bool {
        let mut cache = self.lock();
        let Some(entry) = cache.keepable(key, &fetch.head, &fetch.base, generation, enabled) else {
            return false;
        };
        entry.merge_base = Some(merge_base.to_string());
        true
    }

    /// Runs `read` on the cached detail of `key` and its files' cached parts,
    /// in the detail's file order: the review keys are computed from these
    /// (`pull-request-viewer`: *Review Progress*). `None` when nothing is
    /// cached.
    pub(crate) fn with_entry<R>(
        &self,
        key: &PullRequestKey,
        read: impl FnOnce(&PullRequestDetail, &[CachedFile]) -> R,
    ) -> Option<R> {
        let mut cache = self.lock();
        let entry = cache.touch(key)?;
        Some(read(&entry.detail, &entry.files))
    }

    /// The outcome of the read of `key` already in flight, or of `read`,
    /// started now. `read` runs on the blocking pool, never on the calling
    /// thread, and runs to its end even when no caller is left to hear it,
    /// so it stores what it read. Every caller asking for `key` meanwhile
    /// receives the same outcome.
    pub(crate) async fn read_once(
        &self,
        key: PullRequestKey,
        read: impl FnOnce() -> PullRequestDetailOutcome + Send + 'static,
    ) -> PullRequestDetailOutcome {
        let mut outcome = {
            let mut in_flight = self.in_flight.lock().unwrap();
            match in_flight.get(&key) {
                Some(flight) => flight.subscribe(),
                None => {
                    let (sender, receiver) = watch::channel(None);
                    let sender = Arc::new(sender);
                    in_flight.insert(key.clone(), sender.clone());
                    let flight = Flight {
                        in_flight: self.in_flight.clone(),
                        key,
                        sender,
                    };
                    tokio::task::spawn_blocking(move || flight.land(read()));
                    receiver
                }
            }
        };
        outcome
            .wait_for(Option::is_some)
            .await
            .ok()
            .and_then(|answer| answer.as_ref().cloned())
            // The read ended without an outcome: it panicked.
            .unwrap_or(PullRequestDetailOutcome::Transient)
    }

    /// How many callers are waiting on the read of `key` in flight.
    #[cfg(test)]
    pub(crate) fn waiting(&self, key: &PullRequestKey) -> usize {
        self.in_flight
            .lock()
            .unwrap()
            .get(key)
            .map_or(0, |flight| flight.receiver_count())
    }
}

/// A read in flight. It publishes its outcome to every caller waiting on it,
/// and leaves the reads in flight however it ends, so the next ask starts a
/// read of its own.
struct Flight {
    in_flight: InFlight,
    key: PullRequestKey,
    sender: Arc<watch::Sender<Option<PullRequestDetailOutcome>>>,
}

impl Flight {
    /// Leaves the reads in flight, unless a later read of the same pull
    /// request has already taken its place there.
    fn leave(&self) {
        let mut in_flight = self.in_flight.lock().unwrap();
        if in_flight
            .get(&self.key)
            .is_some_and(|sender| Arc::ptr_eq(sender, &self.sender))
        {
            in_flight.remove(&self.key);
        }
    }

    /// Publishes the read's outcome. It leaves first: a caller asking from
    /// then on finds the cache this read has just filled, or starts a read of
    /// its own, and never joins one that has ended.
    fn land(self, outcome: PullRequestDetailOutcome) {
        self.leave();
        self.sender.send_replace(Some(outcome));
    }
}

impl Drop for Flight {
    /// A read that panicked never lands; it leaves all the same, and its
    /// waiters, their channel closed, answer transient.
    fn drop(&mut self) {
        self.leave();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pull_request_detail::{PatchDigest, PullRequestReference};
    use openspec_core::{FileStatus, Hunk, Line, LineKind};

    const NOW: u64 = 1_800_000_000;

    fn key(provider: PullRequestProvider, number: u64) -> PullRequestKey {
        PullRequestKey {
            provider,
            owner: "acme".to_string(),
            repo: "api".to_string(),
            number,
        }
    }

    fn github(number: u64) -> PullRequestKey {
        key(PullRequestProvider::Github, number)
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
            checks: Some(ChecksState::Pending),
            conflicting: false,
            unresolved_threads: 1,
            source_repo_full_name: "acme/api".to_string(),
        }
    }

    fn hunk(text: &str) -> Hunk {
        Hunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
            section: None,
            lines: vec![Line {
                kind: LineKind::Added,
                old_no: None,
                new_no: Some(1),
                text: text.to_string(),
                no_newline: false,
            }],
        }
    }

    fn file(path: &str, content: DiffContent) -> DiffFile {
        DiffFile {
            old_path: Some(path.to_string()),
            new_path: Some(path.to_string()),
            old_mode: None,
            new_mode: None,
            status: FileStatus::Modified,
            additions: Some(1),
            deletions: Some(0),
            content,
        }
    }

    fn detail(key: &PullRequestKey, read_at: u64) -> PullRequestDetail {
        PullRequestDetail {
            reference: PullRequestReference {
                provider: key.provider,
                owner: key.owner.clone(),
                repo: key.repo.clone(),
                number: key.number,
            },
            row: row(key.number),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            head_commit: "head".to_string(),
            base_commit: "base".to_string(),
            author: None,
            description: String::new(),
            conversation: Vec::new(),
            checks: Vec::new(),
            threads: Vec::new(),
            files: Vec::new(),
            unlisted_files: 0,
            read_at_unix: read_at,
            no_longer_listed: false,
        }
    }

    fn read(key: &PullRequestKey, read_at: u64) -> ReadDetail {
        ReadDetail {
            key: key.clone(),
            detail: detail(key, read_at),
            files: Vec::new(),
            signature: RowSignature::of(&row(key.number)),
            generation: 0,
            manual_at: None,
            reused: false,
            merge_base: None,
            links: None,
        }
    }

    /// A cache holding `read`, stored as a read with nothing in its way.
    fn holding(reads: impl IntoIterator<Item = ReadDetail>) -> PullRequestDetails {
        let details = PullRequestDetails::default();
        for read in reads {
            assert!(details.store(read, || true));
        }
        details
    }

    fn is_cached(details: &PullRequestDetails, key: &PullRequestKey) -> bool {
        details.cached(key, false).is_some()
    }

    // ------------------------------------------------------------ file reads

    /// A cache holding `acme/api#42`, whose `page.tsx` GitHub sent without its
    /// patch, and what a file read of it needs.
    fn with_a_patchless_page() -> (PullRequestDetails, PullRequestKey, FileFetch) {
        let pr = github(42);
        let details = holding([ReadDetail {
            detail: PullRequestDetail {
                files: vec![file("page.tsx", DiffContent::Withheld)],
                ..detail(&pr, NOW)
            },
            files: vec![CachedFile {
                fetch: Some(FetchPaths {
                    old: Some("page.tsx".to_string()),
                    new: Some("page.tsx".to_string()),
                }),
                ..Default::default()
            }],
            ..read(&pr, NOW)
        }]);
        let Ok(FileAnswer::Fetch(fetch)) = details.file(&pr, "page.tsx", "head", "base") else {
            panic!("a patchless file asks to be fetched");
        };
        assert_eq!(fetch.merge_base, None);
        (details, pr, fetch)
    }

    fn loaded() -> Fetched {
        Fetched {
            content: DiffContent::Hunks {
                hunks: vec![hunk("page")],
            },
            hunks: Some(vec![[9; 32]]),
        }
    }

    /// The hunk digests the cache keeps for the patchless page.
    fn page_hunks(details: &PullRequestDetails, pr: &PullRequestKey) -> Option<Vec<[u8; 32]>> {
        details
            .with_entry(pr, |_, files| files[0].hunks.clone())
            .expect("the detail is cached")
    }

    /// `pull-request-viewer`: *A file read is read once and kept*: what a file
    /// read found, its hunks' digests and its merge base are kept, and the
    /// next ask is served. The page's hunks are known only from then on
    /// (*A loaded file's hunks become known*).
    #[test]
    fn a_file_read_keeps_its_content_and_merge_base() {
        let (details, pr, fetch) = with_a_patchless_page();
        assert_eq!(page_hunks(&details, &pr), None);
        assert!(details.keep_fetched(&pr, &fetch, "merge", loaded(), 0, || true));
        let Ok(FileAnswer::Ready(served)) = details.file(&pr, "page.tsx", "head", "base") else {
            panic!("served from the cache");
        };
        assert_eq!(served.content, loaded().content);
        assert_eq!(page_hunks(&details, &pr), Some(vec![[9; 32]]));
        let entry = details.lock();
        assert_eq!(entry.get(&pr).unwrap().merge_base.as_deref(), Some("merge"));
    }

    /// *Credential changes*, for a file read: kept only while the provider is
    /// enabled under the generation the file read started with, each on its
    /// own, and only into the detail it was asked of, each commit on its own.
    #[test]
    fn a_file_read_is_kept_only_under_its_generation_and_commits() {
        let (details, pr, fetch) = with_a_patchless_page();
        assert!(!details.keep_fetched(&pr, &fetch, "merge", loaded(), 0, || false));
        assert!(!details.keep_fetched(&pr, &fetch, "merge", loaded(), 1, || true));
        let other_base = FileFetch {
            base: "rebased".to_string(),
            ..fetch.clone()
        };
        assert!(!details.keep_fetched(&pr, &other_base, "merge", loaded(), 0, || true));
        let other_head = FileFetch {
            head: "pushed".to_string(),
            ..fetch.clone()
        };
        assert!(!details.keep_fetched(&pr, &other_head, "merge", loaded(), 0, || true));
        assert!(
            matches!(
                details.file(&pr, "page.tsx", "head", "base"),
                Ok(FileAnswer::Fetch(_))
            ),
            "nothing was kept"
        );
        assert_eq!(page_hunks(&details, &pr), None, "no digests were kept");
        assert!(!details.keep_fetched(&github(7), &fetch, "merge", loaded(), 0, || true));
    }

    // ------------------------------------------------------------ re-reads

    fn links(diff: &str) -> ReadLinks {
        ReadLinks {
            diff: diff.to_string(),
            diffstat: "diffstat".to_string(),
        }
    }

    /// A cache holding the patchless page of `acme/api#42`, read by `links`,
    /// with a file read of it kept: its content, its hunk keys and the merge
    /// base.
    fn with_a_loaded_page(links: Option<ReadLinks>) -> (PullRequestDetails, PullRequestKey) {
        let (details, pr, fetch) = with_a_patchless_page();
        if links.is_some() {
            let stored = details.with_entry(&pr, |detail, files| ReadDetail {
                detail: detail.clone(),
                files: files.to_vec(),
                links: links.clone(),
                ..read(&pr, NOW)
            });
            assert!(details.store(stored.unwrap(), || true));
        }
        assert!(details.keep_fetched(&pr, &fetch, "merge", loaded(), 0, || true));
        (details, pr)
    }

    /// `pull-request-viewer`: *Detail Re-reads Reuse Unchanged Files*: an
    /// entry under the read's generation, with its head, its base and its
    /// links, gives its files as they are cached, a file read's result, its
    /// hunk keys and the merge base among them.
    #[test]
    fn a_matching_entry_is_reusable_with_everything_it_learned() {
        let (details, pr) = with_a_loaded_page(None);
        let reused = details
            .reusable(&pr, 0, "head", "base", None)
            .expect("reusable");
        assert_eq!(reused.files, vec![file("page.tsx", DiffContent::Withheld)]);
        assert_eq!(reused.cached[0].fetched, Some(loaded().content));
        assert_eq!(reused.cached[0].hunks, Some(vec![[9; 32]]));
        assert_eq!(reused.unlisted_files, 0);
        assert_eq!(reused.merge_base.as_deref(), Some("merge"));

        let by_links = Some(links("diff"));
        let (details, pr) = with_a_loaded_page(by_links.clone());
        assert!(details
            .reusable(&pr, 0, "head", "base", by_links.as_ref())
            .is_some());
    }

    /// One condition at a time: another head, another base, other links (or
    /// none where some were kept, or some where none were), and no entry.
    #[test]
    fn each_condition_alone_refuses_reuse() {
        let kept = Some(links("diff"));
        let (details, pr) = with_a_loaded_page(kept.clone());
        let other = links("other");
        let diffstat = ReadLinks {
            diffstat: "other".to_string(),
            ..links("diff")
        };
        for (head, base, links) in [
            ("pushed", "base", kept.as_ref()),
            ("head", "rebased", kept.as_ref()),
            ("head", "base", Some(&other)),
            ("head", "base", Some(&diffstat)),
            ("head", "base", None),
        ] {
            assert_eq!(
                details.reusable(&pr, 0, head, base, links),
                None,
                "{head}/{base} {links:?}"
            );
        }
        let (details, pr) = with_a_loaded_page(None);
        assert_eq!(
            details.reusable(&pr, 0, "head", "base", Some(&other)),
            None,
            "links where none were kept"
        );
        // A reply that named no head, or no base, against an entry cached
        // with none.
        for (head, base) in [("", "base"), ("head", "")] {
            let pr = github(9);
            let details = holding([ReadDetail {
                detail: PullRequestDetail {
                    head_commit: head.to_string(),
                    base_commit: base.to_string(),
                    ..detail(&pr, NOW)
                },
                ..read(&pr, NOW)
            }]);
            assert_eq!(
                details.reusable(&pr, 0, head, base, None),
                None,
                "{head:?}/{base:?}"
            );
        }
        assert_eq!(details.reusable(&github(7), 0, "head", "base", None), None);
    }

    /// A read that started before a credential change is refused even while
    /// an entry of the same commits exists, stored since under the new
    /// generation.
    #[test]
    fn a_read_begun_under_a_stale_generation_reuses_nothing() {
        let pr = github(42);
        let details = holding([read(&pr, NOW)]);
        details.forget(PullRequestProvider::Github);
        assert!(details.store(
            ReadDetail {
                generation: 1,
                ..read(&pr, NOW)
            },
            || true
        ));
        assert!(details.reusable(&pr, 1, "head", "base", None).is_some());
        assert_eq!(details.reusable(&pr, 0, "head", "base", None), None);
    }

    /// Probing for reuse is no use of the entry: a 33rd read still drops the
    /// entry that was least recently used before the probe.
    #[test]
    fn probing_for_reuse_leaves_the_order_of_use() {
        let keys: Vec<PullRequestKey> = (1..=33).map(github).collect();
        let details = holding(keys[..32].iter().map(|key| read(key, NOW)));
        assert!(details
            .reusable(&keys[0], 0, "head", "base", None)
            .is_some());
        assert!(details.store(read(&keys[32], NOW), || true));
        assert!(
            !is_cached(&details, &keys[0]),
            "still the least recently used"
        );
        assert!(is_cached(&details, &keys[1]));
    }

    fn merge_base_of(details: &PullRequestDetails, pr: &PullRequestKey) -> Option<String> {
        details.lock().get(pr).unwrap().merge_base.clone()
    }

    /// A listed read of `pr` at `head` and `base`, of one plain file.
    fn listed_at(pr: &PullRequestKey, head: &str, base: &str) -> ReadDetail {
        let plain = file(
            "plain.rs",
            DiffContent::Hunks {
                hunks: vec![hunk("p")],
            },
        );
        ReadDetail {
            detail: PullRequestDetail {
                head_commit: head.to_string(),
                base_commit: base.to_string(),
                files: vec![plain],
                ..detail(pr, NOW + 70)
            },
            files: vec![CachedFile::default()],
            ..read(pr, NOW + 70)
        }
    }

    /// A read that reused `reused`, at the cached commits.
    fn reusing(pr: &PullRequestKey, reused: ReusedFiles) -> ReadDetail {
        ReadDetail {
            detail: PullRequestDetail {
                files: reused.files,
                unlisted_files: reused.unlisted_files,
                description: "edited".to_string(),
                ..detail(pr, NOW + 70)
            },
            files: reused.cached,
            reused: true,
            merge_base: reused.merge_base,
            ..read(pr, NOW + 70)
        }
    }

    /// A listed read of the same commits — a manual refresh — replaces the
    /// files but keeps the merge base; one of another head, or of another
    /// base, drops it.
    #[test]
    fn a_listed_read_keeps_the_merge_base_only_for_the_same_commits() {
        let (details, pr) = with_a_loaded_page(None);
        assert!(details.store(listed_at(&pr, "head", "base"), || true));
        assert_eq!(merge_base_of(&details, &pr).as_deref(), Some("merge"));
        let cached = details.cached(&pr, false).unwrap();
        assert_eq!(cached.files[0].new_path.as_deref(), Some("plain.rs"));
        assert_eq!(cached.read_at_unix, NOW + 70);
        assert_eq!(page_hunks(&details, &pr), None, "the fresh files' keys");

        for (head, base) in [("pushed", "base"), ("head", "rebased")] {
            let (details, pr) = with_a_loaded_page(None);
            assert!(details.store(listed_at(&pr, head, base), || true));
            assert_eq!(merge_base_of(&details, &pr), None, "{head}/{base}");
        }
    }

    /// A file read kept between a reusing read's probe and its store is
    /// still there once it is stored, beside what the read itself brought.
    #[test]
    fn a_file_read_kept_during_a_reusing_read_survives_its_store() {
        let (details, pr, fetch) = with_a_patchless_page();
        let reused = details
            .reusable(&pr, 0, "head", "base", None)
            .expect("reusable");
        assert_eq!(reused.cached[0].fetched, None);
        assert!(details.keep_fetched(&pr, &fetch, "merge", loaded(), 0, || true));
        assert!(details.store(reusing(&pr, reused), || true));
        assert_eq!(page_hunks(&details, &pr), Some(vec![[9; 32]]));
        assert_eq!(merge_base_of(&details, &pr).as_deref(), Some("merge"));
        let Ok(FileAnswer::Ready(served)) = details.file(&pr, "page.tsx", "head", "base") else {
            panic!("served from the cache");
        };
        assert_eq!(served.content, loaded().content);
        assert_eq!(details.cached(&pr, false).unwrap().description, "edited");
    }

    /// A reusing read whose entry was dropped meanwhile stores the files it
    /// probed, merge base and all.
    #[test]
    fn a_reusing_read_whose_entry_was_dropped_stores_its_snapshot() {
        let (details, pr) = with_a_loaded_page(None);
        let reused = details.reusable(&pr, 0, "head", "base", None).unwrap();
        for evicting in (100..100 + CAPACITY as u64).map(github) {
            assert!(details.store(read(&evicting, NOW), || true));
        }
        assert!(!is_cached(&details, &pr));
        assert!(details.store(reusing(&pr, reused), || true));
        assert_eq!(page_hunks(&details, &pr), Some(vec![[9; 32]]));
        assert_eq!(merge_base_of(&details, &pr).as_deref(), Some("merge"));
    }

    /// A reusing read whose entry was replaced meanwhile by a read of other
    /// commits stores the files it probed, not that read's. Single flight
    /// (one detail read of a pull request at a time) makes this state
    /// unreachable in production: the test pins the defensive behaviour, not
    /// an intended rule.
    #[test]
    fn a_reusing_read_stores_its_snapshot_over_an_entry_of_other_commits() {
        let (details, pr) = with_a_loaded_page(None);
        let reused = details.reusable(&pr, 0, "head", "base", None).unwrap();
        assert!(details.store(listed_at(&pr, "pushed", "base"), || true));
        assert!(details.store(reusing(&pr, reused), || true));
        assert_eq!(page_hunks(&details, &pr), Some(vec![[9; 32]]));
        assert_eq!(merge_base_of(&details, &pr).as_deref(), Some("merge"));
        let cached = details.cached(&pr, false).unwrap();
        assert_eq!(cached.head_commit, "head");
        assert_eq!(cached.files[0].new_path.as_deref(), Some("page.tsx"));
    }

    /// A reusing read that a credential change overtook is not stored.
    #[test]
    fn a_reusing_read_under_an_advanced_generation_is_not_stored() {
        let (details, pr) = with_a_loaded_page(None);
        let reused = details.reusable(&pr, 0, "head", "base", None).unwrap();
        details.forget(PullRequestProvider::Github);
        assert!(!details.store(reusing(&pr, reused), || true));
        assert!(!is_cached(&details, &pr));
    }

    /// A stored read keeps the links it was read by.
    #[test]
    fn a_stored_read_keeps_its_links() {
        let pr = key(PullRequestProvider::Bitbucket, 42);
        let details = holding([ReadDetail {
            links: Some(links("diff")),
            ..read(&pr, NOW)
        }]);
        assert_eq!(details.lock().get(&pr).unwrap().links, Some(links("diff")));
    }

    // ----------------------------------------------------------- image reads

    /// A cache holding `acme/api#42` with a modified, an added, a deleted and
    /// a renamed image.
    fn with_images() -> (PullRequestDetails, PullRequestKey) {
        let pr = github(42);
        let image = |old: Option<&str>, new: Option<&str>, status| DiffFile {
            old_path: old.map(str::to_string),
            new_path: new.map(str::to_string),
            status,
            content: DiffContent::Binary,
            ..file("", DiffContent::Binary)
        };
        let files = vec![
            image(Some("app.png"), Some("app.png"), FileStatus::Modified),
            image(None, Some("new.png"), FileStatus::Added),
            image(Some("gone.png"), None, FileStatus::Deleted),
            image(
                Some("old.png"),
                Some("moved.png"),
                FileStatus::Renamed { similarity: None },
            ),
        ];
        let details = holding([ReadDetail {
            files: vec![CachedFile::default(); files.len()],
            detail: PullRequestDetail {
                files,
                ..detail(&pr, NOW)
            },
            ..read(&pr, NOW)
        }]);
        (details, pr)
    }

    fn paths(old: Option<&str>, new: Option<&str>) -> FetchPaths {
        FetchPaths {
            old: old.map(str::to_string),
            new: new.map(str::to_string),
        }
    }

    /// `pull-request-viewer`: *Pull-Request Image Reads*: each version's path
    /// from the cached detail, none on a side the file does not have, and the
    /// merge base once a file read or an image read has learned it.
    #[test]
    fn an_image_read_takes_its_paths_and_merge_base_from_the_cached_detail() {
        let (details, pr) = with_images();
        let fetch = |path| {
            details
                .image_fetch(&pr, path, "head", "base")
                .map(|f| f.paths)
        };
        assert_eq!(
            fetch("app.png"),
            Ok(paths(Some("app.png"), Some("app.png")))
        );
        assert_eq!(fetch("new.png"), Ok(paths(None, Some("new.png"))));
        assert_eq!(fetch("gone.png"), Ok(paths(Some("gone.png"), None)));
        assert_eq!(
            fetch("moved.png"),
            Ok(paths(Some("old.png"), Some("moved.png")))
        );

        let first = details.image_fetch(&pr, "app.png", "head", "base").unwrap();
        assert_eq!(
            (
                first.head.as_str(),
                first.base.as_str(),
                first.merge_base.as_deref()
            ),
            ("head", "base", None)
        );
        assert!(details.keep_merge_base(&pr, &first, "merge", 0, || true));
        let next = details.image_fetch(&pr, "new.png", "head", "base").unwrap();
        assert_eq!(next.merge_base.as_deref(), Some("merge"));
    }

    /// `pull-request-viewer`: *An image read after a push answers changed*:
    /// exactly when a file's load would.
    #[test]
    fn an_image_read_answers_changed_when_a_file_load_would() {
        let (details, pr) = with_images();
        for (pr, path, head, base) in [
            (&github(7), "app.png", "head", "base"),
            (&pr, "app.png", "pushed", "base"),
            (&pr, "app.png", "head", "rebased"),
            (&pr, "other.png", "head", "base"),
            (&pr, "old.png", "head", "base"),
        ] {
            assert_eq!(
                details.image_fetch(pr, path, head, base),
                Err(FileChanged),
                "{path} at {head}/{base}"
            );
            assert_eq!(
                details.file(pr, path, head, base).err(),
                Some(FileChanged),
                "{path} at {head}/{base}"
            );
        }
    }

    /// *Credential changes*, for an image read: its merge base is kept only
    /// while the provider is enabled under the generation it started with,
    /// and only into the detail it was asked of, each commit on its own.
    #[test]
    fn an_image_reads_merge_base_is_kept_only_under_its_generation_and_commits() {
        let (details, pr) = with_images();
        let fetch = details.image_fetch(&pr, "app.png", "head", "base").unwrap();
        assert!(!details.keep_merge_base(&pr, &fetch, "merge", 0, || false));
        assert!(!details.keep_merge_base(&pr, &fetch, "merge", 1, || true));
        let other_base = ImageFetch {
            base: "rebased".to_string(),
            ..fetch.clone()
        };
        assert!(!details.keep_merge_base(&pr, &other_base, "merge", 0, || true));
        let other_head = ImageFetch {
            head: "pushed".to_string(),
            ..fetch.clone()
        };
        assert!(!details.keep_merge_base(&pr, &other_head, "merge", 0, || true));
        assert!(!details.keep_merge_base(&github(7), &fetch, "merge", 0, || true));
        let again = details.image_fetch(&pr, "app.png", "head", "base").unwrap();
        assert_eq!(again.merge_base, None, "nothing was kept");
    }

    // ------------------------------------------------------------ freshness

    #[test]
    fn an_entry_exactly_sixty_seconds_old_reads_again_and_one_fifty_nine_old_does_not() {
        assert!(answer_from_cache(59, true, false, None));
        assert!(!answer_from_cache(60, true, false, None));
        assert_eq!(FRESH_SECS, 60);
    }

    #[test]
    fn a_changed_row_reads_again_however_young_the_entry() {
        assert!(!answer_from_cache(0, false, false, None));
    }

    /// A manual refresh exactly 30 s after the last manual read reads, and
    /// one 29 s after does not; with no manual read before it, it reads.
    #[test]
    fn the_manual_bound_is_thirty_seconds_to_the_second() {
        assert!(!answer_from_cache(10, true, true, Some(30)));
        assert!(answer_from_cache(10, true, true, Some(29)));
        assert!(!answer_from_cache(10, true, true, None));
        assert_eq!(MANUAL_REFRESH_SECS, 30);
    }

    /// The manual bound only ever adds reads: a stale entry or a changed row
    /// reads whether or not a manual refresh asks.
    #[test]
    fn a_manual_refresh_never_answers_a_stale_entry_from_the_cache() {
        assert!(!answer_from_cache(70, true, true, Some(10)));
        assert!(!answer_from_cache(10, false, true, Some(10)));
    }

    #[test]
    fn a_row_signature_is_its_updated_time_checks_and_unresolved_threads() {
        let base = row(42);
        let signature = RowSignature::of(&base);
        assert_eq!(signature, RowSignature::of(&base.clone()));
        let retitled = PullRequestSummary {
            title: "Renamed".to_string(),
            ..base.clone()
        };
        assert_eq!(
            RowSignature::of(&retitled),
            signature,
            "a title is no signal"
        );
        for changed in [
            PullRequestSummary {
                updated_at_unix: base.updated_at_unix + 1,
                ..base.clone()
            },
            PullRequestSummary {
                checks: Some(ChecksState::Failing),
                ..base.clone()
            },
            PullRequestSummary {
                unresolved_threads: 2,
                ..base.clone()
            },
        ] {
            assert_ne!(RowSignature::of(&changed), signature, "{changed:?}");
        }
    }

    #[test]
    fn fresh_answers_by_the_rule_through_the_row_as_listed_now() {
        let pr = github(42);
        let details = holding([read(&pr, NOW)]);
        assert!(details.fresh(&pr, &row(42), false, NOW + 59).is_some());
        assert!(details.fresh(&pr, &row(42), false, NOW + 60).is_none());
        let failing = PullRequestSummary {
            checks: Some(ChecksState::Failing),
            ..row(42)
        };
        assert!(details.fresh(&pr, &failing, false, NOW + 1).is_none());
        assert!(details.fresh(&github(7), &row(7), false, NOW).is_none());
    }

    /// The manual bound dates from the manual refresh that sent a read, and
    /// a later read that was not manual keeps it.
    #[test]
    fn a_manual_read_dates_the_bound_and_a_later_read_keeps_it() {
        let pr = github(42);
        let details = holding([ReadDetail {
            manual_at: Some(NOW),
            ..read(&pr, NOW)
        }]);
        assert!(details.fresh(&pr, &row(42), true, NOW + 29).is_some());
        assert!(details.store(read(&pr, NOW + 20), || true));
        assert!(
            details.fresh(&pr, &row(42), true, NOW + 29).is_some(),
            "still bound by the manual read at NOW"
        );
        assert!(details.fresh(&pr, &row(42), true, NOW + 30).is_none());
    }

    // ------------------------------------------------------------ the cache

    /// With 32 cached, a 33rd drops the least recently used and keeps the
    /// other 31; using the oldest first moves the drop to the next oldest.
    #[test]
    fn a_thirty_third_detail_drops_the_least_recently_used() {
        let keys: Vec<PullRequestKey> = (1..=33).map(github).collect();
        let details = holding(keys[..32].iter().map(|key| read(key, NOW)));
        assert!(keys[..32].iter().all(|key| is_cached(&details, key)));
        assert_eq!(CAPACITY, 32);

        // Using #1 again makes #2 the least recently used.
        assert!(details.fresh(&keys[0], &row(1), false, NOW).is_some());
        assert!(details.store(read(&keys[32], NOW), || true));
        assert!(!is_cached(&details, &keys[1]));
        let kept = keys.iter().filter(|key| is_cached(&details, key)).count();
        assert_eq!(kept, 32, "the 33rd and the other 31");
    }

    /// The cache only drops for a pull request new to it: a re-read of one
    /// already cached, with the cache full, drops nothing.
    #[test]
    fn re_reading_a_cached_detail_with_the_cache_full_drops_nothing() {
        let keys: Vec<PullRequestKey> = (1..=32).map(github).collect();
        let details = holding(keys.iter().map(|key| read(key, NOW)));
        assert!(details.store(read(&keys[5], NOW + 70), || true));
        assert!(keys.iter().all(|key| is_cached(&details, key)));
        assert_eq!(
            details.cached(&keys[5], false).unwrap().read_at_unix,
            NOW + 70,
            "replaced in place"
        );
    }

    #[test]
    fn a_cached_detail_is_marked_as_listed_or_not_whatever_its_age() {
        let pr = github(42);
        let details = holding([read(&pr, NOW)]);
        assert!(details.cached(&pr, true).unwrap().no_longer_listed);
        assert!(!details.cached(&pr, false).unwrap().no_longer_listed);
        assert_eq!(details.cached(&github(7), true), None);
    }

    /// Forgetting a provider drops its details, and only its, and advances
    /// its generation, so a read that started before is not stored.
    #[test]
    fn forgetting_a_provider_drops_its_details_and_refuses_reads_begun_before() {
        let (on_github, on_bitbucket) = (github(42), key(PullRequestProvider::Bitbucket, 42));
        let details = holding([read(&on_github, NOW), read(&on_bitbucket, NOW)]);
        assert_eq!(details.generation(PullRequestProvider::Github), 0);

        details.forget(PullRequestProvider::Github);
        assert!(!is_cached(&details, &on_github));
        assert!(is_cached(&details, &on_bitbucket));
        assert_eq!(details.generation(PullRequestProvider::Github), 1);
        assert_eq!(details.generation(PullRequestProvider::Bitbucket), 0);

        assert!(
            !details.store(read(&on_github, NOW), || true),
            "generation 0"
        );
        assert!(details.store(
            ReadDetail {
                generation: 1,
                ..read(&on_github, NOW)
            },
            || true
        ));
        details.forget(PullRequestProvider::Bitbucket);
        assert!(!is_cached(&details, &on_bitbucket));
        assert!(is_cached(&details, &on_github));
        assert_eq!(details.generation(PullRequestProvider::Bitbucket), 1);
    }

    #[test]
    fn a_read_ending_after_its_provider_was_disabled_is_not_stored() {
        let pr = github(42);
        let details = PullRequestDetails::default();
        assert!(!details.store(read(&pr, NOW), || false));
        assert!(!is_cached(&details, &pr));
    }

    // ------------------------------------------------------------ withheld files

    /// A detail of three files: `a.rs` eager, `b.rs` withheld with its hunks
    /// cached, `gone.rs` deleted and eager.
    fn with_files(patch_bytes: usize) -> (PullRequestDetails, PullRequestKey) {
        let pr = github(42);
        let mut deleted = file(
            "gone.rs",
            DiffContent::Hunks {
                hunks: vec![hunk("x")],
            },
        );
        deleted.new_path = None;
        deleted.status = FileStatus::Deleted;
        let files = vec![
            file(
                "a.rs",
                DiffContent::Hunks {
                    hunks: vec![hunk("a")],
                },
            ),
            file("b.rs", DiffContent::Withheld),
            deleted,
        ];
        let cached = |withheld: Option<Vec<Hunk>>, bytes: usize| CachedFile {
            withheld,
            patch: Some(PatchDigest {
                len: bytes,
                sha256: [0; 32],
            }),
            blob_sha: None,
            ..Default::default()
        };
        let details = holding([ReadDetail {
            detail: PullRequestDetail {
                files,
                ..detail(&pr, NOW)
            },
            files: vec![
                cached(None, 10),
                cached(Some(vec![hunk("b")]), patch_bytes),
                cached(None, 10),
            ],
            ..read(&pr, NOW)
        }]);
        (details, pr)
    }

    /// The file a load gets from the cache, which must have it ready.
    fn ready(answer: Result<FileAnswer, FileChanged>) -> DiffFile {
        match answer {
            Ok(FileAnswer::Ready(file)) => file,
            other => panic!("expected a ready file, found {other:?}"),
        }
    }

    #[test]
    fn a_withheld_file_is_served_with_its_hunks_from_the_cache() {
        let (details, pr) = with_files(100);
        let served = ready(details.file(&pr, "b.rs", "head", "base"));
        assert_eq!(
            served.content,
            DiffContent::Hunks {
                hunks: vec![hunk("b")]
            }
        );
        assert_eq!(served.new_path.as_deref(), Some("b.rs"));
        // Any other file comes as the detail carries it, a deleted one by its
        // old path.
        let eager = ready(details.file(&pr, "a.rs", "head", "base"));
        assert_eq!(
            eager.content,
            DiffContent::Hunks {
                hunks: vec![hunk("a")]
            }
        );
        let gone = ready(details.file(&pr, "gone.rs", "head", "base"));
        assert_eq!(gone.status, FileStatus::Deleted);
    }

    /// The per-file ceiling of a load on request: exactly 8 MiB of patch text
    /// is served, a byte more is too large to preview.
    #[test]
    fn a_withheld_file_past_eight_mebibytes_is_too_large() {
        let (details, pr) = with_files(REQUESTED_FILE_BYTES_LIMIT);
        assert!(matches!(
            ready(details.file(&pr, "b.rs", "head", "base")).content,
            DiffContent::Hunks { .. }
        ));
        let (details, pr) = with_files(REQUESTED_FILE_BYTES_LIMIT + 1);
        assert_eq!(
            ready(details.file(&pr, "b.rs", "head", "base")).content,
            DiffContent::TooLarge
        );
        assert_eq!(REQUESTED_FILE_BYTES_LIMIT, 8 * 1024 * 1024);
    }

    /// A head commit a push changed, or a base a retarget changed, is
    /// refused; so is a path not among the files, and a pull request with
    /// nothing cached.
    #[test]
    fn a_withheld_file_load_is_refused_against_another_commit_or_path() {
        let (details, pr) = with_files(100);
        for (path, head, base) in [
            ("b.rs", "pushed", "base"),
            ("b.rs", "head", "retargeted"),
            ("c.rs", "head", "base"),
        ] {
            assert!(
                details.file(&pr, path, head, base).is_err(),
                "{path} at {head}..{base}"
            );
        }
        assert!(details.file(&github(7), "b.rs", "head", "base").is_err());
    }

    #[test]
    fn with_entry_reads_the_cached_detail_and_its_files_in_order() {
        let (details, pr) = with_files(100);
        let read = details.with_entry(&pr, |detail, files| {
            (
                detail.head_commit.clone(),
                files
                    .iter()
                    .map(|file| file.withheld.is_some())
                    .collect::<Vec<_>>(),
            )
        });
        assert_eq!(read, Some(("head".to_string(), vec![false, true, false])));
        assert_eq!(details.with_entry(&github(7), |_, _| ()), None);
    }

    // ------------------------------------------------------------ one read in flight

    /// Awaits `answer`, failing the test rather than hanging it should it
    /// never come: a hung test is a mutation timeout, not a catch.
    async fn within_bound<T>(answer: impl std::future::Future<Output = T>) -> T {
        tokio::time::timeout(std::time::Duration::from_secs(10), answer)
            .await
            .expect("answered within the bound")
    }

    /// Two asks for one pull request while its read is in flight run one
    /// read, and both receive its outcome.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_asks_for_one_pull_request_share_one_read() {
        let details = PullRequestDetails::default();
        let pr = github(42);
        let (entered, entered_rx) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let first = {
            let (details, pr, reads) = (details.clone(), pr.clone(), reads.clone());
            tokio::spawn(async move {
                details
                    .read_once(pr, move || {
                        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        entered.send(()).unwrap();
                        released.recv().unwrap();
                        PullRequestDetailOutcome::Unavailable
                    })
                    .await
            })
        };
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the first read starts");
        let second = {
            let (details, pr) = (details.clone(), pr.clone());
            tokio::spawn(async move {
                details
                    .read_once(pr, || panic!("a second read is never started"))
                    .await
            })
        };
        within_bound(async {
            while details.waiting(&pr) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        release.send(()).unwrap();

        assert_eq!(
            within_bound(first).await.unwrap(),
            PullRequestDetailOutcome::Unavailable
        );
        assert_eq!(
            within_bound(second).await.unwrap(),
            PullRequestDetailOutcome::Unavailable
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(details.waiting(&pr), 0, "the read has left the line");
    }

    /// A read that ended leaves the reads in flight, so the next ask reads
    /// again; one that panicked answers transient, and leaves too.
    #[tokio::test]
    async fn an_ended_read_leaves_and_a_panicked_one_answers_transient() {
        let details = PullRequestDetails::default();
        let pr = github(42);
        let ask = |read: fn() -> PullRequestDetailOutcome| {
            within_bound(details.read_once(pr.clone(), read))
        };
        assert_eq!(
            ask(|| PullRequestDetailOutcome::Unavailable).await,
            PullRequestDetailOutcome::Unavailable
        );
        assert_eq!(
            ask(|| PullRequestDetailOutcome::NotListed).await,
            PullRequestDetailOutcome::NotListed,
            "a read of its own"
        );
        assert_eq!(
            ask(|| panic!("the read fails")).await,
            PullRequestDetailOutcome::Transient
        );
        assert_eq!(details.waiting(&pr), 0);
        assert_eq!(
            ask(|| PullRequestDetailOutcome::Refused).await,
            PullRequestDetailOutcome::Refused
        );
    }
}
