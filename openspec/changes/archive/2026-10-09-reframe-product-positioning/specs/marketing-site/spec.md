## MODIFIED Requirements

### Requirement: Landing page states the product's positioning

The landing page SHALL open with an H1 stating what SpecForge does for spec-driven work. It SHALL currently read "Spec-driven work, from proposal to pull request." The page title and the meta description SHALL carry the same proposal-to-pull-request framing, and the hero kicker SHALL describe SpecForge as a read-only companion for spec-driven development. The word "viewer" SHALL NOT be used to describe the product in site copy, notwithstanding its use in the product README.

The landing page's "One change, end to end" section SHALL follow a change through three steps (Navigate, Read and Verify), and that arc SHALL reach the pull request. The Navigate step SHALL name the Dashboard, and the Verify step SHALL name pull requests and review progress alongside commits and diffs. The docs index intro SHALL introduce SpecForge by the same scope. Where site copy names the menu bar, system tray or status area, it SHALL name it as a place the active-change count appears, never as what SpecForge is.

#### Scenario: Landing page carries its headline

- **WHEN** the landing page renders
- **THEN** its H1 SHALL read "Spec-driven work, from proposal to pull request."
- **AND** its `<title>` SHALL read "SpecForge — Spec-driven work, from proposal to pull request"
- **AND** the meta description SHALL name both the proposal and the pull request
- **AND** the hero kicker SHALL read "A read-only companion for spec-driven development"

#### Scenario: One change, end to end reaches the pull request

- **WHEN** the landing page's "One change, end to end" section renders
- **THEN** its H2 SHALL read "Follow the thinking all the way to the pull request."
- **AND** it SHALL hold exactly three articles, labelled "01 / Navigate", "02 / Read" and "03 / Verify"
- **AND** the Navigate article SHALL name the Dashboard
- **AND** the Verify article SHALL name commits, diffs, pull requests and the files and hunks already viewed

#### Scenario: Docs index intro carries the same framing

- **WHEN** the docs index at `/docs` renders
- **THEN** its intro SHALL say that SpecForge follows changes from proposal to pull request
- **AND** it SHALL name the desktop app, the terminal and the browser as the ways to run it
- **AND** it SHALL NOT say that SpecForge lives in the menu bar

#### Scenario: The product is never introduced as a viewer or a menu-bar app

- **WHEN** the landing page and the docs index intro are read
- **THEN** no copy SHALL call SpecForge a "viewer"
- **AND** no copy SHALL describe SpecForge as a menu-bar app; the menu bar, system tray or status area appears only as a place the count is shown

### Requirement: Read-only is framed as a design choice

The site SHALL frame SpecForge's read-only posture as a deliberate design choice, in a dedicated section stating that it never edits specs, never toggles checkboxes and never touches git, and that it reads pull requests from GitHub or BitBucket only once a token is added and never comments on, approves or merges them. That framing SHALL NOT be hedged with "yet", "for now" or "v1 only".

#### Scenario: The read-only section reads as intent, not limitation

- **WHEN** the landing page renders
- **THEN** a section SHALL describe the read-only posture as a design choice
- **AND** it SHALL contain no hedging language implying the posture is temporary

#### Scenario: The read-only section covers pull requests

- **WHEN** the landing page's read-only section renders
- **THEN** it SHALL state that pull requests are read only after a token is added
- **AND** it SHALL state that SpecForge never comments on, approves or merges a pull request
