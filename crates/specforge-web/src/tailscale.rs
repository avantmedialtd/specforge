//! Resolving this host's own Tailscale (MagicDNS) name.
//!
//! Used to widen the web server's trust-boundary allowlist so `tailscale serve`
//! can proxy to the loopback-bound server. Resolution precedence: an explicit
//! manual override, then discovery from local Tailscale state, then `None`
//! (fail closed — no tailnet authority is trusted). Discovery shells
//! `tailscale status --json` and reads `.Self.DNSName`, which is an FQDN *with a
//! trailing dot* (`machine.tailnet.ts.net.`) that we strip.

/// Resolve the tailnet name to trust: the trimmed manual override if present,
/// else the discovered name, else `None`.
pub fn resolve_name(manual: Option<&str>) -> Option<String> {
    if let Some(name) = manual {
        let name = name.trim();
        if !name.is_empty() {
            return Some(strip_trailing_dot(name));
        }
    }
    discover_name()
}

/// Discover the name from `tailscale status --json`. Returns `None` when the
/// command is missing/fails (Tailscale not installed or stopped) or MagicDNS is
/// off — never panics, never surfaces an error to the request path.
fn discover_name() -> Option<String> {
    let output = std::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_self_dnsname(&output.stdout)
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
