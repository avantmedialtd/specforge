//! The one root set every outbound HTTPS client verifies against
//! (`outbound-https-trust`: *HTTPS Clients Trust Bundled and System Roots*):
//! the Mozilla roots, together with the operating system's trusted roots as
//! `rustls-native-certs` reads them, which honours `SSL_CERT_FILE` and
//! `SSL_CERT_DIR`. A machine on a TLS-inspecting network trusts its corporate
//! root CA there, and so the provider transport (`crate::usage_http`) and the
//! embedded-image transport (`crate::pull_request_content_images`) trust it
//! too.
//!
//! The set is built once per process, on first use, and never rebuilt, so a
//! store is read once rather than on every connection. A system root that
//! does not parse is skipped, and a store that cannot be read at all leaves
//! the Mozilla roots alone, which is how every client behaved before. Nothing
//! here disables or relaxes certificate or hostname verification, and no
//! setting or variable can.
//!
//! ureq takes specific roots only as whole certificates, so the Mozilla roots
//! come from `webpki-root-certs`, which ships as DER certificates the same set
//! that `webpki-roots`, ureq's default, ships as trust anchors.
//!
//! When the set is built, one line on standard error says how many roots of
//! each kind it holds, or that the system roots could not be loaded
//! (`outbound-https-trust`: *The Root Set Is Reported Once*). The terminal
//! frontend owns its screen and turns that line off with [`silence_report`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;
use rustls_native_certs::CertificateResult;
use ureq::tls::{Certificate, RootCerts};

/// Whether the root set's line stays off standard error.
static SILENT: AtomicBool = AtomicBool::new(false);

/// This process's root set.
static ROOTS: TrustRoots = TrustRoots::new();

/// Keeps the root set's line off standard error. The terminal frontend calls
/// this before anything else: the line would land on the screen it draws,
/// and its quota poller still makes HTTPS requests (`outbound-https-trust`:
/// *The Root Set Is Reported Once*).
pub fn silence_report() {
    SILENT.store(true, Ordering::Relaxed);
}

/// The root set every HTTPS client trusts. The first call builds it from the
/// system store and writes its line; every later call returns the same set.
pub(crate) fn root_certs() -> RootCerts {
    ROOTS.get(rustls_native_certs::load_native_certs, report)
}

/// Writes the root set's line to standard error, unless it was silenced.
fn report(line: &str) {
    if !SILENT.load(Ordering::Relaxed) {
        eprintln!("{line}");
    }
}

