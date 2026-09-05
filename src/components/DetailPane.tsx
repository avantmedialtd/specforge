import type { ReactNode, RefObject } from "react"
import {
    branchChipForWorktree,
    changeDirectoryName,
    identChipClass,
    isArchivedChangeId,
    type BranchChip,
} from "../changeIdentity"
import { useRelativeTime } from "../hooks/useRelativeTime"
import { RELATIVE_TIME_WIDEST } from "../relativeTime"
import type { ArtifactRenderTarget, WorkspaceView } from "../types"
import { CopyableIdentity } from "./CopyableIdentity"
import {
    DocumentView,
    IdentityTrailing,
    MissingDocumentLabel,
    type DocumentStatus,
} from "./DocumentView"
import { EmptyState } from "./EmptyState"

// The pane is now a thin skin over `DocumentView`: it decides what the header
// says and hands over one document. The fetch, the freshness policy, the
// document watch and the anchor scrolling live in that component, shared with
// the file browser's preview and with reader windows, so all three surfaces
// keep a reader undisturbed in exactly the same way.
export type { ScrollAnchor } from "./DocumentView"
import type { ScrollAnchor } from "./DocumentView"

interface DetailPaneProps {
    target: ArtifactRenderTarget | null
    scrollAnchor: ScrollAnchor
    /// Workspace views, used only to resolve the rendered artifact's branch
    /// and its owning workspace's palette colour for the identity header.
    /// Deliberately not folded into `target` — see `branchChipForWorktree`.
    ///
    /// Optional because the Archive reader renders through this same pane and
    /// has no views to give: an archived change never shows a branch, so it has
    /// no use for them. The suppression is enforced by `isArchivedChangeId`
    /// below rather than by the caller passing nothing, so an Archive reader
    /// that later gained views still would not sprout a branch chip.
    views?: WorkspaceView[]
    /// Open the artifact this pane is showing in its own reader window. When
    /// omitted the pane offers no such control — the Archive reader renders
    /// through this same pane and has no address to detach.
    onOpenReader?: () => void
}

export function DetailPane({
    target,
    scrollAnchor,
    views = [],
    onOpenReader,
}: DetailPaneProps) {
    return (
        <DocumentView
            source={target ? { kind: "artifact", target } : null}
            scrollAnchor={scrollAnchor}
            onOpenReader={onOpenReader}
            errorTitle="Couldn't load artifact"
            empty={
                <EmptyState
                    title="Nothing selected"
                    body="Pick a Proposal, Design, Tasks, or capability spec from the tree."
                />
            }
            header={(status, headerRef, readerControl) =>
                target && (
                    <ChangeIdentityHeader
                        headerRef={headerRef}
                        changeId={target.changeId}
                        readerControl={readerControl}
                        // An archived change is suppressed here, once, rather
                        // than twice downstream: with no chip there is nothing
                        // to tint, so an archived change cannot be painted in
                        // the colour of the live workspace whose worktree its
                        // artifact happened to be read from (`spec-browser`:
                        // *Change Identity Header in the Detail Pane*, "an
                        // archived change shows no branch chip").
                        chip={
                            isArchivedChangeId(target.changeId)
                                ? { branch: null, color: null }
                                : branchChipForWorktree(target.workspace, views)
                        }
                        status={status}
                    />
                )
            }
        />
    )
}

interface ChangeIdentityHeaderProps {
    headerRef: RefObject<HTMLDivElement | null>
    /// The render target's change id — carries the `archive/` prefix for an
    /// archived change, which `changeDirectoryName` strips.
    changeId: string
    /// What the branch chip should say and what colour to say it in. A null
    /// `branch` (flat workspace, detached HEAD, untracked path, archived
    /// change) renders no chip at all; a null `color` renders it neutral.
    chip: BranchChip
    /// The document's status: when its file was last written, and whether it
    /// still resolves at the address this pane is showing.
    status: DocumentStatus
    /// The reader control to place, or null when this surface offers none.
    readerControl: ReactNode
}

