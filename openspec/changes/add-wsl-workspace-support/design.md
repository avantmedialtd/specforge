## Context

SpecForge ships a Windows build, but its value proposition — an ambient,
live-updating dashboard over OpenSpec workspaces — assumes the filesystem can
be watched and `git` can be queried. Both assumptions break when the workspace
lives in the **WSL2 Linux filesystem**, reached from the Windows app through the
9P share at `\\wsl.localhost\<distro>\…` (or the legacy `\\wsl$\<distro>\…`).

Two hard facts drive the whole design:

1. **`ReadDirectoryChangesW` is deaf over 9P.** `notify`'s Windows backend
   never sees inotify events fired *inside* the WSL VM by Linux processes
   (git, editors). A native watcher on a `\\wsl.localhost\` path registers
   successfully and then reports nothing, forever.
2. **Windows `git.exe` on a 9P checkout is hostile.** The share reports a Linux
   uid Windows-git doesn't own, tripping the `safe.directory` "dubious
   ownership" guard; and reading the `.git` object store file-by-file over 9P
   is slow enough to make the commit graph painful.

The core (`registry`, `watcher`, `parser`, `git`) is already platform-neutral
Rust with no Tauri dependency; the platform-specific code is all UI chrome
(`#[cfg(target_os = "macos")]` tray/dock/menu). So WSL support is not blocked by
the architecture — it is a set of contained changes inside `openspec-core`.

The developer works on macOS and **cannot dogfood this**, which is itself a
design constraint: maximise the logic that is unit-testable off-Windows and
quarantine the rest behind a small, explicit spike.

## Goals / Non-Goals

**Goals:**

- The Windows app can **register, display, watch, parse, and read** an OpenSpec
  workspace stored in WSL2 via its `\\wsl.localhost\<distro>\…` path.
- `git`-driven features (worktree discovery, commit graph / garden, branch,
  trailers) **work** for WSL repos — not merely degrade.
- A single, stable `RepoId` per WSL repository regardless of how a path was
  canonicalised (verbatim `\\?\UNC\…` vs simplified UNC).
- The bulk of new logic is **unit-testable on macOS/CI**; only genuine 9P
  behaviour requires a Windows box.
- Zero behavioural change for macOS, Linux, and local-drive Windows workspaces.

**Non-Goals:**

- Running SpecForge *inside* WSL via WSLg (that's just the existing Linux build;
  the host does the work).
- Generalising the poll strategy to arbitrary UNC/SMB shares — detection is
  **WSL-specific** by decision. Other network shares keep the native watcher.
- A long-lived in-WSL helper / `inotifywait` event stream (considered, rejected
  for v1 — see Decisions).
- Mutating the user's global git config. WSL detection is per-invocation.
- Multi-distro orchestration beyond reading the distro name out of the
  registered path (whatever distro the path names is the distro used).

## Decisions

### D1 — Detect WSL by UNC host, not by "any UNC path"

A pure `wsl.rs` module owns detection and translation:

```
is_wsl_path(&Path) -> bool
parse_wsl_path(&Path) -> Option<WslPath { distro: String, linux_path: String }>
wsl_to_unc(distro, linux_path) -> PathBuf
```

