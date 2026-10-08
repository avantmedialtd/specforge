import { useCallback, useEffect, useMemo, useState } from "react"
import { getCommitDetail, getCommitDiff, getCommitFileImage, openImageWindow } from "../api"
import { fileKey } from "../diffFiles"
import { imageWindowAddress, imageWindowTitle, type ImageWindowSource } from "../imageWindow"
import type { CommitRenderTarget, DiffFile } from "../types"
import { DiffView } from "./DiffView"

interface CommitDetailViewProps {
    target: CommitRenderTarget
}

/// What one read of a commit brought, tagged with the commit it was for, so a
/// newly selected commit never shows the previous one's files.
type CommitRead = { repoId: string; sha: string } & (
    | { files: DiffFile[] }
    | { error: string }
)

/// A commit's header over its diff (`commit-graph`: *Commit Detail View*).
/// The diff is `DiffView`'s, the renderer every host shares: this view only
/// reads the model and names the two sides.
export function CommitDetailView({ target }: CommitDetailViewProps) {
    const { repoId, commit } = target
    const [read, setRead] = useState<CommitRead | null>(null)
    const current = read?.repoId === repoId && read.sha === commit.id ? read : null

    useEffect(() => {
        let cancelled = false
        const sha = commit.id

        // One call reads the whole commit, whatever its number of files: the
        // budgets decide which files arrive with their hunks, and a withheld
        // one loads alone through `loadFile` below.
        ;(async () => {
            try {
                const files = await getCommitDetail(repoId, sha)
                if (!cancelled) setRead({ repoId, sha, files })
            } catch (err) {
                if (!cancelled) setRead({ repoId, sha, error: String(err) })
            }
        })()

        return () => {
            cancelled = true
        }
    }, [repoId, commit.id])

    const firstParent = commit.parents[0]
    const sideNames = useMemo(
        () => ({
            old: firstParent === undefined ? "empty tree" : firstParent.slice(0, 7),
            new: commit.id.slice(0, 7),
        }),
        [firstParent, commit.id],
    )

    // By the file's key and, for a renamed file, its old path too, so the
    // file loads alone and as one renamed file, against the same base as the
    // rest of the diff. A commit never reports a copy (its reads detect
    // renames with `-M`, never copies with `-C`), so only a rename passes one.
    const loadFile = useCallback(
        (file: DiffFile) =>
            getCommitDiff(
                repoId,
                commit.id,
                fileKey(file),
                file.status.kind === "renamed" ? (file.oldPath ?? undefined) : undefined,
            ),
        [repoId, commit.id],
    )

    // An image file's two versions, read from the commit as its section nears
    // the view, by the same paths the loader passes (`commit-graph`: *Commit
    // Detail View*). Opening the commit reads none.
    const readImage = useCallback(
        (file: DiffFile) =>
            getCommitFileImage(
                repoId,
                commit.id,
                fileKey(file),
                file.status.kind === "renamed" ? (file.oldPath ?? undefined) : undefined,
            ),
        [repoId, commit.id],
    )

    // Zoom opens the file's versions in a window of their own, which reads
    // them again from the commit by the same paths.
    const zoomImage = useCallback(
        (file: DiffFile) => {
            const source: ImageWindowSource = {
                kind: "commit",
                repoId,
                sha: commit.id,
                path: fileKey(file),
                oldPath: file.status.kind === "renamed" ? file.oldPath : null,
                sides: sideNames,
            }
            openImageWindow(imageWindowAddress(source), imageWindowTitle(source))
        },
        [repoId, commit.id, sideNames],
    )

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
                {commit.trailers.length > 0 && (
                    <dl className="commit-detail-trailers">
                        {commit.trailers.map((t, i) => (
                            <div
                                key={`${t.key}-${i}`}
                                className="commit-detail-trailer"
                            >
                                <dt className="commit-detail-trailer-key">
                                    {t.key}
                                </dt>
                                <dd
                                    className="commit-detail-trailer-value"
                                    title={t.value}
                                >
                                    {t.value}
                                </dd>
                            </div>
                        ))}
                    </dl>
                )}
            </header>

            {/* A merge is diffed against its first parent as two trees, never
                as a combined diff, and says so above its diff. */}
            {commit.parents.length > 1 && firstParent !== undefined && (
                <p className="commit-detail-base">
                    Changes against first parent <code>{firstParent.slice(0, 7)}</code>
                </p>
            )}

            {current && "error" in current && (
                <code className="detail-pane-error">{current.error}</code>
            )}
            {current === null && <div className="detail-pane-status">Loading commit…</div>}

            {current && "files" in current && current.files.length === 0 && (
                <p className="commit-detail-empty">This commit changed no files.</p>
            )}

            {current && "files" in current && current.files.length > 0 && (
                <DiffView
                    key={commit.id}
                    files={current.files}
                    sideNames={sideNames}
                    loadFile={loadFile}
                    readImage={readImage}
                    imageReads="nearView"
                    zoomImage={zoomImage}
                />
            )}
        </div>
    )
}

function formatTimestamp(iso: string): string {
    const date = new Date(iso)
    if (Number.isNaN(date.getTime())) return iso
    return date.toLocaleString()
}
