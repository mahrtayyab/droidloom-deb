# Droidloom 0.1.0-49 — Ubuntu amd64

Unofficial Debian-format package for **Ubuntu 26.04 amd64 (x86_64), Wayland**.

- Binary: `droidloom_0.1.0-49_amd64.deb`
- Verify with the attached `SHA256SUMS`.
- [Installation instructions](https://github.com/mahrtayyab/droidloom-deb/blob/main/docs/install.md)

The `.deb` contains the base runtime. The repository's `install.sh` applies the
Binder startup, kernel-config, window-control, and private namespace-tool fixes
separately:

```sh
./install.sh --check --runtime-deb "/path/to/droidloom_0.1.0-49_amd64.deb"
./install.sh --runtime-deb "/path/to/droidloom_0.1.0-49_amd64.deb"
```

Experimental: tested on Ubuntu 26.04.1, GNOME Wayland, and Intel graphics.
Fresh-machine installs and other platforms are not yet validated. ARM app
translation remains experimental; Google services are not included.

Upstream release 49: `572e1c51c5caf75183efe25ad811f98c1268bf56`.
See the attached `PROVENANCE.json` and `SOURCES.md` for input and source details.
