//! The separate, credentialless reader for published PR Markdown images.
//! Provider-authenticated file/version readers never share this transport.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::{self, Read};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use openspec_core::diff::REQUESTED_FILE_BYTES_LIMIT;
use openspec_core::pr_content_image::{
    canonical_source, public_resolution, redirect_target, ImageSource,
};
use openspec_core::ImageRefusal;
use serde::Serialize;
use sha2::{Digest, Sha256};
use ureq::unversioned::resolver::{ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{
    ConnectionDetails, Connector, NextTimeout, RustlsConnector, TcpConnector,
};
use ureq::{Agent, Error};

use crate::pull_request_detail::{ImageSide, PullRequestDetail, PullRequestKey};

/// Stable failures, never a raw transport error that could echo a signed URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentImageFailure {
    Refused,
    UnsafeSource,
    UnsafeAddress,
    Unavailable,
    Redirected,
    Transient,
    Deferred,
}

/// The image result crossing both rich-client transports. Only inspected raster
/// bytes may populate `Image`. Debug never dumps those bytes.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PullRequestContentImageOutcome {
    Image {
        mime: String,
        width: u32,
        height: u32,
        data: String,
        final_host: String,
    },
    Refused {
        reason: ImageRefusal,
    },
    Stale,
    Failed {
        reason: ContentImageFailure,
        until_unix: Option<u64>,
    },
}

impl fmt::Debug for PullRequestContentImageOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image { width, height, .. } => f
                .debug_struct("Image")
                .field("width", width)
                .field("height", height)
                .finish_non_exhaustive(),
            Self::Refused { reason } => f.debug_struct("Refused").field("reason", reason).finish(),
            Self::Stale => f.write_str("Stale"),
            Self::Failed { reason, until_unix } => f
                .debug_struct("Failed")
                .field("reason", reason)
                .field("until_unix", until_unix)
                .finish(),
        }
    }
}

/// The read's original published source and the cache generation that granted
/// it. Redirect targets derive authority from this source, never a new caller URL.
#[derive(Clone)]
pub(crate) struct ContentImageAuthority {
    pub(crate) key: PullRequestKey,
    pub(crate) generation: u64,
    pub(crate) body_digest: String,
    pub(crate) source: ImageSource,
}

pub(crate) type PublishedBodies = HashMap<String, Arc<str>>;

pub(crate) fn body_digest(body: &str) -> String {
    format!("{:x}", Sha256::digest(body.as_bytes()))
}

/// Build before taking the cache mutex. Reads then clone one Arc, not an entire
/// detail or all its Markdown. Neither local drafts nor deleted comments enter.
pub(crate) fn published_bodies(detail: &PullRequestDetail) -> PublishedBodies {
    let bodies = std::iter::once(detail.description.as_str())
        .chain(
            detail
                .conversation
                .iter()
                .filter(|entry| !entry.comment.deleted)
                .map(|entry| entry.comment.body.as_str()),
        )
        .chain(
            detail
                .threads
                .iter()
                .flat_map(|thread| &thread.comments)
                .filter(|comment| !comment.deleted)
                .map(|comment| comment.body.as_str()),
        );
    bodies
        .map(|body| (body_digest(body), Arc::from(body)))
        .collect()
}

/// A validated full DNS answer. Truncation to the client's 16-address
/// capacity happens only AFTER checking every address, never before.
#[derive(Debug, Clone)]
pub(crate) struct PinnedTarget {
    source: ImageSource,
    addresses: Vec<SocketAddr>,
}

impl PinnedTarget {
    pub(crate) fn new(
        source: ImageSource,
        addresses: Vec<SocketAddr>,
    ) -> Result<Self, ContentImageFailure> {
        if !public_resolution(&addresses) {
            return Err(ContentImageFailure::UnsafeAddress);
        }
        Ok(Self { source, addresses })
    }
}

/// No Debug: Location can itself be a signed source. The reader consumes only
/// these headers and a bounded decoded body, never a raw diagnostic response.
pub(crate) struct ContentImageReply {
    pub(crate) status: u16,
    pub(crate) location: Option<String>,
    pub(crate) retry_after: Option<String>,
    pub(crate) body: Box<dyn Read>,
}

pub(crate) trait ContentImageIo: Send + Sync {
    fn unix_time(&self) -> u64 {
        crate::pull_request_read::now_unix()
    }
    fn instant(&self) -> Instant {
        Instant::now()
    }
    fn resolve(
        &self,
        source: &ImageSource,
        remaining: Duration,
    ) -> Result<Vec<SocketAddr>, ContentImageFailure>;
    fn get(
        &self,
        target: &PinnedTarget,
        remaining: Duration,
    ) -> Result<ContentImageReply, ContentImageFailure>;
}

#[derive(Default)]
pub(crate) struct LiveContentImageIo;

// A timed-out OS getaddrinfo cannot be cancelled portably. Keep its permit in
// that worker until it really exits, preventing repeated timeouts from
// accumulating an unbounded number of resolver threads.
static DNS_WORKERS: LazyLock<Arc<AtomicUsize>> = LazyLock::new(|| Arc::new(AtomicUsize::new(0)));

struct DnsPermit(Arc<AtomicUsize>);

impl Drop for DnsPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The OS lookup is the only injected part. The live timeout/worker admission
/// and permit lifetime are exercised without asking the machine's resolver.
fn resolve_image_host(
    host: &str,
    remaining: Duration,
    workers: Arc<AtomicUsize>,
    lookup: impl FnOnce(&str) -> Result<Vec<SocketAddr>, ContentImageFailure> + Send + 'static,
) -> Result<Vec<SocketAddr>, ContentImageFailure> {
    if remaining.is_zero() {
        return Err(ContentImageFailure::Transient);
    }
    workers
        .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < 2).then_some(count + 1)
        })
        .map_err(|_| ContentImageFailure::Transient)?;
    let permit = DnsPermit(workers);
    let host = host.to_owned();
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("pr-image-dns".into())
        .spawn(move || {
            let result = lookup(&host);
            drop(permit);
            let _ = tx.send(result);
        })
        .map_err(|_| ContentImageFailure::Transient)?;
    rx.recv_timeout(remaining)
        .map_err(|_| ContentImageFailure::Transient)?
}

