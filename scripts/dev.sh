#!/usr/bin/env bash
# About me: guarded build and test entry point for the USDX Song Editor workspace
# (plan section 2.5). It checks the available memory before every run, caps cargo's
# build parallelism to what the machine can carry, runs cargo inside a memory-limited
# systemd user scope so an out-of-memory situation kills the build and not the
# desktop, and reports an out-of-memory kill as such instead of a test failure.
# Subcommands: check [--release], test [crate] [filter], build [--release],
# run [-- args], corpus. The guard functions are sourced and tested by
# scripts/tests/dev_guard_test.sh.

set -euo pipefail

# cargo lives in ~/.cargo/bin on this machine, which not every shell has on PATH.
case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) ;;
  *) export PATH="$HOME/.cargo/bin:$PATH" ;;
esac

# mem_available_kib prints the MemAvailable value in kB. DEV_MEMINFO overrides
# /proc/meminfo so the guard tests can run without touching the real system.
mem_available_kib() {
  local file="${DEV_MEMINFO:-/proc/meminfo}"
  awk '$1 == "MemAvailable:" { print $2; found = 1; exit }
       END { if (!found) { print "no MemAvailable in " FILENAME > "/dev/stderr"; exit 1 } }' "$file"
}

# mem_ok refuses to start below 4 GiB available; the machine is shared with other work.
mem_ok() {
  local kib giB
  kib="$(mem_available_kib)" || return 1
  giB=$(( kib / 1024 / 1024 ))
  if (( kib < 4 * 1024 * 1024 )); then
    echo "Refusing to start: only ${giB} GiB of memory available (need 4 GiB). Close something and try again." >&2
    return 1
  fi
}

# jobs_for caps the cargo build jobs: max(1, min(nproc / 2, available GiB / 2)).
jobs_for() {
  local nproc="${DEV_NPROC:-$(nproc)}"
  local kib giB
  kib="$(mem_available_kib)" || return 1
  giB=$(( kib / 1024 / 1024 ))
  local jobs=$(( nproc / 2 ))
  local mem_jobs=$(( giB / 2 ))
  if (( mem_jobs < jobs )); then jobs=$mem_jobs; fi
  if (( jobs < 1 )); then jobs=1; fi
  echo "$jobs"
}

# memory_max prints the MemoryMax value for the systemd scope: 70 percent of the
# available memory, rounded down to whole MiB.
memory_max() {
  local kib
  kib="$(mem_available_kib)" || return 1
  echo "$(( kib * 70 / 100 / 1024 ))M"
}

# run_guarded runs a command inside a systemd user scope with the memory limit; where
# systemd-run is not available it runs the command directly. An out-of-memory kill
# (exit 137) is reported as such, not as a failure of what was being built or tested.
run_guarded() {
  local code=0
  if command -v systemd-run >/dev/null 2>&1; then
    systemd-run --user --scope -q -p "MemoryMax=$(memory_max)" -- "$@" || code=$?
  else
    "$@" || code=$?
  fi
  if (( code == 137 )); then
    echo "killed: out of memory (the memory guard stopped the job; this is not a test failure)" >&2
  fi
  return $code
}

# usage prints how to call the script and exits with the argument code.
usage() {
  echo "usage: $0 {check [--release] | test [crate] [filter] | build [--release] | run [-- args] | corpus}" >&2
  exit "$1"
}

# The subcommands below run only when the script is executed, not when the guard
# functions are sourced by the tests.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  if (( $# == 0 )); then usage 2; fi
  cmd="$1"
  shift
  mem_ok
  export CARGO_BUILD_JOBS="$(jobs_for)"
  case "$cmd" in
    check)
      release=""
      if [[ "${1:-}" == "--release" ]]; then release="--release"; shift; fi
      cargo fmt --all --check
      run_guarded cargo clippy --workspace --all-targets $release -- -D warnings
      run_guarded cargo test --workspace $release
      ;;
    test)
      if (( $# > 0 )); then
        crate="$1"
        shift
        run_guarded cargo test -p "$crate" "$@"
      else
        run_guarded cargo test --workspace
      fi
      ;;
    build)
      release=""
      if [[ "${1:-}" == "--release" ]]; then release="--release"; shift; fi
      run_guarded cargo build --workspace $release
      ;;
    run)
      run_guarded cargo run "$@"
      ;;
    corpus)
      export USDX_CORPUS="${USDX_CORPUS:-/home/ueda/workspace/USDX-Song-Factory/stack/output/new:/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/recreated:/home/ueda/workspace/USDX-Song-Factory/stack/output/repaired/sync}"
      run_guarded cargo test -p usdx-format --test corpus -- --ignored
      ;;
    *)
      usage 2
      ;;
  esac
fi
