# Release preparation

## Locally staged candidate

The original, unchanged test package is staged outside Git at:

```text
dist/releases/v0.1.0-49-deb.1/droidloom_0.1.0-49_amd64.deb
```

The same directory contains `SHA256SUMS`, `PROVENANCE.json`,
`RELEASE_NOTES.md`, and `SOURCE_REQUIREMENTS.md`. The reviewable metadata
originals live in [v0.1.0-49-deb.1](v0.1.0-49-deb.1).
Binary staging is local to the preparation checkout; a fresh source clone does
not contain `dist/` or the `.deb`.

**This is a local candidate, not a published GitHub Release.** No remote or
publishing account is configured for this checkout. Complete the source/license
checklist before making binary downloads public.

The `.deb` is the **base runtime**, not a package with the later fixes integrated.
The matching source installer's `--runtime-deb` option installs it and applies
the fixes separately. See [installation](../docs/install.md).

Verify the staged file from the repository root:

```sh
cd dist/releases/v0.1.0-49-deb.1
sha256sum --check SHA256SUMS
```

Do not add the binary to Git, Git LFS, or an unrelated repository. `dist/` and
`*.deb` are ignored intentionally. Do not replace the historical package with a
rebuild carrying a different hash.

## Draft GitHub publishing procedure

These are **manual future steps**, not actions already performed. They require a
GitHub repository you control and an authenticated GitHub CLI (`gh`). Install or
use that client deliberately, then authenticate locally with `gh auth login`;
never put access tokens in this repository or paste them into a conversation.

1. Review the candidate's [source requirements](v0.1.0-49-deb.1/SOURCE_REQUIREMENTS.md)
   and arrange the necessary corresponding source/notices.
2. Review and commit the source repository. It may still have no commits:
   inspect `git status`, stage only the intended source/docs, review
   `git diff --cached`, check your Git author identity, and make the initial
   commit. Never stage `dist/`, `.work/`, runtime data, or credentials.
3. Create the intended public `OWNER/droidloom-deb` GitHub repository, configure
   `origin` if absent, and push the reviewed source branch. Do not overwrite a
   pre-existing remote, force-push, or push another project's checkout.
4. Create and push the release tag only after the source review.

From this repository's root, after substituting the actual owner:

```sh
REPOSITORY="OWNER/droidloom-deb"
TAG="v0.1.0-49-deb.1"
ASSETS="dist/releases/$TAG"
git tag -a "$TAG" -m "Experimental Ubuntu release-49 runtime and compatibility kit"
git push origin "$TAG"
gh release create "$TAG" \
  "$ASSETS/droidloom_0.1.0-49_amd64.deb" \
  "$ASSETS/SHA256SUMS" \
  "$ASSETS/PROVENANCE.json" \
  "$ASSETS/SOURCE_REQUIREMENTS.md" \
  --repo "$REPOSITORY" --verify-tag --draft --prerelease \
  --title "Droidloom-deb release 49: experimental Ubuntu runtime" \
  --notes-file "releases/$TAG/RELEASE_NOTES.md"
```

This creates a **draft**; it does not take the binary public. If the tag or
release already exists, stop and inspect it instead of deleting, forcing, or
silently replacing assets.

Before publication, attach/link all required corresponding source, complete the
checklist, and update the release notes/provenance/status. Review the final draft
and asset hashes. Only then deliberately publish the prerelease through GitHub.
The checked-in candidate metadata must not claim an upload happened when it did
not.

## Rebuild instructions

See [rootless repackaging](../packaging/repackaging.md) for the input manifests,
layout changes, limitations, and verification procedure. A payload-equivalent
rebuild is not guaranteed to reproduce the historical archive's exact checksum.
