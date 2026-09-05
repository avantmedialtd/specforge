## MODIFIED Requirements

### Requirement: In-Process Shared Application Service

The terminal frontend and the desktop shell SHALL consume a single headless application service (`openspec-app`) that owns settings, the dashboard assembly, first-launch backfill/seeding, watcher lifecycle, and configuration-directory resolution. The terminal frontend SHALL call the service in-process (no inter-process or serialization boundary). The dashboard assembly SHALL be reachable from automated tests independently of either frontend.

#### Scenario: Both frontends compute identical results

- **WHEN** the desktop app and the terminal frontend render the dashboard for the same workspaces and identity on the same machine
- **THEN** they present the same computed progress, garden, and ships

#### Scenario: Assembly is unit-testable

- **WHEN** the dashboard assembly is exercised by an automated test
- **THEN** it runs without instantiating a Tauri application or a terminal
