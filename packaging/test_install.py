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
        self.effect = self.root / "effect.so"
        self.effect.write_bytes(b"test effect")
        self.stage = self.root / "stage"
        self.rust_notices = self.root / "rust-notices"
        (self.rust_notices / "licenses").mkdir(parents=True)
        (self.rust_notices / "COPYRIGHT-library.html").write_text("standard library copyright")
        (self.rust_notices / "licenses/MIT.txt").write_text("test license")
        (self.rust_notices / "licenses/Apache-2.0.txt").write_text("test Apache license")

    def install(self, env_overrides=None, omit_rust_license_dir=False):
        env = {
            **os.environ,
            "PREFIX": "/usr/local",
            "DESTDIR": str(self.stage),
            "BINDIR": str(self.bindir),
            "EFFECT_LIB": str(self.effect),
            "QT_PLUGIN_DIR": "/opt/qt/plugins",
        }
        if omit_rust_license_dir:
            env.pop("RUST_LICENSE_DIR", None)
        else:
            env["RUST_LICENSE_DIR"] = str(self.rust_notices)
        if env_overrides:
            env.update(env_overrides)
        return subprocess.run(
            ["bash", str(ROOT / "packaging/install.sh")],
            env=env,
            capture_output=True, text=True, check=False,
        )

    def make_fake_rustc_path(self, sysroot: Path) -> str:
        fake_bin = self.root / f"fake-rustc-{sysroot.name}"
        fake_bin.mkdir(exist_ok=True)
        rustc = fake_bin / "rustc"
        rustc.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = \"--print\" ] && [ \"$2\" = \"sysroot\" ]; then\n"
            f"  printf '%s\\n' \"{sysroot}\"\n"
            "  exit 0\n"
            "fi\n"
            "exit 1\n"
        )
        rustc.chmod(0o755)
        return f"{fake_bin}:{os.environ.get('PATH', '')}"

    def test_installs_required_worker_without_desktop_launcher(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        prefix = self.stage / "usr/local"
        self.assertEqual(sorted(p.name for p in (prefix / "bin").iterdir()), sorted(BINARIES))
        effect = self.stage / "opt/qt/plugins/kwin/effects/plugins/swatchboardpicker.so"
        self.assertEqual(effect.read_bytes(), self.effect.read_bytes())
        self.assertEqual((prefix / "share/licenses/swatch-board/KWIN-LICENSE").read_bytes(),
                         (ROOT / "kwin/LICENSE").read_bytes())
        worker = prefix / "bin/swatch-board-theme-worker"
        self.assertTrue(os.access(worker, os.X_OK))
        entries = list((prefix / "share/applications").glob("*.desktop"))
        self.assertEqual(len(entries), 5)
        self.assertFalse(any("theme-worker" in p.read_text() for p in entries))
        tray = prefix / "share/applications/org.apartment38.SwatchBoard.Tray.desktop"
        self.assertIn('Exec="/usr/local/bin/swatch-board-tray"', tray.read_text())
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

    def test_missing_effect_fails_before_writing_stage(self):
        self.effect.unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing KWin effect", result.stderr)
        self.assertFalse(self.stage.exists())

    def test_arch_distro_rust_sysroot_layout_installs_licenses(self):
        sysroot = self.root / "arch-sysroot"
        (sysroot / "share/licenses/rust").mkdir(parents=True)
        (sysroot / "share/doc/rustc/licenses").mkdir(parents=True)
        (sysroot / "share/licenses/rust/COPYRIGHT-library.html.rustc").write_text(
            "arch rust stdlib copyright"
        )
        (sysroot / "share/doc/rustc/licenses/MIT.txt").write_text("arch MIT text")
        (sysroot / "share/doc/rustc/licenses/Apache-2.0.txt").write_text("arch Apache text")
        (sysroot / "share/doc/rustc/licenses/Unicode-3.0.txt").write_text("arch Unicode text")

        result = self.install(
            env_overrides={"PATH": self.make_fake_rustc_path(sysroot)},
            omit_rust_license_dir=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        installed_rust = self.stage / "usr/local/share/licenses/swatch-board/rust-standard-library"
        self.assertEqual(
            (installed_rust / "COPYRIGHT-library.html").read_text(),
            "arch rust stdlib copyright",
        )
        self.assertEqual((installed_rust / "licenses/MIT.txt").read_text(), "arch MIT text")
        self.assertEqual(
            (installed_rust / "licenses/Apache-2.0.txt").read_text(),
            "arch Apache text",
        )
        self.assertEqual(
            (installed_rust / "licenses/Unicode-3.0.txt").read_text(),
            "arch Unicode text",
        )

    def test_arch_distro_rust_sysroot_layout_without_rustc_suffix(self):
        sysroot = self.root / "arch-sysroot-plain"
        (sysroot / "share/licenses/rust").mkdir(parents=True)
        (sysroot / "share/doc/rustc/licenses").mkdir(parents=True)
        (sysroot / "share/licenses/rust/COPYRIGHT-library.html").write_text(
            "plain arch copyright"
        )
        (sysroot / "share/doc/rustc/licenses/MIT.txt").write_text("plain MIT")
        (sysroot / "share/doc/rustc/licenses/Apache-2.0.txt").write_text("plain Apache")

        result = self.install(
            env_overrides={"PATH": self.make_fake_rustc_path(sysroot)},
            omit_rust_license_dir=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        installed_rust = self.stage / "usr/local/share/licenses/swatch-board/rust-standard-library"
        self.assertEqual(
            (installed_rust / "COPYRIGHT-library.html").read_text(),
            "plain arch copyright",
        )
        self.assertEqual((installed_rust / "licenses/MIT.txt").read_text(), "plain MIT")

    def test_standard_rustup_sysroot_layout_auto_discovery(self):
        sysroot = self.root / "rustup-sysroot"
        (sysroot / "share/doc/rust/licenses").mkdir(parents=True)
        (sysroot / "share/doc/rust/COPYRIGHT-library.html").write_text("rustup copyright")
        (sysroot / "share/doc/rust/licenses/MIT.txt").write_text("rustup MIT")
        (sysroot / "share/doc/rust/licenses/Apache-2.0.txt").write_text("rustup Apache")

        result = self.install(
            env_overrides={"PATH": self.make_fake_rustc_path(sysroot)},
            omit_rust_license_dir=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        installed_rust = self.stage / "usr/local/share/licenses/swatch-board/rust-standard-library"
        self.assertEqual(
            (installed_rust / "COPYRIGHT-library.html").read_text(),
            "rustup copyright",
        )
        self.assertEqual((installed_rust / "licenses/MIT.txt").read_text(), "rustup MIT")

    def test_explicit_invalid_rust_license_dir_does_not_fallback_to_sysroot(self):
        sysroot = self.root / "arch-sysroot-valid"
        (sysroot / "share/licenses/rust").mkdir(parents=True)
        (sysroot / "share/doc/rustc/licenses").mkdir(parents=True)
        (sysroot / "share/licenses/rust/COPYRIGHT-library.html.rustc").write_text("valid")
        (sysroot / "share/doc/rustc/licenses/MIT.txt").write_text("MIT")
        (sysroot / "share/doc/rustc/licenses/Apache-2.0.txt").write_text("Apache")

        result = self.install(
            env_overrides={
                "PATH": self.make_fake_rustc_path(sysroot),
                "RUST_LICENSE_DIR": str(self.root / "does-not-exist"),
            },
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing Rust standard library notices", result.stderr)
        self.assertFalse(self.stage.exists())

    def test_empty_sysroot_fails_before_writing_stage(self):
        sysroot = self.root / "empty-sysroot"
        sysroot.mkdir()
        result = self.install(
            env_overrides={"PATH": self.make_fake_rustc_path(sysroot)},
            omit_rust_license_dir=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing Rust standard library notices", result.stderr)
        self.assertFalse(self.stage.exists())


if __name__ == "__main__":
    unittest.main()
