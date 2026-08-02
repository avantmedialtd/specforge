import { useEffect, useId, useRef, useState } from "react"
import { useDarkScheme } from "../hooks/useDarkScheme"
import { readToken } from "../theme"

/** The mermaid module's default export, resolved lazily. */
type MermaidApi = typeof import("mermaid").default

/**
 * Mermaid drags d3 and dagre in behind it (~2.8MB). Import it on first use so
 * Vite splits it into its own chunk and artifacts with no diagrams never pay
 * for it. The promise is module-level, so concurrent blocks share one load.
 */
let mermaidModule: Promise<MermaidApi> | null = null

function loadMermaid(): Promise<MermaidApi> {
    if (!mermaidModule) {
        mermaidModule = import("mermaid")
            .then((module) => module.default)
            .catch((error: unknown) => {
                // Don't memoise a rejection: a single failed chunk fetch would
                // otherwise poison every diagram for the rest of the session.
                mermaidModule = null
                throw error
            })
    }
    return mermaidModule
}

/**
 * Map the design tokens onto Mermaid's `base` theme so a diagram reads as part
 * of the surrounding surface rather than as stock Mermaid. Read live off
 * `:root` rather than duplicated here — no literal colours belong in this file.
 * `isDark` sets the theme's `darkMode` flag so every variable the base theme
 * derives (rather than receives as-is) is derived toward the active scheme
 * instead of an assumed light palette.
 */
function themeVariables(isDark: boolean) {
    const styles = getComputedStyle(document.documentElement)
    return {
        darkMode: isDark,
        background: readToken("--surface", styles),
        primaryColor: readToken("--surface-2", styles),
        primaryTextColor: readToken("--text", styles),
        primaryBorderColor: readToken("--accent", styles),
        secondaryColor: readToken("--surface-3", styles),
        tertiaryColor: readToken("--surface-3", styles),
        mainBkg: readToken("--surface-2", styles),
        nodeBorder: readToken("--accent", styles),
        clusterBkg: readToken("--bg", styles),
        clusterBorder: readToken("--border", styles),
        lineColor: readToken("--border-strong", styles),
        textColor: readToken("--text", styles),
        titleColor: readToken("--text", styles),
        edgeLabelBackground: readToken("--surface", styles),
        fontFamily: readToken("--font-mono", styles),
        fontSize: readToken("--text-md", styles),
        // rowOdd/rowEven are the v11 `erBox` renderer's actual fill
        // variables for ER attribute rows — the documented
        // attributeBackgroundColorOdd/Even are dead in this code path.
        // Pinned to tokens rather than left to the engine's derive(mainBkg)
        // math, so row contrast is a deliberate, legible choice in both
        // schemes rather than a computed colour at the engine's discretion.
        rowOdd: readToken("--surface-2", styles),
        rowEven: readToken("--surface-3", styles),
    }
}

interface MermaidBlockProps {
    source: string
}

/**
 * Renders one ```mermaid fence as a diagram. Invalid source falls back to the
 * raw text rather than to Mermaid's own error graphic, so a half-written
 * diagram never disrupts the rest of the artifact.
 */
export function MermaidBlock({ source }: MermaidBlockProps) {
    // useId() yields ":r1:" — the colons are invalid in the `#id` selectors
    // Mermaid builds internally, so keep only the alphanumerics.
    const baseId = `mermaid-${useId().replace(/[^a-zA-Z0-9]/g, "")}`
    const attempt = useRef(0)

    const [svg, setSvg] = useState<string | null>(null)
    const [failed, setFailed] = useState(false)

    // Mermaid bakes its palette into the SVG at render time, so unlike CSS a
    // rendered diagram will not follow a scheme change on its own. Track the
    // scheme and let it re-key the render effect below.
    const isDark = useDarkScheme()

    useEffect(() => {
        let ignore = false

        const draw = async () => {
            try {
                const mermaid = await loadMermaid()
                if (ignore) return

                // Re-initialised per render: `themeVariables` are resolved from
                // the tokens of whichever scheme is active right now.
                mermaid.initialize({
                    startOnLoad: false,
                    securityLevel: "strict",
                    suppressErrorRendering: true,
                    theme: "base",
                    themeVariables: themeVariables(isDark),
                })

                // parse() validates without touching the DOM, and with
                // suppressErrors it resolves falsy instead of throwing.
                const parsed = await mermaid.parse(source, {
                    suppressErrors: true,
                })
                if (ignore) return
                if (!parsed) {
                    setSvg(null)
                    setFailed(true)
                    return
                }

                // A fresh id per attempt: StrictMode double-invokes effects and
                // Mermaid keys its scratch DOM node off this id.
                attempt.current += 1
                const { svg: rendered } = await mermaid.render(
                    `${baseId}-${attempt.current}`,
                    source,
                )
                if (ignore) return

                setFailed(false)
                setSvg(rendered)
            } catch {
                if (ignore) return
                setSvg(null)
                setFailed(true)
            }
        }

        void draw()
        return () => {
            ignore = true
        }
    }, [source, isDark, baseId])

    if (failed) {
        return (
            <div className="fence-block--error">
                <p className="fence-block__note">
                    Couldn’t render this diagram — showing its source.
                </p>
                <pre>
                    <code>{source}</code>
                </pre>
            </div>
        )
    }

    if (svg === null) {
        return (
            <div
                className="mermaid-block mermaid-block--pending"
                aria-busy="true"
            />
        )
    }

    return (
        <div
            className="mermaid-block"
            // Mermaid runs the SVG through DOMPurify at securityLevel "strict".
            dangerouslySetInnerHTML={{ __html: svg }}
        />
    )
}
