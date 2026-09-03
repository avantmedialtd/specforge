#!/usr/bin/env node
// British-English guard for the site's customer-facing copy.
//
// Fails the build if American `-ize`/`-ization`/`-yze` forms drift back into the
// rendered prose. The site declares `en-GB`, and this check exists because
// `id="personalization"` once shipped on it.
//
// The schema.org `@type` literal `Organization` (an API identifier, not prose)
// and code identifiers derived from it are intentionally NOT in the banned set,
// so they pass unflagged.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(fileURLToPath(new URL('.', import.meta.url)), '..');
// `src` is covered as well as `pages`: the header, footer and docs shell all
// carry rendered prose, and a banned form there ships just as visibly.
const COPY_DIRS = ['pages', 'src'].map(d => join(ROOT, d));

// Case-insensitive banned American forms. Word-bounded so substrings of larger
// allowed words are not matched. `organiz*` is deliberately excluded to avoid
// false positives on the schema.org `Organization` type and its identifiers.
const BANNED = [
    /\bvisualiz\w*\b/i,
    /\bspecializ\w*\b/i,
    /\boptimiz\w*\b/i,
    /\brecogniz\w*\b/i,
    /\bprioritiz\w*\b/i,
    /\brevolutioniz\w*\b/i,
    /\banalyz\w*\b/i,
    /\bstandardiz\w*\b/i,
    /\bcustomiz\w*\b/i,
    /\brealiz\w*\b/i,
    /\bpersonaliz\w*\b/i,
    /\bdefense\b/i,
];

function walk(dir) {
    const out = [];
    for (const name of readdirSync(dir)) {
        const full = join(dir, name);
        if (statSync(full).isDirectory()) {
            out.push(...walk(full));
        } else if (/\.(ts|tsx)$/.test(name)) {
            out.push(full);
        }
    }
    return out;
}

const violations = [];
for (const file of COPY_DIRS.flatMap(walk)) {
    const lines = readFileSync(file, 'utf8').split('\n');
    lines.forEach((line, i) => {
        for (const rx of BANNED) {
            const m = line.match(rx);
            if (m) {
                violations.push(`${file.slice(ROOT.length + 1)}:${i + 1}  ${m[0]}`);
                break;
            }
        }
    });
}

if (violations.length > 0) {
    console.error('British-English check failed — American spellings found in en-GB copy:');
    for (const v of violations) console.error('  ' + v);
    console.error(
        '\nUse the British -ise/-isation/-yse forms (e.g. "optimise", "visualisation", "analyse").',
    );
    process.exit(1);
}

console.log('British-English check passed: no American -ize/-ization/-yse forms in site copy.');
