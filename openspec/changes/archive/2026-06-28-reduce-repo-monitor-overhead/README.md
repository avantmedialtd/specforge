# reduce-repo-monitor-overhead

Cut SpecForge's idle CPU: scope per-event git-status recompute to the repo that
changed, collapse the four per-repo filesystem watchers into one, and coalesce
git event bursts into a single refresh.
