## ADDED Requirements

### Requirement: HTTPS Clients Trust Bundled and System Roots

Every outbound HTTPS client SpecForge uses SHALL verify the server's certificate chain and hostname against one root set: the bundled Mozilla roots together with the operating system's trusted roots. This covers the authenticated provider transport for BitBucket, GitHub and the usage-quota pollers, and the credentialless embedded-image transport (see the *Embedded Image Transport Cannot Reach Private Networks or Carry Credentials* requirement in the `pr-content-loading` capability). The system roots SHALL include the certificates named by the `SSL_CERT_FILE` and `SSL_CERT_DIR` environment variables when they are set.

$$\text{trusted} = \text{Mozilla} \cup \text{OS}$$

The root set SHALL be built at most once per process, on first use. A system certificate that cannot be parsed SHALL be skipped. When the system roots cannot be loaded at all, the root set SHALL be the bundled Mozilla roots alone. No setting, environment variable or failure SHALL disable or relax certificate or hostname verification.

#### Scenario: A corporate root in the system store is trusted

- **WHEN** a TLS-inspecting network presents `api.bitbucket.org` signed by a root CA installed in the system trust store, and not in the Mozilla set
- **THEN** the poller's requests complete and the snapshot is ok

#### Scenario: SSL_CERT_FILE adds a root

- **WHEN** `SSL_CERT_FILE` names a PEM file holding the root CA that signed the presented chain
- **THEN** the connection is trusted

#### Scenario: Public servers still work without system roots

- **WHEN** the system trust store is empty or unreadable and the server presents a chain to a Mozilla root
- **THEN** the connection is trusted

#### Scenario: An untrusted chain is still refused

- **WHEN** the server presents a self-signed certificate that is in neither the Mozilla set nor the system roots
- **THEN** the connection is refused and no request is sent over it

#### Scenario: The image transport uses the same roots

- **WHEN** an embedded image is read from a host whose chain ends at a root found only in the system store
- **THEN** the image transport trusts it, and every other rule of that transport still applies

### Requirement: The Root Set Is Reported Once

When the root set is first built, `specforge-serve` and the desktop application SHALL write one line to standard error stating how many bundled roots and how many system roots it holds, or that the system roots could not be loaded. The line SHALL be written at most once per process. The terminal frontend SHALL NOT write it.

#### Scenario: The startup line counts system roots

- **WHEN** `specforge-serve` makes its first HTTPS request on a machine whose system store holds 151 roots
- **THEN** standard error receives one line reporting the bundled count and 151 system roots
- **AND** no later request repeats it

#### Scenario: A missing system store is visible

- **WHEN** the system roots cannot be loaded
- **THEN** the line says so, and the bundled Mozilla roots are used alone
