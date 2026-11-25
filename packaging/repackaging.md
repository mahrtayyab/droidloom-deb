# Repackage the release-49 runtime as a local `.deb`

This recipe assembles the three **prebuilt upstream Arch packages** into the
same runtime payload layout as our historical Ubuntu test package. It does
**not** compile Droidloom/Android, create a policy-complete Debian source
package, or integrate the compatibility fixes into the binary.

The boot workaround, private `unshare`, and title-bar presenter are applied
separately by [`install.sh`](../docs/install.md). The existing historical `.deb`
is the [staged release asset](../releases/README.md).

## Inputs and scope

- Target: Ubuntu 26.04 x86_64/amd64 only; other distributions are unverified.
- Upstream source: `572e1c51c5caf75183efe25ad811f98c1268bf56`, release 49.
- Exact input URLs, sizes, hashes, and package build timestamp:
  [inputs-49.json](inputs-49.json).
- Input verification file: [upstream-49.sha256](upstream-49.sha256).
- Control metadata: [reference/control](reference/control).

The cached original inputs were checked against the recorded upstream release
digests. The historical `.deb` was streamed and audited: its 660 regular payload
files match the original staging tree; its sole symlink is
`usr/lib/droidloom/current -> runtime`; all data entries are under `usr/`, owned
by root, and have no setuid/setgid bits. Its control archive contains only
`control`, with no maintainer scripts.

All commands below are **rootless** and use a new ignored working directory.
They do not install a package, access Android app data, stop services, or modify
anything under the host's `/usr`, `/etc`, `/opt`, or `/var`.
Use Bash with `dpkg-deb`, GNU tar/coreutils, zstd, and curl available.
Allow several GiB of temporary disk space.

## 1. Create an isolated workspace

Run from this repository's root:

```bash
set -euo pipefail
umask 022
REPO="$(pwd -P)"
mkdir -p "$REPO/.work"
WORK="$(mktemp -d "$REPO/.work/repack-XXXXXX")"
STAGE="$WORK/stage"
mkdir -p "$WORK/inputs" "$WORK/extracted" "$STAGE"
```

Keep this shell open: subsequent commands use these variables.
Do not reuse an existing staging directory or copy an installed runtime tree.

## 2. Obtain and verify all three input packages

```bash
for component in base image runtime; do
  name="droidloom-${component}-0.1.0-49-x86_64.pkg.tar.zst"
  curl --fail --location --proto '=https' --tlsv1.2 \
    --output "$WORK/inputs/$name" \
    "https://github.com/denialwm/droidloom/releases/download/packages/$name"
done
cd "$WORK/inputs"
sha256sum --check "$REPO/packaging/upstream-49.sha256"
cd "$REPO"
```

The upstream `packages` release can change over time. If these exact historical
assets are unavailable, use cached copies of **all three verified inputs**.
Never substitute the latest package, ignore a failed checksum, or mix versions.
For cached inputs, copy them into `$WORK/inputs` instead of downloading, then run
the same checksum command before extracting.

You can inspect their package metadata without installing anything:

```bash
for component in base image runtime; do
  tar --zstd --extract --to-stdout \
    --file "$WORK/inputs/droidloom-${component}-0.1.0-49-x86_64.pkg.tar.zst" \
    .PKGINFO
done
```

All three must identify `pkgver = 0.1.0-49` and `arch = x86_64`.

## 3. Assemble the payload

```bash
for component in base image runtime; do
  mkdir -p "$WORK/extracted/$component"
  tar --zstd --extract --no-same-owner \
    --file "$WORK/inputs/droidloom-${component}-0.1.0-49-x86_64.pkg.tar.zst" \
    --directory "$WORK/extracted/$component"
  cp -a --reflink=auto "$WORK/extracted/$component/usr" "$STAGE/"
done
```

The merge preserves upstream runtime binaries, Android images, service units,
the relative `current -> runtime` link, and all Android/Mesa/Cargo license
notices. Copy only `usr/`; Arch's `.PKGINFO`, `.BUILDINFO`, and `.MTREE` are not
Debian payload files.

Remove only the three Arch package-manager hooks from this new staging tree:

```bash
rm -- \
  "$STAGE/usr/share/libalpm/hooks/60-droidloom-remove.hook" \
  "$STAGE/usr/share/libalpm/hooks/60-droidloom-upgrade.hook" \
  "$STAGE/usr/share/libalpm/hooks/90-droidloom-installed.hook"
rmdir -- "$STAGE/usr/share/libalpm/hooks" "$STAGE/usr/share/libalpm"
mkdir -p "$STAGE/DEBIAN"
install -m 644 "$REPO/packaging/reference/control" "$STAGE/DEBIAN/control"
```

Those pacman hooks do not run under dpkg. This recipe does not invent equivalent
`postinst`/`prerm` scripts; the explicit installer invokes upstream first-time
setup and controls activation. See the limitations below before treating this
as a general upgrade/removal-ready Debian package.

**Do not copy** `/etc/droidloom/cell.json`, `/var/lib/droidloom/users`, anything
from `/home`, generated kernel overrides, private build dependencies, Google
apps, or user-installed APKs into the package.

## 4. Normalize packaging metadata and build

The chosen timestamp is the verified upstream package build date. This
normalizes the new staging tree's filesystem metadata without changing file
contents or the contents of Android image files:

```bash
export SOURCE_DATE_EPOCH=1795184526
find "$STAGE" -exec touch --no-dereference \
  --date="@$SOURCE_DATE_EPOCH" -- {} +
DEB="$WORK/droidloom_0.1.0-49_amd64.deb"
dpkg-deb --root-owner-group --threads-max=1 \
  -Zzstd -z3 --build "$STAGE" "$DEB"
```

`--root-owner-group` records root ownership without sudo or chown. A single
compression thread limits build load. **No `dpkg -i`, apt, or `make install` is
run by this recipe.**

## 5. Inspect and verify the rebuilt package

```bash
dpkg-deb --info "$DEB"
dpkg-deb --contents "$DEB"
mkdir -p "$WORK/verify"
dpkg-deb --extract "$DEB" "$WORK/verify"
readlink "$WORK/verify/usr/lib/droidloom/current"
sha256sum "$WORK/verify/usr/bin/droidloom-wayland"
sha256sum "$DEB"
```

The symlink must be `runtime`; the packaged presenter's hash must be:

```text
c54fa33c4da86f1938b2395d206389e0dc32df65a334f7319aacec05195c6c08
```

Check that payload files are only under `usr/`, licenses remain present, and no
personal configuration or Android data has appeared. Compare against the pinned
inputs, not the running host installation.

### Payload equivalence is not the original archive hash

This reconstructs the historical package's payload, not necessarily its exact
archive bytes. The original build's timestamps, dpkg/tar/zstd versions, and
compression metadata were not captured as a fully reproducible toolchain.
Normalizing metadata can itself change the archive hash.

The published/staged historical package therefore retains its original SHA256:

```text
a24466c5f305883e8ca50b57d63974719a876d94fe96a3c3c2d7fec5d91766b1
```

**Do not overwrite that asset or change the installer's pin just to accept a new
checksum.** A rebuilt package needs its own reviewed provenance, install tests,
release identity, and deliberate installer-pin update before distribution.

## 6. Release preparation and remaining limitations

Follow [release preparation](../releases/README.md), including the complete
corresponding-source review. Retaining license notices is necessary but does not
by itself satisfy all binary source-distribution obligations.

This is an experimental binary repack, not a full Debian port:

- The reference control file is historical metadata, not a validated dependency
  list for every Debian-derived distribution; it lacks comprehensive ABI/version
  bounds and Debian lifecycle integration.
- The source/patch build toolchain is not a complete Debian source package.
- Fresh-machine installation, upgrades/removal, additional GPUs/desktops, and
  other distributions still need separate validation.
- Runtime images/binaries remain upstream's unmodified payload; the fixes are
  external, reversible installation steps.

Keep the working directory for inspection or remove only your own newly created
workspace after reviewing its exact path. Never clean Android storage to clean a
package-build workspace.

## Validation performed

The recipe was exercised rootlessly with the three verified cached upstream
archives. All 660 regular files in the rebuilt and re-extracted payload matched
the audited historical staging tree; the control file matched the reference.
The original release archive was not changed, and no package was installed.

The temporary verification rebuild had a different archive hash, as expected.
The tool versions, resulting hash/size, and test scope are recorded in
[validation-49.json](validation-49.json). The temporary rebuild/workspace was
removed; it is not the staged historical release asset.
