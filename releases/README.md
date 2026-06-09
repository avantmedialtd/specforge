# Release notes

One Markdown file per release, named for its tag **including the leading `v`** —
`v0.6.0.md`, `v1.0.0.md`, and so on. The name matches `github.ref_name` exactly,
so the release workflow renders the GitHub Release body straight from
`releases/${{ github.ref_name }}.md` with no path munging.

These files are authored by the **`/release`** command, not by hand: it
synthesizes proper, user-facing notes from the OpenSpec changes archived since
the last tag (plus `git log` for bare commits), shows them for approval, then
commits the file and tags that commit. Pushing the tag triggers
`.github/workflows/release.yml`, whose publish job checks out the tagged commit
and uses this file as the release body (GitHub's auto-generated notes are off).

Each file is self-contained: the curated highlights followed by a Downloads
footer that documents every platform's artifact and its install caveats — the
macOS Gatekeeper workaround for the unsigned build, and the Windows portable
build's WebView2 prerequisite — plus a Full-Changelog compare link.
