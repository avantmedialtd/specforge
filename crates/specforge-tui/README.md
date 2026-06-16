# specforge-tui

A terminal frontend for **SpecForge** — the same OpenSpec change browser the
desktop app provides, in a single binary you can run over SSH, drop into a tmux
status line, or keep open beside your editor.

It is a pure consumer of the headless [`openspec-app`](../openspec-app)
service — the same `AppService` the desktop Tauri shell uses — so it reads the
identical registered workspaces, cache, watcher, and gamification state with no
IPC and no parallel logic. The TUI is **read-only**: it never writes to a
workspace.

## Build & run

```bash
cargo run -p specforge-tui            # interactive TUI
cargo run -p specforge-tui -- --status   # one-shot snapshot, then exit
cargo run -p specforge-tui -- --line     # one ambient status line, then exit
```

A release binary is `specforge-tui` (e.g. `target/release/specforge-tui`).

## The three faces

| Mode | What it does | Use it for |
|---|---|---|
| *(default)* | Full interactive TUI — browse, dashboard, season, garden, history. | Working in the terminal. |
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
| `1` `2` `3` `4` `5` | Browse / Dashboard / Season / Garden / History |
| `j` / `k` (or `↓` / `↑`) | Move / scroll |
| `Tab` | Switch the tree ⇄ detail pane (Browse) |
| `Enter` / `l` | Open the selected change |
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
- **Dashboard** — summary metrics, today's ships, the contribution heatmap, and
  the per-author leaderboards (gamification on).
- **Season** — the full 30-tier battle-pass ladder, auto-scrolled to your
  current tier, with the treatment locker (gamification on).
- **Garden** — today's commits per workspace, attributed by person colour.
- **History** — a box-drawing commit-graph rail for the selected change's repo.

The **title bar** also carries an opt-in **Claude usage-quota** gauge — your
5-hour and weekly utilization, colored green → orange → red, with a reset
countdown when a window is spent. It's **off by default** and shares the desktop
app's setting (`com.avantmedia.specforge`). When enabled, a background poll reads
your local Claude Code login (read-only) to query Anthropic's usage endpoint —
the TUI's only network activity; with it off, nothing is read or sent.

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
