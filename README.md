# droidloom-deb

Unofficial Debian-format packaging and Ubuntu fixes for Droidloom.

**Original repository:** [denialwm/droidloom](https://github.com/denialwm/droidloom).

**Target:** Ubuntu 26.04, `amd64` (x86_64), Wayland. Tested on GNOME with Intel
graphics. Other distributions/architectures and fresh-machine installs are not
yet validated.

## Install

Use **`droidloom_0.1.0-49_amd64.deb`** and its accompanying checksum.
See [release details](releases/README.md).
The `.deb` contains the base runtime; `install.sh` applies the fixes separately.

After preparing the [prerequisites](docs/install.md#requirements), run from this
repository as your normal desktop user:

```sh
./install.sh --check --runtime-deb "$HOME/Downloads/droidloom_0.1.0-49_amd64.deb"
./install.sh --runtime-deb "$HOME/Downloads/droidloom_0.1.0-49_amd64.deb"
```

If the matching runtime is already installed, use `./install.sh` without
`--runtime-deb`. Do not run the script with sudo; it requests approval when needed.

## What it fixes

- Binderfs startup failures after reboot, with a stock-module precheck.
- Android's missing-kernel-config boot crash.
- Missing GNOME Wayland title bars and window controls.
- Namespace-tool compatibility, using a private `unshare` when required.

Verified existing fixes are skipped. Original Android images, the packaged
presenter, and Android app data are preserved. ARM app compatibility remains
experimental; these fixes do not make every Android app work.

## Documentation

- [Installation and troubleshooting](docs/install.md)
- [How the fixes work and how to undo them](docs/fixes.md)
- [Repackage the `.deb`](packaging/README.md)
- [Release assets](releases/README.md)

## Tests

```sh
python3 -m unittest -v test_ubuntu_kernel_config.py test_release_metadata.py
cargo test --locked -j 1 --manifest-path tools/installer/Cargo.toml
```

GPL-3.0-or-later. See [LICENSE](LICENSE) and [third-party notices](THIRD_PARTY.md).
