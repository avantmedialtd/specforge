# view-routing Specification

## Purpose

Defines addressable view routing: a serializable, identifier-only Address naming what the centre pane is showing, a pure Address-to-URL codec, registry-slug identity that never embeds a host filesystem path, the shortest-unambiguous-address rule and its disambiguation behaviour, two host-detected history adapters (the browser's session history when served over HTTP, an in-memory stack in the desktop shell), cold-load resolution, transient tree reveal that never writes the persisted collapse overrides, and the discipline governing which navigations create history entries.
## Requirements
### Requirement: Addressable Viewing State

The application SHALL represent what the center pane is currently showing as an **Address**: a serializable value composed only of stable identifiers.

The Address SHALL be able to name:

- the home surface;
- the settings pane, together with which of its groups is shown (see the *Settings Are Organised Into Groups* requirement in the `settings-view` capability);
- the archive browser, optionally a specific archived change;
- the workspace file browser;
- one markdown file within a browse root;
- a change artifact: `proposal`, `design`, `tasks`, or a named capability spec;
- a pull request, by its provider, owner, repository and number (see the *Pull-Request Addresses* requirement).

The Address SHALL NOT carry resolved payloads, derived display labels, or any other value that can be re-derived from the registered workspace views or from the providers' pull-request snapshots; a pull request's Address carries no web URL, no title and no detail. Tree nodes that render nothing — a top-level row's disclosure state, see the *Workspace Tree Hierarchy* requirement in the `spec-browser` capability — SHALL NOT be addressable, so the set of addresses matches the set of states that actually render.

An Address names *what* is shown, never *where* or *how* it is shown. Whether a document is presented in the main window's detail pane or in a reader window SHALL NOT be part of its Address, in the same way that side-pane visibility is not (see the *Side-Pane Visibility Toggles* requirement in the `spec-browser` capability, and the *Reader Presentation Is Not Part of the Address* requirement in the `reader-window` capability). Likewise, whether a pull request is presented in the main window's center pane or in a pull-request window SHALL NOT be part of its Address: both presentations encode it to the same path, and the pull-request window's flag is carried outside that path (see the *Pull-Request Window* requirement in the `pull-request-viewer` capability). Which group of the settings pane is shown is part of the Address, and the width-dependent form of the group navigation is not.

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

#### Scenario: A pull request has an address

- **WHEN** the user opens GitHub pull request 42 of `acme/api` from its panel row
- **THEN** an Address naming the provider GitHub, the owner `acme`, the repository `api` and the number 42 is formed
- **AND** it contains no web URL, no title and no detail of the pull request

#### Scenario: A pull request's presentation is not addressed

- **WHEN** the same pull request is shown in the center pane and in a pull-request window
- **THEN** both are named by the same Address
- **AND** in the served web UI both URLs carry the same path and differ only outside it

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

### Requirement: Workspace Identity Is a Registry Slug

An Address that names a registered workspace or repository, or anything within one, SHALL identify that workspace or repository by a **slug** derived from its stable registered name. A pull-request Address names a provider's repository rather than a registered one: its owner — a GitHub owner or a BitBucket workspace — and its repository are the provider's names, not slugs, and SHALL NOT be resolved against the registered workspaces (see the *Pull-Request Addresses* requirement).

The slug SHALL NOT be derived from the configured display-name override, so renaming a row for presentation never invalidates existing links. No Address of any kind SHALL contain an absolute filesystem path, so host directory layout is never published in a URL, a bookmark, or another person's browser history. A pull-request Address SHALL therefore name a pull request by its reference alone, even when the pull request is linked to a worktree, and never by that worktree's path.

A slug that matches no registered workspace SHALL resolve to a not-found outcome, and SHALL NOT be used to read any filesystem location. An Address therefore cannot name a path the user has not registered.

#### Scenario: Renaming a workspace for display does not break its links

- **WHEN** the user sets or changes a workspace's display-name override
- **THEN** an Address formed before the change still resolves to that workspace

#### Scenario: An address never contains a host path

- **WHEN** any Address, a pull-request Address included, is encoded to a URL path
- **THEN** the path contains no absolute filesystem path segment

#### Scenario: An unknown slug reads nothing

- **WHEN** an Address naming a slug that matches no registered workspace is resolved
- **THEN** the application reports the address as not found
- **AND** no artifact read or directory listing is attempted for it

#### Scenario: A pull-request address is not a registry slug

- **WHEN** a registered workspace's slug is `acme` and the address `/pr/bitbucket/acme/api/7` is resolved
- **THEN** it is looked up in the BitBucket snapshot by the workspace `acme`, the repository `api` and the id 7
- **AND** its outcome is the same whether or not the registered workspace `acme` exists

#### Scenario: A linked pull request's address carries no worktree path

- **WHEN** an Address is formed for GitHub pull request 42 of `acme/api`, which is linked to a worktree at `/Users/ada/src/api-feature`
- **THEN** the encoded path is `/pr/github/acme/api/42`
- **AND** the worktree's path appears nowhere in it

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

On startup the application SHALL decode its initial address immediately. Every address other than a pull-request address SHALL be resolved against the registered workspaces once those are available, yielding exactly one of four outcomes: resolved, ambiguous (see the *Shortest Unambiguous Address* requirement), disabled, or not found.

A pull-request address SHALL instead be resolved against its provider's enabled flag and pull-request snapshot, yielding exactly one of five outcomes — pending, provider off, unavailable, listed, or not listed — each shown in the center pane as the outcome table of the *Pull-Request Addresses* requirement states. It SHALL never yield the ambiguous or the disabled outcome, and an address that resolves as provider off, unavailable or not listed SHALL be reported by that reason rather than as not found.

While resolution is still pending the application SHALL NOT render the home surface as though no address had been supplied, so a deep link does not visibly flash the home surface before settling on its target. A pull-request address is pending while its provider's enabled flag or snapshot has not yet been read, and while that provider is enabled and its snapshot still reads `disabled` because its first poll is running; for that whole time the center pane SHALL show "Loading…".

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

#### Scenario: A pull-request address waits for its provider's first list

- **WHEN** the application is loaded at `/pr/github/acme/api/42` while GitHub is enabled and its first poll is still running
- **THEN** the center pane shows "Loading…" and does not render the home surface in the interim
- **AND** once that poll lists pull request 42 of `acme/api`, the center pane renders that pull request with no further navigation

#### Scenario: A pull-request address into a provider that is off says so

- **WHEN** the application is loaded at the address of a BitBucket pull request while BitBucket's configuration says it is not enabled
- **THEN** the user is told that BitBucket is off, by name, rather than that the address was not found
- **AND** a way to reach the settings view's Integrations group is offered

#### Scenario: Enabling the provider resolves the pull-request address

- **WHEN** the center pane shows the provider-off notice for a GitHub pull-request address, no GitHub backoff deadline holds, and the user follows the notice to the Integrations group, enables GitHub and issues a back gesture
- **THEN** the same address shows "Loading…" for as long as GitHub's first poll is running
- **AND** once that poll lists the pull request, the pull request renders with no further navigation

#### Scenario: A provider whose list cannot be read says which

- **WHEN** the application is loaded at the address of a GitHub pull request while GitHub is enabled and its snapshot is unauthenticated
- **THEN** the center pane says that GitHub's list is unauthenticated and points to Settings
- **AND** neither "Loading…" nor the home surface is shown

#### Scenario: A reloaded pull request that has left its list shows its last detail

- **WHEN** the served web UI is reloaded at the address of a pull request that has since left its provider's list, while the provider stays enabled and the service still holds the pull request's cached detail
- **THEN** the center pane shows that detail, marked "no longer listed"
- **AND** no request is sent to the provider for that pull request

#### Scenario: A pull request neither listed nor cached is reported

- **WHEN** the application is loaded at the address of a pull request that its enabled provider's list does not hold, and the service holds no cached detail for it
- **THEN** the center pane says that the pull request is not in the provider's list
- **AND** no request is sent to the provider for that pull request

### Requirement: Navigation Reveal Is Transient

When an address resolves to an artifact of a change, the application SHALL reveal that change in the workspace tree: the change's top-level row (repository group or flat workspace) SHALL be shown open and the change's row SHALL be shown selected. Which artifact and which instance the address names is shown by the change header in the detail pane (see *Artifact Tab Strip in the Change Header* and *Instance Switcher in the Change Header* in the `spec-browser` capability), not by the tree, which stops at the change row.

The reveal SHALL be a pure function of the resolved address, never independently tracked state. A top-level row opened by a reveal SHALL return to the disclosure state it had once the user navigates to an address that does not reveal a change beneath it. The tree persists no disclosure state across sessions (see *Workspace Tree Hierarchy* in the `spec-browser` capability), so a reveal has nothing it could write and SHALL perform no settings write.

#### Scenario: An artifact address reveals its change

- **WHEN** an address names an artifact of a change whose top-level row is currently closed
- **THEN** that row is shown open so the change's row is visible
- **AND** the change's row is shown as selected
- **AND** the addressed artifact is the active tab in the change header

#### Scenario: Following a link performs no settings write

- **WHEN** the user follows an address that opens a top-level row they had closed
- **THEN** no settings write is performed as a result of the reveal

#### Scenario: A revealed row reverts after navigating away

- **WHEN** a top-level row was opened by a reveal and the user then navigates to an address that reveals nothing beneath it
- **THEN** that row renders in the disclosure state it had before the reveal

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

### Requirement: File Addresses

An Address SHALL be able to name one markdown file by its **browse root and root-relative path**, so that any file the workspace file browser can show is linkable, restorable on load, and openable in a reader window. Its URL grammar SHALL place a reserved `file` segment between the scope prefix and the path:

- `/w/<workspace>/file/<path…>` — a file within a flat workspace
- `/r/<repo>/file/<path…>` — a file within a repository, resolved against its tracked worktrees

The path's segments SHALL each be encoded independently and joined with separators, so the address remains a readable path rather than an opaque token, and a path containing characters that require escaping round-trips unchanged.

The `file` segment SHALL be **reserved** at the position a change id otherwise occupies. This is what lets the codec continue to decide the whole grammar from a closed vocabulary with no registry data, as the *Address and URL Round-Trip Through a Pure Codec* requirement demands: without it, a path such as `openspec/specs/<capability>/spec.md` placed directly after a scope prefix is indistinguishable from a capability-spec address followed by a stray segment. A change directory named exactly `file` is consequently not addressable; this is a documented reservation, not a defect to be worked around by making the grammar data-dependent.

A repository-scoped file address SHALL name the **repository**, not one of its worktrees, and SHALL resolve against the repository's pooled listing (see *Union Markdown Listing Across a Repository's Worktrees* in the `workspace-file-browser` capability). Which worktree's copy is rendered is a per-preview choice and is deliberately **not** addressed: resolution SHALL open a default copy — the main worktree's when it holds the path, otherwise the first copy that does.

