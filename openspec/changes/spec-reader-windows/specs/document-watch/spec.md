# document-watch Delta — Reader Windows for Workspace Documents

## ADDED Requirements

### Requirement: Per-Document Watch Registration

The application SHALL provide a way for a surface rendering one markdown document to register interest in that document's file, and to release it. A registration SHALL be identified by the pair of an authorized browse root and a root-relative path — the same pair the guarded file read takes (see the *Guarded Workspace File Read* requirement in the `workspace-file-browser` capability).

Registration SHALL authorize the browse root against the workspace registry before any filesystem watch is established, refusing a root that is neither a registered or registry-discovered workspace nor a path inside a registered repository. Registration SHALL apply the same path guard the read applies — rejecting absolute paths, parent-directory components, resolved paths that escape the canonicalised root including through a symlink, and paths without a case-insensitive `.md` extension. A registration SHALL NOT be able to establish a watch anywhere the corresponding read would be refused.

Registration SHALL be keyed by canonical root and relative path, using the same canonicalization the registry keys on, so two callers naming one file through different but equivalent paths hold the same registration.

#### Scenario: Registration for an unregistered root is refused

- **WHEN** a document watch is requested for a root that is neither a registered workspace nor inside a registered repository
- **THEN** the registration is refused
- **AND** no filesystem watch is established

#### Scenario: A traversal path is refused

- **WHEN** a document watch is requested for a relative path containing parent-directory components that would resolve outside the browse root
- **THEN** the registration is refused
- **AND** no filesystem watch is established

#### Scenario: A non-markdown path is refused

- **WHEN** a document watch is requested for a path without a `.md` extension
- **THEN** the registration is refused

#### Scenario: Equivalent paths share one registration

- **WHEN** two surfaces register the same file through browse-root paths that canonicalise to the same location
- **THEN** both hold the same registration

### Requirement: Watch Registration Is Reference-Counted

Several surfaces MAY display one document at once — the detail pane, the file browser's preview, and a reader window. Registration SHALL be reference-counted per document: the first registration establishes the filesystem watch, subsequent registrations for the same document join it, and the watch SHALL be torn down only when the last registration is released.

Releasing a registration that is not held SHALL be a no-op rather than an error, so that a surface unmounting twice, or unmounting after a failed registration, cannot tear down a watch another surface depends on.

#### Scenario: Two surfaces on one document share a watch

- **WHEN** a reader window and the detail pane are both displaying the same document
- **THEN** exactly one filesystem watch exists for it

#### Scenario: The watch survives one surface closing

- **WHEN** two surfaces are displaying one document and one of them closes
- **THEN** the remaining surface continues to receive change notifications for that document

#### Scenario: The last release tears the watch down

- **WHEN** the last surface displaying a document closes
- **THEN** the filesystem watch for that document is torn down

#### Scenario: Releasing an unheld registration is harmless

- **WHEN** a release is issued for a document that has no registration
- **THEN** the operation succeeds without error
- **AND** no other document's watch is affected

### Requirement: The Watched Path Is the Parent Directory

A document watch SHALL watch the **parent directory** of the registered file, non-recursively, and SHALL deliver only those events naming that file. It SHALL NOT watch the file path itself.

This is required for correctness, not efficiency. Editors, version-control checkouts, and most atomic writers replace a file by writing a temporary file and renaming it over the target, which unlinks the inode a file-level watch is bound to. A watch established on the file would deliver one event and then go permanently silent while continuing to appear healthy — a document surface that updates exactly once and never again, with no error to observe.

If the watched directory is itself removed or replaced — a version-control operation swapping a whole subtree, or a change directory being archived — the watch SHALL be re-established once a directory exists at that path again, so that a document restored at the same address resumes updating.

#### Scenario: An atomic-rename save is observed

- **WHEN** a watched document is saved by writing a temporary file and renaming it over the target
- **THEN** a change notification is delivered for that document

#### Scenario: Repeated atomic saves keep being observed

- **WHEN** a watched document is saved by atomic rename several times in succession
- **THEN** a change notification is delivered for each save, not only the first

#### Scenario: A sibling file's change is not delivered

- **WHEN** a different file in the watched directory is modified
- **THEN** no change notification is delivered for the watched document

#### Scenario: A replaced directory re-arms the watch

