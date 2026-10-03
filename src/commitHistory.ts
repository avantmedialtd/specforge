/// The Commit history switch's first-paint mirror, and the rule for whether the
/// main window has a rail at all (`commit-graph`: *Commit History Can Be Turned
/// Off*; `spec-browser`: *Rail Exists Only While Occupied*).
///
/// Kept out of the components for the reason `docWidth.ts` gives in its own
/// header: this repository has no component-test infrastructure, and a
/// `src/`-only diff short-circuits the mutation gate, so this module and its
/// tests are the decision's only automated coverage. Every export is a total
/// function of its arguments; the only ambient thing touched is `localStorage`,
/// and only through the two mirror helpers, which take an injectable store.

import { paneOf } from "./components/PullRequestPanel"
import type { PanelPosition } from "./types"

/// Where the first-paint mirror lives — beside the rail's own view state
/// (`specforge.railHidden`, `specforge.railWidth`), though unlike those it
/// mirrors an application setting rather than holding per-surface state.
export const COMMIT_HISTORY_STORAGE_KEY = "specforge.commitHistoryEnabled"

/// The subset of `Storage` the mirror uses, so tests can pass a fake and the
/// helpers never depend on a DOM being present.
export interface CommitHistoryStore {
    getItem(key: string): string | null
    setItem(key: string, value: string): void
}

/// The ambient store, or `null` where there isn't one. Reading
/// `globalThis.localStorage` can itself throw (blocked site data, a non-browser
/// runtime), so the access is guarded, not just the call.
function ambientStore(): CommitHistoryStore | null {
    try {
        return globalThis.localStorage ?? null
    } catch {
        return null
    }
}

/// Read the mirrored switch. Anything but the exact `"false"` this module
/// writes reads as ON — a first run, a cleared store, a corrupted value, a
/// store that throws — so the only way to start without the graph is to have
/// turned it off. Never throws: it runs on the path that paints the first
/// frame.
export function readMirroredCommitHistory(
    store: CommitHistoryStore | null = ambientStore(),
): boolean {
    try {
        return store?.getItem(COMMIT_HISTORY_STORAGE_KEY) !== "false"
    } catch {
        return true
    }
}

/// Mirror the switch for the next cold start. Best-effort, like the reading
/// width's mirror: the authoritative value is in the application settings, and
/// this store failing costs one frame of the wrong rail on a later launch.
export function writeMirroredCommitHistory(
    enabled: boolean,
    store: CommitHistoryStore | null = ambientStore(),
): void {
    try {
        store?.setItem(COMMIT_HISTORY_STORAGE_KEY, String(enabled))
    } catch {
        // Deliberately swallowed; see above.
    }
}

/// One pull-request panel as the rail decision sees it: the slot its setting
/// names (`null` until read), and whether it renders anything (`panelPresent`).
export interface PanelPlacement {
    position: PanelPosition | null
    present: boolean
}

/// Whether the main window has a rail at all.
///
/// The rail's occupants are the commit graph while the switch is on, and every
/// pull-request panel that is both positioned in a rail slot AND present —
/// never one merely positioned there, so a panel parked in a rail slot with its
/// feature off cannot hold an empty column open. With no occupant the rail is
/// absent rather than hidden: no pane, no divider, no restore chevron, and its
/// toggles change nothing.
export function railHasOccupant(
    historyOn: boolean,
    panels: readonly PanelPlacement[],
): boolean {
    return historyOn || panels.some((panel) => panel.present && paneOf(panel.position) === "rail")
}
