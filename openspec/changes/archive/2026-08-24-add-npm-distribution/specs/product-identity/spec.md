## MODIFIED Requirements

### Requirement: Application Crate and Package Names

The Tauri application crate SHALL be named `specforge` and live at `crates/specforge/`. Its library target SHALL be named `specforge_lib`. The frontend npm package declared in the root `package.json` SHALL be named `specforge`; that package is private and is never published to a registry. The headless OpenSpec-format parser crate `openspec-core` SHALL retain its current name and location.

Packages published to the public npm registry SHALL use the `@avantmedia` scope, with the wrapper package named `@avantmedia/specforge` and each platform package named `@avantmedia/specforge-<platform>`. The scope is required rather than stylistic: the unscoped name `specforge` on the public registry belongs to an unrelated project, so the scope is what preserves the product name as the published identity. A published package SHALL NOT be named `specforge` unscoped.

#### Scenario: Cargo workspace membership

- **WHEN** the Cargo workspace is resolved
- **THEN** it contains a member at `crates/specforge` whose package name is `specforge`
- **AND** it contains a member at `crates/openspec-core` whose package name is `openspec-core`

#### Scenario: Application library symbol

- **WHEN** the Tauri entry point invokes the application library
- **THEN** it calls into a crate named `specforge_lib`

#### Scenario: Frontend package name

- **WHEN** `package.json` is read by tooling
- **THEN** its `name` field is `specforge`
- **AND** it is marked private, so it is never published

#### Scenario: Published packages are scoped

- **WHEN** a package is published to the public npm registry for a release
- **THEN** its name begins with the `@avantmedia/` scope

#### Scenario: Published wrapper preserves the product name

- **WHEN** the published wrapper package is named
- **THEN** it is `@avantmedia/specforge`, carrying the product name inside the scope rather than a renamed variant
