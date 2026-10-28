import { useEffect, useState } from "react"
import {
    getChatGptQuotaEnabled,
    getClaudeQuotaEnabled,
    getReviewSkipPatterns,
    onReviewSkipPatternsChanged,
    setBitbucketCredentials,
    setBitbucketEnabled,
    setChatGptQuotaEnabled,
    setClaudeQuotaEnabled,
    setGithubEnabled,
    setGithubToken,
    setReviewSkipPatterns,
} from "../../api"
import type { SettingsGroup } from "../../settingsGroups"
import {
    errorsByIndex,
    parsePattern,
    refusalText,
    withAddedPattern,
    withoutPattern,
    withPattern,
} from "../../skipPatterns"
import type {
    BitbucketConfigView,
    CredentialSource,
    GithubConfigView,
    PanelPosition,
    ReviewSkipPatterns,
    StoredCredentialSource,
} from "../../types"
import { prettifyError } from "../errors"
import { CommittedField } from "./CommittedField"
import {
    credentialSourceLabel,
    CredentialForm,
    CredentialInput,
    CredentialSources,
    LegacyCredentialPrompt,
} from "./CredentialForm"
import { IntegrationCard } from "./IntegrationCard"
import { panelPositionLabel } from "./panelPositions"
import { SettingsRow } from "./SettingsRow"
import {
    useBitbucketConfig,
    useGithubConfig,
    type ProviderConfigState,
} from "./useProviderConfig"
import { useSettingSwitch } from "./useSettingSwitch"

/// Where a BitBucket API token is created.
const BITBUCKET_TOKEN_URL = "https://id.atlassian.com/manage-profile/security/api-tokens"

/// Where a GitHub token is created: fine-grained (read-only, one owner) and
/// classic (every owner, but `repo` can write).
const GITHUB_FINE_GRAINED_TOKEN_URL = "https://github.com/settings/personal-access-tokens/new"
const GITHUB_CLASSIC_TOKEN_URL = "https://github.com/settings/tokens/new"

/// A URL rendered the way this host can safely offer it: a new-tab link in the
/// browser skin, selectable text on the desktop (which has no command that
/// opens an arbitrary URL). The host comes from the prop rather than a global,
/// so the same component tree renders either presentation in a test.
function SettingsUrl({ url, desktop }: { url: string; desktop: boolean }) {
    return desktop ? (
        <code className="settings-selectable">{url}</code>
    ) : (
        <a href={url} target="_blank" rel="noopener noreferrer">
            {url}
        </a>
    )
}

interface IntegrationsGroupProps {
    onSelectGroup: (group: SettingsGroup) => void
    /// Whether this host renders the native credential editors. The browser
    /// keeps the switches, descriptions and non-secret settings but none of
    /// the credential state or mutation controls (`settings-view`: *Provider
    /// Credential Editing Is Desktop-Only*).
    desktop: boolean
}

/// Settings → Integrations (`settings-view`: *Opt-In Integrations Collapse
/// While Off*): every opt-in that reads something on your behalf. Off, each is
/// one row; on, it opens to show what it needs. The panel slots are chosen in
/// Layout, beside the switch that shares the rail with them — an enabled card
/// only names its slot and links there. The review skip patterns follow the
/// pull-request cards while either is on, and nothing stands in for them while
/// both are off (`settings-view`: *Settings Are Organised Into Groups*).
export function IntegrationsGroup({ onSelectGroup, desktop }: IntegrationsGroupProps) {
    // Read here rather than in each card, so the row after them follows a
    // switch flipped in either at once.
    const github = useGithubConfig()
    const bitbucket = useBitbucketConfig()
    const anyOn = github.config?.enabled === true || bitbucket.config?.enabled === true
    return (
        <>
            <section className="settings-section">
                <h3 className="settings-subheading">Pull requests</h3>
                <GithubCard
                    onSelectGroup={onSelectGroup}
                    provider={github}
                    desktop={desktop}
                />
                <BitbucketCard
                    onSelectGroup={onSelectGroup}
                    provider={bitbucket}
                    desktop={desktop}
                />
                {anyOn && <SkipPatternsRow />}
            </section>
            <section className="settings-section">
                <h3 className="settings-subheading">Usage</h3>
                <QuotaCard
                    id="settings-claude-quota"
                    name="Claude usage quota"
                    description="A small gauge of your Claude usage — the 5-hour and weekly windows — in the sidebar footer. Reads your local Claude Code login (read-only) to query Anthropic's usage endpoint. Nothing is read or sent until you turn this on."
                    load={getClaudeQuotaEnabled}
                    save={setClaudeQuotaEnabled}
                />
                <QuotaCard
                    id="settings-chatgpt-quota"
                    name="ChatGPT usage quota"
                    description="A small gauge of your ChatGPT usage — the 5-hour and weekly windows — in the sidebar footer. Reads your local Codex CLI login (read-only) to query ChatGPT's usage endpoint. Nothing is read or sent until you turn this on."
                    load={getChatGptQuotaEnabled}
                    save={setChatGptQuotaEnabled}
                />
            </section>
        </>
    )
}

