import { NearViewQueue } from "./nearViewQueue"
import type { PrContentLoading as Preferences } from "./types"

export type PrReadKind = "image" | "diff"

/// Local progress reconciliation is coalesced, with a final read after a load
/// that finished while the previous reconciliation was outstanding.
export function prProgressReconciler(read: () => Promise<unknown>) {
    let running = false, again = false, disposed = false
    return {
        activate() { disposed = false },
        request() {
            if (disposed) { return }
            again = true
            if (running) { return }
            running = true
            void (async () => {
                try {
                    while (again) {
                        if (disposed) { break }
                        again = false
                        await read().catch(() => undefined)
                    }
                } finally { running = false }
            })()
        },
        dispose() { disposed = true; again = false },
    }
}
export type PrReadState<T> =
    | { status: "idle" }
    | { status: "loading" }
    | { status: "ready"; value: T }
    | { status: "failed" }

const IDLE: PrReadState<never> = { status: "idle" }
const LOADING: PrReadState<never> = { status: "loading" }

interface Subscriber {
    near: boolean
    eligible: () => boolean
    changed: (state: PrReadState<unknown>) => void
}
interface Work {
    kind: PrReadKind
    load: () => Promise<unknown>
    state: PrReadState<unknown>
    promise?: Promise<unknown>
    subscribers: Set<Subscriber>
}

/// One presentation, one mixed queue. Keys include a model or published-body
/// generation; no bytes are shared with another presentation or persisted.
export class PrContentLoading {
    private preferences: Preferences | null = null
    private visible = false
    private disposed = false
    private readonly work = new Map<string, Work>()
    private readonly models = new WeakMap<object, number>()
    private nextModel = 0
    private queue = this.newQueue()

    private newQueue(): NearViewQueue {
        return new NearViewQueue(key => this.run(key), 2, key => this.eligible(key))
    }

    /// React StrictMode replays mount effects with the same memoized owner.
    /// A new queue never inherits a disposed queue's pending completions.
    activate(): void {
        if (this.disposed) { this.queue = this.newQueue(); this.disposed = false }
    }

    modelKey(model: object): string {
        let number = this.models.get(model)
        if (number === undefined) {
            number = ++this.nextModel
            this.models.set(model, number)
        }
        return `model:${number}`
    }

    policy(preferences: Preferences | null, visible: boolean): void {
        this.preferences = preferences
        this.visible = visible
        this.refresh()
    }

    watch<T>(
        key: string,
        kind: PrReadKind,
        load: () => Promise<T>,
        eligible: () => boolean,
        changed: (state: PrReadState<T>) => void = () => {},
    ) {
        let work = this.work.get(key)
        if (!work) {
            work = { kind, load, state: IDLE, subscribers: new Set() }
            this.work.set(key, work)
        }
        const subscriber: Subscriber = {
            near: false, eligible, changed: state => changed(state as PrReadState<T>),
        }
        work.subscribers.add(subscriber)
        subscriber.changed(work.state)
        return {
            near: (near: boolean) => {
                subscriber.near = near
                this.refreshKey(key)
            },
            refresh: () => this.refreshKey(key),
            read: () => this.run(key, true) as Promise<T>,
            dispose: () => {
                work.subscribers.delete(subscriber)
                this.refreshKey(key)
            },
        }
    }

    private eligible(key: string): boolean {
        const work = this.work.get(key)
        return !this.disposed && this.visible && this.preferences !== null && !!work
            && (work.kind === "image" ? this.preferences.autoShowImages : this.preferences.autoLoadLargeDiffs)
            && [...work.subscribers].some(s => s.near && s.eligible())
    }

    private refreshKey(key: string): void {
        if (this.eligible(key)) { this.queue.near(key) }
        else { this.queue.left(key) }
    }

    refresh(): void {
        for (const key of this.work.keys()) { this.refreshKey(key) }
    }

    private run(key: string, retry = false): Promise<unknown> {
        const work = this.work.get(key)
        if (!work || this.disposed) { return Promise.resolve() }
        if (work.promise) { return work.promise }
        if (work.state.status === "ready" && !retry) { return Promise.resolve(work.state.value) }
        const publish = (state: PrReadState<unknown>) => {
            if (this.disposed || this.work.get(key) !== work) { return }
            work.state = state
            for (const s of work.subscribers) { s.changed(state) }
        }
        // Set the promise before invoking a loader which may synchronously
        // cause a render/registration. Every occurrence shares that flight.
        let resolve!: (value: unknown) => void
        let reject!: (error: unknown) => void
        const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no })
        work.promise = promise
        publish(LOADING)
        try { void work.load().then(resolve, reject) } catch (error) { reject(error) }
        void promise.then(
            value => publish({ status: "ready", value }),
            () => publish({ status: "failed" }),
        ).finally(() => { work.promise = undefined })
        return promise
    }

    /// Revokes an obsolete source generation, even while its promise settles.
    forget(key: string): void {
        this.queue.left(key)
        this.work.delete(key)
    }

    retainPublishedBodies(digests: ReadonlySet<string>): void {
        for (const key of this.work.keys()) {
            if (key.startsWith("embedded:") && !digests.has(key.split(":")[1])) { this.forget(key) }
        }
    }

    dispose(): void {
        this.disposed = true
        this.queue.dispose()
        this.work.clear()
    }
}

