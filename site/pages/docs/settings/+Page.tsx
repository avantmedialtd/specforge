import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';

export default function Settings() {
    return (
        <DocsLayout
            title="Settings"
            intro="Settings lives behind the gear in the sidebar footer. The sections below follow that view top to bottom. The terminal UI has a Settings screen of its own, but a much smaller one — the two quota gauges, an Appearance colour scheme, and the workspace list."
            currentPath="/docs/settings"
        >
            <DocsSection id="workspaces" heading="Workspaces">
                <p>
                    Add, remove, rename and recolour workspaces — see{' '}
                    <a href="/docs/workspaces">Workspaces</a> for what registration means. Each
                    workspace also carries an enabled toggle: a disabled workspace disappears from
                    the tree and stops counting toward the badge, but stays registered and watched,
                    and still appears on the <a href="/docs/dashboard">Dashboard</a>. Park a dormant
                    project without unregistering it.
                </p>
            </DocsSection>

            <DocsSection id="identity" heading="Identity and people">
                <p>
                    The Dashboard&rsquo;s personal numbers — streak, heatmap, today&rsquo;s counts —
                    attribute work to <em>you</em>, resolved automatically from your git
                    configuration, with a manual entry when there is nothing to resolve. If your
                    commits span several emails or names, add them as <strong>aliases</strong> so
                    they fold into one developer instead of splitting your history.
                </p>
                <p>
                    The <strong>people roster</strong> does the same for everyone else: name a
                    person, attach their git identities, and the leaderboard and commit garden show
                    one consistently coloured, properly named row per human — or per agent
                    committing under its own identity.
                </p>
            </DocsSection>

            <DocsSection id="notifications" heading="Desktop notifications">
                <p>
                    Notifications fire when a change first <strong>appears</strong> and when it is{' '}
                    <strong>archived</strong> — never on ordinary file edits. Editing a proposal all
                    afternoon produces nothing; finishing one produces exactly one notification.
                </p>
                <p>
                    On macOS the Dock badge mirrors the tray count, so the number is visible in the
                    Dock and the ⌘-Tab switcher as well as the menu bar.
                </p>
            </DocsSection>

            <DocsSection id="quota" heading="Usage-quota gauges">
                <p>
                    SpecForge can show a small gauge for your AI coding-assistant usage — in the
                    desktop sidebar footer, and in the terminal UI&rsquo;s title bar. There are two,
                    independent of one another:
                </p>
                <ul>
                    <li>
                        <strong>Claude</strong> — your 5-hour and weekly usage, with the 5-hour
                        window split into five one-hour segments and the weekly into seven day-long
                        segments. When Anthropic reports separate weekly limits for specific models,
                        the gauge grows one extra weekly bar per model, labelled with the
                        model&rsquo;s name.
                    </li>
                    <li>
                        <strong>ChatGPT</strong> — the same gauge for your ChatGPT plan. Its segment
                        count and length come from whatever window length the usage endpoint
                        reports, falling back to 5 hours and 7 days only when the response omits it.
                    </li>
                </ul>
                <p>
                    Both colour green → orange at 70% → red at 90%, and show a reset countdown when
                    a window is spent. Each bar carries a live &ldquo;now&rdquo; marker at the
                    elapsed point, so you read <em>pace</em> at a glance — budget spent, the fill,
                    against time elapsed, the marker — rather than a per-hour history.
                </p>

                <Note>
                    <p className="m-0 mb-2">
                        <strong>Both are off by default, and independent.</strong>
                    </p>
                    <p className="m-0">
                        Enabling the Claude gauge reads your local Claude Code login to query
                        Anthropic&rsquo;s usage endpoint. Enabling the ChatGPT gauge reads your
                        local Codex CLI login to query ChatGPT&rsquo;s. Both logins are{' '}
                        <strong>read, never modified</strong>, and these are the application&rsquo;s{' '}
                        <strong>only network calls</strong>. With both toggles off, SpecForge makes
                        none at all — nothing is read, and nothing is sent.
                    </p>
                </Note>

                <p>
                    On a narrow terminal the title bar drops whole trailing gauge groups, ChatGPT
                    before Claude, so enabling ChatGPT can never hide an otherwise-visible Claude
                    gauge.
                </p>
            </DocsSection>

            <DocsSection id="startup" heading="Startup">
                <p>
                    <strong>Launch at login</strong> registers SpecForge with the operating
                    system&rsquo;s own login-items mechanism — it is stored in the OS, not in
                    SpecForge&rsquo;s settings file, and queried fresh each time the settings view
                    opens, so what you see is what the OS will actually do.
                </p>
            </DocsSection>

            <DocsSection id="web-ui" heading="Web UI">
                <p>
                    Serve the same UI to a browser at <code>http://127.0.0.1:&lt;port&gt;</code>,
                    with optional Tailscale Serve support for reaching it from your other machines.
                    The trust model deserves its own page:{' '}
                    <a href="/docs/web-ui">Web UI &amp; remote access</a>.
                </p>
            </DocsSection>

            <DocsSection id="wsl" heading="WSL workspaces (Windows)">
                <p>
                    On Windows, workspaces living inside WSL2 are watched by periodic re-scan rather
                    than filesystem events, because the <code>\\wsl.localhost</code> share reports
                    none. This section sets the scan interval — default 10 seconds. It appears only
                    on Windows; see <a href="/docs/workspaces#wsl">Workspaces ▸ WSL2</a>.
                </p>
            </DocsSection>

            <DocsSection id="appearance" heading="Appearance (terminal UI only)">
                <p>
                    The terminal UI&rsquo;s Settings screen carries one control the desktop app has
                    no equivalent for: a colour scheme, cycled on the <strong>Appearance</strong>{' '}
                    row. It is the only setting that does not go into the shared configuration —
                    each frontend owns its own presentation, so the choice is written to the
                    terminal UI&rsquo;s own <code>tui.json</code>, beside the other files rather
                    than inside them. See <a href="/docs/terminal-ui">Terminal UI</a>.
                </p>
            </DocsSection>

            <DocsSection id="what-it-writes" heading="What settings write">
                <p>
                    Toggling a setting, or adding, removing, renaming or recolouring a workspace,
                    writes SpecForge&rsquo;s own configuration — the shared settings file, the
                    workspace registry, and the presentation store. All three live outside any
                    workspace, in the same configuration directory — as does the terminal UI&rsquo;s{' '}
                    <code>tui.json</code>. The one setting stored somewhere else entirely is
                    launch-at-login, which lives in the operating system as described above.
                </p>
                <p>
                    Nothing here writes into a project. SpecForge does not edit specs, toggle task
                    checkboxes, or touch git, whatever the settings say.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
