import { useDashboard } from "../hooks/useDashboard"
import type { ActivityBucket } from "../types"
import { EmptyState } from "./EmptyState"

interface DashboardViewProps {
    /// Navigate to a change's proposal — wired to the same selection contract
    /// the tree uses. Driven by the recent-activity feed.
    onOpenChange: (worktreePath: string, changeId: string) => void
}

/// `YYYY-MM-DD` for a Date in the viewer's local time zone, matching the
/// commit-graph rail's local-day grouping.
function localDayKey(d: Date): string {
    const y = d.getFullYear()
    const m = String(d.getMonth() + 1).padStart(2, "0")
    const day = String(d.getDate()).padStart(2, "0")
    return `${y}-${m}-${day}`
}

/// The day axis the chart renders: `windowDays` local calendar days ending
/// today (newest last), so empty days still occupy a column.
function buildAxis(windowDays: number): Date[] {
    const today = new Date()
    today.setHours(0, 0, 0, 0)
    const days: Date[] = []
    for (let i = windowDays - 1; i >= 0; i--) {
        const d = new Date(today)
        d.setDate(today.getDate() - i)
        days.push(d)
    }
    return days
}

function formatDuration(secs: number): string {
    const days = secs / 86_400
    if (days >= 1) return days < 10 ? `${days.toFixed(1)}d` : `${Math.round(days)}d`
    const hours = secs / 3_600
    if (hours >= 1) return `${Math.round(hours)}h`
    return `${Math.max(1, Math.round(secs / 60))}m`
}

function relativeTime(unixSecs: number): string {
    if (!unixSecs) return ""
    const diff = Date.now() / 1000 - unixSecs
    if (diff < 60) return "just now"
    const mins = Math.floor(diff / 60)
    if (mins < 60) return `${mins}m ago`
    const hours = Math.floor(mins / 60)
    if (hours < 24) return `${hours}h ago`
    return `${Math.floor(hours / 24)}d ago`
}

function ActivityChart({
    activity,
    windowDays,
}: {
    activity: ActivityBucket[]
    windowDays: number
}) {
    const counts = new Map(activity.map((b) => [b.day, b.commitCount]))
    const axis = buildAxis(windowDays)
    const max = Math.max(1, ...axis.map((d) => counts.get(localDayKey(d)) ?? 0))
    const total = axis.reduce((sum, d) => sum + (counts.get(localDayKey(d)) ?? 0), 0)

    if (total === 0) {
        return (
            <div className="dashboard-chart dashboard-chart--empty">
                <span>No commits in the last {windowDays} days</span>
            </div>
        )
    }

    return (
        <div className="dashboard-chart" role="img" aria-label={`${total} commits over ${windowDays} days`}>
            {axis.map((d) => {
                const key = localDayKey(d)
                const count = counts.get(key) ?? 0
                const height = Math.round((count / max) * 100)
                return (
                    <div
                        key={key}
                        className="dashboard-bar-col"
                        title={`${key}: ${count} commit${count === 1 ? "" : "s"}`}
                    >
                        <div
                            className="dashboard-bar"
                            style={{ height: `${Math.max(count > 0 ? 6 : 0, height)}%` }}
                        />
                    </div>
                )
            })}
        </div>
    )
}

