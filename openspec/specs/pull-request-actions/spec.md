# pull-request-actions Specification

## Purpose
Lets a reviewer prepare and submit a pull-request review — a summary, single-line inline comments, replies to existing threads and a Comment, Approve or Request changes decision — to GitHub or BitBucket from the native desktop application only. A durable draft holds the review until one exact revision is frozen by an operating-system confirmation; its submission runs as a journalled operation that writes each step once, reconciles any write it cannot prove, and resumes or abandons only on the reviewer's explicit action. Every boundary re-checks the current credential, actor, listing and commits, and the browser skin and terminal stay unable to write to a provider.

## Requirements
### Requirement: Desktop-Only Pull-Request Reviews

The native desktop application SHALL let a reviewer prepare and submit a pull-request review. One review SHALL contain:

- an optional summary;
- zero or more comments on one current diff line;
- zero or more replies to existing review threads; and
- one explicit decision: Comment, Approve or Request changes, subject to the current pull request's capabilities.

The center-pane and detached-window presentations of a pull request SHALL offer the same review actions. The browser skin and terminal frontend SHALL offer no review composer and no operation that writes to a provider. SpecForge SHALL expose no review action that merges, closes or declines a pull request, resolves or reopens a thread, creates a multiline range or first-class structured suggestion item, or changes repository content or git state. User-authored comment Markdown remains opaque prose and SHALL NOT be rewritten or censored merely because a provider may render some Markdown sequence specially.

#### Scenario: Both native pull-request presentations can review

- **WHEN** an actionable pull request is shown in the native main window and in its native pull-request window
- **THEN** both presentations offer the same draft and review controls
- **AND** an edit in either presentation reaches the other through the shared application service

#### Scenario: Browser and terminal remain provider-read-only

- **WHEN** the same pull request is opened in the browser skin, or the terminal frontend is used
- **THEN** neither frontend offers a review composer or a provider-write operation

#### Scenario: No merge action exists

- **WHEN** every pull-request review action is inspected
- **THEN** none can merge, decline or close the pull request, change a branch, resolve or reopen a thread, or modify local work

### Requirement: Durable Review Drafts Use Optimistic Revisions

The application service SHALL persist review drafts and review-operation journals in its own `pull-request-actions.json` file in the shared configuration directory. This store SHALL be separate from the 32-entry pull-request detail LRU and from `review-progress.json`. Detail eviction, refresh and application restart SHALL NOT discard a draft or an unfinished operation.

Each draft and operation, and each draft item, SHALL have a stable backend-generated UUID. At most one current draft SHALL exist for one canonical pull request. Concurrent creation SHALL return that current draft rather than create competing drafts, and Start over SHALL atomically replace it. Each draft SHALL have a monotonically increasing revision and SHALL record its canonical pull-request reference, head and base commit basis, provider actor and credential fingerprint, summary, decision, ordered items and placement states. No credential, token fragment or reversible secret SHALL be stored in this file or returned through IPC. The non-secret fingerprint SHALL remain private to the persisted authorisation stamp and service and SHALL NOT be returned through IPC or an event or written to a log.

An operation's persisted phase SHALL be `running`, `attempting`, `reconciling`, `paused`, `submitted` or `abandoned`. The first three are live phases that require the execution lock, `paused` is unfinished and permits no further provider mutation without an explicit eligible Resume or Abandon, and only `submitted` and `abandoned` are terminal. A **partial result** SHALL be derived when a non-submitted operation's ledger or receipt contains confirmed review content or a decision that remains at the provider, or an uncertain write or cleanup that may remain. It SHALL NOT be a separate phase, treat a confirmed cleanup as a remaining effect, or imply that Resume is eligible.

Every draft mutation SHALL include the revision the caller read. Every read-modify-write SHALL take an inter-process lock across re-reading the action file, checking the expected revision or operation invariant, and atomically replacing the file. A mutation SHALL apply only when that expected revision is current and SHALL increment the revision exactly once. A revision conflict SHALL change nothing and SHALL return enough non-secret state for the caller to re-read. Before any journal transition can permit a provider mutation, the replacement file and its parent-directory entry SHALL cross the platform's durability barrier; a failed write or flush SHALL send no provider mutation.

Because the same file is also the idempotency journal, damaged input SHALL be preserved and SHALL block every draft or operation mutation as `actionStoreCorrupt`. The service SHALL NOT replace it with an empty model or send a provider mutation until the file has been repaired or explicitly removed outside the action API. Starting the browser server or terminal frontend SHALL neither open nor normalise this store; native action initialisation is the boundary that loads it.

