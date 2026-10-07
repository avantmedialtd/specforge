import {
    Fragment,
    memo,
    useCallback,
    useEffect,
    useId,
    useImperativeHandle,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
    type CSSProperties,
    type FocusEvent,
    type KeyboardEvent,
    type MouseEvent,
    type ReactNode,
    type Ref,
    type RefObject,
} from "react"
import { ChoiceGroup, type ChoiceOption } from "./ChoiceGroup"
import { ChevronDown, ChevronRight } from "./icons"
import {
    contentStateLabel,
    fileKey,
    headerPath,
    navigatorKeyAction,
    navigatorTree,
    notShownInFull,
    notShownInFullLabel,
    sectionBeingRead,
    shownModes,
    statusLabel,
    statusLetter,
    type NavigatorNode,
} from "../diffFiles"
import {
    drawTokens,
    hunkTokens,
    languageFor,
    unifiedSide,
    type DrawnToken,
    type HunkTokens,
    type TokenLine,
} from "../diffHighlight"
import {
    NARROW_FALLBACK_TEXT,
    fallbackHolds,
    foldedHunk,
    hunkFoldControl,
    hunkRange,
    layoutInEffect,
    lineIdentity,
    lineNumberDigits,
    lineNumberLabel,
    modelCopyText,
    navigatorFoldsForSplit,
    nextNamedSide,
    oneColumnSide,
    placeKey,
    pruneShown,
    readStoredLayout,
    splitRowIdentity,
    splitRows,
    storeLayout,
    topmostVisible,
    widthInCh,
    withShown,
    type CodePosition,
    type DiffLayout,
    type ShownHunks,
    type Side,
    type SplitRow,
} from "../diffLayout"
import { escapeLine, escapeText, hunksWarn, sourceOffsetIn, type Segment } from "../hiddenChars"
import type { DiffContent, DiffFile, Hunk, Line, LineKind } from "../types"

export interface DiffViewProps {
    /// The model, in the order its sections and navigator rows are shown.
    files: DiffFile[]
    /// The names of the two sides, which the toolbar shows, escaped, while
    /// side by side is in effect: commit detail's first parent and commit, a
    /// pull request's base and head branches.
    sideNames: { old: string; new: string }
    /// Reads one withheld file alone. Offered only for a withheld file; the
    /// file it resolves to replaces that file's content and nothing else.
    loadFile: (file: DiffFile) => Promise<DiffFile>
    /// Rendered inside the file's sticky header, such as a pull request's
    /// "viewed" mark. Given no column, and the same in either layout.
    renderFileHeaderExtra?: (file: DiffFile) => ReactNode
    /// Rendered between the header and the first hunk, across the section's
    /// full width, such as a pull request's review threads. Given no column,
    /// and the same in either layout.
    renderFilePreamble?: (file: DiffFile) => ReactNode
    /// A file's per-hunk slots and the hunks the host folds, such as a pull
    /// request's hunk marks; undefined for a file that has none. Commit
    /// detail passes none, and its hunks render as they always have.
    hunkSlots?: (file: DiffFile) => HunkSlots | undefined
    /// What a host may ask of the view: to collapse a section once.
    ref?: Ref<DiffViewHandle>
}

/// What a host adds to one file's hunks (`diff-view`: *Diff View Hosts*,
/// *Folded Hunks*). Indices count the file's hunks from zero, as rendered.
export interface HunkSlots {
    /// The hunks the host folds: each renders as one row, its heading, until
    /// the reader shows it.
    folded: ReadonlySet<number>
    /// Rendered in a hunk's heading row, in the gutter left of its `@@`
    /// heading, folded or not. Given no column, and the same in either layout.
    heading: (index: number, hunk: Hunk) => ReactNode
    /// Rendered as a row after an unfolded hunk's last line, across the
    /// section's full width; nothing renders no row.
    end: (index: number, hunk: Hunk) => ReactNode
}

/// What the view does for its host on request.
export interface DiffViewHandle {
    /// Collapses `file`'s section, once, as its header's toggle would: the
    /// collapse is the view's state, and the toggle reverses it.
    collapse: (file: DiffFile) => void
}

/// A withheld file's read on request, by its key.
type FileLoad =
    | { status: "loading" }
    | { status: "failed"; message: string }
    | { status: "loaded"; content: DiffContent }

const NO_LOADS: ReadonlyMap<string, FileLoad> = new Map()
const NO_KEYS: ReadonlySet<string> = new Set()
const NO_SHOWN: ShownHunks = new Map()

const LAYOUT_OPTIONS: readonly ChoiceOption<DiffLayout>[] = [
    { value: "unified", label: "Unified" },
    { value: "split", label: "Side by side" },
]

const MARKERS: Record<LineKind, string> = { context: " ", added: "+", removed: "−" }

