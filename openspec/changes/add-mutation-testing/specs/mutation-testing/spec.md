## ADDED Requirements

### Requirement: Mutation-testing scope

The system SHALL generate mutants only from the headless crates
`openspec-core` and `openspec-app`, and SHALL express that scope in
`.cargo/mutants.toml` rather than on the command line, so that `cargo mutants`
invoked with no arguments from the repository root is already correctly scoped.

The three shell crates SHALL be out of scope. `specforge` and `specforge-web`
cannot be built in a mutation scratch tree at all: both require the built
frontend at compile time — Tauri's `generate_context!` validates `frontendDist`
and `specforge-web` embeds the same directory via `RustEmbed` — and that
directory is gitignored, which the scratch-tree copy honours. `specforge-tui`
is excluded for signal-to-noise, its largest module having no direct tests.

Individual files and mutant patterns MAY be excluded, and every exclusion SHALL
carry a written reason in `.cargo/mutants.toml` stating why a mutant there
would measure something other than product behaviour. Test instrumentation that
is deliberately compiled into the library rather than `#[cfg(test)]`-gated —
because integration tests link the crate as an ordinary dependency — SHALL be
excluded on those grounds.

The configured scope SHALL be expressed such that a single-file command-line
filter still narrows the run to that file. A configured file allowlist is
unioned with the command-line filter rather than intersected with it, which
would silently expand a single-file invocation into a full sweep; the scope is
therefore expressed as exclusions.

#### Scenario: Bare invocation is already scoped
- **WHEN** a developer runs `cargo mutants` with no arguments from the repository root
- **THEN** every generated mutant is in `crates/openspec-core/src/` or `crates/openspec-app/src/`
- **AND** no mutant is generated from `specforge`, `specforge-tui`, or `specforge-web`

#### Scenario: The Tauri dependency graph is never built
- **WHEN** a mutation run builds its baseline in a scratch tree that lacks the gitignored `dist/` directory
- **THEN** the baseline builds and passes
- **AND** no shell crate is compiled, so the absent `dist/` cannot fail the build

#### Scenario: A single-file filter narrows to that file
- **WHEN** a developer runs `cargo mutants -f crates/openspec-core/src/git.rs`
- **THEN** only mutants from that file are tested
- **AND** the run does not silently expand to the full configured scope

### Requirement: Green baseline prerequisite

Mutation testing SHALL be run only against a passing test suite. When the
unmutated baseline fails, the system SHALL stop rather than report results.

Suppressing the baseline check SHALL NOT be used to work around a failing test.
With a failing test in the baseline, every mutant's test run also fails, so
every mutant would be reported as caught and the tool would report a perfect
score indefinitely. A failing test SHALL be fixed, and a test whose assertion
depends on machine speed SHALL be made deterministic rather than given a wider
margin.

#### Scenario: Red baseline halts the run
- **WHEN** any test in an in-scope crate fails before mutation begins
- **THEN** the run stops with a distinct baseline-failure status and tests no mutants

#### Scenario: A timing-raced invariant is made deterministic
- **WHEN** a test proves a thread-ordering invariant by racing two operations and asserting which finishes first
- **THEN** it is replaced by one that suspends the operation under test at the relevant boundary
- **AND** the assertion holds identically regardless of machine speed

### Requirement: Changed-lines gate

The system SHALL run mutation testing in continuous integration on every push,
restricted to the lines that push introduced, and SHALL fail the build when a
mutant on those lines survives. Mutants that survive elsewhere in the codebase
SHALL NOT fail the build.

The diff base SHALL be resolved from the branch being pushed:

$$\mathit{base} = \begin{cases}
\texttt{merge-base(origin/master, HEAD)} & \text{branch push} \\
\texttt{push.before} & \text{master push, } \mathit{before} \text{ resolvable} \\
\texttt{merge-base(origin/master, HEAD)} & \text{first push or force-push}
\end{cases}$$

