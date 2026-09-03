#!/usr/bin/env bash
# Post-deploy smoke check for the SpecForge site.
#
# Every URL is taken from the deployed sitemap rather than a list kept here. The
# hand-maintained version of this check drifted twice — /docs/dashboard and
# /docs/web-ui both launched without being added — so for a while a deploy that
# dropped either would still have reported green. Reading the sitemap makes the
# check self-maintaining: a page that is live is asserted, and a page that
# vanished from the sitemap is caught by the count floor below.
set -euo pipefail

DOMAIN="${1:-specforge.avantmedia.uk}"
BASE_URL="https://${DOMAIN}"

failures=0

check() {
    local url="$1" expected="$2" label="$3" actual
    actual=$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "$url")
    if [ "$actual" = "$expected" ]; then
        printf '  ok    %-52s %s\n' "$label" "$actual"
    else
        printf '  FAIL  %-52s expected %s, got %s\n' "$label" "$expected" "$actual"
        failures=$((failures + 1))
    fi
}

echo "Post-deploy verification: ${BASE_URL}"

sitemap=$(curl -sf --max-time 20 "${BASE_URL}/sitemap.xml") || {
    echo "FAIL: could not fetch ${BASE_URL}/sitemap.xml"
    exit 1
}

# Read with a while loop rather than `mapfile`, which is bash 4+ and so absent on
# a stock macOS shell.
urls=()
while IFS= read -r loc; do
    [ -n "$loc" ] && urls+=("$loc")
# Two explicit expressions rather than `</\?loc>`: the `\?` operator is a GNU
# extension and silently matches nothing under BSD sed, which would leave the
# tags attached and hand curl a malformed URL.
done < <(printf '%s' "$sitemap" | grep -o '<loc>[^<]*</loc>' | sed -e 's|<loc>||' -e 's|</loc>||')

# The site has nine routes. A sitemap that suddenly lists two would otherwise
# pass this check trivially.
if [ "${#urls[@]}" -lt 9 ]; then
    echo "FAIL: sitemap lists ${#urls[@]} URLs, expected at least 9"
    exit 1
fi

echo "Routes (${#urls[@]} from sitemap):"
for url in "${urls[@]}"; do
    check "$url" 200 "${url#"$BASE_URL"}"
done

echo "Assets:"
check "${BASE_URL}/robots.txt" 200 "/robots.txt"
check "${BASE_URL}/specforge-icon.svg" 200 "/specforge-icon.svg"
check "${BASE_URL}/og-specforge.png" 200 "/og-specforge.png"

echo "Negative guards:"
# These exist on the studio site. A 200 here means the wrong app was synced into
# this bucket.
check "${BASE_URL}/services" 404 "/services must not exist"
check "${BASE_URL}/portfolio" 404 "/portfolio must not exist"
# This site publishes no feed — it has no dated articles.
check "${BASE_URL}/feed.xml" 404 "/feed.xml must not exist"

if [ "$failures" -gt 0 ]; then
    echo "Post-deploy verification FAILED: ${failures} check(s)."
    exit 1
fi

echo "Post-deploy verification passed."
