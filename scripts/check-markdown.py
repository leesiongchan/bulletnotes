#!/usr/bin/env python3
"""Run the standalone editor's native headless checks and retain failure evidence."""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys


def main():
    root = Path(__file__).resolve().parent.parent
    output = root / "target/markdown-validation"
    output.mkdir(parents=True, exist_ok=True)
    environment = {**os.environ, "CARGO_TARGET_DIR": str(root / "target")}
    manifest = "prototypes/slint-source-highlighting/Cargo.toml"
    cargo = ["rustup", "run", "1.99.1", "cargo"]
    commands = [
        ("prepare", [sys.executable, "scripts/prepare-slint.py"]),
        ("toolchain", ["rustup", "run", "1.99.1", "rustc", "-vV"]),
        ("format", cargo + ["fmt", "--manifest-path", manifest, "--check"]),
        ("clippy", cargo + ["clippy", "--manifest-path", manifest, "--all-targets", "--locked", "--", "-D", "warnings"]),
        ("tests", cargo + ["test", "--manifest-path", manifest, "--lib", "--bins", "--tests", "--locked"]),
    ]
    summary = {"platform": platform.platform(), "python": platform.python_version(), "passed": False, "steps": []}
    summary_path = output / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n")
    for name, _ in commands:
        (output / f"{name}.log").unlink(missing_ok=True)
    if environment.get("CARGO_BUILD_TARGET"):
        raise SystemExit("Native validation requires CARGO_BUILD_TARGET to be unset")
    for name, command in commands:
        print(f"Markdown validation: {name}", flush=True)
        log_path = output / f"{name}.log"
        with log_path.open("w", encoding="utf-8") as log:
            try:
                code = subprocess.run(command, cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT).returncode
            except OSError as error:
                log.write(str(error) + "\n")
                code = 127
        summary["steps"].append({"name": name, "command": command, "exit_code": code})
        summary_path.write_text(json.dumps(summary, indent=2) + "\n")
        if code:
            print(log_path.read_text(encoding="utf-8", errors="replace"), flush=True)
            print(f"Markdown validation failed: {name}; evidence: {output}", flush=True)
            return code
    summary["passed"] = True
    summary_path.write_text(json.dumps(summary, indent=2) + "\n")
    print(f"Markdown validation passed; evidence: {output}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
