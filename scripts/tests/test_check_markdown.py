"""Check that CI failures cannot be reported as successful probe validation."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "check-markdown.py"


@unittest.skipIf(os.name == "nt", "Failure injection uses a POSIX executable fixture.")
class CheckMarkdownFailures(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "scripts").mkdir()

    def run_check(self, failure):
        shutil.copyfile(SCRIPT, self.root / "scripts/check-markdown.py")
        # Real subprocesses record calls; no Rust compilation is needed to test failure propagation.
        (self.root / "scripts/prepare-slint.py").write_text(
            "import sys\nsys.exit(23 if " + repr(failure == "prepare") + " else 0)\n"
        )
        fake = self.root / "rustup"
        fake.write_text(
            "#!/usr/bin/env python3\nimport json, pathlib, sys\n"
            "with pathlib.Path('calls.jsonl').open('a') as f: f.write(json.dumps(sys.argv[1:])+'\\n')\n"
            "print('generated validation failure' if 'clippy' in sys.argv else 'ok')\n"
            "sys.exit(17 if 'clippy' in sys.argv else 0)\n"
        )
        fake.chmod(0o755)
        environment = {**os.environ, "PATH": str(self.root) + os.pathsep + os.environ["PATH"]}
        environment.pop("CARGO_BUILD_TARGET", None)
        return subprocess.run(
            [sys.executable, self.root / "scripts/check-markdown.py"],
            cwd=self.root, env=environment, capture_output=True, text=True,
        )

    def test_prepare_failure_is_recorded_before_any_cargo_command(self):
        result = self.run_check("prepare")
        self.assertEqual(result.returncode, 23)
        self.assertFalse((self.root / "calls.jsonl").exists())
        summary = json.loads((self.root / "target/markdown-validation/summary.json").read_text())
        self.assertFalse(summary["passed"])
        self.assertEqual(summary["steps"][-1]["name"], "prepare")
        self.assertEqual(summary["steps"][-1]["exit_code"], 23)

    def test_clippy_failure_preserves_diagnostics_and_skips_tests(self):
        output = self.root / "target/markdown-validation"
        output.mkdir(parents=True)
        (output / "tests.log").write_text("previous successful test run")
        result = self.run_check("clippy")
        self.assertEqual(result.returncode, 17)
        calls = [json.loads(line) for line in (self.root / "calls.jsonl").read_text().splitlines()]
        self.assertTrue(any("clippy" in call for call in calls))
        self.assertFalse(any("test" in call for call in calls))
        output = self.root / "target/markdown-validation"
        self.assertFalse((output / "tests.log").exists(), "Skipped stages must not retain stale success logs")
        self.assertIn("generated validation failure", (output / "clippy.log").read_text())
        summary = json.loads((output / "summary.json").read_text())
        self.assertFalse(summary["passed"])
        self.assertEqual(summary["steps"][-1]["exit_code"], 17)


if __name__ == "__main__":
    unittest.main()
