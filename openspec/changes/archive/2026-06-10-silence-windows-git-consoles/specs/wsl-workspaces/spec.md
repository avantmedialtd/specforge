# wsl-workspaces — Delta: Silence Windows Git Console Flashes

## ADDED Requirements

### Requirement: Console-Window-Free Git Invocations on Windows

On Windows, every git child process the application spawns — a direct `git.exe` invocation for a locally hosted repository, or a `wsl.exe -d <distro> git …` invocation for a WSL-hosted repository — SHALL be created without a visible console window, while preserving piped stdin/stdout/stderr, exit codes, and error behaviour unchanged. Builds for other platforms SHALL NOT be affected.

#### Scenario: WSL-routed git runs without flashing a console window

- **WHEN** the application invokes the distribution's git via `wsl.exe` for a WSL-hosted workspace (including repeatedly, on the polling watcher's cadence)
- **THEN** no console window appears, and the invocation's output, exit code, and degradation behaviour are identical to before

#### Scenario: Direct git.exe runs without flashing a console window

- **WHEN** the application invokes `git.exe` for a repository on the local Windows filesystem
- **THEN** no console window appears, and the invocation's output and exit code are identical to before

#### Scenario: Non-Windows builds are unaffected

- **WHEN** the application is built for macOS or Linux
- **THEN** the window-suppression flag is not compiled in and git invocations behave exactly as before this requirement existed
