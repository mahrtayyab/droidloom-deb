# Private namespace-utility prerequisite

The original Ubuntu setup used util-linux **2.42** `unshare` privately under
`/opt/droidloom/bin/unshare`; Ubuntu's `/usr/bin/unshare` was 2.41.3.
This prerequisite was not in the initial compatibility-source repository.

## How it works

Droidloom creates Linux namespaces, including user-namespace mappings used by
shared storage. The original working stack selected the newer utility by setting
`PATH` only for `droidloomd.service`. It did not replace any system command.
Util-linux 2.42 includes user-namespace bind-mount fixes, but this guide does not
claim an unrecorded individual upstream change was conclusively the original
failure's cause.

The installer keeps that tested minimum:

- If the already selected system/private `unshare` is 2.42 or newer, skip it.
- Otherwise download **the 2.42 release source**, verify its pinned SHA256, then
  configure `--disable-all-programs --enable-unshare --disable-nls`.
- Run `make -j1 unshare` as the ordinary user; never `make install`.
- Install only the compiled tool at `/opt/droidloom/bin/unshare` and
  `examples/ubuntu-unshare.conf` at
  `/etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf`.

That drop-in prepends `/opt/droidloom/bin` to the daemon's PATH. It does not
change the user's PATH, the system-wide environment, the kernel, or
`/usr/bin/unshare`.

An existing custom service PATH or inactive private installation that cannot
be safely recognized is a conflict, not permission to overwrite it.
An already working private tool/override from the original setup is left alone.

## Source provenance

The archive URL and SHA256 are in `upstream.json`. Its SHA256 matches the
upstream checksum catalog at:

<https://mirrors.edge.kernel.org/pub/linux/utils/util-linux/v2.42/sha256sums.asc>

The installer checks the pinned hash before extraction. It does not claim to
perform a new OpenPGP-signature verification or install the newest util-linux
release. Review the pinned version when maintaining the project.
The downloaded source, including its license notices, remains in the ignored
build directory. `unshare` is GPL-2.0-or-later; util-linux components retain their
own licenses.

## Rollback

Only roll back the new `ubuntu-unshare.conf` if it still matches the example:

```sh
cmp examples/ubuntu-unshare.conf \
  /etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf
```

If it matches, stop only Droidloom, then remove only that drop-in:

```sh
droidloomctl stop
sudo rm -- /etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf
sudo systemctl daemon-reload
```

The disabled binary can remain in its dedicated directory. Do not remove
pre-existing `override.conf` files or other service overrides. If the installer
skipped an existing private tool, it did not create this drop-in; review the
original setup's override rather than assuming this rollback applies.

On an older system utility, the original compatibility problem may return.
This rollback does not remove the independent kernel-config or title-bar fixes.
