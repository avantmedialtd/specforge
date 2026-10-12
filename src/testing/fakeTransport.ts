// The web transport, faked for `bun test`: `fetch` answers `POST /api/invoke`
// from a handler, and an `EventSource` stands in for `/api/events`, so a test
// can render real hooks (on `fakeDom.ts`) and emit server events to them.

import { act } from "react"
import { __resetEventStreamForTests } from "../eventStream"

type FrameListener = (event: { data: string }) => void

export class FakeEventSource {
    static current: FakeEventSource | null = null
    readyState = 1
    private listeners = new Map<string, Set<FrameListener>>()
    constructor() {
        FakeEventSource.current = this
    }
    addEventListener(type: string, listener: FrameListener) {
        let set = this.listeners.get(type)
        if (!set) this.listeners.set(type, (set = new Set()))
        set.add(listener)
    }
    removeEventListener(type: string, listener: FrameListener) {
        this.listeners.get(type)?.delete(listener)
    }
    close() {}
    /// Delivers one server event, with `payload` as its JSON data.
    emit(type: string, payload?: unknown) {
        const data = payload === undefined ? "" : JSON.stringify(payload)
        for (const listener of [...(this.listeners.get(type) ?? [])]) listener({ data })
    }
}

/// Answers one command: its result, or a promise of it.
export type CommandHandler = (command: string, args: Record<string, unknown>) => unknown

/// Installs the fake transport, with `IS_REACT_ACT_ENVIRONMENT` on; returns the
/// commands seen and the function that restores the globals.
export function installFakeTransport(handler: CommandHandler): {
    calls: string[]
    uninstall: () => void
} {
    const g = globalThis as unknown as Record<string, unknown>
    const names = ["fetch", "EventSource", "IS_REACT_ACT_ENVIRONMENT"]
    const saved = names.map((name) => [name, name in g, g[name]] as const)
    const calls: string[] = []
    g.IS_REACT_ACT_ENVIRONMENT = true
    g.EventSource = FakeEventSource
    g.fetch = async (_url: string, init: { body: string }) => {
        const { command, args } = JSON.parse(init.body) as {
            command: string
            args: Record<string, unknown>
        }
        calls.push(command)
        const body: unknown = await handler(command, args)
        return new Response(JSON.stringify(body ?? null), { status: 200 })
    }
    __resetEventStreamForTests()
    FakeEventSource.current = null
    return {
        calls,
        uninstall() {
            for (const [name, had, value] of saved) {
                if (had) g[name] = value
                else delete g[name]
            }
            __resetEventStreamForTests()
        },
    }
}

/// Lets timers (a coalescer's `setTimeout(0)`) and fetch replies run, each
/// round inside its own `act`, which runs the effects and renders the round
/// started only once it ends.
export async function settle(rounds = 4): Promise<void> {
    for (let round = 0; round < rounds; round += 1) {
        await act(async () => {
            for (let i = 0; i < 5; i += 1) {
                await new Promise<void>((resolve) => {
                    setTimeout(resolve, 0)
                })
            }
        })
    }
}