Each operation SHALL also have an execution lock that serialises work within one process and between processes. A caller SHALL acquire it before first persisting the operation as `running`, before read-only provider reconciliation, and before Resume or Abandon, and SHALL hold it across that activity's provider requests and journal transitions. Every action-store transaction for such activity SHALL be nested inside the execution lock; no path SHALL acquire them in the opposite order. A settings lock SHALL never be acquired while an action-store lock is held: a current-state gate that needs durable credential state SHALL complete that read before the short action-store transaction, and every provider request SHALL repeat the credential gate after the journal transition that precedes it. A caller that cannot acquire the execution lock non-blockingly SHALL only expose the live in-flight phase and SHALL neither normalise, reconcile, resume nor abandon it. A caller MAY change a persisted `running`, `attempting` or `reconciling` phase to paused only while holding the execution lock, thereby proving that no other live runner owns the operation.

#### Scenario: A detail eviction keeps the draft

- **WHEN** a reviewer creates a draft, enough other pull requests are opened to evict its detail, and that pull request is opened again
- **THEN** the draft retains the same UUID, revision and content
- **AND** it cannot be edited until a current authorised detail is available

#### Scenario: A stale window cannot overwrite another window

- **WHEN** two native windows read revision 8, one stores an edit as revision 9, and the other submits an edit expecting revision 8
- **THEN** the second edit is refused as a revision conflict
- **AND** revision 9 and its content remain unchanged

#### Scenario: Restart keeps an unfinished operation

- **WHEN** SpecForge exits after an operation journal was persisted and before submission completed
- **THEN** the next native session can read that operation with the same operation id, frozen revision and ledger

#### Scenario: Persisted action state contains no credential

- **WHEN** `pull-request-actions.json`, an action event and the action-state IPC response are inspected
- **THEN** none contains a token, password, authorisation header, token prefix or reversible credential value
- **AND** only the backend action file may contain the non-secret credential fingerprint; the event and IPC response do not

#### Scenario: Corrupt action state fails closed

- **WHEN** native action initialisation finds a damaged `pull-request-actions.json`
- **THEN** the damaged file is preserved and review actions report `actionStoreCorrupt`
- **AND** no empty replacement, operation journal or provider request is created

#### Scenario: Concurrent mutations are one transaction at a time

- **WHEN** two desktop processes try to update action state from the same on-disk revision
- **THEN** the inter-process lock lets only one transaction commit from that revision
- **AND** the other re-reads the winner before it can write

#### Scenario: Concurrent creation yields one current draft

- **WHEN** two desktop processes create a draft for the same canonical pull request
- **THEN** one store transaction creates it and the other returns that same current draft
- **AND** the store contains no competing second draft for that pull request

#### Scenario: A live runner is not mistaken for crash residue

- **WHEN** one desktop process holds an operation's execution lock while another process opens native action state
- **THEN** the second process leaves the operation in its live phase and cannot resume it
- **AND** it cannot reconcile or abandon the operation
- **AND** only a later opener that acquires the released lock may normalise crash residue to paused

#### Scenario: A failed durability barrier sends nothing

- **WHEN** persisting an `attempting` transition cannot durably flush its replacement
- **THEN** the operation reports a local journal failure and sends no provider mutation

### Requirement: Draft Items and Decisions Are Explicit and Valid

A present summary and every inline-comment or reply body SHALL be stored exactly as the reviewer entered it and SHALL be rejected when it is empty after whitespace is ignored. Each body SHALL be at most 30 KiB (30 × 1024 bytes) when UTF-8 encoded, excluding SpecForge's temporary reconciliation marker, and one draft SHALL contain at most 100 comment/reply items in total. An inline comment SHALL identify exactly one current diff-file identity (its new path, or its old path when deleted), old/new side and current line in the draft's cached commit basis. A reply SHALL identify an existing provider thread/comment that the current detail reports as replyable. This slice SHALL accept no start line, multiline range or structured suggestion item; bodies remain opaque Markdown as *Desktop-Only Pull-Request Reviews* requires.

The service SHALL return provider-neutral capabilities for Comment, Approve, Request changes, inline comment and reply, each either available or unavailable with a reason, and SHALL validate the selected operation against them rather than trusting hidden or disabled controls. Comment SHALL require at least one non-empty summary or item. Approve SHALL permit an otherwise empty review. Request changes SHALL require at least one non-empty summary or item.

For current GitHub discovery, a review-requested pull request whose authenticated actor is not the author SHALL expose Comment, Approve and Request changes, while a pull request authored by the authenticated actor SHALL expose Comment but SHALL NOT expose self-approval or self-request-changes. A provider permission rejection SHALL stop submission as the provider failure boundary requires; capability visibility SHALL NOT turn a failed write into a retry. For current BitBucket discovery, which lists only the authenticated actor's authored pull requests, Comment SHALL be the only exposed decision. BitBucket Approve and Request changes SHALL remain unavailable until reviewer discovery supplies a row that authorises them.

#### Scenario: A comment is anchored to one current line

