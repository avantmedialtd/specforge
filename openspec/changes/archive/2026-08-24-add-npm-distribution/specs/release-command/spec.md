## MODIFIED Requirements

### Requirement: Notes Footer Documents Downloads And Caveats

The synthesized notes SHALL include a Downloads footer documenting the macOS, Windows, and Linux artifacts and their install caveats. The footer SHALL state the macOS Gatekeeper workaround for the unsigned build and SHALL state the Windows portable build's WebView2 prerequisite. The footer SHALL also document the npm install channel for the standalone web server, naming the published package, and SHALL state that an npm install requires no quarantine-clearing step — the workaround documented for the downloaded archives does not apply there. The notes SHALL include a Full-Changelog link comparing the previous tag to the new tag.

#### Scenario: macOS unsigned-app workaround is documented

- **WHEN** the command synthesizes a release's notes
- **THEN** the Downloads footer documents how to open the unsigned macOS build (for example a right-click ▸ Open, or clearing the quarantine attribute)

#### Scenario: Windows WebView2 prerequisite is documented

- **WHEN** the command synthesizes a release's notes
- **THEN** the footer states that the portable Windows build requires the system WebView2 runtime and that the installer is the alternative

#### Scenario: npm channel is documented for the web server

- **WHEN** the command synthesizes a release's notes
- **THEN** the footer documents installing the standalone web server from npm and names the published package

#### Scenario: npm install is stated to need no quarantine step

- **WHEN** the footer documents the npm channel alongside the macOS archive caveat
- **THEN** it states that the quarantine-clearing step applies to the downloaded archive and not to an npm install

#### Scenario: Full-Changelog link is generated

- **WHEN** the command synthesizes notes for a release following a previous tag
- **THEN** the notes include a compare link from the previous tag to the new tag
