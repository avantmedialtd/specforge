// What a change offers to navigate to, and which of it opens first.
//
// The sidebar stops at the change row (`spec-browser`: *Workspace Tree
// Hierarchy*), so two derivations decide what a click on that row shows: the
// change's DEFAULT ARTIFACT and, for a change living in several worktrees, its
// DEFAULT INSTANCE. The same fixed artifact order drives the change header's
// tab strip (*Artifact Tab Strip in the Change Header*), so the first tab and
// the default artifact are one value by construction rather than two lists kept
// in step by review.
//
// Pure functions in their own module because JSX is not exercised by
// `bun test` and a frontend-only diff short-circuits the mutation gate — the
// same reasoning `changeIdentity.ts` and `docWidth.ts` give in their own
// headers. These tests are the only automated coverage this logic gets.

import type {
    ArtifactReadKind,
    ArtifactStatus,
    ChangeInstance,
    DivergenceLabel,
    PaletteColor,
} from "./types"

/// One artifact the change header offers.
export interface ArtifactTab {
    /// Stable identity for React keys, for marking the active tab, and for
    /// comparing two artifact choices. A capability name cannot collide with
    /// the three fixed kinds because it is always prefixed.
    key: string
    /// What the tab says. The three fixed artifacts carry title-case labels;
    /// a capability spec is named by its capability, exactly as the archive
    /// reader's strip already named it.
    label: string
    kind: ArtifactReadKind
    /// Present only for `kind: "spec"`.
    capability?: string
}

/// The identity of one artifact choice. Used to mark the active tab and to
/// test whether an artifact is still offered after a refresh.
export function artifactTabKey(kind: ArtifactReadKind, capability?: string): string {
    return kind === "spec" ? `spec:${capability ?? ""}` : kind
}

/// The tabs a change offers, in the fixed order Proposal, Design, Tasks, then
/// one per capability spec in listing order — **present artifacts only**.
///
/// An absent artifact gets no tab, dimmed or otherwise: the strip never offers
/// a control that cannot render (`spec-browser`: *Artifact Tab Strip in the
/// Change Header*). This replaces the tree's artifact rows, which DID render an
/// absent artifact as a dimmed slot (*Artifact Row Presence Treatment*, now
/// retired).
export function artifactTabs(status: ArtifactStatus): ArtifactTab[] {
    const tabs: ArtifactTab[] = []
    if (status.proposal) tabs.push({ key: "proposal", label: "Proposal", kind: "proposal" })
    if (status.design) tabs.push({ key: "design", label: "Design", kind: "design" })
    if (status.tasks) tabs.push({ key: "tasks", label: "Tasks", kind: "tasks" })
    for (const capability of status.specs) {
        tabs.push({
            key: artifactTabKey("spec", capability),
            label: capability,
            kind: "spec",
            capability,
        })
    }
    return tabs
}

/// The artifact a change row opens: the first present in the fixed order
/// Proposal, Design, Tasks, then the first capability spec in listing order —
/// which is exactly the strip's first tab, so the row and the strip cannot
/// disagree about where a click lands.
///
/// `null` when the change has no artifact at all. The row is still selectable
/// and the pane shows its empty state (*Workspace Tree Hierarchy*); a click
/// must never mint an address that is guaranteed to read not-found.
export function defaultArtifactFor(status: ArtifactStatus): ArtifactTab | null {
    return artifactTabs(status)[0] ?? null
}

/// The instance a multi-worktree change opens in: the repository's main
/// worktree when it hosts the change, otherwise the first instance in
/// aggregation order.
///
/// The same rule `workspace-file-browser` adopted for a default copy, so the
/// two surfaces agree on which worktree is "the" one. `instances` carries only
/// the rendered (non-archived) instances — the aggregator partitions archived
/// ones into their own section before the frontend sees them.
export function defaultInstanceFor(instances: ChangeInstance[]): ChangeInstance | null {
    return instances.find((instance) => instance.isMainWorktree) ?? instances[0] ?? null
}

/// One control in the change header's switcher row — a worktree instance in
/// the live form, an archived copy in the read-only one
/// (`spec-browser`: *Instance Switcher in the Change Header*).
///
/// One shape for both forms, so the read-only variant cannot grow a branch
/// chip by accident: `branch` is what the archive builder always leaves null
/// (`archive-browser`: *Read-Only Artifact Navigation* — "no branch is shown
/// anywhere in the reader, including in the copy switcher").
export interface SwitcherOption {
    /// Stable identity — a worktree path, or a copy's `(worktree, dir)` key.
    key: string
    /// What the control says: a worktree folder basename, or the archive's
    /// own copy label.
    label: string
    /// The branch chip's text, or null for no chip at all.
    branch: string | null
    /// The owning workspace's palette colour, tinting the chip exactly as the
    /// identity row's chip is tinted. Null renders neutral ink.
    color: PaletteColor | null
    /// The instance's divergence label, where it has one.
    divergence: DivergenceLabel | null
}

/// The worktree folder basename — how an instance names itself in the
/// switcher, since its branch is carried separately as a chip.
export function worktreeBasename(path: string): string {
    const parts = path.split("/").filter(Boolean)
    return parts[parts.length - 1] ?? path
}

/// The switcher's controls for a change's rendered instances, in aggregation
/// order. `color` is the owning workspace's palette colour; every instance of
/// one logical change shares it, being worktrees of one repository.
export function instanceOptions(
    instances: ChangeInstance[],
    color: PaletteColor | null,
): SwitcherOption[] {
    return instances.map((instance) => ({
        key: instance.worktreePath,
        label: worktreeBasename(instance.worktreePath),
        branch: instance.branch,
        color,
        divergence: instance.divergence,
    }))
}