/// The one diff renderer every host uses (`diff-view`: *Diff View Hosts*;
/// design D5): commit detail and the pull-request viewer. A host passes the
/// model, the names of its two sides and a loader for withheld files, and adds
/// what it needs through its per-file and per-hunk slots, the hunks it folds
/// and a request to collapse a section; nothing else forks.
///
/// The view owns the layout control, and lays the same model out unified or
/// side by side (D8). The decisions live in pure modules with their tests —
/// `diffLayout.ts`, `diffFiles.ts`, `diffHighlight.ts` and `hiddenChars.ts` —
/// and this component only measures, renders and wires events to them.
///
/// Everything it holds is view state, kept per file key and never persisted:
/// collapse, loaded content and the folded hunks the reader showed (both
/// dropped when the host passes a new `files` array), and the navigator's
/// mark. The layout is read from the surface's stored choice once, as the
/// view mounts, so a host that re-renders with new files keeps the open
/// view's layout, and one that remounts the view for another commit or pull
/// request starts from the stored choice.
///
/// Folding, showing and collapsing keep one row where it was (*Folded
/// Hunks*): the row the reader acted on, and otherwise the topmost visible
/// line, recorded before the change reaches the page and restored before it
/// paints.
export function DiffView({
    files,
    sideNames,
    loadFile,
    renderFileHeaderExtra,
    renderFilePreamble,
    hunkSlots,
    ref,
}: DiffViewProps) {
    const [chosen, setChosen] = useState<DiffLayout>(() => readStoredLayout())
    // The layout in effect: null until the sections column has been measured,
    // which happens before the first paint, so no row ever paints in one
    // layout and then flips to the other (*No flash of the wrong layout*).
    const [inEffect, setInEffect] = useState<DiffLayout | null>(null)
    const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(NO_KEYS)
    // Loaded content belongs to the `files` array it was read for, so a new
    // array from the host drops it with no effect and no stale frame.
    const [loadState, setLoadState] = useState(() => ({ files, byKey: NO_LOADS }))
    const loads = loadState.files === files ? loadState.byKey : NO_LOADS
    // The folded hunks the reader showed belong to the `files` array they
    // were shown in, as loaded content does.
    const [shownState, setShownState] = useState(() => ({ files, byKey: NO_SHOWN }))
    const shown = shownState.files === files ? shownState.byKey : NO_SHOWN
    const fallbackId = useId()

    const rootRef = useRef<HTMLDivElement>(null)
    const bodyRef = useRef<HTMLDivElement>(null)
    const sectionsRef = useRef<HTMLDivElement>(null)
    const chProbeRef = useRef<HTMLSpanElement>(null)
    const navigatorProbeRef = useRef<HTMLSpanElement>(null)

    // Mirrors for the measuring callback and the document listeners, which
    // outlive any one render.
    const chosenRef = useRef(chosen)
    chosenRef.current = chosen
    const filesRef = useRef(files)
    filesRef.current = files
    const loadFileRef = useRef(loadFile)
    loadFileRef.current = loadFile
    // The last layout decided, which is the hysteresis' "until then". State
    // only mirrors it for rendering, so two measurements before a render
    // still decide from the right previous layout.
    const decidedRef = useRef<DiffLayout | null>(null)
    // The reader's place, taken as a switch begins and restored once the new
    // layout has rendered, before it paints.
    const placeRef = useRef<Place | null>(null)
    // The scroll position that restoring the place set, for the navigator,
    // which tells it from a reader's scroll.
    const restoredTopRef = useRef<number | null>(null)
    // The side named for selection: a ref and an attribute on the root, so a
    // drag across thousands of rows renders nothing.
    const namedSideRef = useRef<Side | null>(null)
    // Keeping the reader's place across a fold, a show or a collapse: the
    // row the reader just acted on, measured as the click began; the place a
    // fold change keeps, taken while the page still shows the folds before
    // it; and the folds the page shows now.
    const actedOnRef = useRef<Anchor | null>(null)
    const foldPlaceRef = useRef<Anchor | null>(null)
    const committedFoldsRef = useRef<string | null>(null)

    // ---- Measuring ------------------------------------------------------

    /// Decide the navigator's fold and the layout in effect from the widths
    /// as they stand: the view's and the sections column's, in `ch` of the
    /// code font, which the probe measures so the thresholds follow the
    /// font's size and the zoom. Never the window's width.
    const measure = useCallback(() => {
        const root = rootRef.current
        const body = bodyRef.current
        const sections = sectionsRef.current
        const chProbe = chProbeRef.current
        const navigatorProbe = navigatorProbeRef.current
        if (!root || !body || !sections || !chProbe || !navigatorProbe) return
        // Every width in layout pixels, which no transform on an ancestor
        // scales. The probe is a hundred `ch` wide, for a precise one.
        const chPx = chProbe.offsetWidth / 100
        // The navigator's width is the stylesheet's, read off a probe, never
        // off the navigator itself, which may already be folded.
        const folds = navigatorFoldsForSplit(
            chosenRef.current,
            widthInCh(body.clientWidth, chPx),
            widthInCh(navigatorProbe.offsetWidth, chPx),
        )
        // An attribute the stylesheet folds on, set here rather than through
        // a render so the sections column below is read in its folded width.
        root.toggleAttribute("data-navigator-folded", folds)
        const next = layoutInEffect(
            chosenRef.current,
            widthInCh(sections.clientWidth, chPx),
            decidedRef.current ?? "unified",
        )
        decidedRef.current = next
        setInEffect(next)
    }, [])

    useLayoutEffect(() => {
        measure()
    }, [chosen, measure])

    // One observer keeps both widths current, as `FigureLightbox` does: the
    // view and the sections column for the space, the probe for the font.
    useLayoutEffect(() => {
        const observer = new ResizeObserver(() => measure())
        for (const element of [
            bodyRef.current,
            sectionsRef.current,
            chProbeRef.current,
            navigatorProbeRef.current,
        ]) {
            if (element) observer.observe(element)
        }
        return () => observer.disconnect()
    }, [measure])

    // Restore the reader's place once the layout a switch decided is the one
    // rendered. Runs after the measuring effect above, which is what decides
    // it, and after every commit, since the new layout may land a render
    // later.
    useLayoutEffect(() => {
        const place = placeRef.current
        if (place === null || inEffect !== decidedRef.current) return
        placeRef.current = null
        restoredTopRef.current = restorePlace(sectionsRef.current, place)
    })

    // ---- The layout control ---------------------------------------------

    const choose = useCallback((next: DiffLayout) => {
        placeRef.current = recordPlace(sectionsRef.current)
        // Best-effort: a refused write keeps the choice for this view alone,
        // and the next diff opens in the layout stored before.
        storeLayout(next)
        setChosen(next)
    }, [])

    // ---- Collapse and loading -------------------------------------------

    const toggle = useCallback((key: string) => {
        setCollapsed((previous) => {
            const next = new Set(previous)
            if (!next.delete(key)) next.add(key)
            return next
        })
    }, [])

    const load = useCallback((file: DiffFile) => {
        const key = fileKey(file)
        const requestedFor = filesRef.current
        const settle = (next: FileLoad) =>
            setLoadState((previous) => {
                // A read that lands after the host passed new files is
                // dropped with the content it would have replaced.
                if (filesRef.current !== requestedFor) return previous
                const byKey = new Map(previous.files === requestedFor ? previous.byKey : [])
                byKey.set(key, next)
                return { files: requestedFor, byKey }
            })
        settle({ status: "loading" })
        loadFileRef.current(file).then(
            (loaded) => settle({ status: "loaded", content: loaded.content }),
            (error: unknown) => settle({ status: "failed", message: String(error) }),
        )
    }, [])

    // ---- Folds and showing ----------------------------------------------

    const slotsByKey = useMemo(
        () => new Map(files.map((file) => [fileKey(file), hunkSlots?.(file)] as const)),
        [files, hunkSlots],
    )

    const show = useCallback((key: string, index: number, isShown: boolean) => {
        const shownFor = filesRef.current
        setShownState((previous) => {
            const byKey = previous.files === shownFor ? previous.byKey : NO_SHOWN
            return { files: shownFor, byKey: withShown(byKey, key, index, isShown) }
        })
    }, [])

    // A hunk the host stops folding forgets that the reader showed it, so the
    // host folding it again folds it. Settles at once: nothing to drop leaves
    // the state as it is.
    useLayoutEffect(() => {
        setShownState((previous) => {
            const byKey = pruneShown(previous.byKey, (key) => slotsByKey.get(key)?.folded)
            return byKey === previous.byKey ? previous : { files: previous.files, byKey }
        })
    }, [slotsByKey])

    // What the page folds and collapses, as one value: when it changes, the
    // reader's place is kept across the change.
    const folds = useMemo(() => {
        const parts: string[] = []
        for (const [key, slots] of slotsByKey) {
            const folded = [...(slots?.folded ?? [])].filter(
                (index) => !shown.get(key)?.has(index),
            )
            if (folded.length > 0 || collapsed.has(key)) {
                parts.push(`${key}\u0000${collapsed.has(key) ? "c" : ""}${folded.join(",")}`)
            }
        }
        return parts.join("\u0001")
    }, [slotsByKey, shown, collapsed])

    // Taken during the render that changes the folds, while the page still
    // shows the ones before: the row the reader acted on, else the topmost
    // visible line. Taken once per change, so a render React throws away
    // and repeats takes the same place.
    if (
        inEffect !== null &&
        committedFoldsRef.current !== null &&
        folds !== committedFoldsRef.current &&
        foldPlaceRef.current === null
    ) {
        foldPlaceRef.current = actedOnRef.current ?? placeAnchor(recordPlace(sectionsRef.current))
    }

    useLayoutEffect(() => {
        actedOnRef.current = null
        if (folds === committedFoldsRef.current) return
        const first = committedFoldsRef.current === null
        committedFoldsRef.current = folds
        const anchor = foldPlaceRef.current
        foldPlaceRef.current = null
        if (!first && anchor) restoredTopRef.current = restoreAnchor(sectionsRef.current, anchor)
    }, [folds])

    // The row the reader acts on, measured before anything changes the page.
    // An action that changes nothing leaves no anchor for a later change.
    const actOn = useCallback((anchor: Anchor | null) => {
        actedOnRef.current = anchor
        setTimeout(() => {
            if (actedOnRef.current === anchor) actedOnRef.current = null
        }, 0)
    }, [])

    // A click in a hunk's heading row, its end row, or its file's header.
    const onClickCapture = useCallback(
        (event: MouseEvent<HTMLDivElement>) => {
            if (event.target instanceof Element) actOn(anchorAt(event.target))
        },
        [actOn],
    )

    useImperativeHandle(
        ref,
        () => ({
            collapse(file: DiffFile) {
                const key = fileKey(file)
                const header = sectionOf(sectionsRef.current, key)?.querySelector(
                    ".diff-file-header",
                )
                actOn(header ? anchorAt(header) : null)
                setCollapsed((previous) =>
                    previous.has(key) ? previous : new Set(previous).add(key),
                )
            },
        }),
        [actOn],
    )

    // ---- What the files show now ----------------------------------------

    const hunksByKey = useMemo(() => {
        const byKey = new Map<string, readonly Hunk[]>()
        for (const file of files) {
            const content = contentOf(file, loads)
            if (content.kind === "hunks") byKey.set(fileKey(file), content.hunks)
        }
        return byKey
    }, [files, loads])

    const notShownLabel = useMemo(
        () =>
            notShownInFullLabel(
                notShownInFull(files.map((file) => ({ ...file, content: contentOf(file, loads) }))),
            ),
        [files, loads],
    )

    const viewRef = useRef<ViewSnapshot | null>(null)
    viewRef.current =
        inEffect === null
            ? null
            : {
                  files,
                  inEffect,
                  hunksOf: (key) => hunksByKey.get(key),
                  folded: (key, index) =>
                      foldedHunk(slotsByKey.get(key)?.folded, shown, key, index),
              }

    // ---- Selection and copying ------------------------------------------

    // The side-naming rule, driven from the document: a pointer-down outside
    // any code cell, the view's or not, clears the side. Only the primary
    // button takes part, so opening a context menu to copy keeps the side the
    // drag named.
    useEffect(() => {
        const name = (side: Side | null) => {
            namedSideRef.current = side
            if (side === null) rootRef.current?.removeAttribute("data-named-side")
            else rootRef.current?.setAttribute("data-named-side", side)
        }
        const onPointerDown = (event: PointerEvent) => {
            if (event.button !== 0) return
            const root = rootRef.current
            const cell =
                event.target instanceof Element
                    ? event.target.closest<HTMLElement>(".diff-code")
                    : null
            const codeCell =
                cell && root?.contains(cell)
                    ? ((cell.dataset.side as Side | undefined) ?? "unified")
                    : null
            name(nextNamedSide(namedSideRef.current, { type: "pointerDown", codeCell }))
        }
        const onPointerUp = (event: PointerEvent) => {
            if (event.button !== 0) return
            const root = rootRef.current
            const selection = document.getSelection()
            const collapsed = !selection || selection.rangeCount === 0 || selection.isCollapsed
            const insideView =
                !collapsed && root !== null && selection.getRangeAt(0).intersectsNode(root)
            name(nextNamedSide(namedSideRef.current, { type: "pointerUp", collapsed, insideView }))
        }
        document.addEventListener("pointerdown", onPointerDown, true)
        document.addEventListener("pointerup", onPointerUp, true)
        return () => {
            document.removeEventListener("pointerdown", onPointerDown, true)
            document.removeEventListener("pointerup", onPointerUp, true)
        }
    }, [])

    // Every copy of a selection that reaches into the view is built here and
    // written inside the copy event, which every origin permits. Listened for
    // on the document, because a selection made by dragging leaves the focus
    // where it was, and the event fires there. A copy from a text field or an
    // editable region is that field's own, whatever the document's selection
    // still holds: Firefox keeps a field's selection apart from it, and a
    // host's editor in a slot (a reply box) holds a real document range.
    useEffect(() => {
        const onCopy = (event: ClipboardEvent) => {
            const target = event.target
            if (
                target instanceof HTMLInputElement ||
                target instanceof HTMLTextAreaElement ||
                (target instanceof HTMLElement && target.isContentEditable)
            ) {
                return
            }
            const root = rootRef.current
            const view = viewRef.current
            const selection = document.getSelection()
            if (event.defaultPrevented || !event.clipboardData || !root || !view) return
            if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return
            const range = selection.getRangeAt(0)
            if (!range.intersectsNode(root)) return
            event.clipboardData.setData(
                "text/plain",
                clipboardText(range, root, view, namedSideRef.current),
            )
            event.preventDefault()
        }
        document.addEventListener("copy", onCopy)
        return () => document.removeEventListener("copy", onCopy)
    }, [])

    const fallback = inEffect !== null && fallbackHolds(chosen, inEffect)

    return (
        <div className="diff-view" ref={rootRef}>
            <div className="diff-view-probes" aria-hidden="true" data-copy="skip">
                <span className="diff-view-probe-ch" ref={chProbeRef} />
                <span className="diff-view-probe-navigator" ref={navigatorProbeRef} />
            </div>
            <div className="diff-toolbar">
                <ChoiceGroup
                    options={LAYOUT_OPTIONS}
                    value={chosen}
                    onChange={choose}
                    label="Diff layout"
                    describedBy={fallback ? fallbackId : undefined}
                />
                {fallback && (
                    <span id={fallbackId} className="diff-toolbar-note">
                        {NARROW_FALLBACK_TEXT}
                    </span>
                )}
                {inEffect === "split" && (
                    <span className="diff-side-names">
                        <span className="diff-side-name">
                            <span className="diff-side-label">old</span>{" "}
                            <EscapedText text={sideNames.old} />
                        </span>{" "}
                        <span className="diff-side-name">
                            <span className="diff-side-label">new</span>{" "}
                            <EscapedText text={sideNames.new} />
                        </span>
                    </span>
                )}
                {notShownLabel !== null && (
                    <span className="diff-toolbar-count">{notShownLabel}</span>
                )}
            </div>
            <div className="diff-view-body" ref={bodyRef}>
                <Navigator
                    files={files}
                    sectionsRef={sectionsRef}
                    restoredTopRef={restoredTopRef}
                    layout={inEffect}
                    collapsed={collapsed}
                    loads={loads}
                    folds={folds}
                />
                <div className="diff-sections" ref={sectionsRef} onClickCapture={onClickCapture}>
                    {inEffect !== null &&
                        files.map((file, index) => {
                            const key = fileKey(file)
                            return (
                                <FileSection
                                    key={key}
                                    file={file}
                                    index={index}
                                    content={contentOf(file, loads)}
                                    load={loads.get(key)}
                                    collapsed={collapsed.has(key)}
                                    layout={inEffect}
                                    onToggle={toggle}
                                    onLoad={load}
                                    renderHeaderExtra={renderFileHeaderExtra}
                                    renderPreamble={renderFilePreamble}
                                    slots={slotsByKey.get(key)}
                                    shown={shown.get(key)}
                                    onShow={show}
                                />
                            )
                        })}
                </div>
            </div>
        </div>
    )
}