- **WHEN** the reviewer adds a comment to new line 27 of `src/api.ts`
- **THEN** the draft item records `src/api.ts`, the new side and line 27 under the current head/base basis
- **AND** it records no range or suggestion

#### Scenario: Whitespace is not review content

- **WHEN** a Comment draft has no items and a summary containing only whitespace
- **THEN** it cannot be frozen or submitted

#### Scenario: Draft size is bounded before persistence

- **WHEN** a caller supplies a body beyond 30 KiB or tries to add a 101st comment/reply item
- **THEN** the draft mutation is refused without changing its content or revision
- **AND** no oversized value reaches an operation journal or provider request

#### Scenario: An empty approval is valid

- **WHEN** Approve is available and the draft has no summary or item
- **THEN** the draft can proceed to confirmation as an approval without a body

#### Scenario: An authored GitHub pull request cannot be self-approved

- **WHEN** the authenticated GitHub actor is the pull request's author
- **THEN** Comment is available
- **AND** Approve and Request changes are unavailable before confirmation and through direct command invocation alike

#### Scenario: Authored-only BitBucket discovery exposes Comment

- **WHEN** an actionable BitBucket row comes from the current authored-pull-request snapshot
- **THEN** Comment is the only available decision
- **AND** the implemented approval and request-changes protocols remain capability-gated

### Requirement: Current State Authorises Every Draft and Submission Boundary

The service SHALL authorise draft creation, explicit stale recovery, Start over, every ordinary draft edit, freezing, initial submission, resumption and provider cleanup against current state. Every boundary SHALL require all of the following:

- fresh credential resolution yields an environment or operating-system credential, not a legacy plaintext settings credential;
- the resolved credential has a stable non-secret fingerprint;
- the provider is enabled and its current non-stale snapshot was read with that fingerprint, supplies a stable authenticated actor id, and contains the canonical pull-request reference and trusted remote identifiers;
- the current detail cache entry was read under the current credential generation and fingerprint and supplies the pull request's current head and base commits; and
- the requested decision and item types are currently available capabilities.

Draft creation SHALL record the current commits, fingerprint and actor in a new authorisation stamp. Start over SHALL do the same without comparing or copying the fenced draft's old stamp. Explicit stale recovery SHALL require the current fingerprint and actor to equal the draft's recorded identity, SHALL permit only the commits to differ, and SHALL then record the current commits in the new revision. Every ordinary edit, freeze, initial submission, resumption and provider cleanup SHALL require all three current values to equal the recorded stamp.

Immediately before every provider request, including a later request in the same run, the service SHALL freshly resolve the credential and re-check the enabled flag, credential generation, current snapshot membership and trusted identifiers, snapshot actor, and snapshot/resolved fingerprint against the operation stamp. This check SHALL observe a credential replacement or clear committed by another desktop process rather than rely only on this process's in-memory generation. The provider-specific protocol SHALL additionally perform its required commit and foreign-content checks. After restart, the stable credential fingerprint and actor SHALL be checked before the new process generation is accepted for a resume. Failure of any check SHALL send no provider mutation and SHALL return a structured blocked reason. A display login or username alone SHALL NOT substitute for a stable actor id.

A fenced draft SHALL remain readable but SHALL NOT adopt a new credential or actor. The desktop SHALL offer an explicit local **Start over** action only when the canonical pull request has no unfinished operation. Start over SHALL run the fresh draft-creation checks above, discard the fenced draft and create a fresh empty Comment draft with a new UUID, revision 1, current commit basis and current authorisation stamp. It SHALL copy no summary, item, decision or provider-effect state and SHALL send no provider mutation. If an unfinished operation exists, the reviewer SHALL first explicitly abandon it; its terminal receipt SHALL remain available for inspecting every confirmed or uncertain provider effect before anything is re-authored.

#### Scenario: A pull request leaving the snapshot blocks an edit

- **WHEN** a draft exists and the pull request leaves the provider's current snapshot
- **THEN** an edit or submission is refused as no longer listed
- **AND** no provider mutation is sent

#### Scenario: A saved credential fences the old draft

- **WHEN** a draft was authorised by one credential and the resolved credential changes, even to one for the same displayed login
- **THEN** the credential generation or fingerprint check blocks every further edit and submission of that draft

#### Scenario: Starting over transfers no old authority or content

- **WHEN** a replacement credential is current, the prior operation is terminal, and the reviewer activates Start over
- **THEN** the service creates a fresh empty draft under the current actor, credential and commit basis
- **AND** it copies no frozen or previously attempted content and sends no provider mutation

#### Scenario: An old snapshot cannot stamp a replacement credential

- **WHEN** the resolved credential changed but the provider snapshot and cached detail were read with the former fingerprint
- **THEN** draft creation and Start over require fresh provider state
- **AND** neither combines the replacement credential with the former credential's actor or pull-request row

#### Scenario: A different actor cannot resume after restart

