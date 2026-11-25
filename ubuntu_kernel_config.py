#!/usr/bin/python3
"""Isolated, reversible workaround for Android's mandatory /proc/config.gz read.

No Android data/image backups, kernel changes, host-library replacements, or
security-policy changes. --check is read-only. Only --apply/--remove/--refresh
write files, and they require root. Services are never started by this helper.
"""

import argparse
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import stat
import struct
import subprocess
import sys
import tempfile


SPEC = Path("/etc/droidloom/cell.json")
PREFIX = Path("/usr/local/lib/droidloom-ubuntu-kernel-config")
DROPIN = Path(
    "/etc/systemd/system/droidloomd.service.d/ubuntu-kernel-config.conf"
)
LIBRARY_TARGET = "/system/lib64/libandroid_runtime.so"
DIRECTORY_TARGET = "/droidloom/k"
ORIGINAL_PATH = b"/proc/config.gz\0"
REPLACEMENT_PATH = b"/droidloom/k/gz\0"
MARKER = "droidloom-ubuntu-kernel-config-v1"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def overrides():
    return (
        ("android_file_overrides", {
            "source": str(PREFIX / "libandroid_runtime.so"),
            "target": LIBRARY_TARGET,
        }),
        ("android_runtime_directories", {
            "source": str(PREFIX / "kernel"),
            "target": DIRECTORY_TARGET,
        }),
    )


def overlaps(first, second):
    first, second = PurePosixPath(first), PurePosixPath(second)
    return first == second or first in second.parents or second in first.parents


def edit_spec(spec, enable):
    """Change only our two entries, refusing to replace anyone else's override."""
    edited = copy.deepcopy(spec)
    for key, entry in overrides():
        entries = edited.get(key, [])
        if not isinstance(entries, list):
            raise ValueError(f"{key} must be a list")
        for existing in entries:
            if not isinstance(existing, dict) or not isinstance(
                existing.get("target"), str
            ):
                raise ValueError(f"Invalid entry in {key}")
            if overlaps(existing["target"], entry["target"]) and existing != entry:
                raise ValueError(f"Conflicting existing override: {existing['target']}")
        if enable:
            if entry not in entries:
                edited[key] = entries + [entry]
        elif key in edited:
            edited[key] = [existing for existing in entries if existing != entry]
    return edited


def patch_library(original):
    # This helper is intentionally limited to the installed x86_64 Android ELF.
    if (
        len(original) < 64
        or original[:6] != b"\x7fELF\x02\x01"
        or original[18:20] != b"\x3e\x00"
    ):
        raise ValueError("Expected a little-endian x86_64 ELF library")
    if len(ORIGINAL_PATH) != len(REPLACEMENT_PATH):
        raise ValueError("Replacement pathname must have exactly the same size")
    count = original.count(ORIGINAL_PATH)
    if count != 1:
        raise ValueError(f"Expected exactly one kernel-config pathname, found {count}")
    offset = original.index(ORIGINAL_PATH)
    program_offset = struct.unpack_from("<Q", original, 32)[0]
    entry_size, entry_count = struct.unpack_from("<HH", original, 54)
    if (
        entry_size != 56 or not 0 < entry_count < 1024
        or program_offset + entry_size * entry_count > len(original)
    ):
        raise ValueError("Invalid ELF program-header table")
    read_only_data = False
    for index in range(entry_count):
        start = program_offset + index * entry_size
        kind, flags, file_offset, _, _, file_size, _, _ = struct.unpack_from(
            "<IIQQQQQQ", original, start
        )
        if (
            kind == 1 and flags == 4
            and file_offset <= offset
            and offset + len(ORIGINAL_PATH) <= file_offset + file_size
            and file_offset + file_size <= len(original)
        ):
            read_only_data = True
    if not read_only_data:
        raise ValueError("Config pathname is not in a read-only, non-executable ELF segment")
    patched = original.replace(ORIGINAL_PATH, REPLACEMENT_PATH, 1)
    if (
        len(patched) != len(original)
        or original[:offset] != patched[:offset]
        or original[offset + len(ORIGINAL_PATH):]
        != patched[offset + len(ORIGINAL_PATH):]
    ):
        raise ValueError("Unexpected change outside the config pathname")
    return patched


def kernel_config():
    release = platform.release()
    path = Path("/boot") / f"config-{release}"
    data = path.read_bytes()
    lines = data.splitlines()
    if (
        b"CONFIG_VMAP_STACK=y" not in lines
        and b"# CONFIG_VMAP_STACK is not set" not in lines
    ):
        raise ValueError(f"{path} does not specify CONFIG_VMAP_STACK")
    return release, data


def payloads(spec):
    image = Path(spec["image_dir"]) / "images/system.img"
    result = subprocess.run(
        ["/usr/bin/dump.erofs", "--cat", f"--path={LIBRARY_TARGET}", str(image)],
        check=True, capture_output=True, timeout=30,
    )
    original = result.stdout
    patched = patch_library(original)
    release, config = kernel_config()
    return patched, gzip.compress(config, mtime=0), {
        "marker": MARKER,
        "kernel": release,
        "kernel_config_sha256": digest(config),
        "original_library_sha256": digest(original),
        "patched_library_sha256": digest(patched),
        "image": str(image),
    }


