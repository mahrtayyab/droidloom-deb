# Client-side window controls

The patch adds separate SHM decoration subsurfaces when the Wayland compositor
does not supply server-side decorations. Android keeps its content origin,
touch coordinates, DPI, and direct GPU buffers. Close uses Droidloom's existing
task-close transaction. Fullscreen windows hide the frame.

The patch includes `decorations.rs`, changes to the presenter, the exact
`sctk-adwaita` dependency and Cargo lockfile, documentation, and focused tests.

## What changed in the source

| File in upstream | Change |
| --- | --- |
| `graphics/droidloom-wayland/src/decorations.rs` | New frame creation, geometry conversion, cursor/frame input, window actions, and focused tests. |
| `graphics/droidloom-wayland/src/main.rs` | Bind SHM/subcompositor globals, attach frames to tasks, route decoration input separately, and update/draw/destroy frames alongside windows. |
| `graphics/droidloom-wayland/Cargo.toml` | Add the exact `sctk-adwaita = "=0.11.0"` dependency. |
| `Cargo.lock` | Lock the frame library and its dependencies. |
| `docs/INSTALL.md` | Describe the resulting window controls. |

At runtime the presenter first requests server-side decorations. If the
compositor has no decoration protocol, or selects client-side mode, it creates an
Adwaita frame for that task. Incoming outer-window dimensions are converted to
Android content dimensions by subtracting frame borders; unspecified dimensions
remain unspecified and tiny sizes cannot become zero.

The frame receives pointer events on its own subsurfaces. Button/border actions
send the corresponding Wayland close/minimize/maximize/move/resize requests with
the input serial, rather than forwarding those clicks into Android. Frame state,
scale, fullscreen visibility, and drawing follow the task lifecycle. Child frame
surfaces are destroyed before the window's toplevel is destroyed.

## Obtain and check the pinned source

These commands write only a new upstream checkout in this repository's ignored
`.work` directory. They do not change an existing Droidloom checkout.

```sh
mkdir -p .work
git clone https://github.com/denialwm/droidloom.git .work/droidloom
git -C .work/droidloom switch --detach 572e1c51c5caf75183efe25ad811f98c1268bf56
git -C .work/droidloom apply --check ../../patches/0001-wayland-client-side-decorations.patch
git -C .work/droidloom apply ../../patches/0001-wayland-client-side-decorations.patch
```

The patch must be applied only once to a clean checkout at the pinned commit.
Do not discard existing edits to make it apply. Future releases require a
separate review and matching build.

## Build and test

The pinned source requests Rust 1.93.0 through `rust-toolchain.toml`. Use its
upstream build prerequisites; the tested presenter build needs the Wayland,
xkbcommon, GBM/DRM, and libffi development files and a working C linker.
Do not replace host runtime libraries to satisfy build dependencies.

The original local test used privately extracted development packages in a
build sysroot rather than apt/dpkg installation. When using that approach, the
relevant environment is:

```sh
export PKG_CONFIG_SYSROOT_DIR="/absolute/path/to/private/sysroot"
export PKG_CONFIG_PATH="$PKG_CONFIG_SYSROOT_DIR/usr/lib/x86_64-linux-gnu/pkgconfig"
```

Otherwise use a deliberately prepared build environment with those dependencies.
The private sysroot and downloaded build packages are not included in this repo.

From the patched upstream checkout:

```sh
nice -n 10 cargo test --locked -j 1 -p droidloom-wayland
nice -n 10 cargo build --release --locked -j 1 -p droidloom-wayland
```

Keep at least two logical CPUs free. These are presenter-only builds, not a
complete Android or `.deb` runtime build. The original local validation passed
101 unit tests and eight integration tests, with one hardware-specific GPU
test skipped; this is historical evidence, not a clean-build portability claim.

## Enable separately for your desktop user

Only do this on the matching release-49 runtime. From this repository's root,
check the unchanged packaged binary first:

```sh
sha256sum --check examples/packaged-presenter.sha256
```

If the hash does not match, **stop here**. Do not replace the expected hash merely
to bypass the guard. Rebase and test against the installed runtime instead.

After a successful build, stop Droidloom and copy the presenter separately.
Review existing files at these dedicated paths first; do not overwrite a
different local custom presenter or service override.

```sh
droidloomctl stop
install -d "$HOME/.local/lib/droidloom-titlebar"
install -m 755 .work/droidloom/target/release/droidloom-wayland \
  "$HOME/.local/lib/droidloom-titlebar/droidloom-wayland"
install -m 644 examples/packaged-presenter.sha256 \
  "$HOME/.local/lib/droidloom-titlebar/packaged-presenter.sha256"
install -d "$HOME/.config/systemd/user/droidloom.service.d"
install -m 644 examples/titlebar.conf \
  "$HOME/.config/systemd/user/droidloom.service.d/titlebar.conf"
systemctl --user daemon-reload
droidloomctl start
```

The example uses systemd's `%h` home-directory specifier; it contains no personal
home path. The original `/usr/bin/droidloom-wayland` is not replaced. No GNOME
settings or extensions are changed.

The `ExecCondition` hash guard skips the custom presenter if a package update
changes the original binary. Rebuild/review for that update, or roll back the
override; do not silently reuse an unmatched presenter.

## Rollback

Remove only this specific drop-in after checking that it is still ours:

```sh
cmp examples/titlebar.conf "$HOME/.config/systemd/user/droidloom.service.d/titlebar.conf"
```

The original manual installation used absolute home paths and different
comments. For that installation, compare the active directives to the example
after substituting your own home for `%h`; do not remove an unrelated override.

If it matches:

```sh
droidloomctl stop
rm -- "$HOME/.config/systemd/user/droidloom.service.d/titlebar.conf"
systemctl --user daemon-reload
droidloomctl start
```

Do not remove the entire service-override directory. The disabled custom binary
can remain in its dedicated directory. The independent kernel-config workaround
is not removed.