/// A root set built at most once, on first use.
struct TrustRoots(OnceLock<Arc<Vec<Certificate<'static>>>>);

impl TrustRoots {
    const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// The set. The first call builds it from the system roots `load`
    /// returns and writes its line through `report`; later calls call
    /// neither.
    fn get(
        &self,
        load: impl FnOnce() -> CertificateResult,
        report: impl FnOnce(&str),
    ) -> RootCerts {
        let certs = self.0.get_or_init(|| {
            let set = RootSet::build(load());
            report(&set.line());
            set.certs
        });
        RootCerts::Specific(Arc::clone(certs))
    }
}

/// One build of the root set: the certificates, the bundled ones first, and
/// what its line reports.
struct RootSet {
    certs: Arc<Vec<Certificate<'static>>>,
    bundled: usize,
    system: SystemRoots,
}

/// What the system store contributed.
#[derive(Debug, PartialEq, Eq)]
enum SystemRoots {
    /// This many roots, each of which parses.
    Loaded(usize),
    /// Nothing: the store could not be read at all, for this reason.
    Unreadable(String),
}

impl RootSet {
    /// The Mozilla roots, then every system root in `native` that parses. A
    /// store that yielded no root that parses and reported an error could not
    /// be read at all; one that yielded nothing and reported nothing is empty.
    fn build(native: CertificateResult) -> Self {
        let mut certs: Vec<Certificate<'static>> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(|cert| Certificate::from_der(cert.as_ref()))
            .collect();
        let bundled = certs.len();
        certs.extend(
            native
                .certs
                .iter()
                .filter(|cert| parses(cert))
                .map(|cert| Certificate::from_der(cert).to_owned()),
        );
        let system = match certs.len() - bundled {
            0 => match native.errors.first() {
                Some(error) => SystemRoots::Unreadable(error.to_string()),
                None => SystemRoots::Loaded(0),
            },
            loaded => SystemRoots::Loaded(loaded),
        };
        Self {
            certs: Arc::new(certs),
            bundled,
            system,
        }
    }

    /// The line the set is reported with.
    fn line(&self) -> String {
        match &self.system {
            SystemRoots::Loaded(system) => format!(
                "specforge: tls: trusting {} bundled + {system} system root certificates",
                self.bundled
            ),
            SystemRoots::Unreadable(reason) => format!(
                "specforge: tls: trusting {} bundled root certificates; the system roots could \
                 not be loaded ({reason})",
                self.bundled
            ),
        }
    }
}

/// Whether a system root parses as rustls reads a root.
fn parses(cert: &CertificateDer<'static>) -> bool {
    RootCertStore::empty().add(cert.clone()).is_ok()
}

/// The throwaway roots and server certificates the trust tests use — see
/// `tests/fixtures/tls/README.md`.
#[cfg(test)]
pub(crate) mod fixtures {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use rustls_native_certs::CertificateResult;
    use ureq::tls::RootCerts;

    use super::RootSet;

    /// A root CA in neither the Mozilla set nor any system store, standing
    /// in for a corporate TLS-inspection root.
    pub(crate) const CA: &[u8] = include_bytes!("../tests/fixtures/tls/ca.pem");
    /// A server certificate for `localhost` and `127.0.0.1`, signed by [`CA`].
    const LOCALHOST: &[u8] = include_bytes!("../tests/fixtures/tls/localhost.pem");
    const LOCALHOST_KEY: &[u8] = include_bytes!("../tests/fixtures/tls/localhost.key");
    /// A self-signed server certificate for the same names, which no root
    /// set holds.
    const SELF_SIGNED: &[u8] = include_bytes!("../tests/fixtures/tls/self-signed.pem");
    const SELF_SIGNED_KEY: &[u8] = include_bytes!("../tests/fixtures/tls/self-signed.key");

    /// The certificate a PEM fixture holds.
    pub(crate) fn certificate(pem: &[u8]) -> CertificateDer<'static> {
        CertificateDer::from_pem_slice(pem).expect("a PEM certificate")
    }

    /// What a system store that holds `certs` gives up.
    pub(crate) fn store(certs: Vec<CertificateDer<'static>>) -> CertificateResult {
        let mut result = CertificateResult::default();
        result.certs.extend(certs);
        result
    }

    /// A root set built as the process's is, from a system store holding
    /// `certs`, without touching the process's own.
    pub(crate) fn roots_with(certs: Vec<CertificateDer<'static>>) -> RootCerts {
        RootCerts::Specific(RootSet::build(store(certs)).certs)
    }

    /// A server identity: its certificate chain and its key.
    pub(crate) struct Identity {
        pub(crate) chain: Vec<CertificateDer<'static>>,
        pub(crate) key: PrivateKeyDer<'static>,
    }

    fn identity(cert: &[u8], key: &[u8]) -> Identity {
        Identity {
            chain: vec![certificate(cert)],
            key: PrivateKeyDer::from_pem_slice(key).expect("a PEM private key"),
        }
    }

    /// `localhost`, signed by [`CA`].
    pub(crate) fn localhost() -> Identity {
        identity(LOCALHOST, LOCALHOST_KEY)
    }

    /// `localhost`, signed by itself.
    pub(crate) fn self_signed() -> Identity {
        identity(SELF_SIGNED, SELF_SIGNED_KEY)
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{certificate, store, CA};
    use super::*;
    use std::cell::Cell;

    /// A store that could not be read at all, for the reason a missing
    /// `SSL_CERT_DIR` gives.
    fn unreadable() -> CertificateResult {
        let mut result = CertificateResult::default();
        result.errors.push(rustls_native_certs::Error {
            context: "opening directory",
            kind: rustls_native_certs::ErrorKind::Io {
                inner: std::io::Error::from(std::io::ErrorKind::NotFound),
                path: "/nowhere/certs".into(),
            },
        });
        result
    }

    fn bundled() -> usize {
        webpki_root_certs::TLS_SERVER_ROOT_CERTS.len()
    }

    /// The bundled half is the Mozilla set `webpki-root-certs` ships, every
    /// certificate of it, in its order, and each one parses as rustls reads a
    /// root, so ureq ignores none of them (`outbound-https-trust`: *Public
    /// servers still work without system roots*).
    #[test]
    fn the_bundled_roots_are_the_crates_mozilla_certificates() {
        let set = RootSet::build(store(Vec::new()));
        assert!(bundled() > 0, "a Mozilla set to trust");
        assert_eq!(set.bundled, bundled());
        let ders: Vec<&[u8]> = set.certs.iter().map(Certificate::der).collect();
        let shipped: Vec<&[u8]> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .map(AsRef::as_ref)
            .collect();
        assert_eq!(ders, shipped);
        let mut roots = RootCertStore::empty();
        let parsed =
            roots.add_parsable_certificates(ders.iter().map(|der| CertificateDer::from(*der)));
        assert_eq!(parsed, (bundled(), 0), "every bundled root parses");
    }

    /// `outbound-https-trust`: *A corporate root in the system store is
    /// trusted* — the union holds every bundled root and the system's.
    #[test]
    fn the_set_is_the_bundled_roots_then_the_systems() {
        let set = RootSet::build(store(vec![certificate(CA)]));
        assert_eq!(set.certs.len(), bundled() + 1);
        assert_eq!(set.bundled, bundled());
        assert_eq!(set.system, SystemRoots::Loaded(1));
        assert_eq!(set.certs.last().unwrap().der(), certificate(CA).as_ref());
        assert_eq!(
            set.line(),
            format!(
                "specforge: tls: trusting {} bundled + 1 system root certificates",
                bundled()
            )
        );
    }

    /// A system root that does not parse is skipped, and not counted.
    #[test]
    fn a_system_root_that_does_not_parse_is_skipped() {
        let set = RootSet::build(store(vec![
            CertificateDer::from(b"not a certificate".to_vec()),
            certificate(CA),
        ]));
        assert_eq!(set.certs.len(), bundled() + 1);
        assert_eq!(set.system, SystemRoots::Loaded(1));
        assert_eq!(set.certs.last().unwrap().der(), certificate(CA).as_ref());
    }

    /// `outbound-https-trust`: *A missing system store is visible* — the
    /// bundled roots are used alone, and the line says why.
    #[test]
    fn an_unreadable_store_leaves_the_bundled_roots_alone() {
        let set = RootSet::build(unreadable());
        assert_eq!(set.certs.len(), bundled());
        let line = set.line();
        assert_eq!(
            line,
            format!(
                "specforge: tls: trusting {} bundled root certificates; the system roots could \
                 not be loaded (opening directory: entity not found at '/nowhere/certs')",
                bundled()
            )
        );
    }

    /// An empty store is no failure: it reports zero system roots, which is
    /// the clue for a machine whose root lives elsewhere.
    #[test]
    fn an_empty_store_reports_zero_system_roots() {
        let set = RootSet::build(store(Vec::new()));
        assert_eq!(set.system, SystemRoots::Loaded(0));
        assert!(set
            .line()
            .ends_with(" bundled + 0 system root certificates"));
    }

    /// A store whose only roots do not parse, and which reported an error,
    /// could not be read at all.
    #[test]
    fn a_store_with_errors_and_no_usable_root_is_unreadable() {
        let mut native = unreadable();
        native
            .certs
            .push(CertificateDer::from(b"not a certificate".to_vec()));
        assert!(matches!(
            RootSet::build(native).system,
            SystemRoots::Unreadable(_)
        ));
        // A partial read keeps what it read.
        let mut partial = unreadable();
        partial.certs.push(certificate(CA));
        assert_eq!(RootSet::build(partial).system, SystemRoots::Loaded(1));
    }

    /// `outbound-https-trust`: *The startup line counts system roots* — built
    /// once, reported once, and the same set on every later call.
    #[test]
    fn the_set_is_built_and_reported_once() {
        let roots = TrustRoots::new();
        let loads = Cell::new(0);
        let mut lines = Vec::new();
        let first = roots.get(
            || {
                loads.set(loads.get() + 1);
                store(vec![certificate(CA)])
            },
            |line| lines.push(line.to_string()),
        );
        let second = roots.get(
            || panic!("the store is read once"),
            |_| panic!("the set is reported once"),
        );
        assert_eq!(loads.get(), 1);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with(" bundled + 1 system root certificates"));
        let (RootCerts::Specific(first), RootCerts::Specific(second)) = (first, second) else {
            panic!("specific roots");
        };
        assert!(Arc::ptr_eq(&first, &second), "the same set");
        assert_eq!(first.len(), bundled() + 1);
    }

    /// The process's own set is specific roots, the same set on every call.
    #[test]
    fn the_process_set_is_one_specific_set() {
        let (RootCerts::Specific(first), RootCerts::Specific(second)) =
            (root_certs(), root_certs())
        else {
            panic!("specific roots");
        };
        assert!(Arc::ptr_eq(&first, &second));
        assert!(first.len() >= bundled());
    }

    /// The terminal frontend's switch keeps the line off standard error for
    /// the rest of the process (`outbound-https-trust`: *The Root Set Is
    /// Reported Once*: the terminal frontend does not write it). Turning it on
    /// here only quiets this test process.
    #[test]
    fn silencing_the_report_lasts() {
        silence_report();
        assert!(SILENT.load(Ordering::Relaxed));
        silence_report();
        assert!(SILENT.load(Ordering::Relaxed), "and stays on");
    }
}
