# view-routing Specification

## Purpose

Defines addressable view routing: a serializable, identifier-only Address naming what the centre pane is showing, a pure Address-to-URL codec, registry-slug identity that never embeds a host filesystem path, the shortest-unambiguous-address rule and its disambiguation behaviour, two host-detected history adapters (the browser's session history when served over HTTP, an in-memory stack in the desktop shell), cold-load resolution, transient tree reveal that never writes the persisted collapse overrides, and the discipline governing which navigations create history entries.

## Requirements

### Requirement: Addressable Viewing State

The application SHALL represent what the center pane is currently showing as an **Address**: a serializable value composed only of stable identifiers.

The Address SHALL be able to name the home surface, the settings pane, the archive browser (optionally a specific archived change), the workspace file browser, and a change artifact (`proposal`, `design`, `tasks`, or a named capability spec).

The Address SHALL NOT carry resolved payloads, derived display labels, or any other value that can be re-derived from the registered workspace views. Tree nodes that render nothing — change disclosure rows and the Specs artifact node, see the *Deferred Interaction Nodes* requirement in the `spec-browser` capability — SHALL NOT be addressable, so the set of addresses matches the set of states that actually render.

#### Scenario: An address carries identifiers only

- **WHEN** the user selects an artifact and the application forms its Address
- **THEN** the Address contains only identifiers (workspace slug, change id, artifact kind, optional capability name)
- **AND** it contains no display label, no file contents, and no preloaded view payload

#### Scenario: A non-rendering node has no address

- **WHEN** the user clicks a change disclosure row or the Specs artifact node
- **THEN** the center pane's contents are unchanged
- **AND** no new Address is formed and no history entry is created

### Requirement: Address and URL Round-Trip Through a Pure Codec

The application SHALL convert between an Address and a URL path through a codec that depends on no browser API, no registered-workspace data, and no backend call. Encoding an Address and decoding the result SHALL yield an equal Address.

A path the codec cannot parse SHALL decode to an unresolvable outcome rather than a partially-populated Address, so a malformed link never opens an unintended view.

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

### Requirement: Workspace Identity Is a Registry Slug

An Address SHALL identify a workspace or repository by a **slug** derived from that workspace's stable registered name.

The slug SHALL NOT be derived from the configured display-name override, so renaming a row for presentation never invalidates existing links. An Address SHALL NOT contain an absolute filesystem path, so host directory layout is never published in a URL, a bookmark, or another person's browser history.

A slug that matches no registered workspace SHALL resolve to a not-found outcome, and SHALL NOT be used to read any filesystem location. An Address therefore cannot name a path the user has not registered.

#### Scenario: Renaming a workspace for display does not break its links

- **WHEN** the user sets or changes a workspace's display-name override
- **THEN** an Address formed before the change still resolves to that workspace

#### Scenario: An address never contains a host path

- **WHEN** any Address is encoded to a URL path
- **THEN** the path contains no absolute filesystem path segment

#### Scenario: An unknown slug reads nothing

- **WHEN** an Address naming a slug that matches no registered workspace is resolved
- **THEN** the application reports the address as not found
- **AND** no artifact read or directory listing is attempted for it

### Requirement: Shortest Unambiguous Address

The application SHALL emit the shortest address form that resolves uniquely against the currently registered workspaces, and SHALL include a disambiguating segment only where one is needed.

A logical change with a single instance SHALL be addressed without an instance segment; a logical change with more than one instance SHALL include one. Workspaces whose slugs would collide SHALL each receive a distinguishing suffix.

When resolving an address that matches more than one candidate — because a colliding workspace was registered, or a second instance of a change was created, since the address was formed — the application SHALL present the matching candidates for the user to choose between, and SHALL NOT select one on the user's behalf.

#### Scenario: A unique workspace uses its bare slug

- **WHEN** exactly one registered workspace slugifies to a given name and an Address for it is emitted
- **THEN** the emitted address uses that bare slug with no suffix

#### Scenario: Colliding workspaces are distinguished

- **WHEN** two registered workspaces slugify to the same name and Addresses for both are emitted
- **THEN** each emitted address carries a distinguishing suffix
- **AND** the two addresses are different

#### Scenario: A single-instance change omits the instance segment

- **WHEN** a logical change has exactly one instance and an Address for one of its artifacts is emitted
- **THEN** the emitted address contains no instance segment

#### Scenario: A multi-instance change names its instance

- **WHEN** a logical change has more than one instance and an Address for an artifact of one of them is emitted
- **THEN** the emitted address identifies which instance it refers to

#### Scenario: An address that has become ambiguous presents a choice

- **WHEN** an address that previously resolved uniquely now matches more than one candidate
- **THEN** the application presents the matching candidates for selection
- **AND** it does not render either candidate as though the address were unambiguous

### Requirement: Cold-Load Address Resolution

On startup the application SHALL decode its initial address immediately, and SHALL resolve it against the registered workspaces once those are available, yielding exactly one of three outcomes: resolved, ambiguous (see the *Shortest Unambiguous Address* requirement), or not found.

While resolution is still pending the application SHALL NOT render the home surface as though no address had been supplied, so a deep link does not visibly flash the home surface before settling on its target.

A not-found outcome SHALL be reported to the user as such, with a way to reach the home surface, rather than silently redirecting.

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

### Requirement: Navigation Reveal Is Transient

When an address names a tree node that is not currently visible, the application SHALL reveal that node by opening its ancestors, and SHALL do so without writing the persisted collapse or expand override sets defined in the *User Collapse State Persists Across Sessions* requirement in the `spec-browser` capability.

Following a link SHALL therefore never alter the user's stored tree preferences, and the revealed ancestors SHALL return to their persisted state once the user navigates elsewhere.

#### Scenario: A deep address reveals its node

- **WHEN** an address names an artifact whose ancestor nodes are currently collapsed
- **THEN** those ancestors are shown open so the addressed node is visible
- **AND** the addressed node is shown as selected

#### Scenario: Following a link does not rewrite stored tree preferences

- **WHEN** the user follows an address that reveals nodes they had previously collapsed
- **THEN** the persisted collapsed and expanded override sets are unchanged
- **AND** no settings write is performed as a result of the reveal

#### Scenario: A revealed ancestor reverts after navigating away

- **WHEN** a node was revealed by an address and the user then navigates to an unrelated address
- **THEN** the previously revealed ancestors render according to their persisted state again

### Requirement: History Entry Discipline

Navigations that change what the center pane shows SHALL create a history entry, so that a back gesture returns to the previously shown view.

Interactions that do not change the addressed view SHALL NOT create a history entry — specifically disclosure open/close, tree keyboard focus traversal, scrolling, filter text, and loading more commits into the graph rail. Replacing an address with its canonical equivalent SHALL replace the current entry rather than adding one.

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

### Requirement: Host-Detected History Adapter

The application SHALL drive navigation through a single history interface with two implementations selected by host: a browser implementation backed by the browser's own session history when served over HTTP, and an in-memory implementation when hosted in the SpecForge desktop shell.

Both implementations SHALL produce identical navigation semantics for the same sequence of addresses, so behaviour does not diverge between hosts. When served over HTTP the address SHALL be reflected in the browser's location, so it can be copied, bookmarked, and reloaded.

#### Scenario: The served UI reflects the address in the browser location

- **WHEN** the user selects an artifact in the browser-served UI
- **THEN** the browser's location shows that artifact's address
- **AND** reloading the page renders the same artifact

#### Scenario: The desktop shell navigates without a browser location

- **WHEN** the user navigates between views in the desktop shell and then issues a back gesture
- **THEN** the previously shown view is rendered
- **AND** no browser location bar is required for this to work

#### Scenario: Both hosts agree on the same address sequence

- **WHEN** the same sequence of addresses is applied through the browser implementation and through the in-memory implementation
- **THEN** both yield the same resulting view at each step

### Requirement: Desktop Back and Forward Gestures

The SpecForge desktop shell SHALL provide keyboard gestures for back and forward navigation, so its in-memory history is reachable by the user.

Because browsers already provide these gestures natively, the served web UI SHALL NOT handle them itself, so a single gesture never navigates twice.

#### Scenario: A desktop gesture navigates back

- **WHEN** the user issues the back keyboard gesture in the desktop shell after selecting two artifacts in turn
- **THEN** the center pane renders the first artifact again

#### Scenario: The web UI does not double-handle the gesture

- **WHEN** the user issues the browser's native back gesture in the served web UI
- **THEN** the application moves back exactly one entry
