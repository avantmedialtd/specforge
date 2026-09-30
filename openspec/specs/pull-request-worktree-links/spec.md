# pull-request-worktree-links Specification

## Purpose

Defines how SpecForge joins the pull requests its BitBucket and GitHub panels list with the worktrees it tracks: reading each warm repository's remotes lazily and re-reading them only when its git configuration changes, recording each worktree's upstream from the status it already reads, carrying each row's head repository, the matching rule shaped against a fork's same-named branch and the shared branch a worktree was created from, the links snapshot both transports serve and when a frontend re-reads it, and the worktree marker that leads a panel row back to its change.
## Requirements
### Requirement: Repository Remote Identities

For each tracked git repository that is not disabled, the system SHALL read the repository's remotes and their fetch URLs as git resolves them — including `url.<base>.insteadOf` rewrites — with a single git invocation the first time pull-request links are computed for that repository, and SHALL remember the result until the repository's git configuration file changes. A disabled repository SHALL cause no such invocation at any time, whether or not its git configuration changes (see the *Cold Aggregation of Disabled Rows* requirement in the `workspace-registry` capability); a flat (non-git) workspace has no remotes. A change to the repository's remote-tracking refs alone — a fetch, pull or push — SHALL NOT cause the remotes to be re-read.

Each URL SHALL be parsed into a **remote identity** of a transport — SSH for the scp-like and `ssh://` forms, HTTPS for `https://` — a host, and an `owner/name` path, accepting the scp-like form (`git@host:owner/name.git`), `ssh://` URLs with or without a user and port, and `https://` URLs with or without a user and port, ignoring a trailing `.git` and a trailing slash. Host and path SHALL be compared case-insensitively. A URL that fits none of these forms, or whose path is not exactly two segments, SHALL have no identity and SHALL link nothing.

When a repository's git configuration file changes, the system SHALL forget its remembered remotes and SHALL refresh that repository's working-tree status and announce the refresh — the existing announcement that makes every open view re-read — so a changed remote, and an upstream set or changed through configuration alone (`git branch -u`, `git push -u`), take effect without a restart. For a disabled repository this refresh SHALL spawn nothing, as every refresh of a disabled repository does today.

#### Scenario: The usual URL forms parse to the same identity

- **WHEN** a repository's remotes are `git@github.com:Acme/Api.git`, `ssh://git@github.com:22/acme/api`, and `https://user@github.com/acme/api/`
- **THEN** all three parse to the host `github.com` and the path `acme/api`
- **AND** the first two are SSH and the third HTTPS

#### Scenario: An unparseable URL links nothing

- **WHEN** a remote's URL is a local path such as `/srv/git/api.git`
- **THEN** it has no remote identity and no pull request is linked through it

#### Scenario: Remotes are read once and remembered

- **WHEN** links are computed twice for the same repository with no change to its git configuration in between
- **THEN** exactly one git invocation has read its remotes

#### Scenario: A disabled repository reads no remotes

- **WHEN** a registered repository is disabled and its git configuration file is rewritten
- **THEN** no git invocation reads its remotes or its status

#### Scenario: A fetch does not re-read remotes

- **WHEN** `git fetch` updates a tracked repository's remote-tracking refs and leaves its git configuration unchanged
- **THEN** its remotes are not re-read

#### Scenario: A remote change reaches open views

- **WHEN** the user runs `git remote set-url origin` on a tracked repository while SpecForge is open
- **THEN** the repository's remotes are re-read the next time links are computed, and a refresh of the repository is announced
- **AND** links through the old identity disappear and links through the new one appear without a restart

#### Scenario: An upstream set through configuration takes effect

- **WHEN** the user runs `git branch -u origin/feature` in a tracked worktree while SpecForge is open
- **THEN** the worktree's recorded upstream becomes `origin/feature` without any file in the worktree changing

### Requirement: Worktree Upstreams Are Recorded

For each tracked worktree the system SHALL record the upstream of its current branch — the remote and the remote branch it tracks — from the working-tree status it already reads, without an additional git invocation. The upstream SHALL be split into remote and branch by the longest name among the repository's remotes that prefixes it followed by `/`. A worktree with a detached HEAD, a branch without an upstream, or an upstream whose remote is not among the repository's remotes SHALL have no upstream.

#### Scenario: An upstream is split by the known remote names

- **WHEN** a repository has remotes `origin` and `origin/mirror`, and a worktree's upstream is `origin/mirror/feature`
- **THEN** its upstream remote is `origin/mirror` and its upstream branch is `feature`

#### Scenario: Recording upstreams costs no git invocation

- **WHEN** a worktree's status is refreshed
- **THEN** its upstream is recorded from that same status invocation
- **AND** no git invocation is added to the status refresh

