"""Download the matching FFmpeg architecture and retain its upstream notices."""
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import urllib.request
import zipfile

arch, destination = sys.argv[1], Path(sys.argv[2])
notices = destination / 'licenses/ffmpeg'
notices.mkdir(parents=True, exist_ok=True)

def download(url):
    with urllib.request.urlopen(url, timeout=180) as response:
        return response.read()

if arch == 'arm64':
    url = ('https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-14-13-03/'
           'ffmpeg-n8.1.3-18-g46d899d7a0-winarm64-gpl-8.1.zip')
    data = download(url)
    assert hashlib.sha256(data).hexdigest() == 'b3ac023794cb9a3289c510922bc14ff38d986b6d853373de14e575f8af99250e'
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        executable = next(n for n in archive.namelist() if n.endswith('/bin/ffmpeg.exe'))
        binary = archive.read(executable)
        for name in archive.namelist():
            if not name.endswith('/') and ('/LICENSE' in name.upper() or '/doc/' in name or name.lower().endswith('/readme.txt')):
                target = notices / Path(name).relative_to(Path(name).parts[0])
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(archive.read(name))
else:
    tag = 'b6.0' if arch == 'x86' else 'b6.1.1'
    cpu = 'ia32' if arch == 'x86' else 'x64'
    base = f'https://github.com/eugeneware/ffmpeg-static/releases/download/{tag}'
    url = f'{base}/ffmpeg-win32-{cpu}'
    binary = download(url)
    if arch == 'x86':
        (notices / 'COPYING.GPLv3').write_bytes(download('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n6.0/COPYING.GPLv3'))
        (notices / 'README').write_text('FFmpeg 6.0 Windows x86 from eugeneware/ffmpeg-static release b6.0.\nSource: https://github.com/FFmpeg/FFmpeg/tree/n6.0\nBuild distribution: https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.0\n')
    else:
        for suffix in ('LICENSE', 'README'):
            (notices / suffix).write_bytes(download(f'{base}/win32-{cpu}.{suffix}'))
offset = struct.unpack_from('<I', binary, 0x3c)[0]
machine = struct.unpack_from('<H', binary, offset + 4)[0]
assert machine == {'x64': 0x8664, 'x86': 0x14c, 'arm64': 0xaa64}[arch], hex(machine)
(destination / 'ffmpeg.exe').write_bytes(binary)
encoders = subprocess.check_output([str(destination / 'ffmpeg.exe'), '-hide_banner', '-encoders'], text=True, stderr=subprocess.STDOUT)
webm_encoder = 'libsvtav1' if arch == 'arm64' else 'libvpx-vp9'
for encoder in ('libx264', webm_encoder, 'pcm_s16le', 'libmp3lame', 'flac', 'aac', 'libopus'):
    assert encoder in encoders, f'Missing encoder: {encoder}'
filters = subprocess.check_output([str(destination / 'ffmpeg.exe'), '-hide_banner', '-filters'], text=True, stderr=subprocess.STDOUT)
assert ' ass ' in filters, 'Caption rendering is unavailable'
(notices / 'source.json').write_text(json.dumps({'url': url, 'sha256': hashlib.sha256(binary).hexdigest()}, indent=2))
print(f'Prepared native {arch} FFmpeg')
