"""Build architecture-selecting NSIS installers from tested native payloads."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tomllib

parser = argparse.ArgumentParser()
parser.add_argument('makensis')
parser.add_argument('payload', type=Path)
parser.add_argument('--arch', choices=('x64_x86', 'arm64'), required=True)
args = parser.parse_args()
version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
payload = args.payload.resolve()
architectures = ['arm64'] if args.arch == 'arm64' else ['x64', 'x86']
files, folders = set(), set()
for arch in architectures:
    root = payload / f'windows-{arch}'
    assert (root / 'Tickreel.exe').is_file() and (root / 'ffmpeg.exe').is_file()
    manifest = json.loads((payload / f'update-windows-{arch}.json').read_text())
    binary = (root / 'Tickreel.exe').read_bytes()
    assert manifest['version'] == version, 'Reused payload version does not match Cargo.toml'
    assert manifest['size'] == len(binary) and manifest['sha256'] == hashlib.sha256(binary).hexdigest(), 'Payload hash mismatch'
    machine = struct.unpack_from('<H', binary, struct.unpack_from('<I', binary, 0x3c)[0] + 4)[0]
    assert machine == {'x64': 0x8664, 'x86': 0x14c, 'arm64': 0xaa64}[arch], 'Wrong payload architecture'
    for path in (root / 'licenses').rglob('*'):
        relative = str(path.relative_to(root)).replace('/', '\\')
        assert not any(c in relative for c in ('"', '$', '\n'))
        (folders if path.is_dir() else files).add(relative)
lines = [f'  Delete "$INSTDIR\\{name}"' for name in sorted(files)]
lines += [f'  RMDir "$INSTDIR\\{name}"' for name in sorted(folders | {'licenses'}, key=lambda x: (-x.count('\\'), x))]
(payload / 'uninstall-licenses.nsh').write_text('\n'.join(lines) + '\n', encoding='utf-8')
output = Path(f'dist/Tickreel-{version}-windows-{args.arch}.exe').resolve()
command = [args.makensis, f'/DVERSION={version}', f'/DPAYLOAD={payload}', f'/DOUTPUT={output}']
if args.arch == 'arm64':
    command.append('/DARM64')
command.append(str(Path('packaging/windows/installer.nsi').resolve()))
subprocess.run(command, check=True)
print(output)
