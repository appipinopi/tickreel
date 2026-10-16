"""Package the native Windows x64 OBS module and its corresponding source."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--gallery', type=Path, required=True, help='Tickreel executable providing the standalone app-gallery mode')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
assert version == tomllib.loads((root / 'obs-plugin/Cargo.toml').read_text())['package']['version']
dist = root / 'dist'
dist.mkdir(exist_ok=True)
stage = dist / 'obs-windows-x64'
dll = stage / 'obs-plugins/64bit/tickreel-obs.dll'
dll.parent.mkdir(parents=True, exist_ok=True)
data = args.binary.read_bytes()
assert data[:2] == b'MZ', 'Expected a Windows DLL'
offset = int.from_bytes(data[0x3c:0x40], 'little')
assert data[offset:offset + 4] == b'PE\x00\x00'
assert int.from_bytes(data[offset + 4:offset + 6], 'little') == 0x8664, 'Expected x64 DLL'
shutil.copyfile(args.binary, dll)
shutil.copyfile(args.gallery, dll.parent / 'tickreel-gallery.exe')
shutil.copyfile(root / 'obs-plugin/README.md', stage / 'README.md')
shutil.copyfile(root / 'obs-plugin/LICENSE', stage / 'LICENSE')
shutil.copyfile(root / 'obs-plugin/install-windows.ps1', stage / 'install-windows.ps1')
subprocess.run(['python', str(root / 'scripts/collect-licenses.py'), str(stage / 'licenses')],
               cwd=root / 'obs-plugin', check=True)
subprocess.run(['python', str(root / 'scripts/collect-licenses.py'), str(stage / 'licenses/desktop-helper')],
               cwd=root, check=True)
archive = dist / f'Tickreel-OBS-{version}-windows-x64.zip'
with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as output:
    for file in sorted(stage.rglob('*')):
        if file.is_file():
            output.write(file, file.relative_to(stage))
# GPL corresponding source includes the shared model/store modules used by the DLL.
source = dist / f'Tickreel-OBS-{version}-source.zip'
with zipfile.ZipFile(source, 'w', zipfile.ZIP_DEFLATED) as output:
    for file in [*sorted((root / 'obs-plugin/src').glob('*.rs')),
                 root / 'obs-plugin/Cargo.toml', root / 'obs-plugin/Cargo.lock',
                 root / 'obs-plugin/README.md', root / 'obs-plugin/LICENSE',
                 root / 'obs-plugin/install-windows.ps1', root / 'src/model.rs', root / 'src/store.rs',
                 root / 'src/app_links.rs', root / 'assets/app-signature.json',
                 root / 'scripts/package-obs-plugin.py', root / 'scripts/collect-licenses.py',
                 root / 'scripts/test-obs-plugin.py', root / 'Cargo.toml', root / 'Cargo.lock']:
        output.write(file, file.relative_to(root))
    included = set(output.namelist())
    for directory in ['src', 'assets', 'frontend']:
        for file in sorted((root / directory).rglob('*')):
            name = file.relative_to(root).as_posix()
            if file.is_file() and name not in included:
                output.write(file, name)
                included.add(name)
    output.write(root / 'build.rs', 'build.rs')
checksums = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (archive, source)}
(dist / f'Tickreel-OBS-{version}-sha256.json').write_text(json.dumps(checksums, indent=2), encoding='utf-8')
print(archive)
print(source)
