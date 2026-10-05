## MODIFIED Requirements

### Requirement: Network Bind Exposure Documented For Downloaders

Because the released `specforge-serve` binary can publish an unauthenticated read API on a network interface, the release SHALL document that the binary binds loopback by default and that requesting a non-loopback bind serves the workspace-reading API to everyone who can reach the port, without authentication.

The release SHALL also document that such a bind discloses, to the same audience, the content of the listed pull requests — their changed files and conversations — read with the serving host's credentials (see the *Local Self-Served Web Server* requirement in the `web-ui` capability). Those pull requests need not belong to any repository cloned on the serving host, so a downloader told only that the workspace-reading API is served would not expect their content to be served too.

#### Scenario: Release notes state the network-bind posture

- **WHEN** a release is published that includes the `specforge-serve` archives
- **THEN** the release notes state that the server binds loopback by default, and that a non-loopback bind is unauthenticated and should be used only on a trusted network

#### Scenario: Release notes state the pull-request disclosure

- **WHEN** a release is published that includes the `specforge-serve` archives
- **THEN** the release notes state that a non-loopback bind also discloses the listed pull requests' content to everyone who can reach the port
- **AND** they state that this content is read with the serving host's credentials