- **WHEN** the directory containing a watched document is removed and later recreated with that document in it
- **THEN** subsequent modifications to that document deliver change notifications

### Requirement: Document Change Notification

A debounced batch of filesystem events naming a watched document SHALL produce one **document-changed** notification to the frontends, carrying the browse root and the root-relative path that identify the document. The notification SHALL carry no file content: a surface receiving it re-reads the document through the guarded read, so that exactly one code path reads a file and exactly one guard applies to it.

Events SHALL be debounced, so that a save producing several filesystem events yields one notification rather than several.

The notification SHALL be distinct from the workspace cache's own change events. A document change mutates no cached state and concerns no tree row, so it SHALL NOT be expressed as a variant of the cache-event stream, and existing consumers of that stream SHALL require no change to ignore it.

#### Scenario: A modification notifies with its identifiers

- **WHEN** a watched document is modified on disk
- **THEN** a document-changed notification is delivered carrying that document's browse root and root-relative path

#### Scenario: A burst of events yields one notification

- **WHEN** a single save produces several filesystem events for a watched document within the debounce window
- **THEN** one document-changed notification is delivered

#### Scenario: The notification carries no content

- **WHEN** a document-changed notification is delivered
- **THEN** it carries identifiers only
- **AND** the receiving surface re-reads the document through the guarded read

#### Scenario: Cache-event consumers are unaffected

- **WHEN** a document changes and a document-changed notification is delivered
- **THEN** no additional variant appears on the workspace cache's event stream
- **AND** consumers of that stream behave exactly as before

### Requirement: Watch Cost Is Bounded by Open Documents

The number of filesystem watches this capability establishes SHALL be a function of how many distinct documents are currently open, never of the size of a workspace or the number of files in it. For open document surfaces $S$, the number of live watches satisfies:

$$|W| = \bigl|\{(\text{root},\ \text{path}) : \text{refcount} > 0\}\bigr| \le |S|$$

The capability SHALL NOT establish a recursive watch, SHALL NOT walk a workspace, and SHALL NOT watch a directory for which no document is registered.

#### Scenario: A large workspace costs no more than a small one

- **WHEN** one document is open in a workspace containing many thousands of files
- **THEN** exactly one filesystem watch is established for it

#### Scenario: No watch exists with nothing open

- **WHEN** no document surface holds a registration
- **THEN** this capability holds no filesystem watch

#### Scenario: Watching is never recursive

- **WHEN** a document watch is established
- **THEN** it covers only the document's own directory and does not descend into subdirectories

### Requirement: Independent of the Workspace Watcher

This capability SHALL operate independently of the workspace watcher that maintains the change cache. It SHALL NOT depend on that watcher's roots, its event filtering, or its self-write suppression, and SHALL NOT alter any of them.

A document beneath a change directory therefore lies within both mechanisms, and a single edit to it MAY produce both a cache-change notification and a document-changed notification. This SHALL be harmless: a document surface refreshing twice SHALL coalesce the reads it performs, and a re-read returning unchanged bytes SHALL leave the rendered document, the reading position, and the loading indicator untouched, per the *Reactive Updates from Filesystem* requirement in the `spec-browser` capability.

#### Scenario: A change artifact may notify through both mechanisms

- **WHEN** an artifact beneath a change directory is modified while a reader window is open on it
- **THEN** the surface renders the updated content
- **AND** any duplicate refresh is coalesced and produces no visible flicker or loading indicator

#### Scenario: The workspace watcher is unchanged

- **WHEN** document watches are registered and released
- **THEN** the workspace watcher's roots, filtering, and event stream are unaffected

### Requirement: Cross-Frontend Watch Surface

Registration and release SHALL be exposed as application-service operations dispatched through both the desktop command layer and the web server's command table, and the document-changed notification SHALL be carried on both the desktop event channel and the web event stream, per the command-transport and event-transport parity contracts in the `web-ui` capability.

#### Scenario: Watching works over the web transport

- **WHEN** a document surface in the served web UI registers a document watch
- **THEN** the registration dispatches through the web command table
- **AND** document-changed notifications reach it over the web event stream

#### Scenario: Registration outlives no session it belongs to

- **WHEN** a frontend that holds document registrations disconnects
- **THEN** the application does not retain filesystem watches for documents no surface is displaying
