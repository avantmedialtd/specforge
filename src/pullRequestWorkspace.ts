import type {
    DiffFile, FileReviewState, PullRequestCheck, PullRequestDetail, PullRequestReference,
    PullRequestSummary, ReviewProgress, ReviewThread,
} from "./types"
import { fileKey } from "./diffFiles"
import { progressWords, referenceKey, splitThreads } from "./pullRequestView"

export const WORKSPACE_TABS = ["overview", "files", "discussion"] as const
export type WorkspaceTab = (typeof WORKSPACE_TABS)[number]
export type WorkspaceSurface = "main" | "detached"
export interface ReviewJump {
    kind: "thread" | "draft"
    id: string
    path: string
}
export interface WorkspacePlace {
    path: string
    offset: number
    head: string
    base: string
}
export interface WorkspacePresentation {
    tab: WorkspaceTab
    place: WorkspacePlace | null
}
const STORAGE_KEY = "specforge.prWorkspace.v1"
const LIMIT = 50
const memory = new Map<string, WorkspacePresentation>()
type StorageLike = Pick<Storage, "getItem" | "setItem">

export function workspaceKey(reference: PullRequestReference, surface: WorkspaceSurface): string {
    return `${surface}:${referenceKey(reference)}`
}

function storage(): StorageLike | null {
    try {
        return globalThis.localStorage ?? null
    } catch {
        return null
    }
}

function validPresentation(value: unknown): value is WorkspacePresentation {
    if (!value || typeof value !== "object") return false
    const item = value as Partial<WorkspacePresentation>
    if (!WORKSPACE_TABS.some((tab) => tab === item.tab)) return false
    const place = item.place
    return place === null || (
        typeof place === "object" && place !== undefined &&
        typeof place.path === "string" && typeof place.head === "string" &&
        typeof place.base === "string" && typeof place.offset === "number" &&
        Number.isFinite(place.offset)
    )
}

function records(store: StorageLike | null): Map<string, WorkspacePresentation> {
    try {
        const raw: unknown = JSON.parse(store?.getItem(STORAGE_KEY) ?? "null")
        if (!Array.isArray(raw)) return new Map(memory)
        return new Map(raw.filter((entry): entry is [string, WorkspacePresentation] =>
            Array.isArray(entry) && entry.length === 2 &&
            typeof entry[0] === "string" && validPresentation(entry[1]),
        ).slice(-LIMIT))
    } catch {
        return new Map(memory)
    }
}

export function readWorkspace(key: string, store: StorageLike | null = storage()): WorkspacePresentation {
    return records(store).get(key) ?? { tab: "overview", place: null }
}

export function writeWorkspace(
    key: string,
    value: WorkspacePresentation,
    store: StorageLike | null = storage(),
): void {
    const next = records(store)
    next.delete(key)
    next.set(key, value)
    while (next.size > LIMIT) next.delete(next.keys().next().value!)
    memory.clear()
    for (const [id, item] of next) memory.set(id, item)
    try {
        store?.setItem(STORAGE_KEY, JSON.stringify([...next]))
    } catch {
        // The current window retains its presentation when storage is blocked.
    }
}

export function restoredOffset(place: WorkspacePlace, head: string, base: string): number {
    return place.head === head && place.base === base ? place.offset : 0
}

export function workspaceScrollPort(element: HTMLElement | null): HTMLElement | null {
    // A detached PR's detail pane is itself the scroll port; the main view
    // instead scrolls inside its enclosing split-pane-right.
    for (let parent = element; parent; parent = parent.parentElement) {
        const { overflowY } = getComputedStyle(parent)
        if (overflowY === "auto" || overflowY === "scroll") return parent
    }
    return null
}

export const FILE_FILTERS = [
    { value: "all", label: "All" },
    { value: "unreviewed", label: "Unreviewed" },
    { value: "changed", label: "Changed since review" },
    { value: "discussion", label: "Has discussion" },
    { value: "skipped", label: "Skipped" },
] as const
export type FileFilter = (typeof FILE_FILTERS)[number]["value"]
export const REVIEW_LABELS: Record<FileReviewState, string> = {
    viewed: "Viewed", changedSinceViewed: "Changed since review",
    partlyViewed: "Partly viewed", skipped: "Skipped", unviewed: "Unreviewed",
}

/** Tab progress uses the displayed detail's local review state. The total
 * count still includes skipped and unlisted files; neither implies viewed. */
