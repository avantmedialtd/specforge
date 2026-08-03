//! `specforge-serve` — the standalone headless web server for SpecForge.
//!
//! Bootstraps its own [`openspec_app::AppService`] from the shared config
//! directory (exactly as `specforge-tui` does) and serves only the web UI — no
//! tray, no dock, no native window. The browser is the only skin.
//!
//! Port resolution: `--port <n>` / `--port=<n>`, else `SPECFORGE_WEB_PORT`, else
//! the default. Bind resolution: `--bind <addr>` / `--bind=<addr>`, else
//! `SPECFORGE_WEB_BIND`, else `127.0.0.1` (loopback) — see [`resolve_bind`]. A
//! non-loopback bind disables the request-authority allowlist and is announced
//! at startup; see `design.md` in the `add-network-bind-serve` change for
//! exactly what that trades away.
//!
//! Note: running this *alongside* the desktop app means two `AppService`
//! instances against the same config dir, reintroducing the documented
//! two-writer `activity.json` contention. The desktop app's embedded toggle is
//! the contention-free way to have both skins at once.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

const DEFAULT_PORT: u16 = 4317;
const DEFAULT_BIND: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

const USAGE: &str = "usage: specforge-serve [--bind <addr>] [--port <port>] [--help] [--version]";

#[tokio::main]
async fn main() {
    // Argument validation (help/version/unknown flags) happens before any of
    // config-dir resolution, AppService bootstrap, or socket work, so
    // `--help`/`--version` are cheap and side-effect-free no matter the
    // environment.
    validate_cli_args();

    let bind = match resolve_bind() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let port = resolve_port().unwrap_or(DEFAULT_PORT);

    let Some(config_dir) = openspec_app::config_dir() else {
        eprintln!("could not resolve the SpecForge configuration directory");
        std::process::exit(1);
    };

    let svc = openspec_app::AppService::bootstrap(config_dir);

    // Fail loud, before doing any other startup work, if this bind would
    // silently defeat a configured Tailscale login gate (`design.md` Decision
    // 5): that gate trusts the `Tailscale-User-Login` header only because the
    // server binds loopback, and a network bind makes the header forgeable by
    // any peer that can reach the port.
    let allowed_logins = svc.settings.web_config().tailscale.allowed_logins;
    if specforge_web::login_gate_would_be_voided(bind, &allowed_logins) {
        eprintln!(
            "refusing to start: --bind {bind} is a non-loopback interface, but a \
             Tailscale login allow-list is configured ({}). That allow-list's \
             Tailscale-User-Login check is trustworthy only because the server \
             binds loopback — on a network bind, any peer that can reach the port \
             could forge the header itself. Drop --bind (or bind loopback), or \
             clear the configured allow-list in Settings, then retry.",
            allowed_logins.join(", "),
        );
        std::process::exit(1);
    }

    svc.populate().await;
    // Mirror the other frontends' startup so the dashboard has history and the
    // opt-in quota gauge works when enabled.
    svc.spawn_backfill();
    svc.spawn_quota_poller();
    svc.spawn_chatgpt_quota_poller();

    let addr = SocketAddr::new(bind, port);
    println!("SpecForge web UI on http://{addr}");
    if !specforge_web::is_loopback_bind(bind) {
        println!(
            "warning: bound to a non-loopback interface — the UI is reachable from \
             the network and is UNAUTHENTICATED. Anyone who can reach this port can \
             read every registered workspace."
        );
    }
    if let Err(e) = specforge_web::serve(svc, addr).await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

/// Handle `--help`/`--version` (exit 0) and reject any unrecognized
/// `-`-prefixed flag (exit 2), matching `specforge-tui`'s existing unknown-flag
/// behaviour. `--bind`/`--port` (and their `--flag=value` spelling) are the
/// only recognized value-taking flags; their value is consumed here too, so it
/// is never mistaken for a separate flag. Actual value validation happens in
/// [`resolve_bind`] / [`resolve_port`].
fn validate_cli_args() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                print!("{}", help_text());
                std::process::exit(0);
            }
            "--version" => {
                println!("specforge-serve {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--bind" | "--port" => {
                // The value is consumed here so it's never mistaken for a
                // separate flag; whether it actually parses is checked later.
                args.next();
            }
            _ if arg.starts_with("--bind=") || arg.starts_with("--port=") => {}
            _ if arg.starts_with('-') => {
                eprintln!("unknown flag: {arg}\n{USAGE}");
                std::process::exit(2);
            }
            _ => {}
        }
    }
}

