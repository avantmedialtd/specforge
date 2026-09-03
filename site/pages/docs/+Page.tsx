import { DocsLayout, DocsSection, Note } from '../../src/components/DocsLayout';
import { LATEST_RELEASE_URL, NPM_PACKAGE, OPENSPEC_URL } from '../../src/site-config';

export default function DocsIndex() {
    return (
        <DocsLayout
            title="Getting started"
            intro="SpecForge lives in your menu bar, system tray, or status area and shows every change in flight across the workspaces you register. Setup is four steps."
            currentPath="/docs"
        >
            <DocsSection id="install" heading="1. Install">
                <p>
                    Grab the build for your platform from the{' '}
                    <a href={LATEST_RELEASE_URL}>latest release</a>. Releases are unsigned, so the
                    first launch needs one extra click — see{' '}
                    <a href="/docs/troubleshooting">Troubleshooting</a> for the exact path on each
                    operating system.
                </p>
                <p>
                    Working in a browser instead, or setting up a machine with no display? The web
                    UI runs straight from npm — <code>npx {NPM_PACKAGE}</code> — with no download
                    and no unsigned-binary prompt to clear. See{' '}
                    <a href="/docs/web-ui#install">Getting specforge-serve</a>.
                </p>
            </DocsSection>

            <DocsSection id="launch" heading="2. Launch it">
                <p>
                    SpecForge appears in your menu bar (macOS), system tray (Windows), or status
                    area (Linux). There is no Dock-only window to hunt for. Click the tray icon to
                    open the main window.
                </p>
                <Note>
                    Closing the window only hides it — the app keeps running in the tray. Quit for
                    real from the <strong>Quit SpecForge</strong> tray item, or with ⌘-Q.
                </Note>
            </DocsSection>

            <DocsSection id="add-workspace" heading="3. Add a workspace">
                <p>
                    Open <strong>Settings</strong> — the gear in the sidebar footer — and choose{' '}
                    <strong>+ Add workspace</strong>. Pick any folder containing an{' '}
                    <code>openspec/</code> directory. A folder without one is rejected as{' '}
                    <em>&ldquo;not an OpenSpec workspace&rdquo;</em>.
                </p>
                <p>
                    If the folder is a git repository, SpecForge discovers the repository&rsquo;s
                    other worktrees automatically — you register the repository once, not each
                    worktree. See <a href="/docs/workspaces">Workspaces</a>.
                </p>
            </DocsSection>

            <DocsSection id="read-the-badge" heading="4. Read the badge">
                <p>
                    The badge counts open changes across every enabled workspace. A change being
                    worked on in several git worktrees counts once, not once per worktree. At zero,
                    the badge is hidden entirely, so a number always means something is open.
                </p>
                <p>
                    Where that count appears depends on the platform. On macOS it is drawn as a
                    digit beside the menu-bar icon, and mirrored on the Dock tile. On Windows and
                    Linux the tray icon switches between its idle and active marks, and the count
                    itself lives in the icon&rsquo;s tooltip — hover to read it.
                </p>
            </DocsSection>

            <DocsSection id="dashboard" heading="First look: the Dashboard">
                <p>
                    Clicking the tray icon opens onto the <a href="/docs/dashboard">Dashboard</a> —
                    what shipped today, what is in flight, and a live graph of today&rsquo;s commits
                    — with the change browser one click away in the tree on the left.
                </p>
            </DocsSection>

            <DocsSection id="format" heading="The format it reads">
                <p>
                    SpecForge reads <a href={OPENSPEC_URL}>OpenSpec</a> workspaces. It renders what
                    is on disk and never writes to your projects. The only things it ever writes are
                    its own configuration and its activity log — settings, the workspace registry,
                    the achievement record — all stored in app data, outside any workspace. See{' '}
                    <a href="/docs/settings">Settings</a>.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