impl ContentImageIo for LiveContentImageIo {
    fn resolve(
        &self,
        source: &ImageSource,
        remaining: Duration,
    ) -> Result<Vec<SocketAddr>, ContentImageFailure> {
        resolve_image_host(source.host(), remaining, DNS_WORKERS.clone(), |host| {
            (host, 443)
                .to_socket_addrs()
                .map(|addresses| addresses.collect())
                .map_err(|_| ContentImageFailure::Transient)
        })
    }

    fn get(
        &self,
        target: &PinnedTarget,
        remaining: Duration,
    ) -> Result<ContentImageReply, ContentImageFailure> {
        if remaining.is_zero() {
            return Err(ContentImageFailure::Transient);
        }
        // No DefaultConnector (which includes proxy connectors) and no
        // DefaultResolver. TLS receives the original URI, not an IP rewrite.
        let connector = ().chain(TcpConnector::default()).chain(RustlsConnector::default());
        let agent = pinned_agent(target, remaining, connector);
        let response = agent
            .get(target.source.as_str())
            .header("Connection", "close")
            .call()
            .map_err(|_| ContentImageFailure::Transient)?;
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        Ok(ContentImageReply {
            status: response.status().as_u16(),
            location: header("location"),
            retry_after: header("retry-after"),
            body: Box::new(response.into_body().into_reader()),
        })
    }
}

#[derive(Debug, Clone)]
struct PinnedResolver(PinnedTarget);

impl Resolver for PinnedResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        _: &ureq::config::Config,
        _: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, Error> {
        if canonical_source(&uri.to_string()).as_ref() != Some(&self.0.source)
            || !public_resolution(&self.0.addresses)
        {
            return Err(Error::ConnectionFailed);
        }
        let mut result = self.empty();
        for address in self.0.addresses.iter().take(16) {
            result.push(*address);
        }
        Ok(result)
    }
}

/// Defence at the actual connector boundary, not just at a prior URL check.
#[derive(Debug)]
struct PublicConnectionGuard(ImageSource);

impl Connector for PublicConnectionGuard {
    type Out = ();

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<()>,
    ) -> Result<Option<()>, Error> {
        if chained.is_some()
            || !details.needs_tls()
            || details.config.proxy().is_some()
            || details.config.tls_config().disable_verification()
            || details.config.max_redirects() != 0
            || canonical_source(&details.uri.to_string()).as_ref() != Some(&self.0)
            || !public_resolution(&details.addrs)
        {
            return Err(Error::ConnectionFailed);
        }
        Ok(None)
    }
}

/// The agent for one image read. Its TLS config trusts the same root set as
/// the provider transport (`outbound-https-trust`: *The image transport uses
/// the same roots*) with verification on; every other rule of this transport
/// is the guard's and the resolver's.
fn pinned_agent(target: &PinnedTarget, remaining: Duration, connector: impl Connector) -> Agent {
    let config = Agent::config_builder()
        .timeout_global(Some(remaining))
        .http_status_as_error(false)
        .proxy(None)
        .max_redirects(0)
        .max_idle_connections(0)
        .max_idle_connections_per_host(0)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(crate::trust_roots::root_certs())
                .disable_verification(false)
                .build(),
        )
        .build();
    Agent::with_parts(
        config,
        PublicConnectionGuard(target.source.clone()).chain(connector),
        PinnedResolver(target.clone()),
    )
}

const READ_TIMEOUT: Duration = Duration::from_secs(15);
const PUBLIC_IMAGE_BUDGET: usize = 120;
const WINDOW_SECS: u64 = 3_600;

#[derive(Default)]
struct LimitState {
    in_flight: usize,
    waiting: VecDeque<u64>,
    next_ticket: u64,
    sent: VecDeque<u64>,
    cooldowns: HashMap<String, u64>,
}

#[derive(Clone, Default)]
pub(crate) struct ContentImageLimits(Arc<(Mutex<LimitState>, Condvar)>);

struct ImagePermit(ContentImageLimits);

impl Drop for ImagePermit {
    fn drop(&mut self) {
        let (state, turn) = &*self.0 .0;
        state.lock().unwrap().in_flight -= 1;
        turn.notify_all();
    }
}

fn failed(reason: ContentImageFailure) -> PullRequestContentImageOutcome {
    PullRequestContentImageOutcome::Failed {
        reason,
        until_unix: None,
    }
}

fn deferred(until: u64) -> PullRequestContentImageOutcome {
    PullRequestContentImageOutcome::Failed {
        reason: ContentImageFailure::Deferred,
        until_unix: Some(until),
    }
}

impl LimitState {
    fn held_until(&mut self, origin: &str, now: u64) -> Option<u64> {
        self.sent
            .retain(|sent| now < sent.saturating_add(WINDOW_SECS));
        self.cooldowns.retain(|_, until| now < *until);
        let budget = if self.sent.len() >= PUBLIC_IMAGE_BUDGET {
            let mut times: Vec<_> = self.sent.iter().copied().collect();
            times.sort_unstable();
            Some(times[times.len() - PUBLIC_IMAGE_BUDGET].saturating_add(WINDOW_SECS))
        } else {
            None
        };
        match (budget, self.cooldowns.get(origin).copied()) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}

impl ContentImageLimits {
    fn acquire(
        &self,
        source: &ImageSource,
        current: &dyn Fn() -> bool,
        io: &dyn ContentImageIo,
    ) -> Result<ImagePermit, PullRequestContentImageOutcome> {
        let (state, turn) = &*self.0;
        let ticket = {
            let mut state = state.lock().unwrap();
            let ticket = state.next_ticket;
            state.next_ticket += 1;
            state.waiting.push_back(ticket);
            ticket
        };
        loop {
            // No preference/file/cache reads under the admission mutex.
            let authorised = current();
            let mut state = state.lock().unwrap();
            let held = state.held_until(&source.origin(), io.unix_time());
            if !authorised || held.is_some() {
                state.waiting.retain(|waiting| *waiting != ticket);
                turn.notify_all();
                return Err(if !authorised {
                    PullRequestContentImageOutcome::Stale
                } else {
                    deferred(held.unwrap())
                });
            }
            if state.in_flight < 2 && state.waiting.front() == Some(&ticket) {
                state.waiting.pop_front();
                state.in_flight += 1;
                return Ok(ImagePermit(self.clone()));
            }
            drop(turn.wait(state).unwrap());
        }
    }

