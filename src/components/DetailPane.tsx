import type { ReactNode, RefObject } from "react"
import { useRef, useState } from "react"
import {
    branchChipForWorktree,
    changeDirectoryName,
    identChipClass,
    isArchivedChangeId,
    type BranchChip,
} from "../changeIdentity"
import type { ArtifactTab, SwitcherOption } from "../changeNavigation"
import { useRelativeTime } from "../hooks/useRelativeTime"
import { RELATIVE_TIME_WIDEST } from "../relativeTime"
import type { ArtifactRenderTarget, Section, WorkspaceView } from "../types"
import { CopyableIdentity } from "./CopyableIdentity"
import { DivergenceChip, TaskProgress } from "./changeMarks"
import {
    DocumentView,
    IdentityTrailing,
    MissingDocumentLabel,
    type DocumentStatus,
} from "./DocumentView"
import { EmptyState } from "./EmptyState"
import { CompletionMark } from "./icons"

// The pane is now a thin skin over `DocumentView`: it decides what the header
// says and hands over one document. The fetch, the freshness policy, the
// document watch, the outline and the anchor scrolling live in that component,
// shared with the file browser's preview and with reader windows, so all three
// surfaces keep a reader undisturbed in exactly the same way.
export type { ScrollAnchor } from "./DocumentView"
import type { ScrollAnchor } from "./DocumentView"

/// The change header's navigation rows — the instance switcher and the
/// artifact tab strip — plus the leading row the read-only form adds.
///
/// One shape for both forms (design D2). `App` fills it from `ChangeData` for
/// a live change; `ArchiveView` fills it from the selected copy's on-demand
/// artifact status and the row's copies. A surface that offers no navigation
/// (a reader window) passes nothing and gets the identity row alone.
export interface ChangeHeaderNavigation {
    /// A row ABOVE the identity row. The read-only form puts the control that
    /// returns to the archive listing here, with the selected copy's archive
    /// date and the change's title (`archive-browser`: *Read-Only Artifact
    /// Navigation*).
    leading?: ReactNode
    /// The instance switcher (live) or copy switcher (read-only).
    switcher?: {
        /// Names the control for assistive technology and captions the row.
        label: string
        options: SwitcherOption[]
        activeKey: string | null
        onSelect: (key: string) => void
        /// Render the row even with a single option, as a plain label rather
        /// than a control. The archive's copy selection does this — it names
        /// the worktree a copy was read from even when there is only one
        /// (`archive-browser`: *Copy Selection Within an Opened Archived
        /// Change*) — while a live change with one instance renders no
        /// switcher row and reserves no space for it.
        showSingle?: boolean
    }
    /// The artifact tab strip.
    tabs?: {
        items: ArtifactTab[]
        activeKey: string | null
        onSelect: (tab: ArtifactTab) => void
    }
    /// The change's task counts, feeding the Tasks tab's trailing slot.
    tasks?: { total: number; completed: number }
}

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
    /// The change header's switcher and tab rows. Omitted leaves the identity
    /// row standing alone.
    navigation?: ChangeHeaderNavigation
    /// Per-section task counts for the outline, passed only for a live
    /// change's tasks artifact (`document-outline`: *Section Progress in a
    /// Tasks Outline*).
    sections?: Section[]
}

export function DetailPane({
    target,
    scrollAnchor,
    views = [],
    onOpenReader,
    navigation,
    sections,
}: DetailPaneProps) {
    return (
        <DocumentView
            source={target ? { kind: "artifact", target } : null}
            scrollAnchor={scrollAnchor}
            onOpenReader={onOpenReader}
            sections={sections}
            errorTitle="Couldn't load artifact"
            empty={
                <EmptyState
                    title="Nothing selected"
                    body="Pick a change from the tree, then an artifact from its header."
                />
            }
            header={(status, headerRef, readerControl) =>
                target && (
                    <ChangeHeader
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
                        navigation={navigation}
                    />
                )
            }
        />
    )
}

