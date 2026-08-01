import { useEffect, useId, useRef, useState } from "react"
import { useDarkScheme } from "../hooks/useDarkScheme"

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

function token(styles: CSSStyleDeclaration, name: string): string {
    return styles.getPropertyValue(name).trim()
}

/**
 * Map the design tokens onto Mermaid's `base` theme so a diagram reads as part
 * of the surrounding surface rather than as stock Mermaid. Read live off
 * `:root` rather than duplicated here — no literal colours belong in this file.
 */
function themeVariables() {
    const styles = getComputedStyle(document.documentElement)
    return {
        background: token(styles, "--surface"),
        primaryColor: token(styles, "--surface-2"),
        primaryTextColor: token(styles, "--text"),
        primaryBorderColor: token(styles, "--accent"),
        secondaryColor: token(styles, "--surface-3"),
        tertiaryColor: token(styles, "--surface-3"),
        mainBkg: token(styles, "--surface-2"),
        nodeBorder: token(styles, "--accent"),
        clusterBkg: token(styles, "--bg"),
        clusterBorder: token(styles, "--border"),
        lineColor: token(styles, "--border-strong"),
        textColor: token(styles, "--text"),
        titleColor: token(styles, "--text"),
        edgeLabelBackground: token(styles, "--surface"),
        fontFamily: token(styles, "--font-mono"),
        fontSize: token(styles, "--text-md"),
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
                    themeVariables: themeVariables(),
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
            <div className="mermaid-block mermaid-block--error">
                <p className="mermaid-block__note">
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
