#!/usr/bin/env python3
"""Materialize checksum-verified private Slint sources before Cargo resolves paths."""
from pathlib import Path
import hashlib
import errno
import io
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import time
import urllib.request

root = Path(__file__).resolve().parent.parent
provenance = root / "patches/slint/sources.json"
patch = root / "patches/slint/slint-1.18.1.patch"
packages = json.loads(provenance.read_text())
destination = root / "target/slint-source-highlighting/crates"
stamp = destination / ".patch-sha256"
digest = hashlib.sha256(patch.read_bytes() + provenance.read_bytes() + Path(__file__).read_bytes()).hexdigest()
cargo_cache = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "registry/cache"
destination.parent.mkdir(parents=True, exist_ok=True)
# Serialize before Cargo's build lock, including the cache-hit check.
with (destination.parent / ".prepare.lock").open("a+b") as lock:
    if os.name == "nt":
        import msvcrt
        if lock.tell() == 0:
            lock.write(b"0")
            lock.flush()
        lock.seek(0)
        while True:
            try:
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                break
            except OSError as error:
                if error.errno not in (errno.EACCES, errno.EDEADLK):
                    raise
                time.sleep(0.1)
    else:
        import fcntl
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
    # Closing the file releases the OS lock, including on failure or process exit.
    if (stamp.exists() and stamp.read_text() == digest
            and all((destination / (p["name"] + "-" + p["version"]) / "Cargo.toml").exists() for p in packages)):
        raise SystemExit(0)

    with tempfile.TemporaryDirectory(prefix="prepare-", dir=destination.parent) as temporary:
        staging = Path(temporary)
        for package in packages:
            name, version = package["name"], package["version"]
            filename = f"{name}-{version}.crate"
            payload = None
            for cached in cargo_cache.glob("*/" + filename):
                candidate = cached.read_bytes()
                if hashlib.sha256(candidate).hexdigest() == package["sha256"]:
                    payload = candidate
                    break
            if payload is None:
                # Bootstrap independently: Cargo cannot fetch while path patches are absent.
                url = f"https://static.crates.io/crates/{name}/{filename}"
                print(f"Fetching pinned Slint archive: {name} {version}", flush=True)
                with urllib.request.urlopen(url, timeout=30) as response:
                    payload = response.read()
            if hashlib.sha256(payload).hexdigest() != package["sha256"]:
                raise SystemExit(f"Checksum mismatch for {filename}; refusing to apply patch")
            with tarfile.open(fileobj=io.BytesIO(payload)) as archive:
                archive.extractall(staging, filter="data")
        subprocess.run(["patch", "--batch", "--forward", "-p1", "-i", str(patch)], cwd=staging, check=True)
        # Replace only this project's generated dependency copies, never Cargo's cache.
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(staging, destination)
        stamp.write_text(digest)
    print(f"Prepared patched Slint 1.18.1: {destination}")