### Requirement: Pull-Request Rows Carry Their Head Repository

Every pull-request row, in the BitBucket snapshot and in both lists of the GitHub snapshot, SHALL carry the full name of its **head repository** — the repository its source branch lives in — in addition to the destination repository it already carries: BitBucket's `source.repository.full_name`, GitHub's `headRepository.nameWithOwner`. When the provider reports no head repository (for example, a deleted fork), the field SHALL be empty and the row SHALL link to no worktree.

#### Scenario: A fork's pull request names the fork

- **WHEN** a pull request into `acme/api` comes from the branch `fix` of the fork `ada/api`
- **THEN** its row's destination repository is `acme/api` and its head repository is `ada/api`

#### Scenario: A deleted fork links nothing

- **WHEN** a row's head repository is empty
- **THEN** the row is linked to no worktree

### Requirement: Matching a Pull Request to a Worktree

A worktree $$w$$ SHALL be linked to a pull request $$p$$ exactly when either rule holds, where $$b_p$$ is the pull request's head branch, $$H_p$$ its head repository, $$B_p$$ its destination repository, $$\beta_w$$ the worktree's branch, $$u_w$$ its upstream, $$R$$ the remote identities of the worktree's repository, and $$\simeq$$ is identity agreement as defined below:

$$\text{linked}(w,p) \iff \underbrace{\big(u_w \ne \varnothing \,\wedge\, \iota(u_w.\text{remote}) \simeq H_p \,\wedge\, u_w.\text{branch} = b_p \,\wedge\, (u_w.\text{branch} = \beta_w \vee H_p \ne B_p)\big)}_{\text{upstream rule}} \;\vee\; \underbrace{\big(\beta_w = b_p \,\wedge\, (u_w = \varnothing \vee u_w.\text{branch} \ne \beta_w) \,\wedge\, \exists\, r \in R : r \simeq H_p\big)}_{\text{branch-name rule}}$$

- A remote identity **agrees** with a pull request's head repository when their `owner/name` paths are equal ignoring case and either the remote's host is the pull request's provider host — `github.com` for GitHub, `bitbucket.org` for BitBucket, each also accepting its documented SSH-over-HTTPS host `ssh.github.com` or `altssh.bitbucket.org` — or the remote is an SSH remote whose host is none of those known hosts (an SSH host alias), in which case the path alone decides. An HTTPS remote on any other host SHALL NOT agree.
- Head and destination repositories SHALL be compared ignoring case; branch names SHALL be compared exactly.
- The **upstream rule** SHALL apply when the upstream branch has the same name as the worktree's branch, or when the pull request's head repository differs from its destination — a pull request from a fork, whose local branch may be named differently. An upstream that only records the shared branch a worktree was created from — a differently named branch in the same repository — SHALL NOT link, so a feature worktree created from `develop` is never linked to a pull request whose head is `develop`.
- The **branch-name rule** SHALL match only against the head repository, never the destination repository, so a local branch is never linked to a fork's pull request that merely shares its name; and a worktree whose upstream has the same branch name as the worktree's branch is governed by the upstream rule alone.
- Worktrees without a branch, worktrees the registry does not track, worktrees of disabled repositories, flat workspaces, and rows without a web URL SHALL link nothing.

Links SHALL be many-to-many. A pull request's linked worktrees SHALL be ordered with main worktrees before the others, and by path within each group; a worktree's linked pull requests SHALL be ordered as they appear in their snapshots, BitBucket before GitHub, and within GitHub authored before review-requested.

#### Scenario: A branch pushed with tracking links through its upstream

- **WHEN** a worktree on branch `feature` tracks `origin/feature`, `origin` points at `github.com/acme/api`, and a GitHub pull request into `acme/api` has head `feature` in `acme/api`
- **THEN** the worktree is linked to that pull request by the upstream rule

#### Scenario: A renamed local branch in the same repository does not link

- **WHEN** a worktree on local branch `wip` tracks `origin/feature`, and a pull request into `acme/api` has head `feature` in `acme/api`
- **THEN** the worktree is not linked to that pull request

#### Scenario: A branch pushed without tracking links by name

- **WHEN** a worktree on branch `feature` has upstream `origin/main`, `origin` points at `acme/api`, and a pull request's head is `feature` in `acme/api`
- **THEN** the worktree is linked to that pull request by the branch-name rule

#### Scenario: A worktree branched from a shared branch does not link to its release pull request

- **WHEN** a worktree on branch `feature` was created from `origin/develop` and tracks it, and a pull request from `develop` into `main` is open in the same repository `acme/api`
- **THEN** the worktree is not linked to that pull request