/// A file's content as the view shows it: what its read on request brought,
/// in place of the withheld content, once it has landed.
function contentOf(file: DiffFile, loads: ReadonlyMap<string, FileLoad>): DiffContent {
    const load = loads.get(fileKey(file))
    return load?.status === "loaded" ? load.content : file.content
}

/// The scrolling ancestor whose port the sections scroll through. The view
/// owns no scroller of its own, so its sticky headers stick to the host's.
function scrollPortOf(element: Element | null): HTMLElement | null {
    for (let parent = element?.parentElement ?? null; parent; parent = parent.parentElement) {
        const { overflowY } = getComputedStyle(parent)
        if (overflowY === "auto" || overflowY === "scroll") return parent
    }
    return null
}

// -------------------------------------------------------------------------
// Keeping the reader's place across a switch
// -------------------------------------------------------------------------

/// Where the reader was: a file's section, the side and number of its
/// topmost visible line when one was found, the hunk that line belongs to or
/// the folded hunk whose heading was topmost, and how far below the port's
/// top that line, heading or section sat.
interface Place {
    file: string
    key: { side: Side; line: number } | null
    hunk: number | null
    offset: number
}

/// The reader's place, by the side-qualified identity of the topmost visible
/// line, or a folded hunk's heading when that is topmost. None while the top
/// of the diff is itself in view: the switch then keeps the scroll as it is,
/// toolbar and all.
function recordPlace(sections: HTMLElement | null): Place | null {
    const port = scrollPortOf(sections)
    if (!sections || !port) return null
    const portTop = port.getBoundingClientRect().top
    if (sections.getBoundingClientRect().top >= portTop) return null
    const rows = sections.querySelectorAll<HTMLElement>(
        ".diff-row, .diff-split-row, [data-hunk-folded]",
    )
    const row =
        rows[topmostVisible(rows.length, (i) => rows[i].getBoundingClientRect().bottom, portTop)]
    // Without a line below the port's top, every file left is collapsed or
    // shows a state row: keep the place by the topmost visible section.
    const sectionList = sections.children
    const anchor =
        row ??
        sectionList[
            topmostVisible(
                sectionList.length,
                (i) => sectionList[i].getBoundingClientRect().bottom,
                portTop,
            )
        ]
    const section = anchor?.closest<HTMLElement>(".diff-file")
    if (!anchor || !section) return null
    const number = (value: string | undefined) => (value === undefined ? null : Number(value))
    const foldedHeading = row?.dataset.hunkFolded !== undefined
    const hunk = foldedHeading
        ? number(row?.dataset.hunkHeading)
        : number(row?.querySelector<HTMLElement>("[data-hunk-index]")?.dataset.hunkIndex)
    return {
        file: section.dataset.fileKey ?? "",
        key:
            row && !foldedHeading
                ? placeKey({ old: number(row.dataset.oldLine), new: number(row.dataset.newLine) })
                : null,
        hunk,
        offset: anchor.getBoundingClientRect().top - portTop,
    }
}

