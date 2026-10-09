import { useEffect, useState } from "react"
import {
    getChatGptQuotaEnabled,
    getClaudeQuotaEnabled,
    getReviewSkipPatterns,
    isWeb,
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
    GithubConfigView,
    PanelPosition,
    ReviewSkipPatterns,
} from "../../types"
import { prettifyError } from "../errors"
import { CommittedField } from "./CommittedField"
import { CredentialForm, CredentialInput } from "./CredentialForm"
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

/// A URL rendered the way this transport can safely offer it: a new-tab link in
/// the browser skin, selectable text on the desktop (which has no command that
/// opens an arbitrary URL).
function SettingsUrl({ url }: { url: string }) {
    return isWeb() ? (
        <a href={url} target="_blank" rel="noopener noreferrer">
            {url}
        </a>
    ) : (
        <code className="settings-selectable">{url}</code>
    )
}

interface IntegrationsGroupProps {
    onSelectGroup: (group: SettingsGroup) => void
}

/// Settings → Integrations (`settings-view`: *Opt-In Integrations Collapse
/// While Off*): every opt-in that reads something on your behalf. Off, each is
/// one row; on, it opens to show what it needs. The panel slots are chosen in
/// Layout, beside the switch that shares the rail with them — an enabled card
/// only names its slot and links there. The review skip patterns follow the
/// pull-request cards while either is on, and nothing stands in for them while
/// both are off (`settings-view`: *Settings Are Organised Into Groups*).
export function IntegrationsGroup({ onSelectGroup }: IntegrationsGroupProps) {
    // Read here rather than in each card, so the row after them follows a
    // switch flipped in either at once.
    const github = useGithubConfig()
    const bitbucket = useBitbucketConfig()
    const anyOn = github.config?.enabled === true || bitbucket.config?.enabled === true
    return (
        <>
            <section className="settings-section">
                <h3 className="settings-subheading">Pull requests</h3>
                <GithubCard onSelectGroup={onSelectGroup} provider={github} />
                <BitbucketCard onSelectGroup={onSelectGroup} provider={bitbucket} />
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
/// turning it on, which would start polling with that very credential.
function useRemoveStoredCredential(remove: () => Promise<void>, reload: () => Promise<unknown>) {
    const [busy, setBusy] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const run = async () => {
        setBusy(true)
        setError(null)
        try {
            await remove()
            await reload()
        } catch (err) {
            setError(`Couldn't remove the token — ${prettifyError(err)}`)
        } finally {
            setBusy(false)
        }
    }
    return { run, busy, error }
}

/// GitHub pull requests: the opt-in switch, the write-only token and its
/// instructions (`github-pull-requests`: *The GitHub Token Is Stored
/// Write-Only*). The token field is never pre-filled — no command returns the
/// token — and the configuration says only whether one is set. The refresh
/// interval is deliberately not exposed.
function GithubCard({
    onSelectGroup,
    provider,
}: {
    onSelectGroup: (group: SettingsGroup) => void
    provider: ProviderConfigState<GithubConfigView>
}) {
    const { config, setConfig, loadFailed, reload } = provider
    const [tokenDraft, setTokenDraft] = useState("")
    const [saving, setSaving] = useState(false)
    const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null)
    const { toggle, error } = useProviderSwitch(config, setConfig, setGithubEnabled)
    const removal = useRemoveStoredCredential(() => setGithubToken(""), reload)

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
            await setGithubToken(tokenDraft.trim())
            // Re-read rather than assume: the configuration is the only place
            // that says whether a token is now stored.
            const next = await reload()
            setTokenDraft("")
            setMessage({
                text: next.tokenSet
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
                    <code>api.github.com</code>. SpecForge only reads: it sends fixed, read-only
                    requests and never changes anything on GitHub.
                </>
            }
            enabled={config.enabled}
            onToggle={(next) => void toggle(next)}
            error={error}
            storedCredential={
                config.tokenSet
                    ? {
                          label: "A GitHub token is still stored.",
                          onRemove: () => void removal.run(),
                          busy: removal.busy,
                          error: removal.error,
                      }
                    : null
            }
        >
            <CredentialForm
                note={
                    <>
                        Saving with the field empty removes the stored token.
                        {isWeb() &&
                            " From this browser tab, saving sends the token to the SpecForge server serving this page."}
                    </>
                }
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
                        config.tokenSet
                            ? "Token set — enter a new one to replace it"
                            : "Paste a GitHub token"
                    }
                    onEnter={() => void saveToken()}
                />
            </CredentialForm>
            {/* Open while no token is stored — the instructions are what the
                reader needs next — and folded away once one is. */}
            <details className="settings-disclosure" open={!config.tokenSet}>
                <summary>How to create a token</summary>
                <p className="settings-help">
                    A <strong>fine-grained</strong> token can be read-only — grant{" "}
                    <strong>Pull requests</strong>, <strong>Checks</strong> and{" "}
                    <strong>Commit statuses</strong> read access — but it sees only the one account
                    or organisation it was created for: <SettingsUrl url={GITHUB_FINE_GRAINED_TOKEN_URL} />.
                    A <strong>classic</strong> token sees every organisation you belong to, but needs
                    the <code>repo</code> scope for private repositories, and that scope also permits
                    writes (add <code>read:org</code> for team review requests):{" "}
                    <SettingsUrl url={GITHUB_CLASSIC_TOKEN_URL} />. Either kind must be authorised
                    for each organisation that enforces single sign-on.
                </p>
                <p className="settings-help">
                    Already signed in with the GitHub CLI?{" "}
                    <code className="settings-selectable">gh auth token</code> prints the token it
                    holds, ready to paste here. A <code>GH_TOKEN</code> or <code>GITHUB_TOKEN</code>{" "}
                    environment variable, when set, takes precedence over the stored token.
                </p>
            </details>
            <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
        </IntegrationCard>
    )
}

