import { useState, type CSSProperties } from "react"
import { setBitbucketPanelPosition, setGithubPanelPosition } from "../../api"
import { DOC_WIDTHS, DOC_WIDTH_LABELS, DOC_WIDTH_ORDER } from "../../docWidth"
import type { DocumentWidth, PanelPosition } from "../../types"
import { prettifyError } from "../errors"
import { PANEL_POSITIONS } from "./panelPositions"
import { SettingsRow } from "./SettingsRow"
import { Switch } from "./Switch"
import { useBitbucketConfig, useGithubConfig, type ProviderConfigState } from "./useProviderConfig"

interface LayoutGroupProps {
    /// The reading width and the Commit history switch are owned by `App`, so
    /// the reconciliation and the cross-window listeners exist whether or not
    /// Settings is open. Both setters reject when the write fails, having put
    /// the stored value back.
    documentWidth: DocumentWidth
    onDocumentWidthChange: (width: DocumentWidth) => Promise<void>
    commitHistoryEnabled: boolean
    onCommitHistoryEnabledChange: (enabled: boolean) => Promise<void>
}

/// Settings → Layout (`settings-view`: *The Layout Group Gathers the Side
/// Panes' Occupants*): how documents read, and what sits beside them. The
/// Commit history switch and the slots of the pull-request panels that are on
/// sit together, because together they decide whether the rail exists — which
/// the reader can see rather than be told. An integration that is off has no
/// panel, so it has no row here either.
export function LayoutGroup({
    documentWidth,
    onDocumentWidthChange,
    commitHistoryEnabled,
    onCommitHistoryEnabledChange,
}: LayoutGroupProps) {
    const [widthError, setWidthError] = useState<string | null>(null)
    const [historyError, setHistoryError] = useState<string | null>(null)

    const chooseWidth = (rung: DocumentWidth) => {
        if (rung === documentWidth) return
        setWidthError(null)
        onDocumentWidthChange(rung).catch((err) =>
            setWidthError(`Couldn't save the reading width — ${prettifyError(err)}`),
        )
    }

    const toggleHistory = (next: boolean) => {
        setHistoryError(null)
        onCommitHistoryEnabledChange(next).catch((err) =>
            setHistoryError(`Couldn't save this setting — ${prettifyError(err)}`),
        )
    }

    return (
        <>
            <section className="settings-section">
                <h3 className="settings-subheading">Reading</h3>
                <ReadingWidthRow width={documentWidth} onChange={chooseWidth} error={widthError} />
            </section>

            <section className="settings-section">
                <h3 className="settings-subheading">Side panes</h3>
                <p className="settings-help">
                    What sits beside your documents. The rail on the right appears while commit
                    history is on, or while a pull-request panel is placed in it.
                </p>
                <SettingsRow
                    title="Commit history"
                    controlId="settings-commit-history"
                    description="Show the selected repository's commit graph in the rail. Turning it off removes the graph and stops the git reads it makes, in the desktop app and the browser alike."
                    error={historyError}
                    control={
                        <Switch
                            id="settings-commit-history"
                            checked={commitHistoryEnabled}
                            onChange={toggleHistory}
                        />
                    }
                />
                <BitbucketPanelRow />
                <GithubPanelRow />
            </section>
        </>
    )
}

