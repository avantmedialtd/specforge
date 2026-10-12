//! Resolving this host's own Tailscale (MagicDNS) name.
//!
//! Used to widen the web server's trust-boundary allowlist so `tailscale serve`
//! can proxy to the loopback-bound server. Resolution precedence: an explicit
//! manual override, then discovery from local Tailscale state, then `None`
//! (fail closed — no tailnet authority is trusted). Discovery shells
//! `tailscale status --json` and reads `.Self.DNSName`, which is an FQDN *with a
//! trailing dot* (`machine.tailnet.ts.net.`) that we strip.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The wall-clock bound on `tailscale status --json` (`web-ui`: *Bounded
/// Tailscale Name Discovery*). A local daemon answers in tens of
/// milliseconds, so this only ever cuts off a wedged one.
pub const TAILSCALE_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

/// Resolve the tailnet name to trust: the trimmed manual override if present,
/// else the discovered name, else `None`. Discovery blocks for up to
/// [`TAILSCALE_DISCOVERY_TIMEOUT`], so callers run this on a blocking pool,
/// never on a UI thread or an async worker.
pub fn resolve_name(manual: Option<&str>) -> Option<String> {
    resolve_name_with(manual, discover_name)
}

/// [`resolve_name`] over an injected discovery, so a test can prove that a
/// manual override never starts a Tailscale process.
fn resolve_name_with(
    manual: Option<&str>,
    discover: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if let Some(name) = manual {
        let name = name.trim();
        if !name.is_empty() {
            return Some(strip_trailing_dot(name));
        }
    }
    discover()
}

/// Discover the name from `tailscale status --json`. Returns `None` when the
/// command is missing/fails (Tailscale not installed or stopped), MagicDNS is
/// off, or the command outlives [`TAILSCALE_DISCOVERY_TIMEOUT`] — never panics,
/// never surfaces an error to the request path.
fn discover_name() -> Option<String> {
    let mut cmd = Command::new("tailscale");
    cmd.args(["status", "--json"]);
    discover_with(cmd, TAILSCALE_DISCOVERY_TIMEOUT)
}

/// Run `cmd` and parse `.Self.DNSName` from its stdout, giving up after
/// `bound`. Stdout is read to the end on a thread of its own, because a large
/// tailnet's JSON can outgrow the pipe buffer, and a parent that only waited
/// for the child would then deadlock with it. On timeout the child is killed
/// and reaped, so no process outlives the call, and the result is `None`.
fn discover_with(cmd: Command, bound: Duration) -> Option<String> {
    discover_child(cmd, bound).0
}

/// [`discover_with`], also handing back the child so a test can check that it
/// was reaped.
fn discover_child(
    mut cmd: Command,
    bound: Duration,
) -> (Option<String>, Option<std::process::Child>) {
    let Ok(mut child) = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return (None, None);
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return (None, Some(child));
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout.read_to_end(&mut bytes).map(|_| bytes);
        // The receiver is gone once the bound has passed; nothing to do then.
        let _ = tx.send(read);
    });
    let deadline = Instant::now() + bound;
    let name = match rx.recv_timeout(bound) {
        Ok(Ok(bytes)) if exited_successfully_by(&mut child, deadline) => parse_self_dnsname(&bytes),
        _ => None,
    };
    // Timed out, stdout failed, or the CLI failed or lingered: make sure it
    // is stopped, and reap it. Both are no-ops on a child already reaped.
    let _ = child.kill();
    let _ = child.wait();
    (name, Some(child))
}

/// Whether `child`, whose stdout has closed, exits successfully by `deadline`.
/// Polls rather than waits, so a CLI that closes its stdout and then hangs
/// still cannot hold the caller past the bound.
fn exited_successfully_by(child: &mut std::process::Child, deadline: Instant) -> bool {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => return false,
        }
    }
}

