"""Package an already-built Linux directory as DEB and RPM without rebuilding it."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
machine = subprocess.check_output(['uname', '-m'], text=True).strip()
deb_arch, rpm_arch = {'x86_64': ('amd64', 'x86_64'), 'aarch64': ('arm64', 'aarch64')}[machine]
dist = Path('dist').resolve()
dist.mkdir(exist_ok=True)

with tempfile.TemporaryDirectory(prefix='tickreel-package-') as temporary:
    work = Path(temporary)
    stage = work / 'root'
    app = stage / 'opt/tickreel'
    shutil.copytree(root, app)
    applications = stage / 'usr/share/applications'
    applications.mkdir(parents=True)
    (applications / 'tickreel.desktop').write_text('''[Desktop Entry]
Name=Tickreel
Comment=Turn your daily work into a timelapse
Exec=/opt/tickreel/tickreel
TryExec=/opt/tickreel/tickreel
Icon=tickreel
Terminal=false
Type=Application
Categories=AudioVideo;Recorder;
StartupNotify=true
''')
    icons = stage / 'usr/share/icons/hicolor/scalable/apps'
    icons.mkdir(parents=True)
    shutil.copyfile('snap/gui/tickreel.svg', icons / 'tickreel.svg')
    command = stage / 'usr/bin'
    command.mkdir(parents=True)
    (command / 'tickreel').symlink_to('/opt/tickreel/tickreel')

    # Compute dependency versions from the actual ELF and the native build OS.
    debian = work / 'debian'
    debian.mkdir()
    (debian / 'control').write_text('Source: tickreel\nSection: video\nPriority: optional\nMaintainer: appipinopi <appipinopi@users.noreply.github.com>\n\nPackage: tickreel\nArchitecture: any\nDescription: Desktop work timelapse recorder\n')
    result = subprocess.check_output(['dpkg-shlibdeps', '-O', '-e' + str(app / 'tickreel')], cwd=work, text=True)
    depends = next(line.removeprefix('shlibs:Depends=') for line in result.splitlines() if line.startswith('shlibs:Depends='))
    control = stage / 'DEBIAN'
    control.mkdir()
    size = sum(p.stat().st_size for p in stage.rglob('*') if p.is_file()) // 1024
    (control / 'control').write_text(f'''Package: tickreel
Version: {version}
Section: video
Priority: optional
Architecture: {deb_arch}
Maintainer: appipinopi <appipinopi@users.noreply.github.com>
Homepage: https://github.com/appipinopi/tickreel
Installed-Size: {size}
Depends: {depends}
Recommends: pipewire, xdg-desktop-portal
Description: Local desktop work timelapse recorder
 Record drawing and development workflows with captions, narration and music.
 Screen capture supports X11 and Wayland (ScreenCast portal and PipeWire).
''')
    deb = dist / f'Tickreel-{version}-linux-{deb_arch}.deb'
    subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(stage), str(deb)], check=True)
    subprocess.run(['dpkg-deb', '--info', str(deb)], check=True)
    # Keep the Debian metadata out of the RPM payload.
    shutil.rmtree(control)
    rpm = work / 'rpm'
    rpm.mkdir()
    spec = rpm / 'tickreel.spec'
    spec.write_text(f'''Name: tickreel
Version: {version}
Release: 1
Summary: Local desktop work timelapse recorder
License: LicenseRef-Tickreel-Unspecified
URL: https://github.com/appipinopi/tickreel
BuildArch: {rpm_arch}
# Ubuntu-built preview: explicit runtime sonames plus the build's glibc baseline.
Requires: libc.so.6(GLIBC_2.39)(64bit)
AutoReqProv: yes
Recommends: pipewire
Recommends: xdg-desktop-portal
%description
Record desktop work with captions, narration and music. X11 and Wayland capture supported (ScreenCast portal and PipeWire).
%prep
%build
%install
mkdir -p %{{buildroot}}
cp -a "{stage}/." %{{buildroot}}/
%files
%defattr(-,root,root,-)
/opt/tickreel
/usr/bin/tickreel
/usr/share/applications/tickreel.desktop
/usr/share/icons/hicolor/scalable/apps/tickreel.svg
''')
    subprocess.run(['rpmbuild', '-bb', '--target', rpm_arch, '--define', f'_topdir {rpm}', '--define', '__os_install_post %{nil}', '--define', '_build_id_links none', '--define', '_rpmfilename %{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}.rpm', str(spec)], check=True)
    built = next((rpm / 'RPMS').rglob('*.rpm'))
    output = dist / f'Tickreel-{version}-linux-{rpm_arch}.rpm'
    shutil.copyfile(built, output)
    subprocess.run(['rpm', '-qpl', str(output)], check=True, stdout=subprocess.DEVNULL)
    print('Created:', deb.name, output.name)
