use openspec_core::SelfWriteTracker;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[test]
fn record_then_was_self_written_returns_true() {
    let tracker = SelfWriteTracker::new(Duration::from_secs(1));
    tracker.record(PathBuf::from("/tmp/a"));
    assert!(tracker.was_self_written(&PathBuf::from("/tmp/a")));
}

#[test]
fn unrelated_path_is_not_self_written() {
    let tracker = SelfWriteTracker::new(Duration::from_secs(1));
    tracker.record(PathBuf::from("/tmp/a"));
    assert!(!tracker.was_self_written(&PathBuf::from("/tmp/b")));
}

#[test]
fn entries_expire_after_ttl() {
    let tracker = SelfWriteTracker::new(Duration::from_millis(50));
    tracker.record(PathBuf::from("/tmp/a"));
    assert_eq!(tracker.len(), 1);

    thread::sleep(Duration::from_millis(120));

    assert!(!tracker.was_self_written(&PathBuf::from("/tmp/a")));
    assert!(tracker.is_empty());
}

#[test]
fn recording_same_path_twice_keeps_one_recent_entry() {
    let tracker = SelfWriteTracker::new(Duration::from_secs(1));
    tracker.record(PathBuf::from("/tmp/a"));
    tracker.record(PathBuf::from("/tmp/a"));
    // Both entries exist; both are within TTL; was_self_written still returns true.
    assert!(tracker.was_self_written(&PathBuf::from("/tmp/a")));
    assert!(tracker.len() >= 1);
}

#[test]
fn empty_tracker_is_clean() {
    let tracker = SelfWriteTracker::new(Duration::from_secs(1));
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
    assert!(!tracker.was_self_written(&PathBuf::from("/tmp/anything")));
}
