# Step-by-step installation

This guide joins the existing patch guides into one installation procedure.
The implementation details are already documented in
[kernel-config.md](kernel-config.md) and [titlebar.md](titlebar.md); they are not
duplicated here. The previously undocumented namespace-utility prerequisite is
covered in [unshare.md](unshare.md).

## 1. Understand what the installer can install

`install.sh` applies the three compatibility changes together:

1. A Droidloom-private util-linux 2.42 `unshare`, **only if** the selected utility
   is older. The system `/usr/bin/unshare` is not replaced.
2. The isolated Android kernel-config workaround.
3. The pinned, separately built, per-user title-bar presenter.

It reuses a matching installed runtime and skips verified, already-installed
fixes without rebuilding or restarting Droidloom.

For a **fresh runtime installation**, it also accepts a separately obtained,
trusted local `droidloom_0.1.0-49_amd64.deb`. It does **not** download or build that
runtime package. The exact supported package has SHA256:

```text
a24466c5f305883e8ca50b57d63974719a876d94fe96a3c3c2d7fec5d91766b1
```

This is the package from the original local test, not an official public Debian
release. Having the source repository alone is not enough for a fresh install
until a reviewed runtime package is available. A different package with the
same filename/version is refused. See [packaging status](../packaging/README.md).

For this checkout, the historical package is staged locally under
`dist/releases/v0.1.0-49-deb.1/`, together with its checksum and release notes.
See [release preparation](../releases/README.md). It has not been uploaded to
GitHub. To assemble the same payload from the pinned upstream binary packages,
follow [repackaging](../packaging/repackaging.md); a rebuilt archive can have a
different checksum and is not automatically accepted by the historical pin.

## 2. Check the supported environment

The automated path currently requires:

- **Ubuntu 26.04, x86_64**, inside your ordinary user's **Wayland desktop
  session**, with a working systemd user manager.
- Python 3.10+, Git, Cargo/Rust 1.85+, Rustup, a C compiler, make, pkg-config,
  coreutils, tar/xz, and curl when a source download is needed.
- `dump.erofs` from `erofs-utils`, supporting `--cat` and `--path`.
- Development files for libdrm, GBM, Wayland, xkbcommon, and libffi when the
  title-bar presenter needs building.
- The real `/boot/config-$(uname -r)` for the running kernel.
- Administrator approval through sudo for the named Droidloom-only system
  changes, and enough disk space for source/build caches plus the runtime.
  Allow at least the upstream 20 GiB runtime budget and additional build space.

Other Debian/Ubuntu releases remain unverified and are refused by this
installer rather than having their libraries or security configuration replaced.
Intel/GNOME Wayland is the original tested graphics/desktop combination.
The installer is new: its isolated tests and non-installing checks do not
constitute a fresh-machine end-to-end install test. Review it before first use.

### Build dependencies: choose deliberately

The installer **does not automatically install development packages or upgrade
the operating system**. You can use a prepared build environment, or privately
extract distribution development packages into a sysroot, as in the original
test. Supply its absolute path with `--build-sysroot`.

Alternatively, if you explicitly want the dependencies installed on this host,
review apt's proposed changes before accepting this separate command:

```sh
sudo apt-get install --no-remove --no-install-recommends \
  build-essential pkg-config git curl xz-utils erofs-utils \
  libdrm-dev libgbm-dev libwayland-dev libxkbcommon-dev libffi-dev
```

Cargo and Rustup must already be available; use a trusted Rust installation.
The installer may add the pinned Rust 1.93.0 toolchain **under your user account**
when a presenter build is needed. It does not change your default toolchain.
No development dependencies are needed for a presenter already verified as
installed, although Cargo and a linker are needed to compile the installer itself.

## 3. Open the repository and review the plan

From the `droidloom-deb` directory, as your **normal desktop user**, not root:

```sh
sh install.sh --help
./install.sh --check
```

If using privately extracted development files:

```sh
./install.sh --check --build-sysroot "/absolute/path/to/private/sysroot"
```

For a fresh runtime installation:

```sh
./install.sh --check \
  --runtime-deb "/absolute/path/to/droidloom_0.1.0-49_amd64.deb" \
  --build-sysroot "/absolute/path/to/private/sysroot"
```

Omit `--build-sysroot` when the build libraries are already available.
`--check` performs prerequisite and existing-state checks only: no sudo, upstream
source fetch, package install, configuration writes, or service stop/start.
The small Rust launcher itself may compile/download its locked Cargo dependencies
and write ignored `.work`/Cargo cache files before those checks run.

Checks reject a different runtime version/hash, another user's cell, edited
managed files, conflicting overrides, unsafe output paths, and ambiguous
partially installed fixes. They do not delete anything to make a check pass.