/// How long ago the artifact was last written, advancing on its own.
///
/// The words and the tick both come from the shared relative-time hook, so this
/// label and the sidebar row naming the same change — visible at the same time
/// — cannot spell the same kind of value two different ways, and the text it
/// renders cannot disagree with the text in its own tooltip.
///
/// The tick state lives HERE, in a leaf, not in the pane: an advancing label
/// re-renders only itself, so it never reaches `MarkdownView` whether that is
/// memoized or not. The memo earns its place on the other path — a watcher read
/// that changes only the modification time produces a new document state
/// object, and without the boundary that would re-run the whole markdown
/// pipeline to move these few characters.
function LastChangedLabel({ modifiedAt }: { modifiedAt: number }) {
    const text = useRelativeTime(modifiedAt)
    return (
        // A plain span, matching the branch chip's treatment: informational, not
        // interactive, and therefore not a tab stop — the change name remains
        // the pane's single one. The `title` carries the fuller phrasing, since
        // "9 min ago" standing alone does not say what changed.
        //
        // The reserved width is inline rather than in the stylesheet because it
        // is a property of the formatter, not of the design: it is exactly as
        // wide as the widest label that formatter can emit, so rewording a label
        // moves the box with it and the change name never starts shifting on a
        // tick (`spec-browser`: *…* — "The advancing label never moves the
        // change name").
        <span
            className="identity-changed"
            style={{ minWidth: `${RELATIVE_TIME_WIDEST.length}ch` }}
            title={`Last changed ${text}`}
        >
            {text}
        </span>
    )
}

/// Names the change whose artifact the pane is rendering (`spec-browser`:
/// *Change Identity Header in the Detail Pane*).
///
/// The name is the change's DIRECTORY name, not its proposal title: the title
/// is what the tree already shows, while the directory name is the change's
/// filesystem identity and the token a user hands to external tooling. It is
/// rendered in full — the pane is wide enough, and a truncated identifier is
/// worse than useless when the point is to copy it.
///
/// The branch chip is a SIBLING of the name, never a child. The name carries
/// `user-select: all`, so a nested chip would be swept into the same atomic
/// selection and copied along with the name
/// (`archive/2026-08-16-add-change-identity-headers/design.md`, Decision 2).
///
/// The chip is tinted to the owning workspace's palette colour, built by the
/// same `identChipClass` the tree's chip uses — so where the tree ALSO renders
/// a chip, the two render identically. That is the sole-change-row case; a
/// change living in several worktrees renders its instances as plain labels
/// instead (`labelForInstance` in `WorkspaceTree`), so there is no chip there
/// to match and no equivalence to hold.
function ChangeIdentityHeader({
    headerRef,
    changeId,
    chip,
    status,
    readerControl,
}: ChangeIdentityHeaderProps) {
    // Two elements, not one: the outer bar carries the sticky positioning and
    // an opaque background spanning the full pane width, so scrolled content
    // cannot show through it; the inner element carries the prose column's
    // width bound and horizontal origin, so the identity sits directly above
    // the document's first line instead of floating left of it on a wide
    // window (`archive/2026-08-16-add-change-identity-headers/design.md`,
    // Decision 5). A single element cannot do both — `max-width`
    // would clip the background to the column.
    return (
        <div className="detail-identity" ref={headerRef}>
            <div className="detail-identity-inner">
                <CopyableIdentity
                    value={changeDirectoryName(changeId)}
                    noun="change name"
                />
                {chip.branch && (
                    <span className={identChipClass(chip.color, "identity-branch")}>
                        {chip.branch}
                    </span>
                )}
                {/* A SIBLING of the name, never a child — `.identity-name`
                    carries `user-select: all`, so a nested element would be
                    swept into the atomic selection and copied along with the
                    change name (`spec-browser`: *…* — "The copied value
                    excludes the last-changed label"). */}
                {status.missing && <MissingDocumentLabel />}
                {/* The values that DESCRIBE the artifact, grouped so that ONE
                    auto margin carries the whole cluster to the trailing edge.
                    This pane is the only surface that renders two of them, and
                    while they each carried their own auto margin the free space
                    was split between them and the label came to rest mid-row
                    (see `IdentityTrailing`). */}
                <IdentityTrailing>
                    {/* Deliberately NOT suppressed for an archived change,
                        unlike the chip above. A branch is suppressed because an
                        archived change genuinely has none; its file's
                        modification time exists and means exactly what it means
                        for any other artifact (`spec-browser`: *…* — "An
                        archived artifact reports its modification time like any
                        other"). */}
                    {status.modifiedAt !== null && (
                        <LastChangedLabel modifiedAt={status.modifiedAt} />
                    )}
                    {/* Last in the cluster, so it never sits between the change
                        name and the values that describe it. */}
                    {readerControl}
                </IdentityTrailing>
            </div>
        </div>
    )
}
