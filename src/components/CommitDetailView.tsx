import { useEffect, useState } from "react"
import { getCommitDetail, getCommitDiff } from "../api"
import type { CommitFile, CommitRenderTarget } from "../types"

interface CommitDetailViewProps {
    target: CommitRenderTarget
}

export function CommitDetailView({ target }: CommitDetailViewProps) {
    const { repoId, commit } = target
    const [files, setFiles] = useState<CommitFile[] | null>(null)
    const [diffs, setDiffs] = useState<Record<string, string>>({})
    const [error, setError] = useState<string | null>(null)
    const [loading, setLoading] = useState(false)

    useEffect(() => {
        let cancelled = false
        setLoading(true)
        setError(null)
        setFiles(null)
        setDiffs({})

        ;(async () => {
            try {
                const list = await getCommitDetail(repoId, commit.id)
                if (cancelled) return
                setFiles(list)
                // Fetch each file's diff in parallel (stage-2 scope: a raw
                // unified diff per file, no syntax highlighting).
                const entries = await Promise.all(
                    list.map(async (f) => {
                        const text = await getCommitDiff(repoId, commit.id, f.path)
                        return [f.path, text] as const
                    }),
                )
                if (cancelled) return
                setDiffs(Object.fromEntries(entries))
            } catch (err) {
                if (!cancelled) setError(String(err))
            } finally {
                if (!cancelled) setLoading(false)
            }
        })()

        return () => {
            cancelled = true
        }
    }, [repoId, commit.id])

    return (
        <div className="commit-detail">
            <div className="commit-detail-breadcrumb">
                <code>{commit.id.slice(0, 7)}</code>
                <span> · select an artifact to return</span>
            </div>

            <header className="commit-detail-header">
                <h1 className="commit-detail-subject">{commit.subject}</h1>
                <div className="commit-detail-meta">
                    <span>{commit.author}</span>
                    <span>·</span>
                    <span>{formatTimestamp(commit.date)}</span>
                    <span>·</span>
                    <code title={commit.id}>{commit.id.slice(0, 10)}</code>
                </div>
                {commit.parents.length > 0 && (
                    <div className="commit-detail-parents">
                        {commit.parents.length === 1 ? "Parent" : "Parents"}:{" "}
                        {commit.parents.map((p) => (
                            <code key={p}>{p.slice(0, 7)}</code>
                        ))}
                    </div>
                )}
            </header>

            {error && <code className="detail-pane-error">{error}</code>}
            {loading && !files && (
                <div className="detail-pane-status">Loading commit…</div>
            )}

            {files && files.length === 0 && (
                <p className="commit-detail-empty">
                    This commit changed no files.
                </p>
            )}

            {files && files.length > 0 && (
                <>
                    <ul className="commit-detail-filelist">
                        {files.map((f) => (
                            <li key={f.path} className="commit-detail-fileitem">
                                <span
                                    className={`commit-file-status commit-file-status--${f.status.charAt(0)}`}
                                >
                                    {f.status}
                                </span>
                                <span className="commit-file-path">{f.path}</span>
                                <span className="commit-file-stat">
                                    {f.additions != null && (
                                        <span className="stat-add">
                                            +{f.additions}
                                        </span>
                                    )}
                                    {f.deletions != null && (
                                        <span className="stat-del">
                                            −{f.deletions}
                                        </span>
                                    )}
                                </span>
                            </li>
                        ))}
                    </ul>

                    <div className="commit-detail-diffs">
                        {files.map((f) => (
                            <section key={f.path} className="commit-diff">
                                <h2 className="commit-diff-path">{f.path}</h2>
                                <DiffBlock text={diffs[f.path] ?? ""} />
                            </section>
                        ))}
                    </div>
                </>
            )}
        </div>
    )
}

/// Renders a raw unified diff with per-line +/- coloring. Deliberately
/// minimal — a richer, navigable diff viewer is the documented follow-up.
function DiffBlock({ text }: { text: string }) {
    if (!text.trim()) {
        return <pre className="diff-block diff-block--empty">No textual diff.</pre>
    }
    const lines = text.split("\n")
    return (
        <pre className="diff-block">
            {lines.map((line, i) => (
                <div key={i} className={`diff-line diff-line--${diffLineKind(line)}`}>
                    {line || " "}
                </div>
            ))}
        </pre>
    )
}

function diffLineKind(line: string): string {
    if (line.startsWith("@@")) return "hunk"
    if (line.startsWith("+++") || line.startsWith("---")) return "meta"
    if (line.startsWith("diff ") || line.startsWith("index ")) return "meta"
    if (line.startsWith("+")) return "add"
    if (line.startsWith("-")) return "del"
    return "ctx"
}

function formatTimestamp(iso: string): string {
    const date = new Date(iso)
    if (Number.isNaN(date.getTime())) return iso
    return date.toLocaleString()
}