- **WHEN** an interrupted operation was confirmed by actor A and the next session resolves a credential for actor B
- **THEN** the operation remains blocked as actor-changed
- **AND** no provider reconciliation request, cleanup or remaining review item is sent

#### Scenario: Legacy plaintext can read but cannot review

- **WHEN** the active credential came only from the legacy plaintext settings field
- **THEN** provider snapshots and pull-request details remain readable
- **AND** draft creation, editing and submission are refused until the credential is explicitly re-saved securely

#### Scenario: A stale detail cannot authorise an anchor

- **WHEN** the snapshot is current but no cached detail under the current generation establishes the pull request's head and base commits
- **THEN** an inline draft edit is refused as requiring a current detail
- **AND** no guessed commit or line is accepted

### Requirement: Native Confirmation Freezes One Exact Draft Revision

The initial submission path SHALL exist only as a native Tauri command. It SHALL validate and freeze the caller's expected draft revision as a private native value before showing an operating-system confirmation. The confirmation SHALL name the provider, repository, pull-request number, decision, whether a summary is present, the number of inline comments and replies, and that edits made to the draft after it opened will not be included. It SHALL NOT show the revision number, which belongs to the operation journal rather than the reviewer.

Cancelling or dismissing the confirmation SHALL create no operation journal and send no provider mutation. Accepting it SHALL allocate the operation id and acquire that operation's execution lock first. While holding it, acceptance SHALL repeat current authorisation, releasing any settings lock before it takes the action-store lock. It SHALL then atomically verify that the current draft still has the frozen draft's UUID, that the frozen revision has not already been confirmed, and that the canonical pull request has no other unfinished operation. A later revision of that same draft UUID SHALL not invalidate the frozen value, but a Start over replacement SHALL. Acceptance SHALL durably persist an operation in its `running` phase containing the complete frozen revision before the first provider mutation; the mandatory pre-request gate SHALL catch a credential or provider-state change that occurs after acceptance's authorisation read. At most one unfinished operation SHALL exist for a canonical pull request, and a draft revision SHALL belong to at most one operation. Every mutation in that operation SHALL use the frozen content and order; a later draft revision SHALL NOT alter it. Draft edits MAY continue while the frozen operation runs, but submitting their newer revision SHALL wait until the older operation is submitted or abandoned and SHALL require a new freeze and confirmation.

An interrupted operation that was already confirmed SHALL require an explicit native Resume action before another mutation. Resume SHALL re-authorise the unchanged frozen operation but SHALL NOT silently substitute the current draft or create a new confirmation for different content.

#### Scenario: Cancelling writes nothing

- **WHEN** the reviewer reaches native confirmation and cancels it
- **THEN** no operation journal is created
- **AND** no provider mutation is sent

#### Scenario: A later edit cannot change confirmed content

- **WHEN** revision 12 is frozen and confirmed, and another window then creates revision 13
- **THEN** the operation submits exactly revision 12
- **AND** revision 13 can be submitted only as a separately confirmed operation

#### Scenario: Journal precedes the first mutation

- **WHEN** the reviewer confirms a review and the process exits immediately before the first provider request
- **THEN** the next session finds the confirmed frozen operation
- **AND** no provider content was posted without a recoverable journal

#### Scenario: Two confirmations cannot submit the same revision

- **WHEN** two native windows open confirmation for revision 12 and both accept
- **THEN** exactly one acceptance persists an operation and may begin provider work
- **AND** the other is refused before any second journal or provider mutation is created

#### Scenario: Start over invalidates an older open confirmation

- **WHEN** confirmation is open for one draft UUID and another window replaces that draft through Start over before acceptance
- **THEN** accepting the old confirmation is refused before an operation is persisted
- **AND** only the fresh empty draft remains current

#### Scenario: One unfinished operation fences the next revision

- **WHEN** revision 12 has an unfinished operation and another window edits the draft to revision 13
- **THEN** revision 13 remains editable but cannot start another operation yet
- **AND** it can be confirmed only after the revision-12 operation is submitted or abandoned

### Requirement: GitHub Submission Owns a Commit-Pinned Pending Review

A GitHub operation SHALL submit through one pending review owned by the recorded actor, pinned to the frozen head commit and identifiable by an invisible operation marker. Before creating or adopting one, and again before changing, submitting or deleting it, the service SHALL read pending-review state. It SHALL adopt only a pending review whose actor and marker match the operation and whose body, comments and replies consist solely of the exact marker-bearing or already-reconciled provider objects named by that operation's ledger. It SHALL NOT adopt, submit, edit or delete an unrelated pending review. An unrecognised body edit, comment or reply added through github.com or another client after SpecForge created the review SHALL make that review foreign and pause the operation without cleanup or submission.

