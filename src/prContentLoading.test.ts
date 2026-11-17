import { expect, test } from "bun:test"
import { PrContentLoading } from "./prContentLoading"

async function tick() { for (let i = 0; i < 8; i++) { await Promise.resolve() } }
const ON = { autoShowImages: true, autoLoadLargeDiffs: true }

test("unknown preferences, hidden scopes and mixed read kinds share one bounded queue", async () => {
    const controller = new PrContentLoading()
    const started: string[] = []
    const ends = new Map<string, () => void>()
    let visible = true
    const watch = (key: string, kind: "image" | "diff") => controller.watch(key, kind, () => {
        started.push(key)
        return new Promise<void>(resolve => { ends.set(key, resolve) })
    }, () => visible)
    const a = watch("image", "image"), b = watch("text", "diff"), c = watch("embedded", "image")
    a.near(true); b.near(true); c.near(true)
    await tick()
    expect(started).toEqual([])
    controller.policy(ON, true)
    await tick()
    expect(started).toEqual(["image", "text"])
    visible = false
    ends.get("image")!()
    await tick()
    expect(started).toHaveLength(2)
    visible = true
    c.refresh()
    await tick()
    expect(started).toEqual(["image", "text", "embedded"])
    controller.dispose()
})

test("duplicate sources share bytes, off/on retains results, failures need explicit retry, and late results are fenced", async () => {
    const controller = new PrContentLoading()
    controller.policy(ON, true)
    let attempts = 0, last: unknown
    const load = async () => { attempts++; return "checked bytes" }
    const a = controller.watch("body:source", "image", load, () => true, state => { last = state })
    const b = controller.watch("body:source", "image", load, () => true)
    a.near(true); b.near(true)
    await tick()
    expect(attempts).toBe(1)
    expect(last).toEqual({ status: "ready", value: "checked bytes" })
    controller.policy(null, false); controller.policy(ON, true)
    await tick()
    expect(attempts).toBe(1)
    let failureAttempts = 0
    const failed = controller.watch("failure", "diff", () => {
        failureAttempts++
        return Promise.reject(new Error("held"))
    }, () => true)
    failed.near(true); await tick()
    failed.near(false); failed.near(true)
    controller.policy(ON, false); controller.policy(ON, true)
    await tick()
    expect(failureAttempts).toBe(1)
    await failed.read().catch(() => {})
    expect(failureAttempts).toBe(2)
    let complete!: (value: string) => void
    let oldUpdates = 0
    const old = controller.watch("old-body", "image", () => new Promise<string>(r => { complete = r }),
        () => true, () => { oldUpdates++ })
    old.near(true); await tick()
    const before = oldUpdates
    controller.forget("old-body")
    complete("obsolete")
    await tick()
    expect(oldUpdates).toBe(before)
    const model = {}
    expect(controller.modelKey(model)).toBe(controller.modelKey(model))
    expect(controller.modelKey({})).not.toBe(controller.modelKey(model))
    controller.dispose()
})

test("edited published bodies and closed presentations fence late bytes", async () => {
    const controller = new PrContentLoading()
    controller.policy(ON, true)
    let complete!: (value: string) => void
    const states: unknown[] = []
    const work = controller.watch("embedded:old:source", "image",
        () => new Promise<string>(resolve => { complete = resolve }), () => true,
        state => { states.push(state) })
    work.near(true)
    await tick()
    controller.retainPublishedBodies(new Set(["new"]))
    const before = states.length
    complete("obsolete bytes")
    await tick()
    expect(states).toHaveLength(before)
    controller.dispose()
    controller.policy(ON, true)
    work.near(true)
    await tick()
    expect(states).toHaveLength(before)
})