/// The actual nearest scrollport, not a guessed page/window viewport.
export function contentScrollPort(element: HTMLElement | null): HTMLElement | null {
    for (let parent = element?.parentElement; parent; parent = parent.parentElement) {
        const overflow = getComputedStyle(parent).overflowY
        if (overflow === "auto" || overflow === "scroll") { return parent }
    }
    return null
}

/// Rechecked on dequeue, after observer delivery and React visibility changes.
export function contentIsVisible(element: HTMLElement): boolean {
    if (!element.isConnected || document.visibilityState === "hidden") { return false }
    for (let node: HTMLElement | null = element; node; node = node.parentElement) {
        if (node.hidden || node.getAttribute("aria-hidden") === "true") { return false }
        if (node.tagName === "DETAILS" && !node.hasAttribute("open")) { return false }
        const style = getComputedStyle(node)
        if (style.display === "none" || style.visibility === "hidden") { return false }
    }
    const rect = element.getBoundingClientRect()
    const port = contentScrollPort(element)
    const bounds = port?.getBoundingClientRect()
    const height = port?.clientHeight ?? window.innerHeight
    const top = bounds?.top ?? 0
    return rect.height > 0 && rect.bottom >= top - height && rect.top <= top + height * 2
}

export function observeContent(element: HTMLElement, near: (value: boolean) => void): () => void {
    if (typeof IntersectionObserver === "undefined") { return () => {} }
    const port = contentScrollPort(element)
    const margin = port?.clientHeight ?? window.innerHeight
    const observer = new IntersectionObserver(entries => {
        for (const entry of entries) { near(entry.isIntersecting) }
    }, { root: port, rootMargin: `${margin}px 0px` })
    observer.observe(element)
    return () => observer.disconnect()
}

/// Matches the service's syntactic identity, never network authority.
export function canonicalPrImageSource(source: string): string | null {
    if (!/^https:\/\//i.test(source) || source.trim() !== source
        || [...source].some(c => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127 || c === "\\")) { return null }
    if (source.slice(8).split(/[/?#]/, 1)[0].includes("@")) { return null }
    try {
        const url = new URL(source)
        const host = url.hostname.replace(/\.$/, "")
        if (url.protocol !== "https:" || url.username || url.password || url.port
            || host.startsWith("[") || /^\d+\.\d+\.\d+\.\d+$/.test(host)
            || host.length > 253 || !host.includes(".")
            || !host.split(".").every(label => /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/i.test(label))
            || ["localhost", "local", "internal", "lan", "home", "home.arpa", "onion", "alt"]
                .some(suffix => host === suffix || host.endsWith(`.${suffix}`))) { return null }
        url.hostname = host
        url.hash = ""
        return url.href
    } catch { return null }
}

/// Capture a stable readable DOM neighbour before inserting a preview. Never
/// focuses, rewrites selection or relies on cumulative height estimates.
export function keepContentPlace(element: HTMLElement, focusedOnly = false): (() => void) | null {
    const port = contentScrollPort(element)
    if (!port) { return null }
    const top = port.getBoundingClientRect().top
    const active = document.activeElement
    const focused = active instanceof HTMLElement && port.contains(active)
        && (active.tagName === "TEXTAREA" || active.isContentEditable) ? active : null
    const selection = document.getSelection?.()
    const selected = selection && !selection.isCollapsed
        ? selection.anchorNode?.parentElement?.closest<HTMLElement>(".diff-row, .diff-split-row, p, li") ?? null : null
    const preferred = focused ?? (selected && port.contains(selected) ? selected : null)
    const candidates = port.querySelectorAll<HTMLElement>(
        ".markdown-view p, .markdown-view h2, .markdown-view li, .diff-file-header, [data-line], textarea",
    )
    const anchor = preferred ?? (focusedOnly ? null : [...candidates].find(node => !node.contains(element) && !element.contains(node)
        && node.getBoundingClientRect().top >= top && node.getBoundingClientRect().height > 0)
    )
    if (!anchor) { return null }
    const offset = anchor.getBoundingClientRect().top - top
    return () => {
        if (anchor.isConnected) { port.scrollTop += anchor.getBoundingClientRect().top - port.getBoundingClientRect().top - offset }
    }
}
