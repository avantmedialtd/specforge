import type { ReactNode } from 'react';
import { JsonLd } from '../../src/components/JsonLd';
import {
    LATEST_RELEASE_URL,
    NPM_PACKAGE,
    NPM_PACKAGE_URL,
    OPENSPEC_URL,
    REPO_URL,
    SITE_URL,
    STUDIO_URL,
} from '../../src/site-config';

/**
 * The product itself, as structured data. Every field is checkable against the
 * public repository. There is deliberately no rating, user count or version
 * claim for the site to invent or keep in sync.
 */
const softwareJsonLd = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'SpecForge',
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'macOS 11.0+, Windows, Linux',
    url: SITE_URL,
    downloadUrl: LATEST_RELEASE_URL,
    softwareHelp: `${SITE_URL}/docs`,
    codeRepository: REPO_URL,
    license: 'https://opensource.org/licenses/MIT',
    isAccessibleForFree: true,
    offers: {
        '@type': 'Offer',
        price: '0',
        priceCurrency: 'GBP',
    },
    author: {
        '@type': 'Organization',
        name: 'Avant Media',
        url: STUDIO_URL,
    },
};

function ProductSurface({
    label,
    title,
    meta,
    children,
}: {
    label: string;
    title: string;
    meta: string;
    children: ReactNode;
}) {
    return (
        <article className="surface-card">
            <p className="surface-label">{label}</p>
            <h3>{title}</h3>
            <p>{children}</p>
            <p className="surface-meta">{meta}</p>
        </article>
    );
}

function Download({
    platform,
    detail,
    formats,
}: {
    platform: string;
    detail: string;
    formats: string;
}) {
    return (
        <article className="download-card">
            <h3>{platform}</h3>
            <p>{detail}</p>
            <p className="download-formats">{formats}</p>
        </article>
    );
}

