import copy
from contextlib import redirect_stdout
import gzip
import io
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import ubuntu_kernel_config as workaround

TEMP_ROOT = "/tmp/opencode" if Path("/tmp/opencode").is_dir() else None


class LibraryTests(unittest.TestCase):
    def original(self):
        header = bytearray(64)
        header[:6] = b"\x7fELF\x02\x01"
        header[18:20] = b"\x3e\x00"
        struct.pack_into("<Q", header, 32, 64)
        struct.pack_into("<HH", header, 54, 56, 1)
        data = b"before" + workaround.ORIGINAL_PATH + b"after"
        segment = struct.pack("<IIQQQQQQ", 1, 4, 120, 4096, 0, len(data), len(data), 4096)
        return bytes(header) + segment + data

    def test_only_one_equal_length_path_changes(self):
        original = self.original()
        changed = workaround.patch_library(original)
        self.assertEqual(len(original), len(changed))
        self.assertEqual(
            changed.replace(workaround.REPLACEMENT_PATH, workaround.ORIGINAL_PATH),
            original,
        )
        self.assertEqual(changed[:126], original[:126])
        self.assertTrue(changed.endswith(b"after"))

    def test_missing_path_is_refused(self):
        with self.assertRaises(ValueError):
            workaround.patch_library(self.original().replace(
                workaround.ORIGINAL_PATH, workaround.REPLACEMENT_PATH
            ))

    def test_multiple_paths_are_refused(self):
        with self.assertRaises(ValueError):
            workaround.patch_library(self.original() + workaround.ORIGINAL_PATH)

    def test_host_or_wrong_architecture_library_is_refused(self):
        for header in [b"not an ELF", b"\x7fELF\x01\x01", b"\x7fELF\x02\x02"]:
            with self.subTest(header=header), self.assertRaises(ValueError):
                workaround.patch_library(header + self.original()[len(header):])
        wrong_architecture = bytearray(self.original())
        wrong_architecture[18:20] = b"\xb7\x00"
        with self.assertRaises(ValueError):
            workaround.patch_library(bytes(wrong_architecture))

    def test_executable_or_writable_segments_are_refused(self):
        for flags in [5, 6, 7]:
            original = bytearray(self.original())
            struct.pack_into("<I", original, 68, flags)
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                workaround.patch_library(bytes(original))

    def test_invalid_program_headers_are_refused(self):
        original = bytearray(self.original())
        struct.pack_into("<Q", original, 32, len(original))
        with self.assertRaises(ValueError):
            workaround.patch_library(bytes(original))


class ConfigurationTests(unittest.TestCase):
    def spec(self):
        return {
            "android_file_overrides": [
                {"source": "/original/libbinder.so", "target": "/system/lib64/libbinder.so"}
            ],
            "android_runtime_directories": [
                {"source": "/original/ime", "target": "/droidloom/ime"}
            ],
            "gpu": "intel",
            "user_owned_settings": {"keep": [1, 2, 3]},
            "data_dir": "/unchanged/android/data",
        }

    def test_enable_disable_preserves_all_existing_configuration(self):
        original = self.spec()
        saved = copy.deepcopy(original)
        enabled = workaround.edit_spec(original, True)
        self.assertEqual(original, saved)
        self.assertEqual(enabled["user_owned_settings"], saved["user_owned_settings"])
        self.assertEqual(enabled["data_dir"], saved["data_dir"])
        self.assertEqual(workaround.edit_spec(enabled, False), saved)

    def test_enable_is_idempotent(self):
        enabled = workaround.edit_spec(self.spec(), True)
        self.assertEqual(workaround.edit_spec(enabled, True), enabled)

    def test_conflicting_library_is_never_overwritten(self):
        original = self.spec()
        original["android_file_overrides"].append({
            "source": "/someone/elses/library.so",
            "target": workaround.LIBRARY_TARGET,
        })
        saved = copy.deepcopy(original)
        for enable in [True, False]:
            with self.subTest(enable=enable), self.assertRaises(ValueError):
                workaround.edit_spec(original, enable)
        self.assertEqual(original, saved)

    def test_overlapping_runtime_mounts_are_refused(self):
        for target in ["/droidloom", "/droidloom/k", "/droidloom/k/nested"]:
            original = self.spec()
            original["android_runtime_directories"].append({
                "source": "/someone/elses/directory", "target": target,
            })
            with self.subTest(target=target), self.assertRaises(ValueError):
                workaround.edit_spec(original, True)

    def test_removal_preserves_later_user_changes(self):
        enabled = workaround.edit_spec(self.spec(), True)
        enabled["gpu"] = "nvidia"
        enabled["new_setting"] = True
        disabled = workaround.edit_spec(enabled, False)
        self.assertEqual(disabled["gpu"], "nvidia")
        self.assertIs(disabled["new_setting"], True)


