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

    def install(self):
        return subprocess.run(
            ["bash", str(ROOT / "packaging/install.sh")],
            env={**os.environ, "PREFIX": "/usr/local", "DESTDIR": str(self.stage),
                 "BINDIR": str(self.bindir)},
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

    def test_missing_worker_fails_before_writing_stage(self):
        (self.bindir / "swatch-board-theme-worker").unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("swatch-board-theme-worker", result.stderr)
        self.assertFalse(self.stage.exists())


if __name__ == "__main__":
    unittest.main()
