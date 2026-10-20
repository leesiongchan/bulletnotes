#!/usr/bin/env python3
"""Use the same maintained Slint extension as the application."""
from pathlib import Path
import runpy

runpy.run_path(str(Path(__file__).resolve().parents[2] / "scripts/prepare-slint.py"), run_name="__main__")
