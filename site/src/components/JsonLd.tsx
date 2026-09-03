/**
 * Emits one structured-data block.
 *
 * Kept local: this site carries its own visual identity and depends on no shared
 * component library, and taking a package dependency for a few lines of
 * `<script>` would undo that for no gain.
 *
 * `JSON.stringify` does not escape `<`, so a value containing `</script>` would
 * close the element early and the remainder would parse as markup. Every value
 * here is an author-written constant, but escaping the sequence costs nothing
 * and removes the failure mode rather than relying on that staying true.
 */
export function JsonLd({ data }: { data: Record<string, unknown> }) {
    const json = JSON.stringify(data).replace(/</g, '\\u003c');
    return <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: json }} />;
}
