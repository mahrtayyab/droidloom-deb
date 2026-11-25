# Working on droidloom-deb

- This is an unofficial compatibility repository, not the full Droidloom tree.
- Keep the upstream commit, patch, example hash guard, and documentation aligned.
- Preserve the existing Python helper and its safety tests. Use Rust for new
  substantial tooling; shell commands are appropriate for required build and
  systemd interfaces.
- Run `python3 -m unittest -v test_ubuntu_kernel_config.py` for helper changes.
  Run `python3 -m unittest -v test_release_metadata.py` for package/release metadata.
  Check the title-bar patch with `git apply --check` in a clean, pinned upstream
  checkout. Never stage or discard work in someone else's upstream checkout.
- Documentation and development do not authorize installation, service restarts,
  package-manager changes, clearing Android data, or publishing to a remote.
- Keep packages, Android images/data, APKs, caches, credentials, and personal
  configuration out of Git. Preserve upstream license notices.
- A rootless binary-repack recipe exists; a complete Debian port/source-build
  package does not. Do not claim the rebuilt archive reproduces the historical
  package checksum. Keep the historical release asset unchanged and outside Git.
- Keep release metadata factual; do not claim an upload without verifying it.
  Review corresponding-source obligations before any public binary publication.
