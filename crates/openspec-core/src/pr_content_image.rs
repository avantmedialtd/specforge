//! Pure authority and destination rules for embedded PR images
//! (`pr-content-loading`: *Embedded Image Reads Are Authorised by Published
//! Content*). This module neither resolves DNS nor fetches anything.

use std::collections::HashSet;
use std::fmt;
use std::net::{IpAddr, SocketAddr};

use pulldown_cmark::{Event, Options, Parser, Tag};
use url::{Host, Url};

/// A canonical, syntactically admissible HTTPS image source. This is not
/// network authority: its DNS answer must also pass the public-address gate
/// at the actual connection. Debug deliberately reveals no signed URL.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ImageSource(Url);

impl fmt::Debug for ImageSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageSource").finish_non_exhaustive()
    }
}

impl ImageSource {
    /// The canonical fetch identity, retaining path/query and omitting a fragment.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The DNS/Host/TLS name, never a caller-supplied IP literal.
    pub fn host(&self) -> &str {
        // Construction accepts only a non-empty Domain.
        self.0.host_str().unwrap()
    }

    /// The origin used by the credentialless reader's own cooldowns.
    pub fn origin(&self) -> String {
        self.0.origin().ascii_serialization()
    }
}

/// Parse a source without performing DNS. Ordinary DNS labels only, with a
/// public-style multi-label name: local-only names and IP forms fail before
/// any resolver can see them. Actual routability is a separate check.
pub fn canonical_source(source: &str) -> Option<ImageSource> {
    if !source
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        || source.trim() != source
        || source.bytes().any(|b| b.is_ascii_control() || b == b'\\')
    {
        return None;
    }
    // Url normalises empty user information away; reject its syntax too.
    let authority = source[8..].split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    canonical_url(Url::parse(source).ok()?)
}

/// Check parsed URL semantics independently of raw-source spelling. Redirect
/// resolution produces a parsed URL too; neither path may bypass these rules.
fn canonical_url(mut url: Url) -> Option<ImageSource> {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let Host::Domain(domain) = url.host()? else {
        return None;
    };
    let host = domain.strip_suffix('.').unwrap_or(domain);
    if !dns_hostname(host) {
        return None;
    }
    let host = host.to_owned();
    url.set_host(Some(&host)).ok()?;
    url.set_fragment(None);
    Some(ImageSource(url))
}

fn dns_hostname(host: &str) -> bool {
    const LOCAL_SUFFIXES: &[&str] = &[
        "localhost",
        "local",
        "internal",
        "lan",
        "home",
        "home.arpa",
        "onion",
        "alt",
    ];
    host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && !LOCAL_SUFFIXES
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
}

/// The admissible sources of actual Markdown images, in first-occurrence
/// order. CommonMark parsing excludes code, HTML blocks/comments and ordinary
/// links; reference definitions and escapes are resolved by the parser.
pub fn image_sources(markdown: &str) -> Vec<ImageSource> {
    let mut seen = HashSet::new();
    Parser::new_ext(markdown, image_markdown_options())
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => canonical_source(&dest_url),
            _ => None,
        })
        .filter(|source| seen.insert(source.clone()))
        .collect()
}

/// Keep source authority on the published renderer's CommonMark/GFM dialect,
/// including extensions whose wrapper events do not themselves contain URLs.
fn image_markdown_options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// A source plus at most three separately validated redirects. The history
/// includes the original source and must end at the current hop.
pub const MAX_REDIRECT_HOPS: usize = 3;

/// Resolve one Location without fetching it. Every target must independently
/// meet the source syntax rules; the transport still validates/pins its DNS.
pub fn redirect_target(
    current: &ImageSource,
    location: &str,
    visited: &[ImageSource],
) -> Option<ImageSource> {
    if visited.last() != Some(current)
        || visited.len() > MAX_REDIRECT_HOPS
        || location.is_empty()
        || location.trim() != location
        || location.bytes().any(|b| b.is_ascii_control() || b == b'\\')
    {
        return None;
    }
    let joined = current.0.join(location).ok()?;
    let target = canonical_url(joined)?;
    (!visited.contains(&target)).then_some(target)
}

