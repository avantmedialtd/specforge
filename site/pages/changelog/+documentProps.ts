import type { SiteDocumentProps } from '../../site-kit/postMeta';

/**
 * `modified` is authored, never derived — the discovery plugin fails the build
 * on a page that carries neither `modified` nor `date`, and rejects a date in
 * the future compared in UTC.
 *
 * Because this page's content changes with every release, `/release` rewrites
 * this date in the release commit alongside `RELEASE_VERSION` and the notes
 * file. Left alone it would misreport the page's freshness in `sitemap.xml`
 * from the next release onward.
 */
const documentProps: SiteDocumentProps = {
    title: 'Changelog',
    description:
        'What shipped in each SpecForge release. The current release in full, and every earlier release with the line that summarises it.',
    path: '/changelog',
    modified: '2026-09-04',
};

export default documentProps;
