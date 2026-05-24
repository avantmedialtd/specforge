import type { ReactNode } from "react"

interface EmptyStateProps {
    title: string
    body?: ReactNode
}

export function EmptyState({ title, body }: EmptyStateProps) {
    return (
        <div className="empty-state">
            <div className="empty-state-title">{title}</div>
            {body && <div className="empty-state-body">{body}</div>}
        </div>
    )
}