/// Scroll the line or section a place names back to where it sat, and return
/// the scroll position that leaves, or null when nothing moved. Files are
/// matched by key through the dataset, never interpolated into a selector,
/// since paths may hold any character.
function restorePlace(sections: HTMLElement | null, place: Place): number | null {
    const port = scrollPortOf(sections)
    const section = sectionOf(sections, place.file)
    if (!port || !section) return null
    const line = place.key
        ? section.querySelector(`[data-${place.key.side}-line="${place.key.line}"]`)
        : null
    // A line whose hunk folded meanwhile gives its place to the hunk's
    // heading, as a folded heading that was topmost keeps its own.
    const heading =
        line === null && place.hunk !== null ? headingOf(section, place.hunk) : null
    const target = line ?? heading ?? section
    const offset = line || heading || (!place.key && place.hunk === null) ? place.offset : 0
    port.scrollTop += target.getBoundingClientRect().top - port.getBoundingClientRect().top - offset
    return port.scrollTop
}

/// The section of the file `key`, matched through the dataset, never
/// interpolated into a selector, since paths may hold any character.
function sectionOf(sections: HTMLElement | null, key: string): HTMLElement | null {
    return (
        Array.from(sections?.children ?? []).find(
            (element): element is HTMLElement =>
                element instanceof HTMLElement && element.dataset.fileKey === key,
        ) ?? null
    )
}

/// Hunk `hunk`'s heading row in `section`, folded or not.
function headingOf(section: HTMLElement, hunk: number): HTMLElement | null {
    return section.querySelector<HTMLElement>(`[data-hunk-heading="${hunk}"]`)
}

// -------------------------------------------------------------------------
// Keeping the reader's place across a fold, a show or a collapse
// -------------------------------------------------------------------------

/// The row a fold change keeps where it was (`diff-view`: *Folded Hunks*),
/// with how far below the port's top it sat: a hunk's heading row, for a
/// control in it; the first row after a hunk, for its end row; a file's
/// header, for a control in it or a collapse the host asked for; else the
/// reader's place.
type Anchor =
    | { kind: "heading"; file: string; hunk: number; offset: number }
    | { kind: "after"; file: string; hunk: number; offset: number }
    | { kind: "header"; file: string; offset: number }
    | { kind: "place"; place: Place }

/// The anchor for a reader acting on `target`, measured now; null outside
/// those rows.
function anchorAt(target: Element): Anchor | null {
    const section = target.closest<HTMLElement>(".diff-file")
    const port = scrollPortOf(section)
    if (!section || !port) return null
    const file = section.dataset.fileKey ?? ""
    const below = (element: Element) =>
        element.getBoundingClientRect().top - port.getBoundingClientRect().top
    const heading = target.closest<HTMLElement>("[data-hunk-heading]")
    if (heading) {
        const hunk = Number(heading.dataset.hunkHeading)
        return { kind: "heading", file, hunk, offset: below(heading) }
    }
    const end = target.closest<HTMLElement>("[data-hunk-end]")
    if (end) {
        const hunk = Number(end.dataset.hunkEnd)
        const after = afterHunk(section, hunk)
        return after && { kind: "after", file, hunk, offset: below(after) }
    }
    const header = target.closest<HTMLElement>(".diff-file-header")
    if (header) return { kind: "header", file, offset: below(header) }
    return null
}

/// The reader's place, as an anchor; null where there is none to keep.
function placeAnchor(place: Place | null): Anchor | null {
    return place && { kind: "place", place }
}

/// The first row after hunk `hunk` of `section`: the next hunk's heading
/// row, or after its last hunk, the next file's section.
function afterHunk(section: HTMLElement, hunk: number): HTMLElement | null {
    const next = section.nextElementSibling
    return headingOf(section, hunk + 1) ?? (next instanceof HTMLElement ? next : null)
}

/// Scroll the row `anchor` names back to where it sat, and return the scroll
/// position that leaves, or null when nothing moved.
function restoreAnchor(sections: HTMLElement | null, anchor: Anchor): number | null {
    if (anchor.kind === "place") return restorePlace(sections, anchor.place)
    const port = scrollPortOf(sections)
    const section = sectionOf(sections, anchor.file)
    if (!port || !section) return null
    const target =
        anchor.kind === "heading"
            ? headingOf(section, anchor.hunk)
            : anchor.kind === "after"
              ? afterHunk(section, anchor.hunk)
              : section.querySelector(".diff-file-header")
    if (!target) return null
    port.scrollTop +=
        target.getBoundingClientRect().top - port.getBoundingClientRect().top - anchor.offset
    return port.scrollTop
}

// -------------------------------------------------------------------------
// Copying
// -------------------------------------------------------------------------

/// What the copy handler reads of the view as it stands.
interface ViewSnapshot {
    files: readonly DiffFile[]
    inEffect: DiffLayout
    hunksOf: (key: string) => readonly Hunk[] | undefined
    /// Whether a hunk is folded now, its lines not shown.
    folded: (key: string, hunk: number) => boolean
}

/// The clipboard text for a selection that reaches into the view: built from
/// the model when both ends lie in code cells of one file and unified is in
/// effect or a side is named (`modelCopyText`), and otherwise in document
/// order.
function clipboardText(
    range: Range,
    root: HTMLElement,
    view: ViewSnapshot,
    namedSide: Side | null,
): string {
    const start = codePositionAt(range.startContainer, range.startOffset, root, view)
    const end = codePositionAt(range.endContainer, range.endOffset, root, view)
    return (
        modelCopyText({
            start,
            end,
            inEffect: view.inEffect,
            namedSide,
            hunksOf: view.hunksOf,
            folded: view.folded,
        }) ??
        documentOrderText(range, namedSide)
    )
}

/// One end of a selection mapped back to the model, or null when it is not in
/// one of the view's code cells. The cell names its hunk and line; the offset
/// is the count of characters drawn before the point, mapped through the
/// line's escape segments, so a point inside an escape's label lands before
/// or after its real character.
function codePositionAt(
    node: Node,
    offset: number,
    root: HTMLElement,
    view: ViewSnapshot,
): CodePosition | null {
    const element = node instanceof Element ? node : node.parentElement
    const cell = element?.closest<HTMLElement>(".diff-code")
    const section = cell?.closest<HTMLElement>(".diff-file")
    if (!cell || !section || !root.contains(cell)) return null
    const file = view.files[Number(section.dataset.fileIndex)]
    const hunk = Number(cell.dataset.hunkIndex)
    const index = Number(cell.dataset.lineIndex)
    const line = file ? view.hunksOf(fileKey(file))?.[hunk]?.lines[index] : undefined
    if (!file || !line) return null
    return {
        file: fileKey(file),
        hunk,
        line: index,
        offset: sourceOffsetIn(escapeLine(line).segments, drawnOffset(cell, node, offset)),
    }
}

/// How many characters a code cell draws before a point: none for a point
/// before its text, such as in the marker, and all of them for one after it,
/// such as in the badge.
function drawnOffset(cell: HTMLElement, node: Node, offset: number): number {
    const text = cell.querySelector(".diff-text")
    if (!text) return 0
    const range = document.createRange()
    range.selectNodeContents(text)
    const where = range.comparePoint(node, offset)
    if (where < 0) return 0
    if (where === 0) range.setEnd(node, offset)
    return range.toString().length
}

