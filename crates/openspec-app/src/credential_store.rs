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
        assert!(!format!("{store:?}").contains("never-print-me"));
    }
}
