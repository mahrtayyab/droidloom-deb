# Debian packaging status

**An experimental binary-repack recipe is documented; a complete Debian port and
source-build package are not implemented.**

The original test installation used a locally repackaged `droidloom`
`0.1.0-49` amd64 package. Its historical control file is retained under
`reference/control` for reference only. It is not a complete Debian source
package or a supported dependency list for every Debian-derived distribution.
The [rootless repackaging guide](repackaging.md) now documents assembly from the
three pinned upstream binary packages, with input checksums and validation.

No runtime `.deb`, Android images, private build dependencies, or installer
maintainer scripts are committed to Git. The historical `.deb` is staged as a
[local release candidate](../releases/README.md), not yet uploaded to GitHub.
The source fixes can be reviewed and tested independently using `docs/`.

The root `install.sh` automates compatibility-fix installation, not runtime
package production. For a fresh setup it can install the exact separately
supplied local `.deb` pinned by SHA256 in `upstream.json`, using apt's normal
confirmation and `--no-remove`. It does not publish/download that package,
upgrade an installed runtime, or build the runtime itself. The repackaging guide
does not claim bit-for-bit reproduction of the historical package.

Before publishing installable packages, the project still needs:

1. A pinned, documented source/build process for the host runtime and its Android
   image inputs, including verification of downloaded inputs.
2. Proper Debian source metadata, distribution-specific dependencies, package
   ownership, and idempotent install/upgrade/remove hooks.
3. Isolated build dependencies rather than replacement host libraries or
   kernel/security changes.
4. Clean build and install/upgrade/rollback/removal tests on each claimed target
   distribution, preserving Android data and unrelated configuration.
5. Runtime/protocol version checks for the custom presenter, plus explicit
   opt-in application and removal of compatibility overrides.
6. Full license review, dependency notices, and complete corresponding source
   for distributed binaries.

The current GitHub workflow checks the helper, installer, and source patch.
It does not claim to build a complete runtime or automatically publish a release.
