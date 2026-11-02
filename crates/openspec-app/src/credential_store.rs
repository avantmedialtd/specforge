//! Provider credential storage with an injectable production boundary.
//!
//! The settings file persists only the non-secret [`CredentialEntryId`]. The
//! corresponding secret lives behind [`CredentialStore`], implemented by the
//! operating system in production and by [`InMemoryCredentialStore`] in tests.
//! No unit or integration test needs to access a developer's real keychain.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::events::PullRequestProvider;

/// Stable service namespaces. Entry ids carry their own schema version so a
/// later representation can coexist without silently reinterpreting a secret.
pub const GITHUB_NAMESPACE: &str = "com.avantmedia.specforge.github";
pub const BITBUCKET_NAMESPACE: &str = "com.avantmedia.specforge.bitbucket";

const GITHUB_TOKEN_V1: &str = "token-v1";
const BITBUCKET_USERNAME_V1: &str = "username-v1";
const BITBUCKET_TOKEN_V1: &str = "api-token-v1";
const FINGERPRINT_DOMAIN: &[u8] = b"specforge-provider-credential-v1";

/// The source selected by credential precedence. `None` is used only by
/// non-secret configuration views; a [`ResolvedCredentialContext`] always has
/// one of the first three variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialSource {
    Environment,
    Secure,
    Legacy,
    None,
}

/// Stable, non-secret identity of one provider credential. It is deliberately
/// not serialisable and its debug form is redacted: snapshots and cached
/// details use it only inside the service. A later private action-store type
/// may explicitly encode the digest without making it part of an IPC shape.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CredentialFingerprint([u8; 32]);

impl fmt::Debug for CredentialFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialFingerprint(<redacted>)")
    }
}

impl CredentialFingerprint {
    /// The fingerprint's digest as 64 lowercase hex digits, the spelling an
    /// authorisation stamp persists (`pull-request-actions`: *Current State
    /// Authorises Every Draft and Submission Boundary*). The digest is a
    /// one-way derivative, never a credential or a token fragment; it is
    /// compared and persisted only inside the service and its action file,
    /// and never crosses IPC, an event or a log.
    pub(crate) fn action_digest(&self) -> String {
        self.0
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }
}

/// Non-secret context returned beside a freshly resolved credential. The
/// BitBucket principal is trimmed and Unicode-lowercased before both storage
/// here and fingerprinting; GitHub tokens have no configured principal.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ResolvedCredentialContext {
    provider: PullRequestProvider,
    principal: Option<String>,
    source: CredentialSource,
    fingerprint: CredentialFingerprint,
}

impl fmt::Debug for ResolvedCredentialContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedCredentialContext")
            .field("provider", &self.provider)
            .field("principal_present", &self.principal.is_some())
            .field("source", &self.source)
            .field("fingerprint", &"<redacted>")
            .finish()
    }
}

impl ResolvedCredentialContext {
    pub(crate) fn source(&self) -> CredentialSource {
        self.source
    }

    pub(crate) fn fingerprint(&self) -> CredentialFingerprint {
        self.fingerprint
    }
}

/// A resolved GitHub token plus its non-secret authorisation context. Its
/// debug form never includes the token or digest.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GithubCredential {
    token: String,
    context: ResolvedCredentialContext,
}

impl fmt::Debug for GithubCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GithubCredential")
            .field("context", &self.context)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl GithubCredential {
    pub(crate) fn new(token: String, source: CredentialSource) -> Self {
        let context = credential_context(PullRequestProvider::Github, None, source, &token);
        Self { token, context }
    }

    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    pub(crate) fn context(&self) -> &ResolvedCredentialContext {
        &self.context
    }
}

/// A complete resolved BitBucket pair plus its non-secret authorisation
/// context. The username remains available for Basic authentication; only its
/// normalised form enters the context and fingerprint.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct BitbucketCredential {
    username: String,
    api_token: String,
    context: ResolvedCredentialContext,
}

impl fmt::Debug for BitbucketCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitbucketCredential")
            .field("context", &self.context)
            .field("username", &"<redacted>")
            .field("api_token", &"<redacted>")
            .finish()
    }
}

impl BitbucketCredential {
    pub(crate) fn new(username: String, api_token: String, source: CredentialSource) -> Self {
        let principal = normalize_principal(&username);
        let context = credential_context(
            PullRequestProvider::Bitbucket,
            Some(principal),
            source,
            &api_token,
        );
        Self {
            username,
            api_token,
            context,
        }
    }

    pub(crate) fn username(&self) -> &str {
        &self.username
    }

    pub(crate) fn api_token(&self) -> &str {
        &self.api_token
    }

