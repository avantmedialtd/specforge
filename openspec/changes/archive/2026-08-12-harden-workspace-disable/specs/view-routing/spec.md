# view-routing Specification Delta

## MODIFIED Requirements

### Requirement: Cold-Load Address Resolution

On startup the application SHALL decode its initial address immediately, and SHALL resolve it against the registered workspaces once those are available, yielding exactly one of four outcomes: resolved, ambiguous (see the *Shortest Unambiguous Address* requirement), disabled, or not found.

While resolution is still pending the application SHALL NOT render the home surface as though no address had been supplied, so a deep link does not visibly flash the home surface before settling on its target.

A not-found outcome SHALL be reported to the user as such, with a way to reach the home surface, rather than silently redirecting. Its wording SHALL NOT claim the address matches nothing registered, since a registered workspace can still be missing the change or artifact the address names.

An address whose workspace is registered but **disabled** (see the *Workspace Disable State* requirement in the `workspace-registry` capability) SHALL be reported as disabled rather than as not found. A disabled workspace is absent from the aggregated view and so has nothing to open, but it is still registered, and reporting it as unregistered would contradict the reversibility that disabling promises. The disabled outcome SHALL name the workspace and SHALL offer to re-enable it directly, as well as a way to reach the settings view; re-enabling SHALL make the unchanged address resolve, with no further navigation required.

The disabled outcome SHALL be determined only when the address's workspace token matches no workspace in the aggregated view: a change or artifact that is missing inside a workspace that did resolve remains not found. A token that matches neither a workspace in the aggregated view nor a disabled registered row SHALL remain not found, so the disabled outcome is never reported speculatively.

#### Scenario: A deep address restores its view on load

- **WHEN** the application is loaded at an address naming a change artifact in a registered workspace
- **THEN** the center pane renders that artifact once the workspace list is available
- **AND** the corresponding tree node is revealed and shown as selected

#### Scenario: A pending resolution does not flash the home surface

- **WHEN** the application is loaded at a resolvable deep address and the workspace list has not yet arrived
- **THEN** the home surface is not rendered as the center pane's target in the interim

#### Scenario: A stale address reports not found

- **WHEN** the application is loaded at an address whose workspace is no longer registered
- **THEN** the user is told the address could not be found
- **AND** a way to reach the home surface is offered

#### Scenario: An address into a disabled workspace says so

- **WHEN** the application is loaded at an address naming a workspace that is registered but disabled
- **THEN** the user is told that workspace is disabled, by name, rather than that the address was not found
- **AND** a control to re-enable that workspace is offered
- **AND** a way to reach the settings view is offered

#### Scenario: Re-enabling from the notice resolves the address

- **WHEN** the user re-enables the workspace from the disabled notice
- **THEN** the same address resolves and its view renders
- **AND** no further navigation is required

#### Scenario: A missing change inside a resolvable workspace is still not found

- **WHEN** the application is loaded at an address naming a change that its registered, enabled workspace no longer contains
- **THEN** the outcome is not found, not disabled