export default function Home() {
    return (
        <>
            <JsonLd data={softwareJsonLd} />

            <section className="landing-hero">
                <div className="hero-grid" aria-hidden="true" />
                <div className="hero-rail" aria-hidden="true">
                    <span />
                    <span />
                    <span />
                    <span />
                </div>

                <div className="hero-inner">
                    <div className="hero-copy">
                        <p className="hero-kicker">
                            <span aria-hidden="true" /> A visual companion for spec-driven
                            development
                        </p>
                        <h1>Spec-driven work, in full view.</h1>
                        <p className="hero-summary">
                            SpecForge turns the intent, designs, specs and tasks in your
                            repositories into a live, navigable view. It supports{' '}
                            <a href={OPENSPEC_URL}>OpenSpec</a> today, placing each change beside
                            the repository-wide Git graph, worktrees, commits and diffs.
                        </p>

                        <div className="hero-actions">
                            <a href="#downloads" className="btn-primary">
                                Get SpecForge
                            </a>
                            <a href={REPO_URL} className="btn-secondary btn-secondary-dark">
                                View on GitHub
                            </a>
                        </div>

                        <div className="hero-command" aria-label="Run SpecForge in a local browser">
                            <span>Run the full local interface</span>
                            <code>npx {NPM_PACKAGE}</code>
                        </div>

                        <ul className="hero-proof" aria-label="Product facts">
                            <li>Your files stay the source</li>
                            <li>Read-only to your workspaces</li>
                            <li>Free and MIT licensed</li>
                        </ul>
                    </div>

                    <figure className="product-stage">
                        <div className="product-stage-bar">
                            <span>Selected change</span>
                            <span>Spec + Git</span>
                        </div>
                        <picture>
                            <source srcSet="/screenshot.webp" type="image/webp" />
                            <img
                                src="/screenshot.png"
                                width={1499}
                                height={1126}
                                loading="eager"
                                fetchPriority="high"
                                alt="SpecForge browsing a change's tasks, with the workspace tree on the left and the commit graph on the right"
                            />
                        </picture>
                        <figcaption className="product-legend">
                            <span>
                                <b>01</b> Workspace and worktrees
                            </span>
                            <span>
                                <b>02</b> Proposal, specs and tasks
                            </span>
                            <span>
                                <b>03</b> Commits and diffs
                            </span>
                        </figcaption>
                    </figure>
                </div>

                <div className="hero-platforms" aria-label="Available interfaces">
                    <span>Desktop app</span>
                    <span>Local web app</span>
                    <span>Terminal UI</span>
                    <span>macOS / Windows / Linux</span>
                </div>
            </section>

            <section className="landing-section split-story" id="worktrees">
                <div className="story-copy">
                    <p className="landing-eyebrow">The repository remains the source of truth</p>
                    <h2>Specs give work structure. SpecForge gives it a view.</h2>
                    <p>
                        Spec-driven development works because intent, design and tasks live beside
                        the code. OpenSpec is the format SpecForge reads today: its files are
                        portable, reviewable and legible to people and agents. But as repositories,
                        changes and worktrees multiply, that state becomes hard to hold in your
                        head.
                    </p>
                    <p>
                        Register a workspace once. SpecForge discovers its changes and sibling Git
                        worktrees, groups matching copies, and updates the view when files or
                        commits change.
                    </p>
                    <p className="story-callout">
                        No import. No migration. No second source of truth.
                    </p>
                </div>

                <div
                    className="logical-change"
                    aria-label="Example logical change across worktrees"
                >
                    <div className="logical-change-header">
                        <div>
                            <span className="tree-chevron" aria-hidden="true">
                                v
                            </span>
                            <strong>improve-search-ranking</strong>
                        </div>
                        <span>1 logical change</span>
                    </div>
                    <div className="worktree-row">
                        <span className="branch-dot branch-dot-indigo" aria-hidden="true" />
                        <div className="worktree-name">
                            <strong>agent/implementation</strong>
                            <span>updated now</span>
                        </div>
                        <div className="mini-progress" aria-label="18 of 24 tasks">
                            <span style={{ width: '75%' }} />
                        </div>
                        <code>18 / 24</code>
                        <span className="state-chip state-chip-warn">modified</span>
                    </div>
                    <div className="worktree-row">
                        <span className="branch-dot branch-dot-green" aria-hidden="true" />
                        <div className="worktree-name">
                            <strong>agent/review-fixes</strong>
                            <span>updated 8m ago</span>
                        </div>
                        <div className="mini-progress" aria-label="24 of 24 tasks">
                            <span style={{ width: '100%' }} />
                        </div>
                        <code>24 / 24</code>
                        <span className="state-chip state-chip-ok">committed</span>
                    </div>
                    <div className="worktree-row">
                        <span className="branch-dot branch-dot-amber" aria-hidden="true" />
                        <div className="worktree-name">
                            <strong>agent/spike</strong>
                            <span>updated 2h ago</span>
                        </div>
                        <div className="mini-progress" aria-label="7 of 24 tasks">
                            <span style={{ width: '29%' }} />
                        </div>
                        <code>7 / 24</code>
                        <span className="state-chip">stale</span>
                    </div>
                    <div className="logical-change-footer">
                        <span>3 worktrees resolve to</span>
                        <strong>1</strong>
                        <span>spec change</span>
                    </div>
                </div>
            </section>

            <section className="questions-section">
                <div className="landing-section">
                    <div className="section-intro">
                        <p className="landing-eyebrow">One change, end to end</p>
                        <h2>Follow the thinking all the way to Git.</h2>
                        <p>
                            SpecForge keeps the artifacts, task state and repository evidence
                            together, so a review starts with context and can move directly to what
                            is in Git.
                        </p>
                    </div>

                    <div className="question-grid">
                        <article>
                            <p className="question-number">01 / Navigate</p>
                            <h3>Find the work in context.</h3>
                            <p>
                                Move across registered workspaces and active or archived changes
                                without walking folder trees. Progress, modified times and Git-state
                                badges show where attention belongs.
                            </p>
                        </article>
                        <article>
                            <p className="question-number">02 / Read</p>
                            <h3>Read artifacts as documents.</h3>
                            <p>
                                Open proposals, designs, specs and tasks in a focused reader with
                                highlighted code, Mermaid diagrams, maths and SVG. The filesystem
                                remains underneath; the reading experience does not have to feel
                                like one.
                            </p>
                        </article>
                        <article>
                            <p className="question-number">03 / Verify</p>
                            <h3>Connect intent to evidence.</h3>
                            <p>
                                Keep the repository-wide <code>git log --all</code> graph beside the
                                change. Open a commit to inspect its files and textual diff, then
                                return to the spec context without changing tools.
                            </p>
                        </article>
                    </div>
                </div>
            </section>

            <section className="surfaces-section" id="read-only">
                <div className="landing-section">
                    <div className="section-intro section-intro-light">
                        <p className="landing-eyebrow">Local-first / read-only / one Rust core</p>
                        <h2>A companion, not another system of record.</h2>
                        <p>
                            SpecForge reads the specs and Git state on the machine where it runs.
                            There is no hosted workspace to sync and no competing place to edit the
                            work.
                        </p>
                    </div>

                    <div className="trust-grid">
                        <article>
                            <h3 className="surface-label">Reads locally</h3>
                            <p>
                                OpenSpec's <code>openspec/</code> tree, Markdown artifacts, task
                                state, Git worktrees and repository history.
                            </p>
                        </article>
                        <article>
                            <h3 className="surface-label">Never changes your work</h3>
                            <p>
                                It does not edit a spec, toggle a task, archive a change, check out
                                a branch, merge, rebase or reset anything.
                            </p>
                        </article>
                        <article>
                            <h3 className="surface-label">Writes outside projects</h3>
                            <p>
                                SpecForge writes only its own app state outside your workspaces:
                                registry, settings, favourites, presentation and activity history.
                            </p>
                        </article>
                    </div>

                    <div className="surface-grid">
                        <ProductSurface
                            label="Desktop"
                            title="Stay aware without staying in the app"
                            meta="macOS 11.0+ / Windows x64 / Linux x64"
                        >
                            Keep the change browser one click from the tray. Its badge counts
                            distinct changes while the main window is out of the way.
                        </ProductSurface>
                        <ProductSurface
                            label="Browser"
                            title="Run it locally or reach it remotely"
                            meta={`npx ${NPM_PACKAGE}`}
                        >
                            Run it on localhost, a headless machine, or through an SSH tunnel or
                            Tailscale Serve connection, with durable URLs and browser navigation.
                        </ProductSurface>
                        <ProductSurface
                            label="Terminal"
                            title="Keep it inside SSH and tmux"
                            meta="interactive / --status / --line"
                        >
                            Browse changes in a full TUI, print a one-shot status for scripts, or
                            place a compact SpecForge line in your prompt.
                        </ProductSurface>
                    </div>
                </div>
            </section>

            <section id="downloads" className="downloads-section">
                <div className="landing-section">
                    <div className="downloads-heading">
                        <div>
                            <p className="landing-eyebrow">Start local. Stay local.</p>
                            <h2>Open your first workspace.</h2>
                        </div>
                        <p>
                            Run the browser interface with one npm command, or download the desktop
                            app, TUI or server. SpecForge is free, MIT licensed and in early, active
                            development (v0.x).
                        </p>
                    </div>

                    <div className="quickstart-grid">
                        <article className="npm-quickstart">
                            <p className="surface-label">Fastest start / Local web app</p>
                            <h3>Try the full interface in one command.</h3>
                            <p>
                                The npm package fetches only the binary for your platform, starts on
                                loopback and needs no archive extraction, no postinstall script and
                                no quarantine flag.
                            </p>
                            <pre>
                                <code>npx {NPM_PACKAGE}</code>
                            </pre>
                            <p className="quickstart-note">
                                Requires Node 18+. Open <code>http://127.0.0.1:4317</code>, then add
                                the workspace's host path in Settings.
                            </p>
                            <a href={NPM_PACKAGE_URL}>View the package on npm</a>
                        </article>

                        <div className="release-route">
                            <p className="surface-label">Desktop + standalone tools</p>
                            <h3>Prefer a native app or a single binary?</h3>
                            <p>
                                The latest release includes the desktop app, terminal UI and server,
                                with portable options that need no installer.
                            </p>
                            <a href={LATEST_RELEASE_URL} className="btn-primary">
                                Browse the latest release
                            </a>
                            <a href="/docs" className="text-link">
                                Read setup and platform notes
                            </a>
                        </div>
                    </div>

                    <div className="download-grid">
                        <Download
                            platform="Desktop app"
                            detail="macOS 11.0+, Windows x64 and Linux x64"
                            formats=".dmg / NSIS .exe / portable .exe / .deb / .AppImage"
                        />
                        <Download
                            platform="Terminal UI"
                            detail="Interactive browser, one-shot status and prompt modes"
                            formats="macos-universal / linux-x64 / windows-x64"
                        />
                        <Download
                            platform="Local web server"
                            detail="Browser UI for local, headless or tunnelled access"
                            formats="macos-universal / linux-x64 / linux-arm64 / windows-x64"
                        />
                    </div>

                    <div className="download-notes">
                        <div>
                            <strong>Downloaded releases are unsigned.</strong>
                            <p>
                                macOS Gatekeeper and Windows SmartScreen may warn on first launch.{' '}
                                <a href="/docs/troubleshooting">Troubleshooting</a> gives the exact
                                steps for each platform.
                            </p>
                        </div>
                        <div>
                            <strong>Keep direct network binds private.</strong>
                            <p>
                                Keep <code>specforge-serve</code> on loopback, or use an SSH tunnel
                                or Tailscale Serve. A direct <code>--bind 0.0.0.0</code> is
                                unauthenticated and belongs only on a network you trust.{' '}
                                <a href="/docs/web-ui">Read the remote-access guide</a>.
                            </p>
                        </div>
                    </div>
                </div>
            </section>
        </>
    );
}
