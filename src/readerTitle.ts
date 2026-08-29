/// What a reader window calls itself.
///
/// A reader window carries a native titlebar precisely so the title can name
/// the document — it is the only thing in that window that says what is being
/// read, and on macOS it is also the entry the Window menu lists. So the title
/// has to distinguish two documents a user is likely to have open side by side,
/// and "spec.md" alone does not: every capability spec in the repository is
/// called that.
///
/// The shape follows the platform convention of most-specific-first, narrowing
/// left to right: `<file> — <what it belongs to> — <where it lives>`.
///
/// Pure and host-free, so it is testable without a window
/// (`reader-window`: *Reader Window Title and Titlebar*).

import type { Address } from "./routing/address"

/// The file name an artifact kind maps to on disk — the same mapping
/// `documentPath` uses, expressed here as a leaf name.
function artifactFileName(address: Extract<Address, { kind: "artifact" }>): string {
    return address.artifactKind === "spec" ? "spec.md" : `${address.artifactKind}.md`
}

/// The part of an artifact address that says what the file belongs to: the
/// capability for a spec (because every capability's file is `spec.md`), the
/// change otherwise.
function artifactContext(address: Extract<Address, { kind: "artifact" }>): string[] {
    return address.artifactKind === "spec" && address.capability
        ? [address.capability, address.changeId]
        : [address.changeId]
}

/// The directory a file sits in, or nothing when it sits at the browse root.
function parentDirectory(path: string): string[] {
    const segments = path.split("/").filter((s) => s.length > 0)
    const parent = segments.at(-2)
    return parent ? [parent] : []
}

function baseName(path: string): string {
    const segments = path.split("/").filter((s) => s.length > 0)
    return segments.at(-1) ?? path
}

/// The title for a reader window showing `address`, in a workspace displayed as
/// `label`. Returns an empty string for an address that names no document, so a
/// caller cannot accidentally title a window after a browse root.
export function readerTitle(address: Address, label: string): string {
    const parts =
        address.kind === "file"
            ? [baseName(address.path), ...parentDirectory(address.path), label]
            : address.kind === "artifact"
              ? [artifactFileName(address), ...artifactContext(address), label]
              : []
    // A blank label (an unnamed workspace) must not leave a dangling separator.
    return parts.filter((part) => part.length > 0).join(" — ")
}