/// A selection's text in document order, as the page shows it: a line for
/// every rendered line (an element marked `data-copy-line`) and, elsewhere,
/// for every run of text in a block of its own. It leaves out every element
/// marked `data-copy="skip"` (line numbers, markers, fillers and badges),
/// visually hidden text, and, while a side is named, the other column's cells,
/// which old WebKit copies although `user-select: none` hides them. Every
/// escape becomes the real character it stands for.
///
/// The walk steps from node to node rather than testing each child against
/// the range, which costs a sibling count per test and would go quadratic on
/// a table of thousands of rows.
function documentOrderText(range: Range, namedSide: Side | null): string {
    const root = range.commonAncestorContainer
    const otherSide = namedSide === null ? null : namedSide === "old" ? "new" : "old"
    const styles = new Map<Element, { display: string; whiteSpace: string }>()
    const styleOf = (element: Element) => {
        let style = styles.get(element)
        if (style === undefined) {
            const computed = getComputedStyle(element)
            style = { display: computed.display, whiteSpace: computed.whiteSpace }
            styles.set(element, style)
        }
        return style
    }
    const displayOf = (element: Element) => styleOf(element).display
    const skipped = (element: Element) =>
        element.getAttribute("data-copy") === "skip" ||
        element.classList.contains("sr-only") ||
        (otherSide !== null && element.getAttribute("data-side") === otherSide) ||
        !(element instanceof HTMLElement) ||
        (element.closest("[data-copy-line]") === null && displayOf(element) === "none")
    // The element a run of text belongs to: its line, else its nearest block.
    const unitOf = (node: Node): Element | null => {
        const parent = node instanceof Element ? node : node.parentElement
        const line = parent?.closest("[data-copy-line]")
        if (line) return line
        for (let element = parent; element; element = element.parentElement) {
            const display = displayOf(element)
            if (!display.startsWith("inline") && display !== "contents") return element
        }
        return null
    }

    // A selection wholly inside something left out copies nothing, and one
    // wholly inside an escape's label its character or nothing.
    const rootElement = root instanceof Element ? root : root.parentElement
    for (let element = rootElement; element; element = element.parentElement) {
        if (element.classList.contains("diff-escape")) {
            return escapeSelected(range, element) ? escapedCharacter(element) : ""
        }
        if (skipped(element)) return ""
    }

    // The text so far, a line at a time; `text` is null until a line begins.
    // A code cell's line is the file's text and is kept exactly, an empty
    // one included. Any other line is the page's text: its whitespace is
    // collapsed where the page collapses it, and it is trimmed, and dropped
    // when nothing is left.
    const out = {
        lines: [] as { text: string; exact: boolean }[],
        text: null as string | null,
        unit: null as Element | null,
    }
    const begin = (unit: Element | null) => {
        if (out.text !== null) {
            const exact = out.unit?.classList.contains("diff-code") ?? false
            const text = exact ? out.text : out.text.trim()
            if (exact || text !== "") out.lines.push({ text, exact })
        }
        out.text = ""
        out.unit = unit
    }
    const write = (node: Node, piece: string) => {
        const unit = unitOf(node)
        if (out.text === null || unit !== out.unit) begin(unit)
        const parent = node instanceof Element ? node : node.parentElement
        const collapses =
            !unit?.classList.contains("diff-code") &&
            parent !== null &&
            !styleOf(parent).whiteSpace.startsWith("pre") &&
            !styleOf(parent).whiteSpace.startsWith("break-spaces")
        const text = out.text ?? ""
        if (!collapses) {
            out.text = text + piece
            return
        }
        // Collapsed across neighbouring text, as the page collapses it.
        const collapsed = piece.replace(/[\t\n\f\r ]+/g, " ")
        out.text =
            text + (collapsed.startsWith(" ") && (text === "" || text.endsWith(" "))
                ? collapsed.slice(1)
                : collapsed)
    }
    const finish = () => {
        begin(null)
        return out.lines.map((line) => line.text).join("\n")
    }

    let node = firstNodeOf(range)
    const stop = nodeAfter(range)
    // A start inside a line begins that line, so a selected empty line still
    // counts. A start inside something left out begins after it, and one
    // inside an escape's label after its character, taken or not.
    const startElement = node instanceof Element ? node : (node?.parentElement ?? null)
    const startLine = startElement?.closest("[data-copy-line]")
    if (startLine) begin(startLine)
    for (let element = startElement; element && element !== root; element = element.parentElement) {
        const escape = element.classList.contains("diff-escape")
        if (!escape && !skipped(element)) continue
        if (escape && escapeSelected(range, element)) write(element, escapedCharacter(element))
        if (stop !== null && element.contains(stop)) return finish()
        node = following(element, root, false)
    }

    while (node && node !== stop) {
        let descend = true
        if (node instanceof Text) {
            const from = node === range.startContainer ? range.startOffset : 0
            const to = node === range.endContainer ? range.endOffset : node.data.length
            if (to > from) write(node, node.data.slice(from, to))
        } else if (node instanceof Element) {
            const escape = node.classList.contains("diff-escape")
            if (escape || skipped(node)) {
                descend = false
                if (escape && escapeSelected(range, node)) write(node, escapedCharacter(node))
                if (stop !== null && node.contains(stop)) break
            } else if (node.hasAttribute("data-copy-line")) {
                begin(node)
            }
        }
        node = following(node, root, descend)
    }
    return finish()
}

/// The real character an escape stands for, from the code point it carries.
function escapedCharacter(escape: Element): string {
    const codePoint = Number.parseInt(escape.getAttribute("data-code-point") ?? "", 16)
    return Number.isFinite(codePoint) ? String.fromCodePoint(codePoint) : ""
}

/// The first node a range covers, in document order.
function firstNodeOf(range: Range): Node | null {
    const { startContainer, startOffset } = range
    if (startContainer instanceof CharacterData) return startContainer
    return (
        startContainer.childNodes[startOffset] ??
        following(startContainer, range.commonAncestorContainer, false)
    )
}

/// The first node past a range's end, in document order, or null when the
/// range runs to the end of its common ancestor.
function nodeAfter(range: Range): Node | null {
    const { endContainer, endOffset, commonAncestorContainer } = range
    if (endContainer instanceof CharacterData) {
        return following(endContainer, commonAncestorContainer, false)
    }
    return (
        endContainer.childNodes[endOffset] ??
        following(endContainer, commonAncestorContainer, false)
    )
}

/// The node after `node` in document order within `root`: its first child
/// when `descend`, else the next node past its subtree.
function following(node: Node, root: Node, descend: boolean): Node | null {
    if (descend && node.firstChild) return node.firstChild
    for (let current: Node | null = node; current && current !== root; ) {
        if (current.nextSibling) return current.nextSibling
        current = current.parentNode
    }
    return null
}

/// Whether a selection takes an escape's real character, by the rule
/// `sourceOffset` gives a point inside its label: a start at the label's
/// start takes it, a start further in does not, and an end takes it from one
/// character into the label on.
function escapeSelected(range: Range, escape: Element): boolean {
    const label = escape.firstChild
    if (!label) return false
    return (
        range.comparePoint(label, 0) === 0 &&
        !(range.endContainer === label && range.endOffset === 0)
    )
}

// -------------------------------------------------------------------------
// The navigator
// -------------------------------------------------------------------------

interface NavigatorProps {
    files: readonly DiffFile[]
    sectionsRef: RefObject<HTMLDivElement | null>
    /// The scroll position a switch's place-keeping last set, until the
    /// navigator has seen it: a scroll to it is the view's own, not the
    /// reader's, so it keeps an activated file's mark.
    restoredTopRef: RefObject<number | null>
    /// What moves the sections: the mark is decided again whenever the
    /// layout, a collapse, a loaded file or a fold changes them.
    layout: DiffLayout | null
    collapsed: ReadonlySet<string>
    loads: ReadonlyMap<string, FileLoad>
    folds: string
}