/// An enabled pull-request card's slot line: where its panel is, and the way
/// to the group where that is chosen.
function PanelSlotLine({
    position,
    onSelectGroup,
}: {
    position: PanelPosition
    onSelectGroup: (group: SettingsGroup) => void
}) {
    return (
        <div className="integration-card-slot">
            <span>Panel: {panelPositionLabel(position)}</span>
            <button
                type="button"
                className="settings-link-button"
                onClick={() => onSelectGroup("layout")}
            >
                Change in Layout
            </button>
        </div>
    )
}

/// A switch for a card whose writes go through the provider configuration
/// rather than through `useSettingSwitch`: optimistic, and a failure puts the
/// stored value back and reports on the card.
function useProviderSwitch<C extends { enabled: boolean }>(
    config: C | null,
    setConfig: (update: (c: C | null) => C | null) => void,
    save: (next: boolean) => Promise<void>,
) {
    const [error, setError] = useState<string | null>(null)
    const toggle = async (next: boolean) => {
        if (!config) return
        setError(null)
        setConfig((c) => (c ? { ...c, enabled: next } : c))
        try {
            await save(next)
        } catch (err) {
            setConfig((c) => (c ? { ...c, enabled: !next } : c))
            setError(`Couldn't save this setting — ${prettifyError(err)}`)
        }
    }
    return { toggle, error }
}

/// Removing a credential left behind by an integration that is off — without
/// turning it on, which would start polling with that very credential. `clear`
/// goes through the provider configuration's transaction, so a failure re-reads
/// the stored state before the error is reported (`settings-view`: *Settings
/// Persist by One Rule*).
function useRemoveStoredCredential(clear: () => Promise<unknown>) {
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const run = async () => {
        setBusy(true)
        setError(null)
        try {
            await clear()
        } catch (err) {
            setError(`Couldn't remove the token — ${prettifyError(err)}`)
        } finally {
            setBusy(false)
        }
    }
    return { run, busy, error }
}

/// What a native card says about a stored credential without naming its value.
/// An environment override is called out separately, so clearing the stored
/// entry never reads as clearing the environment.
function storedCredentialLabel(
    credential: string,
    active: CredentialSource,
    stored: StoredCredentialSource,
): string {
    const where = credentialSourceLabel(stored)
    if (active === "environment") {
        return `The environment supplies the active credential; a ${credential} is also stored in ${where}. Removing it leaves the environment credential in use.`
    }
    return `A ${credential} is still stored in ${where}.`
}

