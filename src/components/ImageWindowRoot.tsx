import { useCallback, useEffect, useMemo, useState } from "react"
import { getCommitFileImage, getPullRequestFileImage } from "../api"
import {
    DIFFERENCE_CAPTION,
    UNDRAWABLE_TEXT,
    decodeVersions,
    differenceAvailable,
    differenceName,
    holdsNoImage,
    imageDetails,
    refusalText,
    versionName,
    type DecodedVersions,
    type ImageMode,
} from "../diffImage"
import { type Side } from "../diffLayout"
import { closeDetachedWindow, useDetachedWindow } from "../hooks/useDetachedWindow"
import { escapeText } from "../hiddenChars"
import {
    imageWindowTitle,
    parseImageWindowAddress,
    type ImageWindowSource,
} from "../imageWindow"
import { fileFailureText } from "../pullRequestView"
import type { ImageVersions } from "../types"
import { EscapedText } from "./DiffView"
import { EmptyState } from "./EmptyState"
import { ImageZoom, type ZoomFrame } from "./ImageZoom"

const MODE_OPTIONS = [
    { value: "both" as const, label: "Both" },
    { value: "difference" as const, label: "Difference" },
]

const NO_SIDES: ReadonlySet<Side> = new Set()

/// What the window says when the pull request it was opened from has been
/// read again with other commits since: it cannot follow, and the view can.
const PULL_REQUEST_CHANGED_TEXT =
    "This pull request has changed since this window opened. Open the image again from the pull request."

/// The zoom window remembers no size of its own: each opens at the same size,
/// clamped to the work area.
async function keepNoSize(): Promise<void> {}

/// What the window reads its versions through: its host's own command, so a
/// commit's read is local and a pull request's is an image read, governed as
/// any is. A pull request that has moved on since, or a read that fails, says
/// why in the pull-request view's own words.
async function readVersions(source: ImageWindowSource): Promise<ImageVersions> {
    if (source.kind === "commit") {
        return getCommitFileImage(source.repoId, source.sha, source.path, source.oldPath ?? undefined)
    }
    const outcome = await getPullRequestFileImage(
        source.reference,
        source.path,
        source.head,
        source.base,
    )
    switch (outcome.kind) {
        case "images":
            return { old: outcome.old, new: outcome.new }
        case "changed":
            throw PULL_REQUEST_CHANGED_TEXT
        case "failed":
            throw fileFailureText(source.reference.provider, outcome.reason, outcome.untilUnix)
    }
}

/// Text through the escapes as plain text, for an image's accessible name.
function plainEscaped(text: string): string {
    return escapeText(text)
        .segments.map((segment) => (segment.kind === "text" ? segment.text : segment.label))
        .join("")
}

type WindowRead =
    | { status: "reading" }
    | { status: "failed"; message: string }
    | { status: "read"; versions: DecodedVersions }

/// The zoom window: one image file's versions apart from the main window
/// (`diff-view`: *Image Comparison*, Zooming; design D11). A native window on
/// the desktop and a tab in the browser skin, both carrying the address in
/// `at`. It reads the versions itself, from the address, and refuses any
/// address it cannot read as one. It closes on Escape and Command/Control-W,
/// and its Both/Difference choice is its own.
export function ImageWindowRoot() {
    const source = useMemo(
        () => parseImageWindowAddress(new URLSearchParams(window.location.search).get("at")),
        [],
    )
    useDetachedWindow(source ? imageWindowTitle(source) : "SpecForge", keepNoSize)

    const [read, setRead] = useState<WindowRead>({ status: "reading" })
    useEffect(() => {
        if (source === null) return
        let cancelled = false
        readVersions(source).then(
            (versions) => {
                if (!cancelled) setRead({ status: "read", versions: decodeVersions(versions) })
            },
            (error: unknown) => {
                if (!cancelled) setRead({ status: "failed", message: String(error) })
            },
        )
        return () => {
            cancelled = true
        }
    }, [source])

    const [mode, setMode] = useState<ImageMode>("both")
    const [undrawable, setUndrawable] = useState<ReadonlySet<Side>>(NO_SIDES)
    const markUndrawable = useCallback(
        (side: Side) => setUndrawable((previous) => new Set(previous).add(side)),
        [],
    )

    if (source === null) {
        return (
            <EmptyState
                title="Image not found"
                body="This window's address does not name an image file's versions."
            />
        )
    }
    if (read.status !== "read") {
        return (
            <div className="image-window-status" role={read.status === "failed" ? "alert" : undefined}>
                {read.status === "reading" ? `Reading ${source.path}…` : read.message}
            </div>
        )
    }
    const { versions } = read
    if (holdsNoImage(versions)) {
        return <div className="image-window-status">Neither version is an image this view can show.</div>
    }

    const path = plainEscaped(source.path)
    const sides = source.sides
    const offered = differenceAvailable(versions) && undrawable.size === 0
    const before = versions.old.kind === "image" ? versions.old.image : undefined
    const frames: ZoomFrame[] =
        offered && mode === "difference" && versions.old.kind === "image" && versions.new.kind === "image"
            ? [
                  {
                      kind: "difference",
                      old: versions.old.image,
                      next: versions.new.image,
                      name: differenceName(path, plainEscaped(sides.old), plainEscaped(sides.new)),
                      caption: DIFFERENCE_CAPTION,
                  },
              ]
            : (["old", "new"] as const)
                  .filter((side) => versions[side].kind !== "absent")
                  .map((side): ZoomFrame => {
                      const version = versions[side]
                      if (version.kind === "image" && !undrawable.has(side)) {
                          return {
                              kind: "image",
                              image: version.image,
                              name: versionName(path, plainEscaped(sides[side])),
                              caption: (
                                  <>
                                      <EscapedText text={sides[side]} />
                                      <span className="image-zoom__details">
                                          {" · "}
                                          {imageDetails(version.image, side === "new" ? before : undefined)}
                                      </span>
                                  </>
                              ),
                              onError: () => markUndrawable(side),
                          }
                      }
                      return {
                          kind: "note",
                          text: version.kind === "refused" ? refusalText(version.reason) : UNDRAWABLE_TEXT,
                          caption: <EscapedText text={sides[side]} />,
                      }
                  })

    return (
        <ImageZoom
            label={source.path}
            frames={frames}
            mode={offered ? { value: mode, options: MODE_OPTIONS, onChange: setMode } : null}
            onClose={closeDetachedWindow}
        />
    )
}
