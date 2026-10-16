#!/usr/bin/env bash
# Exercise the real portal and PipeWire on an isolated headless compositor.
# Requires sway, pipewire, wireplumber, xdg-desktop-portal{,-wlr}, dbus, coreutils.
set -euo pipefail
binary="$(realpath "${1:?Pass the Linux Tickreel executable}")"
script="$(realpath "$0")"
if [[ ${TICKREEL_WAYLAND_TEST_CHILD:-} != 1 ]]; then
  runtime="$(mktemp -d /tmp/tickreel-wayland.XXXXXXXX)"
  chmod 700 "$runtime"
  trap 'rm -rf -- "$runtime"' EXIT
  # Hosted runners can keep their checkout below a private home directory.
  # Copy both inputs into the isolated runtime before dropping privileges.
  cp "$script" "$runtime/test-wayland.sh"
  cp "$binary" "$runtime/tickreel-test"
  chmod 755 "$runtime/tickreel-test"
  script="$runtime/test-wayland.sh"
  binary="$runtime/tickreel-test"
  runner=()
  if [[ $(id -u) == 0 ]]; then
    chown nobody "$runtime"
    runner=(runuser -u nobody --)
  fi
  cd "$runtime"
  "${runner[@]}" env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
    XDG_RUNTIME_DIR="$runtime" HOME="$runtime" \
    TICKREEL_WAYLAND_TEST_CHILD=1 \
    dbus-run-session -- bash "$script" "$binary"
  exit
fi
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=sway
export XDG_CONFIG_HOME="$XDG_RUNTIME_DIR/config"
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_HEADLESS_OUTPUTS=2
export TICKREEL_HEADLESS_WAYLAND_TEST=1
unset DISPLAY WAYLAND_DISPLAY
mkdir -p "$XDG_CONFIG_HOME/xdg-desktop-portal"
printf '[preferred]\ndefault=wlr\n' > "$XDG_CONFIG_HOME/xdg-desktop-portal/portals.conf"
printf '[screencast]\nchooser_type=none\noutput_name=HEADLESS-1\nmax_fps=5\n' > "$XDG_RUNTIME_DIR/wlr.conf"
printf 'xwayland disable\noutput * mode 640x480\noutput * bg "#336699" solid_color\n' > "$XDG_RUNTIME_DIR/sway.conf"
pids=()
cleanup() {
  status=$?
  if [[ $status != 0 ]]; then
    for logfile in "$XDG_RUNTIME_DIR"/*.log; do
      printf '\n%s\n' "$logfile"
      cat "$logfile"
    done
  fi
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait || true
}
trap cleanup EXIT
sway --unsupported-gpu --config "$XDG_RUNTIME_DIR/sway.conf" > "$XDG_RUNTIME_DIR/sway.log" 2>&1 &
pids+=("$!")
for ((i=0;i<100;i++)); do
  for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
    if [[ -S "$socket" ]]; then export WAYLAND_DISPLAY="${socket##*/}"; break; fi
  done
  [[ -n ${WAYLAND_DISPLAY:-} ]] && break
  sleep .1
done
if [[ -z ${WAYLAND_DISPLAY:-} ]]; then cat "$XDG_RUNTIME_DIR/sway.log"; exit 1; fi
export SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'sway-ipc.*' -print -quit)"
dbus-update-activation-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE XDG_RUNTIME_DIR XDG_CONFIG_HOME
pipewire > "$XDG_RUNTIME_DIR/pipewire.log" 2>&1 &
pids+=("$!")
for ((i=0;i<100;i++)); do
  [[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]] && break
  sleep .1
done
[[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]] || { cat "$XDG_RUNTIME_DIR/pipewire.log"; exit 1; }
wireplumber > "$XDG_RUNTIME_DIR/wireplumber.log" 2>&1 &
pids+=("$!")
/usr/libexec/xdg-desktop-portal-wlr --loglevel DEBUG --config "$XDG_RUNTIME_DIR/wlr.conf" > "$XDG_RUNTIME_DIR/wlr.log" 2>&1 &
pids+=("$!")
# Let D-Bus activate the frontend portal after the configured backend owns its
# name; starting it concurrently with activation can create two instances.
timeout 20 gdbus wait --session org.freedesktop.impl.portal.desktop.wlr
for ((i=0;i<100;i++)); do
  if gdbus call --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop \
    --method org.freedesktop.DBus.Properties.Get org.freedesktop.portal.ScreenCast AvailableSourceTypes >/dev/null 2>&1; then break; fi
  sleep .1
done
if ! timeout 60 "$binary" --wayland-self-test; then
  cat "$XDG_RUNTIME_DIR/wlr.log" "$XDG_RUNTIME_DIR/pipewire.log"
  exit 1
fi
# Start the actual GTK/WebKit window with Xwayland disabled, then exercise its
# renderer-to-Rust bridge without requiring a source or opening another chooser.
if ! GDK_BACKEND=wayland LIBGL_ALWAYS_SOFTWARE=1 WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 \
  timeout 45 "$binary" --ui-test-update > "$XDG_RUNTIME_DIR/ui.log" 2>&1; then
  cat "$XDG_RUNTIME_DIR/ui.log"
  exit 1
fi
echo 'PASS: native Wayland GTK/WebKit UI (no Xwayland)'