/// GitHub pull requests: the opt-in switch, the write-only token and its
/// instructions (`github-pull-requests`: *The GitHub Token Is Stored
/// Write-Only*). The token field is never pre-filled — no command returns the
/// token — and the configuration says only whether one is set. The refresh
/// interval is deliberately not exposed. The credential editor and its source
/// state render only on the desktop; the browser keeps the switch, description
/// and panel slot (`settings-view`: *Provider Credential Editing Is
/// Desktop-Only*).
function GithubCard({
    onSelectGroup,
    provider,
    desktop,
}: {
    onSelectGroup: (group: SettingsGroup) => void
    provider: ProviderConfigState<GithubConfigView>
    desktop: boolean
}) {
    const { config, setConfig, loadFailed, transact } = provider
    const [tokenDraft, setTokenDraft] = useState("")
    const [saving, setSaving] = useState(false)
    const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null)
    const { toggle, error } = useProviderSwitch(config, setConfig, setGithubEnabled)
    const removal = useRemoveStoredCredential(() => transact(() => setGithubToken("")))

    if (!config) {
        return (
            <section className="integration-card">
                <p className="settings-empty">
                    {loadFailed ? "Could not load the GitHub settings." : "Loading…"}
                </p>
            </section>
        )
    }

    const saveToken = async () => {
        setSaving(true)
        setMessage(null)
        try {
            // The write and the re-read form one transaction: a failed save
            // also re-reads, so the card returns to the stored source state
            // instead of keeping the attempted one.
            const next = await transact(() => setGithubToken(tokenDraft.trim()))
            setTokenDraft("")
            setMessage({
                text: next.storedSource !== "none"
                    ? "Saved. The next refresh uses this token."
                    : "Saved. No token is stored.",
                failed: false,
            })
        } catch (err) {
            setMessage({ text: `Could not save: ${prettifyError(err)}`, failed: true })
        } finally {
            setSaving(false)
        }
    }

    return (
        <IntegrationCard
            id="settings-github"
            name="GitHub pull requests"
            description={
                <>
                    Your open GitHub pull requests, and those awaiting your review, in a panel beside
                    your changes — with their checks, conflicts and unresolved conversations. Opening a
                    pull request reads its files, conversation and checks. Nothing is read or sent
                    until you turn this on, and the token is only ever sent to{" "}
                    <code>api.github.com</code>. Review actions — comments, replies, approvals and
                    requests for changes — are sent only from the desktop app, after you confirm them.
                </>
            }
            enabled={config.enabled}
            onToggle={(next) => void toggle(next)}
            error={error}
            storedCredential={
                desktop && config.storedSource !== "none"
                    ? {
                          label: storedCredentialLabel(
                              "GitHub token",
                              config.activeSource,
                              config.storedSource,
                          ),
                          onRemove: () => void removal.run(),
                          busy: removal.busy,
                          error: removal.error,
                      }
                    : null
            }
        >
            {!desktop ? (
                <>
                    {!config.credentialAvailable && (
                        <p className="settings-help">
                            Credentials are managed in the desktop app. Save a GitHub token
                            there to power this panel; this browser tab cannot set or clear
                            one.
                        </p>
                    )}
                    <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
                </>
            ) : (
                <>
                    <CredentialSources
                        active={config.activeSource}
                        stored={config.storedSource}
                    />
                    {config.activeSource === "legacy" && (
                        <LegacyCredentialPrompt credential="GitHub token" />
                    )}
                    <CredentialForm
                        note="Saving with the field empty removes the stored token."
                        saveLabel="Save token"
                        saving={saving}
                        onSave={() => void saveToken()}
                        message={message}
                    >
                        <CredentialInput
                            id="settings-github-token"
                            label="Token"
                            secret
                            value={tokenDraft}
                            onChange={setTokenDraft}
                            placeholder={
                                config.storedSource !== "none"
                                    ? "Token set — enter a new one to replace it"
                                    : "Paste a GitHub token"
                            }
                            onEnter={() => void saveToken()}
                        />
                    </CredentialForm>
                    {/* Open while no token is stored — the instructions are what
                        the reader needs next — and folded away once one is. */}
                    <details
                        className="settings-disclosure"
                        open={!config.credentialAvailable}
                    >
                        <summary>How to create a token</summary>
                        <p className="settings-help">
                            A <strong>fine-grained</strong> token sees only the one account
                            or organisation it was created for. Grant it read access to{" "}
                            <strong>Pull requests</strong>, <strong>Checks</strong> and{" "}
                            <strong>Commit statuses</strong> to power the panel and viewer,
                            and read/write access to <strong>Pull requests</strong> to
                            submit reviews:{" "}
                            <SettingsUrl
                                url={GITHUB_FINE_GRAINED_TOKEN_URL}
                                desktop={desktop}
                            />
                            . A <strong>classic</strong> token sees every organisation you
                            belong to and needs the <code>repo</code> scope for private
                            repositories; that scope permits the bounded review writes
                            SpecForge performs after you confirm them (add{" "}
                            <code>read:org</code> for team review requests):{" "}
                            <SettingsUrl url={GITHUB_CLASSIC_TOKEN_URL} desktop={desktop} />
                            . A read-only token can still power the panel and viewer, but
                            not review actions. Either kind must be authorised for each
                            organisation that enforces single sign-on.
                        </p>
                        <p className="settings-help">
                            Already signed in with the GitHub CLI?{" "}
                            <code className="settings-selectable">gh auth token</code>{" "}
                            prints the token it holds, ready to paste here. A{" "}
                            <code>GH_TOKEN</code> or <code>GITHUB_TOKEN</code> environment
                            variable, when set, takes precedence over the stored token.
                        </p>
                    </details>
                    <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
                </>
            )}
        </IntegrationCard>
    )
}

