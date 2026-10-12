//! `specforge-tui` — the terminal frontend for SpecForge.
//!
//! One binary, three faces: the default interactive TUI, `--status` (print a
//! snapshot and exit), and `--line` (one ambient status line and exit). All
//! three read the same headless [`openspec_app::AppService`] the desktop shell
//! uses — in-process, with no IPC.

mod app;
mod graph;
mod markdown;
mod modes;
mod prefs;
mod theme;
mod ui;

#[cfg(test)]
mod render_tests;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{Event as CEvent, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use openspec_app::AppService;
use openspec_core::CacheEvent;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::broadcast::{
    self,
    error::{RecvError, TryRecvError},
};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use app::{Model, Msg};

#[tokio::main]
async fn main() -> io::Result<()> {
    let Some(config_dir) = openspec_app::config_dir() else {
        eprintln!("could not resolve the SpecForge configuration directory");
        std::process::exit(1);
    };
    // Resolve terminal capabilities from the environment before any TTY work.
    theme::theme();

    let svc = AppService::bootstrap(config_dir.clone());
    svc.populate().await;

    // Apply the persisted colour scheme (if any) before any rendering.
    if let Some(scheme) = prefs::load_scheme(&config_dir) {
        theme::set_scheme(scheme);
    }

    match std::env::args().nth(1).as_deref() {
        Some("--line") => {
            modes::line(&svc);
            Ok(())
        }
        Some("--status") => {
            modes::status(&svc);
            Ok(())
        }
        Some(other) if other.starts_with('-') => {
            eprintln!("unknown flag: {other}\nusage: specforge-tui [--status | --line]");
            std::process::exit(2);
        }
        _ => run_tui(svc, config_dir).await,
    }
}

async fn run_tui(svc: AppService, config_dir: std::path::PathBuf) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let mut cache_rx = svc.subscribe();
    // Start the opt-in Claude quota poll loop (no-op while disabled). Subscribed
    // above first, so we never miss its initial `QuotaUpdated` emit.
    svc.spawn_quota_poller();
    let (tx, mut data_rx) = mpsc::unbounded_channel::<Msg>();
    let mut model = Model::new(&svc);
    model.config_dir = Some(config_dir);

    let mut events = EventStream::new();
    // The tick only refreshes time-dependent surfaces — the quota countdown
    // and its "now" marker — and both have minute resolution, so once a
    // second is plenty (`terminal-ui`: *Rendering Work Is Reused Across
    // Frames*). `Delay` keeps a stalled loop from firing a burst of catch-up
    // ticks.
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    // A closed watcher channel resolves `recv` immediately forever; the branch
    // is switched off rather than allowed to spin the loop.
    let mut cache_closed = false;
    // Draw only after something the view reads may have changed: an applied
    // message (input, watcher burst, async reply, resize or tick). A key
    // release or a pull-request-only burst applies nothing and draws nothing.
    let mut needs_draw = true;

    let res = loop {
        if needs_draw {
            if let Err(e) = terminal.draw(|f| ui::view(f, &model)) {
                break Err(e);
            }
            needs_draw = false;
        }
        if model.should_quit {
            break Ok(());
        }
        let msgs: [Option<Msg>; 2] = tokio::select! {
            ev = events.next() => [translate(ev), None],
            r = cache_rx.recv(), if !cache_closed => {
                let (burst, closed) = drain_cache(r, &mut cache_rx);
                cache_closed |= closed;
                burst.messages()
            }
            _ = tick.tick() => [Some(Msg::Tick), None],
            m = data_rx.recv() => [m, None],
        };
        for msg in msgs.into_iter().flatten() {
            app::update(&mut model, msg, &svc, &tx);
            needs_draw = true;
        }
    };

    restore_terminal(&mut terminal)?;
    res
}

/// How many already-queued events one loop turn folds into a burst. The
/// watcher's broadcast holds 128, so a whole backlog fits; the bound only
/// keeps a pathological flood from starving keyboard input — anything left
/// over wakes the next turn.
const DRAIN_LIMIT: usize = 128;

/// What one turn's worth of watcher events asks the loop to do (`terminal-ui`:
/// *Watcher Bursts Coalesce Into One Refresh*).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct CacheBurst {
    /// Re-read the tree, the Settings mirror, the open artifact and the open
    /// graph — one [`Msg::Cache`] however many events asked for it.
    refresh: bool,
    /// Re-read the quota snapshots — one [`Msg::Quota`].
    quota: bool,
}

impl CacheBurst {
    /// Fold one event into the burst.
    fn note(&mut self, event: &CacheEvent) {
        match event {
            CacheEvent::QuotaUpdated => self.quota = true,
            // The terminal starts neither pull-request poller and renders
            // no pull-request list (`bitbucket-pull-requests`: *The
            // Terminal Frontend Does Not Render the Panel*;
            // `github-pull-requests`: *The Terminal Frontend Does Not
            // Render the GitHub Panel*), so a snapshot change has nothing
            // to redraw here — and a refresh would re-read every workspace
            // view for nothing.
            CacheEvent::BitbucketPullRequestsUpdated | CacheEvent::GithubPullRequestsUpdated => {}
            // Every other variant (file, change, workspace, logical-change,
            // instance and graph events) may change the tree, the open
            // artifact or the commit graph; a variant added later is safest
            // treated the same way.
            _ => self.refresh = true,
        }
    }

    /// The messages the loop applies for this burst, in order.
    fn messages(self) -> [Option<Msg>; 2] {
        [
            self.refresh.then_some(Msg::Cache),
            self.quota.then_some(Msg::Quota),
        ]
    }
}

