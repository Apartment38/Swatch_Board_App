"""Release verification must fail closed before building when validators are absent."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ReleaseVerificationTests(unittest.TestCase):
    def test_missing_validator_stops_before_build(self):
        with tempfile.TemporaryDirectory(prefix="swatch-release-test-") as directory:
            directory = Path(directory)
            tools = directory / "tools"
            tools.mkdir()
            for tool in ("dirname", "realpath", "mktemp", "rm"):
                target = shutil.which(tool)
                assert target is not None
                (tools / tool).symlink_to(target)
            bash = shutil.which("bash")
            assert bash is not None
            marker = directory / "build-started"
            cargo = tools / "cargo"
            cargo.write_text(f"#!/bin/bash\nprintf started > '{marker}'\n")
            cargo.chmod(0o755)
            result = subprocess.run(
                [bash, str(ROOT / "packaging/verify-release.sh")],
                env={**os.environ, "PATH": str(tools)},
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Missing required release validator: desktop-file-validate", result.stderr)
            self.assertFalse(marker.exists(), "build ran before prerequisite validation")


if __name__ == "__main__":
    unittest.main()
