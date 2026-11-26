# Fixes and rollback

All fixes target upstream release 49, pinned in [upstream.json](../upstream.json).
Use [the installer](install.md) to apply them together.

## Binderfs startup after reboot

If `droidloomctl start` reports that the Android cell is not running and the
daemon logs show `mount .../dev/binderfs ... No such device`, the stock Binder
driver may not be loaded. On the tested Ubuntu kernel, Binder is a loadable
module; loading it once does not persist across reboots.

Read-only checks:

```sh
grep -w binder /proc/filesystems
modinfo -F filename binder_linux
modprobe --show-depends binder_linux
```

The filesystem type is `binder`; `/dev/binderfs` is Android's mountpoint.
No `binder` entry is expected while the module is unloaded.

Run `./install.sh --check`, then `./install.sh` to apply the fix. The installer
checks the running kernel's Binder/Binderfs configuration and matching module
files. For a loadable driver it installs
[`examples/binder.conf`](../examples/binder.conf) as
`/etc/systemd/system/droidloomd.service.d/binder.conf`:

```ini
[Service]
ExecStartPre=/usr/sbin/modprobe binder_linux
```

This loads the stock driver before Droidloom creates Android's namespaces.
The precheck is added even if the module is already loaded, preventing the same
failure after reboot. Registered, built-in Binder support needs no new drop-in.
An existing identical drop-in is reused; edited/conflicting files are refused.

**Caution:** loading a module affects the running host kernel and requires
administrator approval. Only the existing distribution module is used—no
replacement driver, kernel rebuild, force-loading, or security-policy changes.
Missing/mismatched modules, unsupported kernel layouts, and modprobe
blacklist/install rules require manual review. The installer never disables
Secure Boot, AppArmor, signature enforcement, or the kernel's module-loading lock.
If module loading is locked down, arrange the normal OS load before that lock.
The precheck explicitly requests this module: review/remove it if you later
change module-loading policy, rather than assuming a blacklist alone disables it.

`--check` never loads a driver or changes services. `--no-start` installs the
precheck but leaves loading until Droidloom starts. This does not enable
automatic login startup or change global module-loading configuration.
If the on-disk precheck is not effective, reload definitions with
`sudo systemctl daemon-reload` and inspect
`systemctl show droidloomd.service -p ExecStartPre` for conflicting resets.

## Kernel-config boot crash

Android's `Debug.isVmapStack()` aborts `system_server` if `/proc/config.gz` is
unavailable. The tested Ubuntu kernel disables that interface.

`ubuntu_kernel_config.py` extracts `libandroid_runtime.so` from the existing
EROFS image and changes one equal-length pathname in a **separate copy**:

```text
/proc/config.gz -> /droidloom/k/gz
```

It verifies the x86_64 ELF and read-only, non-executable string segment; no
instructions or ELF layout change. The replacement path exposes a compressed
copy of the **real running kernel's** `/boot/config-$(uname -r)`.

Two entries in `/etc/droidloom/cell.json` mount the copied library and config.
Generated files live under `/usr/local/lib/droidloom-ubuntu-kernel-config/`.
The `ubuntu-kernel-config.conf` daemon drop-in refreshes them before startup,
so a kernel/image update does not leave stale files. Unrecognized layouts,
conflicting overrides, and edited generated files are refused.

## GNOME window controls

[The source patch](../patches/0001-wayland-client-side-decorations.patch) adds
`decorations.rs`, updates the presenter, and locks `sctk-adwaita` 0.11.0.

The presenter requests server-side decorations first. When unavailable or when
the compositor selects client-side mode, it draws a title bar on separate SHM
subsurfaces. Border dimensions are kept outside Android's content coordinates.
Frame clicks trigger close/minimize/maximize/move/resize actions rather than
Android touch events. Fullscreen hides the frame; child surfaces are destroyed
before the toplevel.

The custom binary lives under `~/.local/lib/droidloom-titlebar/`. Only
`~/.config/systemd/user/droidloom.service.d/titlebar.conf` selects it.
A SHA256 guard prevents reuse with an unmatched packaged presenter after updates.
`/usr/bin/droidloom-wayland`, GNOME settings, and Android's GPU/input protocols
are not replaced.

## Private namespace utility

The working Ubuntu setup used util-linux 2.42 rather than the system's 2.41.3.
When the selected `unshare` is older, the installer verifies the pinned upstream
archive and builds only this tool:

```sh
./configure --disable-all-programs --enable-unshare --disable-nls
make -j1 unshare
```

It installs `/opt/droidloom/bin/unshare` and selects that directory only through
`droidloomd.service`'s PATH, using `ubuntu-unshare.conf`. The system
`/usr/bin/unshare` and global PATH remain unchanged. A compatible existing tool
is left alone.

## Rollback

Stop Droidloom first:

```sh
droidloomctl stop
```

Undo only the fix you intend to remove.

### Binder startup precheck

Remove only the unchanged project drop-in:

```sh
if cmp examples/binder.conf /etc/systemd/system/droidloomd.service.d/binder.conf; then
  sudo rm -- /etc/systemd/system/droidloomd.service.d/binder.conf
  sudo systemctl daemon-reload
fi
```

This does not unload the driver. Do not use `rmmod` or `modprobe -r` while any
Android runtime is using Binder. Without the precheck, the failure can return
after reboot; unrelated drop-ins and global module settings should be left alone.

### Kernel-config workaround

```sh
sudo /usr/bin/python3 /usr/local/lib/droidloom-ubuntu-kernel-config/helper.py --remove
```

This removes only its two configuration entries and service drop-in. Generated
files remain disabled. The original boot crash can return without the workaround.

### Window controls

Remove the drop-in only if it still matches the project's override:

```sh
if cmp examples/titlebar.conf "$HOME/.config/systemd/user/droidloom.service.d/titlebar.conf"; then
  rm -- "$HOME/.config/systemd/user/droidloom.service.d/titlebar.conf"
  systemctl --user daemon-reload
fi
```

The original manual installation used absolute home paths and different comments;
compare its active directives with the example before removing it.

### Private unshare

Remove only the unchanged installer-created drop-in:

```sh
if cmp examples/ubuntu-unshare.conf /etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf; then
  sudo rm -- /etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf
  sudo systemctl daemon-reload
fi
```

Do not remove a pre-existing `override.conf` or unrelated service settings.
The disabled private binary can remain. An older system tool may bring the
compatibility problem back.

After reviewing the result, start again with `droidloomctl start`. None of these
rollback steps clears Android app data or removes the original images.
