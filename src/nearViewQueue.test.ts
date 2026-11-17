import { expect, test } from "bun:test"
import { NearViewQueue } from "./nearViewQueue"

async function tick() { for (let i = 0; i < 5; i++) { await Promise.resolve() } }

test("mixed work rechecks eligibility, attempts only once and retries explicitly", async () => {
    const started: string[] = []
    const ends = new Map<string, () => void>()
    const allowed = new Set(["image", "text", "embedded", "hidden"])
    const queue = new NearViewQueue(key => {
        started.push(key)
        return new Promise<void>(resolve => { ends.set(key, resolve) })
    }, 2, key => allowed.has(key))
    for (const key of allowed) { queue.near(key) }
    allowed.delete("hidden")
    queue.left("embedded")
    ends.get("image")!(); await tick()
    expect(started).toEqual(["image", "text"])
    queue.near("image")
    expect(started).toHaveLength(2)
    queue.retry("image")
    expect(started).toEqual(["image", "text", "image"])
    queue.retry("image")
    expect(queue.snapshot().waiting).toEqual([])
})

test("paused and disposed queues cannot be pumped by old completions", async () => {
    const started: string[] = []
    const ends: (() => void)[] = []
    const queue = new NearViewQueue(key => {
        started.push(key); return new Promise<void>(resolve => { ends.push(resolve) })
    }, 1)
    queue.near("first"); queue.near("second")
    queue.pause(true)
    ends[0](); await tick()
    expect(started).toEqual(["first"])
    queue.pause(false)
    expect(started).toEqual(["first", "second"])
    queue.near("third")
    queue.dispose()
    ends[1](); await tick()
    queue.retry("fourth")
    expect(started).toEqual(["first", "second"])
})

test("a replacement generation is independent of disposed promises and failed attempts", async () => {
    let complete!: () => void
    const old = new NearViewQueue(() => new Promise<void>(resolve => { complete = resolve }), 1)
    old.near("same-key")
    old.near("obsolete")
    old.dispose()
    let attempts = 0
    const replacement = new NearViewQueue(() => {
        attempts++
        return Promise.reject(new Error("deferred"))
    }, 1)
    replacement.near("same-key")
    await tick()
    replacement.near("same-key")
    replacement.pause(true)
    replacement.pause(false)
    complete()
    await tick()
    expect(old.snapshot().waiting).toEqual([])
    expect(attempts).toBe(1)
    replacement.retry("same-key")
    await tick()
    expect(attempts).toBe(2)
})
