/**
 * Build-time adaptation of the repository's release notes for the changelog page.
 *
 * The notes in `releases/` are authored by `/release` and consumed verbatim as
 * GitHub Release bodies (`release.yml`'s `body_path`). That makes their format a
 * shared contract this site must not change, so every adaptation happens here,
 * at build time, and nothing in this module ships to a visitor.
 *
 * Two things about the notes drive most of this file. They carry a Downloads
 * footer written for GitHub — where the release page is also the download page —
 * which duplicates this site's own download block and names version-pinned
 * artefacts that would go stale on a page describing an older release. And they
 * carry no `#` heading anywhere: the version line is authored as plain text, so
 * rendering a note verbatim would leave the page with no per-release heading at
 * all and its sections attached to nothing.
 *
 * Every assumption this module makes about that format is asserted rather than
 * trusted. The notes are machine-authored by a separate tool, and the failure
 * mode of a silent mismatch is publishing install instructions and stale
 * filenames onto the marketing site.
 *
 * `node:fs`, not a Vite import: `server.fs.allow` resolves to `site/` (the repo
 * root declares no workspaces and there is no pnpm or lerna marker above it), so
 * pulling `../releases` through Vite's module graph would be denied under
 * `vike dev`. A plain read is not subject to that guard and behaves identically
 * in dev, build and CI.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
import { Marked, type Tokens } from 'marked';

/** A release note, cut to its changelog and split into its parts. */
export interface ReleaseNote {
    /** The git tag, including the leading `v` — the notes filename minus `.md`. */
    tag: string;
    /** The tag without its leading `v`, as `site-config.ts` spells the version. */
    version: string;
    /** ISO `YYYY-MM-DD`, or null when no date could be established. */
    date: string | null;
    /**
     * The date as the page displays it, formatted here rather than in the
     * component. The page is server-rendered and then hydrated, so a formatter
     * whose output depends on the host's ICU data or time zone would risk the
     * two renders disagreeing. This one depends on neither.
     */
    dateLabel: string | null;
    /**
     * The note's opening paragraph, written to summarise the release. Empty for
     * a note that goes straight from its version line to its first section —
     * `releases/v0.0.2.md` is the one such note in the corpus today.
     */
    standfirst: string;
    /** The changelog markdown: everything above the Downloads cut, title removed. */
    bodyMarkdown: string;
}

/**
 * A release as the page displays it, without its source markdown.
 *
 * The distinction from {@link ReleaseNote} is not tidiness: `passToClient`
 * serialises whatever `+data` returns into the prerendered document, so any
 * field left on these objects is shipped to every visitor. Keeping the markdown
 * out means the history costs a line each rather than the 78KB of prose it would
 * otherwise carry — on a page that renders only the standfirsts.
 */
export type ReleaseSummary = Omit<ReleaseNote, 'bodyMarkdown'>;

/** A release rendered for display, with its body converted to HTML. */
export interface RenderedRelease extends ReleaseSummary {
    /** The body converted to HTML. First-party content, asserted free of raw HTML. */
    bodyHtml: string;
}

/** What the changelog page needs: one release in full, the rest in summary. */
export interface Changelog {
    current: RenderedRelease;
    earlier: ReleaseSummary[];
}

/**
 * Matches the footer heading each note carries. Anchored and whole-line so a
 * mention of downloads in running prose cannot truncate a release's changelog.
 */
const DOWNLOADS_HEADING = /^#{1,6}\s+Downloads\s*$/;

