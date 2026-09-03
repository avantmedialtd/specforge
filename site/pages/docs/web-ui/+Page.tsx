import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';
import { LATEST_RELEASE_URL, NPM_PACKAGE, NPM_PACKAGE_URL } from '../../../src/site-config';

export default function WebUi() {
    return (
        <DocsLayout
            title="Web UI & remote access"
            intro="The same frontend the desktop app renders, served over HTTP — as an embedded toggle in the desktop app, or as the standalone specforge-serve binary on a machine with no display. Off by default, loopback by default, and deliberate about every step beyond that."
            currentPath="/docs/web-ui"
        >
            <DocsSection id="embedded" heading="The embedded server">
                <p>
                    <strong>Settings ▸ Web UI</strong> in the desktop app serves SpecForge at{' '}
                    <code>http://127.0.0.1:&lt;port&gt;</code> — a browser tab mirroring the
                    app&rsquo;s live state, useful for a second monitor or a browser-native reading
                    setup. It binds loopback only, and no setting exists that can move it onto a
                    network interface.
                </p>
            </DocsSection>

            <DocsSection id="standalone" heading="specforge-serve">
                <p>
                    The standalone binary serves the same UI without the desktop app — a homelab
                    box, a headless server, the machine where the repositories actually live. It
                    binds <code>127.0.0.1:4317</code> by default and renders the workspaces
                    registered on the machine running it.
                </p>
                <p>
                    The web UI behaves like a site, not a port of an app: browser back and forward
                    work, and every view has a durable URL you can bookmark or paste. Settings that
                    only make sense on the desktop are hidden.
                </p>
                <p>
                    If 4317 is already taken, <code>--port</code> moves it. Both it and{' '}
                    <code>--bind</code> also read from the environment, which is the easier route
                    under a service manager — a flag beats the environment, and the environment
                    beats the default:
                </p>
                <pre>
                    <code>{`specforge-serve --port 8080
SPECFORGE_WEB_PORT=8080 specforge-serve`}</code>
                </pre>
                <p>
                    <code>specforge-serve --help</code> prints the whole surface, which is
                    deliberately small: <code>--bind</code>, <code>--port</code>,{' '}
                    <code>--help</code> and <code>--version</code>.
                </p>
            </DocsSection>

            <DocsSection id="install" heading="Getting specforge-serve">
                <p>
                    The server publishes to npm as{' '}
                    <a href={NPM_PACKAGE_URL}>
                        <code>{NPM_PACKAGE}</code>
                    </a>
                    , so the shortest route to a running instance is one command inside any
                    workspace:
                </p>
                <pre>
                    <code>{`cd ~/some-openspec-workspace
npx ${NPM_PACKAGE}`}</code>
                </pre>
                <p>
                    <code>npx</code> re-resolves the package on every invocation. For a server you
                    intend to leave running, install it once instead:
                </p>
                <pre>
                    <code>{`npm install -g ${NPM_PACKAGE}
specforge-serve`}</code>
                </pre>
                <p>
                    macOS on Apple Silicon and Intel, Linux on x64 and arm64, and Windows on x64 are
                    all covered, and npm fetches only the binary matching the machine. There is no
                    install script and nothing is pulled from outside the registry, so the package
                    installs under <code>--ignore-scripts</code>, from an offline cache, and through
                    a private mirror.
                </p>
                <Note>
                    <strong>Nothing to unquarantine.</strong> Files a package manager unpacks are
                    not flagged the way a browser download is, so the <code>xattr</code> step{' '}
                    <a href="/docs/troubleshooting#macos-binaries">the standalone archives need</a>{' '}
                    does not apply to an npm install.
                </Note>
                <p>
                    Every <a href={LATEST_RELEASE_URL}>release</a> also ships{' '}
                    <code>specforge-serve</code> as a standalone archive — macOS universal, Linux
                    x64 and arm64, Windows x64 — if you would rather not involve npm. The Linux
                    builds are statically linked, so one binary covers Alpine and containers as well
                    as glibc distributions.
                </p>
            </DocsSection>

            <DocsSection id="trust" heading="The trust boundary">
                <p>
                    Reachability and authorisation are separate concerns. The bound interface
                    decides who can open a socket; a request-authority allow-list decides which page
                    may drive the API. On loopback, every request&rsquo;s <code>Origin</code> and{' '}
                    <code>Host</code> are checked against known authorities — loopback itself, plus
                    your machine&rsquo;s own <a href="#tailscale">Tailscale name</a> when that is
                    enabled — so a stray web page in your browser cannot quietly drive a server that
                    reads your filesystem.
                </p>
            </DocsSection>

            <DocsSection id="tailscale" heading="Remote access, the sanctioned way">
                <p>
                    For access beyond the machine, SpecForge supports{' '}
                    <a href="https://tailscale.com/kb/1312/serve">Tailscale Serve</a>: the proxy
                    connects to the loopback port, and SpecForge trusts your machine&rsquo;s own
                    tailnet name — resolved automatically, with a manual override, and failing
                    closed when no name is available. The server itself never binds a network
                    interface for this.
                </p>
                <p>
                    An optional allow-list of Tailscale logins narrows it further: when set, a
                    proxied request is accepted only if Tailscale identifies the visitor as one of
                    those users. Left empty, the tailnet itself is the boundary. Local loopback
                    requests never need a login, and Tailscale Funnel — public-internet exposure —
                    is not supported at all.
                </p>
                <p>
                    An SSH tunnel (<code>ssh -L 4317:127.0.0.1:4317 host</code>) works too, with no
                    configuration: from the server&rsquo;s point of view you are a loopback visitor.
                </p>
            </DocsSection>

            <DocsSection id="bind" heading="The --bind escape hatch">
                <p>
                    <code>specforge-serve --bind 0.0.0.0</code> publishes the UI on a network
                    interface directly — <strong>unauthenticated</strong>, with the authority checks
                    necessarily stood down. The network you publish on, and every site any browser
                    on it visits, becomes the trust boundary. It exists for networks you genuinely
                    trust; the flag is per-invocation and never persisted, so the exposure cannot
                    outlive the command that asked for it.
                </p>
                <Note>
                    A network bind and the login allow-list are incompatible by design: reachable
                    from the network, the login header could be forged by any peer, so{' '}
                    <code>specforge-serve</code> refuses to start rather than run with the gate
                    silently disabled.
                </Note>
            </DocsSection>

            <DocsSection id="tablet" heading="On a tablet or phone">
                <p>
                    Once it is reachable from your tailnet, the web UI is a reasonable way to read
                    specs away from the desk. The served document declares its own icon set, so
                    adding it to a home screen gives you a SpecForge mark and a full-screen window
                    rather than a browser chrome and a generic glyph.
                </p>
                <p>
                    The interface answers touch and pen directly: pane dividers are dragged through
                    pointer events, and the controls that have no hover equivalent — the collapse
                    chevrons, the favourite star — stay visible at rest instead of appearing on
                    hover, with larger hit areas on coarse pointers. None of that changes anything
                    on a desktop with a mouse.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
