import { useContext, useEffect, useId, useMemo, useState, type ReactNode } from "react"
import { getPullRequestContentImage } from "../api"
import { decodeBase64, refusalText } from "../diffImage"
import { canonicalPrImageSource } from "../prContentLoading"
import type { PullRequestContentImageOutcome } from "../types"
import { ObjectImage } from "./ObjectImage"
import { PrImageLinkRetries, PublishedPrImageBody, useNearPrRead } from "./PullRequestLoadingContext"

export function contentImageFailure(outcome: PullRequestContentImageOutcome): string | null {
    switch (outcome.kind) {
        case "image": return null
        case "refused": return refusalText(outcome.reason)
        case "stale": return "This published image is no longer available."
        case "failed":
            switch (outcome.reason) {
                case "deferred": return "Image reads are rate limited. Try again later."
                case "transient": return "The image host did not answer."
                case "unsafeAddress": case "unsafeSource": return "This image source is not permitted."
                case "redirected": return "The image redirected somewhere SpecForge cannot follow."
                case "unavailable": return "The image is unavailable without credentials."
                case "refused": return "This image cannot be previewed."
            }
    }
}

export function PullRequestContentImage({ source, alt, fallback, insideLink }: {
    source: string
    alt: string
    fallback: ReactNode
    insideLink: boolean
}) {
    const body = useContext(PublishedPrImageBody)
    const retries = useContext(PrImageLinkRetries)
    const canonical = canonicalPrImageSource(source)
    const key = `embedded:${body?.digest ?? "unpublished"}:${canonical ?? "unsupported"}`
    const read = useNearPrRead<PullRequestContentImageOutcome>(key, "image",
        () => body && canonical ? getPullRequestContentImage(body.reference, body.digest, canonical)
            : Promise.resolve({ kind: "stale" }), !!body && !!canonical)
    const outcome = read.state.status === "ready" ? read.state.value : null
    const image = useMemo(() => {
        if (outcome?.kind !== "image") { return null }
        try {
            return { mime: outcome.mime, width: outcome.width, height: outcome.height, bytes: decodeBase64(outcome.data) }
        } catch { return null }
    }, [outcome])
    const [undrawable, setUndrawable] = useState<string | null>(null)
    const failed = read.state.status === "failed" ? "The image could not be read."
        : outcome ? contentImageFailure(outcome) : null
    const retryable = read.state.status === "failed" || (outcome?.kind === "failed"
        && (outcome.reason === "transient" || outcome.reason === "deferred"))
    const retry = read.retry
    const control = useMemo(() => retryable && read.preferences?.autoShowImages
        ? <button type="button" className="pr-content-image-retry" onClick={retry}>Try again</button> : null,
    [retryable, read.preferences?.autoShowImages, retry])
    const id = useId()
    useEffect(() => {
        if (!insideLink || !retries) { return }
        retries(id, control)
        return () => retries(id, null)
    }, [insideLink, retries, id, control])

    if (!body || !canonical) { return fallback }
    const host = new URL(canonical).hostname
    const finalHost = outcome?.kind === "image" ? outcome.finalHost : host
    const shown = image && undrawable !== key
    return <span ref={read.ref} className="pr-content-image" data-preview-state={read.state.status}>
        {shown ? <>
            <ObjectImage image={image} alt={alt || "image"} className="pr-content-image-preview"
                onError={() => setUndrawable(key)} />
            <span className="pr-content-image-caption">{fallback}{host !== finalHost && ` → ${finalHost}`}</span>
        </> : <>
            {fallback}
            {read.state.status === "loading" && <span className="pr-content-image-status" role="status">Loading image…</span>}
            {(failed || undrawable === key) && <span className="pr-content-image-status" role="status">
                {undrawable === key ? "This image cannot be shown here." : failed}
            </span>}
        </>}
        {!insideLink && control}
    </span>
}
