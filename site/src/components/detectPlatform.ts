import type { Platform } from '../site-config';

interface UserAgentData {
    platform?: string;
}

/**
 * Resolve the visitor's operating system from what the browser already holds.
 *
 * Reads `navigator` only — no request, no cookie, no storage — so the site's
 * "sets nothing, calls nobody" guarantee is untouched. `cookies.spec.ts` is the
 * guard that keeps it that way.
 *
 * Returns `null` whenever the platform is not one of the three the desktop app
 * ships bundles for, so the caller keeps its neutral control rather than
 * guessing. Being wrong here costs a visitor a wasted download, so every
 * ambiguous case resolves to `null`.
 */
export function detectPlatform(): Platform | null {
    if (typeof navigator === 'undefined') return null;

    const uaData = (navigator as Navigator & { userAgentData?: UserAgentData }).userAgentData;
    const haystack = `${uaData?.platform ?? ''} ${navigator.userAgent ?? ''}`.toLowerCase();

    // Phones and tablets first: Android reports "linux" and iOS reports
    // "macintosh"-adjacent strings, and neither can run a desktop bundle.
    if (/android|iphone|ipod|ipad/.test(haystack)) return null;

    // iPadOS Safari reports a desktop Macintosh user agent with no "ipad" in it.
    // A touch-capable "Macintosh" is the tell.
    if (/macintosh/.test(haystack) && navigator.maxTouchPoints > 1) return null;

    // Checked before Windows: "darwin" contains "win".
    if (/macintosh|mac os x|macos|darwin/.test(haystack)) return 'macos';
    if (/windows|win32|win64/.test(haystack)) return 'windows';
    if (/linux|x11|cros/.test(haystack)) return 'linux';

    return null;
}
