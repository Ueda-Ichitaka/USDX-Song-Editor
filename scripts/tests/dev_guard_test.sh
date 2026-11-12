#!/usr/bin/env bash
# About me: tests for the guard functions of scripts/dev.sh (task P0-02). They source
# dev.sh, feed it fake /proc/meminfo files through DEV_MEMINFO and a fake thread count
# through DEV_NPROC, and assert the refusal threshold, the build job cap and the
# systemd MemoryMax value, so the memory guard is verified without touching the real
# desktop session.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../dev.sh
source "$here/../dev.sh"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/dev-guard-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

passed=0
ok() { echo "ok - $1"; passed=$((passed + 1)); }
fail() { echo "FAIL - $1" >&2; exit 1; }

# fake_meminfo writes a /proc/meminfo lookalike with the given available memory in GiB
# and points DEV_MEMINFO at it.
fake_meminfo() {  # fake_meminfo <available-GiB>
  local kib=$(( $1 * 1024 * 1024 ))
  printf 'MemTotal:       %d kB\nMemAvailable:   %d kB\n' "$kib" "$kib" > "$tmp/meminfo"
  export DEV_MEMINFO="$tmp/meminfo"
}

# The refuse threshold: 2 GiB is refused and the refusal names the free amount.
fake_meminfo 2
if mem_ok 2>"$tmp/err"; then fail "2 GiB must be refused"; fi
grep -q "2 GiB" "$tmp/err" || fail "the refusal message does not name the free amount"
ok "2 GiB is refused with the amount named"

# The boundary: exactly 4 GiB is allowed.
fake_meminfo 4
mem_ok || fail "4 GiB must be allowed"
ok "4 GiB is allowed"

# The job cap: 8 GiB with 16 threads gives 4 jobs.
fake_meminfo 8
export DEV_NPROC=16
[[ "$(jobs_for)" == 4 ]] || fail "8 GiB with 16 threads must give 4 jobs, got $(jobs_for)"
ok "8 GiB with 16 threads gives 4 jobs"

# The job cap: 64 GiB with 16 threads gives 8 jobs.
fake_meminfo 64
[[ "$(jobs_for)" == 8 ]] || fail "64 GiB with 16 threads must give 8 jobs, got $(jobs_for)"
ok "64 GiB with 16 threads gives 8 jobs"

# The job cap never goes below 1: 4 GiB with 2 threads gives 1.
fake_meminfo 4
export DEV_NPROC=2
[[ "$(jobs_for)" == 1 ]] || fail "the job cap must not go below 1, got $(jobs_for)"
ok "the job cap does not go below 1"

# The systemd scope limit: 70 percent of what is available, in MiB.
fake_meminfo 8
[[ "$(memory_max)" == "5734M" ]] || fail "8 GiB available must give 5734M, got $(memory_max)"
ok "memory_max is 70 percent of the available memory"

echo "all $passed guard tests passed"
