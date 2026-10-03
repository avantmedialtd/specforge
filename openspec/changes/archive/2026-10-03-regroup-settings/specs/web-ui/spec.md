## MODIFIED Requirements

### Requirement: Desktop-Only Settings Are Hidden in the Web UI

Settings that have no meaning for a browser skin SHALL be hidden when the UI is served over HTTP. These are launch-on-login, OS-level notifications, the embedded web server and its Tailscale options, and tray behaviour. Hiding them SHALL reuse the existing convention of omitting a control when its backing query reports the control is not applicable.

A settings group that this rule leaves with no applicable setting SHALL itself be omitted from the settings view's group navigation (see the *Groups With Nothing to Offer Are Omitted* requirement in the `settings-view` capability), so the browser skin never offers an empty group.

#### Scenario: Launch-on-login is absent in the browser

- **WHEN** the Settings view renders in the web UI
- **THEN** the launch-on-login control is not shown
- **AND** the remaining, applicable settings render and function normally

#### Scenario: The Desktop app group is absent in the browser

- **WHEN** the Settings view renders in the web UI
- **THEN** its group navigation offers no Desktop app group
- **AND** the launch-on-login, notifications and web-server settings appear in no other group
