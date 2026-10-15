"""Sign a GitHub release executable; the private key never belongs in the repository."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

parser = argparse.ArgumentParser()
parser.add_argument("executable", type=Path)
parser.add_argument("version")
parser.add_argument("output", type=Path)
args = parser.parse_args()
encoded = os.environ.get("TICKREEL_UPDATE_SIGNING_KEY")
raw = (base64.b64decode(encoded, validate=True) if encoded else
       (Path(os.environ["LOCALAPPDATA"]) / "TickreelPublisher/update-signing-key.bin").read_bytes())
key = Ed25519PrivateKey.from_private_bytes(raw)
public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
assert public == (Path(__file__).resolve().parent.parent / "assets/update-public-key.bin").read_bytes(), "Signing key does not match the application public key"
data = args.executable.read_bytes()
assert data[:2] == b"MZ", "Expected a Windows executable"
assert args.version and all(c.isdigit() or c == "." for c in args.version)
value = dict(version=args.version,
             url=f"https://github.com/appipinopi/tickreel/releases/download/v{args.version}/Tickreel-Rust.exe",
             size=len(data), sha256=hashlib.sha256(data).hexdigest())
signed = f"Tickreel update\n{value['version']}\n{value['url']}\n{value['size']}\n{value['sha256']}\n"
value["signature"] = base64.b64encode(key.sign(signed.encode())).decode()
args.output.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
print(f"Signed update metadata for {args.version}")
