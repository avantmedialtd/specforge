## ADDED Requirements

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

## MODIFIED Requirements

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
