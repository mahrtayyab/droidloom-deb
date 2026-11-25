# Sources and licenses

The `.deb` repackages unmodified Droidloom release-49 binaries and images.
The Ubuntu fixes are separate source/install steps in this repository.

- **Droidloom source and build tooling:**
  <https://github.com/denialwm/droidloom/tree/572e1c51c5caf75183efe25ad811f98c1268bf56>
- **Android/AOSP, base-image, and native-bridge input pins:**
  <https://github.com/denialwm/droidloom/tree/572e1c51c5caf75183efe25ad811f98c1268bf56/android/manifest>
- **Rust dependency versions and checksums:**
  <https://github.com/denialwm/droidloom/blob/572e1c51c5caf75183efe25ad811f98c1268bf56/Cargo.lock>
- **Upstream build instructions:**
  <https://github.com/denialwm/droidloom/blob/572e1c51c5caf75183efe25ad811f98c1268bf56/docs/BUILDING.md>
- **Packaging and compatibility fixes:**
  <https://github.com/mahrtayyab/droidloom-deb>

The binary inputs and hashes are recorded in `packaging/inputs-49.json`.
The package retains its Android, Mesa, and Cargo notices under
`usr/share/licenses/droidloom-*`. Upstream's license scope is retained in
`LICENSES/Droidloom-THIRD-PARTY.md`.

Before distributing binaries, provide complete corresponding source for covered
components at these exact pins, with clear access instructions beside the
download. A top-level source archive or patch file alone does not include every
third-party source tree. Keep the component notices; the project's GPL license
does not relicense Android, Mesa, dependencies, or separately installed apps.
