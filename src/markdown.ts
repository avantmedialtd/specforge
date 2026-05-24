/**
 * Strip common inline-markdown syntax from a string so it renders as plain
 * text in places (like the sidebar tree) where we don't run a markdown
 * renderer.
 *
 * Handles: inline code (`x`), bold (**x** / __x__), italic (*x* / _x_),
 * and links ([text](url)). Order matters — bold patterns are tried
 * before italic so the `*` characters in `**` aren't mistaken for italic
 * delimiters.
 */
export function stripInlineMarkdown(text: string): string {
    return text
        .replace(/`([^`]+)`/g, "$1")
        .replace(/\*\*([^*]+)\*\*/g, "$1")
        .replace(/__([^_]+)__/g, "$1")
        .replace(/\*([^*]+)\*/g, "$1")
        .replace(/(^|[\s_])_([^_]+)_(?=[\s_]|$)/g, "$1$2")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
}