export function fileTabStatus(
    detail: Pick<PullRequestDetail, "files" | "unlistedFiles" | "headCommit" | "baseCommit"> | null,
    progress: ReviewProgress | null,
) {
    if (!detail) {
        return null
    }
    const count = detail.files.length + detail.unlistedFiles
    const context = [`${count} changed ${count === 1 ? "file" : "files"}`]
    if (detail.unlistedFiles > 0) {
        context.push(`${detail.unlistedFiles} ${detail.unlistedFiles === 1 ? "file" : "files"} not listed`)
    }
    if (!progress || progress.headCommit !== detail.headCommit ||
        progress.baseCommit !== detail.baseCommit || progress.total !== detail.files.length) {
        return { count, label: [...context, "Review progress unavailable"].join("; "), review: null }
    }
    const words = progressWords(progress)
    const total = progress.total - progress.skipped
    return {
        count,
        label: [...context, words?.viewed, words?.skipped, words?.changed, words?.since].filter(Boolean).join("; "),
        review: total > 0 ? {
            completed: progress.viewed,
            total,
            complete: progress.viewed === total && detail.unlistedFiles === 0,
        } : null,
    }
}

export function progressFilter(filter: FileFilter): boolean {
    return filter !== "all" && filter !== "discussion"
}
export function reviewFileList(
    files: readonly DiffFile[],
    progress: ReviewProgress | null,
    threads: readonly ReviewThread[],
    query: string,
    filter: FileFilter,
) {
    const states = new Map(progress?.files.map((file) => [file.path, file.state]))
    const byFile = splitThreads(threads, files).byFile
    const candidates = new Set<string>()
    const remaining = new Set<string>()
    const badges = new Map<string, string>()
    for (const file of files) {
        const key = fileKey(file)
        const state = states.get(key)
        const count = byFile.get(key)?.length ?? 0
        const label = state ? REVIEW_LABELS[state] : "Review progress unknown"
        badges.set(key, `${label}${count ? ` · ${count} discussion${count === 1 ? "" : "s"}` : ""}`)
        const unreviewed = state !== undefined && state !== "viewed" && state !== "skipped"
        const matches = filter === "all" ||
            (filter === "discussion" && count > 0) ||
            (filter === "unreviewed" && unreviewed) ||
            (filter === "changed" && state === "changedSinceViewed") ||
            (filter === "skipped" && state === "skipped")
        if (matches && key.toLowerCase().includes(query.trim().toLowerCase())) {
            candidates.add(key)
            if (unreviewed) remaining.add(key)
        }
    }
    return { candidates, remaining, badges }
}

export function orderedReviewThreads(threads: readonly ReviewThread[], files: readonly DiffFile[]): readonly ReviewThread[] {
    const split = splitThreads(threads, files)
    return files.flatMap((file) => split.byFile.get(fileKey(file)) ?? []).concat(split.unlisted)
}

export function nextCandidate(order: readonly string[], candidates: ReadonlySet<string>, current: string | null): string | null {
    const start = current === null ? -1 : order.indexOf(current)
    for (let step = 1; step <= order.length; step++) {
        const candidate = order[(start + step) % order.length]
        if (candidate !== current && candidates.has(candidate)) return candidate
    }
    return null
}

export function groupedChecks(checks: readonly PullRequestCheck[]) {
    const rank = (check: PullRequestCheck) =>
        check.state === "failing" ? 0 : check.state === "pending" ? 1 : check.state === "passing" ? 3 : 2
    return {
        other: checks.filter((check) => check.state !== "passing").sort((a, b) => rank(a) - rank(b)),
        passing: checks.filter((check) => check.state === "passing"),
    }
}

export function checkSummary(checks: readonly PullRequestCheck[]): string {
    if (checks.length === 0) return "No checks reported"
    const failed = checks.filter((check) => check.state === "failing").length
    const pending = checks.filter((check) => check.state === "pending").length
    const passed = checks.filter((check) => check.state === "passing").length
    const other = checks.length - failed - pending - passed
    return [
        failed ? `${failed} failed` : null, pending ? `${pending} pending` : null,
        passed ? `${passed} passed` : null, other ? `${other} other` : null,
    ].filter(Boolean).join(" · ")
}

export type PanelFilter = "all" | "failing" | "conflicts" | "drafts" | "requested"
export function matchesPanel(pr: PullRequestSummary, query: string, filter: PanelFilter, requested: boolean): boolean {
    const text = [pr.title, pr.repoFullName, `#${pr.id}`, pr.sourceBranch, pr.destinationBranch].join("\n")
    return text.toLowerCase().includes(query.trim().toLowerCase()) &&
        (filter === "all" || (filter === "failing" && pr.checks === "failing") ||
        (filter === "conflicts" && pr.conflicting === true) ||
        (filter === "drafts" && pr.draft === true) || (filter === "requested" && requested))
}

export function panelRowKey(row: PullRequestSummary): string {
    return `${(row.repoFullName || row.url).replace(/[A-Z]/g, (letter) => letter.toLowerCase())}#${row.id}`
}

export function stablePanelRows(previous: readonly PullRequestSummary[], next: readonly PullRequestSummary[]): PullRequestSummary[] {
    const remaining = new Map(next.map((row) => [panelRowKey(row), row]))
    const rows: PullRequestSummary[] = []
    for (const row of previous) {
        const updated = remaining.get(panelRowKey(row))
        if (updated) {
            rows.push(updated)
            remaining.delete(panelRowKey(row))
        }
    }
    return rows.concat([...remaining.values()])
}
