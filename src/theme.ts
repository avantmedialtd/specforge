/**
 * Reads a design token's current value off `:root`. Shared by MermaidBlock
 * (many tokens per render — resolve `getComputedStyle` once and pass it as
 * `styles` so each token lookup doesn't recompute style) and SvgBlock (one
 * token, happy to let `styles` default and resolve it itself).
 */
export function readToken(
    name: string,
    styles: CSSStyleDeclaration = getComputedStyle(document.documentElement),
): string {
    return styles.getPropertyValue(name).trim()
}
