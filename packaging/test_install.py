"""Installer contract tests; dummy executables are never executed."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARIES = (
    "swatch-board-picker", "swatch-board-tray", "swatch-board-history",
    "swatch-board-config", "swatch-board", "swatch-board-theme-worker",
)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="swatch-install-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bindir = self.root / "build"
        self.bindir.mkdir()
        for name in BINARIES:
            path = self.bindir / name
            path.write_text("#!/bin/sh\nexit 99\n")
            path.chmod(0o755)
        self.stage = self.root / "stage"
        self.rust_notices = self.root / "rust-notices"
        (self.rust_notices / "licenses").mkdir(parents=True)
        (self.rust_notices / "COPYRIGHT-library.html").write_text("standard library copyright")
        (self.rust_notices / "licenses/MIT.txt").write_text("test license")
        (self.rust_notices / "licenses/Apache-2.0.txt").write_text("test Apache license")

    def install(self):
        return subprocess.run(
            ["bash", str(ROOT / "packaging/install.sh")],
            env={**os.environ, "PREFIX": "/usr/local", "DESTDIR": str(self.stage),
                 "BINDIR": str(self.bindir), "RUST_LICENSE_DIR": str(self.rust_notices)},
            capture_output=True, text=True, check=False,
        )

    def test_installs_required_worker_without_desktop_launcher(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        prefix = self.stage / "usr/local"
        self.assertEqual(sorted(p.name for p in (prefix / "bin").iterdir()), sorted(BINARIES))
        worker = prefix / "bin/swatch-board-theme-worker"
        self.assertTrue(os.access(worker, os.X_OK))
        entries = list((prefix / "share/applications").glob("*.desktop"))
        self.assertEqual(len(entries), 5)
        self.assertFalse(any("theme-worker" in p.read_text() for p in entries))
        metainfo = prefix / "share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml"
        self.assertTrue(metainfo.is_file())
        self.assertIn("<id>org.apartment38.SwatchBoard.Board</id>", metainfo.read_text())
        self.assertTrue((prefix / "share/swatch-board/plugins/example-terminal/manifest.json").is_file())
        notices = prefix / "share/licenses/swatch-board/THIRD_PARTY_NOTICES.txt"
        self.assertEqual(notices.read_bytes(), (ROOT / "THIRD_PARTY_NOTICES.txt").read_bytes())
        installed_rust = prefix / "share/licenses/swatch-board/rust-standard-library"
        self.assertEqual((installed_rust / "COPYRIGHT-library.html").read_text(), "standard library copyright")
        self.assertEqual((installed_rust / "licenses/MIT.txt").read_text(), "test license")

    def test_missing_rust_notices_fails_before_writing_stage(self):
        (self.rust_notices / "COPYRIGHT-library.html").unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing Rust standard library notices", result.stderr)
        self.assertFalse(self.stage.exists())

    def test_missing_rust_license_text_fails_before_writing_stage(self):
        (self.rust_notices / "licenses/Apache-2.0.txt").unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing Rust standard library notices", result.stderr)
        self.assertFalse(self.stage.exists())

    def test_missing_worker_fails_before_writing_stage(self):
        (self.bindir / "swatch-board-theme-worker").unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("swatch-board-theme-worker", result.stderr)
        self.assertFalse(self.stage.exists())


if __name__ == "__main__":
    unittest.main()
