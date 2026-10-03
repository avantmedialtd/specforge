## MODIFIED Requirements

### Requirement: Addressable Viewing State

The application SHALL represent what the center pane is currently showing as an **Address**: a serializable value composed only of stable identifiers.

The Address SHALL be able to name:

- the home surface;
- the settings pane, together with which of its groups is shown (see the *Settings Are Organised Into Groups* requirement in the `settings-view` capability);
- the archive browser, optionally a specific archived change;
- the workspace file browser;
- one markdown file within a browse root;
- a change artifact: `proposal`, `design`, `tasks`, or a named capability spec.

The Address SHALL NOT carry resolved payloads, derived display labels, or any other value that can be re-derived from the registered workspace views. Tree nodes that render nothing — a top-level row's disclosure state, see the *Workspace Tree Hierarchy* requirement in the `spec-browser` capability — SHALL NOT be addressable, so the set of addresses matches the set of states that actually render.

An Address names *what* is shown, never *where* or *how* it is shown. Whether a document is presented in the main window's detail pane or in a reader window SHALL NOT be part of its Address, in the same way that side-pane visibility is not (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability, and the *Reader Presentation Is Not Part of the Address* requirement in the `reader-window` capability). Which group of the settings pane is shown is part of the Address, and the width-dependent form of the group navigation is not.

#### Scenario: An address carries identifiers only

- **WHEN** the user selects an artifact and the application forms its Address
- **THEN** the Address contains only identifiers (workspace slug, change id, artifact kind, optional capability name)
- **AND** it contains no display label, no file contents, and no preloaded view payload

#### Scenario: A non-rendering node has no address

- **WHEN** the user clicks a change disclosure row or the Specs artifact node
- **THEN** the center pane's contents are unchanged
- **AND** no new Address is formed and no history entry is created

#### Scenario: A file selected in the browser has an address

- **WHEN** the user selects a markdown file in the workspace file browser
- **THEN** an Address naming that file within its browse root is formed

#### Scenario: Presentation is not addressed

- **WHEN** the same document is shown in the detail pane and in a reader window
- **THEN** both are named by the same Address

#### Scenario: A settings group has an address

- **WHEN** the user moves to the Integrations group of the settings pane
- **THEN** an Address naming the settings pane and its Integrations group is formed

### Requirement: Address and URL Round-Trip Through a Pure Codec

The application SHALL convert between an Address and a URL path through a codec that depends on no browser API, no registered-workspace data, and no backend call. Encoding an Address and decoding the result SHALL yield an equal Address.

A path the codec cannot parse SHALL decode to an unresolvable outcome rather than a partially-populated Address, so a malformed link never opens an unintended view.

A settings Address SHALL encode its group as the path segment that follows the settings segment. The codec SHALL know every group regardless of host, so that whether the current host offers a group is decided when the view is resolved, not when the path is parsed. Two settings paths need explicit decoding rules:

- The bare settings path names no group. It SHALL decode to the Address of the settings pane's Workspaces group, so that a settings link written before groups existed still opens.
- A settings path whose group segment names no group the codec knows SHALL decode to an unresolvable outcome.

#### Scenario: An address survives a round trip

- **WHEN** any valid Address is encoded to a URL path and that path is decoded again
- **THEN** the decoded Address is equal to the original

#### Scenario: The codec runs without a browser or a backend

- **WHEN** the codec is exercised with no DOM, no history object, and no registered workspaces available
- **THEN** encoding and decoding still succeed
- **AND** no command is dispatched to the backend

#### Scenario: An unparseable path does not open a view

- **WHEN** a path that does not match the Address grammar is decoded
- **THEN** the result is an unresolvable outcome
- **AND** no artifact, file browser, or archive view is rendered from it

#### Scenario: The bare settings path opens the Workspaces group

- **WHEN** the path `/settings` is decoded
- **THEN** the result is the Address of the settings pane's Workspaces group

#### Scenario: Every settings group survives a round trip

- **WHEN** the Address of each of the five settings groups is encoded and the resulting path is decoded again
- **THEN** each decoded Address names the same group as the original

#### Scenario: An unknown settings group does not open a view

- **WHEN** a settings path whose group segment names no known group is decoded
- **THEN** the result is an unresolvable outcome
- **AND** no settings group is rendered from it

### Requirement: Cold-Load Address Resolution

On startup the application SHALL decode its initial address immediately, and SHALL resolve it against the registered workspaces once those are available, yielding exactly one of four outcomes: resolved, ambiguous (see the *Shortest Unambiguous Address* requirement), disabled, or not found.

While resolution is still pending the application SHALL NOT render the home surface as though no address had been supplied, so a deep link does not visibly flash the home surface before settling on its target.

A not-found outcome SHALL be reported to the user as such, with a way to reach the home surface, rather than silently redirecting. Its wording SHALL NOT claim the address matches nothing registered, since a registered workspace can still be missing the change or artifact the address names.

An address whose workspace is registered but **disabled** (see the *Workspace Disable State* requirement in the `workspace-registry` capability) SHALL be reported as disabled rather than as not found. A disabled workspace is absent from the aggregated view and so has nothing to open, but it is still registered, and reporting it as unregistered would contradict the reversibility that disabling promises. The disabled outcome SHALL name the workspace, SHALL offer to re-enable it directly, and SHALL offer a way to reach the settings view's Workspaces group (see the *Entry Points Open the Workspaces Group* requirement in the `settings-view` capability). Re-enabling SHALL make the unchanged address resolve, with no further navigation required.

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
- **AND** a way to reach the settings view's Workspaces group is offered

#### Scenario: Re-enabling from the notice resolves the address

- **WHEN** the user re-enables the workspace from the disabled notice
- **THEN** the same address resolves and its view renders
- **AND** no further navigation is required

#### Scenario: A missing change inside a resolvable workspace is still not found

- **WHEN** the application is loaded at an address naming a change that its registered, enabled workspace no longer contains
- **THEN** the outcome is not found, not disabled

### Requirement: History Entry Discipline

Navigations that change what the center pane shows SHALL create a history entry, so that a back gesture returns to the previously shown view.

Interactions that do not change the addressed view SHALL NOT create a history entry — specifically disclosure open/close, tree keyboard focus traversal, scrolling, filter text, and loading more commits into the graph rail. Replacing an address with its canonical equivalent SHALL replace the current entry rather than adding one.

Moving between the groups of the settings pane SHALL replace the current entry rather than adding one. The groups are parts of one transient view, so a back gesture from any group closes the settings pane and returns to the view shown before it opened.

#### Scenario: Back returns to the previous view

- **WHEN** the user selects one artifact, then another, then issues a back gesture
- **THEN** the center pane renders the first artifact again

#### Scenario: Disclosure toggling creates no history

- **WHEN** the user opens and closes tree disclosure rows without selecting anything
- **THEN** no history entry is created
- **AND** a subsequent back gesture returns to the view shown before those toggles

#### Scenario: Keyboard focus traversal creates no history

- **WHEN** the user moves the tree's keyboard focus across many rows without activating any of them
- **THEN** no history entry is created for the traversal

#### Scenario: Back closes the settings pane

- **WHEN** the user opens the settings pane from an artifact view and issues a back gesture
- **THEN** the settings pane closes
- **AND** the previously shown artifact is rendered again

#### Scenario: Moving between settings groups adds no history

- **WHEN** the user opens the settings pane from an artifact view, moves to two other groups in turn, and issues a back gesture
- **THEN** the settings pane closes
- **AND** the previously shown artifact is rendered again
