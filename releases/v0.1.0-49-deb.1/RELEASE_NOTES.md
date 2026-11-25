# Droidloom-deb: experimental Ubuntu release-49 runtime

**Candidate tag:** `v0.1.0-49-deb.1`

**Status:** staged locally; not yet published to GitHub.

Unofficial Ubuntu 26.04 amd64 runtime repack for Droidloom upstream release 49,
source commit `572e1c51c5caf75183efe25ad811f98c1268bf56`.
This is not an official Droidloom Debian package or a generally supported
Debian/Ubuntu release.

## Asset

- `droidloom_0.1.0-49_amd64.deb`: 1,025,174,170 bytes.
- SHA256: `a24466c5f305883e8ca50b57d63974719a876d94fe96a3c3c2d7fec5d91766b1`.
- Verify it against `SHA256SUMS` before installation.

**The `.deb` is the original base runtime. It does not contain the later boot,
private-unshare, or window-control fixes.** The source repository's `install.sh`
applies those separately and keeps the packaged presenter and Android images
unchanged.

## Install

Read `docs/install.md` in the matching source checkout and prepare the documented
prerequisites. Put the downloaded `.deb` and checksum file together:

```sh
sha256sum --check SHA256SUMS
```

Then, from the reviewed source repository inside your normal Wayland session:

```sh
./install.sh --check --runtime-deb "/absolute/path/to/droidloom_0.1.0-49_amd64.deb"
./install.sh --runtime-deb "/absolute/path/to/droidloom_0.1.0-49_amd64.deb"
```

Use `--build-sysroot` if using private development packages. Do not run the
installer with sudo; it requests scoped administrator approval when needed.
It refuses a mismatched runtime, preserves app data, and skips verified existing
fixes.

## Compatibility and limitations

The original working environment was Ubuntu 26.04.1 LTS x86_64, GNOME Wayland,
and Intel graphics. Fresh-machine installation and upgrades/removal have not
been validated end-to-end. ARM64 application translation remains experimental;
this release does not fix the observed Twitter failure or PUBG startup stall.
There is no signed apt repository or automatic-update channel.

See `PROVENANCE.json`, `packaging/repackaging.md`, and `docs/compatibility.md`.
Review and complete `SOURCE_REQUIREMENTS.md` before public binary publication.