export function DashboardView({ onOpenChange }: DashboardViewProps) {
    const { data, error } = useDashboard()

    if (error && !data) {
        return (
            <EmptyState
                title="Couldn't load the dashboard"
                body={<code className="detail-pane-error">{error}</code>}
            />
        )
    }

    if (!data) {
        return <div className="detail-pane-status">Loading…</div>
    }

    const { summary, repos, activity, activityWindowDays, lifecycle, recent } = data
    const totalArchived = repos.reduce((sum, r) => sum + r.archivedCount, 0)
    const noWorkspaces = summary.repoCount === 0 && summary.flatCount === 0
    const maxRepoActive = Math.max(1, ...repos.map((r) => r.activeCount))

    if (noWorkspaces) {
        return (
            <EmptyState
                title="No workspaces yet"
                body="Register an OpenSpec workspace from Settings to see an overview of your changes here."
            />
        )
    }

    return (
        <div className="dashboard">
            <header className="dashboard-header">
                <h1>Dashboard</h1>
                <span className="dashboard-subtitle">
                    {summary.activeChanges} active · {totalArchived} archived
                </span>
            </header>

            <section className="dashboard-cards">
                <div className="dashboard-card">
                    <span className="dashboard-card-value">{summary.activeChanges}</span>
                    <span className="dashboard-card-label">Active changes</span>
                </div>
                <div className="dashboard-card">
                    <span className="dashboard-card-value">
                        {summary.completedTasks}
                        <span className="dashboard-card-value-sub"> / {summary.totalTasks}</span>
                    </span>
                    <span className="dashboard-card-label">Tasks · {summary.taskPercent}%</span>
                    <div className="dashboard-meter">
                        <div
                            className="dashboard-meter-fill"
                            style={{ width: `${summary.taskPercent}%` }}
                        />
                    </div>
                </div>
                <div className="dashboard-card">
                    <span className="dashboard-card-value">{summary.specsTouching}</span>
                    <span className="dashboard-card-label">Changes touch specs</span>
                </div>
                <div className="dashboard-card">
                    <span className="dashboard-card-value">{summary.repoCount}</span>
                    <span className="dashboard-card-label">
                        {summary.repoCount === 1 ? "repo" : "repos"} ·{" "}
                        {summary.worktreeCount} worktrees
                        {summary.flatCount > 0 ? ` · ${summary.flatCount} flat` : ""}
                    </span>
                </div>
            </section>

            <div className="dashboard-grid">
                <section className="dashboard-panel">
                    <h2 className="dashboard-panel-title">
                        Activity · last {activityWindowDays} days
                    </h2>
                    <ActivityChart activity={activity} windowDays={activityWindowDays} />
                    <div className="dashboard-lifecycle">
                        <span>
                            <strong>{lifecycle.archivedInWindow}</strong> archived this window
                        </span>
                        <span>
                            avg time-to-archive{" "}
                            <strong>
                                {lifecycle.avgTimeToArchiveSecs != null
                                    ? formatDuration(lifecycle.avgTimeToArchiveSecs)
                                    : "—"}
                            </strong>
                        </span>
                    </div>
                </section>

                <section className="dashboard-panel">
                    <h2 className="dashboard-panel-title">Recent</h2>
                    {recent.length === 0 ? (
                        <p className="dashboard-empty-note">No active changes.</p>
                    ) : (
                        <ul className="dashboard-recent">
                            {recent.map((entry) => (
                                <li key={`${entry.worktreePath}:${entry.changeId}`}>
                                    <button
                                        type="button"
                                        className="dashboard-recent-row"
                                        onClick={() =>
                                            onOpenChange(entry.worktreePath, entry.changeId)
                                        }
                                    >
                                        <span className="dashboard-recent-title">
                                            {entry.title ?? entry.changeId}
                                        </span>
                                        <span className="dashboard-recent-meta">
                                            <span className="dashboard-recent-ws">
                                                {entry.workspaceLabel}
                                            </span>
                                            {entry.modifiedAt > 0 && (
                                                <span className="dashboard-recent-time">
                                                    {relativeTime(entry.modifiedAt)}
                                                </span>
                                            )}
                                        </span>
                                    </button>
                                </li>
                            ))}
                        </ul>
                    )}
                </section>
            </div>

            <section className="dashboard-panel">
                <h2 className="dashboard-panel-title">Per repository</h2>
                <ul className="dashboard-breakdown">
                    {repos.map((repo) => (
                        <li key={repo.label} className="dashboard-breakdown-row">
                            <span className="dashboard-breakdown-label">{repo.label}</span>
                            <span className="dashboard-breakdown-track">
                                <span
                                    className="dashboard-breakdown-fill"
                                    style={{
                                        width: `${Math.round((repo.activeCount / maxRepoActive) * 100)}%`,
                                    }}
                                />
                            </span>
                            <span className="dashboard-breakdown-counts">
                                {repo.activeCount} active · {repo.archivedCount} archived
                            </span>
                        </li>
                    ))}
                </ul>
            </section>
        </div>
    )
}
