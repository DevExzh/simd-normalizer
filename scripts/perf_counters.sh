#!/usr/bin/env bash
# Perf-counter diagnosis wrapper. See:
#   docs/superpowers/specs/2026-04-21-diag-perf-counters-design.md
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ---- pre-flight -----------------------------------------------------------
command -v perf >/dev/null 2>&1 || { echo "perf not on PATH" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "cargo not on PATH" >&2; exit 1; }

PARANOID="$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo 4)"
if [[ "$PARANOID" -gt 2 ]]; then
  echo "warn: perf_event_paranoid=$PARANOID (>2). Userspace counters may still work; continuing." >&2
fi

# ---- vendor detect --------------------------------------------------------
VENDOR_ID="$(grep -m1 '^vendor_id' /proc/cpuinfo | awk '{print $3}')"
case "$VENDOR_ID" in
  GenuineIntel) VENDOR=intel ;;
  AuthenticAMD) VENDOR=amd ;;
  *) echo "unsupported vendor_id: $VENDOR_ID" >&2; exit 1 ;;
esac
echo "vendor=$VENDOR cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')" >&2

# ---- workload list --------------------------------------------------------
WORKLOADS=(cjk arabic hangul emoji mixed)

# Remaining sections wired up in Tasks B2 and B3.
echo "pre-flight OK; vendor=$VENDOR; driver build pending" >&2
