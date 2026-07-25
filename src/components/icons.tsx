/// Hand-rolled inline SVG icons. Kept here to stay dependency-free —
/// the surface we need is small enough that vendoring Lucide would be
/// overkill. All icons use a 24×24 viewBox, currentColor, and 1.5 stroke.

interface IconProps {
    width?: number
    height?: number
    className?: string
    title?: string
}

function Svg({
    width = 14,
    height = 14,
    className,
    title,
    children,
}: IconProps & { children: React.ReactNode }) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width={width}
            height={height}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={className}
            aria-hidden={title ? undefined : true}
            role={title ? "img" : undefined}
        >
            {title && <title>{title}</title>}
            {children}
        </svg>
    )
}

export function ChevronRight(props: IconProps) {
    return (
        <Svg {...props}>
            <polyline points="9 6 15 12 9 18" />
        </Svg>
    )
}

export function ChevronDown(props: IconProps) {
    return (
        <Svg {...props}>
            <polyline points="6 9 12 15 18 9" />
        </Svg>
    )
}

/// Gear (Lucide `settings`). Paired with a text label in the sidebar
/// footer so the 8-tooth silhouette has room to read at 18px+.
export function Settings(props: IconProps) {
    return (
        <Svg {...props}>
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
        </Svg>
    )
}

/// Four-panel grid (Lucide `layout-dashboard`). Paired with a text label in
/// the sidebar header, mirroring the Settings footer entry.
export function Dashboard(props: IconProps) {
    return (
        <Svg {...props}>
            <rect x="3" y="3" width="7" height="9" rx="1" />
            <rect x="14" y="3" width="7" height="5" rx="1" />
            <rect x="14" y="12" width="7" height="9" rx="1" />
            <rect x="3" y="16" width="7" height="5" rx="1" />
        </Svg>
    )
}

/// Archive box (Lucide `archive`). Paired with a text label in the sidebar
/// footer, above the Settings entry.
export function Archive(props: IconProps) {
    return (
        <Svg {...props}>
            <rect width="20" height="5" x="2" y="3" rx="1" />
            <path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" />
            <path d="M10 12h4" />
        </Svg>
    )
}

export function Close(props: IconProps) {
    return (
        <Svg {...props}>
            <line x1="6" y1="6" x2="18" y2="18" />
            <line x1="18" y1="6" x2="6" y2="18" />
        </Svg>
    )
}

export function Check(props: IconProps) {
    return (
        <Svg {...props}>
            <polyline points="5 13 10 18 19 7" />
        </Svg>
    )
}

/// Completion mark — the row grammar's "done" fill, the symmetric partner to
/// the in-progress task-progress meter. A solid disc with a knocked-out check;
/// colours resolve from `.completion-mark` in App.css (disc `--ok-strong`, check
/// punched through in `--surface`). Larger than a 4px status dot and carrying
/// an interior check, so it never reads as a status dot. Does NOT use the `Svg`
/// wrapper: the disc is filled (not a `currentColor` stroke) and its two parts
/// take different tokens.
export function CompletionMark({
    width = 15,
    height = 15,
    className,
    title,
}: IconProps) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width={width}
            height={height}
            viewBox="0 0 24 24"
            className={`completion-mark${className ? ` ${className}` : ""}`}
            aria-hidden={title ? undefined : true}
            role={title ? "img" : undefined}
        >
            {title && <title>{title}</title>}
            <circle className="completion-mark-disc" cx="12" cy="12" r="10" />
            <polyline
                className="completion-mark-check"
                points="7.5 12.5 10.5 15 16.5 8.5"
            />
        </svg>
    )
}

/// Filled circle — for emphatic state indicators (e.g. activity badges).
/// Renders solid in currentColor with no stroke.
export function DotFilled({ width = 14, height = 14, className, title }: IconProps) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width={width}
            height={height}
            viewBox="0 0 24 24"
            fill="currentColor"
            stroke="none"
            className={className}
            aria-hidden={title ? undefined : true}
            role={title ? "img" : undefined}
        >
            {title && <title>{title}</title>}
            <circle cx="12" cy="12" r="4" />
        </svg>
    )
}

/// Outlined circle — neutral / absent indicator.
export function DotOutline(props: IconProps) {
    return (
        <Svg {...props}>
            <circle cx="12" cy="12" r="4" />
        </Svg>
    )
}

/// Task checkbox "done" glyph — the markdown view's squared sibling of the
/// completion mark: a solid rounded square with a knocked-out check, using
/// the same check geometry, stroke-width, and cap/join style as
/// `.completion-mark-check` so the two "done" marks read as one family.
/// Colours resolve from `.task-check-mark-box` / `.task-check-mark-check`
/// in App.css (box fill `--ok-strong`, check punched through in `--bg` —
/// the plane every `.markdown-view` surface sits on, unlike the sidebar
/// tree's `--surface` plane). Does NOT use the `Svg` wrapper: the box is
/// filled (not a `currentColor` stroke) and its two parts take different
/// tokens.
export function TaskCheckMark({
    width = 16,
    height = 16,
    className,
    title,
}: IconProps) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width={width}
            height={height}
            viewBox="0 0 24 24"
            className={className}
            aria-hidden={title ? undefined : true}
            role={title ? "img" : undefined}
        >
            {title && <title>{title}</title>}
            <rect
                className="task-check-mark-box"
                x="3"
                y="3"
                width="18"
                height="18"
                rx="4.5"
            />
            <polyline
                className="task-check-mark-check"
                points="7.5 12.5 10.5 15 16.5 8.5"
            />
        </svg>
    )
}

export function CheckSquare(props: IconProps) {
    return (
        <Svg {...props}>
            <rect x="4" y="4" width="16" height="16" rx="2" />
            <polyline points="8 12 11 15 16 9" />
        </Svg>
    )
}

export function Square(props: IconProps) {
    return (
        <Svg {...props}>
            <rect x="4" y="4" width="16" height="16" rx="2" />
        </Svg>
    )
}
