//! Pure embedded-image authority tests: no network or machine configuration.

use openspec_core::pr_content_image::{
    canonical_source, image_sources, public_address, public_resolution, redirect_target,
    MAX_REDIRECT_HOPS,
};
use serde::Deserialize;

fn sources(markdown: &str) -> Vec<String> {
    image_sources(markdown)
        .iter()
        .map(|source| source.as_str().to_owned())
        .collect()
}

#[test]
fn actual_images_resolve_references_escapes_and_link_nesting() {
    assert_eq!(
        sources(
            "![inline](https://images.example/a.png)\n\
             [![linked][screen]](https://example.com/page)\n\
             ![escaped](https://images.example/a\\(1\\).png?x=1&amp;y=2)\n\
             \n[screen]: https://images.example/b.png \"Screenshot\"\n"
        ),
        [
            "https://images.example/a.png",
            "https://images.example/b.png",
            "https://images.example/a(1).png?x=1&y=2"
        ]
    );
}

#[test]
fn code_html_comments_and_ordinary_links_grant_nothing() {
    let url = "https://images.example/a.png";
    for markdown in [
        format!("`![code]({url})`"),
        format!("```markdown\n![code]({url})\n```"),
        format!("    ![indented]({url})"),
        format!("<!-- ![comment]({url}) -->"),
        format!("<img src=\"{url}\">"),
        format!("<div>\n![html block]({url})\n</div>"),
        format!("[ordinary link]({url})"),
        "![undefined][ref]\n\n[ref]: <javascript:alert(1)>".to_string(),
    ] {
        assert!(sources(&markdown).is_empty(), "{markdown}");
    }
}

#[test]
fn canonicalisation_preserves_signed_path_and_query() {
    let source =
        canonical_source("HTTPS://Images.Example.:443/assets/a%2fb.png?X=a%2Fb&key=1+2#preview")
            .unwrap();
    assert_eq!(
        source.as_str(),
        "https://images.example/assets/a%2fb.png?X=a%2Fb&key=1+2"
    );
    assert_eq!(source.host(), "images.example");
    assert_eq!(source.origin(), "https://images.example");
    assert!(!format!("{source:?}").contains("key"));
    assert!(format!("{source:?}").starts_with("ImageSource"));
    assert_eq!(source, source.clone());
}

#[test]
fn forbidden_source_forms_are_rejected_before_dns() {
    for source in [
        "",
        "images.example/a.png",
        "/a.png",
        "//images.example/a.png",
        "http://images.example/a.png",
        "https:images.example/a.png",
        "data:image/png;base64,AAAA",
        "blob:https://images.example/a",
        "file:///a.png",
        "javascript:alert(1)",
        "https://u:p@images.example/a",
        "https://@images.example/a",
        "https://images.example:80/a",
        "https://images.example:444/a",
        "https://127.0.0.1/a",
        "https://2130706433/a",
        "https://0x7f000001/a",
        "https://[::1]/a",
        "https://[::ffff:127.0.0.1]/a",
        "https://localhost/a",
        "https://localhost./a",
        "https://a.localhost/a",
        "https://a.local/a",
        "https://a.internal/a",
        "https://a.home.arpa/a",
        "https://a.onion/a",
        "https://server/a",
        "https://images..example/a",
        "https://images.example../a",
        "https://-images.example/a",
        "https://images-.example/a",
        "https://images_example.com/a",
        " https://images.example/a",
        "https://images.example/a ",
        "https://images.\nexample/a",
        "https://images.example\\@localhost/a",
    ] {
        assert!(canonical_source(source).is_none(), "{source}");
    }
}

#[test]
fn sources_are_deduplicated_in_first_occurrence_order() {
    assert_eq!(
        sources(
            "![a](https://images.example/a#one)\n\
             ![b](https://IMAGES.EXAMPLE.:443/a#two)\n\
             ![c](https://images.example/b)"
        ),
        ["https://images.example/a", "https://images.example/b"]
    );
}

#[test]
fn gfm_containers_keep_their_images() {
    assert_eq!(
        sources(
            "| Status |\n| --- |\n| ![table](https://images.example/a.png) |\n\
             \n- [ ] ~~![task](https://images.example/b.png)~~\n"
        ),
        [
            "https://images.example/a.png",
            "https://images.example/b.png"
        ]
    );
}

#[derive(Deserialize)]
struct SourceCase {
    name: String,
    markdown: String,
    sources: Vec<String>,
}

#[test]
fn shared_markdown_source_corpus() {
    let cases: Vec<SourceCase> =
        serde_json::from_str(include_str!("fixtures/pr-content-images.json")).unwrap();
    assert!(cases.len() >= 12);
    for case in cases {
        assert_eq!(sources(&case.markdown), case.sources, "{}", case.name);
    }
}