/** `v1.2.3`, capturing the parts, with any prerelease suffix in group 4. */
const TAG = /^v(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/;

/**
 * The notes directory, resolved from the site root.
 *
 * Every site command runs with the working directory set to `site/` — the root
 * `site:*` scripts go through `bun run --cwd site` and the workflow sets
 * `working-directory: site` — which is the same assumption `site-kit`'s build
 * plugins make when they resolve `<cwd>/pages`.
 */
function notesDir(): string {
    const dir = resolve(process.cwd(), '..', 'releases');
    if (!existsSync(dir)) {
        throw new Error(
            `[changelog] No release-notes directory at ${dir}. Site commands must run with the working directory set to site/.`,
        );
    }
    return dir;
}

/**
 * Drop the Downloads footer.
 *
 * The cut is what makes the remaining markdown safe to render: every code fence,
 * every bare URL and every pinned install command in the corpus sits below this
 * heading, along with artefact filenames that are correct on a GitHub release
 * page and stale anywhere else.
 *
 * A note without the heading is a build failure, not a note to publish whole.
 * The heading is a convention between this site and a tool that authors these
 * files independently; nothing enforces it, so a reworded footer must stop the
 * build rather than quietly ship install instructions as changelog copy.
 */
function cutAtDownloads(source: string, file: string): string {
    const lines = source.split('\n');
    const cut = lines.findIndex(line => DOWNLOADS_HEADING.test(line));
    if (cut === -1) {
        throw new Error(
            `[changelog] ${file} carries no Downloads heading, so its changelog cannot be separated from its download footer. Either the note is malformed or the release template changed; the site renders nothing rather than publish the footer as changelog copy.`,
        );
    }
    const body = lines.slice(0, cut);
    // The footer is introduced by a thematic break. Cutting above the heading
    // leaves it dangling as a rule with nothing after it.
    while (body.length > 0) {
        const last = body[body.length - 1].trim();
        if (last === '' || last === '---') {
            body.pop();
            continue;
        }
        break;
    }
    return body.join('\n');
}

/**
 * Split a cut note into its version line, its standfirst, and its body.
 *
 * The version line is authored as plain text rather than as a heading, so it is
 * removed here and re-emitted by the page as a real heading. Without that the
 * page would carry no per-release heading and every section would attach to
 * nothing.
 */
function splitNote(cutBody: string): { standfirst: string; bodyMarkdown: string } {
    const lines = cutBody.split('\n');

    // Line 1 is the version line. Drop it, then any blank lines below it.
    let i = 1;
    while (i < lines.length && lines[i].trim() === '') i++;

    // A standfirst is the paragraph that follows. A note whose next content is
    // already a section heading simply has none.
    const standfirst: string[] = [];
    if (i < lines.length && !lines[i].startsWith('#')) {
        while (i < lines.length && lines[i].trim() !== '') {
            standfirst.push(lines[i].trim());
            i++;
        }
    }

    while (i < lines.length && lines[i].trim() === '') i++;

    return {
        standfirst: standfirst.join(' '),
        bodyMarkdown: lines.slice(i).join('\n').trim(),
    };
}

/**
 * The date each note was committed, for every note at once.
 *
 * `/release` commits a note and tags that same commit, so the commit that adds
 * `releases/<tag>.md` is the release. This reads that in a single `git log`
 * rather than one call per file.
 *
 * It needs history: the site workflow checks out with `fetch-depth: 0` for this
 * reason. Where history is unavailable — a shallow clone, an archive export, a
 * note not yet committed during a local `/release` run — the caller falls back
 * rather than failing, because a missing date degrades the page by one line
 * while a wrong date misinforms.
 */
function commitDates(dir: string): Map<string, string> {
    const dates = new Map<string, string>();
    let out: string;
    try {
        out = execFileSync(
            'git',
            ['log', '--diff-filter=A', '--format=%x00%cI', '--name-only', '--', dir],
            { cwd: dir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] },
        );
    } catch {
        return dates;
    }

    let current: string | null = null;
    for (const line of out.split('\n')) {
        if (line.startsWith('\0')) {
            current = line.slice(1, 11);
            continue;
        }
        const name = basename(line.trim());
        // git log walks newest first; the first add we see for a file wins.
        if (current && name.endsWith('.md') && !dates.has(name)) {
            dates.set(name, current);
        }
    }
    return dates;
}

const MONTHS = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
];

/** `2026-09-02` to `2 September 2026`, without Intl or a time zone. */
function formatDate(iso: string): string | null {
    const parts = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
    if (!parts) return null;
    const month = MONTHS[Number(parts[2]) - 1];
    if (!month) return null;
    return `${Number(parts[3])} ${month} ${parts[1]}`;
}

/** Newest first, by semantic version rather than by filename. */
function byVersionDesc(a: ReleaseNote, b: ReleaseNote): number {
    const pa = TAG.exec(a.tag);
    const pb = TAG.exec(b.tag);
    if (!pa || !pb) return b.tag.localeCompare(a.tag);
    for (let i = 1; i <= 3; i++) {
        const diff = Number(pb[i]) - Number(pa[i]);
        if (diff !== 0) return diff;
    }
    return 0;
}

/**
 * Every final release, newest first.
 *
 * Prereleases are excluded: a release candidate's notes are a near-duplicate of
 * the final release that follows days later, and listing both reads as the page
 * repeating itself.
 */