The service SHALL add frozen inline comments and replies in their recorded order. While pending, the review body SHALL carry an invisible operation marker. Each provisional comment or reply body SHALL carry an invisible marker containing its operation id and item id until that item has been reconciled and cleaned. The operation journal SHALL record the returned provider id for every confirmed remote object.

After every mutation, and after every transport error or otherwise ambiguous response, the service SHALL read provider state. One reconciliation pass SHALL read at most ten provider pages in total across all relevant pending-review connections, requesting at most 100 objects per page, and at most 10 MiB for any one response body. It SHALL retry an item only after a complete in-bound read proves the matching marker absent; a marker found exactly once SHALL confirm the item without repeating it, and a duplicate marker, reached bound or otherwise inconclusive read SHALL pause the operation. GraphQL `clientMutationId` alone SHALL NOT be treated as deduplication.

Once all content items are confirmed, the service SHALL remove and reconcile each item marker from comments and replies while retaining the review-level operation marker for as long as the review is pending. It SHALL submit that same review with the clean summary and selected Comment, Approve or Request-changes event by using the submit mutation's body to replace the final operation marker atomically with submission. An ambiguous final response SHALL be reconciled from that review's stable id and state.

Explicit abandonment SHALL be permitted to delete only the pending review carrying this operation's marker, containing no foreign content, after current authorisation succeeds. Before that deletion, the journal SHALL durably record cleanup as attempting. A response or complete in-bound reconciliation read that conclusively reports the stable review id absent SHALL confirm cleanup without another deletion; bound exhaustion SHALL remain uncertain. An ambiguous or failed cleanup that cannot be reconciled SHALL still end in local abandonment, retain the pending-review reference and the failed or uncertain cleanup outcome in the receipt, and report that provider-side handling may remain; it SHALL NOT blindly repeat the deletion. When deletion is unsafe because authorisation no longer matches or foreign content exists, Abandon SHALL likewise perform only the local terminal transition and SHALL NOT delete or edit that review.

#### Scenario: An unrelated pending review is untouched

- **WHEN** the actor already has a pending review on the pull request without this operation's marker
- **THEN** the SpecForge operation pauses before adding any content
- **AND** that review is not adopted, edited, submitted or deleted

#### Scenario: The pending review is pinned to the confirmed head

- **WHEN** the frozen draft names head commit `abc123` and GitHub submission begins
- **THEN** the pending review is created for `abc123`
- **AND** no comment is silently attached to a later head

#### Scenario: A lost create response is reconciled before retry

- **WHEN** GitHub creates an inline comment but its response is lost
- **THEN** the next step reads pending-review content and finds the operation/item marker
- **AND** it records the existing comment id and does not create a duplicate

#### Scenario: Markers are absent from the submitted review

- **WHEN** a GitHub operation completes
- **THEN** its submitted summary, comments and replies contain none of SpecForge's operation/item markers
- **AND** the journal records the submitted review's stable id and final state

#### Scenario: The operation marker survives until atomic submission

- **WHEN** every item marker has been cleaned and the process exits before the submit mutation
- **THEN** the pending review still carries the operation marker and can be identified after restart
- **AND** only the submit mutation may replace it with the clean summary

#### Scenario: Content added elsewhere makes the owned review foreign

- **WHEN** SpecForge created its marked pending review and the actor then adds another pending comment through github.com
- **THEN** the next provider read pauses the operation without editing, submitting or deleting that review
- **AND** abandonment does not delete the actor's added content

#### Scenario: Ambiguous cleanup remains visible after local abandonment

- **WHEN** an authorised deletion of the exact owned pending review has an ambiguous result that bounded reconciliation cannot decide
- **THEN** the operation becomes locally abandoned without blindly repeating the deletion
- **AND** its receipt retains the pending-review reference, uncertain cleanup and manual-handling guidance

### Requirement: BitBucket Submission Is Sequential and Durably Partial

A BitBucket operation SHALL perform immediate writes one at a time in deterministic order: inline comments and replies in frozen draft order, then a non-empty summary as a general comment, then the selected provider decision. Approve or Request changes SHALL always be the final write. Comment SHALL consist only of the content writes. Because BitBucket accepts no expected-head condition on these endpoints, immediately before every mutation the operation SHALL read the pull request and require its source and destination commits to equal the frozen basis; after every successful or ambiguous mutation result it SHALL read them again before continuing or permitting a retry. A changed commit SHALL stop the operation as stale. Any mutation that raced the change SHALL remain recorded as a confirmed or uncertain partial effect and SHALL never be rolled back or silently attached to another draft item.

Before each content mutation, the journal SHALL durably record that item as attempting. A comment or reply body SHALL carry an invisible operation/item marker wherever BitBucket preserves it. A successful reply SHALL be recorded with its provider comment id. After an ambiguous result, the service SHALL read at most ten pages of 100 comments and at most 10 MiB for any one response body. It SHALL treat one matching marker as success, absence after a complete in-bound read as eligible for retry only after explicit Resume, and a duplicate marker, reached bound or otherwise inconclusive result as paused. It SHALL NOT blindly repeat a POST.

