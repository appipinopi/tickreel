#!/usr/bin/env bash
# Run the integration fixture on Sway 1.11+, which supports software-only
# ext-image-copy capture. Ubuntu 24.04's Sway 1.9 requires a GPU for portals.
# The shipped binary remains built on Ubuntu 24.04 for compatibility.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends sway pipewire wireplumber \
  xdg-desktop-portal xdg-desktop-portal-wlr dbus-x11 libglib2.0-bin \
  libgtk-3-0t64 libwebkit2gtk-4.1-0 libasound2t64 libxdo3 libxrandr2 \
  libxss1 libgbm1 libpipewire-0.3-0t64 libxcb-randr0 fonts-noto-cjk xvfb
bash /workspace/scripts/test-wayland.sh "${1:?Pass the executable within /workspace}"
WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 LIBGL_ALWAYS_SOFTWARE=1 \
  XDG_SESSION_TYPE=x11 xvfb-run -a "$1" --ui-test-update
