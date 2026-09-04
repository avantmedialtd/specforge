/// The reading-width ladder: which pair of widths each rung stands for, and how
/// a stored value is read back safely.
///
/// Kept out of the components that consume it so it is unit-testable without a
/// DOM — the same reasoning `figureFloor.ts` and `figureZoom.ts` state in their
/// own headers. This repository has no component-test infrastructure, and a
/// `src/`-only diff short-circuits the mutation gate (`.cargo/mutants.toml`
/// scopes it to `openspec-core` + `openspec-app`), so this module and its tests
/// are the ladder's only automated coverage. Every export is a total function of
/// its arguments; the only ambient thing touched is `localStorage`, and only
/// through the two mirror helpers, which take an injectable store.

import type { DocumentWidth } from "./types"

/// The two tiers of the content column, as CSS values ready to be assigned to
/// the custom properties `--doc-column` and `--doc-measure`.
export interface DocWidthTokens {
    /// The OBJECT tier: tables, fenced code, diagrams, SVG images, display
    /// mathematics. `"none"` at `full`, where objects take the whole surface.
    column: string
    /// The PROSE tier, in `ch` of the prose font. Bounded at every rung,
    /// `full` included — the widest rung widens the objects, not the text.
    measure: string
}

/// The rung an unrecognised or absent value resolves to. Reproduces the
/// rendering the reading surface had before the ladder existed.
export const DEFAULT_DOCUMENT_WIDTH: DocumentWidth = "default"

/// The ladder itself.
///
/// The object column steps by a constant 160px. The measure does NOT step
/// evenly, and that is deliberate — see below.
///
/// Measured line lengths, taken from real line boxes in Inter at `--text-lg`
/// rather than estimated (`ch` is the advance of the digit zero, 10.09px here,
/// while an average prose character is ~7.6px — so a `ch` figure overstates
/// the characters it buys by about a quarter):
///
/// | rung      | measure |   px | prose chars | column | code chars |
/// |-----------|---------|------|-------------|--------|------------|
/// | `compact` |  `50ch` |  505 |         ~65 |  720px |        ~76 |
/// | `default` |  `74ch` |  747 |         ~97 |  880px |        ~94 |
/// | `wide`    |  `86ch` |  868 |        ~113 | 1040px |       ~112 |
/// | `full`    |  `96ch` |  969 |        ~125 |   none |       pane |
///
/// `compact` tightens prose proportionally more than it tightens the column
/// (70% of it, against ~85% at the other bounded rungs). That is the point of
/// the rung: a reader reaches for it because the TEXT feels too wide, and a
/// rung that narrowed both in step would leave prose at ~83 characters —
/// still outside the range conventionally called comfortable. At 50ch it
/// lands at ~65, which is that range.
///
/// `default` is the pre-existing rendering exactly, so an installation that
/// never chooses anything does not move — including its ~97-character line,
/// which is wider than the `visual-identity` spec's own prose once claimed.
/// Correcting the claim is in scope for this change; changing the default
/// rendering is not.
///
/// These are the ONLY place the pixel figures live. `crates/openspec-app`
/// stores the rung's *name* and knows nothing of its widths, so there is no
/// second copy to keep in step.
export const DOC_WIDTHS: Record<DocumentWidth, DocWidthTokens> = {
    compact: { column: "720px", measure: "50ch" },
    default: { column: "880px", measure: "74ch" },
    wide: { column: "1040px", measure: "86ch" },
    full: { column: "none", measure: "96ch" },
}

/// The rungs in ladder order, for rendering a picker.
export const DOC_WIDTH_ORDER: readonly DocumentWidth[] = [
    "compact",
    "default",
    "wide",
    "full",
]

/// Human labels for the picker. Kept beside the ladder so a new rung cannot be
/// added without one.
export const DOC_WIDTH_LABELS: Record<DocumentWidth, string> = {
    compact: "Compact",
    default: "Default",
    wide: "Wide",
    full: "Full",
}

/// Fold any value to a rung.
///
/// The backend applies the same rule when loading its settings file, and for
/// the same reason: the Rust enum and the TypeScript union are hand-mirrored
/// with no codegen, so a value neither side agrees on must land somewhere
/// harmless rather than propagate as an error into a startup path.
export function normalizeDocumentWidth(value: unknown): DocumentWidth {
    // `hasOwnProperty`, not `in`: `in` walks the prototype chain, so
    // `"constructor" in DOC_WIDTHS` is true and would admit `Object` itself as
    // a rung — leaving `docWidthTokens` to hand back `undefined` widths and the
    // bootstrap to stamp them.
    return typeof value === "string" &&
        Object.prototype.hasOwnProperty.call(DOC_WIDTHS, value)
        ? (value as DocumentWidth)
        : DEFAULT_DOCUMENT_WIDTH
}

/// The tokens for a value, normalising it first. Total for any input.
export function docWidthTokens(value: unknown): DocWidthTokens {
    return DOC_WIDTHS[normalizeDocumentWidth(value)]
}

/// Where the first-paint mirror lives.
export const DOC_WIDTH_STORAGE_KEY = "specforge:docWidth"

/// The subset of `Storage` the mirror uses, so tests can pass a fake and the
/// helpers never depend on a DOM being present.
export interface DocWidthStore {
    getItem(key: string): string | null
    setItem(key: string, value: string): void
}

/// The ambient store, or `null` where there isn't one.
///
/// Reading `globalThis.localStorage` can itself throw — a browser set to block
/// site data does exactly that, as does a non-browser runtime — so the access
/// is guarded, not just the call.
function ambientStore(): DocWidthStore | null {
    try {
        return globalThis.localStorage ?? null
    } catch {
        return null
    }
}

/// Read the mirrored rung. Never throws and never returns anything but a rung:
/// this runs before React mounts, on the path that paints the first frame, and
/// a storage exception there would take the whole application down rather than
/// cost it a preference.
export function readMirroredDocumentWidth(
    store: DocWidthStore | null = ambientStore(),
): DocumentWidth {
    try {
        return normalizeDocumentWidth(store?.getItem(DOC_WIDTH_STORAGE_KEY))
    } catch {
        return DEFAULT_DOCUMENT_WIDTH
    }
}

/// Mirror the rung for the next cold start. Best-effort by design — the
/// authoritative value is in the application settings, and this store failing
/// costs one frame at the default rung on some later launch, nothing more.
export function writeMirroredDocumentWidth(
    width: DocumentWidth,
    store: DocWidthStore | null = ambientStore(),
): void {
    try {
        store?.setItem(DOC_WIDTH_STORAGE_KEY, width)
    } catch {
        // Deliberately swallowed; see above.
    }
}