Before attempting or retrying a markerless approval or request-changes endpoint, the service SHALL read the recorded actor's participant state. The matching state SHALL confirm success without another POST; a state that proves the decision absent SHALL be attempted only after explicit Resume; ambiguous state SHALL cause no mutation.

Every confirmed write SHALL remain confirmed. A later failure SHALL produce a durable partial result listing succeeded, unresolved and failed items; Resume SHALL skip succeeded items, and Abandon SHALL not claim to undo visible BitBucket content.

#### Scenario: Decision is applied last

- **WHEN** a BitBucket operation contains two inline comments, one reply, a summary and an available Request-changes decision
- **THEN** the three items are attempted in draft order, followed by the summary
- **AND** Request changes is attempted only after all four content writes are confirmed

#### Scenario: A middle failure preserves earlier success

- **WHEN** the first of three immediate comments succeeds and the second reaches a run-stopping failure
- **THEN** the journal records the first comment's provider id and reports a partial result
- **AND** neither the third comment nor the decision is attempted in that run

#### Scenario: Resume never repeats a confirmed comment

- **WHEN** the reviewer resumes a partial operation whose first comment is confirmed and whose next step is eligible for retry
- **THEN** the first comment is skipped
- **AND** work starts at the first unresolved ledger entry after current authorisation succeeds

#### Scenario: An ambiguous approval is read before retry

- **WHEN** the approval request's response is lost
- **THEN** the service reads the recorded actor's participant state before any further approval POST
- **AND** a state already showing approval completes that ledger step without repeating the POST

#### Scenario: A push stops the next BitBucket write

- **WHEN** one immediate comment is confirmed and the pull request's source commit changes before the next item
- **THEN** the commit read before that item stops the operation as stale and sends no next mutation
- **AND** the first comment remains listed as a partial provider effect

### Requirement: Interrupted Operations Reconcile Without Automatic Mutation

An operation persisted as running, attempting or reconciling when the native action store is opened SHALL be exposed as paused only after the opener acquires its execution lock non-blockingly, proving no live process still owns the run. If the lock is held, the operation SHALL remain exposed in its live phase and that process SHALL not normalise, reconcile, resume or abandon it. The browser server and terminal frontend SHALL not open, normalise or request action state.

When native action state is requested and provider reconciliation is needed, the service SHALL first acquire and hold the operation's execution lock non-blockingly across the bounded provider reads and resulting journal transitions. Those reads SHALL require the provider still enabled, the canonical pull request still listed, the current snapshot to have been read under the freshly resolved environment/secure credential fingerprint, and that fingerprint and the snapshot actor to equal the operation stamp. They SHALL not require unchanged commits or decision capability, because their purpose includes classifying a write that raced a push or later capability change. They SHALL NOT trigger a provider mutation or move the operation into `running`.

Only an explicit desktop Resume or Abandon action SHALL permit another provider mutation. Either action SHALL acquire and hold the operation's execution lock non-blockingly, re-read the operation under the nested action-store lock, and release that store lock before any durable-credential authorisation read. Resume SHALL be available only when the first unresolved ledger step is eligible for another attempt; it SHALL repeat current authorisation, durably persist its transition before writing, skip every confirmed ledger entry and stop at the first unresolved step. When a read cannot prove whether an attempted write landed, the operation SHALL remain paused and SHALL NOT infer absence. Abandon SHALL preserve the receipt of every confirmed or uncertain provider effect. When provider cleanup is not currently authorised, its result is uncertain or GitHub content is foreign, Abandon SHALL still make the local terminal transition, SHALL identify the provider state that may require manual handling and SHALL send no further cleanup mutation.

After a completed submission, the service SHALL invalidate the affected cached detail, arrange an ordinary provider read, and publish action state as submitted. Submitted conversation and threads SHALL then return through the same pull-request detail model as provider content created outside SpecForge.

The durable transition that records the submission SHALL, in the same write, take what it posted out of the current draft when that draft still has the frozen draft's UUID: every item it holds exactly as frozen, the summary when it is still the frozen one, and the decision, reset to Comment, when it is still the frozen one. Content added or edited after the freeze SHALL stay for a later submission, and a draft that Start over put in place SHALL be left untouched. When the draft changed it SHALL take one new revision and its invalidation SHALL be published. The cached detail SHALL be invalidated before any window is told of the submission.

A native view that learns of a submission while open, in its own window or another, SHALL read the detail again under the freshness rule, and SHALL read action state again whenever a detail read lands, since action state answers from the cached detail that read replaces. Neither read SHALL send a provider mutation.

#### Scenario: Restart never resumes by itself

