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

# ---- build ---------------------------------------------------------------
cargo build --release --bin perf_driver >&2

# ---- measurement --------------------------------------------------------
REPEATS="${DIAG_REPEATS:-10}"
SMOKE="${DIAG_SMOKE:-}"
if [[ -n "$SMOKE" ]]; then
  REPEATS=1
fi

RESULTS_DIR="$(mktemp -d -t diag-perf-XXXXXX)"
echo "results_dir=$RESULTS_DIR repeats=$REPEATS smoke=${SMOKE:-0}" >&2

for w in "${WORKLOADS[@]}"; do
  OUT="$RESULTS_DIR/${w}.csv"
  echo "measuring workload=$w -> $OUT" >&2
  # perf stat:
  #   -r N            : repeat N runs (stddev reported in CSV)
  #   -x,             : comma-separated output, one row per event per run
  #   --log-fd 3      : send perf's own CSV to fd 3 (our file); keep driver's
  #                     stderr out of the CSV.
  DIAG_SMOKE="$SMOKE" perf stat \
    -r "$REPEATS" \
    -x, \
    -e "$EVENT_CSV" \
    --log-fd 3 \
    target/release/perf_driver "$w" \
    3>"$OUT" \
    >/dev/null 2>>"$RESULTS_DIR/${w}.driver.log"
done

# Export for Part C (report generation).
export RESULTS_DIR FAM_ORDER RESOLVED VENDOR
# `FAM_ORDER` is an indexed array → serialize to a file Part C can read.
printf '%s\n' "${FAM_ORDER[@]}" > "$RESULTS_DIR/fam_order.txt"
for fam in "${!RESOLVED[@]}"; do
  printf '%s\t%s\n' "$fam" "${RESOLVED[$fam]}" >> "$RESULTS_DIR/resolved.tsv"
done
echo "measurement done" >&2
# Part C appended below this line.

# ---- report generation ---------------------------------------------------
REPORT="docs/perf-counters-$(date -u +%Y-%m-%d).md"
{
  echo "# Perf counters — slow-path workloads ($(date -u +%Y-%m-%d))"
  echo
  echo "**Host:** $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')  |  **Kernel:** $(uname -r)  |  **perf:** $(perf --version | awk '{print $3}')"
  echo "**Vendor counter set:** $VENDOR"
  echo
} > "$REPORT"

for w in "${WORKLOADS[@]}"; do
  CSV="$RESULTS_DIR/${w}.csv"
  {
    echo "## $w"
    echo
    echo "| counter | median | min | max | stddev | per-1k-insn |"
    echo "| --- | ---: | ---: | ---: | ---: | ---: |"
    declare -A MED
    while IFS= read -r line; do
      ev=$(echo "$line" | awk '{print $1}')
      counts=$(echo "$line" | cut -d' ' -f2-)
      med=$(printf '%s\n' $counts | sort -n | awk '
        { a[NR]=$1 } END {
          n=NR; if (n==0){print 0; exit}
          if (n%2==1) print a[(n+1)/2]; else print (a[n/2]+a[n/2+1])/2
        }')
      MED[$ev]=$med
    done < <(awk -F, '
      $3 != "" && $1 ~ /^[0-9.]+$/ { vals[$3] = vals[$3] " " $1 }
      END { for (e in vals) print e, vals[e] }
    ' "$CSV")
    INSN=${MED[instructions]:-0}
    mapfile -t FAMS < "$RESULTS_DIR/fam_order.txt"
    while IFS=$'\t' read -r fam ev; do
      if [[ "$ev" == "n/a" ]]; then
        echo "| $fam | n/a | n/a | n/a | n/a | n/a |"
        continue
      fi
      m=${MED[$ev]:-0}
      per1k=$(awk -v m="$m" -v i="$INSN" 'BEGIN{ if (i>0) printf "%.3f", m*1000/i; else print "n/a" }')
      stats=$(awk -F, -v ev="$ev" '$3==ev && $1 ~ /^[0-9.]+$/ {print $1}' "$CSV" \
        | sort -n \
        | awk '{ a[NR]=$1 }
          END {
            n=NR; if (n==0){print "-,-,-"; exit}
            min=a[1]; max=a[n]
            sum=0; for (i=1;i<=n;i++) sum+=a[i]; mean=sum/n
            sq=0; for (i=1;i<=n;i++) sq+=(a[i]-mean)^2
            sd=(n>1)?sqrt(sq/(n-1)):0
            printf "%d,%d,%.2f", min, max, sd
          }')
      mn=$(echo "$stats" | cut -d, -f1)
      mx=$(echo "$stats" | cut -d, -f2)
      sd=$(echo "$stats" | cut -d, -f3)
      echo "| $fam (\`$ev\`) | $m | $mn | $mx | $sd | $per1k |"
    done < "$RESULTS_DIR/resolved.tsv"
    echo
  } >> "$REPORT"
done
