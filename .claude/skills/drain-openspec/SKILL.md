---
name: drain-openspec
description: Autonomously drain the specforge OpenSpec change queue — a fresh subagent implements each pending change in its own git worktree, then the main session runs the code-review gate and the build/clippy/test verification gate, archives, commits, FF-merges into master, pushes, and watches CI, repeating until the queue is empty. Use when asked to drain / work through / clear the pending OpenSpec changes, or to autonomously implement the whole change backlog end to end.
---

Autonomously work through every pending OpenSpec change in this repo, one at a time, until the queue is empty.

Each change is **implemented** by its own fresh subagent in its own git worktree (named after the change), so the main session's context stays lean — a clean implementation context per change without ever needing `/clear`. The subagent implements and runs the verification gate; the **main session** (the persistent loop owner) runs the code-review gate, archives, commits, FF-merges, pushes to `master`, and watches CI.

> Adapted for specforge from a Docker/E2E-heavy sibling repo. specforge has **no E2E/Docker/Playwright suite**, so the merge gate is the CI set — `bun run build`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace` — which is fast and orphan-safe. That collapses the original's "stop-before-E2E" subagent split and its triage/visual-baseline machinery; the split here is only for context hygiene, not orphan-avoidance.

**Input**: none required — drains the whole queue. Optionally pass a single change id as `$ARGUMENTS` to process only that change.

**Configuration baked in:**

- **Master integration:** FF-merge each completed change into `master` **and push `origin/master`** (full autonomy). The push runs in the **foreground** (a backgrounded `git push origin master` is blocked by the auto-mode classifier; the same command in the foreground goes through).
- **Verification gate (per change) — the specforge CI set, in order:**
  ```bash
  bun run build                                        # tsc --noEmit + vite build (also feeds Tauri's generate_context!)
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  All four must be clean before the change is committed. This mirrors `.github/workflows/ci.yml` exactly, so a green local gate predicts green CI.
- **Code-review gate (per change):** after the implementation subagent returns (and its work is verified present on disk), the **`code-review-expert`** agent reviews the full worktree diff *before* the change is committed. Every 🔴 Critical and 🟡 Important finding must be fixed (and the gate re-run) before commit; never merge over an unresolved Critical.
- **Post-push CI watch (per change):** after each push, **watch the GitHub build** and confirm it is green before starting the next change (`gh run watch`, or `gh run list --branch master` then `gh run view <id> --log-failed` on failure) — the repo convention (CLAUDE.md) is to monitor CI after every push.
- **On failure:** **try to fix** every problem (blocked `/opsx:apply`, red gate, clippy denials, merge conflict, red CI). For a large breakage the main session may spawn a **fix-subagent** (a general-purpose agent in fix mode — implementation-only) with a precise root-cause analysis. Only **halt and report** when genuinely out of ideas — never merge a broken change, never skip past it. **Every halt must also go out via `PushNotification`** — the loop may run unattended, and a silent halt wastes the rest of the session.

### Tool-access facts that shape the mechanics

- A **subagent cannot** `EnterWorktree` ("cannot create a worktree from a subagent with a cwd override") and has **Edit/Write blocked** by the isolation guard → it creates the worktree with `git worktree add` and mutates files via Bash heredocs (quoted `<<'EOF'` when the body has backticks / `$` / `${...}`) and the `openspec` CLI, using absolute paths everywhere (Bash cwd does not persist between calls).
- The **main session CAN Edit/Write files under `.claude/worktrees/<id>/`** from the repo-root cwd (the guard does not block it), but **cannot `EnterWorktree path:`** into an existing worktree from the repo root. So it drives the worktree by Bash: the gate via `cargo`/`bun` run in the worktree, archive via `openspec archive <id> --yes`, commit via `git -C <worktree> …` — no need to enter it.
- **Avoid `cd`** (a `cd` in a compound command can trigger a permission prompt). Use `git -C <worktree>`, `cargo` (which finds the workspace), and absolute paths. When a tool genuinely needs the worktree as cwd (e.g. `openspec archive`, `bun run build`), use a subshell: `( cd "<worktree>" && … )`.

---

## The loop (run in THIS, the main session)

Stay in the main checkout on `master` for the whole loop. Repeat until `openspec list` shows no pending (non-archived) changes:

### 0. Re-entrancy & resume guard (before anything else, every invocation)

If this is wrapped in a recurring `/loop`, one iteration takes far longer than the cron interval, so most fires arrive mid-iteration.