/// BitBucket pull requests: the opt-in switch, the write-only credential pair
/// and its instructions (`bitbucket-pull-requests`: *Credentials Are Stored
/// Write-Only*). The token field is never pre-filled and shows only whether
/// one is set. The refresh interval is deliberately not exposed.
function BitbucketCard({
    onSelectGroup,
    provider,
}: {
    onSelectGroup: (group: SettingsGroup) => void
    provider: ProviderConfigState<BitbucketConfigView>
}) {
    const { config, setConfig, loadFailed, reload } = provider
    const [usernameDraft, setUsernameDraft] = useState("")
    const [tokenDraft, setTokenDraft] = useState("")
    const [saving, setSaving] = useState(false)
    const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null)
    const { toggle, error } = useProviderSwitch(config, setConfig, setBitbucketEnabled)
    // Clearing the token keeps the username — the specified "Clearing the
    // token" behaviour.
    const removal = useRemoveStoredCredential(
        () => setBitbucketCredentials(config?.username ?? "", ""),
        reload,
    )

    // The username is not secret, so the field shows the stored one — taken
    // up on load and after every save.
    const storedUsername = config?.username ?? ""
    useEffect(() => {
        setUsernameDraft(storedUsername)
    }, [storedUsername])

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
            await setBitbucketCredentials(usernameDraft.trim(), tokenDraft)
            // Re-read rather than assume: the configuration is the only place
            // that says whether a token is now stored.
            const next = await reload()
            setUsernameDraft(next.username ?? "")
            setTokenDraft("")
            setMessage({
                text: next.tokenSet
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
                    token is only ever sent to <code>api.bitbucket.org</code>.
                </>
            }
            enabled={config.enabled}
            onToggle={(next) => void toggle(next)}
            error={error}
            storedCredential={
                config.tokenSet
                    ? {
                          label: "A BitBucket API token is still stored.",
                          onRemove: () => void removal.run(),
                          busy: removal.busy,
                          error: removal.error,
                      }
                    : null
            }
        >
            <CredentialForm
                note={
                    <>
                        Saving replaces both. Saving with the token field empty removes the stored
                        token.
                        {isWeb() &&
                            " From this browser tab, saving sends the token to the SpecForge server serving this page."}
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
                        config.tokenSet
                            ? "Token set — enter a new one to replace it"
                            : "Paste a BitBucket API token"
                    }
                    onEnter={() => void saveCredentials()}
                />
            </CredentialForm>
            <details className="settings-disclosure" open={!config.tokenSet}>
                <summary>How to create a token</summary>
                <p className="settings-help">
                    BitBucket Cloud needs its own API token — a Jira or Confluence token is not
                    accepted. Create one with read access to <strong>Account</strong>,{" "}
                    <strong>Workspace membership</strong>, <strong>Pull requests</strong> and{" "}
                    <strong>Repositories</strong> (choose BitBucket when asked which app) at{" "}
                    <SettingsUrl url={BITBUCKET_TOKEN_URL} />, and pair it with your Atlassian
                    account email or BitBucket username. Repository read is what lets an opened
                    pull request show its changed files.
                </p>
            </details>
            <PanelSlotLine position={config.panelPosition} onSelectGroup={onSelectGroup} />
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
        onReviewSkipPatternsChanged((payload) => {
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
