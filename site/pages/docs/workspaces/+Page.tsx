import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';

export default function Workspaces() {
    return (
        <DocsLayout
            title="Workspaces"
            intro="A workspace is any folder containing an openspec/ directory. Register it once and SpecForge tracks every change inside it — including the ones living in other git worktrees."
            currentPath="/docs/workspaces"
        >
            <DocsSection id="adding" heading="Adding a workspace">
                <p>
                    Open <strong>Settings</strong> from the gear in the sidebar footer, choose{' '}
                    <strong>+ Add workspace</strong>, and pick a folder. The folder must contain an{' '}
                    <code>openspec/</code> directory; anything else is rejected as{' '}
                    <em>&ldquo;not an OpenSpec workspace&rdquo;</em>.
                </p>
                <p>
                    In a browser there is no native folder dialog to open, so the same section shows
                    a path field instead: type or paste the absolute path and choose{' '}
                    <strong>+ Add</strong>. It is validated exactly as the picker&rsquo;s folder is.
                    That is the route you take after <code>npx @avantmedia/specforge</code>, and
                    also inside the desktop app&rsquo;s own Web UI tab — see{' '}
                    <a href="/docs/web-ui">Web UI &amp; remote access</a>.
                </p>
                <p>
                    Workspaces can also be added, removed, renamed and recoloured from the terminal
                    UI&rsquo;s Settings screen — see <a href="/docs/terminal-ui">Terminal UI</a>.
                    All three surfaces share one registry.
                </p>
            </DocsSection>

            <DocsSection id="worktrees" heading="Git-worktree discovery">
                <p>
                    If a registered workspace is a git repository, SpecForge discovers the
                    repository&rsquo;s other worktrees automatically. You register the repository
                    once, not each worktree.
                </p>
                <p>
                    In the tree, a repository groups its worktrees together. A change open in
                    several branches expands to one instance per worktree, and each instance
                    carries:
                </p>
                <ul>
                    <li>the branch name it lives on</li>
                    <li>a task-progress meter</li>
                    <li>a relative &ldquo;modified&rdquo; time</li>
                    <li>
                        <code>[diverged]</code> when that branch&rsquo;s copy of the change differs
                        from the default branch&rsquo;s
                    </li>
                    <li>
                        <code>[stale]</code> when the change is already archived on the default
                        branch but is still active here — the signal that this branch can probably
                        be closed. It is about state, not content: a stale copy can be
                        byte-identical to the archived one
                    </li>
                </ul>
                <Note>
                    The tray badge still counts that change <strong>once</strong>. The tree shows
                    you every copy; the badge answers &ldquo;how many distinct things are in
                    flight&rdquo;, which is a different question.
                </Note>
            </DocsSection>

            <DocsSection id="working-tree" heading="Committed, modified, or untracked">
                <p>
                    Each change instance also carries a working-tree badge answering the question an
                    agent&rsquo;s &ldquo;done&rdquo; does not: has this spec actually landed in git?{' '}
                    <strong>Committed</strong> means the change&rsquo;s files match the branch;{' '}
                    <strong>modified</strong> and <strong>untracked</strong> mean there is work
                    sitting in the working tree that no commit holds yet. Repository rows carry a
                    dirty rollup, so an unclean worktree is visible without expanding anything.
                </p>
                <p>
                    The badges are derived from <code>git status</code> runs that never block the
                    UI, and refresh on git activity, spec edits, and whenever the window regains
                    focus.
                </p>
            </DocsSection>

            <DocsSection id="personalisation" heading="Names and colours">
                <p>
                    Each workspace carries a display name you can rename inline, and a tint colour
                    picked from a curated swatch set. The colour marks the workspace&rsquo;s rows in
                    the tree, so repositories stay tellable apart at a glance; both persist across
                    restarts, along with your expand and collapse state and the window geometry.
                </p>
                <p>
                    Commits are coloured differently — by <em>author</em> — in the{' '}
                    <a href="/docs/dashboard#garden">commit garden</a>.
                </p>
            </DocsSection>

            <DocsSection id="parking" heading="Parking a workspace">
                <p>
                    Every workspace has an enabled toggle in Settings. Disabled, it disappears from
                    the tree and stops counting toward the badge — but it stays registered and
                    watched, and still shows up on the <a href="/docs/dashboard">Dashboard</a>. Use
                    it to park a dormant project without losing it.
                </p>
            </DocsSection>

            <DocsSection id="file-browser" heading="Browsing the rest of the repository">
                <p>
                    Clicking a repository&rsquo;s top-level row opens a read-only file browser over
                    every markdown file in it — not just the OpenSpec artifacts — with a folder
                    tree, a path filter, and the same rendering the spec views use. Files ignored by
                    git stay hidden.
                </p>
                <p>
                    This is the one deliberately pull-based view in the app: rescanning a whole
                    repository on every file event would be wasteful, so the browser carries a{' '}
                    <strong>Refresh</strong> button instead of a watcher.
                </p>
            </DocsSection>

            <DocsSection id="archive" heading="The archive">
                <p>
                    Shipped changes stay readable. The archive browser — from the sidebar footer, or
                    by clicking a ship on the <a href="/docs/dashboard#ships">Dashboard</a> — lists
                    a workspace&rsquo;s archived changes newest first with their dates, searchable,
                    each opening into the same artifact reader as an active change. It loads on
                    demand and refreshes live while open.
                </p>
            </DocsSection>

            <DocsSection id="wsl" heading="WSL2 workspaces (Windows)">
                <p>
                    On Windows, a workspace living inside WSL2 — reached via{' '}
                    <code>\\wsl.localhost\&lt;distro&gt;\…</code> — is detected automatically and
                    handled specially: the share reports no filesystem events, so SpecForge falls
                    back to periodic re-scanning (interval configurable in{' '}
                    <a href="/docs/settings#wsl">Settings</a>, default 10 seconds), and git commands
                    run through <code>wsl.exe</code> inside the distro rather than against the slow
                    9P share. Registration, badges and the tree work the same as for a native
                    workspace.
                </p>
            </DocsSection>

            <DocsSection id="remote" heading="Local and remote state">
                <p>
                    The registry lives in SpecForge&rsquo;s own configuration directory, shared by
                    the desktop app and the terminal UI on the same machine — register a workspace
                    in one and the other sees it, a running desktop app on its next launch.
                </p>
                <p>
                    Over SSH that directory belongs to the <em>remote</em> machine, so a remote
                    terminal UI reflects whatever is registered there, independent of your laptop.
                    There is no network sync: register workspaces on the host where you run it.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