/// BitBucket pull requests: the opt-in switch, the write-only credential pair
/// and its instructions (`bitbucket-pull-requests`: *Credentials Are Stored
/// Write-Only*). The token field is never pre-filled and shows only whether
/// one is set. The refresh interval is deliberately not exposed. The credential
/// editor and its source state render only on the desktop; the browser keeps
/// the switch, description and panel slot.
function BitbucketCard({
    onSelectGroup,
    provider,
    desktop,
}: {
    onSelectGroup: (group: SettingsGroup) => void
    provider: ProviderConfigState<BitbucketConfigView>
    desktop: boolean
}) {
    const { config, setConfig, loadFailed, transact } = provider
    const [usernameDraft, setUsernameDraft] = useState("")
    const [tokenDraft, setTokenDraft] = useState("")
    const [saving, setSaving] = useState(false)
    const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null)
    const { toggle, error } = useProviderSwitch(config, setConfig, setBitbucketEnabled)
    // Clearing the token keeps the username — the specified "Clearing the
    // token" behaviour.
    const removal = useRemoveStoredCredential(() =>
        transact(() => setBitbucketCredentials("", "")),
    )

    if (!config) {
        return (
            <section className="integration-card">
                <p className="settings-empty">
                    {loadFailed ? "Could not load the BitBucket settings." : "Loading…"}
                </p>
            </section>
        )
    }

    const saveCredentials = async () => {
        setSaving(true)
        setMessage(null)
        try {
            // As with GitHub: a failed write re-reads the stored pair before
            // the failure is reported.
            const next = await transact(() =>
                setBitbucketCredentials(usernameDraft.trim(), tokenDraft),
            )
            setUsernameDraft("")
            setTokenDraft("")
            setMessage({
                text: next.storedSource !== "none"
                    ? "Saved. The next refresh uses these credentials."
                    : "Saved. No token is stored.",
                failed: false,
            })
        } catch (err) {
            setMessage({ text: `Could not save: ${prettifyError(err)}`, failed: true })
        } finally {
            setSaving(false)
        }
    }

    return (
        <IntegrationCard
            id="settings-bitbucket"
            name="BitBucket pull requests"
            description={
                <>
                    The open pull requests you authored, across every BitBucket workspace you belong
                    to, in a panel beside your changes. Opening a pull request reads its files,
                    conversation and checks. Nothing is read or sent until you turn this on, and the
                    token is only ever sent to <code>api.bitbucket.org</code>. Review actions —
                    comments, replies, approvals and change requests — are sent only from the desktop
                    app, after you confirm them.
                </>
            }
            enabled={config.enabled}
            onToggle={(next) => void toggle(next)}
            error={error}
            storedCredential={
                desktop && config.storedSource !== "none"
                    ? {
                          label: storedCredentialLabel(
                              "BitBucket credential pair",
                              config.activeSource,
                              config.storedSource,
                          ),
                          onRemove: () => void removal.run(),
                          busy: removal.busy,
                          error: removal.error,
                      }
                    : null
            }
        >
            {!desktop ? (
                <>
                    {!config.credentialAvailable && (
                        <p className="settings-help">
                            Credentials are managed in the desktop app. Save a BitBucket
                            username and API token there to power this panel; this browser
                            tab cannot set or clear them.
                        </p>
                    )}
                    <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
                </>
            ) : (
                <>
                    <CredentialSources
                        active={config.activeSource}
                        stored={config.storedSource}
                    />
                    {config.activeSource === "legacy" && (
                        <LegacyCredentialPrompt credential="BitBucket credential pair" />
                    )}
                    <CredentialForm
                        note={
                            <>
                                Saving replaces both. Saving with the token field empty
                                removes the stored token.
                            </>
                        }
                        saveLabel="Save credentials"
                        saving={saving}
                        onSave={() => void saveCredentials()}
                        message={message}
                    >
                        <CredentialInput
                            id="settings-bitbucket-username"
                            label="Username or email"
                            value={usernameDraft}
                            onChange={setUsernameDraft}
                            placeholder="you@example.com"
                        />
                        <CredentialInput
                            id="settings-bitbucket-token"
                            label="API token"
                            secret
                            value={tokenDraft}
                            onChange={setTokenDraft}
                            placeholder={
                                config.storedSource !== "none"
                                    ? "Token set — enter a new one to replace it"
                                    : "Paste a BitBucket API token"
                            }
                            onEnter={() => void saveCredentials()}
                        />
                    </CredentialForm>
                    <details
                        className="settings-disclosure"
                        open={!config.credentialAvailable}
                    >
                        <summary>How to create a token</summary>
                        <p className="settings-help">
                            BitBucket Cloud needs its own API token — a Jira or Confluence
                            API token is not accepted. Create one with read access to{" "}
                            <strong>Account</strong>, <strong>Workspace membership</strong>,{" "}
                            <strong>Pull requests</strong> and <strong>Repositories</strong>{" "}
                            (choose BitBucket when asked which app) at{" "}
                            <SettingsUrl url={BITBUCKET_TOKEN_URL} desktop={desktop} />,
                            and pair it with your Atlassian account email or BitBucket
                            username. Repository read is what lets an opened pull request
                            show its changed files.
                        </p>
                        <p className="settings-help">
                            Add the <strong>Pull requests</strong> write scope for
                            comments, replies, approvals and change requests. A read-only
                            token still powers the panel and viewer, but not review
                            actions. Current authored-only discovery lists only your own
                            pull requests and offers <strong>Comment</strong> only.
                        </p>
                    </details>
                    <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
                </>
            )}
        </IntegrationCard>
    )
}