/// Whether an address is ordinary globally routable unicast. Special-purpose
/// networks are deliberately denied even where one of their specific services
/// is globally reachable: the image reader needs no transition/tunnel/service
/// address. This is a conservative allow of global IPv6, not `!is_private()`.
pub fn public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            const FORBIDDEN: &[(u32, u32)] = &[
                (0x0000_0000, 8),  // This network.
                (0x0a00_0000, 8),  // Private.
                (0x6440_0000, 10), // Shared address space.
                (0x7f00_0000, 8),  // Loopback.
                (0xa9fe_0000, 16), // Link local and metadata.
                (0xac10_0000, 12), // Private.
                (0xc000_0000, 24), // IETF protocol assignments.
                (0xc000_0200, 24), // Documentation.
                (0xc01f_c400, 24), // AS112 service.
                (0xc034_c100, 24), // AMT service.
                (0xc058_6300, 24), // Deprecated 6to4 relay.
                (0xc0a8_0000, 16), // Private.
                (0xc0af_3000, 24), // AS112 direct delegation.
                (0xc612_0000, 15), // Benchmarking.
                (0xc633_6400, 24), // Documentation.
                (0xcb00_7100, 24), // Documentation.
                (0xe000_0000, 4),  // Multicast.
                (0xf000_0000, 4),  // Reserved and broadcast.
            ];
            let bits = u32::from(address);
            !FORBIDDEN
                .iter()
                .any(|&(base, prefix)| bits & (u32::MAX << (32 - prefix)) == base)
        }
        IpAddr::V6(address) => {
            let bits = u128::from(address);
            // Global unicast only. This also excludes mapped IPv4, NAT64,
            // translation/discard space, ULA, link-local and multicast.
            if bits >> 125 != 1 {
                return false;
            }
            const FORBIDDEN: &[(u128, u32)] = &[
                (0x2001_0000_0000_0000_0000_0000_0000_0000, 23), // IETF special purpose.
                (0x2001_0db8_0000_0000_0000_0000_0000_0000, 32), // Documentation.
                (0x2002_0000_0000_0000_0000_0000_0000_0000, 16), // 6to4 embeds IPv4.
                (0x2620_004f_8000_0000_0000_0000_0000_0000, 48), // AS112 service.
                (0x3fff_0000_0000_0000_0000_0000_0000_0000, 20), // Documentation.
            ];
            !FORBIDDEN
                .iter()
                .any(|&(base, prefix)| bits & (u128::MAX << (128 - prefix)) == base)
        }
    }
}

/// Validate the entire DNS answer before handing any of it to a connector.
/// Empty/mixed answers fail closed; a resolver cannot substitute another port.
pub fn public_resolution(addresses: &[SocketAddr]) -> bool {
    !addresses.is_empty()
        && addresses
            .iter()
            .all(|address| address.port() == 443 && public_address(address.ip()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_parser_keeps_each_published_renderers_gfm_extension() {
        let options = image_markdown_options();
        assert!(options.contains(Options::ENABLE_TABLES));
        assert!(options.contains(Options::ENABLE_STRIKETHROUGH));
        assert!(options.contains(Options::ENABLE_TASKLISTS));
    }

    #[test]
    fn parsed_url_semantics_independently_reject_schemes_credentials_ports_and_hosts() {
        for source in [
            "http://images.example/a",
            "ftp://images.example/a",
            "https://user@images.example/a",
            "https://:password@images.example/a",
            "https://images.example:444/a",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://a.localhost/a",
        ] {
            assert!(
                canonical_url(Url::parse(source).unwrap()).is_none(),
                "{source}"
            );
        }
        let source =
            canonical_url(Url::parse("https://images.example/a?sig=a%2Fb#part").unwrap()).unwrap();
        assert_eq!(source.as_str(), "https://images.example/a?sig=a%2Fb");
    }
}
