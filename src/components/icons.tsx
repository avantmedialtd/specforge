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
