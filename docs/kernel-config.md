# Ubuntu kernel-config workaround

Android 17's `Debug.isVmapStack()` aborts `system_server` when it cannot read
`/proc/config.gz`. The tested Ubuntu kernel has `CONFIG_IKCONFIG` disabled.
This workaround supplies the **real running kernel configuration** without
rebuilding the kernel or inventing kernel features.

## What changes

The helper reads `libandroid_runtime.so` from Droidloom's existing EROFS system
image with `dump.erofs`. In a separate copy, it replaces exactly one equal-length
pathname in a read-only, non-executable ELF segment:

```text
/proc/config.gz -> /droidloom/k/gz
```

It compresses `/boot/config-$(uname -r)` and adds only:

- Two entries in `/etc/droidloom/cell.json`: an Android library override and a
  read-only runtime directory.
- Generated files and a manifest under
  `/usr/local/lib/droidloom-ubuntu-kernel-config/`.
- `/etc/systemd/system/droidloomd.service.d/ubuntu-kernel-config.conf`, whose
  startup precheck refreshes the files after kernel or Android image updates.

### How Android uses it

On boot, Android's Java `Debug.isVmapStack()` reaches its native implementation
in `libandroid_runtime.so`. With the override mounted at the original Android
library path, that implementation opens `/droidloom/k/gz` instead of
`/proc/config.gz`, reads the compressed kernel configuration, and checks the
real `CONFIG_VMAP_STACK` setting. It no longer aborts merely because Ubuntu does
not expose the optional `/proc/config.gz` interface.

The helper verifies the library is an x86_64 ELF, that exactly one original
pathname exists, and that it lies in a read-only, non-executable segment. Only
that equal-length byte string changes; no machine instructions, ELF layout, or
kernel-feature values are changed. The startup refresh repeats this validation
and extraction so a kernel/image update cannot silently reuse an old config.

The helper does not modify the original Android image, host libraries, kernel,
security policies, unrelated services, or Android data. It creates no Android
data/image backups. `systemctl daemon-reload` reloads unit definitions; it does
not restart unrelated services.

It refuses conflicting overrides, unsafe output paths, unexpected ELF layouts,
or edited generated files. Its pathname validation is not a guarantee of
compatibility with an arbitrary future Android image; review and test updates.

## Requirements

- An existing x86_64 Droidloom installation with `/etc/droidloom/cell.json`.
- Python 3.10 or newer, systemd, and `dump.erofs` from `erofs-utils`, providing
  `--cat` and `--path`.
- The real running kernel's readable `/boot/config-$(uname -r)`, specifying
  `CONFIG_VMAP_STACK`.

This is not an installer for the base Droidloom runtime.

## Read-only checks

From the repository root:

```sh
python3 -m unittest -v test_ubuntu_kernel_config.py
python3 ubuntu_kernel_config.py --check
```

The check reads the installed image and kernel configuration but does not write
files or change services. The tests use temporary files and mocked privileged
operations.

## Apply

Administrator access is needed only to write the dedicated generated files,
the two Droidloom configuration entries, and the Droidloom-only service drop-in.
Review the helper and stop Droidloom first:

```sh
droidloomctl stop
sudo /usr/bin/python3 ubuntu_kernel_config.py --apply
droidloomctl start
```

The helper itself never starts or stops services. Apply/remove refuse to run
while `droidloomd.service` is active. If it refuses an existing installation,
review the conflict rather than removing its manifest or forcing an overwrite.

## Diagnostics

The optional bounded diagnostic mode reads boot properties and the crash buffer
from Droidloom's Android init namespace:

```sh
sudo /usr/bin/python3 /usr/local/lib/droidloom-ubuntu-kernel-config/helper.py --diagnose
```

Review diagnostic output for private information before sharing it.

## Rollback

```sh
droidloomctl stop
sudo /usr/bin/python3 /usr/local/lib/droidloom-ubuntu-kernel-config/helper.py --remove
```

Removal deletes only the exact two configuration entries and the exact service
drop-in belonging to the workaround. Other configuration, including later user
changes, is preserved. It refuses conflicting or edited drop-ins.
Disabled generated files remain in the dedicated directory; they are not backups.

The original missing-kernel-config boot problem may return after rollback.
Rollback of this fix is independent of the custom title-bar presenter.