    pub(crate) fn context(&self) -> &ResolvedCredentialContext {
        &self.context
    }
}

fn normalize_principal(principal: &str) -> String {
    principal.trim().to_lowercase()
}

/// A fixture fingerprint for tests: `seed` digested alone, never equal to a
/// real credential's. Test-only, so no production path can stamp data with it.
#[cfg(test)]
pub(crate) fn test_fingerprint(seed: &str) -> CredentialFingerprint {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, seed.as_bytes());
    CredentialFingerprint(hasher.finalize().into())
}

fn credential_context(
    provider: PullRequestProvider,
    principal: Option<String>,
    source: CredentialSource,
    secret: &str,
) -> ResolvedCredentialContext {
    debug_assert!(source != CredentialSource::None);
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, FINGERPRINT_DOMAIN);
    hash_field(
        &mut hasher,
        match provider {
            PullRequestProvider::Github => b"github",
            PullRequestProvider::Bitbucket => b"bitbucket",
        },
    );
    hash_field(
        &mut hasher,
        principal.as_deref().unwrap_or_default().as_bytes(),
    );
    hash_field(&mut hasher, secret.as_bytes());
    ResolvedCredentialContext {
        provider,
        principal,
        source,
        fingerprint: CredentialFingerprint(hasher.finalize().into()),
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

/// A non-secret key into the operating-system credential store.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialEntryId {
    namespace: &'static str,
    entry: String,
}

impl CredentialEntryId {
    pub fn new(namespace: &'static str, entry: impl Into<String>) -> Self {
        Self {
            namespace,
            entry: entry.into(),
        }
    }

    pub fn namespace(&self) -> &'static str {
        self.namespace
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }
}

/// Allocate a versioned secure entry for a staged GitHub token.
pub fn new_github_entry_id() -> CredentialEntryId {
    versioned_entry(GITHUB_NAMESPACE, GITHUB_TOKEN_V1)
}

/// Allocate the two versioned entries that form one staged BitBucket pair.
pub fn new_bitbucket_entry_ids() -> (CredentialEntryId, CredentialEntryId) {
    let transaction = Uuid::new_v4();
    (
        CredentialEntryId::new(
            BITBUCKET_NAMESPACE,
            format!("{BITBUCKET_USERNAME_V1}:{transaction}"),
        ),
        CredentialEntryId::new(
            BITBUCKET_NAMESPACE,
            format!("{BITBUCKET_TOKEN_V1}:{transaction}"),
        ),
    )
}

fn versioned_entry(namespace: &'static str, version: &str) -> CredentialEntryId {
    CredentialEntryId::new(namespace, format!("{version}:{}", Uuid::new_v4()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialStoreOperation {
    Read,
    Write,
    Delete,
}

/// A deliberately secret-free credential-store error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialStoreError {
    operation: CredentialStoreOperation,
    message: String,
}

impl CredentialStoreError {
    pub fn new(operation: CredentialStoreOperation, message: impl Into<String>) -> Self {
        Self {
            operation,
            message: message.into(),
        }
    }

    pub fn operation(&self) -> CredentialStoreOperation {
        self.operation
    }
}

impl fmt::Display for CredentialStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "credential-store {:?} failed: {}",
            self.operation, self.message
        )
    }
}

impl std::error::Error for CredentialStoreError {}

/// Minimal secret-store operations needed by the recoverable settings
/// transaction. Missing entries read as `None`; deleting one is idempotent.
pub trait CredentialStore: Send + Sync {
    fn read(&self, id: &CredentialEntryId) -> Result<Option<String>, CredentialStoreError>;
    fn write(&self, id: &CredentialEntryId, secret: &str) -> Result<(), CredentialStoreError>;
    fn delete(&self, id: &CredentialEntryId) -> Result<(), CredentialStoreError>;
}

/// Production implementation backed by the platform keychain/keyring.
#[derive(Debug, Default)]
pub struct OsCredentialStore;

impl OsCredentialStore {
    fn entry(
        id: &CredentialEntryId,
        operation: CredentialStoreOperation,
    ) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(id.namespace(), id.entry()).map_err(|_| {
            CredentialStoreError::new(
                operation,
                "the operating system rejected the entry identifier",
            )
        })
    }
}

