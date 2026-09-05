import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';

export default function Dashboard() {
    return (
        <DocsLayout
            title="Dashboard"
            intro="The view the app opens onto: a read-only overview of every registered workspace, and an honest reading of what shipped today. Everything on it is measured — mined from git and an activity log — and none of it can be edited from here."
            currentPath="/docs/dashboard"
        >
            <DocsSection id="progress" heading="Today’s progress">
                <p>
                    A header opens the view: an identicon derived from your resolved git identity,
                    the date, and — on the right — your <strong>streak</strong>, the number of
                    consecutive days with at least one recorded achievement. The flame lights only
                    once the streak is running.
                </p>
                <p>
                    Below that, four counts aggregated across every workspace: changes{' '}
                    <strong>shipped</strong> today, changes <strong>in flight</strong> right now,{' '}
                    <strong>commits landed</strong> today, and <strong>tasks completed</strong>{' '}
                    today. The three today counts each carry a comparison against your recent daily
                    average, so the numbers answer &ldquo;is today a good day?&rdquo; without you
                    having to remember what a normal one looks like.
                </p>
                <p>
                    Then a <strong>contribution heatmap</strong> over recent weeks, with
                    today&rsquo;s cell marked. The cells are not just a picture: click one to read
                    that day back — what shipped, what landed, what was completed — or{' '}
                    <em>Nothing logged</em> for a quiet day.
                </p>
                <Note>
                    These personal counts attribute work to <em>you</em> as resolved from your git
                    identity. If your commits span several emails or machine-specific names, fold
                    them into one identity in{' '}
                    <a href="/docs/settings#identity">Settings ▸ Identity</a> — the numbers are only
                    as honest as the attribution.
                </Note>
            </DocsSection>

            <DocsSection id="ships" heading="Today’s ships">
                <p>
                    Every change archived today, across every workspace, newest first — with a
                    relative archive time when git can recover it. Clicking a ship opens the change
                    in the archive browser, so &ldquo;what was that one?&rdquo; is one click, not a
                    directory listing.
                </p>
                <p>
                    While the Dashboard is open, shipping a change plays a brief celebration and
                    completing a task a quieter one. Both respect your reduced-motion preference,
                    and neither blocks anything.
                </p>
            </DocsSection>

            <DocsSection id="garden" heading="The commit garden">
                <p>
                    At the bottom, each repository that has commits today draws a faithful git graph
                    of <strong>today&rsquo;s commits</strong> — real lanes, merges and refs, not a
                    sparkline. Each commit is coloured by its author: you in the accent colour,
                    everyone else — teammates, agents committing under their own identity — in a
                    stable hue of their own. It fills in live as commits land, and at local midnight
                    it starts again.
                </p>
                <p>
                    A repository with nothing on today&rsquo;s date is left out rather than shown
                    empty, and on a day when nothing has landed anywhere the whole section is absent
                    — so the garden is only ever a picture of work that happened, never a row of
                    blank plots.
                </p>
                <p>
                    Your own commits draw in the accent colour — but only for the emails you have
                    added as <a href="/docs/settings#identity">aliases</a>. An address you commit
                    under but have not claimed is treated as someone else&rsquo;s, and is left out
                    of your streak and heatmap too, so it is worth adding them all. Everyone else
                    is coloured by the git author recorded on the commit, with no naming step — so
                    a teammate who commits under two identities gets two colours.
                </p>
            </DocsSection>

            <DocsSection id="overview" heading="The cross-workspace overview">
                <p>
                    The rest of the Dashboard aggregates state: active and archived totals, a
                    per-repository breakdown, and change-lifecycle figures — how many changes ship,
                    and how long they take to get there.
                </p>
            </DocsSection>

            <DocsSection id="honest" heading="Where the numbers come from">
                <p>
                    An append-only activity log records achievements — a task ticked, an artifact
                    added, a change created or archived — as the watcher observes them, and is
                    backfilled from git history, so the Dashboard is not empty on first run. The log
                    lives in SpecForge&rsquo;s own application data, never inside a workspace.
                </p>
                <p>
                    There are no points, badges, or levels. The Dashboard measures what already
                    happened and invents nothing — which is also why there is no setting to turn it
                    off: it is a reading, not a game.
                </p>
            </DocsSection>
        </DocsLayout>
    );
}