Recognised forms: `\\wsl$\<distro>\…`, `\\wsl.localhost\<distro>\…`, and the
verbatim `\\?\UNC\wsl$\…` / `\\?\UNC\wsl.localhost\…` that Rust's
`canonicalize()` emits. The first path segment after the host is the distro;
the remainder (with `\`→`/`) is the Linux path.

*Why WSL-specific over "any UNC → poll":* narrower blast radius and an honest
contract. We can only *make git work* (D3) for shares whose distro we can name
and reach via `wsl.exe`; a generic SMB share has no such backend. Tying the poll
strategy to the same WSL predicate keeps watcher and git aligned on one
definition of "this is a WSL workspace." *Alternative considered:* treat all UNC
paths as poll-only and leave their git best-effort — rejected because it would
advertise partial support for shares we can't actually serve.

Everything in `wsl.rs` is pure string/path logic with **no Windows API and no
process execution**, so it is fully unit-tested on macOS.

### D2 — Per-workspace watcher backend via an enum

`WatcherEntry` already holds `Debouncer<RecommendedWatcher, FileIdMap>`. It
becomes:

```rust
#[cfg(windows)]
enum WatcherKind {
    Native(Debouncer<RecommendedWatcher, FileIdMap>),  // event-driven, 200ms debounce
    Poll  (Debouncer<PollWatcher,        FileIdMap>),  // stat sweep, configurable interval
}
// non-Windows: WatcherEntry keeps only the native debouncer, exactly as today.
```

`add_workspace()` consults a pure `watch_strategy(&Path) -> Native | Poll`
(`Poll` iff `is_wsl_path`) and, on Windows, builds the matching debouncer via
`new_debouncer_opt::<PollWatcher, _>(…)` with
`notify::Config::with_poll_interval(<configured>)`. The callback-bridge plumbing
(the mpsc channel, the `Weak<Inner>` processing task, the `openspec/changes/`
event filter) is **identical** for both arms — only the watcher object differs.
The decision is per-workspace, so mixed local+WSL setups each get the right
backend.

**Poll interval — 10s, configurable.** The default is **10 seconds**, not ~1s:
the watched tree is a handful of rarely-touched markdown files and SpecForge is
an *ambient* dashboard, so a coarser sweep is the right floor for cost vs.
freshness, and power users who want snappier updates can tighten it. The
interval is a user setting (`AppSettings`, surfaced only on Windows) threaded
from the shell into the core watcher the same way `debounce` already is; on
macOS/Linux the setting and the poll arm are simply absent (see D6).

*Why polling over an in-WSL event stream:* polling needs nothing installed in
the distro and the watched tree (`openspec/changes/`) is a handful of small
markdown files, so sweep cost and ~1s latency are negligible. *Alternative
considered:* `wsl.exe -d <distro> inotifywait` streaming native events out of
the VM (more responsive, reuses the D3 `wsl.exe` dependency) — rejected for v1
because it needs `inotify-tools` present in every distro and a long-lived child
process per workspace to parse. Polling is the dependency-free floor; an
inotifywait fast-path can layer on later without changing the detection model.

### D3 — `git` via `wsl.exe` to the native Linux git, with output translation

`git.rs` stays the "thin wrapper over the system git binary," but command
construction branches on the workspace path:

```
native:  git -C <cwd> <args…>
wsl:     wsl.exe -d <distro> git -C <linux_path> <args…>
```

Path **inputs** we hand to git (e.g. `-C <cwd>`) are translated UNC→Linux;
path **outputs** git returns (porcelain worktree paths, `--git-common-dir`) are
translated Linux→UNC via `wsl_to_unc`, so every path the rest of the app stores
or `fs::read`s is a consistent Windows-side UNC path.

*Why `wsl.exe` over Windows-git + `-c safe.directory=<p>`:* the alternative is a
smaller diff (silence the ownership guard, paths already come back UNC-shaped,
no translation), **but** it still reads `.git` over 9P (slow), and — decisively
for this project — almost all of its real behaviour can *only* be observed on a
Windows box. The `wsl.exe` route runs git at native VM speed and pushes the new
logic into **pure, macOS-testable** translation/argv-construction code; only the
`wsl.exe` execution itself needs the spike. Given the validation constraint,
maximising testable surface wins.

*Degradation:* if `wsl.exe` is missing or the distro is stopped, the git call
exits non-zero and `git.rs` returns `None` exactly as it does today — the WSL
workspace remains usable as a *flat* workspace (parse + poll + read).

### D4 — One `dunce`-based `canonicalize` helper everywhere

Rust's `std::fs::canonicalize` returns verbatim extended-length paths
(`\\?\UNC\wsl.localhost\…`). If the registry canonicalises one way and a git
output is translated another way, the **same WSL repo yields two `RepoId`s** —
double-counting the tray badge and splitting the aggregated repo view. A single
`canonicalize()` helper (backed by `dunce`, which simplifies verbatim forms)
replaces every raw call site (`registry.rs`, `commands.rs::read_artifact`,
`git::git_common_dir`). `dunce::simplified` also gives the UI a clean
`\\wsl.localhost\…` instead of the `\\?\UNC\` sludge. Pure, round-trip
unit-tested on macOS.

### D5 — Quarantine the unverifiable behind a 4-item spike

The design deliberately concentrates uncertainty. Everything else is asserted by
unit tests that run on the current runners; the Windows spike has exactly four
yes/no checks (see Risks). This is the structural payoff of D1/D3/D4 choosing the
more pure-logic-heavy option each time.

### D6 — `cfg(windows)`-gate the backend; keep the pure helpers cross-platform

WSL paths are a Windows-host concept — the `\\wsl.localhost\<distro>\…` 9P share
does not exist on macOS or Linux — so the **functional backend has no reason to
compile into the macOS/Linux builds**. The OS-touching integration is therefore
`#[cfg(target_os = "windows")]`-gated:

- `watcher.rs`: the `WatcherKind::Poll` arm and the `watch_strategy`-driven
  selection. Non-Windows `WatcherEntry` keeps only the native debouncer,
  byte-for-byte as today.
- `git.rs`: the `wsl.exe` command construction and Linux→UNC output translation.
  Non-Windows builds construct only the native `git -C` command.
- `settings.rs` (Tauri shell): the configurable poll-interval field is surfaced
  only on Windows.

The **one subtlety**, and the reason this is its own decision rather than a
footnote: D5's testing strategy depends on the pure `wsl.rs` logic (parse,
translate, `watch_strategy`, `is_wsl_path`) being **unit-testable on macOS/CI**.
If D6 gated *everything* to Windows, those tests could not compile off-Windows —
and since validation is spike-on-a-box (no Windows CI), they would never run.
Resolution: `wsl.rs` stays **compiled on all targets** (it contains zero Windows
API and no process execution — just string/path math), annotated
`#[cfg_attr(not(target_os = "windows"), allow(dead_code))]` because nothing
calls it off-Windows in non-test builds. Its tests still exercise it everywhere
with synthetic `\\wsl.localhost\…` inputs. Net: backend absent from Mac/Linux,
macOS-testable surface preserved, D5's four-item spike unchanged.