/// The changed files as a tree by directory (`diff-view`: *File Navigator*):
/// one Tab stop with a roving current row, as the workspace tree keeps one.
/// Activating a file scrolls its section into view and marks it, and
/// scrolling moves the mark to the section being read. It holds the mark and
/// its scroll listener itself, so scrolling renders the navigator alone and
/// never the sections.
const Navigator = memo(function Navigator({
    files,
    sectionsRef,
    restoredTopRef,
    layout,
    collapsed,
    loads,
    folds,
}: NavigatorProps) {
    const tree = useMemo(() => navigatorTree(files), [files])
    const [closed, setClosed] = useState<ReadonlySet<string>>(NO_KEYS)
    const [marked, setMarked] = useState<string | null>(null)
    const treeRef = useRef<HTMLDivElement>(null)
    const currentRef = useRef<string | null>(null)
    // Set by an activation: the mark stays on the activated file until the
    // port scrolls away from where the activation left it, even where the
    // file is too near the end for its section to reach the port's top.
    const lockRef = useRef<{ top: number } | null>(null)

    const recompute = useCallback(() => {
        const sections = sectionsRef.current
        const port = scrollPortOf(sections)
        if (!sections || !port) return
        // Whichever comes first after a switch, its scroll event or the
        // effect below, sees the place-keeping's position once.
        const restored = restoredTopRef.current
        restoredTopRef.current = null
        if (lockRef.current !== null) {
            if (restored !== null && Math.abs(port.scrollTop - restored) < 1) {
                lockRef.current = { top: port.scrollTop }
                return
            }
            if (Math.abs(port.scrollTop - lockRef.current.top) < 1) return
            lockRef.current = null
        }
        const list = sections.children
        const index = sectionBeingRead(
            list.length,
            (i) => list[i].getBoundingClientRect().top,
            port.getBoundingClientRect().top + 1,
        )
        const section = list[index]
        setMarked(section instanceof HTMLElement ? (section.dataset.fileKey ?? null) : null)
    }, [sectionsRef, restoredTopRef])

    useEffect(() => {
        const port = scrollPortOf(sectionsRef.current)
        if (!port) return
        port.addEventListener("scroll", recompute, { passive: true })
        return () => port.removeEventListener("scroll", recompute)
    }, [recompute, sectionsRef])

    useEffect(() => {
        recompute()
    }, [recompute, files, layout, collapsed, loads, folds])

    const activate = useCallback(
        (index: number, key: string) => {
            const section = sectionsRef.current?.children[index]
            const port = scrollPortOf(sectionsRef.current)
            if (!section || !port) return
            port.scrollTop += section.getBoundingClientRect().top - port.getBoundingClientRect().top
            lockRef.current = { top: port.scrollTop }
            setMarked(key)
        },
        [sectionsRef],
    )

    const toggleDirectory = useCallback((path: string) => {
        setClosed((previous) => {
            const next = new Set(previous)
            if (!next.delete(path)) next.add(path)
            return next
        })
    }, [])

    const handleFocus = (e: FocusEvent<HTMLDivElement>) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>('[role="treeitem"]')
        if (!row) return
        currentRef.current = row.dataset.navId ?? null
        rovingStop(treeRef.current, row)
    }

    const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
        if (e.altKey || e.ctrlKey || e.metaKey) return
        const tree = treeRef.current
        const row = (e.target as HTMLElement).closest<HTMLElement>('[role="treeitem"]')
        if (!tree || !row) return
        const rows = Array.from(tree.querySelectorAll<HTMLElement>('[role="treeitem"]'))
        const action = navigatorKeyAction(
            e.key,
            rows.map((r) => ({
                level: Number(r.getAttribute("aria-level")),
                expanded: r.hasAttribute("aria-expanded")
                    ? r.getAttribute("aria-expanded") === "true"
                    : null,
            })),
            rows.indexOf(row),
        )
        if (action === null) return
        e.preventDefault()
        if (action.kind === "focus") {
            const target = rows[action.index]
            target.focus()
            target.scrollIntoView({ block: "nearest" })
        } else if (action.kind !== "none") {
            // Toggling a directory and activating a file are both its
            // click, so the keyboard shares the pointer's contract.
            row.click()
        }
    }

    // Exactly one row is the Tab stop after every render: the current row,
    // or the first once the current one has gone with a closed directory.
    useEffect(() => {
        const tree = treeRef.current
        if (!tree) return
        const rows = Array.from(tree.querySelectorAll<HTMLElement>('[role="treeitem"]'))
        rovingStop(tree, rows.find((r) => r.dataset.navId === currentRef.current) ?? rows[0])
    })

    return (
        // Chrome, like the workspace tree: its rows select no text, and a copy
        // in document order leaves it out.
        <div className="diff-navigator" data-copy="skip">
            <div
                role="tree"
                aria-label="Changed files"
                ref={treeRef}
                onKeyDown={handleKeyDown}
                onFocus={handleFocus}
            >
                <NavigatorRows
                    nodes={tree}
                    level={1}
                    closed={closed}
                    marked={marked}
                    onToggle={toggleDirectory}
                    onActivate={activate}
                />
            </div>
        </div>
    )
})

/// Make `row` the tree's one Tab stop.
function rovingStop(tree: HTMLElement | null, row: HTMLElement | undefined) {
    if (!tree || !row) return
    for (const other of tree.querySelectorAll<HTMLElement>('[role="treeitem"][tabindex="0"]')) {
        if (other !== row) other.tabIndex = -1
    }
    row.tabIndex = 0
}

interface NavigatorRowsProps {
    nodes: readonly NavigatorNode[]
    level: number
    closed: ReadonlySet<string>
    marked: string | null
    onToggle: (path: string) => void
    onActivate: (index: number, key: string) => void
}

function NavigatorRows({ nodes, level, closed, marked, onToggle, onActivate }: NavigatorRowsProps) {
    // Indented as the workspace tree's rows are.
    const indent = { paddingLeft: (level - 1) * 12 + 4 }
    return nodes.map((node) => {
        if (node.kind === "directory") {
            const open = !closed.has(node.path)
            return (
                <div key={`d:${node.path}`}>
                    <div
                        className="diff-nav-row diff-nav-row--directory"
                        role="treeitem"
                        tabIndex={-1}
                        aria-level={level}
                        aria-expanded={open}
                        data-nav-id={`d:${node.path}`}
                        style={indent}
                        onClick={() => onToggle(node.path)}
                    >
                        <span className="chevron" aria-hidden="true">
                            {open ? <ChevronDown /> : <ChevronRight />}
                        </span>
                        <span className="diff-nav-name">
                            <EscapedText text={node.name} />
                        </span>
                    </div>
                    {open && (
                        <div role="group">
                            <NavigatorRows
                                nodes={node.children}
                                level={level + 1}
                                closed={closed}
                                marked={marked}
                                onToggle={onToggle}
                                onActivate={onActivate}
                            />
                        </div>
                    )}
                </div>
            )
        }
        return (
            <div
                key={`f:${node.index}`}
                className="diff-nav-row"
                role="treeitem"
                tabIndex={-1}
                aria-level={level}
                aria-current={marked === node.path ? "true" : undefined}
                data-nav-id={`f:${node.index}`}
                style={indent}
                onClick={() => onActivate(node.index, node.path)}
            >
                <span
                    className={`diff-nav-status diff-status--${node.file.status.kind}`}
                    aria-hidden="true"
                >
                    {statusLetter(node.file.status)}
                </span>
                {/* The spaces between the row's flex items draw nothing, and
                    keep its parts apart in its accessible name. */}
                <span className="sr-only">{statusLabel(node.file.status)}</span>{" "}
                <span className="diff-nav-name">
                    <EscapedText text={node.name} />
                </span>{" "}
                <Counts file={node.file} />
            </div>
        )
    })
}

// -------------------------------------------------------------------------
// File sections
// -------------------------------------------------------------------------

interface FileSectionProps {
    file: DiffFile
    index: number
    /// What the file shows now: its loaded content in place of a withheld one.
    content: DiffContent
    load: FileLoad | undefined
    collapsed: boolean
    layout: DiffLayout
    onToggle: (key: string) => void
    onLoad: (file: DiffFile) => void
    renderHeaderExtra: ((file: DiffFile) => ReactNode) | undefined
    renderPreamble: ((file: DiffFile) => ReactNode) | undefined
    /// The host's hunk slots and folds for this file, if any.
    slots: HunkSlots | undefined
    /// The folded hunks of this file the reader showed.
    shown: ReadonlySet<number> | undefined
    onShow: (key: string, index: number, shown: boolean) => void
}

/// What both layouts draw around a file's hunks: the host's slots, and which
/// hunks are folded now.
interface HunkFolds {
    slots: HunkSlots | undefined
    /// Whether the host folds hunk `index`, shown or not.
    hostFolds: (index: number) => boolean
    /// Whether hunk `index` is folded now.
    folded: (index: number) => boolean
    /// Shows hunk `index`, or hides it again.
    onShow: (index: number, shown: boolean) => void
}

