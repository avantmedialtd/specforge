# worktree-dev-slots Specification

## Purpose
TBD - created by archiving change worktree-dev-slots. Update Purpose after archive.
## Requirements
### Requirement: Per-Worktree Dev-Server Slot Command

The project SHALL provide a developer command (`bun run wt:dev`) that launches the SpecForge dev application on a port derived from the invoking worktree's slot. The command SHALL be runnable from the main checkout or from any git worktree, and SHALL leave the on-disk `vite.config.ts` and `tauri.conf.json` unmodified — the dev-server port and Tauri `devUrl` SHALL be supplied only at launch time.

#### Scenario: Running the dev command in the main checkout

- **WHEN** a developer runs `bun run wt:dev` from the main checkout
- **THEN** the SpecForge dev server starts on port 1420 (slot 0, today's default)
- **AND** the Tauri shell's `devUrl` points at `http://localhost:1420`

#### Scenario: Running the dev command in a worktree alongside the main checkout

- **WHEN** the main checkout's dev server is already running on 1420
- **AND** a developer runs `bun run wt:dev` from a git worktree assigned a non-zero slot
- **THEN** the worktree's dev server starts on its own slot port without a `strictPort` collision
- **AND** the Tauri shell loads the frontend from that same slot port

### Requirement: Lowest-Free Slot Allocation

The command SHALL assign each worktree a slot from a registry that maps worktree path to slot number. The main checkout SHALL be pinned to slot 0. When a worktree has no slot yet, the command SHALL assign the lowest non-negative integer not currently claimed by a live worktree, then persist it. When a worktree already has a slot, the command SHALL reuse it. The registry SHALL be reconciled against the set of live worktrees so that a removed worktree's slot becomes available again.

#### Scenario: First run assigns the lowest free slot

- **WHEN** `bun run wt:dev` runs in a worktree that has no slot recorded
- **AND** slot 0 is held by the main checkout and slot 1 is held by another worktree
- **THEN** the worktree is assigned slot 2
- **AND** the assignment is persisted to the registry

#### Scenario: Subsequent runs reuse the same slot

- **WHEN** `bun run wt:dev` runs again in a worktree that already has a recorded slot
- **THEN** the same slot is used and the same port is bound
- **AND** no new slot is allocated

#### Scenario: Removing a worktree frees its slot

- **WHEN** a worktree previously assigned slot N is removed
- **AND** `bun run wt:dev` is next run in a worktree with no slot
- **THEN** slot N is treated as free and may be reassigned

### Requirement: Slot-To-Port Derivation

The dev-server port SHALL be derived from the slot as `port = 1420 + slot * 10`, so slot 0 resolves to the default 1420 and each subsequent slot is offset by 10. Both the vite dev-server `--port` and the Tauri `devUrl` SHALL use this derived port for a given run so the shell and the frontend agree.

#### Scenario: Derived ports for successive slots

- **WHEN** slots 0, 1, and 2 are launched
- **THEN** their dev-server ports are 1420, 1430, and 1440 respectively
- **AND** in every case the Tauri `devUrl` matches the launched dev-server port

### Requirement: Shared Application State Across Slots

Slots SHALL isolate only the dev-server port. All slot instances SHALL continue to resolve the same application config directory (derived from the `com.avantmedia.specforge` identifier) and therefore SHALL share the registered-workspaces, settings, presentation, and activity stores. The command SHALL NOT relocate or namespace the application config directory.

#### Scenario: A worktree app shows the main checkout's workspaces

- **WHEN** a developer launches a worktree's dev app via `bun run wt:dev`
- **THEN** the app shows the same registered workspaces as the main checkout, without re-registration
- **AND** no per-slot application config directory is created

