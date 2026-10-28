import { afterAll, beforeAll, beforeEach, describe, expect, test } from "bun:test"
import { act } from "react"
import { FakeElement, installFakeDom, type FakeDocument } from "../../testing/fakeDom"
import { installFakeTransport, settle } from "../../testing/fakeTransport"
import type { SettingsGroup } from "../../settingsGroups"
import type {
    BitbucketConfigView,
    GithubConfigView,
    ReviewSkipPatterns,
} from "../../types"
import { IntegrationsGroup } from "./IntegrationsGroup"

// `settings-view`: *Provider Credential Editing Is Desktop-Only*, *Settings
// Persist by One Rule*, *Opt-In Integrations Collapse While Off*; `web-ui`:
// *Desktop-Only Settings Are Hidden in the Web UI*. The native credential
// editor's transactional failures and separate source state, and the browser
// card that keeps its switch and non-secret settings but no credential
// control.

function githubConfig(overrides: Partial<GithubConfigView> = {}): GithubConfigView {
    return {
        enabled: false,
        credentialAvailable: false,
        activeSource: "none",
        storedSource: "none",
        refreshSecs: 60,
        panelPosition: "right-top",
        ...overrides,
    }
}

function bitbucketConfig(overrides: Partial<BitbucketConfigView> = {}): BitbucketConfigView {
    return {
        enabled: false,
        credentialAvailable: false,
        activeSource: "none",
        storedSource: "none",
        refreshSecs: 60,
        panelPosition: "left-top",
        ...overrides,
    }
}

const NO_PATTERNS: ReviewSkipPatterns = { patterns: [], errors: [] }

let dom: { document: FakeDocument; uninstall: () => void }
let createRoot: typeof import("react-dom/client").createRoot
const g = globalThis as unknown as Record<string, unknown>

beforeAll(async () => {
    dom = installFakeDom()
    g.IS_REACT_ACT_ENVIRONMENT = true
    createRoot = (await import("react-dom/client")).createRoot
})

afterAll(() => {
    delete g.IS_REACT_ACT_ENVIRONMENT
    dom.uninstall()
})

beforeEach(() => {
    delete (g.window as Record<string, unknown>).__TAURI_INTERNALS__
    delete (g.window as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__
})

interface Sent {
    command: string
    args: Record<string, unknown>
}

interface Transport {
    sent: Sent[]
    calls: (command: string) => Sent[]
    uninstall: () => void
}

type Answers = Record<string, (args: Record<string, unknown>) => unknown>

/// The native shell: Tauri's `invoke`, answering each command from `answers`
/// and recording what was sent.
function installDesktop(answers: Answers): Transport {
    const host = g.window as Record<string, unknown>
    const sent: Sent[] = []
    let callbackId = 0
    host.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    host.__TAURI_INTERNALS__ = {
        invoke: (command: string, args?: Record<string, unknown>) => {
            sent.push({ command, args: args ?? {} })
            const answer = answers[command]
            if (!answer) return Promise.resolve(undefined)
            try {
                return Promise.resolve(answer(args ?? {}))
            } catch (err) {
                return Promise.reject(err)
            }
        },
        transformCallback: () => {
            callbackId += 1
            return callbackId
        },
    }
    return {
        sent,
        calls: (command) => sent.filter((call) => call.command === command),
        uninstall() {
            delete host.__TAURI_INTERNALS__
            delete host.__TAURI_EVENT_PLUGIN_INTERNALS__
        },
    }
}

/// The browser skin: the fake web transport, with the same command names and
/// argument records.
function installBrowser(answers: Answers): Transport {
    const sent: Sent[] = []
    const transport = installFakeTransport((command, args) => {
        sent.push({ command, args })
        const answer = answers[command]
        return answer ? answer(args) : null
    })
    return {
        sent,
        calls: (command) => sent.filter((call) => call.command === command),
        uninstall: transport.uninstall,
    }
}

/// The commands every mount reads, before a test overrides a write.
function baseAnswers(
    github: () => GithubConfigView,
    bitbucket: () => BitbucketConfigView,
    patterns: () => ReviewSkipPatterns = () => NO_PATTERNS,
): Answers {
    return {
        get_github_config: () => github(),
        get_bitbucket_config: () => bitbucket(),
        get_claude_quota_enabled: () => false,
        get_chatgpt_quota_enabled: () => false,
        get_review_skip_patterns: () => patterns(),
    }
}

function reactProps(element: FakeElement): Record<string, unknown> {
    const holder = element as unknown as Record<string, unknown>
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key === "string" && key.startsWith("__reactProps$")) {
            return holder[key] as Record<string, unknown>
        }
    }
    for (const key of Reflect.ownKeys(element)) {
        if (typeof key !== "string" || !key.startsWith("__reactFiber$")) continue
        const fiber = holder[key] as { memoizedProps?: Record<string, unknown> }
        if (fiber.memoizedProps !== undefined) return fiber.memoizedProps
    }
    throw new Error("the element carries no React props")
}

