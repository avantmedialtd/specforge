//! `specforge-serve` — the standalone headless web server for SpecForge.
//!
//! Bootstraps its own [`openspec_app::AppService`] from the shared config
//! directory (exactly as `specforge-tui` does) and serves only the web UI — no
//! tray, no dock, no native window. The browser is the only skin.
//!
//! Port resolution: `--port <n>` / `--port=<n>`, else `SPECFORGE_WEB_PORT`, else
//! the default. Binds the loopback interface only.
//!
//! Note: running this *alongside* the desktop app means two `AppService`
//! instances against the same config dir, reintroducing the documented
//! two-writer `activity.json` contention. The desktop app's embedded toggle is
//! the contention-free way to have both skins at once.

use std::net::SocketAddr;

const DEFAULT_PORT: u16 = 4317;

#[tokio::main]
async fn main() {
    let Some(config_dir) = openspec_app::config_dir() else {
        eprintln!("could not resolve the SpecForge configuration directory");
        std::process::exit(1);
    };

    let svc = openspec_app::AppService::bootstrap(config_dir);
    svc.populate().await;
    // Mirror the other frontends' startup so the dashboard has history and the
    // opt-in quota gauge works when enabled.
    svc.spawn_backfill();
    svc.spawn_quota_poller();

    let port = resolve_port().unwrap_or(DEFAULT_PORT);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("SpecForge web UI on http://{addr}");
    if let Err(e) = specforge_web::serve(svc, addr).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

/// Resolve the listen port from `--port <n>` / `--port=<n>`, falling back to the
/// `SPECFORGE_WEB_PORT` environment variable.
fn resolve_port() -> Option<u16> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--port=") {
            return value.parse().ok();
        }
        if arg == "--port" {
            return args.next().and_then(|v| v.parse().ok());
        }
    }
    std::env::var("SPECFORGE_WEB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
}
