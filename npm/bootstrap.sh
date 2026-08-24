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
#   ./npm/bootstrap.sh deprecate           mark the 0.0.0 placeholders unusable
#   ./npm/bootstrap.sh retire              drop the bootstrap dist-tag, once a
#                                          real release holds `latest`
#
# publish, deprecate and retire all write to the registry and so all need 2FA.
# Each takes an optional one-time code as its second argument
# (e.g. `./npm/bootstrap.sh retire 123456`) for use where the interactive
# browser flow cannot complete — a non-interactive shell, or CI.
#
# Run `deprecate` immediately after `publish`. npm assigns `latest` to a
# package's first published version REGARDLESS of `--tag`, so until the first
# real release these placeholders are what `npx @avantmedia/specforge` resolves
# to. Deprecating them is the only available mitigation: it does not move
# `latest`, but it makes any install of one print a warning saying so.
#
# Publishing requires two-factor auth, so `publish` must be run from a terminal
# where you can answer the prompt — or with a one-time code as the second
# argument, or with a granular token that has 2FA bypass enabled.
set -euo pipefail

MODE="${1:-dry}"
OTP="${2:-}"

# Every mode that writes to the registry — publish, deprecate, retire — needs
# two-factor auth. Passing a one-time code as the second argument is what makes
# them runnable somewhere the interactive browser flow cannot complete.
#
# Built once here and expanded as ${OTP_ARGS[@]+"${OTP_ARGS[@]}"} at each use:
# macOS ships bash 3.2, where expanding an EMPTY array under `set -u` is an
# unbound-variable error, and no-OTP is the common case.
OTP_ARGS=()
if [ -n "$OTP" ]; then OTP_ARGS+=(--otp "$OTP"); fi

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
      echo "Then: ./npm/bootstrap.sh deprecate  (these placeholders now hold 'latest')"
    fi
    ;;

  deprecate)
    # npm gives a package's first version the `latest` tag whatever `--tag`
    # says, so these placeholders are currently what an unversioned install
    # resolves to. Deprecation does not move `latest` — nothing can, until a
    # real version exists — but it makes any such install print a warning.
    echo "=== deprecating the six 0.0.0 placeholders ==="
    for p in $PKGS; do
      echo "--- @avantmedia/$p ---"
      npm deprecate "@avantmedia/$p@0.0.0" \
        "Placeholder reserving this name for npm trusted publishing. Not a usable release — install a published version instead." \
        ${OTP_ARGS[@]+"${OTP_ARGS[@]}"}
    done
    echo "=== deprecate complete ==="
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

  retire)
    # Run once a real release has moved `latest` off the placeholder. The
    # bootstrap dist-tag has no purpose after that; it only leaves a tag on the
    # package page pointing at a deprecated empty version.
    #
    # `latest` is never removed — npm does not allow it, and it now points at a
    # real release. The 0.0.0 versions stay published and deprecated: see
    # npm/README.md for why unpublishing a name is the one thing not to do.
    echo "=== dropping the bootstrap dist-tag ==="
    for p in $PKGS; do
      echo "--- @avantmedia/$p ---"
      npm dist-tag rm "@avantmedia/$p" bootstrap ${OTP_ARGS[@]+"${OTP_ARGS[@]}"}
    done
    echo "=== retire complete ==="
    ;;

  *)
    echo "usage: bootstrap.sh [dry|publish [otp]|trust|deprecate|retire]" >&2
    exit 1
    ;;
esac