/// One file (`diff-view`: *File Sections*): a sticky header that is the
/// section's own child, never a cell of a grid and outside unified's
/// sideways scroller; then the preamble slot; then the content in the layout
/// in effect, or one full-width state row. Every file keeps its section,
/// whatever its state. Memoised, so collapsing or loading one file renders
/// that file alone.
const FileSection = memo(function FileSection({
    file,
    index,
    content,
    load,
    collapsed,
    layout,
    onToggle,
    onLoad,
    renderHeaderExtra,
    renderPreamble,
    slots,
    shown,
    onShow,
}: FileSectionProps) {
    const key = fileKey(file)
    const hunks = content.kind === "hunks" ? content.hunks : null
    const folds: HunkFolds = {
        slots,
        hostFolds: (index) => slots?.folded.has(index) === true,
        folded: (index) => slots?.folded.has(index) === true && shown?.has(index) !== true,
        onShow: (index, isShown) => onShow(key, index, isShown),
    }
    const modes = shownModes(file)
    const state = contentStateLabel(content)
    const style = { "--diff-num-ch": lineNumberDigits(hunks ?? []) } as CSSProperties
    // A slot that renders nothing for this file leaves no box behind.
    const headerExtra = renderHeaderExtra?.(file)
    const preamble = renderPreamble?.(file)
    // The toggle is named by the file's path, so a long commit's toggles are
    // told apart, and its state is `aria-expanded`'s.
    const pathId = useId()
    const toggleRef = useRef<HTMLButtonElement>(null)
    // Whether "Load diff" held the focus when it was pressed: the button
    // leaves with the state row once the file lands, and the focus then moves
    // to this file's toggle rather than falling to the page.
    const refocusRef = useRef(false)
    const loading = load?.status === "loading"
    useLayoutEffect(() => {
        if (load?.status !== "loaded" || !refocusRef.current) return
        refocusRef.current = false
        toggleRef.current?.focus()
    }, [load?.status])
    return (
        <section className="diff-file" data-file-index={index} data-file-key={key} style={style}>
            <div className="diff-file-header" data-copy-line="">
                <button
                    ref={toggleRef}
                    type="button"
                    className="diff-file-toggle"
                    aria-expanded={!collapsed}
                    aria-labelledby={pathId}
                    data-copy="skip"
                    onClick={() => onToggle(key)}
                >
                    {collapsed ? <ChevronRight /> : <ChevronDown />}
                </button>{" "}
                <h2 className="diff-file-path" id={pathId}>
                    <EscapedText text={headerPath(file)} />
                </h2>{" "}
                <span className={`diff-chip diff-status diff-status--${file.status.kind}`}>
                    {statusLabel(file.status)}
                </span>{" "}
                {modes && (
                    <span className="diff-chip diff-file-modes">
                        {modes.old} → {modes.new}
                    </span>
                )}{" "}
                <Counts file={file} />{" "}
                {hunks !== null && hunksWarn(hunks) && (
                    <span className="diff-chip diff-warning">hidden characters</span>
                )}
                {hasContent(headerExtra) && (
                    <span className="diff-file-extra">{headerExtra}</span>
                )}
            </div>
            {!collapsed && (
                <div className="diff-file-body">
                    {hasContent(preamble) && (
                        <div className="diff-file-preamble">{preamble}</div>
                    )}
                    {hunks !== null && hunks.length > 0 ? (
                        layout === "split" ? (
                            <SplitHunks hunks={hunks} language={languageFor(key)} folds={folds} />
                        ) : (
                            <UnifiedHunks hunks={hunks} language={languageFor(key)} folds={folds} />
                        )
                    ) : (
                        <div
                            className="diff-state-row"
                            data-copy-line=""
                            aria-busy={loading || undefined}
                        >
                            <span>{state}</span>
                            {content.kind === "withheld" && (
                                <>
                                    {" "}
                                    <Counts file={file} />{" "}
                                    {/* `aria-disabled` rather than `disabled`, which
                                        would drop a keyboard reader's focus. */}
                                    <button
                                        type="button"
                                        className="diff-load"
                                        data-copy="skip"
                                        aria-disabled={loading || undefined}
                                        onClick={(event) => {
                                            if (loading) return
                                            refocusRef.current =
                                                document.activeElement === event.currentTarget
                                            onLoad(file)
                                        }}
                                    >
                                        {loading ? "Loading…" : "Load diff"}
                                    </button>
                                    {load?.status === "failed" && (
                                        <>
                                            {" "}
                                            <span className="diff-load-error" role="alert">
                                                {load.message}
                                            </span>
                                        </>
                                    )}
                                </>
                            )}
                        </div>
                    )}
                </div>
            )}
        </section>
    )
})

/// Whether a slot rendered anything: React draws nothing for these.
function hasContent(node: ReactNode): boolean {
    return node !== undefined && node !== null && node !== false && node !== true && node !== ""
}

function Counts({ file }: { file: DiffFile }) {
    if (file.additions === null && file.deletions === null) return null
    return (
        <span className="diff-counts">
            {file.additions !== null && (
                <span className="diff-count diff-count--added">+{file.additions}</span>
            )}
            {file.additions !== null && file.deletions !== null && " "}
            {file.deletions !== null && (
                <span className="diff-count diff-count--removed">−{file.deletions}</span>
            )}
        </span>
    )
}

// -------------------------------------------------------------------------
// Unified
// -------------------------------------------------------------------------

/// One column in the model's order (`diff-view`: *Unified and Side-by-Side
/// Layouts*). Lines never wrap: they scroll sideways in a scroller that holds
/// the lines alone, with the sticky header outside it.
///
/// Rows are drawn by plain functions rather than components, here and side by
/// side: a page at the budget holds thousands of them, and a component per row
/// costs a switch its weight in fibers for no state. A folded hunk draws its
/// heading row alone, and its tokens wait until it is shown.
function UnifiedHunks({
    hunks,
    language,
    folds,
}: {
    hunks: readonly Hunk[]
    language: string | null
    folds: HunkFolds
}) {
    return (
        <div className="diff-unified">
            <div className="diff-unified-lines">
                {hunks.map((hunk, h) => {
                    const folded = folds.folded(h)
                    const heading = (
                        <div
                            className={folded ? "diff-hunk diff-hunk--folded" : "diff-hunk"}
                            data-copy-line=""
                            data-hunk-heading={h}
                            data-hunk-folded={folded ? "" : undefined}
                        >
                            {hunkHeadingRow(hunk, h, folds, folded)}
                        </div>
                    )
                    if (folded) return <Fragment key={h}>{heading}</Fragment>
                    const tokens = hunkTokens(hunk, language)
                    const end = folds.slots?.end(h, hunk)
                    return (
                        <Fragment key={h}>
                            {heading}
                            {hunk.lines.map((line, i) => {
                                const identity = lineIdentity(line)
                                return (
                                    <div
                                        key={i}
                                        className={`diff-row diff-line--${line.kind}`}
                                        data-old-line={identity.old ?? undefined}
                                        data-new-line={identity.new ?? undefined}
                                    >
                                        <span className="diff-num" data-copy="skip">
                                            {lineNumber("old", identity.old, line.kind)}
                                        </span>
                                        <span className="diff-num" data-copy="skip">
                                            {lineNumber("new", identity.new, line.kind)}
                                        </span>
                                        <span
                                            className="diff-code"
                                            data-hunk-index={h}
                                            data-line-index={i}
                                            data-copy-line=""
                                        >
                                            {codeLine(line, tokens[unifiedSide(line.kind)][i])}
                                        </span>
                                    </div>
                                )
                            })}
                            {hasContent(end) && (
                                <div className="diff-hunk-end" data-hunk-end={h} data-copy="skip">
                                    {end}
                                </div>
                            )}
                        </Fragment>
                    )
                })}
            </div>
        </div>
    )
}

// -------------------------------------------------------------------------
// Side by side
// -------------------------------------------------------------------------

const COLUMN_HEADERS: Record<Side, readonly [string, string]> = {
    old: ["old line", "old"],
    new: ["new line", "new"],
}

