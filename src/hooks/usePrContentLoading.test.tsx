import { expect, test } from "bun:test"
import { act, useEffect } from "react"
import { installFakeDom } from "../testing/fakeDom"
import { FakeEventSource, installFakeTransport, settle } from "../testing/fakeTransport"
import { usePrContentLoading } from "./usePrContentLoading"

test("preferences stay unknown until authority, and a late getter cannot reverse a notice", async () => {
    const dom = installFakeDom()
    let answer!: (value: unknown) => void
    const transport = installFakeTransport(() => new Promise(resolve => { answer = resolve }))
    const { createRoot } = await import("react-dom/client")
    let held!: ReturnType<typeof usePrContentLoading>
    function Probe() {
        const state = usePrContentLoading()
        useEffect(() => { held = state })
        return null
    }
    const root = createRoot(dom.document.createElement("div") as unknown as Element)
    try {
        await act(async () => root.render(<Probe />))
        await settle(1)
        expect(held.preferences).toBeNull()
        await act(async () => FakeEventSource.current!.emit("pr-content-loading-changed", {
            autoShowImages: true, autoLoadLargeDiffs: false,
        }))
        await act(async () => answer({ autoShowImages: false, autoLoadLargeDiffs: true }))
        await settle(1)
        expect(held.preferences).toEqual({ autoShowImages: true, autoLoadLargeDiffs: false })
    } finally {
        await act(async () => root.unmount())
        transport.uninstall(); dom.uninstall()
    }
})

test("two clients follow notices, independent saves and failed writes preserve newer authority", async () => {
    const dom = installFakeDom()
    const pair = { autoShowImages: false, autoLoadLargeDiffs: true }
    let fail = false
    const transport = installFakeTransport((command, args) => {
        if (command === "set_pr_content_loading") {
            if (fail) { throw new Error("disk unavailable") }
            pair[args.preference as keyof typeof pair] = args.enabled as boolean
            FakeEventSource.current!.emit("pr-content-loading-changed", pair)
        }
        return { ...pair }
    })
    const { createRoot } = await import("react-dom/client")
    const held: ReturnType<typeof usePrContentLoading>[] = []
    function Probe({ index }: { index: number }) {
        const state = usePrContentLoading()
        useEffect(() => { held[index] = state })
        return null
    }
    const root = createRoot(dom.document.createElement("div") as unknown as Element)
    try {
        await act(async () => root.render(<><Probe index={0} /><Probe index={1} /></>))
        await settle(1)
        await act(async () => held[0].choose("autoShowImages", true))
        expect(held[0].preferences).toEqual({ autoShowImages: true, autoLoadLargeDiffs: true })
        expect(held[1].preferences).toEqual(held[0].preferences)
        fail = true
        await act(async () => held[0].choose("autoLoadLargeDiffs", false))
        expect(held[0].saveErrors.autoLoadLargeDiffs).toContain("Couldn't save")
        expect(held[0].preferences).toEqual(held[1].preferences)
        expect(pair.autoLoadLargeDiffs).toBe(true)
    } finally {
        await act(async () => root.unmount())
        transport.uninstall(); dom.uninstall()
    }
})

test("a failed authoritative read leaves automatic loading disabled", async () => {
    const dom = installFakeDom()
    const transport = installFakeTransport(() => { throw new Error("offline") })
    const { createRoot } = await import("react-dom/client")
    let held!: ReturnType<typeof usePrContentLoading>
    function Probe() {
        const state = usePrContentLoading()
        useEffect(() => { held = state })
        return null
    }
    const root = createRoot(dom.document.createElement("div") as unknown as Element)
    try {
        await act(async () => root.render(<Probe />))
        await settle(1)
        expect(held.preferences).toBeNull()
        expect(held.error).toContain("Could not load")
    } finally {
        await act(async () => root.unmount())
        transport.uninstall(); dom.uninstall()
    }
})
