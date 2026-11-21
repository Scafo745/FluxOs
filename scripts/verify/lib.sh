# Shared helpers for scripts/verify/*.sh (sourced, not executed).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FLUXD_BIN="${FLUXD_BIN:-$ROOT/target/debug/fluxd}"
AGENT_MAIN="${AGENT_MAIN:-$ROOT/agent/dist/main.js}"
PASS=0
FAIL=0
PIDS=()

cleanup() {
  for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done
  return 0
}
trap cleanup EXIT

need() {
  for c in "$@"; do
    command -v "$c" >/dev/null 2>&1 || { echo "missing required command: $c" >&2; exit 2; }
  done
}

need_fluxd() {
  [ -x "$FLUXD_BIN" ] || { echo "fluxd not built: run 'cargo build -p fluxd' (or set FLUXD_BIN)" >&2; exit 2; }
}

section() { printf '\n\033[1m%s\033[0m\n' "$1"; }

# check <description> <actual> <expected>
check() {
  if [ "$2" = "$3" ]; then
    PASS=$((PASS + 1))
    printf '  \033[32m✓\033[0m %s\n' "$1"
  else
    FAIL=$((FAIL + 1))
    printf '  \033[31m✗\033[0m %s\n      expected: %s\n      got:      %s\n' "$1" "$3" "$2"
  fi
}

# check_match <description> <actual> <extended regex>
check_match() {
  if [[ "$2" =~ $3 ]]; then check "$1" ok ok; else check "$1" "$2" "/$3/"; fi
}

wait_http() {
  for _ in $(seq 1 "${2:-150}"); do
    curl -s -o /dev/null "$1" && return 0
    sleep 0.1
  done
  echo "timed out waiting for $1" >&2
  return 1
}

# Deterministic hardware so results do not depend on the host (AMD GPU, no NVIDIA).
write_hardware() { # <state-dir> [gpu-vendor] [vendor-id]
  cat >"$1/hardware.json" <<EOF
{"source":"override","arch":"x86_64","cpu":{"vendor":"AuthenticAMD","model":"Verification CPU","threads":8,"freq_driver":null},
 "memory_mb":16384,"gpus":[{"vendor":"${2:-amd}","vendor_id":"${3:-0x1002}","device_id":"0x0001","driver":null}],
 "uefi":true,"secure_boot":false,"virtualization":null,"kernel_release":"6.18.64","sched_ext_supported":true,
 "is_nixos":false,"disks":[],"network":[]}
EOF
}

# start_fluxd <state-dir> <port> [extra fluxd args...]; sets FLUXD_PID
start_fluxd() {
  local state=$1 port=$2
  shift 2
  "$FLUXD_BIN" serve --state-dir "$state" --listen "127.0.0.1:$port" "$@" >>"$state.log" 2>&1 &
  FLUXD_PID=$!
  PIDS+=("$FLUXD_PID")
  wait_http "http://127.0.0.1:$port/v1/health"
}

stop() {
  kill "$1" 2>/dev/null
  wait "$1" 2>/dev/null
  return 0
}

summary() {
  printf '\n\033[1m%d passed, %d failed\033[0m\n' "$PASS" "$FAIL"
  [ "$FAIL" -eq 0 ]
}
