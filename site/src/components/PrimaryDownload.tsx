import { useEffect, useState } from 'react';
import {
    LATEST_RELEASE_URL,
    LEAD_DOWNLOADS,
    PLATFORM_NAMES,
    RELEASE_VERSION,
    assetUrl,
    type Platform,
} from '../site-config';
import { detectPlatform } from './detectPlatform';

/**
 * The landing page's primary action: a real download, not an anchor.
 *
 * The server renders a platform-neutral control pointing at the releases page,
 * so the button works with scripting unavailable and React hydrates without a
 * mismatch. Detection runs in an effect — after hydration — and upgrades the
 * label and target to the visitor's own bundle.
 */
export function PrimaryDownload() {
    const [platform, setPlatform] = useState<Platform | null>(null);

    useEffect(() => {
        setPlatform(detectPlatform());
    }, []);

    const lead = platform ? LEAD_DOWNLOADS[platform] : null;

    return (
        <div className="download-primary">
            <a
                className="btn-download"
                href={lead ? assetUrl(lead.file) : LATEST_RELEASE_URL}
                data-platform={platform ?? 'unknown'}
            >
                <span className="btn-download-label">
                    {platform ? `Download for ${PLATFORM_NAMES[platform]}` : 'Download SpecForge'}
                </span>
                <span className="btn-download-detail">
                    {lead ? lead.detail : 'macOS · Windows · Linux'}
                </span>
            </a>
            <p className="download-primary-meta">
                Version {RELEASE_VERSION} ·{' '}
                <a className="text-link" href="#downloads">
                    every platform and binary
                </a>
            </p>
        </div>
    );
}