#### Scenario: A fork's same-named branch does not link

- **WHEN** a worktree on branch `main` tracks `origin/main`, `origin` points at `acme/api`, and a pull request into `acme/api` has head `main` in the fork `ada/api`
- **THEN** the worktree is not linked to that pull request

#### Scenario: A checked-out fork links through its upstream

- **WHEN** a repository has a remote `ada` pointing at `github.com/ada/api`, a worktree on local branch `ada-fix` tracks `ada/fix`, and a pull request into `acme/api` has head `fix` in `ada/api`
- **THEN** the worktree is linked to that pull request by the upstream rule

#### Scenario: An SSH alias matches on the path

- **WHEN** a remote's URL is `git@github-work:acme/api.git` and a GitHub pull request's head repository is `acme/api`
- **THEN** that remote agrees with the head repository

#### Scenario: An unknown HTTPS host does not agree

- **WHEN** a remote's URL is `https://gitlab.com/acme/api.git` and a GitHub pull request's head repository is `acme/api`
- **THEN** that remote does not agree with the head repository

#### Scenario: Hosts must agree when both are known

- **WHEN** a remote points at `bitbucket.org/acme/api` and a GitHub pull request's head repository is `acme/api`
- **THEN** that remote does not agree with the head repository

#### Scenario: Two clones both link, in a stated order

- **WHEN** two registered clones of `acme/api`, at `/code/b/api` and `/code/a/api`, each have their main worktree on the pull request's head branch
- **THEN** the pull request is linked to both worktrees, `/code/a/api` first

### Requirement: The Pull-Request Links Snapshot

The system SHALL serve a `get_pull_request_links` command on both the desktop and the browser transports returning, for every linked worktree, its path and its linked pull requests — each with provider, number, title, web URL, destination repository, draft flag, checks state, conflicting flag, review summary, and whether it is the viewer's own or awaiting their review — and, for every linked pull request, its web URL and its linked worktrees as repository id, worktree path and branch. The snapshot SHALL be computed in the headless application layer from the current pull-request snapshots, the current repository data and the remembered remotes, SHALL make no network request, and SHALL read no credential.

A frontend SHALL re-read the snapshot whenever it re-reads the workspace views and whenever either pull-request snapshot is announced as updated, so a link appears or disappears no later than the next of those announcements.

#### Scenario: The command is a pure read

- **WHEN** `get_pull_request_links` is invoked over the browser transport
- **THEN** it returns the links snapshot
- **AND** no request is sent to any pull-request provider and no host-side effect occurs

#### Scenario: Switching branch updates the link

- **WHEN** the user checks out a pull request's head branch in a tracked worktree
- **THEN** after the worktree's status refresh is announced, the links snapshot links that worktree to the pull request

#### Scenario: Disabling a provider removes its links

- **WHEN** the GitHub feature is disabled
- **THEN** once its snapshot is announced as disabled, the links snapshot contains no GitHub pull request

### Requirement: Pull-Request Rows Lead to Their Worktree

In the BitBucket panel and the GitHub panel, a row linked to at least one worktree SHALL show a **worktree marker** beside the row, naming the first linked worktree by its branch (or folder basename when the branch is unknown) and listing every linked worktree in its tooltip and accessible name. The marker SHALL be its own control, separate from the row's control that opens the pull request, and SHALL be reachable by keyboard. Activating it SHALL navigate SpecForge, on both transports, to the first linked worktree:

- when that worktree hosts exactly one active change, to that change's default artifact in that worktree's instance;
- when it hosts several, to the default artifact of the one most recently modified, in that instance;
- when it hosts none, to the repository's file browser.

Activating the marker SHALL NOT open the pull request, SHALL NOT navigate the browser skin's page away from SpecForge, and SHALL NOT act on the serving host. A row with no linked worktree SHALL show no marker and SHALL render as it does without this capability.

#### Scenario: The marker opens the change in its worktree

- **WHEN** a linked row's worktree hosts the single active change `add-rate-limits` and the user activates the marker
- **THEN** the detail pane shows `add-rate-limits`'s default artifact read from that worktree
- **AND** the address names that instance when the change has several

#### Scenario: A worktree without a change opens the file browser

- **WHEN** the linked worktree hosts no active change and the user activates the marker
- **THEN** the repository's file browser opens

#### Scenario: The marker and the row do different things

- **WHEN** the user activates the row itself rather than the marker
- **THEN** the pull request opens as it did before this capability
- **AND** SpecForge does not navigate

#### Scenario: Several linked worktrees are all named

- **WHEN** a pull request is linked to two worktrees
- **THEN** the marker names the first and its tooltip lists both
- **AND** activating it navigates to the first