export function loadReleaseNotes(): ReleaseNote[] {
    const dir = notesDir();
    const dates = commitDates(dir);

    const notes: ReleaseNote[] = [];
    for (const name of readdirSync(dir)) {
        if (!name.endsWith('.md')) continue;
        const tag = name.slice(0, -3);
        const parsed = TAG.exec(tag);
        if (!parsed) continue; // README.md and anything else that is not a note.
        if (parsed[4]) continue; // A prerelease.

        const file = join(dir, name);
        const { standfirst, bodyMarkdown } = splitNote(cutAtDownloads(readFileSync(file, 'utf8'), name));

        let date = dates.get(name) ?? null;
        if (!date) {
            // No history to read. The file's own timestamp is the honest
            // remaining answer; on a fresh clone it is the checkout time, which
            // is why git is asked first.
            try {
                date = statSync(file).mtime.toISOString().slice(0, 10);
            } catch {
                date = null;
            }
        }

        notes.push({
            tag,
            version: tag.slice(1),
            date,
            dateLabel: date ? formatDate(date) : null,
            standfirst,
            bodyMarkdown,
        });
    }

    return notes.sort(byVersionDesc);
}

/** Strip emoji and punctuation to a URL-safe fragment. */
function slugify(text: string): string {
    return text
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '');
}

/**
 * Convert one note's body to HTML.
 *
 * Two adjustments, both about the page rather than the note. Headings are
 * demoted one level so the note's sections sit under the release heading the
 * page emits, keeping document order intact. And heading ids are namespaced by
 * release: the corpus reuses five section names across every release, so an
 * un-namespaced slugger would emit many elements sharing one id and break every
 * deep link into the page.
 */
export function renderNoteHtml(note: ReleaseNote): string {
    const marked = new Marked({ gfm: true });

    marked.use({
        /**
         * The notes are first-party prose written without escaping discipline,
         * and marked passes raw HTML straight through to output that this site
         * injects with `dangerouslySetInnerHTML`. There is no raw HTML in the
         * corpus today — every angle bracket sits inside a code span — so the
         * build asserts that rather than trusting it to stay true.
         *
         * This walks the whole token tree, not just block-level tokens: inline
         * HTML inside a paragraph is a nested token, and checking only the top
         * level would let exactly the dangerous case through while appearing to
         * guard against it.
         */
        walkTokens(token) {
            if (token.type === 'html') {
                throw new Error(
                    `[changelog] ${note.tag}.md contains raw HTML (${JSON.stringify(token.raw.slice(0, 60))}), which this site does not render. Express the content as markdown instead.`,
                );
            }
        },
        renderer: {
            heading(this: { parser: { parseInline: (t: Tokens.Generic[]) => string } }, token: Tokens.Heading) {
                const depth = Math.min(token.depth + 1, 6);
                const id = `${slugify(note.tag)}-${slugify(token.text)}`;
                return `<h${depth} id="${id}">${this.parser.parseInline(token.tokens)}</h${depth}>\n`;
            },
        },
    });

    return marked.parse(note.bodyMarkdown, { async: false });
}

/**
 * The changelog page's content: the advertised release in full, the rest listed.
 *
 * Rendering every release would be 12,605 words — around an hour's reading — and
 * because the page hydrates, its content ships twice. Earlier releases are
 * summarised by the standfirst their author already wrote for that purpose.
 */
export function loadChangelog(currentVersion: string): Changelog {
    const notes = loadReleaseNotes();
    const index = notes.findIndex(note => note.version === currentVersion);

    if (index === -1) {
        throw new Error(
            `[changelog] No release note for the advertised version ${currentVersion}. Expected ${resolve(notesDir(), `v${currentVersion}.md`)}. RELEASE_VERSION in site/src/site-config.ts and the notes file are written by the same /release step and must move together.`,
        );
    }

    // Drop `bodyMarkdown` from everything that crosses into the page: the
    // current release travels as rendered HTML, and earlier releases as their
    // standfirst alone. Both are serialised into the document by `passToClient`,
    // so what is not stripped here is downloaded by every visitor.
    const summarise = ({ bodyMarkdown: _drop, ...rest }: ReleaseNote): ReleaseSummary => rest;

    const current = notes[index];
    return {
        current: { ...summarise(current), bodyHtml: renderNoteHtml(current) },
        earlier: notes.filter((_, i) => i !== index).map(summarise),
    };
}