/// One grid per file in two fixed, equal halves: old number, old code, new
/// number, new code, exposed as a table with visually hidden column headers.
/// A file whose every line is on one side takes one full-width column headed
/// by that side instead (`oneColumnSide`). Code wraps inside its cell and a
/// row is as tall as its taller cell, so paired lines stay level.
function SplitHunks({
    hunks,
    language,
    folds,
}: {
    hunks: readonly Hunk[]
    language: string | null
    folds: HunkFolds
}) {
    const only = oneColumnSide(hunks)
    const sides: readonly Side[] = only === null ? ["old", "new"] : [only]
    return (
        <table className={only === null ? "diff-split" : "diff-split diff-split--one"}>
            <colgroup>
                {sides.map((side) => (
                    <Fragment key={side}>
                        <col className="diff-split-num" />
                        <col />
                    </Fragment>
                ))}
            </colgroup>
            <thead>
                <tr>
                    {sides.flatMap((side) =>
                        COLUMN_HEADERS[side].map((header) => (
                            <th key={header} scope="col">
                                <span className="sr-only">{header}</span>
                            </th>
                        )),
                    )}
                </tr>
            </thead>
            <tbody>
                {hunks.map((hunk, h) => {
                    const folded = folds.folded(h)
                    const heading = (
                        <tr
                            className={
                                folded ? "diff-hunk-row diff-hunk-row--folded" : "diff-hunk-row"
                            }
                            data-hunk-heading={h}
                            data-hunk-folded={folded ? "" : undefined}
                        >
                            <td
                                className={folded ? "diff-hunk diff-hunk--folded" : "diff-hunk"}
                                colSpan={sides.length * 2}
                                data-copy-line=""
                            >
                                {hunkHeadingRow(hunk, h, folds, folded)}
                            </td>
                        </tr>
                    )
                    if (folded) return <Fragment key={h}>{heading}</Fragment>
                    const tokens = hunkTokens(hunk, language)
                    const end = folds.slots?.end(h, hunk)
                    return (
                        <Fragment key={h}>
                            {heading}
                            {only === null
                                ? splitRows(hunk).map((row, r) => splitRow(hunk, h, row, tokens, r))
                                : hunk.lines.map((line, i) => {
                                      const identity = lineIdentity(line)
                                      return (
                                          <tr
                                              key={i}
                                              className="diff-split-row"
                                              data-old-line={identity.old ?? undefined}
                                              data-new-line={identity.new ?? undefined}
                                          >
                                              {sideCells(only, line, h, i, tokens[only][i])}
                                          </tr>
                                      )
                                  })}
                            {hasContent(end) && (
                                <tr className="diff-hunk-end-row" data-hunk-end={h}>
                                    <td
                                        className="diff-hunk-end"
                                        colSpan={sides.length * 2}
                                        data-copy="skip"
                                    >
                                        {end}
                                    </td>
                                </tr>
                            )}
                        </Fragment>
                    )
                })}
            </tbody>
        </table>
    )
}

/// One side-by-side row: each half the line it shows, or a filler.
function splitRow(
    hunk: Hunk,
    hunkIndex: number,
    row: SplitRow,
    tokens: HunkTokens,
    key: number,
): ReactNode {
    const identity = splitRowIdentity(hunk, row)
    const half = (side: Side) => {
        const index = row[side]
        if (index === null) {
            return <td className="diff-filler" colSpan={2} aria-hidden="true" data-copy="skip" />
        }
        return sideCells(side, hunk.lines[index], hunkIndex, index, tokens[side][index])
    }
    return (
        <tr
            key={key}
            className="diff-split-row"
            data-old-line={identity.old ?? undefined}
            data-new-line={identity.new ?? undefined}
        >
            {half("old")}
            {half("new")}
        </tr>
    )
}

/// One half of a side-by-side row: the line's number on that side, then its
/// code. Both carry the side, which the named-side rule keys on, and the
/// line's kind, which tints them.
function sideCells(
    side: Side,
    line: Line,
    hunkIndex: number,
    lineIndex: number,
    tokens: TokenLine | null,
): ReactNode {
    const cell = `diff-line--${line.kind} diff-cell--${side}`
    return (
        <>
            <td className={`diff-num ${cell}`} data-copy="skip">
                {lineNumber(side, side === "old" ? line.oldNo : line.newNo, line.kind)}
            </td>
            <td
                className={`diff-code ${cell}`}
                data-side={side}
                data-hunk-index={hunkIndex}
                data-line-index={lineIndex}
                data-copy-line=""
            >
                <div className="diff-code-line">{codeLine(line, tokens)}</div>
            </td>
        </>
    )
}

// -------------------------------------------------------------------------
// Pieces both layouts draw
// -------------------------------------------------------------------------

/// A hunk's heading row, the same in either layout: the host's heading extra
/// in the gutter, the heading, and while the host folds the hunk, the control
/// that shows its lines or hides them again (`diff-view`: *Folded Hunks*).
/// Only the heading is copied.
function hunkHeadingRow(hunk: Hunk, index: number, folds: HunkFolds, folded: boolean): ReactNode {
    const extra = folds.slots?.heading(index, hunk)
    const control = folds.hostFolds(index) ? hunkFoldControl(hunk, !folded) : null
    return (
        <>
            {hasContent(extra) && (
                <span className="diff-hunk-gutter" data-copy="skip">
                    {extra}
                </span>
            )}
            {hunkHeading(hunk)}
            {control && (
                <>
                    {" "}
                    <button
                        type="button"
                        className="diff-hunk-action diff-hunk-fold"
                        aria-expanded={!folded}
                        aria-label={control.label}
                        data-copy="skip"
                        onClick={() => folds.onShow(index, folded)}
                    >
                        {control.text}
                    </button>
                </>
            )}
        </>
    )
}

/// A hunk header, `@@ -a,b +c,d @@`, and its section heading, drawn through
/// the escapes as context text.
function hunkHeading(hunk: Hunk): ReactNode {
    return (
        <>
            {hunkRange(hunk)}
            {hunk.section !== null && (
                <>
                    {" "}
                    <span className="diff-hunk-section">
                        <EscapedText text={hunk.section} />
                    </span>
                </>
            )}
        </>
    )
}

/// A line number, its visible digits hidden from assistive technology and
/// its side, number and kind named for it instead.
function lineNumber(side: Side, number: number | null, kind: LineKind): ReactNode {
    if (number === null) return null
    return (
        <>
            <span aria-hidden="true">{number}</span>
            <span className="sr-only">{lineNumberLabel(side, number, kind)}</span>
        </>
    )
}

/// Each line's tokens with its escapes drawn inside them, kept per token line
/// they were drawn from (a context line is drawn from both sides' tokens side
/// by side), so a layout switch draws no line again. The model's lines and
/// `hunkTokens`' arrays are stable, so both serve as keys.
const drawnByLine = new WeakMap<Line, Map<TokenLine | null, DrawnToken[]>>()

function drawnTokens(line: Line, tokens: TokenLine | null): DrawnToken[] {
    let byTokens = drawnByLine.get(line)
    if (!byTokens) {
        byTokens = new Map()
        drawnByLine.set(line, byTokens)
    }
    let drawn = byTokens.get(tokens)
    if (!drawn) {
        drawn = drawTokens(
            tokens ?? (line.text === "" ? [] : [{ text: line.text, scopes: [] }]),
            escapeLine(line).segments,
        )
        byTokens.set(tokens, drawn)
    }
    return drawn
}

/// A code cell's content: the marker, the line's tokens with its escapes
/// drawn inside them, and the no-newline badge. Only the tokens are code; the
/// marker and the badge are left out of every copy.
function codeLine(line: Line, tokens: TokenLine | null): ReactNode {
    const drawn = drawnTokens(line, tokens)
    return (
        <>
            <span className="diff-marker" aria-hidden="true" data-copy="skip">
                {MARKERS[line.kind]}
            </span>
            <span className="diff-text">
                {drawn.map((token, index) =>
                    scoped(token.scopes, token.pieces.map(drawSegment), index),
                )}
            </span>
            {line.noNewline && (
                <span className="diff-badge" data-copy="skip">
                    <span aria-hidden="true">no newline</span>
                    <span className="sr-only">No newline at end of file</span>
                </span>
            )}
        </>
    )
}

/// A token's pieces inside one span per scope, outermost first, as
/// `rehype-highlight` nests them, so the palette's descendant rules
/// (`.hljs-class .hljs-title`) match here as they do in fenced code.
function scoped(scopes: readonly string[], children: ReactNode, key: number): ReactNode {
    if (scopes.length === 0) return <Fragment key={key}>{children}</Fragment>
    const inner = scopes
        .slice(1)
        .reduceRight<ReactNode>((child, scope) => <span className={scope}>{child}</span>, children)
    return (
        <span key={key} className={scopes[0]}>
            {inner}
        </span>
    )
}

/// Text through the escapes: a path, a side name or a hunk's section heading,
/// and a host's text outside changed lines, such as a pull request's title and
/// branch names (`pull-request-viewer`: *Hidden characters in the header are
/// visible*).
export function EscapedText({ text }: { text: string }) {
    return <>{escapeText(text).segments.map(drawSegment)}</>
}

/// One drawn segment: text as itself, and an escape as its label, marked,
/// carrying its code point so a copy in document order puts the real
/// character back. The character itself is never in the page, so it can
/// reorder nothing around it.
function drawSegment(segment: Segment, key: number): ReactNode {
    if (segment.kind === "text") return segment.text
    return (
        <span
            key={key}
            className={segment.warns ? "diff-escape diff-escape--warns" : "diff-escape"}
            data-code-point={segment.text.codePointAt(0)?.toString(16)}
        >
            {segment.label}
        </span>
    )
}
