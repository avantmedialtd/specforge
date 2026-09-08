## Context

`browse-archive-across-worktrees` solved this exact problem for archived changes and landed on master. This change applies its model to the file browser, so the two surfaces are consistent rather than each inventing a worktree story.

The archive design's decisions transfer almost wholesale — union the listing, de-duplicate on a logical identity, keep the copy choice per-item and view-local, never assert that copies are the same. What does **not** transfer is the empirical premise underneath one of them:

```
  archive-browser                     workspace-file-browser
  ───────────────                     ──────────────────────
  114/116 dirs byte-identical         every worktree is on a
  across worktrees                    different branch
        │                                     │
        ▼                                     ▼
  divergence is a ~39-min window        divergence is the point
        │                                     │
        ▼                                     ▼
  copy control is usually a label       copy control is usually live
  divergence marker = noise             divergence marker = the signal
```

That inversion is why this is a sibling design rather than a copy: the same mechanism, with the cost/benefit of one control flipped.

## Goals / Non-Goals

**Goals:**

- Every markdown file in a repository is reachable from its Repo group, whichever worktree holds it.
- Where a file exists in several worktrees, the tree says whether the copies differ, and the preview can be pointed at any of them.
- The existing address grammar is untouched.
- The refusal to watch a workspace-sized tree survives pooling.

**Non-Goals:**

- Making the copy choice addressable. That would require a worktree axis in the file-address grammar, which `view-routing` reserves against for a stated reason. The archive precedent is that a per-item copy choice is view-local.
- Diffing copies against each other, or merging them. Divergence is *surfaced*, never resolved.
- Registering a watcher for the listing. Pooling makes that worse, not better.
- Changing anything for a flat workspace, which has exactly one root.

## Decisions

### D1. De-duplicate on the root-relative path

The union's identity is the forward-slash path relative to each worktree's own root, which is what the enumeration already returns and what the address already carries.

*Rejected — identity by content hash.* It would collapse identical files and split differing ones, which sounds appealing and is wrong: the tree would then show one path twice when two branches disagree, which is precisely the shape a folder tree cannot express.

*Rejected — identity by absolute path.* Never collapses anything, so the tree renders the same file once per worktree, which is the duplication the union exists to remove.

### D2. The tree marks divergence; it does not rank or resolve it

A row whose copies differ carries a marker. The marker states *that* they differ, not which is newer or better.

*Rejected — no marker (the archive's stance).* Correct there, where 114/116 were identical and a marker would have been permanent noise. Here divergence is the normal state of two branches, and the marker is the most useful thing the row can carry.

*Rejected — show which copy is newest.* Modification time answers a question nobody asked: a file edited later is not more authoritative, and on a fresh `git checkout` every mtime is checkout time, so the signal is mostly noise about when worktrees were created.

**How divergence is determined is deliberately left to `tasks.md`** rather than fixed here, because the cheap options (size, mtime, content hash) differ in cost by orders of magnitude and the right choice depends on measurement against a real repository. The requirement states the observable behaviour; the implementation picks the mechanism.

### D3. Relative links are contained by the selected copy's worktree

`Preview Link Handling` makes the browse root both the resolution base and the containment root for relative links. Pooling means there is no single root, so it becomes the worktree of the copy being previewed.

*Rejected — keep the repository's main worktree as the containment root.* It is the smaller edit and it is unsound: a file previewed from worktree B would resolve its relative links against a *different* worktree's tree, so a link would silently open the wrong file — or, where the target exists only in B, be refused as outside the root. Containment must follow the bytes being rendered.

This is the one place where pooling changes a security-adjacent contract rather than a presentational one, so it gets its own scenario.

### D4. The copy choice is view-local, and the address names the repository

The file address keeps naming a repository and a root-relative path, with no worktree segment. Which copy opens is resolved at load time (main worktree when it has the file, else the first copy that does).

*Rejected — a worktree segment in the address (`/r/<repo>/@<wt>/file/<path>`).* It makes the choice linkable, and it breaks the property `view-routing` protects explicitly: the codec decides the whole grammar from a closed vocabulary with no registry data. A worktree token is registry data. The archive shipped without addressing its copy choice and the gap has not been felt.

*Consequence accepted:* a link to a file that exists only in a feature worktree resolves to that worktree because it is the only copy — but once the branch merges, the same link opens the main worktree's copy. The address names a file in a repository, not a file in a worktree, and that is the intended reading.

### D5. Untracked files stay included, per worktree

`--others --exclude-standard` already includes untracked-but-not-ignored files for a single root; pooling keeps that per worktree.

*Rejected — restrict the union to tracked files.* It would make the union quieter and would defeat the motivating case: the draft you wrote in a feature worktree this morning is exactly the file you cannot currently reach.

Worth noting a property that falls out for free: `**/.claude/worktrees/` is in this repository's `.git/info/exclude`, so a worktree nested inside the main checkout is skipped by the main worktree's own enumeration. The union therefore does not double-count nested worktrees — but that is a consequence of the ignore rules, not something the union enforces, so a repository that does *not* ignore its nested worktrees would see duplicates. That is the user's ignore configuration talking, and the right place to fix it.

## Risks / Trade-offs

- **A pooled folder tree has a shape no single worktree has** — `docs/` may show a file from worktree A beside one from worktree B → This is the honest cost of the union and it is accepted deliberately: the browser stops meaning "one worktree's files" and starts meaning "this repository's files across its worktrees". The archive union has the same property; a folder tree just wears it more visibly than a list, which is why the copy control and the divergence marker exist — they keep every row traceable to a real worktree.

- **Divergence determination could dominate the listing's cost** — comparing content across N copies of 661 files is not free → Keep it out of the enumeration's critical path: the listing is what the tree needs, and divergence can arrive after it or be computed from metadata the enumeration already has. Measure before choosing (D2).

- **Pooling multiplies what a refresh costs, and the listing is pull-based by design** → Unchanged in kind: one `git ls-files` per worktree, on demand, off any watcher. The requirement that refuses a workspace-sized watcher is restated rather than weakened, and pooling strengthens the case for that refusal rather than eroding it.

- **A file present in one worktree only looks like it belongs to the repository** → It does, in the sense the browser now means. The copy control names its worktree, so the row is never anonymous — the same answer the archive gives for a change archived in one worktree.

- **`fix-ipc-wire-shape-mismatches` is in flight and touches `src/types.ts`** → Whichever lands second rebases. If that change lands first, its camelCase wire guard covers any new type this one adds, which is the better order.
