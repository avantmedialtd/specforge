## MODIFIED Requirements

### Requirement: macOS Hidden Inset Titlebar Layout

On macOS, the main application window SHALL use a hidden / overlay titlebar so that the system traffic lights float over the top-left of the sidebar. The sidebar background on every platform — including macOS — SHALL render `var(--surface)` via the application stylesheet; no platform-specific transparent fallback or operating-system vibrancy effect is applied beneath the sidebar.

The sidebar MAY render a 1px inner right-edge highlight via `box-shadow: var(--sidebar-edge)` (`inset -1px 0 0 0 rgba(255, 255, 255, 0.03)` in dark, `rgba(0, 0, 0, 0.04)` in light) so that the solid sidebar reads as the front-most plane against the darker detail pane. This is a `box-shadow` on the same solid `--surface` element and introduces NO `NSVisualEffectView` / window-vibrancy material; the solid-background lock is unchanged.

The top of the sidebar SHALL reserve `--space-6` (32px) of safe-area padding on macOS so that traffic-light buttons do not overlap interactive content. The application SHALL provide an explicit drag region across the top 32px of the window on macOS so that the hidden inset titlebar remains draggable; the drag region MAY be either a `data-tauri-drag-region` element or an explicit `getCurrentWindow().startDragging()` call wired to mousedown. The `core:window:allow-start-dragging` permission SHALL be present in the Tauri capabilities ACL so the IPC drag call is allowed.

This layout is a property of the **native desktop window** and applies only there. Whether the frontend is running inside that native window SHALL be determined from the host itself and SHALL NOT be inferred from the browser user-agent string; only once the native host is established may the operating system be distinguished by any means available. The served web UI SHALL NOT reserve the traffic-light safe-area padding and SHALL NOT render the titlebar drag region on any platform or device — including a browser running on macOS, and including a browser whose user-agent reports a `Macintosh` or `Mac OS X` token while running on a mobile or tablet operating system. Because the drag region is an interaction surface layered over the top of the window, rendering it outside the native window would intercept input intended for the content beneath it; the served web UI SHALL therefore leave that area free.

On Windows and Linux, the operating system's default titlebar SHALL be used. The sidebar background SHALL be `var(--surface)`, matching macOS.

#### Scenario: macOS sidebar renders a solid surface background

- **WHEN** the application launches on macOS
- **THEN** the sidebar element's computed background resolves to `var(--surface)`
- **AND** no `NSVisualEffectView` / `window-vibrancy` material is applied to the main window
- **AND** the sidebar MAY carry the `--sidebar-edge` inner box-shadow highlight, which is not a vibrancy material
- **AND** the traffic-light buttons still appear inset over the sidebar's top-left

#### Scenario: macOS sidebar reserves traffic-light safe area

- **WHEN** the application launches on macOS
- **THEN** the `.split-pane-left` element has top padding of `--space-6` (32px)
- **AND** the first sidebar row clears the traffic-light buttons

#### Scenario: Window draggable from the titlebar strip on macOS

- **WHEN** the user presses and holds the primary mouse button anywhere in the top 32px of the window on macOS, outside the settings-toggle button
- **THEN** the window enters native drag mode
- **AND** moving the mouse moves the window

#### Scenario: Served web UI on macOS reserves no titlebar chrome

- **WHEN** the served web UI is loaded in a browser running on macOS
- **THEN** the side panes reserve no traffic-light safe-area padding
- **AND** no titlebar drag region is rendered over the top of the page
- **AND** the full width of the top of the detail pane accepts input

#### Scenario: A Mac-like mobile user-agent does not enable desktop titlebar chrome

- **WHEN** the served web UI is loaded in a browser whose user-agent contains a `Macintosh` or `Mac OS X` token but which is not the native desktop window
- **THEN** the side panes reserve no traffic-light safe-area padding
- **AND** no titlebar drag region is rendered
- **AND** no vertical space is consumed for window controls that do not exist

#### Scenario: Windows and Linux render solid chrome

- **WHEN** the application launches on Windows or Linux
- **THEN** the sidebar background is `var(--surface)`
- **AND** the operating system's default titlebar is used
