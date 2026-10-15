"""Collect dependency notices for redistributable packages, without local paths."""
import json
import shutil
import subprocess
import sys
from pathlib import Path

destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--format-version", "1"]))
summary = []
for package in metadata["packages"]:
    summary.append({key: package.get(key) for key in ("name", "version", "license", "repository")})
    root = Path(package["manifest_path"]).parent
    files = {p for p in root.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "UNLICENSE"))}
    if package.get("license_file"):
        path = root / package["license_file"]
        if path.is_file():
            files.add(path)
    if files:
        folder = destination / f"{package['name']}-{package['version']}"
        folder.mkdir(exist_ok=True)
        for path in files:
            shutil.copyfile(path, folder / path.name)
(destination / "rust-dependencies.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
