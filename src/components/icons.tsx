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

/// Three-row tuning-knobs / sliders. Reads as "settings" without the
/// visual noise of a 6-tooth gear at 14px.
export function Settings(props: IconProps) {
    return (
        <Svg {...props}>
            <line x1="4" y1="6" x2="20" y2="6" />
            <circle cx="14" cy="6" r="2" />
            <line x1="4" y1="12" x2="20" y2="12" />
            <circle cx="8" cy="12" r="2" />
            <line x1="4" y1="18" x2="20" y2="18" />
            <circle cx="16" cy="18" r="2" />
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
