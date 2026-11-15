"""Detect lockfile changes that require a new third-party notice review."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class NoticeInventoryTests(unittest.TestCase):
    def test_every_locked_registry_package_has_the_expected_archive_checksum(self):
        packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
        expected = {(p["name"], p["version"], p["checksum"]) for p in packages
                    if p.get("source", "").startswith("registry+")}
        text = (ROOT / "THIRD_PARTY_NOTICES.txt").read_text()
        records = re.findall(
            r"(?m)^={78}\n([^\s]+) ([^\s]+)\nLicense declaration: [^\n]*\n"
            r"Repository: [^\n]*\nCrate SHA-256: ([0-9a-f]{64})$", text,
        )
        self.assertEqual(len(records), len(set(records)), "duplicate notice records")
        self.assertEqual(set(records), expected, "refresh and review notices after changing Cargo.lock")


if __name__ == "__main__":
    unittest.main()
