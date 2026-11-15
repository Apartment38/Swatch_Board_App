"""Arch recipe contracts; does not fetch sources or install packages."""
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ArchRecipeTests(unittest.TestCase):
    def test_native_and_vcs_build_prerequisites_are_declared(self):
        result = subprocess.run(
            ["bash", "-c", 'source "$1"; printf "%s\\n" "${makedepends[@]}"',
             "bash", str(ROOT / "packaging/arch/PKGBUILD")],
            capture_output=True, text=True, check=True,
        )
        dependencies = set(result.stdout.splitlines())
        self.assertTrue({"git", "cargo", "pkgconf", "wayland", "libxkbcommon"}
                        <= dependencies, dependencies)

    def test_desktop_runtime_dependencies_are_declared(self):
        result = subprocess.run(
            ["bash", "-c", 'source "$1"; printf "%s\\n" "${depends[@]}"',
             "bash", str(ROOT / "packaging/arch/PKGBUILD")],
            capture_output=True, text=True, check=True,
        )
        dependencies = set(result.stdout.splitlines())
        self.assertTrue({"hicolor-icon-theme", "kwin"} <= dependencies, dependencies)
        self.assertNotIn("pipewire", dependencies)

    def test_cross_language_objects_do_not_use_gcc_lto(self):
        result = subprocess.run(
            ["bash", "-c", 'source "$1"; printf "%s\\n" "${options[@]}"',
             "bash", str(ROOT / "packaging/arch/PKGBUILD")],
            capture_output=True, text=True, check=True,
        )
        self.assertIn("!lto", result.stdout.splitlines())

    @unittest.skipUnless(shutil.which("makepkg"), "Arch makepkg is not installed")
    def test_srcinfo_matches_recipe(self):
        directory = ROOT / "packaging/arch"
        result = subprocess.run(["makepkg", "--printsrcinfo"], cwd=directory,
                                capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout, (directory / ".SRCINFO").read_text())

    def test_pinned_source_contains_release_payload(self):
        result = subprocess.run(
            ["bash", "-c", 'source "$1"; printf "%s" "$_commit"',
             "bash", str(ROOT / "packaging/arch/PKGBUILD")],
            capture_output=True, text=True, check=True,
        )
        commit = result.stdout
        self.assertRegex(commit, r"^[0-9a-f]{40}$")
        for path in ("share/metainfo/org.apartment38.SwatchBoard.Board.metainfo.xml",
                     "plugins/example-terminal/manifest.json", "packaging/verify-release.sh"):
            subprocess.run(["git", "cat-file", "-e", f"{commit}:{path}"], cwd=ROOT, check=True)


if __name__ == "__main__":
    unittest.main()
