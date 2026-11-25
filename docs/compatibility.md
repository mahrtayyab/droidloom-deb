# Tested environment and limitations

## Original local validation

| Component | Observed environment |
| --- | --- |
| Host | Ubuntu 26.04.1 LTS, x86_64 |
| Desktop | GNOME on Wayland |
| Graphics | Intel, native DRM backend |
| Droidloom | Local unofficial `droidloom` package, `0.1.0-49`, amd64 |
| Upstream source | `572e1c51c5caf75183efe25ad811f98c1268bf56` |
| Android | Android 17 |

The boot workaround restored Android startup after the missing kernel-config
abort. Client-side controls were observed working with Android Settings.
The helper's 19 tests passed. The patched presenter passed 101 unit tests and
eight integration tests; one hardware-dependent GPU test was skipped.

These results describe the original installation, not a complete clean-install
test on every Ubuntu/Debian version. Other releases, GPUs, desktops, host
architectures, and Android image versions are unverified.

## Application compatibility is separate

Droidloom's ARM64 translation on x86_64 is experimental. Successful Android boot
and title bars do not establish compatibility for all Android applications.

- X/Twitter was observed failing in SoLoader: it searched an x86_64 APK path
  despite an ARM64-only `libyoga.so`. No per-app fix is included here.
- PUBG Mobile was observed staying at its startup splash with two busy native
  threads. Its engine library loaded, but the exact cause of the stall was not
  established. No game/anti-cheat workaround is included here.
- This project does not supply Google services, device certification, or fixes
  for integrity checks.

## Shared files

When Droidloom's user-folder sharing is enabled, Android's Download, Documents,
Pictures, Music, and Movies can be the same files as the desktop user's matching
folders. Android edits and deletions can therefore affect host files. These
fixes do not change that setting.
