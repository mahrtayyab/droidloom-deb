# Fixes and rollback

All fixes target upstream release 49, pinned in [upstream.json](../upstream.json).
Use [the installer](install.md) to apply them together.

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