    fn check(&self, source: &ImageSource, now: u64) -> Result<(), PullRequestContentImageOutcome> {
        let held = self.0 .0.lock().unwrap().held_until(&source.origin(), now);
        match held {
            Some(until) => Err(deferred(until)),
            None => Ok(()),
        }
    }

    fn request(
        &self,
        source: &ImageSource,
        now: u64,
    ) -> Result<(), PullRequestContentImageOutcome> {
        let mut state = self.0 .0.lock().unwrap();
        if let Some(until) = state.held_until(&source.origin(), now) {
            return Err(deferred(until));
        }
        state.sent.push_back(now);
        Ok(())
    }

    fn rate_limited(&self, source: &ImageSource, header: Option<&str>, now: u64) -> u64 {
        let seconds = retry_after_seconds(header, now);
        let until = now.saturating_add(seconds);
        let mut state = self.0 .0.lock().unwrap();
        let held = state.cooldowns.entry(source.origin()).or_default();
        *held = (*held).max(until);
        *held
    }
}

fn retry_after_seconds(header: Option<&str>, now: u64) -> u64 {
    let delay = header
        .and_then(|header| {
            let header = header.trim();
            if !header.is_empty() && header.bytes().all(|byte| byte.is_ascii_digit()) {
                return Some(header.parse::<u64>().unwrap_or(u64::MAX));
            }
            chrono::DateTime::parse_from_rfc2822(header)
                .ok()
                .map(|date| {
                    u64::try_from(date.timestamp())
                        .unwrap_or(0)
                        .saturating_sub(now)
                })
        })
        .unwrap_or(60);
    delay.min(WINDOW_SECS)
}

fn bounded_body(body: &mut dyn Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    body.take(REQUESTED_FILE_BYTES_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Called only after cached-source authorisation. Every hop rechecks that same
/// authority, independently pins its public DNS answer, and consumes the
/// service's separate budget. An HTTP body never decides where credentials go.
pub(crate) fn read_content_image(
    source: &ImageSource,
    limits: &ContentImageLimits,
    current: &dyn Fn() -> bool,
    io: &dyn ContentImageIo,
) -> PullRequestContentImageOutcome {
    let _permit = match limits.acquire(source, current, io) {
        Ok(permit) => permit,
        Err(outcome) => return outcome,
    };
    let deadline = io.instant() + READ_TIMEOUT;
    let remaining = || deadline.saturating_duration_since(io.instant());
    let mut history = vec![source.clone()];
    loop {
        let source = history.last().unwrap();
        if !current() {
            return PullRequestContentImageOutcome::Stale;
        }
        if let Err(outcome) = limits.check(source, io.unix_time()) {
            return outcome;
        }
        if remaining().is_zero() {
            return failed(ContentImageFailure::Transient);
        }
        let addresses = match io.resolve(source, remaining()) {
            Ok(addresses) => addresses,
            Err(reason) => return failed(reason),
        };
        let target = match PinnedTarget::new(source.clone(), addresses) {
            Ok(target) => target,
            Err(reason) => return failed(reason),
        };
        if !current() {
            return PullRequestContentImageOutcome::Stale;
        }
        if remaining().is_zero() {
            return failed(ContentImageFailure::Transient);
        }
        if let Err(outcome) = limits.request(source, io.unix_time()) {
            return outcome;
        }
        let mut reply = match io.get(&target, remaining()) {
            Ok(reply) => reply,
            Err(reason) => return failed(reason),
        };
        if !current() {
            return PullRequestContentImageOutcome::Stale;
        }
        if remaining().is_zero() {
            return failed(ContentImageFailure::Transient);
        }
        match reply.status {
            200..=299 => {
                let bytes = match bounded_body(&mut *reply.body) {
                    Ok(bytes) => bytes,
                    Err(_) => return failed(ContentImageFailure::Transient),
                };
                if !current() {
                    return PullRequestContentImageOutcome::Stale;
                }
                if remaining().is_zero() {
                    return failed(ContentImageFailure::Transient);
                }
                return match ImageSide::of(&bytes) {
                    ImageSide::Image {
                        mime,
                        width,
                        height,
                        data,
                    } => PullRequestContentImageOutcome::Image {
                        mime,
                        width,
                        height,
                        data,
                        final_host: source.host().to_owned(),
                    },
                    ImageSide::Refused { reason } => {
                        PullRequestContentImageOutcome::Refused { reason }
                    }
                    ImageSide::Absent => failed(ContentImageFailure::Unavailable),
                };
            }
            300..=399 => {
                let Some(target) = reply
                    .location
                    .as_deref()
                    .and_then(|location| redirect_target(source, location, &history))
                else {
                    return failed(ContentImageFailure::Redirected);
                };
                // Do not read a redirect body, including before disposing it.
                history.push(target);
            }
            429 => {
                return deferred(limits.rate_limited(
                    source,
                    reply.retry_after.as_deref(),
                    io.unix_time(),
                ));
            }
            401 | 403 | 404 | 410 => return failed(ContentImageFailure::Unavailable),
            _ => return failed(ContentImageFailure::Transient),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A blocked admission is a failed assertion, not an indefinitely pending
    /// test/runtime shutdown. The sender lives with the injected image I/O,
    /// including any worker that still owns it after the test returns.
    pub(crate) struct ReadDeadline {
        _alive: mpsc::Sender<()>,
    }

    impl ReadDeadline {
        pub(crate) fn new() -> Self {
            let (alive, ended) = mpsc::channel();
            std::thread::spawn(move || {
                if matches!(
                    ended.recv_timeout(Duration::from_secs(30)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    eprintln!("assertion failed: an injected content-image read did not finish or release its admission");
                    std::process::exit(1);
                }
            });
            Self { _alive: alive }
        }
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;
    use std::io::{self, Cursor};
    use std::sync::atomic::AtomicBool;

    const NOW: u64 = 1_000;
    const PNG: &[u8] = include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two.png");

    struct Reply {
        status: u16,
        location: Option<String>,
        retry_after: Option<String>,
        bytes: Vec<u8>,
        reads: Arc<AtomicUsize>,
    }

    impl Reply {
        fn image(bytes: &[u8]) -> Self {
            Self {
                status: 200,
                location: None,
                retry_after: None,
                bytes: bytes.to_vec(),
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }
        fn redirect(location: &str) -> Self {
            Self {
                status: 302,
                location: Some(location.into()),
                ..Self::image(b"do not read")
            }
        }
    }

    struct CountedBody {
        cursor: Cursor<Vec<u8>>,
        reads: Arc<AtomicUsize>,
    }

    impl Read for CountedBody {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let count = self.cursor.read(out)?;
            self.reads.fetch_add(count, Ordering::AcqRel);
            Ok(count)
        }
    }

    struct FakeIo {
        _deadline: test_support::ReadDeadline,
        replies: Mutex<VecDeque<Reply>>,
        requests: Mutex<Vec<String>>,
        resolutions: Mutex<Vec<String>>,
        durations: Mutex<Vec<Duration>>,
        now: AtomicUsize,
        elapsed_ms: AtomicUsize,
        start: Instant,
        advance_ms: usize,
        current: Arc<AtomicBool>,
        revoke_on_resolve: bool,
        revoke_on_get: bool,
        private_host: Option<String>,
        clock_announced: Mutex<Option<mpsc::SyncSender<()>>>,
    }

    impl FakeIo {
        fn new(replies: Vec<Reply>) -> Self {
            Self {
                _deadline: test_support::ReadDeadline::new(),
                replies: Mutex::new(replies.into()),
                requests: Mutex::new(Vec::new()),
                resolutions: Mutex::new(Vec::new()),
                durations: Mutex::new(Vec::new()),
                now: AtomicUsize::new(NOW as usize),
                elapsed_ms: AtomicUsize::new(0),
                start: Instant::now(),
                advance_ms: 0,
                current: Arc::new(AtomicBool::new(true)),
                revoke_on_resolve: false,
                revoke_on_get: false,
                private_host: None,
                clock_announced: Mutex::new(None),
            }
        }
        fn read(&self, limits: &ContentImageLimits) -> PullRequestContentImageOutcome {
            read_content_image(
                &source(),
                limits,
                &|| self.current.load(Ordering::Acquire),
                self,
            )
        }
    }

    impl ContentImageIo for FakeIo {
        fn unix_time(&self) -> u64 {
            let announced = self.clock_announced.lock().unwrap().take();
            if let Some(tx) = announced {
                tx.send(()).unwrap();
            }
            self.now.load(Ordering::Acquire) as u64
        }
        fn instant(&self) -> Instant {
            self.start + Duration::from_millis(self.elapsed_ms.load(Ordering::Acquire) as u64)
        }
        fn resolve(
            &self,
            source: &ImageSource,
            _: Duration,
        ) -> Result<Vec<SocketAddr>, ContentImageFailure> {
            self.resolutions.lock().unwrap().push(source.host().into());
            if self.revoke_on_resolve {
                self.current.store(false, Ordering::Release);
            }
            let ip = if self.private_host.as_deref() == Some(source.host()) {
                "10.0.0.1:443"
            } else {
                "1.1.1.1:443"
            };
            Ok(vec![ip.parse().unwrap()])
        }
        fn get(
            &self,
            target: &PinnedTarget,
            remaining: Duration,
        ) -> Result<ContentImageReply, ContentImageFailure> {
            assert!(public_resolution(&target.addresses));
            self.requests
                .lock()
                .unwrap()
                .push(target.source.as_str().into());
            self.durations.lock().unwrap().push(remaining);
            self.elapsed_ms.fetch_add(self.advance_ms, Ordering::AcqRel);
            if self.revoke_on_get {
                self.current.store(false, Ordering::Release);
            }
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected GET");
            Ok(ContentImageReply {
                status: reply.status,
                location: reply.location,
                retry_after: reply.retry_after,
                body: Box::new(CountedBody {
                    cursor: Cursor::new(reply.bytes),
                    reads: reply.reads,
                }),
            })
        }
    }

    fn source() -> ImageSource {
        canonical_source("https://images.example/a.png").unwrap()
    }

    #[test]
    fn content_image_read_checks_public_cdn_redirects_and_never_reads_their_bodies() {
        let redirect = Reply::redirect("https://cdn.example/screen.png");
        let reads = redirect.reads.clone();
        let io = FakeIo::new(vec![redirect, Reply::image(PNG)]);
        let result = io.read(&ContentImageLimits::default());
        assert!(matches!(result, PullRequestContentImageOutcome::Image {
            width: 3, height: 2, ref mime, ref final_host, ..
        } if mime == "image/png" && final_host == "cdn.example"));
        assert_eq!(reads.load(Ordering::Acquire), 0);
        assert_eq!(
            io.resolutions.lock().unwrap().as_slice(),
            ["images.example", "cdn.example"]
        );
        assert_eq!(io.requests.lock().unwrap().len(), 2);
    }

    #[test]
    fn content_image_read_rejects_unsafe_redirects_before_connecting() {
        for location in [
            "http://cdn.example/a",
            "https://127.0.0.1/a",
            "https://cdn.local/a",
            "https://user:password@cdn.example/a",
            "https://images.example/a.png#loop",
        ] {
            let io = FakeIo::new(vec![Reply::redirect(location)]);
            assert_eq!(
                io.read(&ContentImageLimits::default()),
                failed(ContentImageFailure::Redirected)
            );
            assert_eq!(io.requests.lock().unwrap().len(), 1);
        }
        let mut io = FakeIo::new(vec![Reply::redirect("https://private.example/a")]);
        io.private_host = Some("private.example".into());
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            failed(ContentImageFailure::UnsafeAddress)
        );
        assert_eq!(io.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn content_image_read_limits_hops_and_shares_one_deadline() {
        let mut io = FakeIo::new(vec![
            Reply::redirect("/one"),
            Reply::redirect("/two"),
            Reply::redirect("/three"),
            Reply::image(PNG),
        ]);
        io.advance_ms = 1_000;
        assert!(matches!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Image { .. }
        ));
        assert_eq!(
            io.durations.lock().unwrap().as_slice(),
            [
                Duration::from_secs(15),
                Duration::from_secs(14),
                Duration::from_secs(13),
                Duration::from_secs(12),
            ]
        );
        let io = FakeIo::new(vec![
            Reply::redirect("/one"),
            Reply::redirect("/two"),
            Reply::redirect("/three"),
            Reply::redirect("/four"),
        ]);
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            failed(ContentImageFailure::Redirected)
        );
        assert_eq!(io.requests.lock().unwrap().len(), 4);
        let body = Reply::image(PNG);
        let reads = body.reads.clone();
        let mut io = FakeIo::new(vec![body]);
        io.advance_ms = 15_000;
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            failed(ContentImageFailure::Transient)
        );
        assert_eq!(reads.load(Ordering::Acquire), 0);
    }

    #[test]
    fn content_image_read_fences_revocation_after_dns_and_after_get() {
        let mut io = FakeIo::new(vec![]);
        io.revoke_on_resolve = true;
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Stale
        );
        assert!(io.requests.lock().unwrap().is_empty());
        let mut io = FakeIo::new(vec![Reply::image(PNG)]);
        io.revoke_on_get = true;
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Stale
        );
    }

    #[test]
    fn content_image_read_inspects_bytes_and_never_reads_past_the_ceiling() {
        for bytes in [
            b"<svg></svg>".as_slice(),
            b"<html>not a PNG</html>".as_slice(),
        ] {
            let io = FakeIo::new(vec![Reply::image(bytes)]);
            assert_eq!(
                io.read(&ContentImageLimits::default()),
                PullRequestContentImageOutcome::Refused {
                    reason: ImageRefusal::NotImage
                }
            );
        }
        let mut huge = Reply::image(PNG);
        huge.bytes.resize(REQUESTED_FILE_BYTES_LIMIT + 100, 0);
        let read_count = huge.reads.clone();
        let io = FakeIo::new(vec![huge]);
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Refused {
                reason: ImageRefusal::TooLarge
            }
        );
        assert_eq!(
            read_count.load(Ordering::Acquire),
            REQUESTED_FILE_BYTES_LIMIT + 1
        );
        let mut pixels = PNG.to_vec();
        pixels[16..20].copy_from_slice(&10_000_u32.to_be_bytes());
        pixels[20..24].copy_from_slice(&10_000_u32.to_be_bytes());
        let io = FakeIo::new(vec![Reply::image(&pixels)]);
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Refused {
                reason: ImageRefusal::TooManyPixels
            }
        );
        let lfs = b"version https://git-lfs.github.com/spec/v1\noid sha256:0000000000000000000000000000000000000000000000000000000000000000\nsize 123\n";
        let io = FakeIo::new(vec![Reply::image(lfs)]);
        assert_eq!(
            io.read(&ContentImageLimits::default()),
            PullRequestContentImageOutcome::Refused {
                reason: ImageRefusal::Lfs
            }
        );
    }

    #[test]
    fn content_image_budget_counts_hops_is_shared_and_expires_at_the_boundary() {
        let limits = ContentImageLimits::default();
        let cloned = limits.clone();
        for _ in 0..PUBLIC_IMAGE_BUDGET - 1 {
            limits.request(&source(), NOW).unwrap();
        }
        let io = FakeIo::new(vec![Reply::redirect("https://cdn.example/a")]);
        assert_eq!(io.read(&cloned), deferred(NOW + WINDOW_SECS));
        assert_eq!(io.requests.lock().unwrap().len(), 1);
        let held = FakeIo::new(vec![]);
        assert_eq!(held.read(&limits), deferred(NOW + WINDOW_SECS));
        assert!(held.resolutions.lock().unwrap().is_empty());
        held.current.store(false, Ordering::Release);
        assert_eq!(held.read(&limits), PullRequestContentImageOutcome::Stale);
        held.current.store(true, Ordering::Release);
        assert_eq!(held.read(&limits), deferred(NOW + WINDOW_SECS));
        held.now
            .store((NOW + WINDOW_SECS - 1) as usize, Ordering::Release);
        assert_eq!(held.read(&limits), deferred(NOW + WINDOW_SECS));
        let fresh = FakeIo::new(vec![Reply::image(PNG)]);
        fresh
            .now
            .store((NOW + WINDOW_SECS) as usize, Ordering::Release);
        assert!(matches!(
            fresh.read(&limits),
            PullRequestContentImageOutcome::Image { .. }
        ));
    }

    #[test]
    fn content_image_cooldown_is_per_origin_bounded_and_does_not_retry() {
        let limits = ContentImageLimits::default();
        let io = FakeIo::new(vec![Reply {
            status: 429,
            retry_after: Some("999999999999999999999".into()),
            ..Reply::image(b"")
        }]);
        assert_eq!(io.read(&limits), deferred(NOW + WINDOW_SECS));
        assert_eq!(io.read(&limits), deferred(NOW + WINDOW_SECS));
        assert_eq!(io.requests.lock().unwrap().len(), 1);
        let other = canonical_source("https://cdn.example/a").unwrap();
        assert!(limits.check(&other, NOW).is_ok());
        assert_eq!(retry_after_seconds(None, NOW), 60);
        assert_eq!(retry_after_seconds(Some("invalid"), NOW), 60);
        assert_eq!(retry_after_seconds(Some("0"), NOW), 0);
        assert_eq!(retry_after_seconds(Some("20"), NOW), 20);
        assert_eq!(
            retry_after_seconds(Some("Thu, 01 Jan 1970 00:17:00 GMT"), NOW),
            20
        );
    }

    #[test]
    fn content_image_cooldowns_expire_at_the_exact_boundary() {
        let limits = ContentImageLimits::default();
        assert_eq!(limits.rate_limited(&source(), Some("20"), NOW), NOW + 20);
        assert_eq!(limits.check(&source(), NOW + 19), Err(deferred(NOW + 20)));
        assert_eq!(limits.check(&source(), NOW + 20), Ok(()));
        assert!(limits.0 .0.lock().unwrap().cooldowns.is_empty());
    }

    #[test]
    fn content_image_rolling_budget_releases_the_oldest_distinct_request() {
        let limits = ContentImageLimits::default();
        for offset in 0..PUBLIC_IMAGE_BUDGET {
            limits.request(&source(), NOW + offset as u64).unwrap();
        }
        assert_eq!(
            limits.check(&source(), NOW + 120),
            Err(deferred(NOW + WINDOW_SECS))
        );
        assert_eq!(
            limits.check(&source(), NOW + WINDOW_SECS - 1),
            Err(deferred(NOW + WINDOW_SECS))
        );
        limits.request(&source(), NOW + WINDOW_SECS).unwrap();
        assert_eq!(
            limits.check(&source(), NOW + WINDOW_SECS),
            Err(deferred(NOW + WINDOW_SECS + 1))
        );
    }

    #[test]
    fn content_image_redirect_cooldown_is_checked_before_resolving_the_next_hop() {
        let limits = ContentImageLimits::default();
        let cdn = canonical_source("https://cdn.example/a").unwrap();
        limits.rate_limited(&cdn, Some("60"), NOW);
        let io = FakeIo::new(vec![Reply::redirect(cdn.as_str())]);
        assert_eq!(io.read(&limits), deferred(NOW + 60));
        assert_eq!(
            io.resolutions.lock().unwrap().as_slice(),
            ["images.example"]
        );
        assert_eq!(io.requests.lock().unwrap().len(), 1);
    }

    #[test]
    fn content_image_protected_and_missing_responses_are_unavailable_without_reading_bodies() {
        for status in [401, 403, 404, 410] {
            let mut reply = Reply::image(b"private login response");
            reply.status = status;
            let body_reads = reply.reads.clone();
            let io = FakeIo::new(vec![reply]);
            assert_eq!(
                io.read(&ContentImageLimits::default()),
                failed(ContentImageFailure::Unavailable)
            );
            assert_eq!(body_reads.load(Ordering::Acquire), 0);
            assert_eq!(io.requests.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn content_image_waiting_tickets_are_unique_and_admitted_in_order() {
        let limits = ContentImageLimits::default();
        let io = Arc::new(FakeIo::new(vec![]));
        let first = limits.acquire(&source(), &|| true, &*io).unwrap();
        let second = limits.acquire(&source(), &|| true, &*io).unwrap();
        let (finished, results) = mpsc::channel();
        let mut workers = Vec::new();
        for index in 0..2 {
            let (announced, entered) = mpsc::sync_channel(1);
            let announced = Mutex::new(Some(announced));
            let limits = limits.clone();
            let io = io.clone();
            let finished = finished.clone();
            workers.push(std::thread::spawn(move || {
                let result = limits.acquire(
                    &source(),
                    &|| {
                        let announced = announced.lock().unwrap().take();
                        if let Some(announced) = announced {
                            announced.send(()).unwrap();
                        }
                        true
                    },
                    &*io,
                );
                finished.send((index, result)).unwrap();
            }));
            entered.recv_timeout(Duration::from_secs(30)).unwrap();
        }
        {
            let state = limits.0 .0.lock().unwrap();
            assert_eq!(state.waiting.len(), 2);
            assert_ne!(
                state.waiting[0], state.waiting[1],
                "canceling one waiter must not erase another's ticket"
            );
        }
        drop(first);
        let (index, permit) = results.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(index, 0);
        assert!(results.try_recv().is_err());
        drop(permit.unwrap());
        let (index, permit) = results.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(index, 1);
        drop(permit.unwrap());
        drop(second);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(limits.0 .0.lock().unwrap().in_flight, 0);
    }

    #[test]
    fn content_image_waiters_recheck_authority_when_one_of_two_slots_frees() {
        let limits = ContentImageLimits::default();
        let io = Arc::new(FakeIo::new(vec![]));
        let first = limits.acquire(&source(), &|| true, &*io).unwrap();
        let second = limits.acquire(&source(), &|| true, &*io).unwrap();
        let (asked, started) = mpsc::sync_channel(1);
        *io.clock_announced.lock().unwrap() = Some(asked);
        let (completed, result) = mpsc::sync_channel(1);
        let enabled = Arc::new(AtomicBool::new(true));
        let worker_limits = limits.clone();
        let worker_io = io.clone();
        let worker_enabled = enabled.clone();
        let worker = std::thread::spawn(move || {
            let outcome = worker_limits.acquire(
                &source(),
                &|| worker_enabled.load(Ordering::Acquire),
                &*worker_io,
            );
            completed.send(outcome.map(|_| ())).unwrap();
        });
        started.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(limits.0 .0.lock().unwrap().in_flight, 2);
        assert!(result.try_recv().is_err());
        enabled.store(false, Ordering::Release);
        drop(first);
        assert_eq!(
            result.recv_timeout(Duration::from_secs(30)).unwrap(),
            Err(PullRequestContentImageOutcome::Stale)
        );
        worker.join().unwrap();
        drop(second);
        assert_eq!(limits.0 .0.lock().unwrap().in_flight, 0);
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Mutex;
    use ureq::unversioned::transport::time::Instant as UreqInstant;
    use ureq::unversioned::transport::{Buffers, LazyBuffers, Transport};

    #[derive(Debug, Clone)]
    struct CaptureConnector {
        request: Arc<Mutex<Vec<u8>>>,
        connections: Arc<AtomicUsize>,
        response: Vec<u8>,
    }

    impl Connector for CaptureConnector {
        type Out = CaptureTransport;

        fn connect(
            &self,
            details: &ConnectionDetails,
            chained: Option<()>,
        ) -> Result<Option<Self::Out>, Error> {
            assert!(chained.is_none());
            assert!(details.needs_tls());
            assert_eq!(details.uri.host(), Some("images.example"));
            assert!(details.config.proxy().is_none());
            assert!(!details.config.tls_config().disable_verification());
            // `outbound-https-trust`: *The image transport uses the same
            // roots* as the provider transport, the process's one set.
            let (ureq::tls::RootCerts::Specific(roots), ureq::tls::RootCerts::Specific(shared)) = (
                details.config.tls_config().root_certs(),
                crate::trust_roots::root_certs(),
            ) else {
                panic!("specific roots");
            };
            assert!(Arc::ptr_eq(roots, &shared));
            assert_eq!(details.config.max_redirects(), 0);
            assert_eq!(details.config.max_idle_connections(), 0);
            assert_eq!(details.config.max_idle_connections_per_host(), 0);
            assert_eq!(&details.addrs[..], ["1.1.1.1:443".parse().unwrap()]);
            self.connections.fetch_add(1, Ordering::AcqRel);
            Ok(Some(CaptureTransport {
                buffers: LazyBuffers::new(1024, 1024),
                request: self.request.clone(),
                response: Cursor::new(self.response.clone()),
            }))
        }
    }

    #[derive(Debug)]
    struct CaptureTransport {
        buffers: LazyBuffers,
        request: Arc<Mutex<Vec<u8>>>,
        response: Cursor<Vec<u8>>,
    }

    impl Transport for CaptureTransport {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }
        fn transmit_output(&mut self, amount: usize, _: NextTimeout) -> Result<(), Error> {
            self.request
                .lock()
                .unwrap()
                .extend_from_slice(&self.buffers.output()[..amount]);
            Ok(())
        }
        fn await_input(&mut self, _: NextTimeout) -> Result<bool, Error> {
            let read = self
                .response
                .read(self.buffers.input_append_buf())
                .map_err(Error::Io)?;
            self.buffers.input_appended(read);
            Ok(read > 0)
        }
        fn is_open(&mut self) -> bool {
            true
        }
        fn is_tls(&self) -> bool {
            true
        }
    }

    fn target() -> PinnedTarget {
        PinnedTarget::new(
            canonical_source("https://images.example/a?sig=secret").unwrap(),
            vec!["1.1.1.1:443".parse().unwrap()],
        )
        .unwrap()
    }

    #[test]
    fn content_image_diagnostics_keep_typed_metadata_but_no_url_or_bytes() {
        let image = PullRequestContentImageOutcome::Image {
            mime: "image/png".into(),
            width: 3,
            height: 2,
            data: "private-image-bytes".into(),
            final_host: "private-host.example".into(),
        };
        let debug = format!("{image:?}");
        assert!(debug.contains("width: 3") && debug.contains("height: 2"));
        assert!(!debug.contains("private"));
        assert_eq!(
            format!("{:?}", PullRequestContentImageOutcome::Stale),
            "Stale"
        );
        assert_eq!(
            format!(
                "{:?}",
                PullRequestContentImageOutcome::Refused {
                    reason: ImageRefusal::Lfs
                }
            ),
            "Refused { reason: Lfs }"
        );
        let failure = format!("{:?}", deferred(1_234));
        assert!(failure.contains("Deferred") && failure.contains("1234"));
    }

    #[test]
    fn content_image_live_clock_and_zero_deadlines_use_the_transport_contract() {
        let io = LiveContentImageIo;
        let before = crate::pull_request_read::now_unix();
        let now = io.unix_time();
        let after = crate::pull_request_read::now_unix();
        assert!((before..=after).contains(&now));
        assert!(io.resolve(&target().source, Duration::ZERO).is_err());
        assert!(io.get(&target(), Duration::ZERO).is_err());
    }

    #[test]
    fn content_image_dns_workers_keep_timed_out_slots_until_the_lookup_really_ends() {
        let workers = Arc::new(AtomicUsize::new(0));
        let expected = vec!["1.1.1.1:443".parse().unwrap()];
        assert_eq!(
            resolve_image_host(
                "images.example",
                Duration::from_secs(30),
                workers.clone(),
                |host| {
                    assert_eq!(host, "images.example");
                    Ok(vec!["1.1.1.1:443".parse().unwrap()])
                }
            )
            .unwrap(),
            expected
        );
        assert_eq!(workers.load(Ordering::Acquire), 0);
        assert_eq!(
            resolve_image_host("images.example", Duration::ZERO, workers.clone(), |_| {
                panic!("an expired request must not start DNS")
            }),
            Err(ContentImageFailure::Transient)
        );
        let mut releases = Vec::new();
        for count in 1..=2 {
            let (release, wait) = mpsc::sync_channel(1);
            releases.push(release);
            let result = resolve_image_host(
                "images.example",
                Duration::from_millis(1),
                workers.clone(),
                move |_| {
                    wait.recv_timeout(Duration::from_secs(30)).unwrap();
                    Ok(vec!["1.1.1.1:443".parse().unwrap()])
                },
            );
            assert_eq!(result, Err(ContentImageFailure::Transient));
            assert_eq!(workers.load(Ordering::Acquire), count);
        }
        let blocked = resolve_image_host(
            "images.example",
            Duration::from_secs(30),
            workers.clone(),
            |_| Err(ContentImageFailure::UnsafeAddress),
        );
        assert_eq!(blocked, Err(ContentImageFailure::Transient));
        assert_eq!(workers.load(Ordering::Acquire), 2);
        for release in releases {
            release.send(()).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while workers.load(Ordering::Acquire) != 0 {
            assert!(
                Instant::now() < deadline,
                "finished DNS workers leaked their permits"
            );
            std::thread::yield_now();
        }
        assert!(
            resolve_image_host("images.example", Duration::from_secs(30), workers, |_| Err(
                ContentImageFailure::Unavailable
            ),)
            .is_err()
        );
    }

    #[test]
    fn content_image_connection_guard_independently_refuses_every_unsafe_detail() {
        let target = target();
        let resolver = PinnedResolver(target.clone());
        for case in [
            "safe", "chained", "http", "proxy", "tls", "redirect", "uri", "empty", "mixed", "port",
        ] {
            let uri = match case {
                "http" => "http://images.example/a?sig=secret",
                "uri" => "https://other.example/a?sig=secret",
                _ => target.source.as_str(),
            }
            .parse()
            .unwrap();
            let config = Agent::config_builder()
                .proxy(if case == "proxy" {
                    Some(ureq::Proxy::new("http://127.0.0.1:8000").unwrap())
                } else {
                    None
                })
                .max_redirects(if case == "redirect" { 1 } else { 0 })
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .root_certs(crate::trust_roots::root_certs())
                        .disable_verification(case == "tls")
                        .build(),
                )
                .build();
            let mut addrs = resolver.empty();
            if case != "empty" {
                addrs.push(
                    if case == "port" {
                        "1.1.1.1:80"
                    } else {
                        "1.1.1.1:443"
                    }
                    .parse()
                    .unwrap(),
                );
            }
            if case == "mixed" {
                addrs.push("10.0.0.1:443".parse().unwrap());
            }
            let details = ConnectionDetails {
                uri: &uri,
                addrs,
                config: &config,
                request_level: false,
                resolver: &resolver,
                now: UreqInstant::now(),
                timeout: NextTimeout {
                    after: Duration::from_secs(1).into(),
                    reason: ureq::Timeout::Global,
                },
                current_time: Arc::new(UreqInstant::now),
                run_connector: Arc::new(|_| panic!("the guard must never open a connection")),
            };
            let result = PublicConnectionGuard(target.source.clone())
                .connect(&details, (case == "chained").then_some(()));
            assert_eq!(result.is_ok(), case == "safe", "{case}");
        }
    }

    #[test]
    fn content_image_transport_uses_pins_original_host_and_no_credentials() {
        let request = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let connector = CaptureConnector {
            request: request.clone(),
            connections: connections.clone(),
            response: b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK"
                .to_vec(),
        };
        let target = target();
        let agent = pinned_agent(&target, Duration::from_secs(15), connector);
        let mut response = agent
            .get(target.source.as_str())
            .header("Connection", "close")
            .call()
            .unwrap();
        assert_eq!(response.body_mut().read_to_string().unwrap(), "OK");
        let sent = String::from_utf8(request.lock().unwrap().clone())
            .unwrap()
            .to_ascii_lowercase();
        assert!(sent.starts_with("get /a?sig=secret http/1.1\r\n"));
        assert!(sent.contains("host: images.example\r\n"));
        for forbidden in [
            "authorization:",
            "proxy-authorization:",
            "cookie:",
            "referer:",
            "origin:",
        ] {
            assert!(!sent.contains(forbidden), "{forbidden}");
        }
        assert_eq!(connections.load(Ordering::Acquire), 1);
    }

    #[test]
    fn content_image_transport_does_not_automatically_follow_redirects() {
        let request = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let connector = CaptureConnector {
            request, connections: connections.clone(),
            response: b"HTTP/1.1 302 Found\r\nLocation: https://private.local/a\r\nContent-Length: 0\r\n\r\n".to_vec(),
        };
        let target = target();
        let response = pinned_agent(&target, Duration::from_secs(15), connector)
            .get(target.source.as_str())
            .call()
            .unwrap();
        assert_eq!(response.status().as_u16(), 302);
        assert_eq!(connections.load(Ordering::Acquire), 1);
    }

    #[test]
    fn content_image_transport_caps_actual_gzip_expansion_not_just_content_length() {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;
        let png = include_bytes!("../../openspec-core/tests/fixtures/images/three-by-two.png");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(png).unwrap();
        io::copy(
            &mut io::repeat(0).take(REQUESTED_FILE_BYTES_LIMIT as u64),
            &mut encoder,
        )
        .unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(compressed.len() < REQUESTED_FILE_BYTES_LIMIT);
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            compressed.len()
        ).into_bytes();
        response.extend_from_slice(&compressed);
        let connector = CaptureConnector {
            request: Arc::new(Mutex::new(Vec::new())),
            connections: Arc::new(AtomicUsize::new(0)),
            response,
        };
        let target = target();
        let response = pinned_agent(&target, Duration::from_secs(15), connector)
            .get(target.source.as_str())
            .call()
            .unwrap();
        let decoded = bounded_body(&mut response.into_body().into_reader()).unwrap();
        assert_eq!(decoded.len(), REQUESTED_FILE_BYTES_LIMIT + 1);
        assert!(matches!(
            ImageSide::of(&decoded),
            ImageSide::Refused {
                reason: ImageRefusal::TooLarge
            }
        ));
    }

    #[test]
    fn content_image_transport_rejects_private_answers_even_past_client_capacity() {
        let mut addresses = vec!["1.1.1.1:443".parse().unwrap(); 16];
        addresses.push("10.0.0.1:443".parse().unwrap());
        assert!(matches!(
            PinnedTarget::new(target().source, addresses),
            Err(ContentImageFailure::UnsafeAddress)
        ));
    }

    #[test]
    fn content_image_pinned_resolver_never_resolves_a_different_uri() {
        let resolver = PinnedResolver(target());
        let timeout = NextTimeout {
            after: Duration::from_secs(1).into(),
            reason: ureq::Timeout::Global,
        };
        assert!(resolver
            .resolve(
                &"https://other.example/a".parse().unwrap(),
                &ureq::config::Config::default(),
                timeout,
            )
            .is_err());
    }
}