def encode_json(value):
    return (json.dumps(value, indent=2) + "\n").encode()


def read_spec():
    value = json.loads(SPEC.read_bytes())
    if not isinstance(value, dict):
        raise ValueError("Droidloom cell configuration must be an object")
    return value


def check_root_path(path):
    """Privileged outputs must not traverse user-writable paths or symlinks."""
    for part in [path, *path.parents]:
        try:
            info = part.lstat()
        except FileNotFoundError:
            continue
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != 0
            or info.st_mode & 0o022
        ):
            raise ValueError(f"Unsafe privileged output path: {part}")


def atomic_write(path, data, mode=0o644):
    if path.is_symlink():
        raise ValueError(f"Refusing to overwrite a symlink: {path}")
    previous = path.stat() if path.exists() else None
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            if previous:
                if os.geteuid() == 0:
                    os.fchown(stream.fileno(), previous.st_uid, previous.st_gid)
                os.fchmod(stream.fileno(), stat.S_IMODE(previous.st_mode))
            else:
                os.fchmod(stream.fileno(), mode)
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def require_root():
    if os.geteuid() != 0:
        raise ValueError("Administrator approval is required for this operation")
    for path in [SPEC, PREFIX, DROPIN]:
        check_root_path(path)


def require_stopped():
    result = subprocess.run(
        ["/usr/bin/systemctl", "show", "droidloomd.service",
         "--property=ActiveState", "--value"],
        check=True, capture_output=True, text=True, timeout=10,
    )
    if result.stdout.strip() not in {"inactive", "failed"}:
        raise ValueError("Stop only Droidloom first: droidloomctl stop")


def dropin_bytes():
    return (
        "# Droidloom-only Ubuntu kernel-config workaround.\n"
        "[Service]\n"
        f"ExecStartPre=/usr/bin/python3 {PREFIX}/helper.py --refresh\n"
    ).encode()


def installed_manifest(verify_payloads=True):
    if not PREFIX.exists():
        return None
    state_path = PREFIX / "manifest.json"
    check_root_path(state_path)
    state = json.loads(state_path.read_bytes())
    if state.get("marker") != MARKER:
        raise ValueError(f"Refusing to change an unfamiliar directory: {PREFIX}")
    if not verify_payloads:
        return state
    for name, key in [
        ("libandroid_runtime.so", "patched_library_sha256"),
        ("kernel/gz", "compressed_config_sha256"),
        ("helper.py", "helper_sha256"),
    ]:
        path = PREFIX / name
        check_root_path(path)
        if digest(path.read_bytes()) != state[key]:
            raise ValueError(f"Installed file was changed; review it first: {path}")
    return state


def apply():
    require_root()
    require_stopped()
    spec = read_spec()
    updated = edit_spec(spec, True)
    previous = installed_manifest()
    if previous is None and any(
        entry in spec.get(key, []) for key, entry in overrides()
    ):
        raise ValueError("Found existing workaround entries without its manifest")
    if DROPIN.exists() and DROPIN.read_bytes() != dropin_bytes():
        raise ValueError(f"Refusing to replace a modified drop-in: {DROPIN}")
    library, config, manifest = payloads(spec)
    helper = Path(__file__).read_bytes()
    manifest.update(
        enabled=True, compressed_config_sha256=digest(config),
        helper_sha256=digest(helper),
    )
    # Prepare every payload before changing the service or its configuration.
    PREFIX.mkdir(mode=0o755, exist_ok=True)
    (PREFIX / "kernel").mkdir(mode=0o755, exist_ok=True)
    for path, data in [
        (PREFIX / "libandroid_runtime.so", library),
        (PREFIX / "kernel/gz", config),
        (PREFIX / "helper.py", helper),
        (PREFIX / "manifest.json", encode_json(manifest)),
    ]:
        check_root_path(path)
        atomic_write(path, data)
    DROPIN.parent.mkdir(mode=0o755, exist_ok=True)
    atomic_write(DROPIN, dropin_bytes())
    if updated != spec:
        atomic_write(SPEC, encode_json(updated))
    subprocess.run(["/usr/bin/systemctl", "daemon-reload"], check=True, timeout=20)
    print("Installed the Droidloom-only workaround. Droidloom remains stopped.")
    print("Original Android image, host libraries, kernel, and Android data are unchanged.")
    print("No Android data or image backups were created.")
    print(f"Rollback: sudo /usr/bin/python3 {PREFIX}/helper.py --remove")


