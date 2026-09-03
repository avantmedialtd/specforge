import { DocsLayout, DocsSection, Note } from '../../../src/components/DocsLayout';

export default function CommitGraph() {
    return (
        <DocsLayout
            title="Reading the commit graph"
            intro="The right-hand rail is a faithful git log --all graph for the selected change’s repository — not a simplified summary of it."
            currentPath="/docs/commit-graph"
        >
            <DocsSection id="lanes" heading="Lanes and topology">
                <p>
                    Each commit sits in a lane, and lanes track branches as they diverge and rejoin.
                    Branch points and merge commits are drawn as they actually happened, so a merge
                    reads as two lanes converging rather than as a flat list with a label.
                </p>
                <p>
                    Because the graph is built from <code>git log --all</code>, it shows every ref
                    in the repository — not only the branch you happen to have checked out.
                </p>
            </DocsSection>

            <DocsSection id="decorations" heading="Ref decorations">
                <p>
                    Commits carrying refs — branch heads, tags — are decorated with them, so you can
                    locate a branch tip in the graph without reading hashes.
                </p>
            </DocsSection>

            <DocsSection id="day-bands" heading="Day bands">
                <p>
                    Commits are grouped into day bands, labelled <em>Today</em>, <em>Yesterday</em>,
                    then weekday names for the rest of the week, then absolute dates further back.
                </p>
                <p>
                    Read cadence down the page rather than across. A day with many commits is a
                    taller run of rows — one row per commit, beneath a label of fixed height. Width
                    belongs to branch topology, not volume: it tracks how many lanes are alive at
                    that point in the history.
                </p>
                <p>
                    A day with no commits produces no band at all. Separators are drawn only above
                    the first commit of a day, so quiet stretches are skipped rather than left as a
                    gap — six silent months and six consecutive daily commits look the same here.
                    The Dashboard&rsquo;s <a href="/docs/dashboard">contribution heatmap</a> is
                    where absence is meant to show.
                </p>
            </DocsSection>

            <DocsSection id="commits" heading="Opening a commit">
                <p>
                    Click a commit to see the files it changed and their diffs, in the same window —
                    no need to switch to a git client to answer &ldquo;what was in that one?&rdquo;
                </p>
            </DocsSection>

            <DocsSection id="live" heading="It updates itself">
                <p>
                    The graph is driven by the same filesystem watcher as the rest of the app.
                    Commit something in your editor and the rail reflects it — the badge, tree,
                    detail pane and graph carry no refresh button. (The one deliberately pull-based
                    view is the <a href="/docs/workspaces#file-browser">repository file browser</a>,
                    which does.)
                </p>
                <Note>
                    Reading the graph is all it does. SpecForge never checks out, commits, merges,
                    or otherwise touches git — see the <a href="/#read-only">read-only section</a>{' '}
                    on the home page.
                </Note>
            </DocsSection>
        </DocsLayout>
    );
}
