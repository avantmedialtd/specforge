# npm distribution — maintainer notes

This directory builds and publishes the npm channel for `specforge-serve`. It is
not itself published.

```
packaging.mjs        platform table, manifest shapes, dist-tag, publish order (pure; unit-tested)
build-packages.mjs   CLI: assemble six package dirs from release binaries
publish.mjs          CLI: publish them in order, idempotently
wrapper/             the published wrapper's sources (bin shim + resolution lib)
```

The published graph is one wrapper, `@avantmedia/specforge`, exposing a
`specforge-serve` bin, over five platform packages selected by npm's `os`/`cpu`
fields. Exactly one platform package is downloaded per install. Nothing here
runs a `postinstall`.

## The bootstrap: read this before the first release

**npm's trusted publishing cannot perform a package's first publish.** The
trusted-publisher configuration is stored *on a package*, and npm's own
prerequisite is that the package must already exist. There is no scope-level
publisher, no name reservation, and no pending-publisher mechanism. The npm CLI
issue asking for first-publish-over-OIDC is still open.

So each of the six names needs a **one-time manual placeholder publish** before
the pipeline can ever publish it. Skipping this does not fail early — it fails
on the first `npm publish` of the first tagged release, *after* the GitHub
Release is already public.

This is permanent, not a one-off: **adding a sixth platform later means
bootstrapping that new package name the same way.**

### Step 0 — account prerequisites (once)

1. Enable two-factor auth on the npm account. New TOTP enrolment is disabled, so
   register a **passkey / WebAuthn** (Touch ID, Windows Hello, a security key).
   `npm trust` refuses to run without account-level 2FA.
2. Confirm you are an owner or admin of the `avantmedia` org, and that your team
   has read-write access to the scope.
3. Locally:

   ```sh
   npm i -g npm@latest      # `npm trust` needs >= 11.15.0
   npm login
   npm whoami
   npm org ls avantmedia
   ```

### The short version

`npm/bootstrap.sh` does steps 1 and 2 for you. Publishing needs two-factor
auth, so run it from a terminal where you can answer the prompt:

```sh
./npm/bootstrap.sh dry          # validate, publish nothing
./npm/bootstrap.sh publish      # the six placeholders
./npm/bootstrap.sh trust        # the six trusted publishers, then verify
./npm/bootstrap.sh deprecate    # mark the placeholders unusable — do not skip
```

`deprecate` is not optional housekeeping. Because npm gives a first publish the
`latest` tag no matter what you ask for, the placeholders *are* what
`npx @avantmedia/specforge` resolves to until the first real release.

`publish` also accepts a one-time code as a second argument
(`./npm/bootstrap.sh publish 123456`) if you would rather not be prompted.
The rest of this section is what that script does and why, which is worth
reading once before running it.

### Step 1 — publish six placeholders

Platform packages first, wrapper last — the same order the real publish uses.

```sh
set -euo pipefail
PKGS="specforge-darwin-arm64 specforge-darwin-x64 specforge-linux-x64 \
specforge-linux-arm64 specforge-win32-x64 specforge"

for p in $PKGS; do
  d="$(mktemp -d)"
  cat > "$d/package.json" <<EOF
{
  "name": "@avantmedia/$p",
  "version": "0.0.0",
  "description": "Placeholder reserving this name for npm trusted publishing. Do not install.",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/avantmedialtd/specforge.git" },
  "publishConfig": { "access": "public" }
}
EOF
  echo "Placeholder. See https://github.com/avantmedialtd/specforge" > "$d/README.md"
  npm publish "$d" --access public --tag bootstrap
done
```

- `--access public` is load-bearing. Scoped packages default to `restricted`,
  and on a free org plan the publish fails outright with a payment error.
- **No `--provenance` here.** Provenance needs a cloud CI runner; it cannot be
  generated from a laptop. These six `0.0.0` publishes are the only unattested
  versions these packages will ever have.
- `--tag bootstrap` does **not** keep `latest` off the placeholder. npm assigns
  `latest` to a package's first published version whatever tag you ask for —
  verified on this project's own bootstrap, where all six ended up
  `{"bootstrap":"0.0.0","latest":"0.0.0"}`. Until the first real release,
  `npx @avantmedia/specforge` therefore resolves to an empty placeholder.
  Nothing can move `latest` while 0.0.0 is the only version, so the mitigation
  is to **deprecate immediately** (`./npm/bootstrap.sh deprecate`), which makes
  such an install warn rather than silently do nothing — and to ship the first
  real release promptly. Check with `npm dist-tag ls @avantmedia/specforge`.

### Step 2 — configure the six trusted publishers

```sh
for p in $PKGS; do
  npm trust github "@avantmedia/$p" \
    --repo avantmedialtd/specforge \
    --file release.yml \
    --allow-publish \
    --yes
  sleep 2
done

for p in $PKGS; do echo "== $p"; npm trust list "@avantmedia/$p"; done
```

Four details that each cause a failure only visible after a public release:

- **`--repo avantmedialtd/specforge`** — the *GitHub* org is `avantmedialtd`,
  the *npm* org is `avantmedia`. They differ. npm does not validate the repo
  when saving, so a typo here is silent until the publish is rejected.
- **`--file release.yml`** — a bare filename with extension, not a path.
- **`--allow-publish` is required.** Without at least one explicit allowed
  action, CI's publish is rejected and waits for a human approval.
- **Do not pass `--env`.** The `publish-npm` job declares no `environment:`, and
  a stale environment claim is the most common OIDC rejection.

The first call prompts for 2FA and offers to skip it for a few minutes — take
that and the remaining five run unattended.

### Step 3 — after the first successful release

```sh
for p in $PKGS; do npm dist-tag rm "@avantmedia/$p" bootstrap; done
for p in $PKGS; do
  npm deprecate "@avantmedia/$p@0.0.0" \
    "Placeholder for trusted-publishing setup; install the latest release."
done
```

`deprecate`, not `unpublish`. Unpublishing every version of a name locks that
name for 24 hours, which would lock this project out of its own release. Both
commands are unavailable for a few minutes after a publish while malware
scanning runs — wait and retry if they error.

Then, per package, set **Settings → Publishing access** to *"Require two-factor
authentication and disallow tokens"*. npm's own note confirms this affects only
token authentication; trusted publishers keep working. That permanently closes
the token path used for the bootstrap.

## Local checks

```sh
bun test npm/                    # packaging, shim resolution, conflict classifier

# End-to-end against real or stub binaries:
node npm/build-packages.mjs --version 0.0.0-dev --binaries <dir> --out /tmp/npm-dist
node npm/publish.mjs --dist /tmp/npm-dist --dry-run
```

`--binaries <dir>` holds one subdirectory per platform key, each containing that
platform's executable (`specforge-serve`, or `specforge-serve.exe` on win32).
The same shape is asserted in CI's `npm-packaging` job against stubs.

## Things that are deliberately the way they are

- **Linux platform packages declare no `libc` field.** The Linux binaries are
  statically linked against musl, so they run on glibc distributions too.
  Declaring `libc: ["musl"]` looks obviously correct and would exclude most
  Linux users. Asserted by a test.
- **Manifests are generated, never committed.** Six version fields that could
  drift from each other and from the tag are a bug waiting to happen.
- **The wrapper's bin is `specforge-serve`, not `specforge`.** The unscoped
  `specforge` name on the public registry belongs to an unrelated project with a
  `specforge` bin; renaming ours would collide on `PATH`.
- **The wrapper is published last.** An npm publish cannot be retracted, so a
  partial failure must leave unreferenced orphans, never a wrapper pinning
  versions that do not exist.