/// Pull `.Self.DNSName` (trailing dot stripped) out of a `tailscale status
/// --json` payload. Factored out so it is testable without invoking the binary.
fn parse_self_dnsname(json: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(json).ok()?;
    let dns = value.get("Self")?.get("DNSName")?.as_str()?;
    let name = strip_trailing_dot(dns);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn strip_trailing_dot(value: &str) -> String {
    value.trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_override_wins_and_is_dot_stripped() {
        assert_eq!(
            resolve_name(Some("m.tailnet.ts.net.")),
            Some("m.tailnet.ts.net".to_string())
        );
    }

    #[test]
    fn blank_manual_falls_through_to_discovery() {
        // Discovery may or may not find a name in the test env; the point is a
        // blank override does not short-circuit to `Some("")`.
        assert_ne!(resolve_name(Some("   ")), Some(String::new()));
    }

    #[test]
    fn a_manual_override_never_calls_discovery_and_a_blank_one_does() {
        let mut calls = 0;
        assert_eq!(
            resolve_name_with(Some(" box.ts.net. "), || {
                calls += 1;
                None
            }),
            Some("box.ts.net".to_string())
        );
        assert_eq!(calls, 0, "a manual name starts no Tailscale process");

        for manual in [None, Some("  ")] {
            let mut calls = 0;
            assert_eq!(
                resolve_name_with(manual, || {
                    calls += 1;
                    Some("found.ts.net".to_string())
                }),
                Some("found.ts.net".to_string())
            );
            assert_eq!(calls, 1, "{manual:?} falls through to discovery");
        }
    }

    #[cfg(unix)]
    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script]);
        cmd
    }

    #[cfg(unix)]
    #[test]
    fn a_cli_that_never_exits_is_killed_and_reaped_within_the_bound() {
        let started = std::time::Instant::now();
        let (name, child) = discover_child(sh("sleep 60"), Duration::from_millis(50));
        assert_eq!(name, None);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "gave up within the bound, not after the child's own minute"
        );
        let mut child = child.expect("the child was spawned");
        assert!(
            matches!(child.try_wait(), Ok(Some(_))),
            "the child has exited and been reaped, not left running"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_payload_larger_than_the_pipe_buffer_is_read_whole() {
        // Pad the payload well past a 64 KiB pipe buffer, so a parent that
        // waited before reading would deadlock with the child.
        let padding = "x".repeat(200 * 1024);
        let payload =
            format!(r#"{{"Peer":{{"pad":"{padding}"}},"Self":{{"DNSName":"big.ts.net."}}}}"#);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("status.json");
        std::fs::write(&file, payload).unwrap();
        let name = discover_with(
            sh(&format!("cat '{}'", file.display())),
            Duration::from_secs(30),
        );
        assert_eq!(name, Some("big.ts.net".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_cli_resolves_no_name() {
        let name = discover_with(
            sh(r#"echo '{"Self":{"DNSName":"no.ts.net."}}'; exit 3"#),
            Duration::from_secs(30),
        );
        assert_eq!(name, None, "a non-zero exit is not trusted");
    }

    #[test]
    fn a_missing_cli_resolves_no_name() {
        let name = discover_with(
            Command::new("specforge-no-such-tailscale-binary"),
            Duration::from_secs(1),
        );
        assert_eq!(name, None);
    }

    #[test]
    fn parses_self_dnsname_with_trailing_dot() {
        let blob = br#"{"Self":{"DNSName":"box.tail-scale.ts.net."},"Peer":{}}"#;
        assert_eq!(
            parse_self_dnsname(blob),
            Some("box.tail-scale.ts.net".to_string())
        );
    }

    #[test]
    fn missing_or_empty_dnsname_is_none() {
        assert_eq!(parse_self_dnsname(br#"{"Self":{}}"#), None);
        assert_eq!(parse_self_dnsname(br#"{"Self":{"DNSName":"."}}"#), None);
        assert_eq!(parse_self_dnsname(br#"{}"#), None);
        assert_eq!(parse_self_dnsname(b"not json"), None);
    }
}