*Alternative considered:* gate the pure module too and run its tests only on a
Windows CI runner — rejected because it contradicts the chosen validation model
and would silently delete the macOS-testability that D1/D3/D4 were designed to
buy.

## Risks / Trade-offs

- **[The poll watcher might not fire over 9P either]** → The whole feature rests
  on `PollWatcher` (which stats, rather than subscribing to OS events) seeing
  Linux-side writes through the 9P share. This is sound in principle — a stat
  loop reads directory mtimes/entries like any other file access — but it is the
  #1 spike item. Mitigation: verify first on the box; if stat-visibility lags
  unacceptably, fall back to an explicit manual-refresh affordance rather than
  shipping a silently-stale dashboard.
- **[`wsl.exe` launch latency per git call]** → Each invocation pays VM-entry
  overhead (~100–300ms). Acceptable for the occasional worktree-list /
  commit-graph refresh; would not be for a hot loop. Mitigation: git calls are
  already event-driven and debounced; batch where a single call can serve
  multiple needs.
- **[Distro name / path assumptions]** → We trust the first UNC segment as the
  distro and assume the standard `\\wsl.localhost\<distro>\<linux-path>` layout.
  Unusual mounts or non-default share names could mis-parse. Mitigation: keep
  `parse_wsl_path` total (return `Option`) and fall back to native/flat handling
  when parsing fails, never panic.
- **[Cannot dogfood on macOS]** → 9P behaviour is unverified until the spike.
  Mitigation: D1/D3/D4 maximise unit-tested surface; the spike is scoped to four
  checks: (1) `PollWatcher` fires on a WSL-side edit; (2)
  `wsl.exe -d <distro> git -C <linux> worktree list --porcelain` returns the
  expected porcelain and the translation round-trips; (3) a WSL repo's `RepoId`
  is stable across two registered worktrees; (4) end-to-end edit→UI latency is
  acceptable at the default 10s interval, and tightening the setting shortens it
  as expected.
- **[New `dunce` dependency]** → Minimal, well-scoped, widely used; trade-off
  accepted to remove the verbatim-UNC `RepoId`-split class of bug at the root.

## Migration Plan

Additive and platform-gated — no migration for existing users. New code paths
activate only when `is_wsl_path` matches. Rollback is removing the WSL branch;
local/macOS/Linux behaviour is untouched throughout. Recommended sequencing:
land the pure `wsl.rs` + `dunce` canonicalisation (fully testable) first, then
the watcher enum, then the `wsl.exe` git backend, then run the Windows spike to
confirm the four behavioural claims before announcing the feature.

## Open Questions

- **Poll interval** — *Resolved (D2):* 10s default, user-configurable
  (Windows-only setting). Open sub-question: also expose a lower bound / warn if
  a user sets an aggressively short interval that hammers 9P? (Lean: soft floor,
  no hard cap.)
- **`inotifywait` fast-path** — worth a follow-up once `wsl.exe` plumbing
  exists, or leave polling permanently? (Deferred; not needed for the contract.)
- **Stopped-distro UX** — accessing the share usually auto-starts the distro;
  do we need any explicit "distro is starting…" affordance, or is the existing
  `is_missing` recompute enough? (Lean: existing handling suffices for v1.)
- **`safe.directory` fallback** — keep the D3-alternative (Windows-git +
  `-c safe.directory`) as a documented fallback if `wsl.exe` is unexpectedly
  unavailable but the user still wants graph features? (Open; currently we
  simply degrade to flat.)
