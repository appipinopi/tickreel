"""Exercise a built plugin against a real portable Windows libobs runtime.

python scripts/test-obs-plugin.py <OBS root> <tickreel_obs.dll>
Runs in an isolated APPDATA; does not touch installed OBS or user recordings.
"""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile
import time

runtime, plugin = (Path(arg).resolve() for arg in sys.argv[1:])
os.chdir(runtime / 'bin' / '64bit')
dll_directory = os.add_dll_directory(str(Path.cwd()))
obs = c.CDLL(str(Path.cwd() / 'obs.dll'))
P = c.c_void_p
S = c.c_char_p
U = c.c_uint32
B = c.c_bool

def bind(name, result, *args):
    fn = getattr(obs, name)
    fn.restype, fn.argtypes = result, list(args)
    return fn

startup = bind('obs_startup', B, S, S, P)
shutdown = bind('obs_shutdown', None)
add_data = bind('obs_add_data_path', None, S)
reset_video = bind('obs_reset_video', c.c_int, P)
open_module = bind('obs_open_module', c.c_int, c.POINTER(P), S, S)
init_module = bind('obs_init_module', B, P)
create_data = bind('obs_data_create', P)
release_data = bind('obs_data_release', None, P)
set_int = bind('obs_data_set_int', None, P, S, c.c_int64)
create_source = bind('obs_source_create', P, S, S, P, P)
release_source = bind('obs_source_release', None, P)
get_properties = bind('obs_source_properties', P, P)
get_property = bind('obs_properties_get', P, P, S)
click = bind('obs_property_button_clicked', B, P, P)
destroy_properties = bind('obs_properties_destroy', None, P)
get_description = bind('obs_property_description', S, P)

class VideoInfo(c.Structure):
    _fields_ = [('graphics_module', S), ('fps_num', U), ('fps_den', U),
                ('base_width', U), ('base_height', U), ('output_width', U),
                ('output_height', U), ('output_format', U), ('adapter', U),
                ('gpu_conversion', B), ('colorspace', U), ('range', U),
                ('scale_type', U)]

with tempfile.TemporaryDirectory(prefix='tickreel-obs-test-') as temporary:
    # dirs::data_dir on Windows obtains RoamingAppData from the KnownFolder API,
    # so use the explicit plugin recording-root override for isolation.
    os.environ['TICKREEL_OBS_RECORDINGS_DIR'] = str(Path(temporary) / 'obs-recordings')
    assert startup(b'en-US', str(Path(temporary) / 'obs-config').encode(), None)
    add_data(str(runtime / 'data' / 'libobs').encode())
    video = VideoInfo(b'libobs-d3d11.dll', 30, 1, 320, 180, 320, 180, 2, 0, True, 2, 1, 4)
    assert reset_video(c.byref(video)) == 0, 'OBS D3D11 video initialization failed'
    module = P()
    assert open_module(c.byref(module), str(plugin).encode(), str(Path(temporary)).encode()) == 0
    assert init_module(module), 'OBS rejected the plugin'
    settings = create_data()
    set_int(settings, b'interval', 1)
    set_int(settings, b'fps', 15)
    source = create_source(b'tickreel_timelapse', b'Tickreel test', settings, None)
    assert source, 'Source registration failed'
    props = get_properties(source)
    assert props
    assert b'Made by appipinopi' in get_description(get_property(props, b'creator'))
    assert get_property(props, b'website'), 'Shared official site control is missing'
    assert click(get_property(props, b'start'), source)
    status = get_description(get_property(props, b'status')).decode()
    assert 'Recording' in status, status
    time.sleep(2.3)
    assert click(get_property(props, b'stop'), source)
    status = get_description(get_property(props, b'status')).decode()
    assert 'Saved' in status, status
    records = list(Path(temporary).glob('obs-recordings/*/recording.json'))
    assert len(records) == 1, records
    record = json.loads(records[0].read_text(encoding='utf-8'))
    assert record['status'] == 'ready' and record['frames'] >= 2, record
    assert record['fps'] == 15 and record['idleSpeed'] == 1
    frames = list(records[0].parent.glob('frame-*.jpg'))
    assert len(frames) == record['frames']
    assert len((records[0].parent / 'timeline.jsonl').read_text().splitlines()) == record['frames']
    # Source destruction must stop, drain and finalize a second active session.
    assert click(get_property(props, b'start'), source)
    time.sleep(1.2)
    destroy_properties(props)
    release_source(source)
    release_data(settings)
    shutdown()
    records = list(Path(temporary).glob('obs-recordings/*/recording.json'))
    assert len(records) == 2
    assert all(json.loads(path.read_text())['status'] == 'ready' for path in records)
    print(f'PASS: real libobs load, source properties, raw frames, stop, restart and destruction; {len(frames)} JPEG frames')
