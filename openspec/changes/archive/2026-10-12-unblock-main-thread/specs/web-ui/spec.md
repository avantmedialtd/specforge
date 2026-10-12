## ADDED Requirements

### Requirement: Bounded Tailscale Name Discovery

Discovering the host's Tailscale name from local Tailscale state SHALL complete within a fixed wall-clock bound of 3 seconds. When the process the system spawned to run the local Tailscale CLI has not finished by then, the system SHALL terminate that process and treat the name as undiscovered. Resolution then yields no name, so no non-loopback authority is trusted (fail closed), as the *Tailscale Serve Access* requirement prescribes when no name is available. A timeout SHALL NOT surface as an error and SHALL NOT leave the spawned process running.

A manually configured name SHALL short-circuit discovery, so no Tailscale process is started when an override is set, and a hung daemon cannot affect a host that names itself.

Discovery SHALL NOT run on the desktop's main (UI) thread, and SHALL NOT run inline on an async runtime worker. This covers the Settings view's resolved-name query and the trust-boundary construction when the embedded or standalone server starts. It runs on a blocking pool. A hung Tailscale daemon therefore cannot freeze the desktop window, and it delays server start-up by no more than the bound.

#### Scenario: A hung Tailscale CLI falls back within the bound

- **WHEN** Tailscale Serve support is enabled with no manual name, and the local Tailscale CLI does not exit
- **THEN** discovery gives up within the bound and the spawned process is terminated
- **AND** no Tailscale name is trusted, so a request bearing a tailnet `Host`/`Origin` is refused

#### Scenario: A manual override starts no process

- **WHEN** a manual Tailscale name is configured
- **THEN** resolving the name starts no Tailscale process

#### Scenario: The Settings view stays responsive while the name resolves

- **WHEN** the desktop Settings view asks for the resolved Tailscale name while the local Tailscale CLI is slow to answer
- **THEN** the window keeps painting and handling input until the answer, or the timeout, arrives

#### Scenario: Server start-up is not stalled by a hung CLI

- **WHEN** the embedded or standalone server starts with Tailscale Serve support enabled, no manual name, and a hung local Tailscale CLI
- **THEN** the server begins serving loopback requests within the bound, with no tailnet authority trusted
