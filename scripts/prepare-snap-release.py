"""Pin Snapcraft's payload URL, checksum and version to a public release."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import time
import tomllib
import urllib.error
import urllib.request


def replace_once(recipe, field, value, original_pattern=r".*"):
    result, count = re.subn(rf"(?m)^(\s*{re.escape(field)}:) {original_pattern}$", lambda match: f"{match[1]} {value}", recipe)
    if count != 1:
        raise ValueError(f"Expected one {field} field in snapcraft.yaml")
    return result


def pin_recipe(recipe, version, url, checksum):
    recipe = replace_once(recipe, "version", f"'{version}'")
    # Standalone Snap builds use a tested local payload; Store updates pin the
    # exact published tarball instead. Leave the separate desktop part alone.
    recipe = replace_once(recipe, "source", url,
                          r"(?:https://github\.com/appipinopi/tickreel/releases/download/\S+|dist/Tickreel-linux-x86_64)")
    checksum_fields = re.findall(r"(?m)^\s*source-checksum:", recipe)
    if checksum_fields:
        return replace_once(recipe, "source-checksum", f"sha256/{checksum}")
    source_line = f"    source: {url}"
    if recipe.count(source_line) != 1:
        raise ValueError("Expected one release payload source")
    return recipe.replace(source_line, f"{source_line}\n    source-checksum: sha256/{checksum}", 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--attempts", type=int, default=1)
    args = parser.parse_args()
    if not re.fullmatch(r"v\d+\.\d+\.\d+", args.tag):
        raise ValueError("Use a stable semantic release tag such as v1.0.1")
    root = Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    if args.tag != f"v{version}":
        raise ValueError("Release tag and Cargo version differ")
    name = f"Tickreel-{version}-linux-x86_64.tar.gz"
    expected_url = f"https://github.com/appipinopi/tickreel/releases/download/{args.tag}/{name}"
    request = urllib.request.Request(f"https://api.github.com/repos/appipinopi/tickreel/releases/tags/{args.tag}", headers={"User-Agent": "Tickreel-store-publisher"})
    release = None
    asset = None
    for attempt in range(args.attempts):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                release = json.load(response)
            asset = next((item for item in release.get("assets", []) if item["name"] == name), None)
            if asset:
                break
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
        if attempt + 1 < args.attempts:
            print("Waiting for the Linux release asset", flush=True)
            time.sleep(30)
    if not release or not asset:
        raise ValueError("The Linux payload has not been attached to the release")
    if release.get("draft") or release.get("prerelease") or asset["browser_download_url"] != expected_url:
        raise ValueError("Use a published, non-prerelease Tickreel release")
    digest = hashlib.sha256()
    with urllib.request.urlopen(expected_url, timeout=180) as response:
        while chunk := response.read(1024 * 1024):
            digest.update(chunk)
    checksum = digest.hexdigest()
    if asset.get("digest") and asset["digest"] != f"sha256:{checksum}":
        raise ValueError("Downloaded payload does not match the GitHub asset digest")
    path = root / "snap/snapcraft.yaml"
    recipe = path.read_text(encoding="utf-8")
    recipe = pin_recipe(recipe, version, expected_url, checksum)
    path.write_text(recipe, encoding="utf-8")
    print(f"Pinned Snap payload {version}: sha256/{checksum}")


if __name__ == "__main__":
    main()
