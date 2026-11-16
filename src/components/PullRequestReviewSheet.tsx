import { useLayoutEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { isTauri } from "../api"
import { useReviewDraftEditorContext } from "../hooks/useReviewDraftEditor"
import type { ReviewJump } from "../pullRequestWorkspace"
import type { PullRequestDetail, PullRequestReference, ReviewProgress } from "../types"
import { PullRequestReviewComposer } from "./PullRequestReviewComposer"
import { EscapedText } from "./DiffView"

/** Lives below the shared editor, so closing the sheet cannot discard edits. */
export function PullRequestReviewSheet({
    reference, detail, progress, onJump,
}: {
    reference: PullRequestReference
    detail: PullRequestDetail | null
    progress: ReviewProgress | null
    onJump: (jump: ReviewJump) => void
}) {
    const editor = useReviewDraftEditorContext()
    const [open, setOpen] = useState(false)
    const trigger = useRef<HTMLButtonElement>(null)
    const dialog = useRef<HTMLDialogElement>(null)
    const jumpRef = useRef(false)
    useLayoutEffect(() => {
        if (!open) { return }
        const element = dialog.current
        const invoking = trigger.current
        element?.showModal()
        return () => {
            element?.close()
            if (!jumpRef.current) { invoking?.focus({ preventScroll: true }) }
            jumpRef.current = false
        }
    }, [open])
    if (!isTauri() || !editor) { return null }
    const close = () => {
        editor.saveForm()
        setOpen(false)
    }
    const items = editor.draft?.items ?? []
    const knownProgress = detail && progress?.headCommit === detail.headCommit &&
        progress.baseCommit === detail.baseCommit ? progress : null
    const failed = editor.notice?.failed === true
    return <>
        <button type="button" className="pull-request-view-action pr-finish-review"
            ref={trigger} aria-haspopup="dialog" onClick={() => setOpen(true)}
            onPointerDown={(event) => {
                event.preventDefault()
                event.currentTarget.focus({ preventScroll: true })
            }}
            title={failed ? editor.notice?.text : undefined}>
            Finish review ({items.length}){editor.saving ? " · Saving…" : failed ? " · Save needs attention" : ""}
        </button>
        {failed && !open && <span className="sr-only" role="status">{editor.notice?.text}</span>}
        {open && createPortal(
            <dialog className="pr-review-sheet" ref={dialog} aria-labelledby="pr-review-sheet-title"
                onCancel={(event) => { event.preventDefault(); close() }}
                onKeyDown={(event) => {
                    if (event.key === "Escape") {
                        event.preventDefault()
                        event.stopPropagation()
                        close()
                    } else if (event.key === "Tab" && !event.ctrlKey && !event.metaKey && !event.altKey) {
                        const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(
                            'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], summary, [tabindex="0"]',
                        )).filter((element) => element.tabIndex >= 0 && element.getClientRects().length > 0)
                        const first = controls[0]
                        const last = controls[controls.length - 1]
                        if (event.shiftKey && document.activeElement === first) {
                            event.preventDefault()
                            last?.focus()
                        } else if (!event.shiftKey && document.activeElement === last) {
                            event.preventDefault()
                            first?.focus()
                        }
                    }
                }}>
                <div className="pr-section-toolbar">
                    <h2 id="pr-review-sheet-title">Finish review</h2>
                    <button type="button" className="pull-request-view-action" onClick={close}>Close</button>
                </div>
                <p className="pull-request-view-note">
                    {knownProgress
                        ? `${knownProgress.viewed} reviewed · ${knownProgress.total - knownProgress.viewed - knownProgress.skipped} remaining · ${knownProgress.skipped} skipped`
                        : "Review progress unavailable."}
                </p>
                <h3 className="pull-request-view-heading">{items.length} pending comments and replies</h3>
                <ol className="pr-thread-index">
                    {items.map((item) => {
                        const path = "path" in item ? item.path :
                            detail?.threads.find((thread) => thread.id === item.target)?.path ?? ""
                        const unplaced = item.kind === "unplacedInline" || item.kind === "unplacedReply"
                        return <li key={item.id}>
                            <button type="button" className="pr-thread-jump" onClick={() => {
                                jumpRef.current = true
                                close()
                                onJump({ kind: "draft", id: item.id, path })
                            }}>
                                <span>{unplaced ? "Needs placement" : item.kind === "placedReply" ? "Reply" : "Comment"}
                                    {path && <> · <EscapedText text={path} /></>}</span>
                                <span className="pr-thread-excerpt"><EscapedText text={item.body.slice(0, 180)} /></span>
                                <span className="pr-thread-jump-label">{unplaced ? "Choose placement →" : "View in Files →"}</span>
                            </button>
                        </li>
                    })}
                </ol>
                <PullRequestReviewComposer reference={reference} />
            </dialog>, document.body,
        )}
    </>
}