impl CredentialStore for OsCredentialStore {
    fn read(&self, id: &CredentialEntryId) -> Result<Option<String>, CredentialStoreError> {
        match Self::entry(id, CredentialStoreOperation::Read)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(CredentialStoreError::new(
                CredentialStoreOperation::Read,
                "the operating-system credential store is unavailable",
            )),
        }
    }

    fn write(&self, id: &CredentialEntryId, secret: &str) -> Result<(), CredentialStoreError> {
        Self::entry(id, CredentialStoreOperation::Write)?
            .set_password(secret)
            .map_err(|_| {
                CredentialStoreError::new(
                    CredentialStoreOperation::Write,
                    "the operating-system credential store rejected the credential",
                )
            })
    }

    fn delete(&self, id: &CredentialEntryId) -> Result<(), CredentialStoreError> {
        match Self::entry(id, CredentialStoreOperation::Delete)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialStoreError::new(
                CredentialStoreOperation::Delete,
                "the operating-system credential store could not remove the credential",
            )),
        }
    }
}

#[derive(Default)]
struct MemoryState {
    entries: HashMap<CredentialEntryId, String>,
    failures: Vec<InjectedFailure>,
}

struct InjectedFailure {
    operation: CredentialStoreOperation,
    successes_before_failure: usize,
}

/// Cloneable injected fake. Failures are queued by operation, allowing tests
/// to exercise rollback at each secure-store boundary without touching an OS
/// credential prompt or a developer's real keychain.
#[derive(Clone, Default)]
pub struct InMemoryCredentialStore {
    state: Arc<Mutex<MemoryState>>,
}

impl fmt::Debug for InMemoryCredentialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f.debug_struct("InMemoryCredentialStore")
            .field("entry_count", &state.entries.len())
            .field("queued_failure_count", &state.failures.len())
            .finish()
    }
}

impl InMemoryCredentialStore {
    pub fn fail_next(&self, operation: CredentialStoreOperation) {
        self.fail_after(operation, 0);
    }