#[test]
fn ipv4_forbidden_range_edges_and_public_neighbours() {
    for (first, last) in [
        ("0.0.0.0", "0.255.255.255"),
        ("10.0.0.0", "10.255.255.255"),
        ("100.64.0.0", "100.127.255.255"),
        ("127.0.0.0", "127.255.255.255"),
        ("169.254.0.0", "169.254.255.255"),
        ("172.16.0.0", "172.31.255.255"),
        ("192.0.0.0", "192.0.0.255"),
        ("192.0.2.0", "192.0.2.255"),
        ("192.31.196.0", "192.31.196.255"),
        ("192.52.193.0", "192.52.193.255"),
        ("192.88.99.0", "192.88.99.255"),
        ("192.168.0.0", "192.168.255.255"),
        ("192.175.48.0", "192.175.48.255"),
        ("198.18.0.0", "198.19.255.255"),
        ("198.51.100.0", "198.51.100.255"),
        ("203.0.113.0", "203.0.113.255"),
        ("224.0.0.0", "239.255.255.255"),
        ("240.0.0.0", "255.255.255.255"),
    ] {
        for value in [first, last] {
            assert!(!public_address(value.parse().unwrap()), "{value}");
        }
    }
    for value in [
        "1.0.0.0",
        "9.255.255.255",
        "11.0.0.0",
        "100.63.255.255",
        "100.128.0.0",
        "126.255.255.255",
        "128.0.0.0",
        "169.253.255.255",
        "169.255.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "191.255.255.255",
        "192.0.1.0",
        "192.0.3.0",
        "192.31.195.255",
        "192.31.197.0",
        "192.52.192.255",
        "192.52.194.0",
        "192.88.98.255",
        "192.88.100.0",
        "192.167.255.255",
        "192.169.0.0",
        "192.175.47.255",
        "192.175.49.0",
        "198.17.255.255",
        "198.20.0.0",
        "198.51.99.255",
        "198.51.101.0",
        "203.0.112.255",
        "203.0.114.0",
        "223.255.255.255",
        "8.8.8.8",
        "1.1.1.1",
    ] {
        assert!(public_address(value.parse().unwrap()), "{value}");
    }
}

#[test]
fn ipv6_only_allows_ordinary_global_unicast() {
    for value in [
        "::",
        "::1",
        "::ffff:8.8.8.8",
        "::ffff:127.0.0.1",
        "::ffff:0:10.0.0.1",
        "64:ff9b::808:808",
        "64:ff9b:1::a00:1",
        "100::1",
        "100:0:0:1::1",
        "2001::",
        "2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff",
        "2001:2::1",
        "2001:10::1",
        "2001:20::1",
        "2001:db8::",
        "2001:db8:ffff:ffff:ffff:ffff:ffff:ffff",
        "2002::",
        "2002:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        "2620:4f:8000::",
        "2620:4f:8000:ffff:ffff:ffff:ffff:ffff",
        "3fff::",
        "3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff",
        "4000::1",
        "fc00::",
        "fdff::1",
        "fe80::1",
        "ff02::1",
    ] {
        assert!(!public_address(value.parse().unwrap()), "{value}");
    }
    for value in [
        "2000::",
        "2001:200::",
        "2001:db7:ffff:ffff:ffff:ffff:ffff:ffff",
        "2001:db9::",
        "2001:4860:4860::8888",
        "2003::",
        "2606:4700:4700::1111",
        "2620:4f:7fff:ffff:ffff:ffff:ffff:ffff",
        "2620:4f:8001::",
        "3ffe:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        "3fff:1000::",
        "3fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
    ] {
        assert!(public_address(value.parse().unwrap()), "{value}");
    }
}

#[test]
fn complete_dns_answer_must_be_public_and_https() {
    assert!(!public_resolution(&[]));
    assert!(public_resolution(&[
        "1.1.1.1:443".parse().unwrap(),
        "[2606:4700:4700::1111]:443".parse().unwrap(),
    ]));
    for bad in [
        "10.0.0.1:443",
        "169.254.169.254:443",
        "[::ffff:1.1.1.1]:443",
        "1.1.1.1:80",
    ] {
        let bad = bad.parse().unwrap();
        let good = "1.1.1.1:443".parse().unwrap();
        assert!(!public_resolution(&[good, bad]));
        assert!(!public_resolution(&[bad, good]));
    }
}

#[test]
fn relative_redirects_preserve_queries_and_reject_loops() {
    let source = canonical_source("https://images.example/dir/a.png?q=one").unwrap();
    let history = [source.clone()];
    let target = redirect_target(&source, "../b.png?sig=a%2Fb+z#preview", &history).unwrap();
    assert_eq!(target.as_str(), "https://images.example/b.png?sig=a%2Fb+z");
    assert!(redirect_target(&source, "#different-fragment", &history).is_none());
    assert!(redirect_target(
        &source,
        "https://IMAGES.EXAMPLE.:443/dir/a.png?q=one",
        &history
    )
    .is_none());
    assert!(redirect_target(&source, "/b.png", &[]).is_none());
    assert!(redirect_target(&target, "/c.png", &history).is_none());
}

#[test]
fn redirect_targets_reapply_syntax_and_hop_limits() {
    let source = canonical_source("https://images.example/a").unwrap();
    for location in [
        "",
        "http://cdn.example/a",
        "https://u:p@cdn.example/a",
        "//127.0.0.1/a",
        "https://[::1]/a",
        "https://cdn.local/a",
        "https://cdn.example:444/a",
        "javascript:alert(1)",
        "data:image/png,AAAA",
        "\\\\localhost\\a",
        "\nhttps://cdn.example/a",
        " https://cdn.example/a",
        "/a\nb.png",
        "/a\\b.png",
    ] {
        assert!(
            redirect_target(&source, location, std::slice::from_ref(&source)).is_none(),
            "{location}"
        );
    }
    let mut history = vec![source];
    assert_eq!(MAX_REDIRECT_HOPS, 3);
    for hop in 1..=MAX_REDIRECT_HOPS {
        let current = history.last().unwrap();
        let target =
            redirect_target(current, &format!("https://cdn.example/{hop}"), &history).unwrap();
        history.push(target);
    }
    assert!(redirect_target(
        history.last().unwrap(),
        "https://cdn.example/extra",
        &history
    )
    .is_none());
}
