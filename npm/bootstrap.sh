#!/usr/bin/env bash
# One-time bootstrap for the npm channel.
#
# npm's trusted publishing cannot perform a package's FIRST publish — the
# trusted-publisher configuration is stored on a package that must already
# exist. So each name is claimed here by a 0.0.0 placeholder, after which
# `npm trust` can be configured and every later publish is automated.
#
# This is not a one-off: adding a new platform package later means bootstrapping
# that new name the same way. See npm/README.md for the full runbook.
#
#   ./npm/bootstrap.sh dry                 validate, publish nothing
#   ./npm/bootstrap.sh publish             real publishes (will prompt for 2FA)
#   ./npm/bootstrap.sh publish 123456      real publishes, passing a one-time code
#   ./npm/bootstrap.sh trust               configure the six trusted publishers
#
# Publishing requires two-factor auth, so `publish` must be run from a terminal
# where you can answer the prompt — or with a one-time code as the second
# argument, or with a granular token that has 2FA bypass enabled.
set -euo pipefail

MODE="${1:-dry}"
OTP="${2:-}"

# Platform packages first, wrapper last — the same order the real publish uses,
# so a partial run never leaves a wrapper pinning versions that do not exist.
PKGS="specforge-darwin-arm64 specforge-darwin-x64 specforge-linux-x64 specforge-linux-arm64 specforge-win32-x64 specforge"

GITHUB_REPO="avantmedialtd/specforge"   # NB: the GitHub org differs from the npm org
WORKFLOW_FILE="release.yml"

stage_placeholders() {
  STAGE="$(mktemp -d)"
  for p in $PKGS; do
    mkdir -p "$STAGE/$p"
    cat > "$STAGE/$p/package.json" <<EOF
{
  "name": "@avantmedia/$p",
  "version": "0.0.0",
  "description": "Placeholder reserving this name for npm trusted publishing. Do not install.",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/${GITHUB_REPO}.git" },
  "homepage": "https://github.com/${GITHUB_REPO}#readme",
  "publishConfig": { "access": "public" }
}
EOF
    echo "Placeholder reserving this name. See https://github.com/${GITHUB_REPO}" \
      > "$STAGE/$p/README.md"
  done
}

case "$MODE" in
  dry|publish)
    stage_placeholders
    EXTRA=()
    if [ "$MODE" = "dry" ]; then EXTRA+=(--dry-run); fi
    if [ -n "$OTP" ]; then EXTRA+=(--otp "$OTP"); fi

    echo "=== ${MODE}: 6 placeholder publishes ==="
    for p in $PKGS; do
      echo "--- @avantmedia/$p ---"
      # No --provenance: it needs a cloud CI runner and cannot be generated here.
      # These six 0.0.0 versions are the only unattested ones these packages
      # will ever have.
      #
      # `${EXTRA[@]+"${EXTRA[@]}"}` rather than a plain `"${EXTRA[@]}"`: macOS
      # ships bash 3.2, where expanding an EMPTY array under `set -u` is an
      # unbound-variable error. `publish` with no one-time code is exactly that
      # case, so the plain form fails on the default shell of the machine this
      # script is most likely to be run from.
      npm publish "$STAGE/$p" --access public --tag bootstrap ${EXTRA[@]+"${EXTRA[@]}"}
    done
    echo "=== ${MODE} complete ==="
    # An `if` rather than `[ … ] && …`: as the final command under `set -e`, a
    # false test would exit the script non-zero on a successful dry run.
    if [ "$MODE" = "publish" ]; then
      echo "Next: ./npm/bootstrap.sh trust"
    fi
    ;;

  trust)
    echo "=== configuring 6 trusted publishers ==="
    for p in $PKGS; do
      echo "--- @avantmedia/$p ---"
      # --allow-publish is required: without an explicit allowed action the
      # CI publish is rejected and waits for a human approval.
      # --env is deliberately NOT passed: the publish-npm job declares no
      # environment, and a stale environment claim is the most common rejection.
      npm trust github "@avantmedia/$p" \
        --repo "$GITHUB_REPO" \
        --file "$WORKFLOW_FILE" \
        --allow-publish \
        --yes
      sleep 2
    done
    echo "=== verifying ==="
    for p in $PKGS; do
      echo "--- @avantmedia/$p ---"
      npm trust list "@avantmedia/$p"
    done
    ;;

  *)
    echo "usage: bootstrap.sh [dry|publish [otp]|trust]" >&2
    exit 1
    ;;
esac
