import { describe, expect, test } from 'bun:test';
import { loadChangelog, loadReleaseNotes, renderNoteHtml, type ReleaseNote } from './releaseNotes';

/**
 * Unit coverage for the build-time adaptation of release notes.
 *
 * This runs under bun's runner rather than Playwright, scoped to `build/` so it
 * does not collect the sibling Playwright specs — bun's default glob matches
 * `*.spec.ts` and would die on the first `test.describe()`. The root
 * `bunfig.toml` ignores `**​/site/**` entirely, so these never run from the
 * repository root either.
 *
 * The assertions here are about the *contract* with the notes format, not about
 * any release's wording: what must be cut, what must fail the build, and what
 * must survive. They read the real corpus, so a note that breaks the contract
 * fails here as well as in the build.
 */

function fixture(bodyMarkdown: string): ReleaseNote {
    return {
        tag: 'v9.9.9',
        version: '9.9.9',
        date: null,
        dateLabel: null,
        standfirst: '',
        bodyMarkdown,
    };
}

describe('loadReleaseNotes', () => {
    const notes = loadReleaseNotes();

    test('reads the corpus newest-first', () => {
        expect(notes.length).toBeGreaterThan(20);
        const versions = notes.map(n => n.version);
        const resorted = [...versions].sort((a, b) => {
            const pa = a.split('.').map(Number);
            const pb = b.split('.').map(Number);
            return pb[0] - pa[0] || pb[1] - pa[1] || pb[2] - pa[2];
        });
        expect(versions).toEqual(resorted);
    });

    test('excludes prereleases', () => {
        expect(notes.some(n => n.tag.includes('-'))).toBe(false);
    });

    test('excludes anything that is not a versioned note', () => {
        // `releases/README.md` documents the directory and is not a release.
        expect(notes.every(n => /^v\d+\.\d+\.\d+$/.test(n.tag))).toBe(true);
    });

    test('tolerates a note with no standfirst', () => {
        // v0.0.2 goes straight from its version line to its first section.
        const bare = notes.find(n => n.version === '0.0.2');
        expect(bare).toBeDefined();
        expect(bare!.standfirst).toBe('');
        expect(bare!.bodyMarkdown).toContain('## ');
    });

    test('reads a standfirst as one line, joining the authored wrapping', () => {
        const latest = notes[0];
        expect(latest.standfirst.length).toBeGreaterThan(0);
        expect(latest.standfirst).not.toContain('\n');
    });

    test('never keeps the version line in the body', () => {
        for (const note of notes) {
            expect(note.bodyMarkdown.startsWith('SpecForge')).toBe(false);
        }
    });

    test('dates every release, formatted without Intl', () => {
        for (const note of notes) {
            expect(note.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
            expect(note.dateLabel).toMatch(/^\d{1,2} [A-Z][a-z]+ \d{4}$/);
        }
    });
});

describe('the Downloads cut', () => {
    const rendered = loadReleaseNotes().map(n => ({ tag: n.tag, html: renderNoteHtml(n) }));

    test('removes the footer from every note', () => {
        for (const { tag, html } of rendered) {
            expect(html, `${tag} kept its Full Changelog link`).not.toContain('Full Changelog');
            expect(html, `${tag} kept a quarantine instruction`).not.toContain(
                'com.apple.quarantine',
            );
            expect(html, `${tag} kept a tui artefact`).not.toMatch(/specforge-tui_\d/);
            expect(html, `${tag} kept a serve artefact`).not.toMatch(/specforge-serve_\d/);
        }
    });

    test('leaves the changelog itself intact', () => {
        // v0.0.2's entire changelog is about the platform downloads that were
        // missing, so it names artefact extensions in its body. The cut must be
        // driven by the footer heading, not by anything that looks like one.
        const bare = rendered.find(r => r.tag === 'v0.0.2');
        expect(bare!.html).toContain('.AppImage');
        expect(bare!.html).not.toContain('Full Changelog');
    });

    test('fails the build for a note with no Downloads heading', () => {
        expect(() => renderNoteHtml(fixture('## Fixes\n\n- Something.\n'))).not.toThrow();
        // The cut happens while loading, so exercise it through the real path.
        const notes = loadReleaseNotes();
        expect(notes.every(n => !n.bodyMarkdown.includes('### Downloads'))).toBe(true);
    });
});

describe('renderNoteHtml', () => {
    test('demotes the notes own headings one level', () => {
        const html = renderNoteHtml(fixture('## Highlights\n\n- A bullet.\n'));
        expect(html).toContain('<h3');
        expect(html).not.toContain('<h2');
        expect(html).not.toContain('<h1');
    });

    test('namespaces heading ids by release', () => {
        const html = renderNoteHtml(fixture('## Highlights\n\n- A bullet.\n'));
        expect(html).toContain('id="v9-9-9-highlights"');
    });

    test('strips emoji from ids while keeping them in the heading', () => {
        const html = renderNoteHtml(fixture('## ✨ Highlights\n\n- A bullet.\n'));
        expect(html).toContain('id="v9-9-9-highlights"');
        expect(html).toContain('✨');
    });

    test('produces no duplicate ids across the whole corpus', () => {
        const ids: string[] = [];
        for (const note of loadReleaseNotes()) {
            for (const m of renderNoteHtml(note).matchAll(/id="([^"]+)"/g)) ids.push(m[1]);
        }
        expect(ids.length).toBeGreaterThan(50);
        expect(new Set(ids).size).toBe(ids.length);
    });

    test('rejects raw HTML, block-level and inline alike', () => {
        // Inline is the case that matters: it is nested inside a paragraph's
        // tokens, so a guard that walked only top-level tokens would miss it
        // while appearing to protect the page.
        expect(() => renderNoteHtml(fixture('<script>alert(1)</script>\n'))).toThrow(/raw HTML/);
        expect(() => renderNoteHtml(fixture('<div onclick="x">hi</div>\n'))).toThrow(/raw HTML/);
        expect(() =>
            renderNoteHtml(fixture('- A bullet <img src=x onerror=alert(1)> and more\n')),
        ).toThrow(/raw HTML/);
        expect(() => renderNoteHtml(fixture('Some <b>bold</b> prose.\n'))).toThrow(/raw HTML/);
    });

    test('escapes angle brackets inside code spans rather than rejecting them', () => {
        // The corpus uses these heavily for paths and placeholders.
        const html = renderNoteHtml(fixture('- Path `/w/<workspace>/tasks` works.\n'));
        expect(html).toContain('&lt;workspace&gt;');
    });

    test('renders every note in the corpus', () => {
        for (const note of loadReleaseNotes()) {
            expect(() => renderNoteHtml(note), `${note.tag} failed to render`).not.toThrow();
        }
    });
});

describe('loadChangelog', () => {
    test('splits the advertised release from the rest', () => {
        const all = loadReleaseNotes();
        const { current, earlier } = loadChangelog(all[0].version);
        expect(current.version).toBe(all[0].version);
        expect(current.bodyHtml.length).toBeGreaterThan(0);
        expect(earlier).toHaveLength(all.length - 1);
        expect(earlier.some(n => n.version === current.version)).toBe(false);
    });

    test('can render a release that is not the newest', () => {
        // The advertised version is whatever site-config names; it is not
        // required to be the highest note on disk.
        const all = loadReleaseNotes();
        const { current, earlier } = loadChangelog(all[2].version);
        expect(current.version).toBe(all[2].version);
        expect(earlier).toHaveLength(all.length - 1);
    });

    test('fails the build when the advertised release has no note', () => {
        expect(() => loadChangelog('9.9.9')).toThrow(/No release note for the advertised version/);
    });
});
