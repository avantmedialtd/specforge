//! Path-scoped process-local and inter-process file locking.
//!
//! Advisory file locks are not enough by themselves: depending on the
//! platform, two handles in one process can be treated as one lock owner. A
//! process-local mutex keyed by the same lock-file path closes that gap. Every
//! caller therefore acquires the local mutex first and the OS lock second.
//! Keeping that order in one small abstraction also makes it possible for the
//! settings and review-action stores to share the same locking rule.

use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, TryLockError, Weak};

static LOCAL_LOCKS: LazyLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// A lock whose identity is one on-disk sidecar path.
///
/// Clones and independently constructed values for the same path share the
/// same process-local mutex. The sidecar is opened lazily when a guard is
/// acquired, so merely constructing a service does not touch the store.
#[derive(Clone, Debug)]
pub struct PathLock {
    path: PathBuf,
    local: Arc<Mutex<()>>,
}

impl PathLock {
    /// Lock the exact `path`. Callers normally use [`Self::for_resource`] so a
    /// data file is never itself held open while it is atomically replaced.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = absolute(path.into());
        let local = {
            let mut locks = LOCAL_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
            match locks.get(&path).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(Mutex::new(()));
                    locks.insert(path.clone(), Arc::downgrade(&lock));
                    lock
                }
            }
        };
        Self { path, local }
    }

    /// Use `<resource filename>.lock` beside a replaceable data file.
    pub fn for_resource(resource: impl AsRef<Path>) -> Self {
        Self::new(sidecar_path(resource.as_ref()))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Wait for both the process-local and operating-system locks.
    pub fn lock(&self) -> io::Result<PathLockGuard<'_>> {
        let local = self
            .local
            .lock()
            .map_err(|_| io::Error::other("path lock was poisoned"))?;
        let file = open_lock_file(&self.path)?;
        FileExt::lock_exclusive(&file)?;
        Ok(PathLockGuard {
            _local: local,
            file,
        })
    }

    /// Acquire both locks without waiting. `Ok(None)` means another live
    /// holder owns either the local or inter-process half.
    pub fn try_lock(&self) -> io::Result<Option<PathLockGuard<'_>>> {
        let local = match self.local.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Poisoned(_)) => {
                return Err(io::Error::other("path lock was poisoned"));
            }
        };
        let file = open_lock_file(&self.path)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(PathLockGuard {
                _local: local,
                file,
            })),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// Holds both halves of a [`PathLock`]. Dropping it releases the OS lock before
/// the process-local mutex (field drop order), preserving acquisition order.
pub struct PathLockGuard<'a> {
    _local: MutexGuard<'a, ()>,
    file: File,
}

impl Drop for PathLockGuard<'_> {
    fn drop(&mut self) {
        // Unlock errors cannot be acted on while unwinding a scope. Closing the
        // file is the platform backstop and happens immediately after `drop`.
        let _ = FileExt::unlock(&self.file);
    }
}

/// The sidecar path used for a replaceable resource.
pub fn sidecar_path(resource: &Path) -> PathBuf {
    let mut name = resource
        .file_name()
        .map_or_else(|| "store".into(), std::ffi::OsString::from);
    name.push(".lock");
    resource.with_file_name(name)
}

fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_lock_uses_a_sidecar() {
        let path = Path::new("/config/settings.json");
        assert_eq!(
            sidecar_path(path),
            PathBuf::from("/config/settings.json.lock")
        );
    }

    #[test]
    fn independently_constructed_locks_serialize_in_one_process() {
        let dir = tempfile::tempdir().unwrap();
        let resource = dir.path().join("state.json");
        let first = PathLock::for_resource(&resource);
        let second = PathLock::for_resource(&resource);

        let held = first.lock().unwrap();
        assert!(second.try_lock().unwrap().is_none());
        drop(held);
        assert!(second.try_lock().unwrap().is_some());
    }

    #[test]
    fn different_paths_do_not_contend() {
        let dir = tempfile::tempdir().unwrap();
        let first = PathLock::for_resource(dir.path().join("one.json"));
        let second = PathLock::for_resource(dir.path().join("two.json"));

        let _held = first.lock().unwrap();
        assert!(second.try_lock().unwrap().is_some());
    }
}