A file address SHALL NOT carry a worktree instance segment. This prohibition is unchanged and load-bearing: a worktree token is registry data, and admitting one into the grammar would defeat the closed-vocabulary property the reserved `file` segment exists to preserve. Keeping the copy choice out of the Address is what lets that property survive a browse root that now spans several worktrees.

(This supersedes the previous contract, under which a repository-scoped file address named the repository's main worktree. A repository with one tracked worktree resolves exactly as it did before.)

A file address SHALL carry no host filesystem path, only a registry slug and a path relative to the browse root that slug resolves to, per the *Workspace Identity Is a Registry Slug* requirement.

Resolution SHALL follow the *Cold-Load Address Resolution* requirement: a file address into an unknown slug SHALL report not found, one into a disabled workspace SHALL say so, and one naming a file that does not exist beneath a resolvable root SHALL report not found rather than rendering an empty document.

#### Scenario: A file address round-trips

- **WHEN** an Address naming a file within a browse root is encoded to a URL path and decoded again
- **THEN** the decoded Address is equal to the original, including the full relative path

#### Scenario: A nested path keeps its structure

- **WHEN** a file address names a path several directories deep, such as a capability specification beneath the workspace's `openspec/specs/` directory
- **THEN** the encoded path contains that relative path after the reserved `file` segment
- **AND** decoding recovers exactly the same relative path

#### Scenario: A path segment needing escapes survives

- **WHEN** a file address names a path whose segments contain characters that require percent-encoding
- **THEN** the encoded path escapes them per segment
- **AND** decoding recovers the original path unchanged

#### Scenario: The reserved segment disambiguates from an artifact address

- **WHEN** a path whose relative portion begins with `openspec/specs/` is decoded as a file address
- **THEN** it decodes to a file address naming that whole relative path
- **AND** it does not decode to a capability-spec artifact address

#### Scenario: The codec decodes a file address with no registry data

- **WHEN** a file address is decoded with no registered workspaces available
- **THEN** decoding succeeds and yields the scope slug and the relative path
- **AND** no command is dispatched to the backend

#### Scenario: A file address carries no host path

- **WHEN** an Address is formed for a file in a registered workspace
- **THEN** the Address contains that workspace's registry slug and a root-relative path
- **AND** it contains no absolute filesystem path

#### Scenario: A file address carries no worktree segment

- **WHEN** an Address is formed for a file whose copies live in several of a repository's worktrees
- **THEN** the Address names the repository and the root-relative path only
- **AND** it contains no segment identifying a worktree

#### Scenario: A repository file address resolves to a default copy

- **WHEN** a repository-scoped file address is resolved for a repository with several tracked worktrees that all hold the path
- **THEN** it resolves against the repository's main worktree

#### Scenario: A repository file address resolves to the only worktree holding the file

- **WHEN** a repository-scoped file address names a path held by exactly one tracked worktree, which is not the main worktree
- **THEN** it resolves against that worktree
- **AND** it does not report not found

#### Scenario: A file address into an unknown workspace reports not found

- **WHEN** a file address naming a slug that matches no registered workspace or repository is opened
- **THEN** the application reports not found
- **AND** no file is read

#### Scenario: A file address naming a missing file reports not found

- **WHEN** a file address resolves to a registered browse root but names a path that does not exist beneath it
- **THEN** the application reports not found
- **AND** it does not render an empty document

### Requirement: Pull-Request Addresses

An Address SHALL be able to name one pull request by its **provider**, its **owner** (a GitHub owner, or a BitBucket workspace), its **repository** and its **number**, so that a pull request its provider lists is linkable, restorable on load, and reachable with Back and Forward. Its URL grammar SHALL be:

- `/pr/github/<owner>/<repo>/<number>` — a GitHub pull request
- `/pr/bitbucket/<workspace>/<repo>/<id>` — a BitBucket pull request

`pr` SHALL be a top-level word of the codec's closed vocabulary, beside `settings`, `archive`, `w` and `r`, and `github` and `bitbucket` SHALL be the only words accepted beneath it. The codec therefore continues to decide the whole grammar with no provider, snapshot or registry data, as the *Address and URL Round-Trip Through a Pure Codec* requirement demands. The owner and the repository SHALL each be percent-encoded as one segment, as every identifier is; both providers' names use only characters that need no escaping in a path segment, so the path stays readable. The number SHALL be a positive decimal integer written without a sign or leading zeros, and no larger than the codec represents exactly, so each pull request's number has exactly one spelling. Any other shape beneath `pr` — another provider word, a missing or an extra segment, or a number written any other way — SHALL decode to an unresolvable outcome, never to a partial Address. Encoding a pull-request Address and decoding the result SHALL yield an equal Address.

**The reference.** The provider, owner, repository and number that a pull-request Address names SHALL be the pull request's **reference**. Two references $$a$$ and $$b$$ SHALL be equal exactly when

$$a \equiv b \iff \text{provider}_a = \text{provider}_b \;\wedge\; \text{number}_a = \text{number}_b \;\wedge\; \operatorname{lower}(\text{owner}_a) = \operatorname{lower}(\text{owner}_b) \;\wedge\; \operatorname{lower}(\text{repo}_a) = \operatorname{lower}(\text{repo}_b)$$

where $$\operatorname{lower}$$ maps the ASCII letters `A`–`Z` to `a`–`z` and leaves every other character unchanged, so owners and repositories compare ignoring ASCII case. A pull-request Address SHALL carry its reference and nothing else: no web URL, no host, no title, no detail, and nothing chosen inside the view, such as the selected file or the diff layout.

**Resolution.** A pull-request Address SHALL be resolved by a pure lookup against two inputs, as the resolving view last read them: the providers' pull-request snapshots, and each provider's enabled flag. The main window resolves it for its center pane, and the pull-request window resolves its own address with the same lookup (see the *Pull-Request Window* requirement in the `pull-request-viewer` capability). Each SHALL read the flags through `get_github_config` and `get_bitbucket_config` when it mounts, and SHALL keep them current through the `pull-request-provider-changed` notice, carrying the provider and its enabled flag, that the service raises whenever a provider's enabled flag is set. That notice reaches every window and every browser tab of one service, so switching a provider off or on anywhere re-resolves every pull-request address any of them shows, without a reload.

Resolution SHALL reach the first of these outcomes, in this order, whose condition holds for the Address's provider:

| Outcome | Condition | The center pane shows |
|---|---|---|
| **pending** | the provider's enabled flag or its snapshot has not been read yet, or the provider is enabled and its snapshot still reads `disabled` because its first poll is running | "Loading…", never the home surface |
| **provider off** | the provider's configuration says it is not enabled | a notice naming the provider, with a way to reach the settings view's Integrations group |
| **unavailable** | the provider's list is unauthenticated or unavailable | a notice saying which, pointing to Settings when the list is unauthenticated |
| **listed** | the provider's list, fresh or stale, holds a row whose reference equals the Address's and whose web URL is not empty | the pull request |
| **not listed** | otherwise | the pull request's cached detail, marked "no longer listed", when the service still holds one; otherwise a notice that the pull request is not in the provider's list |

For GitHub, the provider's list means both lists of its snapshot, authored and review-requested.

A provider's snapshot reads `disabled` from startup until its first poll completes, so pending and provider off SHALL be told apart by the provider's enabled flag, never by its snapshot's status: an enabled provider whose snapshot still reads `disabled` is pending. Pending SHALL cover only a flag or snapshot not yet read and a first poll that is running. A provider re-enabled while its poller waits out a backoff deadline publishes an unavailable snapshot at once (see the *GitHub Polling With Caching and Backoff* requirement in the `github-pull-requests` capability and the *Polling With Caching and Backoff* requirement in the `bitbucket-pull-requests` capability), so its addresses SHALL resolve as unavailable, never as pending for as long as the deadline holds.

A row whose web URL is empty, such as a GitHub row whose link is foreign (see the *GitHub Row Signals* requirement in the `github-pull-requests` capability), SHALL never resolve an Address, so a pull request that cannot be opened from its row cannot be opened through a hand-made address either. Resolution itself SHALL build no URL from an Address and SHALL send no request to either provider, so a pull request that no list holds is never fetched; only a listed pull request's detail is ever requested from its provider (see the *Detail Reads Are Scoped to the Snapshot* requirement in the `pull-request-viewer` capability). Resolution SHALL never be ambiguous: a pull-request Address never presents candidates to choose between (see the *Shortest Unambiguous Address* requirement).

**Canonical spelling.** When the owner or the repository of a pull-request Address differs from the matched row's only in ASCII case, the Address SHALL be replaced in place with the row's spelling, replacing the current history entry rather than adding one (see the *History Entry Discipline* requirement). Every launch of a listed pull request — a click on a panel row or a header chip, a click with the platform's new-window modifier (Cmd on macOS, Ctrl elsewhere), and the view's "Open in its own window" control — SHALL encode its Address with the matched row's spelling, so one pull request has one address, and one pull-request window, however a link spelled it. For a pull request the view shows from its cached detail after it left its provider's list, the view's "Open in its own window" control SHALL encode its Address with the spelling of the row that detail was read through (see the *Pull-Request Window* requirement in the `pull-request-viewer` capability).

**History.** Opening a pull request in the center pane, from a panel row, a header chip or a link, SHALL add a history entry, as opening an artifact does. Opening one in its own window SHALL add no history entry and SHALL leave the launching view as it was. Nothing that only changes what the pull-request view shows SHALL add a history entry: not switching between its files, not switching the diff layout, not marking files viewed, and not refreshing. A control in the center-pane view that leaves the pull request, such as its linked change's name (see the *Linked Change in the Pull-Request View* requirement in the `pull-request-viewer` capability) or a pointer to the settings view, SHALL navigate as any navigation does and add a history entry, so a back gesture returns to the pull request. Back and Forward SHALL resolve a pull-request Address again against the flags and snapshots as they then are, and a reload SHALL resolve it cold (see the *Cold-Load Address Resolution* requirement).

#### Scenario: A pull-request address round-trips

- **WHEN** the Address of GitHub pull request 42 in `acme/api` and the Address of BitBucket pull request 7 in the workspace `acme` and the repository `api` are encoded
- **THEN** the paths are `/pr/github/acme/api/42` and `/pr/bitbucket/acme/api/7`
- **AND** decoding each path yields an Address equal to the original

#### Scenario: A name needing escapes survives

- **WHEN** the Address of GitHub pull request 3 in the owner `acme` and the repository `a b` is encoded
- **THEN** the repository's segment is `a%20b`
- **AND** decoding the path recovers the repository `a b` unchanged

#### Scenario: The codec decodes a pull-request address with no data

- **WHEN** `/pr/github/acme/api/42` is decoded with no provider snapshot, no enabled flag and no registered workspace available
- **THEN** decoding yields the provider GitHub, the owner `acme`, the repository `api` and the number 42
- **AND** no command is dispatched to the backend

#### Scenario: A malformed pull-request path does not open a view

- **WHEN** `/pr/gitlab/acme/api/42`, `/pr/github/acme/api`, `/pr/github/acme/api/42/files`, `/pr/github/acme/api/forty-two`, `/pr/github/acme/api/042`, `/pr/github/acme/api/+42` or `/pr/github/acme/api/99999999999999999999` is decoded
- **THEN** each result is an unresolvable outcome, not a partial Address
- **AND** no pull-request view is rendered from it

#### Scenario: References compare names ignoring case

- **WHEN** GitHub is enabled, its snapshot lists pull request 42 of `Acme/API` with a web URL, and the address `/pr/github/acme/api/42` is resolved
- **THEN** the outcome is listed, matched to that row
- **AND** the address `/pr/bitbucket/acme/api/42` is not matched to that row, because the providers differ

#### Scenario: A case variant takes the row's spelling

- **WHEN** the served web UI is loaded at `/pr/github/acme/api/42` and the matched GitHub row spells the repository `Acme/API`
- **THEN** the browser's location becomes `/pr/github/Acme/API/42`
- **AND** the current history entry is replaced rather than a new one added

#### Scenario: Pending is told from provider off by the flag

- **WHEN** a GitHub pull-request address is resolved after GitHub's configuration and snapshot have both been read, and the snapshot still reads `disabled`
- **THEN** the outcome is pending if GitHub's configuration says it is enabled
- **AND** the outcome is provider off if it says it is not enabled

#### Scenario: A provider waiting out a deadline resolves as unavailable

- **WHEN** BitBucket is re-enabled while its backoff deadline still holds, and the center pane shows the address of one of its pull requests
- **THEN** the address resolves as unavailable at once
- **AND** "Loading…" is not shown while the deadline holds

#### Scenario: A stale list still resolves

- **WHEN** GitHub is enabled and its snapshot is stale but still holds the addressed pull request's row with its web URL
- **THEN** the outcome is listed

#### Scenario: A row without a URL never resolves

- **WHEN** GitHub is enabled and its snapshot holds a row whose reference equals the address's but whose web URL is empty, because its link is foreign
- **THEN** the outcome is not listed
- **AND** no request is sent to GitHub for that pull request

#### Scenario: A pull-request address is never ambiguous

- **WHEN** a pull-request address is resolved under any combination of its provider's enabled flag and snapshot state
- **THEN** exactly one of pending, provider off, unavailable, listed and not listed is reached
- **AND** no candidates are presented for the user to choose between

#### Scenario: Switching the provider off re-resolves a shown address

- **WHEN** a GitHub pull request is shown in the center pane and GitHub is switched off from another window or browser tab of the same service
- **THEN** the center pane replaces the pull request with the provider-off notice
- **AND** no reload or navigation is needed

#### Scenario: Opening a pull request adds a history entry

- **WHEN** the user clicks a pull-request row while an artifact is shown, and then issues a back gesture
- **THEN** the center pane first shows the pull request at its address
- **AND** the back gesture renders the artifact again

#### Scenario: Opening a pull request in its own window adds no history

- **WHEN** the user Cmd-clicks a pull-request row on macOS, or Ctrl-clicks one elsewhere, while an artifact is shown
- **THEN** no history entry is added
- **AND** the center pane still shows the artifact

#### Scenario: Changing what the view shows adds no history

- **WHEN** the user, with a pull request shown in the center pane, switches between its files, switches the diff layout, marks a file viewed, refreshes, and then issues a back gesture
- **THEN** none of those interactions created a history entry
- **AND** the back gesture renders the view shown before the pull request was opened

#### Scenario: Leaving the pull request from its view adds history

- **WHEN** the user, with a pull request shown in the center pane, activates the name of its linked change in the view's header, and then issues a back gesture
- **THEN** the center pane first shows that change's default artifact
- **AND** the back gesture shows the pull request again, at its address

#### Scenario: Back resolves a pull-request address again

- **WHEN** the user opens a pull request, then opens an artifact, and the pull request's provider is switched off from another browser tab before the user issues a back gesture
- **THEN** the back gesture shows the provider-off notice at the pull request's address
- **AND** it does not show the pull request as it was last rendered

#### Scenario: A reload resolves the address cold

- **WHEN** the served web UI shows a pull request and the page is reloaded
- **THEN** the browser's location keeps the pull request's address
- **AND** the address is resolved again from pending, showing "Loading…" rather than the home surface until its outcome is reached

