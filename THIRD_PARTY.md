# Licensing and attribution

This is an independent compatibility project for
[Droidloom](https://github.com/denialwm/droidloom), not an official release.

## Droidloom source patch

`patches/0001-wayland-client-side-decorations.patch` modifies upstream Droidloom
release 49, commit `572e1c51c5caf75183efe25ad811f98c1268bf56`, and includes the
new `decorations.rs` source and the locked Cargo dependency changes.
Droidloom's original implementation is copyright 2026 Doctor Logix and is
licensed under GPL-3.0-or-later. The patch and this project's original helper,
tests, examples, and documentation use the same license.

The unmodified upstream license notice is retained as
`LICENSES/Droidloom-NOTICE.txt`, its scope document as
`LICENSES/Droidloom-THIRD-PARTY.md`, and the full GPL text as
`LICENSES/GPL-3.0-or-later.txt`.

The optional private namespace tool is built locally from util-linux 2.42
source, verified against the pinned upstream archive hash. `unshare` is
GPL-2.0-or-later; its source and util-linux's existing component license notices
remain in the ignored source/build tree. No util-linux binary or source archive
is committed here.

## Dependencies and binaries

The patch adds a dependency on `sctk-adwaita` 0.11.0. It and all other upstream
Cargo dependencies retain their own licenses. This repository does not vendor
their source in Git. No compiled runtime binary, Android image, Google
application, or user-installed Android application is committed to this source
repository.

The Rust installer uses `serde_json` and its locked transitive dependencies,
which retain their published licenses. They are downloaded by Cargo, not
vendored in this repository.

Before distributing binaries, review the licenses of all bundled components
and provide their complete corresponding source where required. A patch file
alone is not the complete corresponding source for a packaged runtime.
Consult the retained upstream scope document for additional obligations.

The historical runtime `.deb` is staged locally outside Git as a release
candidate. Its upstream license inventories remain in the payload. Public
binary-release preparation is documented in `releases/README.md`; complete
corresponding-source availability and component notices must be reviewed before
that candidate is published. A checksum/payload audit is not a completed license
or corresponding-source review.
