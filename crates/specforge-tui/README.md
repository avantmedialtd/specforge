# specforge-tui

A terminal frontend for **SpecForge** — the same OpenSpec change browser the
desktop app provides, in a single binary you can run over SSH, drop into a tmux
status line, or keep open beside your editor.

It is a pure consumer of the headless [`openspec-app`](../openspec-app)
service — the same `AppService` the desktop Tauri shell uses — so it reads the
identical registered workspaces, cache, watcher, and settings state with no
IPC and no parallel logic. The TUI is **read-only** with respect to your
workspaces: it never writes to a workspace. It does write SpecForge's own **app
config** (the shared `settings.json`, the workspace registry, and the
presentation store — all outside any workspace) when you flip a toggle or add,
remove, rename, or recolour a workspace on the Settings screen.

## Build & run

```bash
cargo run -p specforge-tui            # interactive TUI
cargo run -p specforge-tui -- --status   # one-shot snapshot, then exit
cargo run -p specforge-tui -- --line     # one ambient status line, then exit
```

A release binary is `specforge-tui` (e.g. `target/release/specforge-tui`).

## Install (prebuilt)

Every SpecForge [release](https://github.com/avantmedialtd/specforge/releases/latest)
ships the TUI as a standalone archive — no need to build from source:

| Platform | Asset |
|---|---|
| **macOS** (universal) | `specforge-tui_<version>_macos-universal.tar.gz` |
| **Linux** (x64) | `specforge-tui_<version>_linux-x64.tar.gz` |
| **Windows** (x64) | `specforge-tui_<version>_windows-x64.zip` |

Extract and run `./specforge-tui`. The binaries are **unsigned**; on macOS a
terminal binary has no Gatekeeper "right-click ▸ Open" dialog, so clear the
quarantine flag before the first run:

```bash
xattr -dr com.apple.quarantine specforge-tui
```

## The three faces

| Mode | What it does | Use it for |
|---|---|---|
| *(default)* | Full interactive TUI — browse, dashboard, garden, history, settings. | Working in the terminal. |
| `--status` | Prints every workspace and its active changes, then exits. | Piping, scripts, a quick glance. |
| `--line` | Prints `SpecForge · N workspaces · M open changes`, then exits. | A prompt segment or tmux status bar. |

Ambient examples:

```bash
# tmux status-right
set -g status-right '#(specforge-tui --line)'

# shell prompt precmd (zsh)
precmd() { specforge-tui --line }
```

## Interactive keys

| Key | Action |
|---|---|
| `1` `2` `3` `4` `5` | Browse / Dashboard / Garden / History / Settings |
| `j` / `k` (or `↓` / `↑`) | Move / scroll |
| `Tab` | Switch the tree ⇄ detail pane (Browse) |
| `Enter` / `l` | Open the selected change |
| `Space` / `Enter` | Toggle the focused setting (Settings screen) |
| `a` / `x` | Add / remove a workspace (Settings screen) |
| `r` / `c` | Rename / recolour the focused workspace (Settings screen) |
| `[` / `]` | Previous / next artifact tab (proposal · design · tasks · spec:&lt;cap&gt;) |
| `h` | Back to the tree |
| `/` | Filter the tree by title/name (`Enter` applies, `Esc` clears) |
| `m` | Load more history (History screen) |
| `?` | Toggle the help overlay |
| `Esc` | Back to Browse, or clear the filter / close help |
| `q` / `Ctrl-c` | Quit |

### Screens

- **Browse** — a workspace/change tree with status glyphs and a task-progress
  bar, beside a markdown detail pane with an artifact tab bar. Below ~90 columns
  it collapses to a single focused pane.
- **Dashboard** — summary metrics, today's ships, the contribution heatmap, the
  streak, and the per-author leaderboard.
- **Garden** — today's commits per workspace, attributed by person colour.
- **History** — a box-drawing commit-graph rail for the selected change's repo.
- **Settings** — toggle rows for the app settings the terminal acts on — the
  **Claude** and **ChatGPT usage-quota** gauges (independent toggles) —
  followed by a **Workspaces** section that
  manages the registry: `a` adds a workspace (type or paste a folder path
  containing `openspec/`), `x` removes the focused one (with a cascade-aware
  confirm), `r` renames it, and `c` cycles its palette colour. `j`/`k` move and
  every change is persisted immediately and reflected in the running TUI at once;
  a running desktop app picks it up on its next launch.

The **title bar** also carries up to two opt-in usage-quota gauge groups, Claude
first and ChatGPT second — your 5-hour and weekly (or, for ChatGPT, whatever
window lengths its usage endpoint reports) utilization, colored green → orange →
red, with a reset countdown when a window is spent. Each bar is segmented by
time — Claude's fixed 5 hour / 7 day cells, ChatGPT's cells derived from its
own reported window length (hours up to 24h, days beyond, falling back to 5h/7d
only when the response omits it) — with the current segment underlined as a
live "now" marker, so the fill (budget spent) reads against the marker (time
elapsed) as pace. Both are **off by default** and share the desktop app's
settings (`com.avantmedia.specforge`); when enabled, a background poll per
provider reads your local Claude Code or Codex CLI login (read-only) to query
that provider's usage endpoint — the TUI's only network activity; with both off,
nothing is read or sent. On a narrow terminal the title bar drops whole trailing
groups (ChatGPT before Claude) until the rest fits, so enabling ChatGPT can never
hide an otherwise-visible Claude gauge — at the narrowest widths the gauge
disappears entirely and only the screen title remains.

## Terminal capabilities

Colour and glyphs are detected once from the environment and degrade cleanly:

- **Colour** — truecolor when `COLORTERM=truecolor`/`24bit`; otherwise 256-colour
  (`*-256color`), then the 16 ANSI names. `NO_COLOR` or `TERM=dumb` drops to no
  colour at all (the layout still reads via bold/box-drawing).
- **Glyphs** — emoji/Unicode markers are used only when the locale advertises
  UTF-8 (`LANG`/`LC_*`) and the terminal isn't `dumb`; otherwise everything falls
  back to ASCII.

## State: shared locally, isolated remotely

The TUI resolves the **same config directory** as the desktop app
(`com.avantmedia.specforge`). On your own machine that means it shows the exact
workspaces you've registered in SpecForge — they share one source of truth, and
edits made through the desktop app appear live (the TUI subscribes to the same
filesystem watcher).

Over SSH or on a different machine, that config directory is the *remote's* —
so a remote `specforge-tui` reflects whatever workspaces are registered there,
independent of your laptop. Register workspaces on the host where you run the
TUI; there is no network sync.