- **In flight?** If a backgrounded gate task is still running (`TaskList`), or the conversation shows an iteration mid-step, this fire is a **no-op**: report "drain in progress (<change-id>)" and stop. Do NOT start a second iteration or touch the in-flight worktree.
- **Crashed/stalled?** A worktree at `.claude/worktrees/<pending-change-id>` with NO running task means an earlier iteration died mid-flight. **Resume it — don't re-spawn blindly.** Read its state (`git -C <wt> status --short`, `git rev-list --count origin/master..<id>`) and continue from the matching step: uncommitted implementation present → re-run the gate, resume at the code-review gate (step 3); already committed → resume at FF-merge (step 5); worktree empty → remove it and restart the change at step 2.

### 1. Check for work

```bash
git fetch origin
git checkout master
git pull --ff-only
openspec list
```

- If `openspec list` reports **no pending changes** → the queue is drained. Go to the **Final phase** (skip it only if **zero** changes were merged this run — then report "queue already empty" and stop). Do not reschedule, do not poll.
- Otherwise pick the **next** pending change id. If `$ARGUMENTS` named a specific change, use only that one (and stop after it).
- **Heads-up:** draft OpenSpec commits for the pending changes may be local-only (ahead of `origin/master`). Subagent worktrees branch off `origin/master`, so they will not see the change files unless the drafts are pushed — `git push origin master` (foreground) the draft commits first so the worktrees can read `openspec/changes/<id>/`.
- **Order coupled changes deliberately.** When several pending changes touch the same subsystem (skim their `proposal.md` files — cheap), fix the drain order once for the whole run: foundational changes (shared `openspec-core` types/helpers, the IPC `types.rs`↔`src/types.ts` contract) first, broad cleanup/refactor passes last — `openspec list` order is creation order, not dependency order. Tell each implementer which sibling changes merged just ahead of it (expect task-text drift and adapt, don't force stale line references).

### 2. Subagent implements the change (implementation + gate)

Spawn **one** general-purpose subagent (Agent tool) per change, using the spawn prompt under "Spawning the implementer" below. Foreground is fine — with no E2E, its longest step is `cargo test`/`cargo build` (minutes), which completes cleanly. Wait for it to return.

The subagent returns: the branch name, the changed-file list, whether the full gate (`bun run build` + `cargo fmt` + `cargo clippy` + `cargo test`) is clean, whether it touched the IPC boundary (`types.rs` ↔ `src/types.ts`) or any cross-crate contract, and any assumptions/unfinished items. It leaves everything **uncommitted** in the worktree.

**Verify the real git state — never trust the report alone:**

```bash
git -C ".claude/worktrees/<change-id>" status --short        # non-empty = work present (expected: uncommitted)
git rev-list --count "origin/master..<change-id>"            # 0 = not yet committed (expected at this stage)
```

If the worktree is empty / the implementation is clearly incomplete (tasks unchecked, files missing), spawn another implementation subagent to finish it, or finish the remaining edits yourself with Edit/Write on the worktree files. Only proceed once the implementation is actually present and the gate is green.

### 3. Code-review gate (main session spawns the reviewer)

Before committing, have the **`code-review-expert`** agent review the implementation. Spawn it with the Agent tool (`subagent_type: code-review-expert`), foreground — it only reads (subagent Edit/Write is blocked anyway, and the review must not mutate the worktree). Prompt template — fill in `<change-id>` and the absolute repo path:

> Review the uncommitted implementation of OpenSpec change **`<change-id>`** in the git worktree at `/ABS/PATH/TO/specforge/.claude/worktrees/<change-id>`.
>
> - **Intent first:** read `openspec/changes/<change-id>/` in that worktree (proposal.md, design.md, tasks.md, specs/) — the diff must actually satisfy those artifacts, not just compile.
> - **Scope = the full change:** `git -C <worktree> diff origin/master` **plus** untracked files from `git -C <worktree> status --short` (read the new files too).
> - **Apply specforge's standards** (root `CLAUDE.md`, the capability spec you are implementing against):
>   - The two-layer split: state/filesystem/watcher/registry/parser logic belongs in **`openspec-core`** (headless, `cargo test`-able), never in the **`specforge`** Tauri crate.
>   - Every Rust type crossing the IPC boundary uses `#[serde(rename_all = "camelCase")]`, and its hand-written mirror in **`src/types.ts`** must match exactly (no codegen — both sides in sync).
>   - **Product vs format naming:** *SpecForge* is the product; *OpenSpec* is the on-disk format. User-facing copy, errors, dialogs, and path segments must pick the right one (see the `product-identity` spec).
>   - `read_artifact` and any path-taking command must reject paths outside the workspace's `openspec/changes/` subtree; caller paths should be validated against the registry.
>   - The tray SVG rasterizer's pure-black + alpha invariant (macOS template rendering).
> - The gate (`bun run build`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`) already passed — focus on what it cannot catch: logic bugs, edge cases, security (path traversal, git argument injection, IPC input validation), concurrency (locks held across await, broadcast lag, task leaks), spec/tasks mismatches, dishonest or missing test assertions, dead code.
> - Do NOT modify any file, do NOT run the app, do NOT commit.
> - Report each finding with file:line, why it matters, and a concrete fix, organized 🔴 Critical / 🟡 Important / 🟢 Minor, then a clear verdict: ready to merge or not.

Then act on the report — in the worktree, before commit:

1. **Fix every 🔴 Critical and 🟡 Important finding.** The main session edits the worktree files directly (Edit/Write work under `.claude/worktrees/<id>/`), or spawns a fix-subagent for a large rework. Apply 🟢 nits that are quick wins; skip the rest deliberately.
2. **Dismiss only with evidence.** If a finding is factually wrong (the reviewer misread the code or a deliberate repo convention), verify in the source and note why it's dismissed — don't "fix" correct code.
3. **Re-run the full gate** after any fix.
4. **Re-review when fixes were substantial** (beyond mechanical tweaks): spawn the reviewer again on the new diff. Cap at ~2 fix→review rounds — if Criticals keep surfacing, treat it as a large breakage (root-cause + fix-subagent), then review once more.

Proceed to commit only when a review round reports **no open 🔴/🟡 findings**.

### 4. Archive, verify, commit (main session, on the worktree)

All commands use absolute paths / `git -C` / a subshell into the worktree.

```bash
# Mark any remaining verification tasks done, then archive (updates main specs in the worktree).
( cd ".claude/worktrees/<change-id>" && openspec archive "<change-id>" --yes )   # add --skip-specs for tooling/doc-only changes
# Insurance re-run of the gate on the post-archive tree before the master push:
( cd ".claude/worktrees/<change-id>" && bun run build ) \
  && cargo fmt --all --manifest-path ".claude/worktrees/<change-id>/Cargo.toml" -- --check \
  && cargo clippy --manifest-path ".claude/worktrees/<change-id>/Cargo.toml" --workspace --all-targets -- -D warnings \
  && cargo test --manifest-path ".claude/worktrees/<change-id>/Cargo.toml" --workspace
git -C ".claude/worktrees/<change-id>" add -A
git -C ".claude/worktrees/<change-id>" commit -m "<concise change summary>" \
  --trailer "OpenSpec-Id=<change-id>"
```

Follow the repo's commit-message convention: a concise title, a short body (what changed + which capability specs it touches + a one-line test result), and trailers. specforge history carries `OpenSpec-Id=<id>` plus the session's `Co-Authored-By:` and `Claude-Session:` trailers (as the harness/CLAUDE.md dictate) — include those per the current session, and add `Issue=<key>` only if the proposal metadata carries a Jira key. Confirm the commit landed:

```bash
git -C ".claude/worktrees/<change-id>" status --short        # must be empty now
git rev-list --count "origin/master..<change-id>"            # must be >= 1
```

### 5. FF-merge & push to master

```bash
git fetch origin
git checkout master
git pull --ff-only
git merge --ff-only "<change-id>"
git push origin master        # FOREGROUND — a backgrounded push to master is classifier-blocked
```

`git merge --ff-only` only moves `master`'s pointer to the change tip; it does not disturb the worktree. If the auto-mode classifier denies the push despite this skill's standing authorization (settings allow-rules do NOT bypass it; only user intent visible in context does), that is a halt: send the `PushNotification`, leave the merged-but-unpushed state in place, and stop — it resumes cleanly at step 5 once the user authorizes.

### 6. Watch CI, then tear down and loop

```bash
gh run watch                                  # or: gh run list --branch master  →  gh run view <id> --log-failed
```

- **CI green** → tear down and loop:
  ```bash
  git worktree remove ".claude/worktrees/<change-id>"   # add --force if it refuses; the change is already on master
  git branch -d "<change-id>"
  ```
  Then go back to **step 1**.
- **CI red** → treat exactly like a failed gate: read `gh run view <id> --log-failed`, fix on `master` (or reopen the worktree), re-push, re-watch. Halt + `PushNotification` only when genuinely stuck — never leave `master` red.

---

## Final phase: integration backstop (main session)

Once **step 1** finds the queue empty *and* at least one change was merged this run, run the full gate once more over integrated `master` as the integration backstop (individual changes each branched off an earlier `origin/master`, so this catches interaction bugs):

1. Sync: `git fetch origin && git checkout master && git pull --ff-only`.
2. Run the full gate on `master`: `bun run build && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
3. Fix anything red (pass non-trivial fixes through the code-review gate before committing), commit on `master` with trailers, `git push origin master` (foreground), and watch CI.
4. Confirm the final CI run on `master` is green. Print the final summary — changes drained + the gate/CI result — **and send it via `PushNotification`** (the user may be away).

If you exhaust your ideas on a genuine failure, **stop and report** it — which check, what you tried, the failing test name / clippy lint — rather than pushing a broken state or skipping it.

---

## Spawning the implementer

Spawn a general-purpose subagent (Agent tool). Keep the prompt minimal but self-contained — the subagent starts fresh:

> Implement OpenSpec change **`<change-id>`** in the specforge repo at `/ABS/PATH/TO/specforge`. The change artifacts are on `origin/master` under `openspec/changes/<change-id>/`.
>
> 1. Create an isolated worktree: `git worktree add "/ABS/PATH/TO/specforge/.claude/worktrees/<change-id>" -b "<change-id>" origin/master`. You cannot use EnterWorktree and your Edit/Write are blocked by the isolation guard — mutate files via Bash heredocs (quoted `<<'EOF'` when the body contains backticks / `$`) and the `openspec` CLI, with absolute paths everywhere (Bash cwd does not persist).
> 2. Implement every task in `openspec/changes/<change-id>/tasks.md`, following the change's proposal/design/specs and the repo's `CLAUDE.md` conventions (the `openspec-core` vs `specforge` split; `#[serde(rename_all = "camelCase")]` IPC types mirrored by hand in `src/types.ts`; SpecForge-vs-OpenSpec naming). This is the same work `/opsx:apply <change-id>` performs — you may drive it that way if the skill is available to you.
> 3. Check off completed tasks in `tasks.md` and run `openspec validate <change-id> --strict` (from the worktree) until it passes.
> 4. Run the full gate from the worktree and make it clean: `bun run build`, then `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
> 5. **Do NOT commit.** Leave everything uncommitted in the worktree.
> 6. Report: the branch name, the changed-file list, the gate result (each of the four green?), whether you touched the IPC boundary (`crates/openspec-core/src/types.rs` ↔ `src/types.ts`) or any cross-crate contract, and any assumptions or unfinished items.

Add change-specific context only when the loop knows something the artifacts don't (a prior attempt's pitfall, a sibling change merged just ahead of this one).

**Fix mode** (large code-review findings or gate/CI rework): spawn the same general-purpose subagent with the precise root-cause analysis and the path to the EXISTING worktree. It operates on that worktree instead of creating one, applies the fixes, re-runs the gate, and returns with the fixes uncommitted — committing and merging remain the main session's job.

---

## Notes

- **Serial by design.** One change at a time → the worktree is removed before the next starts. Don't parallelize: concurrent FF-merges to one `master` race, and branching a later change off a pre-merge `origin/master` makes its FF-merge fail.
- **Context stays lean** because each subagent returns only a short report — the main loop accumulates a handful of summaries + git/gate output, never the implementation detail. The main session does carry review + gate + archive + merge + CI-watch; keep it tight (delegate big fixes to a fix-subagent).
- **A new change can appear mid-run** (someone drafts + commits one while you drain). It surfaces in the next `openspec list`; the loop picks it up. If it is a materially larger commitment than the original queue, it's reasonable to confirm with the user before draining it.
- **To poll for *new* work** instead of stopping when the queue empties, wrap this in `/loop`: `/loop /drain-openspec`. By itself it drains once and stops.
- **UI-only changes that need visual confirmation:** run the app on this worktree's dev slot with `bun run wt:dev` (side-by-side, per `CLAUDE.md`) rather than the fixed port 1420, and screenshot to verify — but the merge gate remains the CI set, not a manual look.