async function fire(element: FakeElement, prop: string, event: unknown): Promise<void> {
    const handler = reactProps(element)[prop]
    if (typeof handler !== "function") throw new Error(`the element has no ${prop} handler`)
    await act(async () => {
        ;(handler as (event: unknown) => void)(event)
    })
}

function buttonNamed(container: FakeElement, name: string): FakeElement {
    const found = container.findAll("button").filter((button) => button.textContent === name)
    if (found.length !== 1) {
        throw new Error(`expected one ${name} button, found ${found.length}`)
    }
    return found[0]!
}

function inputById(container: FakeElement, id: string): FakeElement | undefined {
    return container.findAll("input").find((input) => input.getAttribute("id") === id)
}

function inputValue(element: FakeElement): string {
    return (element as unknown as { value?: string }).value ?? ""
}

function switchById(container: FakeElement, id: string): FakeElement {
    const found = inputById(container, id)
    if (!found) throw new Error(`expected the ${id} switch`)
    return found
}

async function mount(
    desktop: boolean,
    onSelectGroup: (group: SettingsGroup) => void = () => {},
) {
    const container = dom.document.createElement("div") as unknown as FakeElement
    const root = createRoot(container as unknown as Element)
    await act(async () =>
        root.render(<IntegrationsGroup desktop={desktop} onSelectGroup={onSelectGroup} />),
    )
    await settle()
    return {
        container,
        async unmount() {
            await act(async () => root.unmount())
        },
    }
}

