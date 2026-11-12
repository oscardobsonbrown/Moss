import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest


@unittest.skipUnless(sys.platform == "darwin", "macOS app signing is required")
class InstallMacOSTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.source = self.make_app("new", "new build")
        self.target = self.make_app("installed", "old build")
        self.installer = Path(__file__).with_name("install-macos.sh")

    def make_app(self, name, text):
        app = self.root / f"{name}.app"
        binary = app / "Contents/MacOS/Moss"
        binary.parent.mkdir(parents=True)
        binary.write_text(f"#!/bin/sh\n# {text}\n")
        binary.chmod(0o755)
        with (app / "Contents/Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": "app.moss.test", "CFBundleExecutable": "Moss"}, file)
        subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True, capture_output=True)
        return app

    def run_installer(self, env=None):
        return subprocess.run([str(self.installer), str(self.source), str(self.target)], env=env, capture_output=True, text=True)

    def test_replaces_existing_app_with_verified_bundle(self):
        result = self.run_installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("new build", (self.target / "Contents/MacOS/Moss").read_text())
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(self.target)], check=True)
        self.assertEqual(list(self.root.glob(".moss-install.*")), [])

    def test_invalid_source_leaves_existing_app_intact(self):
        (self.source / "Contents/MacOS/Moss").write_text("changed after signing")
        result = self.run_installer()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("old build", (self.target / "Contents/MacOS/Moss").read_text())
        self.assertEqual(list(self.root.glob(".moss-install.*")), [])

    def test_failed_replacement_restores_existing_app(self):
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        wrapper = bin_dir / "mv"
        wrapper.write_text('#!/bin/sh\ncase "$1" in */Moss.app) exit 1;; esac\nexec /bin/mv "$@"\n')
        wrapper.chmod(0o755)
        env = {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}"}
        result = self.run_installer(env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("old build", (self.target / "Contents/MacOS/Moss").read_text())
        self.assertEqual(list(self.root.glob(".moss-install.*")), [])


if __name__ == "__main__":
    unittest.main()