class FileTests(unittest.TestCase):
    def test_atomic_update_preserves_permissions(self):
        with tempfile.TemporaryDirectory(dir=TEMP_ROOT) as folder:
            path = Path(folder) / "cell.json"
            path.write_text("old")
            path.chmod(0o640)
            workaround.atomic_write(path, workaround.encode_json({"new": True}))
            self.assertEqual(json.loads(path.read_bytes()), {"new": True})
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o640)
            self.assertEqual(len(list(Path(folder).iterdir())), 1)

    def test_symlink_destinations_are_not_followed(self):
        with tempfile.TemporaryDirectory(dir=TEMP_ROOT) as folder:
            target = Path(folder) / "untouched"
            target.write_bytes(b"original")
            link = Path(folder) / "link"
            link.symlink_to(target)
            with self.assertRaises(ValueError):
                workaround.atomic_write(link, b"replacement")
            self.assertEqual(target.read_bytes(), b"original")

    def test_user_writable_privileged_paths_are_refused(self):
        with tempfile.TemporaryDirectory(dir=TEMP_ROOT) as folder:
            path = Path(folder)
            path.chmod(0o777)
            with self.assertRaises(ValueError):
                workaround.check_root_path(path / "output")

    def test_non_root_apply_does_not_write(self):
        with patch.object(os, "geteuid", return_value=1000):
            with self.assertRaises(ValueError):
                workaround.require_root()


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.enterContext(redirect_stdout(io.StringIO()))
        self.temporary = tempfile.TemporaryDirectory(dir=TEMP_ROOT)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.spec = self.root / "cell.json"
        self.prefix = self.root / "generated"
        self.dropin = self.root / "dropins" / "workaround.conf"
        self.original = ConfigurationTests().spec()
        self.spec.write_bytes(workaround.encode_json(self.original))
        self.patches = [
            patch.object(workaround, "SPEC", self.spec),
            patch.object(workaround, "PREFIX", self.prefix),
            patch.object(workaround, "DROPIN", self.dropin),
            patch.object(workaround, "require_root"),
            patch.object(workaround, "check_root_path"),
            patch.object(workaround.subprocess, "run", side_effect=self.run_command),
            patch.object(workaround, "payloads", side_effect=self.payloads),
        ]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)
        self.commands = []

    def run_command(self, command, **kwargs):
        self.commands.append(command)
        if command == [
            "/usr/bin/systemctl", "show", "droidloomd.service",
            "--property=ActiveState", "--value",
        ]:
            return subprocess.CompletedProcess(command, 0, "inactive\n", "")
        if command == ["/usr/bin/systemctl", "daemon-reload"]:
            return subprocess.CompletedProcess(command, 0)
        self.fail(f"Unexpected command, no host service operations allowed: {command}")

    def payloads(self, spec):
        library = workaround.patch_library(LibraryTests().original())
        config = gzip.compress(b"CONFIG_VMAP_STACK=y\n", mtime=0)
        return library, config, {
            "marker": workaround.MARKER,
            "kernel": "test-kernel",
            "patched_library_sha256": workaround.digest(library),
        }

    def test_install_refresh_remove_preserve_other_settings(self):
        workaround.apply()
        self.assertTrue(self.dropin.is_file())
        current = json.loads(self.spec.read_bytes())
        for key, entry in workaround.overrides():
            self.assertIn(entry, current[key])
        workaround.refresh()
        current["new_user_setting"] = "keep me"
        self.spec.write_bytes(workaround.encode_json(current))
        workaround.remove()
        expected = dict(self.original, new_user_setting="keep me")
        self.assertEqual(json.loads(self.spec.read_bytes()), expected)
        self.assertFalse(self.dropin.exists())
        self.assertFalse(json.loads((self.prefix / "manifest.json").read_bytes())["enabled"])
        self.assertTrue(self.commands)

    def test_preflight_failure_makes_no_configuration_changes(self):
        with patch.object(workaround, "payloads", side_effect=ValueError("refuse")):
            with self.assertRaises(ValueError):
                workaround.apply()
        self.assertEqual(json.loads(self.spec.read_bytes()), self.original)
        self.assertFalse(self.prefix.exists())
        self.assertFalse(self.dropin.exists())

    def test_edited_generated_files_are_not_overwritten(self):
        workaround.apply()
        library = self.prefix / "libandroid_runtime.so"
        library.write_bytes(b"a later user edit")
        current = self.spec.read_bytes()
        with self.assertRaises(ValueError):
            workaround.refresh()
        self.assertEqual(library.read_bytes(), b"a later user edit")
        self.assertEqual(self.spec.read_bytes(), current)
        workaround.remove()
        self.assertEqual(library.read_bytes(), b"a later user edit")
        self.assertFalse(self.dropin.exists())

    def test_edited_dropin_is_not_removed(self):
        workaround.apply()
        self.dropin.write_bytes(b"a later user edit")
        current = self.spec.read_bytes()
        with self.assertRaises(ValueError):
            workaround.remove()
        self.assertEqual(self.dropin.read_bytes(), b"a later user edit")
        self.assertEqual(self.spec.read_bytes(), current)


if __name__ == "__main__":
    unittest.main()