## 4. Install everything applicable in one invocation

For an existing matching runtime:

```sh
./install.sh
```

For a fresh setup using the reviewed local runtime package:

```sh
./install.sh --runtime-deb "/absolute/path/to/droidloom_0.1.0-49_amd64.deb"
```

Add `--build-sysroot "/absolute/path/to/private/sysroot"` if applicable.
The script shows an apply/skip plan and asks for confirmation. Save work in
Android apps before approving: activation stops **only Droidloom**.

Useful options:

| Option | Meaning |
| --- | --- |
| `--yes` | Approve the displayed installer plan; sudo and apt may still prompt. |
| `--no-start` | Leave Droidloom stopped after making changes. |
| `--util-linux-archive FILE` | Use an already downloaded, SHA256-verified 2.42 source archive. Other source/Cargo downloads may still be required. |
| `--runtime-deb FILE` | Install the pinned local runtime only if none is installed. Never automatically upgrade/downgrade an existing runtime. |

There is no `--force` bypass for mismatched versions or conflicting files.

### Order of operations

1. Inspect prerequisites, pinned inputs, and existing installations.
2. Ask you to approve the plan.
3. Fetch the exact upstream commit into a new ignored build directory, check and
   apply the source patch, and run focused presenter tests before building.
4. If needed, fetch/verify util-linux 2.42 and compile **only** `unshare`.
   These builds run as your user with one compiler job. Nothing runs `make install`.
5. Request sudo and recheck the installation state after the build.
6. Stop Droidloom, or, on a fresh system, install the explicitly supplied `.deb`
   with apt and run upstream first-time setup for your UID.
7. Install the private namespace tool/override if needed, call the existing
   kernel-config helper, and write the per-user guarded presenter installation.
8. Reload only the relevant systemd definitions and start Droidloom, waiting for
   Android readiness, unless `--no-start` was selected.

For a fresh install, apt owns runtime dependency resolution and asks before
changes; the installer passes `--no-remove`, never `dist-upgrade` or `-y`.
Normal apt dependency configuration can affect shared host packages/services,
so review its proposed changes; it is not the same as the fixes-only path,
which invokes no package manager.
The upstream setup helper creates Droidloom's cell/data and its narrowly scoped
authorization rule. The installer does not invent a replacement `cell.json`,
adopt unidentified existing storage, or clear Android data.

## 5. Verify

```sh
droidloomctl status
droidloomctl launch com.android.settings
./install.sh --check
```

Check Android boots and the Settings window has controls on GNOME Wayland.
If a fix was already verified, the installer reports it as skipped. In the
all-skipped case it does not stop/start anything, even with `--no-start`.

New title-bar installations record their binary hash in
`~/.local/lib/droidloom-titlebar/installer-state.json`. The original manually
installed presenter is also recognized by its exact recorded binary hash and
equivalent guarded unit, without altering that existing installation.

## 6. Failure handling and rollback

This is **not an all-or-nothing package transaction**. Build/test errors occur
before service or system changes. An activation error stops the installer;
earlier explicitly applied steps may remain. There is no broad automatic cleanup
that could discard user edits. A readiness failure triggers an attempt to stop
only Droidloom.

Inspect the reported error and:

```sh
systemctl --user status droidloom.service
journalctl --user -u droidloom.service -n 100 --no-pager
journalctl -u droidloomd.service -n 100 --no-pager
```

- [Roll back the kernel workaround](kernel-config.md#rollback).
- [Roll back the title-bar override](titlebar.md#rollback).
- [Roll back a newly installed private-unshare override](unshare.md#rollback).

The installer preserves the original packaged presenter, original Android
images, unrelated overrides, and app data. An interrupted per-user file copy
may leave an incomplete dedicated title-bar directory: review its contents
before repairing/removing individual files. Do not recursively delete Android
storage or service-override directories.

An interrupted process can also leave `.work/install.lock`; check the recorded
PID and confirm no installer is running before removing that exact stale lock.
Build directories remain under `.work/install-build-*` for inspection and are
ignored by Git.

## What was patched, how, and why

| Change | Mechanism and detailed explanation |
| --- | --- |
| Missing kernel configuration | [Equal-length pathname replacement in a separate Android library copy; real compressed kernel config, runtime mounts, startup refresh](kernel-config.md). |
| Missing GNOME window controls | [Pinned Rust source patch; separate decoration subsurfaces, frame actions, content geometry, and per-user version guard](titlebar.md). |
| Namespace-tool compatibility | [Separate upstream util-linux build selected only through Droidloom's service PATH](unshare.md). No host utility is patched in place. |

Application compatibility is a separate issue; see
[the observed limitations](compatibility.md). These changes do not fix the
observed Twitter library lookup or establish why PUBG's native startup stalled.
