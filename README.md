# droidloom-deb

Unofficial Ubuntu/Debian compatibility work for
[Droidloom](https://github.com/denialwm/droidloom).

**Status: source-based compatibility kit with an opt-in installer.**
This repository contains the fixes tested locally on Ubuntu 26.04.1 LTS
(x86_64), not a complete Debian port. The installer reuses the matching runtime
or accepts the exact reviewed local runtime `.deb` for a fresh setup; that
package is staged locally as a [release candidate](releases/README.md), not yet
published or automatically downloaded. A rootless binary-repack recipe is
documented; a complete Debian port/source-build package is still pending.
See [packaging](packaging/README.md).
Nothing is installed or enabled merely by cloning this repository.

## Included fixes

- **Android boot:** an isolated, reversible workaround for the `system_server`
  abort `Kernel configs could not be fetched. b/151092221` on kernels without
  `/proc/config.gz`. It exposes the real running kernel's configuration to
  Android without changing the host kernel or original Android image.
- **GNOME Wayland window controls:** a source patch adding client-side close,
  minimize, maximize/restore, dragging, and resizing when the compositor does
  not supply decorations. The original packaged presenter stays untouched.
- **Safety checks and tests:** the kernel-config helper refuses conflicting
  overrides and edited generated files; the example presenter override checks
  the packaged presenter's hash before starting.
- **Namespace utility:** when needed, build a private util-linux 2.42 `unshare`
  selected only by Droidloom's service PATH, keeping Ubuntu's binary unchanged.

The title-bar patch is pinned to upstream release 49, commit
`572e1c51c5caf75183efe25ad811f98c1268bf56`. See [upstream.json](upstream.json).
Do not apply it blindly to other releases.

## Start here

- [Step-by-step installation and one-command installer](docs/install.md)
- [Kernel-config workaround: check, apply, and rollback](docs/kernel-config.md)
- [Title-bar patch: build, enable, and rollback](docs/titlebar.md)
- [Private namespace utility: operation and rollback](docs/unshare.md)
- [Tested environment and limitations](docs/compatibility.md)
- [Debian packaging status](packaging/README.md)
- [Repackage the pinned upstream packages as a `.deb`](packaging/repackaging.md)
- [Local release asset and draft publishing instructions](releases/README.md)
- [Licensing and attribution](THIRD_PARTY.md)

From your ordinary Wayland desktop user's terminal, after reviewing the
[prerequisites](docs/install.md#2-check-the-supported-environment):

```sh
./install.sh --check
./install.sh
```

Do not run the installer with sudo. It requests narrowly scoped administrator
access only when needed. Missing runtime packages/build prerequisites must be
provided explicitly; see the guide. It skips verified existing fixes and
refuses conflicts rather than overwriting them.

Run the tests without administrator access:

```sh
python3 -m unittest -v test_ubuntu_kernel_config.py test_release_metadata.py
cargo test --locked -j 1 --manifest-path tools/installer/Cargo.toml
```

The tests use temporary files and mock privileged installation and systemd
operations. They do not stop Droidloom or change its configuration.
Release tests also check manifest/checksum consistency and, when present,
read the local staged binary to verify its hash; CI does not download it.
The GitHub Actions workflow also checks the installer and the patch against
the pinned upstream source; it does not publish packages or install these fixes.

## Safety and scope

Review the instructions before enabling anything. Applying the boot workaround
needs administrator approval and a stopped Droidloom runtime. Enabling the
custom presenter is a separate, per-user operation.

These fixes do not require replacing host libraries, altering the host kernel,
changing GNOME settings, disabling security policies, or deleting Android data.
They do not guarantee that ARM64 applications or games work under translation.
Do not commit Android data, APKs, images, credentials, generated binaries, or
machine-specific `cell.json` files.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE). Droidloom's original code is copyright
2026 Doctor Logix. Its notices and the full GPL text are retained under
[LICENSES](LICENSES). This project is not an official Droidloom release.