/// Classify the event that woke the loop, then drain every event already
/// waiting (up to [`DRAIN_LIMIT`]) into the same burst — no timer, so the
/// refresh lands in this very turn. A lagged receiver skipped events it can
/// no longer see, so it counts as a refresh rather than nothing. Returns
/// whether the channel has closed.
fn drain_cache(
    first: Result<CacheEvent, RecvError>,
    rx: &mut broadcast::Receiver<CacheEvent>,
) -> (CacheBurst, bool) {
    let mut burst = CacheBurst::default();
    match first {
        Ok(event) => burst.note(&event),
        Err(RecvError::Lagged(_)) => burst.refresh = true,
        Err(RecvError::Closed) => return (burst, true),
    }
    for _ in 0..DRAIN_LIMIT {
        match rx.try_recv() {
            Ok(event) => burst.note(&event),
            Err(TryRecvError::Lagged(_)) => burst.refresh = true,
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Closed) => return (burst, true),
        }
    }
    (burst, false)
}

/// Translate a crossterm event into a `Msg`, filtering out key-release events
/// (which some terminals send and which would double-fire bindings).
fn translate(ev: Option<Result<CEvent, io::Error>>) -> Option<Msg> {
    match ev {
        Some(Ok(CEvent::Key(k)))
            if matches!(k.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
        {
            Some(Msg::Key(k))
        }
        Some(Ok(CEvent::Resize(_, _))) => Some(Msg::Resize),
        _ => None,
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

/// Restore the terminal on panic before the default hook prints — mandatory
/// under `panic = "abort"`, or a crash leaves the terminal in raw mode.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn updated() -> CacheEvent {
        CacheEvent::Updated {
            workspace: PathBuf::from("/ws"),
        }
    }

    fn change_added() -> CacheEvent {
        CacheEvent::ChangeAdded {
            workspace: PathBuf::from("/ws"),
            change_id: "demo".to_string(),
        }
    }

    /// Send `events`, then wake the way the loop does — one `recv` — and drain.
    async fn burst_of(capacity: usize, events: Vec<CacheEvent>) -> (CacheBurst, bool) {
        let (tx, mut rx) = broadcast::channel(capacity);
        for event in events {
            tx.send(event).unwrap();
        }
        let first = rx.recv().await;
        let out = drain_cache(first, &mut rx);
        drop(tx);
        out
    }

    #[tokio::test]
    async fn a_mixed_burst_yields_one_refresh() {
        let events = vec![
            updated(),
            change_added(),
            CacheEvent::GraphChanged {
                repo_id: PathBuf::from("/ws"),
            },
            CacheEvent::GithubPullRequestsUpdated,
            updated(),
        ];
        let (burst, closed) = burst_of(16, events).await;
        assert_eq!(
            burst,
            CacheBurst {
                refresh: true,
                quota: false
            }
        );
        assert!(!closed);
        let msgs = burst.messages();
        assert!(matches!(msgs, [Some(Msg::Cache), None]));
    }

    #[tokio::test]
    async fn a_pull_request_only_burst_yields_nothing() {
        let events = vec![
            CacheEvent::BitbucketPullRequestsUpdated,
            CacheEvent::GithubPullRequestsUpdated,
            CacheEvent::BitbucketPullRequestsUpdated,
        ];
        let (burst, closed) = burst_of(16, events).await;
        assert_eq!(burst, CacheBurst::default());
        assert!(!closed);
        assert!(matches!(burst.messages(), [None, None]));
    }

    #[tokio::test]
    async fn quota_and_a_change_yield_both_flags() {
        let (burst, _) = burst_of(16, vec![CacheEvent::QuotaUpdated, updated()]).await;
        assert_eq!(
            burst,
            CacheBurst {
                refresh: true,
                quota: true
            }
        );
        assert!(matches!(
            burst.messages(),
            [Some(Msg::Cache), Some(Msg::Quota)]
        ));
    }

    #[tokio::test]
    async fn a_lagged_receiver_still_refreshes() {
        // Only pull-request events, so the refresh can come from the lag
        // alone: the channel holds two, five were sent, three were skipped.
        let events = vec![CacheEvent::GithubPullRequestsUpdated; 5];
        let (burst, closed) = burst_of(2, events).await;
        assert!(burst.refresh, "skipped events count as a change");
        assert!(!burst.quota);
        assert!(!closed);
    }

    #[tokio::test]
    async fn a_closed_channel_is_reported() {
        let (tx, mut rx) = broadcast::channel(4);
        tx.send(updated()).unwrap();
        drop(tx);
        let first = rx.recv().await;
        let (burst, closed) = drain_cache(first, &mut rx);
        assert!(burst.refresh, "the event before the close still counts");
        assert!(closed);

        // Woken by the close itself.
        let first = rx.recv().await;
        let (burst, closed) = drain_cache(first, &mut rx);
        assert_eq!(burst, CacheBurst::default());
        assert!(closed);
    }

    #[tokio::test]
    async fn the_drain_is_bounded() {
        let (tx, mut rx) = broadcast::channel(DRAIN_LIMIT * 2);
        for _ in 0..DRAIN_LIMIT + 10 {
            tx.send(updated()).unwrap();
        }
        let first = rx.recv().await;
        let (burst, closed) = drain_cache(first, &mut rx);
        assert!(burst.refresh);
        assert!(!closed);
        // One woke the loop and `DRAIN_LIMIT` were drained; the rest wait
        // for the next turn rather than holding keyboard input up.
        assert_eq!(rx.len(), 9);
    }
}