- **WHEN** SpecForge starts with a journal whose last phase was running
- **THEN** the operation is paused only after no live process holds its execution lock
- **AND** startup sends no provider mutation

#### Scenario: A fenced operation can be abandoned locally

- **WHEN** a replacement credential no longer authorises cleanup of a paused operation and the reviewer activates Abandon
- **THEN** the operation becomes locally abandoned with its confirmed and uncertain effects retained
- **AND** no provider mutation is sent and provider-side manual handling is reported

#### Scenario: Reconciliation confirms but does not continue

- **WHEN** a native window reads a paused operation and a provider read proves its attempting item already exists
- **THEN** that item becomes confirmed in the ledger
- **AND** the next mutation waits for the reviewer to activate Resume

#### Scenario: An uncertain effect is partial but not retryable

- **WHEN** bounded reconciliation cannot prove whether an attempted write landed after an earlier item was confirmed
- **THEN** the operation remains paused and exposes a partial result with the uncertain effect
- **AND** Resume is unavailable for that step while Abandon remains available

#### Scenario: Completion refreshes through the read model

- **WHEN** a review submission completes
- **THEN** its cached pull-request detail is invalidated and read again
- **AND** the submitted review content is not injected directly into the detail as an optimistic provider result

#### Scenario: Submitted content leaves the draft

- **WHEN** a draft holding one inline comment and a summary is submitted unchanged
- **THEN** the current draft keeps its UUID, takes the next revision and holds no item, no summary and the Comment decision
- **AND** a second Submit has nothing of the first review to post again

#### Scenario: Edits made during the run survive completion

- **WHEN** the reviewer edits one frozen comment and adds another while the frozen operation runs, and it then completes
- **THEN** the edited and the added comments remain in the draft
- **AND** the comments posted exactly as frozen do not

#### Scenario: An open view recovers its controls after completion

- **WHEN** a native view is open while a submission of its pull request completes
- **THEN** it reads the detail again and shows the submitted threads
- **AND** once that read lands its action state is no longer blocked for a current read

### Requirement: Review Operations Respect Provider Failure Boundaries

Every review-operation request SHALL use the provider integration's credential-safe HTTP transport, official host, no-redirect rule, shared rate-limit deadlines and existing per-provider in-flight gate. Immediately before admission it SHALL pass the enabled, current-row, trusted-identifier, credential-generation, fresh-fingerprint and actor checks in *Current State Authorises Every Draft and Submission Boundary*, including changes committed by another desktop process. Action requests SHALL NOT use an ambient proxy and SHALL NOT bypass a deadline set by the poller, a pull-request read or an earlier action reply. They SHALL be journalled separately from the pull-request detail hourly budget and SHALL NOT consume that hourly read budget; a review item SHALL not be refused merely because read budget is exhausted.

A rate-limited response SHALL set the applicable existing provider deadline, stop the current run and leave the next unresolved step paused until both the deadline has passed and the reviewer explicitly resumes. A credential or insufficient-permission response SHALL stop the run, report a non-retryable authorisation reason for the stamped credential and send no later mutation. Repeating the operation with that credential SHALL be unavailable. Saving a different credential SHALL fence the frozen operation under *Current State Authorises Every Draft and Submission Boundary* rather than silently transferring its authority; continuing under the new credential requires locally abandoning the old operation, explicitly starting over with a fresh empty draft, re-authoring any desired content after inspecting the retained receipt, and confirming that new draft. A stale commit or invalid anchor response SHALL stop the run as stale so recovery never retries it against another line. Transport failures and malformed success bodies SHALL be ambiguous until the provider-specific reconciliation read decides them. Provider reconciliation reads SHALL use the ten-page/100-object and 10-MiB response bounds specified by the provider protocols; exhausting one SHALL remain uncertain rather than prove absence. Raw response bodies and secrets SHALL NOT be persisted in the journal.

#### Scenario: A shared deadline blocks an action request

- **WHEN** GitHub's applicable GraphQL deadline or BitBucket's provider deadline still holds and the reviewer activates Resume
- **THEN** no action request is sent before the deadline
- **AND** the operation remains paused with the local time at which it can be resumed

#### Scenario: A rate limit stops later items

- **WHEN** a provider rate-limits the second item of a multi-item operation
- **THEN** the shared provider deadline is set and no third item is attempted
- **AND** the second item is reconciled before any later explicit resume can retry it

#### Scenario: A permission failure is not retried

- **WHEN** the provider refuses a review mutation because the active token lacks review-write permission
- **THEN** the operation reports the permission failure and sends no later mutation
- **AND** replacing the credential does not transfer the frozen operation; the reviewer must locally abandon it, Start over with a fresh empty draft and explicitly confirm newly authored content

#### Scenario: Read budget does not erase a confirmed action

- **WHEN** the pull-request detail hourly budget is exhausted after the reviewer confirmed an otherwise authorised review
- **THEN** the action protocol remains governed by provider deadlines and its own durable ledger
- **AND** no action item is counted as a detail read

