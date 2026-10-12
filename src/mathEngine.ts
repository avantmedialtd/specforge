// The mathematics engine, loaded on demand (`spec-browser`: *Surfaces and
// Rendering Engines Load on Demand*, *Mathematical Notation Rendering*;
// design D7).
//
// `rehype-katex` and KaTeX's stylesheet are the renderer's heaviest
// dependency after the diagram engine, and most documents hold no maths. So
// `MarkdownView` no longer imports them: it asks `mayContainMath` whether a
// document could hold any, and while the engine is not loaded keeps such a
// document's previous rendering (or nothing) rather than showing its source.
// Each window has its own JavaScript context, so each loads the engine for
// itself: the main window warms it once its first views read has landed, and a
// reader or pull-request window loads it when its document first needs it,
// a local read of a bundled chunk either way.
//
// Both are bundled chunks, so loading needs no network access.

import type rehypeKatex from "rehype-katex"

/// The `rehype-katex` plugin, once loaded.
export type MathPlugin = typeof rehypeKatex

/// How the engine is fetched: the two bundled chunks.
type MathLoader = () => Promise<MathPlugin>

const loadBundled: MathLoader = () =>
    Promise.all([import("rehype-katex"), import("katex/dist/katex.min.css")]).then(
        ([module]) => module.default,
    )

let loader: MathLoader = loadBundled
let plugin: MathPlugin | null = null
let loading: Promise<MathPlugin> | null = null
/// Bumped by a reset, so a load started before it never writes after it.
let generation = 0

/// Loads the engine and its stylesheet, once. Every later call returns the
/// same promise. A failed load (a chunk that could not be read) is not
/// remembered: the promise rejects for everyone waiting on it, and the next
/// call starts a new load. A view that saw the failure stops waiting and
/// renders without the engine (`mathReady`).
export function loadMath(): Promise<MathPlugin> {
    if (loading) return loading
    const started = generation
    loading = loader().then(
        (loaded) => {
            if (started === generation) plugin = loaded
            return loaded
        },
        (err: unknown) => {
            if (started === generation) loading = null
            throw err
        },
    )
    return loading
}

/// Tests only: forgets the loaded engine and any load in flight, and swaps
/// the loader (the bundled one when none is given), since the loaded flag is
/// module state that every test file in one `bun test` process shares.
export function resetMathEngineForTest(next: MathLoader = loadBundled): void {
    generation += 1
    loader = next
    plugin = null
    loading = null
}

/// The engine when it has loaded, else null. Read synchronously by a render,
/// so the first render after `loadMath()` resolves typesets at once.
export function loadedMath(): MathPlugin | null {
    return plugin
}

/// Whether a view may render `content` now: the engine has loaded, the
/// content cannot hold maths, or the engine failed to load for this view, which
/// then renders the maths as code rather than stay blank and stop following
/// edits.
export function mathReady(content: string, failed: boolean): boolean {
    return plugin !== null || failed || !mayContainMath(content)
}

/// A run of three or more backticks or tildes, optional spaces, and `math`:
/// a `math` fence's opening, wherever it stands. Deliberately not anchored to
/// a line start, because a fence inside a list item or a blockquote opens
/// after `- ` or `> `.
const MATH_FENCE = /(?:`{3,}|~{3,})[ \t]*math/

/// Whether `content` could hold mathematics: `$$` anywhere, or a `math`
/// fence's opening anywhere. Under `REMARK_MATH_OPTIONS`
/// (`singleDollarTextMath: false`) those are the only sources of what
/// `rehype-katex` typesets, so this can answer true for a document with no
/// maths (`$$` in prose or code) but never false for one with some.
export function mayContainMath(content: string): boolean {
    return content.includes("$$") || MATH_FENCE.test(content)
}