interface ChangeHeaderProps {
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
    navigation?: ChangeHeaderNavigation
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

/// The change header: it names the change whose artifact the pane is rendering
/// and carries that change's navigation (`spec-browser`: *Change Identity
/// Header in the Detail Pane*, *Rows*).
///
/// Rows in a fixed order — an optional leading row (the read-only form's back
/// control, date and title), the identity row, the instance switcher when the
/// change has more than one rendered instance, and the artifact tab strip.
/// Every row lies inside the single sticky, MEASURED element, so the macOS
/// titlebar clearance and the anchor clearance bind the whole header and a
/// scroll anchor clears all of it (*Anchoring*).
///
/// ONE component, placed by two callers (design D2). `App` places it in live
/// mode; `ArchiveView` places it in read-only mode and renders no chrome of its
/// own. The two headers had already drifted once; a shared component is the
/// only equivalence that holds without a spec sentence policing it.
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
/// a chip, the two render identically. That is the singleton change row's case;
/// a change living in several worktrees carries an instance-count chip on its
/// row instead and names its branches here, in the switcher.
export function ChangeHeader({
    headerRef,
    changeId,
    chip,
    status,
    readerControl,
    navigation,
}: ChangeHeaderProps) {
    const switcher = navigation?.switcher
    const showSwitcher =
        switcher !== undefined &&
        (switcher.options.length > 1 ||
            (switcher.showSingle === true && switcher.options.length === 1))
    const tabs = navigation?.tabs
    return (
        // The outer bar carries the sticky positioning and an opaque background
        // spanning the full pane width, so scrolled content cannot show through
        // it; each inner row carries the prose column's width bound and
        // horizontal origin, so the header sits directly above the document's
        // first line instead of floating left of it on a wide window
        // (`archive/2026-08-16-add-change-identity-headers/design.md`,
        // Decision 5). A single element cannot do both — `max-width` would clip
        // the background to the column.
        <div className="detail-identity" ref={headerRef}>
            {navigation?.leading && (
                <div className="detail-identity-inner detail-identity-leading">
                    {navigation.leading}
                </div>
            )}
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
            {showSwitcher && switcher && (
                <div
                    className="detail-identity-inner identity-switcher"
                    role="group"
                    aria-label={switcher.label}
                >
                    <span className="identity-switcher-label">{switcher.label}</span>
                    {switcher.options.length === 1 ? (
                        <span className="identity-switcher-single">
                            {switcher.options[0]!.label}
                        </span>
                    ) : (
                        switcher.options.map((option) => (
                            <SwitcherControl
                                key={option.key}
                                option={option}
                                selected={option.key === switcher.activeKey}
                                onSelect={switcher.onSelect}
                            />
                        ))
                    )}
                </div>
            )}
            {tabs && tabs.items.length > 0 && (
                <ArtifactTabStrip
                    items={tabs.items}
                    activeKey={tabs.activeKey}
                    onSelect={tabs.onSelect}
                    tasks={navigation?.tasks}
                />
            )}
        </div>
    )
}

/// One instance (or archived copy) in the switcher row.
///
/// A row of buttons rather than a `<select>` (design D4): a native option
/// cannot carry a branch chip or a divergence label, and those two are exactly
/// what distinguishes one worktree from another. The branch chip is built by
/// the same `identChipClass` the identity row's chip is, so the tints agree.
function SwitcherControl({
    option,
    selected,
    onSelect,
}: {
    option: SwitcherOption
    selected: boolean
    onSelect: (key: string) => void
}) {
    return (
        <button
            type="button"
            className={`identity-switcher-option${selected ? " identity-switcher-option--selected" : ""}`}
            aria-pressed={selected}
            onClick={() => onSelect(option.key)}
        >
            <span className="identity-switcher-name">{option.label}</span>
            {option.branch && (
                <span className={identChipClass(option.color, "identity-switcher-branch")}>
                    {option.branch}
                </span>
            )}
            {option.divergence && <DivergenceChip label={option.divergence} />}
        </button>
    )
}

/// The artifact tab strip — a WAI-ARIA tab list with MANUAL activation
/// (`spec-browser`: *Artifact Tab Strip in the Change Header*, design D3).
///
/// Arrow keys, Home and End move focus among the tabs without changing what is
/// shown; Enter, Space and a click activate. Activation is a NAVIGATION, so
/// automatic activation would push one history entry per tab the focus passed
/// over and make Back useless.
///
/// A `<button>` fires its click handler on Enter and on Space natively, which
/// is precisely the manual-activation contract — there is no key handling for
/// activation here, only for focus movement.
function ArtifactTabStrip({
    items,
    activeKey,
    onSelect,
    tasks,
}: {
    items: ArtifactTab[]
    activeKey: string | null
    onSelect: (tab: ArtifactTab) => void
    tasks?: { total: number; completed: number }
}) {
    // Roving tabindex: the strip is ONE position in the pane's Tab order. The
    // active tab carries it until the user arrows away, after which the
    // focused tab does — reset by the active tab changing, since `focused` is
    // cleared whenever focus leaves the strip.
    const [focused, setFocused] = useState<number | null>(null)
    const activeIndex = Math.max(
        0,
        items.findIndex((item) => item.key === activeKey),
    )
    const current = Math.min(focused ?? activeIndex, items.length - 1)
    const refs = useRef<(HTMLButtonElement | null)[]>([])

    const move = (next: number) => {
        const clamped = Math.max(0, Math.min(items.length - 1, next))
        setFocused(clamped)
        refs.current[clamped]?.focus()
    }

    return (
        <div
            className="detail-identity-inner identity-tabs"
            role="tablist"
            aria-label="Artifacts"
            onKeyDown={(e) => {
                switch (e.key) {
                    case "ArrowRight":
                        e.preventDefault()
                        move(current + 1)
                        break
                    case "ArrowLeft":
                        e.preventDefault()
                        move(current - 1)
                        break
                    case "Home":
                        e.preventDefault()
                        move(0)
                        break
                    case "End":
                        e.preventDefault()
                        move(items.length - 1)
                        break
                }
            }}
        >
            {items.map((item, index) => {
                const isActive = item.key === activeKey
                return (
                    <button
                        key={item.key}
                        type="button"
                        role="tab"
                        aria-selected={isActive}
                        tabIndex={index === current ? 0 : -1}
                        ref={(el) => {
                            refs.current[index] = el
                        }}
                        className={`identity-tab${isActive ? " identity-tab--active" : ""}`}
                        onFocus={() => setFocused(index)}
                        onClick={() => onSelect(item)}
                    >
                        <span className="identity-tab-label">{item.label}</span>
                        {item.kind === "tasks" && tasks && (
                            <TasksTabProgress
                                total={tasks.total}
                                completed={tasks.completed}
                            />
                        )}
                    </button>
                )
            })}
        </div>
    )
}

/// The Tasks tab's trailing slot: the progress meter while at least one task is
/// incomplete, the completion ✓ when every task is complete, and neither when
/// the change parses no tasks — the same rule the change row applies, fed by
/// the same counts, so the two never disagree on one screen.
function TasksTabProgress({ total, completed }: { total: number; completed: number }) {
    if (total <= 0) return null
    if (completed >= total) return <CompletionMark />
    return <TaskProgress completed={completed} total={total} />
}