describe("the native credential editor", () => {
    test("a failed token save reports on the control and re-reads the stored source state", async () => {
        let github = githubConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "secure",
            storedSource: "secure",
        })
        const desktop = installDesktop({
            ...baseAnswers(() => github, () => bitbucketConfig()),
            set_github_token: () => {
                throw "the credential store refused it"
            },
        })
        const mounted = await mount(true)
        try {
            const input = inputById(mounted.container, "settings-github-token")
            if (!input) throw new Error("expected the GitHub token field")
            await fire(input, "onChange", { target: { value: "ghp_new" } })
            expect(inputValue(input)).toBe("ghp_new")

            await fire(buttonNamed(mounted.container, "Save token"), "onClick", {})
            await settle()

            expect(desktop.calls("set_github_token")[0]?.args).toEqual({ token: "ghp_new" })
            // One read on mount, one after the failed transaction.
            expect(desktop.calls("get_github_config")).toHaveLength(2)
            expect(mounted.container.textContent).toContain(
                "Could not save: the credential store refused it",
            )
            expect(mounted.container.textContent).toContain(
                "the operating-system credential store",
            )
            expect(mounted.container.textContent).not.toContain("Saved.")
            // The field keeps what was typed so the reader can retry, and no
            // secret is ever pre-filled.
            const retry = inputById(mounted.container, "settings-github-token")
            expect(retry && inputValue(retry)).toBe("ghp_new")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("a failed clear reports beside the action and keeps the prior stored source", async () => {
        const github = githubConfig({
            enabled: false,
            credentialAvailable: true,
            activeSource: "secure",
            storedSource: "secure",
        })
        const desktop = installDesktop({
            ...baseAnswers(() => github, () => bitbucketConfig()),
            set_github_token: () => {
                throw "the credential store refused it"
            },
        })
        const mounted = await mount(true)
        try {
            expect(mounted.container.textContent).toContain(
                "A GitHub token is still stored in the operating-system credential store.",
            )
            await fire(buttonNamed(mounted.container, "Remove"), "onClick", {})
            await settle()

            expect(desktop.calls("set_github_token")[0]?.args).toEqual({ token: "" })
            expect(desktop.calls("get_github_config")).toHaveLength(2)
            expect(mounted.container.textContent).toContain(
                "Couldn't remove the token — the credential store refused it",
            )
            expect(mounted.container.textContent).toContain(
                "A GitHub token is still stored in the operating-system credential store.",
            )
            expect(inputById(mounted.container, "settings-github-token")).toBeUndefined()
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("clearing a stored token while off leaves an environment override in place", async () => {
        let github = githubConfig({
            enabled: false,
            credentialAvailable: true,
            activeSource: "environment",
            storedSource: "secure",
        })
        const desktop = installDesktop({
            ...baseAnswers(() => github, () => bitbucketConfig()),
            set_github_token: () => {
                github = { ...github, storedSource: "none" }
            },
        })
        const mounted = await mount(true)
        try {
            expect(mounted.container.textContent).toContain(
                "The environment supplies the active credential; a GitHub token is also stored in the operating-system credential store. Removing it leaves the environment credential in use.",
            )
            // Off: no field, instructions or source block.
            expect(inputById(mounted.container, "settings-github-token")).toBeUndefined()

            await fire(buttonNamed(mounted.container, "Remove"), "onClick", {})
            await settle()

            expect(desktop.calls("set_github_token")[0]?.args).toEqual({ token: "" })
            expect(desktop.calls("get_github_config")).toHaveLength(2)
            // Clearing never turns the integration on.
            expect(desktop.calls("set_github_enabled")).toHaveLength(0)
            expect(mounted.container.textContent).not.toContain("also stored")
            expect(switchById(mounted.container, "settings-github-switch")).toBeDefined()
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("reports active and stored sources separately, and asks a legacy source to be re-saved", async () => {
        const github = githubConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "legacy",
            storedSource: "legacy",
        })
        const desktop = installDesktop(
            baseAnswers(() => github, () => bitbucketConfig()),
        )
        const mounted = await mount(true)
        try {
            expect(mounted.container.textContent).toContain("Active credential")
            expect(mounted.container.textContent).toContain("legacy plaintext settings")
            expect(mounted.container.textContent).toContain("Stored credential")
            expect(mounted.container.textContent).toContain(
                "This GitHub token is stored as legacy plaintext in the settings file.",
            )
            expect(mounted.container.textContent).toContain(
                "review actions need it re-saved in the operating-system credential store",
            )
            // The legacy value is never filled into the field.
            const input = inputById(mounted.container, "settings-github-token")
            expect(input && inputValue(input)).toBe("")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("states the GitHub review scopes and where a token is created", async () => {
        const github = githubConfig({ enabled: true })
        const desktop = installDesktop(
            baseAnswers(() => github, () => bitbucketConfig()),
        )
        const mounted = await mount(true)
        try {
            const text = mounted.container.textContent
            expect(text).toContain("read/write access to Pull requests to submit reviews")
            expect(text).toContain("that scope permits the bounded review writes")
            expect(text).toContain("A read-only token can still power the panel and viewer")
            expect(text).toContain("organisation that enforces single sign-on")
            expect(text).toContain("gh auth token")
            expect(text).toContain("https://github.com/settings/personal-access-tokens/new")
            expect(text).toContain("https://github.com/settings/tokens/new")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("states the BitBucket read and write scopes and the authored-only limitation", async () => {
        const bitbucket = bitbucketConfig({ enabled: true })
        const desktop = installDesktop(
            baseAnswers(() => githubConfig(), () => bitbucket),
        )
        const mounted = await mount(true)
        try {
            const text = mounted.container.textContent
            expect(text).toContain("Jira or Confluence API token is not accepted")
            expect(text).toContain("Workspace membership")
            expect(text).toContain("Repositories")
            expect(text).toContain("Pull requests write scope")
            expect(text).toContain("A read-only token still powers the panel and viewer")
            expect(text).toContain("offers Comment only")
            expect(text).toContain("https://id.atlassian.com/manage-profile/security/api-tokens")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("asks a legacy BitBucket pair to be re-saved without filling either field", async () => {
        const bitbucket = bitbucketConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "legacy",
            storedSource: "legacy",
        })
        const desktop = installDesktop(
            baseAnswers(() => githubConfig(), () => bitbucket),
        )
        const mounted = await mount(true)
        try {
            expect(mounted.container.textContent).toContain(
                "This BitBucket credential pair is stored as legacy plaintext",
            )
            const username = inputById(mounted.container, "settings-bitbucket-username")
            const token = inputById(mounted.container, "settings-bitbucket-token")
            expect(username && inputValue(username)).toBe("")
            expect(token && inputValue(token)).toBe("")
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })

    test("shows an environment credential and a stored legacy entry as two sources", async () => {
        const github = githubConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "environment",
            storedSource: "legacy",
        })
        const desktop = installDesktop(
            baseAnswers(() => github, () => bitbucketConfig()),
        )
        const mounted = await mount(true)
        try {
            expect(mounted.container.textContent).toContain("environment variables")
            expect(mounted.container.textContent).toContain("legacy plaintext settings")
            // The legacy prompt is about the source in use, so an environment
            // override does not raise it.
            expect(mounted.container.textContent).not.toContain(
                "review actions need it re-saved",
            )
        } finally {
            await mounted.unmount()
            desktop.uninstall()
        }
    })
})

describe("the browser integrations", () => {
    test("keep the switches, panel slot and skip patterns but show no credential control", async () => {
        const github = githubConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "secure",
            storedSource: "secure",
        })
        const bitbucket = bitbucketConfig({
            enabled: true,
            credentialAvailable: true,
            activeSource: "secure",
            storedSource: "secure",
        })
        const browser = installBrowser(
            baseAnswers(
                () => github,
                () => bitbucket,
                () => ({ patterns: ["**/tests/**"], errors: [] }),
            ),
        )
        const selected: SettingsGroup[] = []
        const mounted = await mount(false, (group) => selected.push(group))
        try {
            expect(switchById(mounted.container, "settings-github-switch")).toBeDefined()
            expect(switchById(mounted.container, "settings-bitbucket-switch")).toBeDefined()
            expect(inputById(mounted.container, "settings-github-token")).toBeUndefined()
            expect(inputById(mounted.container, "settings-bitbucket-username")).toBeUndefined()
            expect(inputById(mounted.container, "settings-bitbucket-token")).toBeUndefined()
            expect(
                mounted.container.findAll("button").map((button) => button.textContent),
            ).not.toContain("Save token")
            expect(
                mounted.container.findAll("button").map((button) => button.textContent),
            ).not.toContain("Save credentials")
            // The only Remove here belongs to a skip pattern, a non-secret
            // setting the browser keeps.
            expect(mounted.container.textContent).not.toContain("is still stored")
            expect(mounted.container.textContent).not.toContain(
                "The environment supplies the active credential",
            )
            expect(mounted.container.textContent).not.toContain("Active credential")
            expect(mounted.container.textContent).not.toContain("Stored credential")
            expect(mounted.container.textContent).not.toContain("How to create a token")
            expect(mounted.container.textContent).not.toContain("legacy plaintext")

            // Non-secret settings remain.
            expect(mounted.container.textContent).toContain("Panel:")
            expect(mounted.container.textContent).toContain("Change in Layout")
            expect(mounted.container.textContent).toContain("Skip in review")
            expect(inputById(mounted.container, "settings-skip-pattern-0")).toBeDefined()

            const layoutLinks = mounted.container
                .findAll("button")
                .filter((button) => button.textContent === "Change in Layout")
            expect(layoutLinks).toHaveLength(2)
            await fire(layoutLinks[0]!, "onClick", {})
            expect(selected).toEqual(["layout"])

            await fire(switchById(mounted.container, "settings-github-switch"), "onChange", {
                currentTarget: { checked: false },
            })
            await settle()
            expect(browser.calls("set_github_enabled")[0]?.args).toEqual({ enabled: false })
        } finally {
            await mounted.unmount()
            browser.uninstall()
        }
    })

    test("say credentials are managed in the desktop app when none can power the integration", async () => {
        const github = githubConfig({
            enabled: true,
            credentialAvailable: false,
            activeSource: "none",
            storedSource: "none",
        })
        const bitbucket = bitbucketConfig({
            enabled: true,
            credentialAvailable: false,
            activeSource: "none",
            storedSource: "none",
        })
        const browser = installBrowser(baseAnswers(() => github, () => bitbucket))
        const mounted = await mount(false)
        try {
            const guidance = "Credentials are managed in the desktop app"
            expect(mounted.container.textContent.split(guidance).length - 1).toBe(2)
            expect(inputById(mounted.container, "settings-github-token")).toBeUndefined()
            expect(inputById(mounted.container, "settings-bitbucket-token")).toBeUndefined()
            expect(
                mounted.container.findAll("button").map((button) => button.textContent),
            ).not.toContain("Save token")
            expect(
                mounted.container.findAll("button").map((button) => button.textContent),
            ).not.toContain("Save credentials")
        } finally {
            await mounted.unmount()
            browser.uninstall()
        }
    })
})
