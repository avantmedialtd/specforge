## MODIFIED Requirements

### Requirement: Pull-Request View

The **pull-request view** SHALL render one pull request from the detail the application service reads for it (see *Detail Reads Are Scoped to the Snapshot*), and SHALL render its read content the same way in two presentations, in the desktop application and in the browser skin alike:

- **the center pane** of the main window, at the pull request's address (see the *Pull-Request Addresses* requirement in the `view-routing` capability), which is where a pull request opens by default, as an artifact does;
- **the pull-request window**, which detaches it (see *Pull-Request Window*).

**Content.** The view SHALL show:

- **a header** naming the pull request by its number, title and repository, with its head and base branch names, and carrying the signals its row shows in its provider's pull-request panel (see the *GitHub Pull-Request Panel* requirement in the `github-pull-requests` capability and the *Pull-Request Panel* requirement in the `bitbucket-pull-requests` capability), expressed as readable actionable summaries under the *Actionable PR Status* requirement in the `pr-review-workspace` capability;
- **the description**, rendered as untrusted content (see *Pull-Request Content Is Untrusted*), in Overview;
- **the conversation**, in Discussion: the pull request's own comments and every submitted review summary that has a body, as one list in submission order. A review without a body SHALL add no entry. A comment or review that GitHub reports as minimised SHALL render collapsed behind GitHub's stated reason;
- **the checks**, in Overview, each listed by its name and its state, with a link out to its page when its provider gives one. On either host a check SHALL offer a link only for an absolute `http` or `https` URL with a host; a check whose URL is anything else SHALL show no link. The link SHALL open as a link in pull-request content does (see *Desktop Link Opener*): in the desktop application only through `open_pull_request_link`, and in the browser skin in a new opener-isolated tab;
- **the changed files**, in Files (see *Changed Files in the Pull-Request View*).

The working views, counts and resumption SHALL follow the *PR Working Views and Resumption* requirement in the `pr-review-workspace` capability. A detail the service reports as no longer listed SHALL be shown marked "no longer listed". The view SHALL offer a manual refresh, which asks for a read under the freshness rule of *Detail Reads Are Scoped to the Snapshot*.

**The provider's page.** The header SHALL carry an "Open on GitHub" or "Open on BitBucket" control, naming the pull request's provider, that opens the pull request's web page outside SpecForge:

- in the desktop application, through `open_pull_request` while the pull request is listed, and once it is no longer listed, through `open_pull_request_link` with the pull request's own URL from its cached detail (see *Desktop Link Opener*);
- in the browser skin, as a link that opens the pull request's URL in a new opener-isolated tab.

**The pop-out control.** In the center pane only, the header SHALL also carry the shared "Open in its own window" control. Activating it SHALL open the pull-request window exactly as the new-window gesture does (see *Opening a Pull Request Like a Document*), and SHALL leave the center pane's contents, reading position, selection and history unchanged. The pull-request window SHALL carry the provider control and no pop-out control, since its pull request is already detached. On a device that reports no hover capability both controls SHALL be visible at rest, and on a device whose primary pointer is coarse the pop-out control SHALL keep the enlarged hit area the shared control presents (see the *Essential Controls Are Discoverable Without Hover* and *Interactive Targets Meet a Minimum Size on Coarse Pointers* requirements in the `touch-input` capability).

**Clearance.** In the native desktop main window on macOS, the header SHALL keep its controls clear of the titlebar drag region at every scroll position, leaving the region itself draggable, as the change header does (see the *Change Identity Header in the Detail Pane* requirement in the `spec-browser` capability). The pull-request window has a native titlebar and SHALL take no such inset, and neither SHALL the served web UI.

**Review actions.** In the native desktop application, both presentations SHALL offer the same draft and explicitly confirmed review controls when current capabilities allow them (see the *Desktop-Only Pull-Request Reviews* requirement in the `pull-request-actions` capability). A persistent Finish review control SHALL open the review sheet holding pending drafts, summary, decision and Submit, as specified by the *Persistent Finish-Review Sheet* requirement in the `pr-review-workspace` capability, rather than placing the composer above changed files. In the browser skin the view SHALL offer no provider-write control. No presentation SHALL offer a merge, decline, close, thread-resolution or thread-reopening control. Marking a file or hunk viewed SHALL remain local to this machine (see *Review Progress*).

