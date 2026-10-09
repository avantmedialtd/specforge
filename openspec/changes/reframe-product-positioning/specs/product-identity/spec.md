## ADDED Requirements

### Requirement: Positioning Is Consistent Across Product Surfaces

Every surface that introduces SpecForge outside the running app SHALL describe it by its scope: SpecForge follows **OpenSpec** changes across your repositories and worktrees, from the proposal to the pull request. Those surfaces are:

- the README's headline and introduction;
- `bundle.shortDescription` and `bundle.longDescription` in `crates/specforge/tauri.conf.json`;
- the tagline in the macOS About panel's credits text (see the *About Panel States Product and Format* requirement in the `product-identity` capability);
- the `description` of the npm wrapper package built by `wrapperManifest` in `npm/packaging.mjs`;
- the first sentence of `context` in `openspec/config.yaml`, which every artifact-creation prompt receives.

None of them SHALL call SpecForge a "menu-bar viewer". The menu bar, system tray or status area MAY be named only as a place the active-change count appears. The README SHALL also describe SpecForge's network access accurately: each network feature (the Claude and ChatGPT usage gauges and the GitHub and BitBucket pull request panels) is off by default, and none is described as the only one.

#### Scenario: Bundle descriptions and the About tagline carry the positioning

- **WHEN** `crates/specforge/tauri.conf.json` and the About panel's credits text are read
- **THEN** `bundle.shortDescription` SHALL read "SpecForge — follow OpenSpec changes from proposal to pull request, across your repositories"
- **AND** the credits tagline SHALL read "Follow OpenSpec changes from proposal to pull request, across your repositories."
- **AND** `bundle.longDescription` SHALL name the proposal and the pull request
- **AND** none of the three SHALL contain "menu-bar viewer"

#### Scenario: README leads with the scope

- **WHEN** `README.md` is read
- **THEN** its bold headline SHALL read "Follow spec-driven work across all your repositories, from proposal to pull request."
- **AND** its introduction SHALL name the desktop app, the terminal UI and the local web server
- **AND** its Features list SHALL carry bullets for the Dashboard and for pull requests
- **AND** neither the headline nor the introduction SHALL call SpecForge a "menu-bar viewer" or say that it lives in the menu bar

#### Scenario: README states network access accurately

- **WHEN** the README's descriptions of the usage gauges, the pull request panels and the terminal UI's Settings screen are read
- **THEN** no feature SHALL be described as SpecForge's only network call
- **AND** the Claude gauge, the ChatGPT gauge and both pull request panels SHALL each be described as off by default
- **AND** the terminal UI's Settings screen SHALL be described as toggling both quota gauges and both pull request panels

#### Scenario: npm metadata and the agent context carry the positioning

- **WHEN** the manifest returned by `wrapperManifest` and the `context` in `openspec/config.yaml` are read
- **THEN** the manifest's `description` SHALL read "Run the SpecForge web server: follow OpenSpec changes from proposal to pull request, in a browser."
- **AND** the first sentence of `context` SHALL describe SpecForge as following OpenSpec changes from proposal to pull request through a desktop app, a terminal UI and a local web server
