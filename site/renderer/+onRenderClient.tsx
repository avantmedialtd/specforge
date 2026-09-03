import { PageDataContext } from '../site-kit/PageData';
import type { ComponentType } from 'react';
import ReactDOM from 'react-dom/client';
import type { OnRenderClientAsync } from 'vike/types';
import { Layout } from '../src/Layout';
import '../src/styles.css';

/**
 * Hydrates the same `<Layout>`-wrapped tree the server rendered into `#root`,
 * so chrome and page hydrate together (full-page hydration, no client
 * boundaries) — the same model as the studio apps, against this app's own
 * Layout rather than the shared brutalist one.
 */
export const onRenderClient: OnRenderClientAsync = async pageContext => {
    const Page = pageContext.Page as ComponentType;
    const root = document.getElementById('root');
    if (!root) throw new Error('#root element not found');

    const currentPath =
        typeof window !== 'undefined' ? window.location.pathname : (pageContext.urlPathname ?? '/');

    ReactDOM.hydrateRoot(
        root,
        <Layout currentPath={currentPath}>
            <PageDataContext.Provider value={pageContext.data}>
                <Page />
            </PageDataContext.Provider>
        </Layout>,
    );
};
