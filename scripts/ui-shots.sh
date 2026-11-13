#!/usr/bin/env bash
# About me: renders the application's demo states (UI_SPEC 8.1) to PNG files,
# one per demo state and theme, into target/ui-shots/<state>-<theme>.png, for
# the acceptance comparison against the mockup images. It reuses the memory
# guard of scripts/dev.sh (sourced, so the guard functions are shared), builds
# the application, and runs every screenshot with the fixed offscreen
# environment and a temporary XDG_CONFIG_HOME that carries the Breeze colour
# scheme as kdeglobals - which also isolates the app settings, so the shots
# never touch the user's desktop or settings. The start-screen state seeds the
# recent list with songs from the approved USDX Song Factory test folders
# (plan section 9.5); without those folders it renders the empty start screen.
# The audio states open their reference songs (read-only; the app opens a copy):
# audio-states-missing and audio-states-loading the aequitaS - Realize The Real
# Lies sample of the karaoke dashboard, audio-states-no-vocals a corpus song
# without a #VOCALS track; a missing song skips that state with a message.
# Usage: scripts/ui-shots.sh [state ...]  (no arguments: every known state).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# The memory guard functions (mem_ok, run_guarded) live in dev.sh and are
# sourced, not executed: the file runs its subcommands only when it is the
# invoked script.
source "$SCRIPT_DIR/dev.sh"

readonly APP_ID="io.github.ueda_ichitaka.UsdxSongEditor"
readonly BIN="$REPO_ROOT/target/debug/usdx-song-editor"
readonly OUT_DIR="$REPO_ROOT/target/ui-shots"
readonly COLOR_SCHEME_DIR="/usr/share/color-schemes"
readonly SIZE="1280x800"

# The demo states that exist so far; the task that builds a screen adds its
# state here (UI_SPEC 8.1).
readonly STATES=("start-screen" "audio-states-missing" "audio-states-loading" "audio-states-no-vocals")

# The reference songs of the audio states (user decisions recorded in
# Plan/PROGRESS.md, task P4-04).
readonly REALIZE_SONG="/home/ueda/workspace/karaoke-dashboard/songs-sample/aequitaS - Realize The Real Lies/aequitaS - Realize The Real Lies.txt"
readonly NO_VOCALS_SONG="/home/ueda/workspace/USDX-Song-Factory/stack/output/new/Laserkraft 3D - Nein Mann/song.txt"

# song_for prints the song a demo state opens, empty for states without one.
song_for() {
  case "$1" in
    audio-states-missing|audio-states-loading) echo "$REALIZE_SONG" ;;
    audio-states-no-vocals) echo "$NO_VOCALS_SONG" ;;
    *) echo "" ;;
  esac
}

# The approved real-song folders for the recent list (plan section 9.5).
readonly CORPUS_FOLDERS=(
  /home/ueda/workspace/USDX-Song-Factory/stack/output/new
  /home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/recreated
  /home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/sync
)

usage() {
  echo "usage: $0 [state ...]   states: ${STATES[*]}" >&2
  exit 2
}

# is_state reports whether the named demo state exists.
is_state() {
  local wanted="$1" state
  for state in "${STATES[@]}"; do
    if [[ "$state" == "$wanted" ]]; then return 0; fi
  done
  return 1
}

# find_recent_songs prints up to three song file paths from the approved test
# folders: a .txt counts as a song when it holds a #BPM header; the Factory
# helper files and .bak files are skipped (plan section 9.2). The folders are
# read-only here: nothing is written into them.
find_recent_songs() {
  local wanted="$1" folder file count=0
  for folder in "${CORPUS_FOLDERS[@]}"; do
    [[ -d "$folder" ]] || continue
    # Find first, sort for a stable choice across runs.
    while IFS= read -r file; do
      if grep -q "^#BPM" "$file" 2>/dev/null; then
        echo "$file"
        count=$(( count + 1 ))
        (( count >= wanted )) && return 0
      fi
    done < <(find "$folder" -maxdepth 2 -name '*.txt' ! -name '*.bak' \
      ! -iname 'lyrics.txt' ! -iname 'language.txt' \
      ! -iname 'song_end.txt' ! -iname 'anchors.txt' | sort)
  done
}

# seed_settings writes a settings file for the temporary config home so the
# start screen shows the recent songs the reference image has (UI_SPEC 8.1).
seed_settings() {
  local config_home="$1"
  shift
  [[ $# -gt 0 ]] || return 0
  local app_dir="$config_home/$APP_ID"
  mkdir -p "$app_dir"
  local now opened path
  now="$(date +%s)"
  {
    echo "recent_files = ["
    local index=0
    for path in "$@"; do
      # newest, yesterday, three days ago: the relative dates of UI_SPEC 4.1
      opened=$(( now - index * 86400 ))
      echo "  { path = \"$path\", opened = $opened },"
      index=$(( index + 1 ))
    done
    echo "]"
    echo "window_size = [1280, 800]"
  } > "$app_dir/settings.toml"
}

# render_one renders one demo state in one theme with a fresh isolated
# config home; the theme comes from the colour scheme copied as kdeglobals.
render_one() {
  local state="$1" theme="$2" out="$3"
  local scheme_name scheme conf
  if [[ "$theme" == "light" ]]; then scheme_name="BreezeLight"; else scheme_name="BreezeDark"; fi
  scheme="$COLOR_SCHEME_DIR/$scheme_name.colors"
  if [[ ! -f "$scheme" ]]; then
    echo "ui-shots: missing colour scheme $scheme" >&2
    return 1
  fi
  conf="$(mktemp -d)"
  trap 'rm -rf "$conf"' RETURN
  cp "$scheme" "$conf/kdeglobals"
  if [[ "$state" == "start-screen" ]]; then
    # Best effort: with the Factory test folders present the recent list
    # matches the reference image, else the empty start screen renders.
    local songs
    mapfile -t songs < <(find_recent_songs 3)
    seed_settings "$conf" "${songs[@]}"
  fi
  local song song_args=()
  song="$(song_for "$state")"
  if [[ -n "$song" ]]; then
    if [[ ! -f "$song" ]]; then
      echo "ui-shots: skipped $state: its song is missing: $song" >&2
      return 1
    fi
    song_args=(--song "$song")
  fi
  env XDG_CONFIG_HOME="$conf" \
      QT_QPA_PLATFORM=offscreen \
      QT_QUICK_CONTROLS_STYLE=org.kde.desktop \
      QT_QPA_PLATFORMTHEME=kde \
      "$BIN" --screenshot "$out" --demo-state "$state" "${song_args[@]}" --size "$SIZE"
}

# The guard functions need the environment overrides only for their own tests;
# here the real system values are used.
mem_ok

# Optional arguments select the states; unknown names fail early.
selected=()
if (( $# == 0 )); then
  selected=("${STATES[@]}")
else
  for state in "$@"; do
    is_state "$state" || usage
    selected+=("$state")
  done
fi

mkdir -p "$OUT_DIR"
run_guarded cargo build --workspace --manifest-path "$REPO_ROOT/Cargo.toml"
if [[ ! -x "$BIN" ]]; then
  echo "ui-shots: the application binary did not build at $BIN" >&2
  exit 1
fi

failed=0
for state in "${selected[@]}"; do
  for theme in light dark; do
    out="$OUT_DIR/$state-$theme.png"
    if render_one "$state" "$theme" "$out"; then
      echo "ui-shots: wrote $out"
    else
      echo "ui-shots: FAILED for $state ($theme)" >&2
      failed=1
    fi
  done
done
exit "$failed"
