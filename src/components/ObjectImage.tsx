import { useLayoutEffect, useState } from "react"
import type { DecodedImage } from "../diffImage"

/// An image drawn from bytes in memory (`diff-view`: *Image Comparison*): an
/// object URL made for this mount and revoked when it unmounts, never a URL
/// that names a host. Its box is reserved from the dimensions the service
/// read before it decodes. The inline comparison and the maximized one both
/// draw through it.
export function ObjectImage({
    image,
    alt,
    className,
    onError,
}: {
    image: DecodedImage
    alt: string
    className?: string
    onError?: () => void
}) {
    const [url, setUrl] = useState<string | null>(null)
    useLayoutEffect(() => {
        const made = URL.createObjectURL(new Blob([image.bytes], { type: image.mime }))
        setUrl(made)
        return () => URL.revokeObjectURL(made)
    }, [image])
    return (
        <img
            className={className}
            src={url ?? undefined}
            width={image.width}
            height={image.height}
            alt={alt}
            decoding="async"
            draggable={false}
            onError={onError}
        />
    )
}
