import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';
import { LATEST_RELEASE_URL } from '../../../src/site-config';

export default function TerminalUi() {
    return (
        <DocsLayout
            title="Terminal UI"
            intro="The change browser, dashboard and commit garden in a single binary you can run over SSH, drop into a tmux status line, or keep open beside your editor."
            currentPath="/docs/terminal-ui"
        >
            <DocsSection id="install" heading="Install">
                <p>
                    Every <a href={LATEST_RELEASE_URL}>release</a> ships the terminal UI as a
                    standalone archive — macOS universal, Linux x64, and Windows x64. Extract it and
                    run <code>./specforge-tui</code>.
                </p>
                <Note>
                    <p className="m-0 mb-2">
                        <strong>macOS: clear the quarantine flag first.</strong> A terminal binary
                        has no Gatekeeper &ldquo;right-click ▸ Open&rdquo; dialog, so the desktop
                        workaround does not apply here. Run:
                    </p>
                    <pre className="m-0">
                        <code>xattr -dr com.apple.quarantine specforge-tui</code>
                    </pre>
                </Note>
            </DocsSection>

            <DocsSection id="modes" heading="Three ways to run it">
                <ul>
                    <li>
                        <strong>Interactive</strong> — the full TUI. Browse, dashboard, garden,
                        history, settings.
                    </li>
                    <li>
                        <code>--status</code> — prints every workspace and its active changes, then
                        exits. For piping, scripts, or a quick glance.
                    </li>
                    <li>
                        <code>--line</code> — prints{' '}
                        <code>SpecForge · N workspaces · M open changes</code>, then exits. Built
                        for a prompt segment or a tmux status bar.
                    </li>
                </ul>
                <pre>
                    <code>{`# tmux status-right
set -g status-right '#(specforge-tui --line)'

# zsh prompt
precmd() { specforge-tui --line }`}</code>
                </pre>
            </DocsSection>

            <DocsSection id="keys" heading="Keys">
                <ul>
                    <li>
                        <code>1</code>–<code>5</code> — Browse, Dashboard, Garden, History, Settings
                    </li>
                    <li>
                        <code>j</code> / <code>k</code> (or <code>↓</code> / <code>↑</code>) — move
                        and scroll
                    </li>
                    <li>
                        <code>Tab</code> — switch between the tree and the detail pane
                    </li>
                    <li>
                        <code>Enter</code> / <code>l</code> — open the selected change;{' '}
                        <code>h</code> goes back to the tree
                    </li>
                    <li>
                        <code>[</code> / <code>]</code> — previous and next artifact tab
                    </li>
                    <li>
                        <code>/</code> — filter the tree by title or name
                    </li>
                    <li>
                        <code>?</code> — help overlay; <code>q</code> or <code>Ctrl-c</code> quits
                    </li>
                </ul>
            </DocsSection>

            <DocsSection id="screens" heading="The screens">
                <p>
                    <strong>Browse</strong> is the workspace and change tree with status glyphs and
                    a task-progress bar, beside a markdown detail pane with an artifact tab bar.
                    Below about 90 columns it collapses to a single focused pane.{' '}
                    <strong>Dashboard</strong> is the progress overview — today&rsquo;s counts,
                    streak, heatmap and ships — rendered for a TTY. <strong>Garden</strong> draws
                    today&rsquo;s commits per repository, coloured per author.{' '}
                    <strong>History</strong> draws the commit-graph rail with box-drawing
                    characters. <strong>Settings</strong> toggles the app settings and manages the
                    workspace registry — <code>a</code> adds, <code>x</code> removes, <code>r</code>{' '}
                    renames, <code>c</code> recolours.
                </p>
                <p>
                    Changes made on the Settings screen persist immediately and appear at once in
                    the running TUI; a running desktop app picks them up on its next launch.
                </p>
            </DocsSection>

            <DocsSection id="capabilities" heading="Terminal capabilities">
                <p>
                    Colour and glyphs are detected once and degrade cleanly. <code>truecolor</code>{' '}
                    is used when <code>COLORTERM</code> advertises it, then 256-colour, then the 16
                    ANSI names; <code>NO_COLOR</code> or <code>TERM=dumb</code> drops colour
                    entirely and the layout still reads through bold and box-drawing. Unicode
                    markers are used only when the locale advertises UTF-8, and fall back to ASCII
                    otherwise.
                </p>
            </DocsSection>

            <DocsSection id="read-only" heading="What it writes">
                <p>
                    The terminal UI is read-only with respect to your workspaces — it never writes
                    into one. It does write SpecForge&rsquo;s <em>own</em> configuration when you
                    flip a toggle or add, remove, rename or recolour a workspace: the shared
                    settings file, the workspace registry, and the presentation store, all of which
                    live outside any workspace.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
