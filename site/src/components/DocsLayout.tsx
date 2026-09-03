import type { ReactNode } from 'react';
import { DOCS_NAV, SITE_URL } from '../site-config';
import { JsonLd } from './JsonLd';

/**
 * SpecForge ▸ Documentation ▸ this page, as structured data. Built here rather
 * than per page so a new docs page inherits it by using this layout, the same
 * way it inherits the sidebar.
 */
function breadcrumbFor(title: string, currentPath: string) {
    const trail = [
        { name: 'SpecForge', item: SITE_URL },
        { name: 'Documentation', item: `${SITE_URL}/docs` },
    ];
    // `/docs` is already the second crumb; only deeper pages add themselves.
    if (currentPath !== '/docs') {
        trail.push({ name: title, item: `${SITE_URL}${currentPath}` });
    }
    return {
        '@context': 'https://schema.org',
        '@type': 'BreadcrumbList',
        itemListElement: trail.map((crumb, i) => ({
            '@type': 'ListItem',
            position: i + 1,
            name: crumb.name,
            item: crumb.item,
        })),
    };
}

/**
 * Two-column documentation shell: a table of contents that marks the current
 * page, and a prose column. The nav is driven by `DOCS_NAV`, the same array the
 * footer uses, so a new docs page appears in both by adding one entry.
 */
export function DocsLayout({
    title,
    intro,
    currentPath,
    children,
}: {
    title: string;
    intro: ReactNode;
    currentPath: string;
    children: ReactNode;
}) {
    return (
        <>
            <JsonLd data={breadcrumbFor(title, currentPath)} />
            <div className="site-shell grid gap-10 py-12 md:grid-cols-[200px_1fr]">
                <nav
                    aria-label="Documentation"
                    className="text-sm md:sticky md:top-20 md:self-start"
                >
                    <p className="mb-3 font-semibold tracking-tight">Documentation</p>
                    <ul className="m-0 list-none space-y-2 p-0">
                        {DOCS_NAV.map(item => {
                            const active = item.href === currentPath;
                            return (
                                <li key={item.href}>
                                    <a
                                        href={item.href}
                                        aria-current={active ? 'page' : undefined}
                                        className={
                                            active
                                                ? 'text-[var(--accent)] no-underline'
                                                : 'text-[var(--text-muted)] no-underline hover:text-[var(--text)]'
                                        }
                                    >
                                        {item.label}
                                    </a>
                                </li>
                            );
                        })}
                    </ul>
                </nav>

                <article className="prose-docs min-w-0">
                    <h1 className="mt-0 mb-4 text-3xl font-semibold tracking-tight">{title}</h1>
                    <p className="mb-8 text-lg text-[var(--text-muted)]">{intro}</p>
                    {children}
                </article>
            </div>
        </>
    );
}

/** A documentation section heading, with a stable id for deep links. */
export function DocsSection({
    id,
    heading,
    children,
}: {
    id: string;
    heading: string;
    children: ReactNode;
}) {
    return (
        <section className="mb-10">
            <h2 id={id} className="mb-3 text-xl font-semibold tracking-tight">
                {heading}
            </h2>
            {children}
        </section>
    );
}

/** A callout for the caveats that would otherwise be missed in running prose. */
export function Note({ children }: { children: ReactNode }) {
    return (
        <div className="my-5 rounded-md border border-[var(--border-strong)] bg-[var(--surface-2)] px-4 py-3 text-sm">
            {children}
        </div>
    );
}
