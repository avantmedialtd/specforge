import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"

// `src/api.ts` cannot be imported here — it reaches for the Tauri runtime at
// module scope — so the surface is asserted against the source text. That is
// the right granularity anyway: what matters is that no wrapper EXISTS to be
// called, not what one would do if it did.
const API_SOURCE = readFileSync(
    fileURLToPath(new URL("./api.ts", import.meta.url)),
    "utf8",
)

describe("the tree's collapse-state wrappers are gone", () => {
    // Top-level disclosure is session-only (design D6), so a reveal has no
    // settings API left to call — which is what makes `view-routing`'s
    // *Navigation Reveal Is Transient* ("no settings write is performed as a
    // result of the reveal") true by construction rather than by review.
    //
    // The Rust handlers, the Tauri dispatch arms and the settings fields stay
    // in place on purpose, so an existing settings file still parses; this
    // pins only that the FRONTEND can no longer reach them.
    test.each([
        "getCollapsedTreeNodeIds",
        "setCollapsedTreeNodeIds",
        "getExpandedTreeNodeIds",
        "setExpandedTreeNodeIds",
    ])("%s is not exported from src/api.ts", (name) => {
        expect(API_SOURCE).not.toContain(`export async function ${name}`)
        expect(API_SOURCE).not.toContain(`export function ${name}`)
    })

    test.each([
        "get_collapsed_tree_node_ids",
        "set_collapsed_tree_node_ids",
        "get_expanded_tree_node_ids",
        "set_expanded_tree_node_ids",
    ])("the %s command is never invoked", (command) => {
        expect(API_SOURCE).not.toContain(`invokeLogged<string[]>("${command}")`)
        expect(API_SOURCE).not.toContain(`invokeLogged<void>("${command}"`)
    })
})
