"""Sign a GitHub release executable; the private key never belongs in the repository."""
import argparse
import base64
import hashlib
import json
import os
import struct
from pathlib import Path
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

parser = argparse.ArgumentParser()
parser.add_argument("executable", type=Path)
parser.add_argument("version")
parser.add_argument("output", type=Path)
parser.add_argument("--arch", choices=("x64", "x86", "arm64"), default="x64")
args = parser.parse_args()
encoded = os.environ.get("TICKREEL_UPDATE_SIGNING_KEY")
raw = (base64.b64decode(encoded.strip(), validate=True) if encoded else
       (Path(os.environ["LOCALAPPDATA"]) / "TickreelPublisher/update-signing-key.bin").read_bytes())
key = Ed25519PrivateKey.from_private_bytes(raw)
public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
assert public == (Path(__file__).resolve().parent.parent / "assets/update-public-key.bin").read_bytes(), "Signing key does not match the application public key"
data = args.executable.read_bytes()
assert data[:2] == b"MZ", "Expected a Windows executable"
pe_offset = struct.unpack_from('<I', data, 0x3c)[0]
assert data[pe_offset:pe_offset + 4] == b'PE\x00\x00', 'Invalid PE executable'
assert struct.unpack_from('<H', data, pe_offset + 4)[0] == {'x64': 0x8664, 'x86': 0x14c, 'arm64': 0xaa64}[args.arch], 'Executable architecture does not match the update channel'
assert args.version and all(c.isdigit() or c == "." for c in args.version)
filename = f"Tickreel-{args.version}-windows-{args.arch}-portable.exe"
value = dict(version=args.version,
             url=f"https://github.com/appipinopi/tickreel/releases/download/v{args.version}/{filename}",
             size=len(data), sha256=hashlib.sha256(data).hexdigest())
signed = f"Tickreel update\n{value['version']}\n{value['url']}\n{value['size']}\n{value['sha256']}\n"
value["signature"] = base64.b64encode(key.sign(signed.encode())).decode()
args.output.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
print(f"Signed update metadata for {args.version}")
