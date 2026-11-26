#!/bin/sh
# Required shell entry point; installation logic lives in the tested Rust driver.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)

case "${1:-}" in
  -h|--help)
    cat <<'HELP'
Usage: ./install.sh [OPTIONS]

Install the pinned Droidloom compatibility fixes as your ordinary desktop user.
An existing matching runtime is reused. A fresh setup needs the separately
obtained, trusted release-49 .deb; this repository does not distribute it.

Options:
  --check                  Check prerequisites/state without installing or stopping
  --yes                    Approve the installer's plan (sudo/apt may still prompt)
  --no-start               Leave Droidloom stopped after applying changes
  --runtime-deb FILE       Install the exact supported local .deb if runtime is absent
  --build-sysroot DIR      Use privately extracted development libraries for the build
  --util-linux-archive FILE
                           Use the verified 2.42 source archive instead of downloading
  -h, --help               Show this help without compiling anything

Run ./install.sh --check first. The launcher may create local Cargo build/cache
files, but --check does not use sudo, install packages, fetch upstream sources,
change configuration, or stop/start services.

See docs/install.md for prerequisites, the full walkthrough, and rollback.
Binder checks do not load drivers. Approved installation adds a Droidloom-only
startup precheck for the stock module when needed; loading affects the host kernel.
HELP
    exit 0 ;;
esac

if [ "$(id -u)" -eq 0 ]; then
  echo "Run ./install.sh as your ordinary Wayland desktop user, not with sudo." >&2
  exit 1
fi
command -v cargo >/dev/null 2>&1 || {
  echo "Cargo/Rust is required to build the installer. See docs/install.md." >&2
  exit 1
}

# Never build as root, change the default Rust toolchain, or run make install.
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="$ROOT/.work/installer-target"
exec cargo run --quiet --locked --manifest-path "$ROOT/tools/installer/Cargo.toml" -- "$@"