fn help_text() -> String {
    format!(
        "specforge-serve {version} — standalone SpecForge web server\n\
\n\
Serves the web UI from this machine's registered OpenSpec workspaces (the\n\
same AppService the desktop app and specforge-tui read).\n\
\n\
{USAGE}\n\
\n\
  --bind <addr>   interface to bind [env: SPECFORGE_WEB_BIND] (default: 127.0.0.1)\n\
  --port <port>   port to listen on [env: SPECFORGE_WEB_PORT] (default: 4317)\n\
  --help          print this message and exit\n\
  --version       print the version and exit\n\
\n\
Binding a non-loopback address (e.g. 0.0.0.0) publishes the web UI on the\n\
network, UNAUTHENTICATED: anyone who can reach the port can read every\n\
registered workspace. Loopback (the default) is unaffected.\n",
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// Resolve the bind address from `--bind <addr>` / `--bind=<addr>`, falling
/// back to the `SPECFORGE_WEB_BIND` environment variable, then to the loopback
/// default. Mirrors [`resolve_port`]'s scan style. A value that is present but
/// fails to parse is an error, never a silent fallback (`design.md` Decision
/// 6).
fn resolve_bind() -> Result<IpAddr, String> {
    parse_bind(std::env::args().skip(1), || {
        std::env::var("SPECFORGE_WEB_BIND").ok()
    })
}

/// The pure scan behind [`resolve_bind`]: the argv tail and the env lookup are
/// parameters so precedence (flag beats env beats default) and the
/// malformed-value error paths are unit-testable without mutating real
/// process state (parallel `#[test]`s touching `std::env` would race).
fn parse_bind(
    mut args: impl Iterator<Item = String>,
    env_bind: impl FnOnce() -> Option<String>,
) -> Result<IpAddr, String> {
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--bind=") {
            return value
                .parse()
                .map_err(|_| format!("invalid --bind value: {value:?}"));
        }
        if arg == "--bind" {
            return match args.next() {
                Some(value) => value
                    .parse()
                    .map_err(|_| format!("invalid --bind value: {value:?}")),
                None => Err("--bind requires a value".to_string()),
            };
        }
    }
    match env_bind() {
        Some(value) => value
            .parse()
            .map_err(|_| format!("invalid SPECFORGE_WEB_BIND value: {value:?}")),
        None => Ok(DEFAULT_BIND),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args<'a>(values: &'a [&'static str]) -> impl Iterator<Item = String> + 'a {
        values.iter().map(|s| s.to_string())
    }

    #[test]
    fn bind_flag_space_separated() {
        assert_eq!(
            parse_bind(args(&["--bind", "0.0.0.0"]), || None),
            Ok(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))
        );
    }

    #[test]
    fn bind_flag_equals_separated() {
        assert_eq!(
            parse_bind(args(&["--bind=0.0.0.0"]), || None),
            Ok(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))
        );
    }

    #[test]
    fn bind_flag_accepts_ipv4() {
        assert_eq!(
            parse_bind(args(&["--bind", "192.168.1.5"]), || None),
            Ok("192.168.1.5".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn bind_flag_accepts_ipv6() {
        assert_eq!(
            parse_bind(args(&["--bind", "::1"]), || None),
            Ok("::1".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn bind_flag_accepts_unspecified_v4() {
        assert_eq!(
            parse_bind(args(&["--bind", "0.0.0.0"]), || None),
            Ok("0.0.0.0".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn bind_falls_back_to_env_when_flag_absent() {
        assert_eq!(
            parse_bind(args(&[]), || Some("192.168.1.5".to_string())),
            Ok("192.168.1.5".parse().unwrap())
        );
    }

    #[test]
    fn bind_flag_takes_precedence_over_env() {
        assert_eq!(
            parse_bind(args(&["--bind", "10.0.0.1"]), || Some(
                "192.168.1.5".to_string()
            )),
            Ok("10.0.0.1".parse().unwrap())
        );
    }

    #[test]
    fn bind_falls_back_to_loopback_default_when_neither_set() {
        assert_eq!(parse_bind(args(&[]), || None), Ok(DEFAULT_BIND));
    }

    #[test]
    fn malformed_bind_flag_is_an_error_not_a_default() {
        let result = parse_bind(args(&["--bind", "not-an-address"]), || None);
        let err = result.expect_err("malformed --bind must be an error");
        assert!(err.contains("not-an-address"), "{err}");
    }

    #[test]
    fn malformed_env_bind_is_an_error_not_a_default() {
        let result = parse_bind(args(&[]), || Some("not-an-address".to_string()));
        let err = result.expect_err("malformed SPECFORGE_WEB_BIND must be an error");
        assert!(err.contains("not-an-address"), "{err}");
    }

    #[test]
    fn bind_flag_missing_its_value_is_an_error() {
        assert!(parse_bind(args(&["--bind"]), || None).is_err());
    }
}