const SKIP_PATTERNS_TITLE_ID = "settings-skip-patterns-title"

/// What the row says the patterns do (`pull-request-viewer`: *Review Skip
/// Patterns*, In Settings).
const SKIP_PATTERNS_DESCRIPTION = (
    <p>
        Files whose path matches a pattern are collapsed in a pull request and left out of its
        viewed count, until you choose to review them. A pattern without <code>/</code> matches
        the file's name wherever it is, and one with <code>/</code> the whole path from the
        repository root; <code>/…/</code> is a regular expression, searched in the path.
    </p>
)

/// The review skip patterns, "Skip in review" (`pull-request-viewer`: *Review
/// Skip Patterns*): one field per pattern, each with a control that removes
/// it, then an empty field that adds one. The list starts empty, and while it
/// is, the add field alone is shown, its placeholder suggesting a pattern.
/// Every field persists by the one rule (`settings-view`: *Settings Persist by
/// One Rule*), and each commit and removal stores the whole list. A refused
/// list stores nothing and is reported on the field that sent it; a pattern a
/// hand-edited settings file holds that a list would not be accepted with is
/// reported on its own field. A change made in another window arrives as
/// `review-skip-patterns-changed`, and shows here without the view being
/// reopened.
function SkipPatternsRow() {
    const [skip, setSkip] = useState<ReviewSkipPatterns | null>(null)
    const [loadFailed, setLoadFailed] = useState(false)
    // Why the last Remove did not take, on the place of the pattern the
    // reader removed.
    const [failure, setFailure] = useState<{ at: number; text: string } | null>(null)
    const [busy, setBusy] = useState(false)
    // A new add field after each pattern added, so it starts empty again.
    const [added, setAdded] = useState(0)

    useEffect(() => {
        let cancelled = false
        getReviewSkipPatterns()
            .then((read) => {
                if (!cancelled) setSkip(read)
            })
            .catch(() => {
                if (!cancelled) setLoadFailed(true)
            })
        let unlisten: (() => void) | undefined
        void onReviewSkipPatternsChanged((payload) => {
            // An unparseable SSE frame arrives as `undefined`. A list just
            // stored was accepted, so none of its patterns is in error.
            if (!cancelled && payload) setSkip({ ...payload, errors: [] })
        }).then((u) => {
            if (cancelled) u()
            else unlisten = u
        })
        return () => {
            cancelled = true
            unlisten?.()
        }
    }, [])

    if (!skip) {
        return (
            <SettingsRow
                title="Skip in review"
                description={SKIP_PATTERNS_DESCRIPTION}
                layout="stacked"
                control={
                    <p className="settings-empty">
                        {loadFailed ? "Could not load the skip patterns." : "Loading…"}
                    </p>
                }
            />
        )
    }

    const { patterns } = skip
    const storedErrors = errorsByIndex(skip.errors)

    // Stores `next`, the empty list included, and shows the list now stored;
    // a refusal comes back as what the field at `index` says, or with a null
    // `index` as what a removal says, which sends no pattern of its own.
    const store = async (next: string[], index: number | null): Promise<string | null> => {
        const outcome = await setReviewSkipPatterns(next)
        if (outcome.kind === "refused") return refusalText(outcome.refused, index)
        setSkip({ patterns: outcome.patterns, errors: [] })
        return null
    }

    // Remove persists when activated, and reports on its own pattern's place.
    const activate = async (next: string[], at: number, failed: string) => {
        setBusy(true)
        setFailure(null)
        try {
            const refusal = await store(next, null)
            if (refusal !== null) setFailure({ at, text: `${failed} — ${refusal}` })
        } catch (err) {
            setFailure({ at, text: `${failed} — ${prettifyError(err)}` })
        } finally {
            setBusy(false)
        }
    }

    const reported = (text: string) => (
        <p className="settings-error settings-row-error" role="alert">
            {text}
        </p>
    )

    return (
        <SettingsRow
            title="Skip in review"
            titleId={SKIP_PATTERNS_TITLE_ID}
            description={SKIP_PATTERNS_DESCRIPTION}
            layout="stacked"
            control={
                <div className="skip-patterns" role="group" aria-labelledby={SKIP_PATTERNS_TITLE_ID}>
                    {patterns.length > 0 && (
                        <ul className="skip-patterns-list">
                            {patterns.map((pattern, index) => (
                                <li key={index} className="skip-patterns-item">
                                    <div className="skip-patterns-field">
                                        <CommittedField
                                            id={`settings-skip-pattern-${index}`}
                                            stored={pattern}
                                            format={(value) => value}
                                            parse={parsePattern}
                                            ariaLabel={`Skip pattern ${index + 1}`}
                                            onCommit={async (value) => {
                                                setFailure(null)
                                                const refusal = await store(
                                                    withPattern(patterns, index, value),
                                                    value === "" ? null : index,
                                                )
                                                if (refusal !== null) throw refusal
                                            }}
                                        />
                                        <button
                                            type="button"
                                            className="btn-remove"
                                            aria-label={`Remove ${pattern}`}
                                            disabled={busy}
                                            onClick={() =>
                                                void activate(
                                                    withoutPattern(patterns, index),
                                                    index,
                                                    "Couldn't remove it",
                                                )
                                            }
                                        >
                                            Remove
                                        </button>
                                    </div>
                                    {storedErrors.has(index) &&
                                        reported(`Left out of matching: ${storedErrors.get(index)}`)}
                                    {failure && failure.at === index && reported(failure.text)}
                                </li>
                            ))}
                        </ul>
                    )}
                    <CommittedField
                        key={added}
                        id="settings-skip-pattern-add"
                        stored=""
                        format={(value) => value}
                        parse={parsePattern}
                        placeholder="Add a pattern, such as **/tests/**"
                        ariaLabel="Add a skip pattern"
                        onCommit={async (value) => {
                            setFailure(null)
                            const refusal = await store(
                                withAddedPattern(patterns, value),
                                patterns.length,
                            )
                            if (refusal !== null) throw refusal
                            setAdded((n) => n + 1)
                        }}
                    />
                </div>
            }
        />
    )
}

/// A usage-quota opt-in: no credential in Settings — it reads the local CLI
/// login — so its card is the header row whether on or off.
function QuotaCard({
    id,
    name,
    description,
    load,
    save,
}: {
    id: string
    name: string
    description: string
    load: () => Promise<boolean>
    save: (next: boolean) => Promise<void>
}) {
    const { value, flip, error } = useSettingSwitch(load, save, false)
    return (
        <IntegrationCard
            id={id}
            name={name}
            description={description}
            enabled={value}
            onToggle={(next) => void flip(next)}
            error={error}
        />
    )
}
