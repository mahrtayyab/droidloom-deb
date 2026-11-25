# Repackage the `.deb`

This assembles the three prebuilt upstream release-49 packages. It does not
compile Android/Droidloom or integrate the separately installed fixes.
Run in Bash with `dpkg-deb`, tar, zstd, curl, and coreutils. No sudo is needed.

Inputs and hashes: [inputs-49.json](inputs-49.json),
[upstream-49.sha256](upstream-49.sha256). Target: Ubuntu 26.04 **amd64**.

## 1. Download and verify

From the repository root:

```bash
set -euo pipefail
umask 022
REPO="$(pwd -P)"
mkdir -p "$REPO/.work"
WORK="$(mktemp -d "$REPO/.work/repack-XXXXXX")"
STAGE="$WORK/stage"
mkdir -p "$WORK/inputs" "$STAGE"
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

Verified cached packages can replace the downloads. Do not mix versions or skip
checksums if the historical upstream assets are unavailable.

## 2. Assemble

```bash
for component in base image runtime; do
  mkdir -p "$WORK/extracted/$component"
  tar --zstd --extract --no-same-owner \
    --file "$WORK/inputs/droidloom-${component}-0.1.0-49-x86_64.pkg.tar.zst" \
    --directory "$WORK/extracted/$component"
  cp -a --reflink=auto "$WORK/extracted/$component/usr" "$STAGE/"
done
rm -- \
  "$STAGE/usr/share/libalpm/hooks/60-droidloom-remove.hook" \
  "$STAGE/usr/share/libalpm/hooks/60-droidloom-upgrade.hook" \
  "$STAGE/usr/share/libalpm/hooks/90-droidloom-installed.hook"
rmdir -- "$STAGE/usr/share/libalpm/hooks" "$STAGE/usr/share/libalpm"
mkdir -p "$STAGE/DEBIAN"
install -m 644 "$REPO/packaging/reference/control" "$STAGE/DEBIAN/control"
```

This preserves upstream binaries, images, licenses, service units, and the
`current -> runtime` symlink. Arch package metadata/hooks are omitted.
Never copy the installed host runtime, `cell.json`, or Android user data into
the staging tree.

## 3. Build and inspect

```bash
export SOURCE_DATE_EPOCH=1795184526
find "$STAGE" -exec touch --no-dereference \
  --date="@$SOURCE_DATE_EPOCH" -- {} +
DEB="$WORK/droidloom_0.1.0-49_amd64.deb"
dpkg-deb --root-owner-group --threads-max=1 -Zzstd -z3 \
  --build "$STAGE" "$DEB"
dpkg-deb --info "$DEB"
dpkg-deb --contents "$DEB"
mkdir -p "$WORK/verify"
dpkg-deb --extract "$DEB" "$WORK/verify"
sha256sum "$WORK/verify/usr/bin/droidloom-wayland"
sha256sum "$DEB"
```

The presenter's SHA256 must be
`c54fa33c4da86f1938b2395d206389e0dc32df65a334f7319aacec05195c6c08`.
Keep several GiB free; compression uses one thread.

## Verification and release

The rootless recipe was tested: all **660 payload files** matched the historical
package. Details: [validation-49.json](validation-49.json).

Equivalent payloads can produce different archive hashes because of packaging
tool versions and metadata. Do not replace the original release asset or bypass
the installer's checksum pin with a rebuild. Review/test a rebuilt package
separately before changing its release identity and installer pin.

This is a binary repack, not a complete Debian source package with validated
upgrade/removal hooks. See [release assets](../releases/README.md) and
[sources/licenses](../releases/v0.1.0-49-deb.1/SOURCES.md).
