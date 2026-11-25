"""Read-only consistency checks; release binaries are optional and never installed."""

import hashlib
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parent
TAG = "v0.1.0-49-deb.1"
CANDIDATE = ROOT / "releases" / TAG


def load(path):
    return json.loads(path.read_text())


class ReleaseMetadataTests(unittest.TestCase):
    def setUp(self):
        self.upstream = load(ROOT / "upstream.json")
        self.inputs = load(ROOT / "packaging/inputs-49.json")
        self.provenance = load(CANDIDATE / "PROVENANCE.json")

    def test_input_packages_match_pins_and_checksum_file(self):
        self.assertEqual(self.inputs["upstream_commit"], self.upstream["commit"])
        self.assertEqual(
            self.inputs["version"], self.upstream["local_runtime_package"]["version"]
        )
        self.assertEqual(self.inputs["architecture"], "x86_64")
        self.assertEqual(
            {item["component"] for item in self.inputs["inputs"]},
            {"base", "image", "runtime"},
        )
        expected = {}
        for item in self.inputs["inputs"]:
            self.assertEqual(item["name"], Path(item["name"]).name)
            self.assertRegex(item["sha256"], r"^[0-9a-f]{64}$")
            self.assertGreater(item["bytes"], 0)
            self.assertEqual(
                item["url"],
                "https://github.com/denialwm/droidloom/releases/download/packages/"
                + item["name"],
            )
            expected[item["name"]] = item["sha256"]
        actual = {}
        for line in (ROOT / "packaging/upstream-49.sha256").read_text().splitlines():
            digest, name = line.split("  ", 1)
            actual[name] = digest
        self.assertEqual(actual, expected)

    def test_historical_release_asset_agrees_with_installer_pin(self):
        artifact = self.provenance["artifact"]
        pin = self.upstream["local_runtime_package"]
        self.assertEqual(
            artifact["name"],
            f"{pin['name']}_{pin['version']}_{pin['architecture']}.deb",
        )
        self.assertEqual(self.provenance["tag"], TAG)
        self.assertEqual(
            artifact["sha256"], self.upstream["local_runtime_package"]["sha256"]
        )
        self.assertEqual(
            (CANDIDATE / "SHA256SUMS").read_text(),
            artifact["sha256"] + "  " + artifact["name"] + "\n",
        )
        self.assertEqual(
            self.provenance["upstream"]["commit"], self.upstream["commit"]
        )

    def test_release_is_base_runtime_not_a_falsely_integrated_fix_package(self):
        artifact = self.provenance["artifact"]
        self.assertFalse(artifact["contains_compatibility_fixes"])
        self.assertEqual(artifact["compatibility_installer"], "install.sh")
        self.assertEqual(self.provenance["target"]["architecture"], "amd64")

    def test_recorded_audit_keeps_data_paths_and_modes_scoped(self):
        audit = self.provenance["payload_audit"]
        self.assertEqual(audit["all_data_paths_under"], "usr/")
        self.assertEqual(
            audit["symlinks"], {"usr/lib/droidloom/current": "runtime"}
        )
        self.assertTrue(audit["root_ownership"])
        self.assertFalse(audit["setuid_setgid_members"])
        self.assertEqual(audit["control_members"], ["control"])
        self.assertTrue(audit["regular_file_bytes_match_upstream_usr_merge"])

    def test_reference_control_uses_the_same_supported_package_identity(self):
        fields = {}
        for line in (ROOT / "packaging/reference/control").read_text().splitlines():
            if line and not line[0].isspace() and ":" in line:
                key, value = line.split(":", 1)
                fields[key] = value.strip()
        pin = self.upstream["local_runtime_package"]
        for key in ["name", "version", "architecture"]:
            control_key = {
                "name": "Package",
                "version": "Version",
                "architecture": "Architecture",
            }[key]
            self.assertEqual(fields[control_key], pin[key])

    def test_repack_validation_does_not_claim_original_archive_reproduction(self):
        validation = load(ROOT / "packaging/validation-49.json")
        self.assertEqual(validation["rootless_repack_test"], "passed")
        self.assertEqual(
            validation["upstream_inputs_verified"], len(self.inputs["inputs"])
        )
        self.assertEqual(
            validation["source_date_epoch"], self.inputs["source_date_epoch"]
        )
        self.assertEqual(
            validation["regular_payload_files_verified"],
            self.provenance["payload_audit"]["regular_files"],
        )
        self.assertNotEqual(
            validation["rebuilt_sha256"], self.provenance["artifact"]["sha256"]
        )
        self.assertFalse(validation["historical_archive_bit_for_bit_reproduction"])
        self.assertFalse(validation["installation_performed"])
        self.assertTrue(validation["historical_archive_unchanged"])

    def test_binary_staging_is_excluded_from_source_control(self):
        patterns = (ROOT / ".gitignore").read_text().splitlines()
        self.assertIn("dist/", patterns)
        self.assertIn("*.deb", patterns)
        for name in ["RELEASE_NOTES.md", "SOURCES.md", "PROVENANCE.json"]:
            self.assertTrue((CANDIDATE / name).is_file())

    def test_optional_local_release_asset_matches_its_declared_bytes(self):
        artifact = self.provenance["artifact"]
        path = ROOT / "dist/releases" / TAG / artifact["name"]
        if not path.is_file():
            self.skipTest(
                "Binary release assets are intentionally absent from source checkouts"
            )
        self.assertEqual(path.stat().st_size, artifact["bytes"])
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        self.assertEqual(digest.hexdigest(), artifact["sha256"])


if __name__ == "__main__":
    unittest.main()