Merge-base resolution gives three-dot semantics, so a branch is never blamed for
commits that landed on `master` after it diverged. Because work lands on
`master` by fast-forward, `push.before..HEAD` is exactly the batch that just
landed. The gate SHALL NOT be keyed on pull-request events, since work in this
repository lands by fast-forward rather than through pull requests.

When the resolved diff contains no changes to in-scope crate sources, the job
SHALL succeed without installing or running the mutation tool.

The gate SHALL run in a workflow separate from the `continuous-integration`
pipeline, with its own concurrency policy and job timeout, so that a long
mutation run neither delays nor is cancelled by that pipeline. See the
*Parallel Job Execution* and *Pipeline Trigger* requirements in the
`continuous-integration` capability.

#### Scenario: A surviving mutant on a changed line fails the build
- **WHEN** a push changes a line in `openspec-core` and a mutant of that line is caught by no test
- **THEN** the mutation job exits non-zero and the build is marked failed
- **AND** the surviving mutant is reported as a run annotation and in the job summary

#### Scenario: Pre-existing survivors elsewhere do not fail the build
- **WHEN** a push changes one file, and unrelated in-scope files contain surviving mutants
- **THEN** only mutants within the pushed diff are tested and the unrelated survivors are ignored

#### Scenario: A commit touching no in-scope Rust skips quickly
- **WHEN** a push changes only OpenSpec artefacts, frontend sources, or documentation
- **THEN** the job reports an empty diff and succeeds without installing the mutation tool

#### Scenario: Fast-forward onto master re-checks only what landed
- **WHEN** `master` is fast-forwarded and pushed
- **THEN** the diff is taken from the previous `master` commit to the new head

#### Scenario: A force-pushed branch still resolves a base
- **WHEN** a branch is force-pushed and the previous head commit no longer resolves
- **THEN** the job falls back to the merge-base with `origin/master` rather than failing

#### Scenario: Mutation runs do not cancel or delay the CI pipeline
- **WHEN** a mutation run is in progress and a new commit is pushed to `master`
- **THEN** the in-progress run is not cancelled and the new commit starts its own run
- **AND** the `continuous-integration` pipeline's jobs are unaffected

### Requirement: Mutant timeout policy

The system SHALL bound how long any single mutant may run, with a floor high
enough that a mutant stalling tests which carry their own multi-second deadlines
is reported as caught rather than as timed out. The mutation job SHALL also
carry an independent wall-clock timeout well below the platform default, so a
runaway run is terminated in minutes rather than hours.

#### Scenario: A mutant that stalls deadline-bearing tests is reported caught
- **WHEN** a mutant causes every timing-bounded test to run to its own deadline and fail
- **THEN** the mutant is reported as caught, not as timed out

#### Scenario: A runaway run is terminated
- **WHEN** a mutation job exceeds its configured wall-clock budget
- **THEN** the job is terminated and marked failed rather than running to the platform default

### Requirement: Local invocation and recorded baseline

The system SHALL document, in both the `README.md` and `CLAUDE.md` command
tables, how a developer reproduces the gate's verdict locally before pushing,
how to list the mutants in scope without building, and how to mutate a single
file. The documented local command SHALL match what continuous integration runs,
so that a green local result predicts a green gate.

Because no scheduled sweep exists, the full mutation picture SHALL be produced
by a documented manual command, and its result SHALL be recorded in `README.md`
as a dated table giving the scope and the caught, missed, timed-out and unviable
counts. Survivors recorded there are a backlog and SHALL NOT fail any build.

#### Scenario: Developer reproduces the gate before pushing
- **WHEN** a developer runs the documented local command on a branch with in-scope changes
- **THEN** the mutants tested are the same ones the gate would test for that branch

#### Scenario: Scope is verifiable without a build
- **WHEN** a developer lists the mutants in scope after editing the configuration
- **THEN** the list is produced without compiling the workspace

#### Scenario: The recorded baseline is dated and reproducible
- **WHEN** a full sweep is completed
- **THEN** `README.md` carries its date, scope, counts, and the command that produced it