/// The reading-width picker.
///
/// Presented on both hosts: a reading width is expressed entirely in the
/// served stylesheet and behaves identically in a browser tab (`web-ui`:
/// *Desktop-Only Settings Are Hidden in the Web UI* does not apply to it).
///
/// The sample is not decoration. Settings is a routed view that REPLACES the
/// document rather than overlaying it, so without a sample the only way to
/// judge a rung is to choose it, close Settings, and look. It renders one
/// paragraph and one code well — the two tiers — inside a container that
/// carries the rung's tokens locally, so the preview never touches <body> and
/// hovering a rung cannot change what the reading surfaces are showing.
function ReadingWidthRow({
    width,
    onChange,
    error,
}: {
    width: DocumentWidth
    onChange: (width: DocumentWidth) => void
    error: string | null
}) {
    return (
        <SettingsRow
            layout="stacked"
            title="Reading width"
            titleId="settings-reading-width-title"
            description={
                <>
                    How wide documents render. Each step moves both the text measure and the width
                    available to tables, code blocks and diagrams. <strong>Full</strong> lets those
                    take the whole pane — useful for wide diagrams — while body text stays bounded.
                </>
            }
            error={error}
            control={
                <>
                    <div
                        className="settings-choice-row"
                        role="radiogroup"
                        aria-labelledby="settings-reading-width-title"
                    >
                        {DOC_WIDTH_ORDER.map((rung) => (
                            <button
                                key={rung}
                                type="button"
                                role="radio"
                                aria-checked={rung === width}
                                className="settings-choice"
                                onClick={() => onChange(rung)}
                            >
                                {DOC_WIDTH_LABELS[rung]}
                            </button>
                        ))}
                    </div>
                    <div
                        className="doc-width-sample"
                        style={
                            {
                                "--doc-column": DOC_WIDTHS[width].column,
                                "--doc-measure": DOC_WIDTHS[width].measure,
                            } as CSSProperties
                        }
                    >
                        <p className="doc-width-sample-prose">
                            Requirements are written so the reader can tell what the system must do
                            without reading the code that does it. This paragraph wraps at the
                            measure the selected width sets.
                        </p>
                        <pre className="doc-width-sample-object">
                            <code>{"openspec validate --strict <change>"}</code>
                        </pre>
                    </div>
                </>
            }
        />
    )
}

function BitbucketPanelRow() {
    const state = useBitbucketConfig()
    return (
        <PanelPositionRow
            id="settings-bitbucket-panel"
            name="BitBucket pull requests"
            state={state}
            persist={setBitbucketPanelPosition}
        />
    )
}

function GithubPanelRow() {
    const state = useGithubConfig()
    return (
        <PanelPositionRow
            id="settings-github-panel"
            name="GitHub pull requests"
            state={state}
            persist={setGithubPanelPosition}
        />
    )
}

interface PanelPositionRowProps<C> {
    id: string
    name: string
    state: ProviderConfigState<C>
    persist: (position: PanelPosition) => Promise<void>
}

/// One provider's panel slot, presented only while that provider is on. Off,
/// there is no panel to place, so nothing stands in for the row — no note, no
/// link, and no loading or error placeholder while the configuration is read
/// (the Integrations card reports a configuration it cannot load). The way to
/// the slot is the enabled card's "Change in Layout" link. A slot chosen while
/// on stays persisted while off, so the panel lands there again when turned
/// back on. A move made elsewhere arrives through the provider hook's
/// `pull-request-panel-moved` listener.
function PanelPositionRow<C extends { enabled: boolean; panelPosition: PanelPosition }>({
    id,
    name,
    state,
    persist,
}: PanelPositionRowProps<C>) {
    const { config, setConfig } = state
    const [error, setError] = useState<string | null>(null)

    if (!config || !config.enabled) return null

    const choose = async (position: PanelPosition) => {
        const previous = config.panelPosition
        if (position === previous) return
        setError(null)
        setConfig((c) => (c ? { ...c, panelPosition: position } : c))
        try {
            // The backend announces the move; every window (this one
            // included) re-seats the panel from that event.
            await persist(position)
        } catch (err) {
            setConfig((c) => (c ? { ...c, panelPosition: previous } : c))
            setError(`Couldn't move the panel — ${prettifyError(err)}`)
        }
    }

    const titleId = `${id}-title`
    return (
        <SettingsRow
            layout="stacked"
            title={`${name} panel`}
            titleId={titleId}
            description="Which slot the panel occupies."
            error={error}
            control={
                <div className="settings-choice-row" role="radiogroup" aria-labelledby={titleId}>
                    {PANEL_POSITIONS.map(({ value, label }) => (
                        <button
                            key={value}
                            type="button"
                            role="radio"
                            aria-checked={value === config.panelPosition}
                            className="settings-choice"
                            onClick={() => void choose(value)}
                        >
                            {label}
                        </button>
                    ))}
                </div>
            }
        />
    )
}
