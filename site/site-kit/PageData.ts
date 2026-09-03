import { createContext, useContext } from 'react';

/**
 * Carries the value returned by a page's Vike `+data` hook to its component,
 * provided by the renderer around `<Page />`. Every page on this site is static
 * and none defines a `+data` hook, so the provided value is always `undefined`.
 * The plumbing is kept so a data-driven route could be added without reworking
 * the renderer.
 */
export const PageDataContext = createContext<unknown>(undefined);

/** Read the current page's `+data` result. */
export function usePageData<T>(): T {
    return useContext(PageDataContext) as T;
}
