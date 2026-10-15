"""Exercise the published ARM64 installer on an ephemeral native ARM64 runner."""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import tomllib

version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
with tempfile.TemporaryDirectory(prefix='tickreel-release-') as temporary:
    stage = Path(temporary)
    for name in (f'Tickreel-{version}-windows-arm64.exe', 'update-windows-arm64.json'):
        subprocess.run(['gh', 'release', 'download', f'v{version}', '--pattern', name, '--dir', str(stage)], check=True)
    installed = stage / 'installed'
    installer = stage / f'Tickreel-{version}-windows-arm64.exe'
    subprocess.run([str(installer), '/S', f'/D={installed}'], check=True, timeout=120)
    executable = installed / 'Tickreel.exe'
    binary = executable.read_bytes()
    machine = struct.unpack_from('<H', binary, struct.unpack_from('<I', binary, 0x3c)[0] + 4)[0]
    assert machine == 0xaa64, hex(machine)
    manifest = json.loads((stage / 'update-windows-arm64.json').read_text())
    assert hashlib.sha256(binary).hexdigest() == manifest['sha256']
    result = subprocess.run([str(executable), '--check-update'], capture_output=True, text=True, encoding='utf-8', check=True, timeout=40)
    assert json.loads(result.stdout)['phase'] == 'current', result.stdout
    encoders = subprocess.check_output([str(installed / 'ffmpeg.exe'), '-hide_banner', '-encoders'], text=True, stderr=subprocess.STDOUT)
    print(encoders, flush=True)
    for name in ('libx264', 'libsvtav1', 'pcm_s16le', 'libmp3lame', 'flac', 'aac', 'libopus'):
        assert name in encoders, name
    filters = subprocess.check_output([str(installed / 'ffmpeg.exe'), '-hide_banner', '-filters'], text=True, stderr=subprocess.STDOUT)
    assert ' ass ' in filters
    result = subprocess.run([str(executable), '--export-self-test'], capture_output=True, text=True, encoding='utf-8', timeout=300)
    print(result.stdout, result.stderr, flush=True)
    assert result.returncode == 0 and '12 export formats' in result.stdout
    keep = installed / 'keep-user-data.txt'
    keep.write_text('keep')
    subprocess.run([str(installed / 'Uninstall.exe'), '/S'], check=True, timeout=90)
    for attempt in range(20):
        if not executable.exists() and not (installed / 'Uninstall.exe').exists():
            break
        time.sleep(1)
    assert not executable.exists()
    assert keep.read_text() == 'keep'
    assert not (installed / 'ffmpeg.exe').exists()
    print('PASS: native ARM64 install, signed update check, FFmpeg encoders/captions, uninstall and user data preservation')
