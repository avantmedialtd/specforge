import {
    useCallback,
    useEffect,
    useId,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
} from "react"
import type { CSSProperties } from "react"
import { useDarkScheme } from "../hooks/useDarkScheme"
import { useReducedFigure } from "../hooks/useReducedFigure"
import { readToken } from "../theme"
import { floorWidth } from "./figureFloor"
import { FigureLightbox } from "./FigureLightbox"
import { Maximize } from "./icons"

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

/** The smallest authored label size in `svg`, in its own user units — what the
 * labels measure before the diagram is scaled to fit. `getComputedStyle` on a
 * node inside a scaled SVG reports the authored value, not the visual one,
 * which is exactly what the floor has to be computed from. An unmeasurable
 * result is left for `floorWidth` to substitute.
 *
 * The SMALLEST, because the floor's promise is that no label falls below
 * MIN_LABEL_PX — a diagram is only as legible as its least legible text.
 * The cost is a diagram carrying one unusually small annotation floors at its
 * natural width and scrolls rather than fitting; that is the conservative
 * direction, and the honest one.
 *
 * `.label` is deliberately not queried. Mermaid v11 wraps each label in a
 * `<g class="label">` containing the `.nodeLabel` span, and a selector list
 * matches the first element in TREE order rather than trying its branches in
 * order — so including it would measure the wrapper `<g>`, which carries only
 * the SVG root's inherited size. That reads correctly for flowcharts by
 * coincidence (root and label are both the theme size) and wrongly for the
 * diagram types that size text per element, which are exactly the ones the
 * floor has to get right. */
function labelFontPx(svg: SVGSVGElement): number {
    let smallest = Number.NaN
    for (const label of svg.querySelectorAll(".nodeLabel, text")) {
        const size = parseFloat(getComputedStyle(label).fontSize)
        // Written as a negated comparison so a NaN incumbent is replaced (any
        // comparison against NaN is false) without a separate guard.
        if (Number.isFinite(size) && size > 0 && !(smallest <= size)) {
            smallest = size
        }
    }
    return smallest
}

/** The legibility floor for `svg` in CSS pixels, or null when the diagram
 * carries no usable viewBox. The DOM reads live here; the arithmetic they
 * feed lives in `figureFloor.ts`, where it is unit-tested (`spec-browser`:
 * *Mermaid Diagram Rendering*, *Wide Block Containment*). */
function legibilityFloorPx(svg: SVGSVGElement): number | null {
    return floorWidth(svg.viewBox.baseVal?.width ?? Number.NaN, labelFontPx(svg))
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

    // Whether this diagram is currently open in the maximized view. Local to
    // the block rather than hoisted: MarkdownView's memo is a documented
    // correctness prerequisite for the detail pane's equality guard, and a
    // callback prop threaded through it would defeat the shallow comparison
    // silently (design.md: *Decision 1*). Deliberately NOT cleared when
    // `source` or the scheme changes — the lightbox renders from `svg`
    // below, so a re-render flows through it in place.
    const [maximized, setMaximized] = useState(false)
    const closeMaximized = useCallback(() => setMaximized(false), [])

    // Mermaid bakes its palette into the SVG at render time, so unlike CSS a
    // rendered diagram will not follow a scheme change on its own. Track the
    // scheme and let it re-key the render effect below.
    const isDark = useDarkScheme()

    // The frame is the measured element for both the legibility floor and the
    // reduced-figure affordance: `svg` is the identity of what it holds, so a
    // re-themed or replaced diagram re-measures.
    const frameRef = useRef<HTMLDivElement>(null)

    // The floor has to be MEASURED — it depends on the rendered label size,
    // which is only knowable once the diagram is in the document — but it must
    // be APPLIED declaratively, as a custom property React owns. Setting
    // `style.minWidth` on the injected SVG directly does not survive: this
    // component re-renders when `reduced` below flips, and the imperative
    // style is lost with the node React re-establishes from `svg`.
    //
    // useLayoutEffect, not useEffect: a passive effect runs after the browser
    // may already have painted, which shows twice — every wide diagram would
    // paint one frame fitted (below the floor) before snapping wider, and on
    // the live-edit path a newly rendered small diagram would paint stretched
    // to the PREVIOUS diagram's floor, since `min-width` outranks mermaid's
    // inline `max-width`. Both reads here are cheap and layout-independent,
    // so measuring before paint costs nothing (FigureLightbox.tsx measures
    // its fit scale the same way, for the same reason).
    const [floorPx, setFloorPx] = useState<number | null>(null)
    useLayoutEffect(() => {
        const el = frameRef.current?.querySelector<SVGSVGElement>(
            ".mermaid-block > svg",
        )
        setFloorPx(el ? legibilityFloorPx(el) : null)
    }, [svg])

    // Keyed on the floor as well as the source: applying the floor changes the
    // rendered width without resizing the frame (the diagram overflows inside
    // its own block), so nothing else would prompt a re-measure, and the
    // verdict would be the pre-floor one.
    const reduced = useReducedFigure(frameRef, `${svg ?? ""}|${floorPx ?? ""}`)

    // A fresh `{__html}` literal each render is re-assigned to `innerHTML` on
    // every re-render, which rebuilds the SVG and resets `.mermaid-block`'s
    // `scrollLeft` — the scroll surface the legibility floor creates. This
    // component now re-renders on three triggers (`floorPx`, `reduced`,
    // `maximized`), so without a stable object a reader who scrolls a wide
    // diagram right and opens the lightbox returns to find it snapped back to
    // the left edge.
    const markup = useMemo(() => ({ __html: svg ?? "" }), [svg])

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

    // Wrapped rather than restructured: `.mermaid-block` stays the element
    // the SVG is injected into, so every existing rule keyed on it — the
    // block's own spacing and the `> svg` width cap that keeps the inline
    // figure inside the pane (design.md: *Decision 5*) — matches exactly as
    // before. The frame exists only to position the affordance over it.
    return (
        <div
            ref={frameRef}
            className={
                reduced ? "figure-frame figure-frame--reduced" : "figure-frame"
            }
        >
            <div
                className="mermaid-block"
                // The measured floor, handed to CSS as a custom property so the
                // rule that consumes it (`.mermaid-block > svg`) applies to the
                // injected diagram without this component reaching into it.
                style={
                    floorPx === null
                        ? undefined
                        : ({
                              "--figure-floor": `${floorPx}px`,
                          } as CSSProperties)
                }
                // Mermaid runs the SVG through DOMPurify at securityLevel "strict".
                dangerouslySetInnerHTML={markup}
            />
            <button
                type="button"
                className="figure-maximize"
                onClick={() => setMaximized(true)}
                aria-label="Maximize diagram"
                title="Maximize diagram"
            >
                <Maximize width={14} height={14} />
            </button>
            {maximized && (
                <FigureLightbox label="Maximized diagram" onClose={closeMaximized}>
                    <div dangerouslySetInnerHTML={markup} />
                </FigureLightbox>
            )}
        </div>
    )
}
