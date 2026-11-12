#!/usr/bin/env bash
# About me: integration test for the CMake install (task P0-04). It configures the
# project with a temporary install prefix, builds it through CMake inside the memory
# guard of scripts/dev.sh, installs into the temporary prefix and asserts that the
# binary, the desktop file, the AppStream metainfo and the icon land in the right
# places, that the installed binary runs, and that desktop-file-validate and
# appstreamcli pass when they are installed.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
# shellcheck source=../dev.sh
source "$here/../dev.sh"

tmp="$(mktemp -d "${TMPDIR:-/tmp/opencode}/cmake-install-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

passed=0
ok() { echo "ok - $1"; passed=$((passed + 1)); }
fail() { echo "FAIL - $1" >&2; exit 1; }

app_id="io.github.ueda_ichitaka.UsdxSongEditor"
build_dir="$tmp/build"
prefix="$tmp/prefix"

mem_ok
export CARGO_BUILD_JOBS="$(jobs_for)"

# The build goes through CMake, which drives cargo; the memory guard wraps the build.
# The PATH hides cargo on purpose: in a plain login shell cargo is not on the PATH
# (it lives in ~/.cargo/bin here), and CMake must locate it itself.
run_guarded env PATH=/usr/bin:/bin cmake -S "$repo" -B "$build_dir" -DCMAKE_INSTALL_PREFIX="$prefix" 2>"$tmp/configure.log" \
  || { cat "$tmp/configure.log" >&2; fail "cmake configure failed"; }
ok "cmake configure succeeds with Kirigami and qqc2-desktop-style found"

run_guarded env PATH=/usr/bin:/bin cmake --build "$build_dir" 2>"$tmp/build.log" \
  || { tail -30 "$tmp/build.log" >&2; fail "cmake build failed"; }
ok "cmake build succeeds without cargo on the PATH"

run_guarded cmake --install "$build_dir" 2>"$tmp/install.log" \
  || { cat "$tmp/install.log" >&2; fail "cmake install failed"; }
ok "cmake install succeeds"

# The binary is installed into bin and runs.
bin="$prefix/bin/usdx-song-editor"
[[ -x "$bin" ]] || fail "the binary is not installed at $bin"
version="$(QT_QPA_PLATFORM=offscreen "$bin" --version)"
[[ "$version" == "usdx-song-editor 0.1.0" ]] || fail "the installed binary prints: $version"
ok "the installed binary runs and prints the version"

# The desktop file is installed into share/applications and validates.
desktop="$prefix/share/applications/$app_id.desktop"
[[ -f "$desktop" ]] || fail "the desktop file is not installed at $desktop"
if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$desktop" 2>"$tmp/desktop.log" \
    || { cat "$tmp/desktop.log" >&2; fail "desktop-file-validate rejects the desktop file"; }
  ok "desktop-file-validate passes"
else
  echo "skip - desktop-file-validate not installed"
fi

# The AppStream metainfo is installed into share/metainfo and validates.
metainfo="$prefix/share/metainfo/$app_id.metainfo.xml"
[[ -f "$metainfo" ]] || fail "the metainfo is not installed at $metainfo"
if command -v appstreamcli >/dev/null 2>&1; then
  # --no-net keeps the validator offline: the project URL is only reachable once
  # the repository is published, and the test must not depend on network access.
  appstreamcli validate --no-net "$metainfo" 2>"$tmp/appstream.log" \
    || { cat "$tmp/appstream.log" >&2; fail "appstreamcli rejects the metainfo"; }
  ok "appstreamcli validate passes"
else
  echo "skip - appstreamcli not installed"
fi

# The icon is installed into the hicolor scalable apps directory.
icon="$prefix/share/icons/hicolor/scalable/apps/$app_id.svg"
[[ -f "$icon" ]] || fail "the icon is not installed at $icon"
ok "the icon is installed"

echo "all $passed cmake install tests passed"
