# wsl-workspaces Specification

## Purpose
TBD - created by archiving change add-wsl-workspace-support. Update Purpose after archive.
## Requirements
### Requirement: WSL Workspace Path Detection

The application SHALL recognise a workspace path as WSL-hosted when, and only when, it matches one of the WSL 9P share forms: `\\wsl$\<distro>\<path>`, `\\wsl.localhost\<distro>\<path>`, or the verbatim extended-length forms `\\?\UNC\wsl$\<distro>\<path>` and `\\?\UNC\wsl.localhost\<distro>\<path>`. From a matching path the application SHALL extract the distribution name (the first segment after the host) and the in-distro Linux path (the remainder). A path that does not match any of these forms SHALL be treated as a local workspace. Detection MUST be total — an unparseable path yields "not WSL" rather than an error or panic.

#### Scenario: Modern localhost share is detected

- **WHEN** a workspace is registered at `\\wsl.localhost\Ubuntu\home\dev\project`
- **THEN** the application recognises it as WSL-hosted
- **AND** records the distribution as `Ubuntu` and the Linux path as `/home/dev/project`

#### Scenario: Legacy wsl$ share is detected

- **WHEN** a workspace path uses the legacy `\\wsl$\Ubuntu\home\dev\project` form
- **THEN** the application recognises it as WSL-hosted with the same distribution and Linux path

#### Scenario: Verbatim extended-length form is detected

- **WHEN** a path is presented in the verbatim form `\\?\UNC\wsl.localhost\Ubuntu\home\dev\project` (as `canonicalize` may produce)
- **THEN** the application recognises it as WSL-hosted with distribution `Ubuntu` and Linux path `/home/dev/project`

#### Scenario: Local path is not treated as WSL

- **WHEN** a workspace is registered at a local drive-letter path such as `C:\Users\dev\project`
- **THEN** the application does not treat it as WSL-hosted
- **AND** it is handled by the existing local-workspace code paths unchanged

### Requirement: Linux and UNC Path Translation

For a WSL-hosted workspace the application SHALL translate deterministically between the Windows UNC form and the in-distro Linux form, and the translation MUST round-trip: translating a UNC path to its Linux form and back SHALL yield the original distribution and path. This translation is the single primitive used both to pass path arguments into the in-distro git and to map paths git returns back to Windows-side UNC paths.

#### Scenario: UNC path translates to a Linux path

- **WHEN** the application needs the Linux form of `\\wsl.localhost\Ubuntu\home\dev\project`
- **THEN** it produces `/home/dev/project` for distribution `Ubuntu`

#### Scenario: Linux path translates back to a UNC path

- **WHEN** the application reconstructs the Windows path for distribution `Ubuntu` and Linux path `/home/dev/project/.git/worktrees/feature`
- **THEN** it produces `\\wsl.localhost\Ubuntu\home\dev\project\.git\worktrees\feature`

#### Scenario: Translation round-trips

- **WHEN** a UNC WSL path is translated to its Linux form and that result is translated back
- **THEN** the reconstructed UNC path identifies the same location as the original

### Requirement: Polling Watcher for WSL Workspaces

Because the Windows directory-change API (`ReadDirectoryChangesW`) receives no events for writes made inside the WSL VM across the 9P share, for each WSL-hosted workspace the application SHALL establish its `openspec/changes/` watcher using a polling backend that periodically re-scans the tree, rather than the native OS event backend. Local (non-WSL) workspaces SHALL continue to use the native event backend. The backend choice is made per workspace, so a user may have local workspaces watched natively and WSL workspaces watched by polling at the same time. A change written inside the WSL filesystem SHALL be reflected in the cache and UI within one poll interval.

#### Scenario: WSL workspace uses the polling backend

- **WHEN** a WSL-hosted workspace is registered
- **THEN** its `openspec/changes/` watcher is established with the polling backend

#### Scenario: Local workspace keeps the native backend

