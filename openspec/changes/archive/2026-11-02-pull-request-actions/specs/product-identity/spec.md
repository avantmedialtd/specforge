## MODIFIED Requirements

### Requirement: Positioning Is Consistent Across Product Surfaces

Every surface that introduces SpecForge outside the running app SHALL describe it by its scope: SpecForge follows **OpenSpec** changes across your repositories and worktrees, from the proposal to the pull request. Those surfaces are:

- the README's headline and introduction;
- `bundle.shortDescription` and `bundle.longDescription` in `crates/specforge/tauri.conf.json`;
- the tagline and safety line in the macOS About panel's credits text (see the *About Panel States Product and Format* requirement in the `product-identity` capability);
- the `description` of the npm wrapper package built by `wrapperManifest` in `npm/packaging.mjs`;
- the first sentence and safety boundary of `context` in `openspec/config.yaml`, which every artifact-creation prompt receives.

None of them SHALL call SpecForge a "menu-bar viewer". They SHALL name the menu bar, system tray or status area only as a place the active-change count appears. None SHALL describe the whole product as read-only. Wherever these surfaces state the mutation boundary, they SHALL state it accurately and consistently: SpecForge never changes local work or git state; only the native desktop app posts a pull-request review and only after explicit confirmation; the browser skin and terminal do not post reviews; and SpecForge never merges.

The README SHALL also describe SpecForge's network access accurately: each network feature (the Claude and ChatGPT usage gauges and the GitHub and BitBucket pull-request panels) is off by default, and none is described as the only one. It SHALL distinguish provider reads from the desktop's explicitly confirmed review writes and SHALL not imply that a served browser can perform those writes.

#### Scenario: Bundle descriptions and the About tagline carry the positioning

- **WHEN** `crates/specforge/tauri.conf.json` and the About panel's credits text are read
- **THEN** `bundle.shortDescription` SHALL read "SpecForge — follow OpenSpec changes from proposal to pull request, across your repositories"
- **AND** the credits tagline SHALL read "Follow OpenSpec changes from proposal to pull request, across your repositories."
- **AND** `bundle.longDescription` SHALL name the proposal and pull request and state the local-work, confirmed-native-review and never-merge boundary
- **AND** the About credits SHALL carry a concise line with the same boundary
- **AND** none SHALL contain "menu-bar viewer" or describe all of SpecForge as read-only

#### Scenario: README leads with the scope

- **WHEN** `README.md` is read
- **THEN** its bold headline SHALL read "Follow spec-driven work across all your repositories, from proposal to pull request."
- **AND** its introduction SHALL name the desktop app, terminal UI and local web server
- **AND** its Features list SHALL carry bullets for the Dashboard and for pull requests, including explicitly confirmed desktop reviews
- **AND** it SHALL state that local work is never changed, browser and terminal do not post reviews, desktop review submission requires confirmation, and SpecForge never merges
- **AND** neither the headline nor the introduction SHALL call SpecForge a "menu-bar viewer" or say that it lives in the menu bar

#### Scenario: README states network access accurately

- **WHEN** the README's descriptions of the usage gauges, pull-request integrations, review actions and terminal Settings screen are read
- **THEN** no feature SHALL be described as SpecForge's only network call
- **AND** the Claude gauge, ChatGPT gauge and both pull-request panels SHALL each be described as off by default
- **AND** the terminal Settings screen SHALL be described as toggling both quota gauges and both pull-request panels, not as submitting reviews
- **AND** provider review writes SHALL be attributed only to an explicitly confirmed native desktop action

#### Scenario: npm metadata and the agent context carry the positioning

- **WHEN** the manifest returned by `wrapperManifest` and the `context` in `openspec/config.yaml` are read
- **THEN** the manifest's `description` SHALL read "Run the SpecForge web server: follow OpenSpec changes from proposal to pull request in a provider-read-only browser."
- **AND** the manifest SHALL not imply that the browser can submit a review
- **AND** the first sentence of `context` SHALL describe SpecForge as following OpenSpec changes from proposal to pull request through a desktop app, terminal UI and local web server without calling the whole product read-only
- **AND** the context SHALL state that local work is never changed, review writes are native-desktop-only after explicit confirmation, and SpecForge never merges
