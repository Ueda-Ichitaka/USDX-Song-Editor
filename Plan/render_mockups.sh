#!/usr/bin/env bash
# About me: renders every mock screen of Plan/usdx-song-editor-plan.html to PNG files in
# Plan/mockups/ (light and dark theme), so the reference images in Plan/UI_SPEC.md can be
# regenerated after the mock page changes. Uses headless Firefox with a throwaway profile
# and ImageMagick to trim the page margin. Checks free memory before every Firefox start.
#
# Usage: Plan/render_mockups.sh [panel-id...]   (default: all panels, e.g. m-editor m-start)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
page="$here/usdx-song-editor-plan.html"
out="$here/mockups"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/profile" "$out"

declare -A NAME=(
  [m-editor]=editor [m-repair]=repair-anchor [m-block]=repair-block [m-tap]=tap-along
  [m-text]=line-text [m-props]=song-properties [m-start]=start-screen [m-menu]=main-menu
  [m-audio]=audio-states [m-readonly]=read-only [m-unsaved]=unsaved-changes
  [m-external]=changed-on-disk [m-export]=export-anchors [m-upgrade]=upgrade
  [m-settings]=settings [m-calib]=latency-calibration [m-messages]=errors-and-notifications
)
panels=("$@")
if [ ${#panels[@]} -eq 0 ]; then
  panels=(m-editor m-repair m-block m-tap m-text m-props m-start m-menu m-audio m-readonly
          m-unsaved m-external m-export m-upgrade m-settings m-calib m-messages)
fi

for p in "${panels[@]}"; do
  [ -n "${NAME[$p]:-}" ] || { echo "unknown panel: $p" >&2; exit 2; }
  for theme in light dark; do
    avail_gib=$(awk '/MemAvailable/ {print int($2 / 1048576)}' /proc/meminfo)
    if [ "$avail_gib" -lt 2 ]; then
      echo "stopped: only ${avail_gib} GiB memory available, need 2" >&2
      exit 1
    fi
    raw="$work/raw.png"
    timeout 90 firefox --headless --no-remote --profile "$work/profile" --window-size=1340,1300 \
      --screenshot "$raw" "file://$page#shot-$p-$theme" >/dev/null 2>&1
    magick "$raw" -trim +repage "$out/${NAME[$p]}-$theme.png"
    echo "${NAME[$p]}-$theme.png $(magick identify -format '%wx%h' "$out/${NAME[$p]}-$theme.png")"
  done
done