- **WHEN** a local drive-letter workspace is registered on the same machine
- **THEN** its watcher uses the native OS event backend, unchanged

#### Scenario: A WSL-side edit is reflected within the poll interval

- **WHEN** a file under a WSL-hosted workspace's `openspec/changes/` directory is modified by a process inside the WSL distribution
- **THEN** the in-memory cache for that workspace is updated within one poll interval
- **AND** the tree pane and badge reflect the change without an application restart

### Requirement: Configurable Poll Interval

The polling interval for WSL workspaces SHALL default to 10 seconds and SHALL be user-configurable through application settings. The setting is persisted across restarts. Because WSL workspaces only occur on Windows, the setting SHALL be surfaced only in the Windows build; the macOS and Linux builds SHALL NOT present it.

#### Scenario: Default interval is ten seconds

- **WHEN** a WSL-hosted workspace is watched and the user has not changed the interval setting
- **THEN** the polling backend re-scans on a 10-second interval

#### Scenario: User adjusts the interval

- **WHEN** the user sets the poll interval to a different value and the setting is persisted
- **THEN** subsequently-established WSL watchers re-scan at the configured interval
- **AND** the configured value is restored after an application restart

#### Scenario: Setting is absent off Windows

- **WHEN** the application runs on macOS or Linux
- **THEN** no poll-interval setting is presented, because WSL workspaces cannot occur there

### Requirement: Git Operations Routed Through the WSL Distribution

For a WSL-hosted repository the application SHALL gather git metadata (repository identity, worktree list, branch, commit history) by invoking the distribution's native git via `wsl.exe -d <distro> git …` rather than pointing a Windows git binary at the 9P checkout. Path arguments passed to git SHALL be translated to their Linux form, and any paths git returns SHALL be translated back to Windows-side UNC paths so that the rest of the application stores and reads consistent paths. If `wsl.exe` is unavailable or the target distribution cannot be reached, the git operation SHALL degrade to no result and the workspace SHALL continue to function as a flat (non-git) workspace, exactly as a non-git workspace does today.

#### Scenario: Worktree list is gathered through the distribution

- **WHEN** the application enumerates the worktrees of a WSL-hosted repository
- **THEN** it invokes the distribution's git via `wsl.exe`
- **AND** the Linux worktree paths it returns are translated to their `\\wsl.localhost\<distro>\…` UNC forms before use

#### Scenario: Repository identity is gathered through the distribution

- **WHEN** the application resolves the git common directory of a WSL-hosted workspace
- **THEN** it invokes the distribution's git via `wsl.exe`
- **AND** the returned Linux common-directory path is translated to its UNC form to form the repository identifier

#### Scenario: Missing wsl.exe degrades to a flat workspace

- **WHEN** the application attempts a git operation for a WSL-hosted workspace and `wsl.exe` cannot be invoked or the distribution is unreachable
- **THEN** the git operation returns no result
- **AND** the workspace continues to function as a flat workspace without aborting registration

### Requirement: Windows-Scoped WSL Backend

The functional WSL backend — the polling watcher arm, the `wsl.exe`-routed git invocation, and the poll-interval setting — SHALL be compiled only into the Windows build. The macOS and Linux builds SHALL NOT include the WSL backend and SHALL behave exactly as they did before this capability existed. The pure path-detection and path-translation logic MAY be compiled on all platforms so that it can be unit-tested off Windows, but it SHALL be inert there (never selected at runtime, since WSL paths cannot occur).

#### Scenario: Non-Windows builds exclude the backend

- **WHEN** the application is built for macOS or Linux
- **THEN** the polling watcher arm, the `wsl.exe` git routing, and the poll-interval setting are not compiled in
- **AND** local-workspace watching and git behaviour are unchanged from before this capability

#### Scenario: Pure detection logic remains testable everywhere

- **WHEN** the test suite runs on macOS or Linux
- **THEN** the WSL path-detection and translation logic is exercised by unit tests with synthetic WSL paths
- **AND** those tests pass without requiring a Windows host

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

