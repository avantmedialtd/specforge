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
use tokio::sync::mpsc;

use app::{Model, Msg};

#[tokio::main]
async fn main() -> io::Result<()> {
    let Some(config_dir) = openspec_app::config_dir() else {
        eprintln!("could not resolve the SpecForge configuration directory");
        std::process::exit(1);
    };
    // Resolve terminal capabilities from the environment before any TTY work.
    theme::theme();

    let svc = AppService::bootstrap(config_dir);
    svc.populate().await;

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
        _ => run_tui(svc).await,
    }
}

async fn run_tui(svc: AppService) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let mut cache_rx = svc.subscribe();
    // Start the opt-in Claude quota poll loop (no-op while disabled). Subscribed
    // above first, so we never miss its initial `QuotaUpdated` emit.
    svc.spawn_quota_poller();
    let (tx, mut data_rx) = mpsc::unbounded_channel::<Msg>();
    let mut model = Model::new(&svc);

    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    let res = loop {
        if let Err(e) = terminal.draw(|f| ui::view(f, &model)) {
            break Err(e);
        }
        if model.should_quit {
            break Ok(());
        }
        let msg = tokio::select! {
            ev = events.next() => translate(ev),
            r = cache_rx.recv() => r.ok().map(|ev| match ev {
                CacheEvent::QuotaUpdated => Msg::Quota,
                _ => Msg::Cache,
            }),
            _ = tick.tick() => Some(Msg::Tick),
            m = data_rx.recv() => m,
        };
        if let Some(msg) = msg {
            app::update(&mut model, msg, &svc, &tx);
        }
    };

    restore_terminal(&mut terminal)?;
    res
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
