import type { Changelog, ReleaseSummary } from '../../build/releaseNotes';
import { usePageData } from '../../site-kit/PageData';
import { LATEST_RELEASE_URL, releaseTagUrl } from '../../src/site-config';

/**
 * A release's date, or nothing at all.
 *
 * The date comes from the commit that added the note. Where history is
 * unavailable it is absent rather than guessed, so this renders nothing instead
 * of a plausible wrong date.
 */
function ReleaseDate({ note }: { note: ReleaseSummary }) {
    if (!note.date || !note.dateLabel) return null;
    return (
        <time dateTime={note.date} className="text-sm text-[var(--text-muted)]">
            {note.dateLabel}
        </time>
    );
}

/**
 * One earlier release: what it was called, when it landed, and the line its
 * author wrote to summarise it. The full notes are one link away.
 */
function EarlierRelease({ note }: { note: ReleaseSummary }) {
    return (
        <li className="border-t border-[var(--border)] py-4">
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <a href={releaseTagUrl(note.tag)} className="font-semibold no-underline">
                    {note.tag}
                </a>
                <ReleaseDate note={note} />
            </div>
            {note.standfirst && (
                <p className="mt-1 max-w-[68ch] text-[var(--text-muted)]">{note.standfirst}</p>
            )}
        </li>
    );
}

/**
 * The changelog.
 *
 * The current release is rendered from its own notes file; earlier releases are
 * listed rather than rendered, because the full history is around an hour's
 * reading and the page would ship all of it on every visit.
 *
 * `dangerouslySetInnerHTML` carries markup this site generated at build time
 * from first-party prose, and the build refuses any note containing raw HTML —
 * see `build/releaseNotes.ts`. The same string is used by the server render and
 * the hydration pass, so the subtree matches and React leaves it alone.
 */
export default function ChangelogPage() {
    const { current, earlier } = usePageData<Changelog>();

    return (
        <div className="site-shell py-12">
            <h1 className="mt-0 mb-4 text-3xl font-semibold tracking-tight">Changelog</h1>
            <p className="mb-10 max-w-[68ch] text-lg text-[var(--text-muted)]">
                What shipped in each release of SpecForge. The current release in full, then every
                earlier release with the line that summarises it.
            </p>

            <article className="prose-notes">
                <header className="mb-4 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                    <h2
                        id={`release-${current.version.replace(/\./g, '-')}`}
                        className="m-0 text-2xl font-semibold tracking-tight"
                    >
                        SpecForge {current.tag}
                    </h2>
                    <ReleaseDate note={current} />
                </header>

                {current.standfirst && (
                    <p className="mb-6 text-lg text-[var(--text-muted)]">{current.standfirst}</p>
                )}

                <div dangerouslySetInnerHTML={{ __html: current.bodyHtml }} />
            </article>

            {earlier.length > 0 && (
                <section className="mt-14">
                    <h2 className="mb-2 text-xl font-semibold tracking-tight">Earlier releases</h2>
                    <p className="mb-4 max-w-[68ch] text-[var(--text-muted)]">
                        Full notes for each of these are on{' '}
                        <a href={LATEST_RELEASE_URL}>the releases page</a>.
                    </p>
                    <ul className="m-0 max-w-[68ch] list-none p-0">
                        {earlier.map(note => (
                            <EarlierRelease key={note.tag} note={note} />
                        ))}
                    </ul>
                </section>
            )}
        </div>
    );
}
