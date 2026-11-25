# Installation

## Requirements

- Ubuntu 26.04 **amd64**, a Wayland desktop, and a working systemd user session.
- Python 3.10+, Cargo/Rust 1.85+, and Rustup.
- At least 20 GiB for the runtime, plus room for build files.
- The running kernel's `/boot/config-$(uname -r)` and administrator access.

Install missing build dependencies only after reviewing apt's proposed changes:

```sh
sudo apt-get install --no-remove --no-install-recommends \
  build-essential pkg-config git curl xz-utils erofs-utils \
  libdrm-dev libgbm-dev libwayland-dev libxkbcommon-dev libffi-dev
```

The installer does not install these development packages automatically.
You can instead use privately extracted libraries with `--build-sysroot DIR`.
When needed, it installs Rust 1.93.0 under your user account without changing
your default Rust toolchain.

## 1. Get the source and runtime package

```sh
git clone https://github.com/mahrtayyab/droidloom-deb.git
cd droidloom-deb
```

Use **`droidloom_0.1.0-49_amd64.deb`** and its accompanying `SHA256SUMS`;
see [release details](../releases/README.md). Verify the checksum from the
directory containing both files:

```sh
sha256sum --check SHA256SUMS
```

The package is Debian-format, but the supported target is Ubuntu 26.04 amd64,
not every Debian-based distribution.

## 2. Check and install

From the source directory, inside your Wayland session:

```sh
./install.sh --check --runtime-deb "$HOME/Downloads/droidloom_0.1.0-49_amd64.deb"
./install.sh --runtime-deb "$HOME/Downloads/droidloom_0.1.0-49_amd64.deb"
```

Change the package path if needed. If Droidloom is already installed at the
matching version, omit `--runtime-deb`.

`--check` does not install packages, change configuration, or restart services;
the launcher may compile its own Rust executable and populate Cargo caches.
The actual installation shows a plan and asks for confirmation.

The installer verifies versions and hashes, builds/tests missing components as
your user, then stops only Droidloom to activate the fixes. It uses apt for a
fresh runtime installation, whose dependency changes need your approval.
Existing verified fixes are skipped; conflicting or edited overrides are refused.

| Option | Purpose |
| --- | --- |
| `--build-sysroot DIR` | Use privately extracted development libraries. |
| `--no-start` | Leave Droidloom stopped after applying fixes. |
| `--yes` | Approve the installer plan; sudo/apt may still prompt. |
| `--util-linux-archive FILE` | Use a local, checksum-verified util-linux 2.42 archive. |

Save work in Android apps first. Do not run `sudo ./install.sh`.

## 3. Verify

```sh
droidloomctl status
droidloomctl launch com.android.settings
```

Android should boot and the Settings window should have controls on GNOME Wayland.
Running `./install.sh --check` again should report the installed fixes as skipped.

## Troubleshooting

```sh
systemctl --user status droidloom.service
journalctl --user -u droidloom.service -n 100 --no-pager
journalctl -u droidloomd.service -n 100 --no-pager
```

- **Missing tools/libraries:** prepare the requirements above or use a build sysroot.
- **Hash/version mismatch:** use the supported release; do not bypass the guard.
- **Conflicting override:** inspect the named file rather than deleting other settings.
- **Interrupted installation:** review the reported changes. Activation is not an
  all-or-nothing transaction; see [rollback](fixes.md#rollback). Remove a stale
  `.work/install.lock` only after confirming its recorded process is no longer running.

Build files stay under ignored `.work/`. App data is not cleared. If Droidloom
shares your desktop folders with Android, changes from Android affect those same
host files.