def refresh():
    require_root()
    previous = installed_manifest()
    if not previous or not previous.get("enabled"):
        raise ValueError("Workaround is not installed and enabled")
    spec = read_spec()
    for key, entry in overrides():
        if entry not in spec.get(key, []):
            raise ValueError("Workaround entries changed; refusing a stale override")
    library, config, manifest = payloads(spec)
    manifest.update(
        enabled=True, compressed_config_sha256=digest(config),
        helper_sha256=previous["helper_sha256"],
    )
    # Re-extract each start so package/kernel updates cannot leave a stale copy.
    atomic_write(PREFIX / "libandroid_runtime.so", library)
    atomic_write(PREFIX / "kernel/gz", config)
    atomic_write(PREFIX / "manifest.json", encode_json(manifest))
    print(f"Droidloom kernel config refreshed for {manifest['kernel']}.")


def remove():
    require_root()
    require_stopped()
    # Disabling must still work if a generated payload is damaged or edited;
    # removal never overwrites or deletes those payloads.
    state = installed_manifest(verify_payloads=False)
    if not state:
        raise ValueError("Workaround is not installed")
    if DROPIN.exists() and DROPIN.read_bytes() != dropin_bytes():
        raise ValueError(f"Refusing to remove a modified drop-in: {DROPIN}")
    spec = read_spec()
    updated = edit_spec(spec, False)
    if updated != spec:
        atomic_write(SPEC, encode_json(updated))
    if DROPIN.exists():
        DROPIN.unlink()
    state["enabled"] = False
    atomic_write(PREFIX / "manifest.json", encode_json(state))
    subprocess.run(["/usr/bin/systemctl", "daemon-reload"], check=True, timeout=20)
    print("Removed only the two workaround entries and its service drop-in.")
    print("Unrelated configuration and all Android data are unchanged.")
    print(f"Disabled generated files remain in {PREFIX}; no backups were made.")


def check():
    spec = read_spec()
    edit_spec(spec, True)
    library, config, manifest = payloads(spec)
    print("READ-ONLY CHECK: no files or services changed.")
    print(f"Running kernel: {manifest['kernel']}")
    print(f"Original library SHA256: {manifest['original_library_sha256']}")
    print(f"Patched library SHA256:  {digest(library)}")
    print(f"Exactly one equal-length pathname replacement; ELF remains {len(library)} bytes.")
    print(f"Real host kernel config compressed to {len(config)} bytes.")
    print(f"Generated files: {PREFIX}")
    print(f"Droidloom-only changes: {SPEC} and {DROPIN}")


def diagnose():
    """Read-only Android diagnostics using init, not the unshare monitor."""
    require_root()
    result = subprocess.run(
        ["/usr/bin/systemctl", "show", "droidloomd.service",
         "--property=MainPID", "--value"],
        check=True, capture_output=True, text=True, timeout=10,
    )
    owner = int(result.stdout.strip())
    if owner <= 1:
        raise ValueError("Droidloom is not running")

    def children(pid):
        path = Path(f"/proc/{pid}/task/{pid}/children")
        return [int(value) for value in path.read_text().split()]

    candidates = []
    for monitor in children(owner):
        for child in children(monitor):
            try:
                args = Path(f"/proc/{child}/cmdline").read_bytes().split(b"\0")
                status = Path(f"/proc/{child}/status").read_text().splitlines()
                namespace_pids = next(
                    line.split()[1:] for line in status if line.startswith("NSpid:")
                )
                group = Path(f"/proc/{child}/cgroup").read_text()
                if (
                    args[:2] == [b"/init", b"second_stage"]
                    and len(namespace_pids) > 1 and namespace_pids[-1] == "1"
                    and "droidloomd.service" in group
                ):
                    candidates.append(child)
            except (FileNotFoundError, ProcessLookupError):
                continue
    if len(candidates) != 1:
        raise ValueError("Could not identify exactly one Droidloom namespace init")
    prefix = [
        "/usr/bin/timeout", "--kill-after=2s", "12s",
        "/usr/bin/nsenter", "--target", str(candidates[0]),
        "--mount", "--uts", "--ipc", "--net", "--pid", "--cgroup",
        "--root", "--wd", "--env", "--",
    ]
    for name, command in [
        ("Boot completed", ["/system/bin/getprop", "sys.boot_completed"]),
        ("Zygote", ["/system/bin/getprop", "init.svc.zygote"]),
        ("Current crash buffer", ["/system/bin/logcat", "-b", "crash", "-d", "-t", "80"]),
    ]:
        print(f"\n--- {name} ---", flush=True)
        result = subprocess.run(prefix + command, capture_output=True, timeout=16)
        print((result.stdout + result.stderr).decode(errors="replace")[-80000:])
        if result.returncode:
            print(f"Diagnostic exit code: {result.returncode}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group()
    for option in ["check", "apply", "refresh", "remove", "diagnose"]:
        group.add_argument(f"--{option}", action="store_true")
    args = parser.parse_args()
    action = next(
        (name for name in ["apply", "refresh", "remove", "diagnose"]
         if getattr(args, name)),
        "check",
    )
    try:
        globals()[action]()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Stopped without broader host changes: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
