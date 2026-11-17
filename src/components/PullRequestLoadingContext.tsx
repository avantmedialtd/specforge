import {
    createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState,
    type ReactNode,
} from "react"
import { usePrContentLoading } from "../hooks/usePrContentLoading"
import {
    PrContentLoading, contentIsVisible, keepContentPlace, observeContent,
    type PrReadKind, type PrReadState,
} from "../prContentLoading"
import type { PrContentLoading as Preferences } from "../types"
import type { PullRequestReference } from "../types"

interface LoadingContext {
    controller: PrContentLoading
    preferences: Preferences | null
}
const Context = createContext<LoadingContext | null>(null)
const Visible = createContext(true)
export const PublishedPrImageBody = createContext<{ reference: PullRequestReference; digest: string } | null>(null)
export const PrImageLinkRetries = createContext<((id: string, control: ReactNode | null) => void) | null>(null)

export function PullRequestLoadingProvider({ children, bodies }: { children: ReactNode; bodies?: ReadonlySet<string> }) {
    const { preferences } = usePrContentLoading()
    const controller = useMemo(() => new PrContentLoading(), [])
    useLayoutEffect(() => {
        controller.activate()
        const visibility = () => controller.policy(preferences, document.visibilityState !== "hidden")
        visibility()
        document.addEventListener("visibilitychange", visibility)
        return () => document.removeEventListener("visibilitychange", visibility)
    }, [controller, preferences])
    useEffect(() => () => controller.dispose(), [controller])
    useLayoutEffect(() => { if (bodies) { controller.retainPublishedBodies(bodies) } }, [controller, bodies])
    const value = useMemo(() => ({ controller, preferences }), [controller, preferences])
    return <Context.Provider value={value}>{children}</Context.Provider>
}

export function PullRequestLoadingScope({ visible, children }: { visible: boolean; children: ReactNode }) {
    const parent = useContext(Visible)
    return <Visible.Provider value={parent && visible}>{children}</Visible.Provider>
}

export function usePullRequestLoading() { return useContext(Context) }

/// A placeholder's lease on shared work. Unchanged source keys retain results
/// across tab/preference/collapse changes; replacement keys receive no old result.
export function useNearPrRead<T>(key: string, kind: PrReadKind, load: () => Promise<T>, allowed = true) {
    const context = useContext(Context)
    const visible = useContext(Visible)
    const element = useRef<HTMLSpanElement>(null)
    const latest = useRef({ load, visible, allowed })
    latest.current = { load, visible, allowed }
    const restore = useRef<(() => void) | null>(null)
    const [held, setHeld] = useState<{ key: string; state: PrReadState<T> }>({ key, state: { status: "idle" } })
    const lease = useRef<{ refresh: () => void; read: () => Promise<T> } | null>(null)
    const controller = context?.controller
    useEffect(() => {
        const node = element.current
        if (!controller || !node) { return }
        const work = controller.watch(key, kind, () => latest.current.load(),
            () => latest.current.allowed && latest.current.visible && contentIsVisible(node),
            state => {
                if (state.status === "ready") { restore.current = keepContentPlace(node) }
                setHeld({ key, state })
            })
        lease.current = work
        const off = observeContent(node, work.near)
        // Native <details> toggles do not render React. Refresh its descendants'
        // eligibility when a minimized comment is opened/closed.
        const toggle = () => { work.near(contentIsVisible(node)) }
        document.addEventListener("toggle", toggle, true)
        return () => {
            off(); work.dispose(); lease.current = null
            document.removeEventListener("toggle", toggle, true)
        }
    }, [controller, key, kind])
    useLayoutEffect(() => { lease.current?.refresh() }, [visible, context?.preferences])
    useLayoutEffect(() => { restore.current?.(); restore.current = null })
    const retry = useCallback(() => { void lease.current?.read().catch(() => undefined) }, [])
    return {
        ref: element,
        state: held.key === key ? held.state : { status: "idle" } as PrReadState<T>,
        retry,
        preferences: context?.preferences ?? null,
    }
}
