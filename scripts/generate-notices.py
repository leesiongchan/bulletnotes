#!/usr/bin/env python3
"""Render runtime notices and cover Slint's custom license with verified full text."""
import hashlib
import html
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import tomllib

root = pathlib.Path(__file__).resolve().parent.parent
config = tomllib.loads((root / "packaging/about.toml").read_text())
metadata = json.loads(subprocess.check_output(
    ["rustup", "run", "1.99.1", "cargo", "metadata", "--locked", "--filter-platform", "aarch64-apple-darwin", "--format-version", "1"], cwd=root))
packages = {(p["name"], p["version"]): p for p in metadata["packages"]}
with tempfile.TemporaryDirectory(prefix="bulletnotes-notices-") as temporary:
    target = pathlib.Path(temporary) / "notices.html"
    result = subprocess.run(["rustup", "run", "1.99.1", "cargo", "about", "generate", "--locked",
        "--config", "packaging/about.toml", "--output-file", str(target), "packaging/notices.hbs"],
        cwd=root, capture_output=True, text=True)
    if result.returncode:
        sys.stderr.write(result.stderr)
        sys.exit(result.returncode)
    warnings = set()
    for line in result.stderr.splitlines():
        if "WARN" not in line:
            continue
        match = re.search(r"LicenseRef-Slint-Royalty-free-2.0 has no license file for crate '([^ ]+) ([^']+)'", line)
        if not match:
            sys.exit("Uncovered attribution warning: " + line)
        warnings.add(match.groups())
    license_file = root / "packaging/Slint-License.md"
    full_text = license_file.read_bytes()
    components = []
    for name, version in sorted(warnings):
        package = packages[(name, version)]
        if "LicenseRef-Slint-Royalty-free-2.0" not in package["license"]:
            sys.exit("Slint license grant changed; review attribution")
        clarification = config[name]["clarify"]
        reference = clarification["files"][0]
        source = pathlib.Path(package["manifest_path"]).parent / reference["path"]
        source_bytes = source.read_bytes()
        if hashlib.sha256(source_bytes).hexdigest() != reference["checksum"] or source_bytes != full_text:
            sys.exit("Slint license text changed; review attribution")
        components.append(name + " " + version)
    # Git-sourced crates contain nested crates, so cargo-about counts their identical license twice.
    rendered = re.sub(r"(?<=<p>)[^<]*(?=</p>)",
        lambda m: "; ".join(dict.fromkeys(m.group().split("; "))), target.read_text())
    if components:
        supplement = "<section><h2>Slint Royalty-free License 2.0</h2><p>" + html.escape("; ".join(components)) + "</p><pre>" + html.escape(full_text.decode()) + "</pre></section>"
        rendered = rendered.replace("</html>", supplement + "</html>")
    destination = root / "packaging/ThirdPartyNotices.html"
    destination.write_text(rendered)
    print("Runtime notices generated; covered " + str(len(components)) + " custom Slint license warnings with hash-verified full text.")
