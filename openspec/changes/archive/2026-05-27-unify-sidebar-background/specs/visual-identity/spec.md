## ADDED Requirements

### Requirement: macOS Hidden Inset Titlebar Layout

On macOS, the main application window SHALL use a hidden / overlay titlebar so that the system traffic lights float over the top-left of the sidebar. The sidebar background on every platform — including macOS — SHALL render `var(--surface)` via the application stylesheet; no platform-specific transparent fallback or operating-system vibrancy effect is applied beneath the sidebar.

The top of the sidebar SHALL reserve `--space-6` (32px) of safe-area padding on macOS so that traffic-light buttons do not overlap interactive content. The application SHALL provide an explicit drag region across the top 32px of the window on macOS so that the hidden inset titlebar remains draggable; the drag region MAY be either a `data-tauri-drag-region` element or an explicit `getCurrentWindow().startDragging()` call wired to mousedown. The `core:window:allow-start-dragging` permission SHALL be present in the Tauri capabilities ACL so the IPC drag call is allowed.

On Windows and Linux, the operating system's default titlebar SHALL be used. The sidebar background SHALL be `var(--surface)`, matching macOS.

#### Scenario: macOS sidebar renders a solid surface background

- **WHEN** the application launches on macOS
- **THEN** the sidebar element's computed background resolves to `var(--surface)`
- **AND** no `NSVisualEffectView` / `window-vibrancy` material is applied to the main window
- **AND** the traffic-light buttons still appear inset over the sidebar's top-left

#### Scenario: macOS sidebar reserves traffic-light safe area

- **WHEN** the application launches on macOS
- **THEN** the `.split-pane-left` element has top padding of `--space-6` (32px)
- **AND** the first sidebar row clears the traffic-light buttons

#### Scenario: Window draggable from the titlebar strip on macOS

- **WHEN** the user presses and holds the primary mouse button anywhere in the top 32px of the window on macOS, outside the settings-toggle button
- **THEN** the window enters native drag mode
- **AND** moving the mouse moves the window

#### Scenario: Windows and Linux render solid chrome

- **WHEN** the application launches on Windows or Linux
- **THEN** the sidebar background is `var(--surface)`
- **AND** the operating system's default titlebar is used

## REMOVED Requirements

### Requirement: macOS Sidebar Vibrancy and Hidden Inset Titlebar

**Reason**: The sidebar's background is now uniform across platforms — `var(--surface)` on macOS, Windows, and Linux — so the wallpaper-blurred vibrancy effect on macOS is no longer part of the visual identity. The hidden-inset-titlebar half of this requirement survives, narrowed and renamed under the new `macOS Hidden Inset Titlebar Layout` requirement.

**Migration**: The `window-vibrancy` crate, the `apply_vibrancy` call in `crates/specforge/src/lib.rs`, the `body[data-platform="mac"] { background: transparent }` rule, the `body[data-platform="mac"] .split-pane { background: transparent }` rule, and the `background: transparent` line inside `body[data-platform="mac"] .split-pane-left` are all removed. The `padding-top: var(--space-6)` line in the `.split-pane-left` mac override stays. The `titlebar-drag-region` element, the `data-platform="mac"` body attribute, and the `core:window:allow-start-dragging` ACL permission stay. Existing screenshots showing wallpaper-blur sidebars on macOS are obsolete — re-capture against the new solid surface.