### Requirement: Stale Commit Recovery Never Remaps an Inline Comment

When either current cached commit differs from a draft's head/base basis, the draft SHALL be stale and SHALL NOT be edited, frozen or submitted in place. Recovery SHALL occur only after an explicit reviewer action onto a current authorised detail, SHALL create a new revision and basis, and SHALL:

- preserve the summary;
- preserve the selected decision only when that decision remains available, otherwise select Comment;
- preserve a reply as placed only when its exact stable provider target still exists and is replyable, otherwise retain its body and former target as an unplaced item;
- turn every inline item into an unplaced item retaining its body, former path, side, line and commit basis; and
- perform no automatic placement, even when the same path, side, line number, hunk text or surrounding text exists in the new diff.

Every unplaced item SHALL be ineligible for submission until the reviewer explicitly chooses either one current line, converting it to an inline item, or one current replyable thread, converting it to a reply. Its former anchor or reply target SHALL be reference text only and SHALL never select the new target automatically. A confirmed operation SHALL never be rewritten onto new commits; it SHALL pause as stale and retain its frozen receipt.

#### Scenario: A push preserves summary and unplaces comments

- **WHEN** a draft with a summary and two inline comments was based on head `abc`, and the current detail has head `def`
- **THEN** recovery keeps the summary and both comment bodies
- **AND** both inline comments are unplaced until the reviewer anchors them again

#### Scenario: An equal-looking line is not an automatic match

- **WHEN** the former path and line number exist in the new diff with identical surrounding text
- **THEN** recovery still leaves the inline comment unplaced

#### Scenario: A valid reply survives recovery

- **WHEN** the current detail still contains the exact reply target and reports it replyable
- **THEN** recovery retains that reply's target
- **AND** a reply whose target disappeared is retained only as unplaced prose

#### Scenario: An orphaned reply requires a new explicit target

- **WHEN** recovery retains a reply whose former thread is gone
- **THEN** its body and former target remain visible but ineligible for submission
- **AND** it becomes eligible only after the reviewer explicitly chooses a current line or replyable thread

### Requirement: Review Action State Synchronises Only Native Windows

The application service SHALL publish review-action invalidations on a broadcast separate from cache events and `ServiceNotice`. Each invalidation SHALL carry only the canonical pull-request reference and the non-secret draft revision or operation phase. The Tauri shell SHALL forward this broadcast to native windows under its own event name. The web server SHALL neither subscribe to it nor expose it through SSE.

The main and detached pull-request windows SHALL re-read action state after the event and SHALL show the same current draft revision, blocked reason, progress ledger, partial result, stale recovery state and submitted receipt. Draft prose SHALL NOT be carried in the event. A receiver that missed an event SHALL obtain current state on mount.

#### Scenario: A detached-window edit reaches the main window

- **WHEN** a reviewer stores an edit in a detached pull-request window
- **THEN** the main window receives a native action invalidation for that pull request
- **AND** it re-reads and shows the new revision

#### Scenario: Served tabs receive no action event

- **WHEN** a review operation advances while browser tabs are connected to the event stream
- **THEN** no review-action event or draft content is sent through SSE

#### Scenario: The event contains no prose

- **WHEN** an action invalidation is inspected
- **THEN** it contains a pull-request reference and revision or phase only
- **AND** it contains no summary, comment, reply or credential-derived value

### Requirement: Application Commands Are Granted Per Window Kind

The Tauri application manifest SHALL register every application command in the invoke handler: all 83 handlers that predate this capability and every review-action handler added by it. Once command ACL enforcement is active, capabilities SHALL grant commands by window kind rather than matching the main and reader windows together.

The main window SHALL receive the commands its complete application surface uses. `pull-request-*` windows SHALL receive only pull-request reads, local review-progress operations, their checked link/window operations and review-action operations. `reader-*` windows SHALL receive only document read/watch, checked-link and reader-window operations. `image-*` windows SHALL receive only the image read and window operations they use. Reader and image capabilities SHALL grant no review-action command. Plugin permissions and application-command permissions SHALL both remain least-privilege.

#### Scenario: Every registered command has a manifest entry

- **WHEN** the invoke handler and `AppManifest::commands` are compared
- **THEN** every one of the 83 existing handlers and every new review handler appears exactly once in the manifest
- **AND** no manifest command lacks a registered handler

#### Scenario: Pull-request windows can review without broad plugins

- **WHEN** a native pull-request window is opened
- **THEN** its pull-request read, progress and review commands succeed
- **AND** its capability still grants no autostart, notification, menu or tray plugin permission

#### Scenario: Reader and image windows cannot invoke review actions

- **WHEN** a reader or image window invokes any review-action command directly
- **THEN** Tauri refuses the invocation before the service method runs

