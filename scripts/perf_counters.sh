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

# ---- counter sets (per umbrella spec §Counter sets) ----------------------
# Each EVENT_FAMILY is an associative-array key; the value is the vendor-
# specific perf event name. `n/a` rows come from probe_event() below.
declare -A EVENT_INTEL=(
  [dtlb_walks]="dtlb_load_misses.miss_causes_a_walk"
  [l1d_miss]="mem_load_retired.l1_miss"
  [l2_miss]="mem_load_retired.l2_miss"
  [l3_miss]="mem_load_retired.l3_miss"
  [backend_stalls]="cycle_activity.stalls_backend"
  [br_misp]="br_misp_retired.all_branches"
  [dsb_uops]="idq.dsb_uops"
  [lsd_uops]="lsd.uops"
)
declare -A EVENT_AMD=(
  # Zen-5 event names verified on AMD Ryzen AI 9 HX PRO 370 (perf 6.12).
  # Rationale per family (umbrella spec §Counter sets):
  #   dtlb_walks     : L1-dTLB miss that also missed L2-dTLB -> forces a
  #                    page-table walk. `.all` alone would also count
  #                    L2-dTLB hits (no walk), which is noisier.
  #   l1d_miss       : all demand L1D fills (every fill implies a miss).
  #   l2_miss        : data-cache request that missed in L2 (served from
  #                    L3 or beyond).
  #   l3_miss        : demand fill from local-node DRAM -> L3 miss.
  #   backend_stalls : pipeline slots where the backend could not accept
  #                    uops. Name survived from Zen-4.
  #   br_misp        : retired mispredicted branches (unchanged).
  #   opcache_uops   : uops dispatched from op-cache (renamed from Zen-4
  #                    `de_src_op_dist.opcache`).
  #   decoder_uops   : uops dispatched from x86 decoder (renamed likewise).
  [dtlb_walks]="ls_l1_d_tlb_miss.all_l2_miss"
  [l1d_miss]="ls_dmnd_fills_from_sys.all"
  [l2_miss]="l2_cache_req_stat.ls_rd_blk_c"
  [l3_miss]="ls_dmnd_fills_from_sys.dram_io_near"
  [backend_stalls]="de_no_dispatch_per_slot.backend_stalls"
  [br_misp]="ex_ret_brn_misp"
  [opcache_uops]="de_src_op_disp.op_cache"
  [decoder_uops]="de_src_op_disp.x86_decoder"
)

declare -n EVENT_MAP=EVENT_${VENDOR^^}   # Bash nameref: EVENT_INTEL or EVENT_AMD

# probe_event "<name>" -> echoes the event name iff perf-stat accepts it,
# else "n/a". Uses `perf stat -e NAME --no-big-num true` as the canonical
# accept test (the same parser perf-stat will invoke during measurement).
# Substring matching against `perf list` output is insufficient on Zen 5,
# where short stems like `ls_l1_d_tlb_miss.tlb_reload` collide with longer
# siblings that perf-stat then rejects as "Bad event name".
probe_event() {
  local ev="$1"
  if perf stat -e "$ev" --no-big-num true >/dev/null 2>&1; then
    printf '%s' "$ev"
  else
    printf 'n/a'
  fi
}

# Resolve every family to either the event name (if available) or "n/a".
declare -A RESOLVED
for fam in "${!EVENT_MAP[@]}"; do
  RESOLVED[$fam]="$(probe_event "${EVENT_MAP[$fam]}")"
done

# Comma-joined event list for perf stat, skipping n/a families.
EVENT_CSV=""
declare -a FAM_ORDER=()
for fam in "${!RESOLVED[@]}"; do
  if [[ "${RESOLVED[$fam]}" != "n/a" ]]; then
    EVENT_CSV+="${RESOLVED[$fam]},"
    FAM_ORDER+=("$fam")
  fi
done
# Companion always-on counters (spec §Counter sets last paragraph).
EVENT_CSV+="task-clock,cycles,instructions,branches"
FAM_ORDER+=(task_clock cycles instructions branches)

# ---- workload list --------------------------------------------------------
WORKLOADS=(cjk arabic hangul emoji mixed)

# Remaining sections wired up in Tasks B2 and B3.
echo "resolved=${RESOLVED[*]}" >&2
echo "pre-flight OK; vendor=$VENDOR; driver build pending" >&2