#### Scenario: One view in both presentations

- **WHEN** the same pull request is shown in the center pane and in its pull-request window
- **THEN** both offer its header, description, conversation, checks and changed files in the same working views, from the same detail

#### Scenario: The header carries the row's signals

- **WHEN** a GitHub pull request's panel row shows failing checks and a conflict marker
- **THEN** the view's header conveys the same failing-check and conflict signals with readable actionable status

#### Scenario: The conversation follows submission order

- **WHEN** a pull request has a comment posted at 10:00, a review submitted at 10:05 with the body "Looks close", and a comment posted at 10:10
- **THEN** the conversation in Discussion lists the first comment, the review summary and the second comment, in that order

#### Scenario: A review without a body adds nothing to the conversation

- **WHEN** a reviewer approves the pull request without writing a body
- **THEN** no entry for that review appears in the conversation

#### Scenario: A minimised comment stays collapsed

- **WHEN** a comment in the conversation is minimised on GitHub with the reason "outdated"
- **THEN** it renders collapsed, showing that reason in place of its body

#### Scenario: Checks link out

- **WHEN** a pull request has a check named `build` that failed and whose provider gives it a details page
- **THEN** Overview checks show its name and its failed state
- **AND** activating its link opens that page outside SpecForge without navigating the view

#### Scenario: A check link with another scheme opens nothing

- **WHEN** a check's details URL uses a `file:` or `data:` URL or a custom scheme
- **THEN** the check shows no link and nothing is opened

#### Scenario: The provider's page opens from the header

- **WHEN** the user activates "Open on GitHub" in the desktop application for a listed pull request
- **THEN** the page opens through `open_pull_request` in the system browser
- **AND** the SpecForge window does not navigate

#### Scenario: A pull request no longer listed still opens its page

- **WHEN** the user activates the provider control for a cached pull request marked "no longer listed"
- **THEN** the desktop opens the page through `open_pull_request_link` with its cached detail URL

#### Scenario: The provider's page in the browser skin

- **WHEN** the user activates "Open on BitBucket" in the browser skin
- **THEN** the page opens in a new tab with `noopener noreferrer` semantics
- **AND** the SpecForge page does not navigate

#### Scenario: Only the center pane offers the pop-out control

- **WHEN** a pull request is shown in the center pane and in its pull-request window
- **THEN** the center pane's header carries "Open in its own window"
- **AND** the pull-request window carries the provider control and no pop-out control

#### Scenario: The pop-out control disturbs nothing

- **WHEN** the user activates "Open in its own window" while the center pane is scrolled within Files
- **THEN** the pull-request window opens, or is focused if it is already open
- **AND** the center pane's contents, reading position, selection and history are unchanged

#### Scenario: The controls are reachable without hover

- **WHEN** the view is displayed on a device that reports no hover capability
- **THEN** the provider control, and in the center pane "Open in its own window", are visible at rest

#### Scenario: The header clears the macOS titlebar

- **WHEN** the center pane shows a pull request in the native window on macOS, scrolled to any position
- **THEN** the header's controls lie clear of the titlebar drag region and respond to a click
- **AND** the drag region remains draggable

#### Scenario: Review controls follow the host boundary

- **WHEN** the same actionable pull request is shown in the native desktop and the browser skin
- **THEN** the native view offers Finish review and its composer sheet and the browser view does not
- **AND** neither view offers merge, decline, close, thread-resolution or thread-reopening controls

#### Scenario: Nothing can be posted

- **WHEN** the view shows a pull request in the browser skin
- **THEN** it offers no control that comments, approves, requests changes, merges or marks anything on either host
- **AND** in the native desktop nothing is posted to either host except a review action the reviewer explicitly confirms, and marking a file or hunk viewed stays local to this machine