    /// Fail one operation after `successes_before_failure` matching calls.
    /// Calls of the other operation kinds do not consume the countdown.
    pub fn fail_after(&self, operation: CredentialStoreOperation, successes_before_failure: usize) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .failures
            .push(InjectedFailure {
                operation,
                successes_before_failure,
            });
    }

    pub fn contains(&self, id: &CredentialEntryId) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .contains_key(id)
    }

    pub fn entry_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }

    fn maybe_fail(
        state: &mut MemoryState,
        operation: CredentialStoreOperation,
    ) -> Result<(), CredentialStoreError> {
        if let Some(index) = state
            .failures
            .iter()
            .position(|queued| queued.operation == operation)
        {
            if state.failures[index].successes_before_failure > 0 {
                state.failures[index].successes_before_failure -= 1;
            } else {
                state.failures.remove(index);
                return Err(CredentialStoreError::new(operation, "injected failure"));
            }
        }
        Ok(())
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn read(&self, id: &CredentialEntryId) -> Result<Option<String>, CredentialStoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        Self::maybe_fail(&mut state, CredentialStoreOperation::Read)?;
        Ok(state.entries.get(id).cloned())
    }

    fn write(&self, id: &CredentialEntryId, secret: &str) -> Result<(), CredentialStoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        Self::maybe_fail(&mut state, CredentialStoreOperation::Write)?;
        state.entries.insert(id.clone(), secret.to_string());
        Ok(())
    }

    fn delete(&self, id: &CredentialEntryId) -> Result<(), CredentialStoreError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        Self::maybe_fail(&mut state, CredentialStoreOperation::Delete)?;
        state.entries.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_provider_scoped_versioned_and_unique() {
        let first = new_github_entry_id();
        let second = new_github_entry_id();
        assert_eq!(first.namespace(), GITHUB_NAMESPACE);
        assert!(first.entry().starts_with("token-v1:"));
        assert_ne!(first, second);

        let (username, token) = new_bitbucket_entry_ids();
        assert_eq!(username.namespace(), BITBUCKET_NAMESPACE);
        assert_eq!(token.namespace(), BITBUCKET_NAMESPACE);
        assert!(username.entry().starts_with("username-v1:"));
        assert!(token.entry().starts_with("api-token-v1:"));
        assert_eq!(
            username.entry().split_once(':').unwrap().1,
            token.entry().split_once(':').unwrap().1
        );
    }

    #[test]
    fn memory_store_round_trips_and_deletes_without_an_os_store() {
        let store = InMemoryCredentialStore::default();
        let id = new_github_entry_id();
        assert_eq!(store.read(&id).unwrap(), None);
        store.write(&id, "secret").unwrap();
        assert_eq!(store.read(&id).unwrap().as_deref(), Some("secret"));
        store.delete(&id).unwrap();
        store.delete(&id).unwrap();
        assert_eq!(store.read(&id).unwrap(), None);
    }

    #[test]
    fn memory_store_injects_each_operation_failure() {
        let store = InMemoryCredentialStore::default();
        let id = new_github_entry_id();
        for operation in [
            CredentialStoreOperation::Write,
            CredentialStoreOperation::Read,
            CredentialStoreOperation::Delete,
        ] {
            store.fail_next(operation);
            let error = match operation {
                CredentialStoreOperation::Read => store.read(&id).unwrap_err(),
                CredentialStoreOperation::Write => store.write(&id, "secret").unwrap_err(),
                CredentialStoreOperation::Delete => store.delete(&id).unwrap_err(),
            };
            assert_eq!(error.operation(), operation);
        }
    }

    #[test]
    fn debug_output_never_contains_stored_values() {
        let store = InMemoryCredentialStore::default();
        let id = new_github_entry_id();
        store.write(&id, "never-print-me").unwrap();
        let debug = format!("{store:?}");
        assert!(!debug.contains("never-print-me"));
        assert!(debug.contains("InMemoryCredentialStore"), "{debug}");
        assert!(debug.contains("entry_count: 1"), "{debug}");
    }

    #[test]
    fn a_delayed_failure_lets_the_counted_calls_through_first() {
        let store = InMemoryCredentialStore::default();
        let id = new_github_entry_id();
        store.fail_after(CredentialStoreOperation::Write, 2);
        // Reads never consume a write's countdown.
        store.read(&id).unwrap();
        store.write(&id, "first").unwrap();
        store.write(&id, "second").unwrap();
        let error = store.write(&id, "third").unwrap_err();
        assert_eq!(error.operation(), CredentialStoreOperation::Write);
        assert_eq!(store.read(&id).unwrap().as_deref(), Some("second"));
        // The failure fires once.
        store.write(&id, "fourth").unwrap();
        assert_eq!(store.read(&id).unwrap().as_deref(), Some("fourth"));
    }

    #[test]
    fn credential_debug_forms_name_the_type_but_never_a_secret() {
        let github = GithubCredential::new("ghp-secret-token".into(), CredentialSource::Secure);
        let bitbucket = BitbucketCredential::new(
            "Secret-User".into(),
            "bb-secret-token".into(),
            CredentialSource::Environment,
        );
        let digests = [
            github.context().fingerprint().action_digest(),
            bitbucket.context().fingerprint().action_digest(),
        ];

        let github_debug = format!("{github:?}");
        assert!(
            github_debug.starts_with("GithubCredential"),
            "{github_debug}"
        );
        assert!(github_debug.contains("Secure"), "{github_debug}");
        assert!(!github_debug.contains("ghp-secret-token"), "{github_debug}");

        let bitbucket_debug = format!("{bitbucket:?}");
        assert!(
            bitbucket_debug.starts_with("BitbucketCredential"),
            "{bitbucket_debug}"
        );
        assert!(bitbucket_debug.contains("Environment"), "{bitbucket_debug}");
        assert!(
            bitbucket_debug.contains("principal_present: true"),
            "{bitbucket_debug}"
        );
        for secret in ["bb-secret-token", "Secret-User", "secret-user"] {
            assert!(!bitbucket_debug.contains(secret), "{bitbucket_debug}");
        }

        let context_debug = format!("{:?}", bitbucket.context());
        assert!(
            context_debug.starts_with("ResolvedCredentialContext"),
            "{context_debug}"
        );
        assert!(context_debug.contains("Bitbucket"), "{context_debug}");

        let fingerprint_debug = format!("{:?}", github.context().fingerprint());
        assert!(
            fingerprint_debug.starts_with("CredentialFingerprint"),
            "{fingerprint_debug}"
        );
        for debug in [
            &github_debug,
            &bitbucket_debug,
            &context_debug,
            &fingerprint_debug,
        ] {
            for digest in &digests {
                assert!(!debug.contains(digest.as_str()), "{debug}");
            }
        }
    }

    #[test]
    fn a_bitbucket_principal_is_trimmed_and_lowercased_before_fingerprinting() {
        let fingerprint = |username: &str| {
            BitbucketCredential::new(username.into(), "token".into(), CredentialSource::Secure)
                .context()
                .fingerprint()
        };
        let context = BitbucketCredential::new(
            "  Ada.Lovelace ".into(),
            "token".into(),
            CredentialSource::Secure,
        );
        assert_eq!(context.context().principal.as_deref(), Some("ada.lovelace"));
        // The username used for authentication is kept verbatim.
        assert_eq!(context.username(), "  Ada.Lovelace ");
        assert_eq!(fingerprint("  Ada.Lovelace "), fingerprint("ada.lovelace"));
        assert_ne!(fingerprint("ada.lovelace"), fingerprint("grace.hopper"));
    }

    #[test]
    fn the_store_error_names_its_operation_and_message() {
        let error = CredentialStoreError::new(CredentialStoreOperation::Delete, "keychain locked");
        let text = error.to_string();
        assert!(text.contains("Delete"), "{text}");
        assert!(text.contains("keychain locked"), "{text}");
    }

    // ---- the OS store, through keyring's own credential-builder seam ----

    /// A process-local keyring backend installed as keyring's default
    /// builder, so [`OsCredentialStore`] runs its real code path without a
    /// developer's keychain or a Linux Secret Service daemon. Entries are
    /// shared by `(service, user)`, as a platform store's are. Any entry in
    /// [`UNAVAILABLE_NAMESPACE`] fails as a locked store would.
    mod test_keyring {
        use keyring::credential::{Credential, CredentialApi, CredentialBuilderApi};
        use std::any::Any;
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex, Once};

        pub const UNAVAILABLE_NAMESPACE: &str = "com.avantmedia.specforge.test-unavailable";

        type Entries = Arc<Mutex<HashMap<(String, String), Vec<u8>>>>;

        #[derive(Debug)]
        struct Builder(Entries);

        #[derive(Debug)]
        struct Entry {
            entries: Entries,
            key: (String, String),
        }

        fn locked() -> keyring::Error {
            keyring::Error::NoStorageAccess(Box::new(std::io::Error::other("locked")))
        }

        impl Entry {
            fn check(&self) -> keyring::Result<()> {
                if self.key.0 == UNAVAILABLE_NAMESPACE {
                    Err(locked())
                } else {
                    Ok(())
                }
            }
        }

        impl CredentialApi for Entry {
            fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
                self.check()?;
                self.entries
                    .lock()
                    .unwrap()
                    .insert(self.key.clone(), secret.to_vec());
                Ok(())
            }

            fn get_secret(&self) -> keyring::Result<Vec<u8>> {
                self.check()?;
                self.entries
                    .lock()
                    .unwrap()
                    .get(&self.key)
                    .cloned()
                    .ok_or(keyring::Error::NoEntry)
            }

            fn delete_credential(&self) -> keyring::Result<()> {
                self.check()?;
                self.entries
                    .lock()
                    .unwrap()
                    .remove(&self.key)
                    .map(|_| ())
                    .ok_or(keyring::Error::NoEntry)
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        impl CredentialBuilderApi for Builder {
            fn build(
                &self,
                _target: Option<&str>,
                service: &str,
                user: &str,
            ) -> keyring::Result<Box<Credential>> {
                Ok(Box::new(Entry {
                    entries: Arc::clone(&self.0),
                    key: (service.to_string(), user.to_string()),
                }))
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        /// Install the backend once for this test binary. No other test in
        /// the crate reaches `OsCredentialStore` (`SettingsStore::load` uses
        /// the in-memory store under `cfg(test)`), and every entry id is a
        /// fresh UUID, so tests sharing the backend never collide.
        pub fn install() {
            static INSTALL: Once = Once::new();
            INSTALL.call_once(|| {
                keyring::set_default_credential_builder(Box::new(Builder(Entries::default())));
            });
        }
    }

    #[test]
    fn the_os_store_round_trips_and_deletes_idempotently() {
        test_keyring::install();
        let store = OsCredentialStore;
        let id = new_github_entry_id();
        assert_eq!(store.read(&id).unwrap(), None);
        store.write(&id, "ghp-stored").unwrap();
        assert_eq!(store.read(&id).unwrap().as_deref(), Some("ghp-stored"));
        // Entries are keyed by id: another entry reads nothing.
        assert_eq!(store.read(&new_github_entry_id()).unwrap(), None);
        store.delete(&id).unwrap();
        assert_eq!(store.read(&id).unwrap(), None);
        // A missing entry deletes as success.
        store.delete(&id).unwrap();
    }

    #[test]
    fn an_unavailable_os_store_fails_each_operation_without_the_secret() {
        test_keyring::install();
        let store = OsCredentialStore;
        let id = CredentialEntryId::new(test_keyring::UNAVAILABLE_NAMESPACE, "token-v1:locked");
        let read = store.read(&id).unwrap_err();
        assert_eq!(read.operation(), CredentialStoreOperation::Read);
        let write = store.write(&id, "never-print-me").unwrap_err();
        assert_eq!(write.operation(), CredentialStoreOperation::Write);
        assert!(!write.to_string().contains("never-print-me"));
        let delete = store.delete(&id).unwrap_err();
        assert_eq!(delete.operation(), CredentialStoreOperation::Delete);
    }
}
