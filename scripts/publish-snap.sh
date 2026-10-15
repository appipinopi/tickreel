#!/usr/bin/env bash
set -euo pipefail
package="${1:?usage: publish-snap.sh PACKAGE.snap [edge|beta|candidate|stable]}"
channel="${2:-edge}"
case "$channel" in edge|beta|candidate|stable|latest/edge|latest/beta|latest/candidate|latest/stable) ;; *) echo 'Unsupported Snap channel' >&2; exit 2;; esac
test -f "$package" || { echo 'Snap package not found' >&2; exit 2; }
metadata="$(unsquashfs -cat "$package" meta/snap.yaml)"
grep -qx 'name: tickreel' <<<"$metadata" || { echo 'Unexpected Snap name' >&2; exit 2; }
grep -qx 'confinement: strict' <<<"$metadata" || { echo 'Refusing to publish a non-strict Snap' >&2; exit 2; }
if [[ "${channel##*/}" == stable || "${channel##*/}" == candidate ]]; then
  grep -qx 'grade: stable' <<<"$metadata" || { echo 'A devel Snap cannot be released to candidate/stable; validate the app before changing grade' >&2; exit 2; }
fi
if [[ -z "${SNAPCRAFT_STORE_CREDENTIALS:-}" ]]; then
  echo 'SNAPCRAFT_STORE_CREDENTIALS is required; export a tickreel-scoped Snapcraft login first' >&2
  exit 2
fi
snapcraft upload "$package" --release="$channel"
snapcraft status tickreel
