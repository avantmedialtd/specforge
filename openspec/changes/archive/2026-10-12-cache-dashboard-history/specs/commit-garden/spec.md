## ADDED Requirements

### Requirement: Commit Mining At Most Once Per History Change

The application SHALL mine each registered repository's garden commits at most once per change to that repository's history, rather than once per garden fetch. The garden SHALL use the same history-moved signal, and the same single-flight collapse of concurrent derivations, that the *Change Lifecycle Mining* requirement in the `dashboard` capability uses. A garden fetch for a repository whose history has not moved since its last successful mine SHALL issue no git invocation for that repository. When a repository's history moves, including a commit on a detached HEAD in a linked worktree, the application SHALL re-mine that repository and SHALL NOT re-mine any other. The *Live, Today-Scoped Updates* freshness therefore still holds within the watcher's debounce window.

Only the mined commit list SHALL be retained. On every fetch the plots SHALL still be scoped to the current local day, their authors resolved against the current identity configuration, and their active-change counts read from the live registry. A change of day, of identity aliases or of active changes therefore SHALL take effect on the next fetch without a history move. A failed mine SHALL NOT be retained as though it were a result: it SHALL be retried on the next fetch, while a successful mine that found no commits MAY be retained. The retained commits SHALL live in memory only, and unregistering a repository SHALL discard them. The derived plots SHALL be identical to those an uncached mine would produce for the same history, day and identity.

#### Scenario: An unchanged repository is not re-mined

- **WHEN** the garden is fetched twice
- **AND** no registered repository's history moved between the fetches
- **THEN** the second fetch issues no git invocation for any repository

#### Scenario: A commit re-mines only its own repository

- **WHEN** a commit lands in one of two registered repositories
- **AND** the garden is fetched
- **THEN** only that repository is re-mined
- **AND** the new commit appears in its plot within the debounce window

#### Scenario: A detached-HEAD commit in a linked worktree is picked up

- **WHEN** a commit is made on a detached HEAD in a linked worktree of a registered repository
- **AND** the garden is fetched after the debounce window
- **THEN** the repository is re-mined and the commit appears in its plot

#### Scenario: The day boundary needs no history move

- **WHEN** the garden is fetched after the local midnight boundary with no history move since the previous fetch
- **THEN** the plots are scoped to the new day
- **AND** no git invocation is issued

#### Scenario: An identity change needs no history move

- **WHEN** an alias is added to the developer identity and the garden is fetched again with no history move
- **THEN** the nodes for that alias carry the application accent

#### Scenario: A transient mining failure is retried

- **WHEN** mining a repository's garden commits fails
- **AND** the garden is fetched again
- **THEN** the mine is retried for that repository rather than serving a dormant plot for the rest of the session

#### Scenario: The cache persists nothing

- **WHEN** garden commits are cached and the application is restarted
- **THEN** no garden file exists under the application's data directory
- **AND** the first fetch after restart mines each repository afresh
