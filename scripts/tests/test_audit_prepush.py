"""Fixture tests for scripts/audit-prepush.ps1 release checks (no cargo, git or network)."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).parents[1] / "audit-prepush.ps1"
SHELL = shutil.which("pwsh") or shutil.which("powershell")

LIB = 'pub const PRODUCT_VERSION: &str = "v0.016-arm";\n'
UPDATER = "fn t() { assert_eq!(version(crate::PRODUCT_VERSION), Some((0, 0, 16))); }\n"
TOML = '[package]\nname = "ctdozlite"\nversion = "0.0.16"\nedition = "2021"\n'
LOCK = 'version = 4\n\n[[package]]\nname = "ctdozlite"\nversion = "0.0.16"\n'
CHANGELOG = "# Changelog\n\n## v0.016-arm — Windows 11 ARM64\n"


@unittest.skipUnless(SHELL, "PowerShell is not installed")
class PrepushReleaseTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.files = {
            "src/lib.rs": LIB,
            "src/updater.rs": UPDATER,
            "Cargo.toml": TOML,
            "Cargo.lock": LOCK,
            "CHANGELOG.md": CHANGELOG,
        }

    def run_script(self, tag):
        for name, text in self.files.items():
            if text is None:
                continue
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        result = subprocess.run(
            [SHELL, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(SCRIPT),
             "-Root", str(self.root), "-SkipCargo", "-Tag", tag],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
        )
        return result.returncode, result.stdout + result.stderr

    def test_consistent_tagged_release_passes(self):
        code, out = self.run_script("v0.016-arm")
        self.assertEqual(code, 0, out)
        for step in ["tag", "updater regression", "Cargo.toml", "Cargo.lock", "CHANGELOG"]:
            self.assertIn(f"[CHECKED] {step}", out)
        self.assertIn("[SKIPPED] cargo check", out)
        self.assertIn("[HEURISTIC] UI stub scan", out)

    def test_each_mismatch_fails_a_tagged_release(self):
        cases = {
            "tag": ("tag", None),
            "updater regression": ("src/updater.rs", UPDATER.replace("16", "15")),
            "Cargo.toml": ("Cargo.toml", TOML.replace("0.0.16", "0.0.15")),
            "Cargo.lock": ("Cargo.lock", LOCK.replace("0.0.16", "0.0.15")),
            "CHANGELOG": ("CHANGELOG.md", "# Changelog\n\n## v0.015-arm\n"),
        }
        for step, (name, text) in cases.items():
            with self.subTest(step=step):
                self.setUp()
                tag = "v0.017-arm" if name == "tag" else "v0.016-arm"
                if name != "tag":
                    self.files[name] = text
                code, out = self.run_script(tag)
                self.assertEqual(code, 1, out)
                self.assertIn(f"[FAILED] {step}", out)
                self.assertIn("Pre-push gate FAILED", out)

    def test_missing_product_version_fails(self):
        self.files["src/lib.rs"] = "// no version\n"
        code, out = self.run_script("")
        self.assertEqual(code, 1, out)
        self.assertIn("[FAILED] PRODUCT_VERSION", out)

    def test_untagged_drift_is_advisory_not_fatal(self):
        self.files["CHANGELOG.md"] = "# Changelog\n"
        code, out = self.run_script("")
        self.assertEqual(code, 0, out)
        self.assertIn("[SKIPPED] tag", out)
        self.assertIn("[HEURISTIC] CHANGELOG", out)
        self.assertIn("not verified", out)


if __name__ == "__main__":
    unittest.main()
