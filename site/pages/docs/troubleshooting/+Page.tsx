import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';

export default function Troubleshooting() {
    return (
        <DocsLayout
            title="Troubleshooting"
            intro="SpecForge releases are unsigned, so every operating system asks a question the first time you run one. Here is the answer on each — plus the other things that most often need explaining: WSL2 latency, specforge-serve exposure, and workspace validation."
            currentPath="/docs/troubleshooting"
        >
            <DocsSection id="macos" heading="macOS — Gatekeeper">
                <p>
                    Gatekeeper warns on first launch. Instead of double-clicking,{' '}
                    <strong>right-click the app and choose Open</strong>, then confirm in the
                    dialog. macOS remembers the decision, so this is a one-time step.
                </p>
                <p>
                    If that dialog does not appear — some managed Macs suppress it — clear the
                    quarantine flag from the installed app instead, which has the same effect:
                </p>
                <pre>
                    <code>xattr -dr com.apple.quarantine /Applications/SpecForge.app</code>
                </pre>
            </DocsSection>

            <DocsSection id="macos-binaries" heading="macOS — the standalone binaries">
                <p>
                    The terminal UI and the headless server ship as command-line binaries. For these
                    clearing the quarantine flag is the only route, not an alternative one: there is
                    no right-click ▸ Open affordance, because that dialog belongs to the Finder and
                    these are not launched from it. Clear the flag before the first run:
                </p>
                <pre>
                    <code>{`xattr -dr com.apple.quarantine specforge-tui
xattr -dr com.apple.quarantine specforge-serve`}</code>
                </pre>
                <Note>
                    Installing the server from npm skips this entirely — a package manager does not
                    set the quarantine attribute on what it unpacks, so there is nothing to clear.
                    See <a href="/docs/web-ui#install">Getting specforge-serve</a>.
                </Note>
            </DocsSection>

            <DocsSection id="windows" heading="Windows — SmartScreen">
                <p>
                    SmartScreen may warn that the publisher is unrecognised. Choose{' '}
                    <strong>More info</strong>, then <strong>Run anyway</strong>.
                </p>
            </DocsSection>

            <DocsSection id="webview2" heading="Windows — WebView2 on older machines">
                <p>
                    The single-file <strong>portable</strong> <code>.exe</code> relies on the system{' '}
                    <strong>WebView2 runtime</strong>. It is preinstalled on current Windows; on an
                    older machine you may need to install it manually.
                </p>
                <Note>
                    The <strong>NSIS installer</strong> handles WebView2 for you. If you would
                    rather not think about it, use the installer rather than the portable build.
                </Note>
            </DocsSection>

            <DocsSection id="linux" heading="Linux — .deb and .AppImage">
                <p>
                    Install the <code>.deb</code> with your package manager. For the{' '}
                    <code>.AppImage</code>, make it executable and run it:
                </p>
                <pre>
                    <code>{`chmod +x SpecForge_*.AppImage
./SpecForge_*.AppImage`}</code>
                </pre>
            </DocsSection>

            <DocsSection id="wsl" heading="Windows — workspaces inside WSL2">
                <p>
                    A workspace living in the WSL2 filesystem is supported, but the{' '}
                    <code>\\wsl.localhost</code> share reports no filesystem events, so updates
                    arrive on a periodic re-scan rather than instantly. If changes seem slow to
                    appear, that is the scan interval — tune it in{' '}
                    <a href="/docs/settings#wsl">Settings ▸ WSL workspaces</a>, and see{' '}
                    <a href="/docs/workspaces#wsl">Workspaces ▸ WSL2</a> for how git is handled.
                </p>
            </DocsSection>

            <DocsSection id="serve" heading="specforge-serve is unauthenticated">
                <p>
                    <code>specforge-serve</code> binds <code>127.0.0.1:4317</code> by default, which
                    is reachable only from that machine. Passing <code>--bind 0.0.0.0</code> (or
                    another interface address) publishes it on the network{' '}
                    <strong>without authentication</strong> — only do that on a network you trust.
                    Run <code>specforge-serve --help</code> for the full flag and
                    environment-variable reference, and see{' '}
                    <a href="/docs/web-ui">Web UI &amp; remote access</a> for the safe remote
                    routes.
                </p>
            </DocsSection>

            <DocsSection id="not-a-workspace" heading="“Not an OpenSpec workspace”">
                <p>
                    The full message reads{' '}
                    <em>&ldquo;not an OpenSpec workspace (no openspec/ subdirectory)&rdquo;</em>,
                    and it means what it says: a folder is only accepted if it contains an{' '}
                    <code>openspec/</code> directory. Point SpecForge at the repository root rather
                    than at <code>openspec/</code> itself, and remember that a repository&rsquo;s
                    worktrees are discovered automatically — you do not add them separately. See{' '}
                    <a href="/docs/workspaces">Workspaces</a>.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
