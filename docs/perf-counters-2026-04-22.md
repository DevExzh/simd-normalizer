# Perf counters — slow-path workloads (2026-04-22)

**Host:** AMD Ryzen AI 9 HX PRO 370 w/ Radeon 890M  |  **Kernel:** 6.8.0-51-generic  |  **perf:** 6.12.74
**Vendor counter set:** amd

## cjk

| counter | median | min | max | stddev | per-1k-insn |
| --- | ---: | ---: | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5683 | 5683 | 5683 | 0.00 | 0.002 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 3567 | 3567 | 3567 | 0.00 | 0.001 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2406858 | 2406858 | 2406858 | 0.00 | 1.010 |
| opcache_uops (`de_src_op_disp.op_cache`) | 2028255389 | 2028255389 | 2028255389 | 0.00 | 851.368 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 35527 | 35527 | 35527 | 0.00 | 0.015 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 39780 | 39780 | 39780 | 0.00 | 0.017 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 629812207 | 629812207 | 629812207 | 0.00 | 264.366 |
| br_misp (`ex_ret_brn_misp`) | 437724 | 437724 | 437724 | 0.00 | 0.184 |

## arabic

| counter | median | min | max | stddev | per-1k-insn |
| --- | ---: | ---: | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 4264 | 4264 | 4264 | 0.00 | 0.002 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 3761 | 3761 | 3761 | 0.00 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2283148 | 2283148 | 2283148 | 0.00 | 0.996 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1926125126 | 1926125126 | 1926125126 | 0.00 | 839.892 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 34193 | 34193 | 34193 | 0.00 | 0.015 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 43642 | 43642 | 43642 | 0.00 | 0.019 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 682837666 | 682837666 | 682837666 | 0.00 | 297.753 |
| br_misp (`ex_ret_brn_misp`) | 859202 | 859202 | 859202 | 0.00 | 0.375 |

## hangul

| counter | median | min | max | stddev | per-1k-insn |
| --- | ---: | ---: | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5081 | 5081 | 5081 | 0.00 | 0.002 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 4047 | 4047 | 4047 | 0.00 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2331285 | 2331285 | 2331285 | 0.00 | 1.075 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1828348217 | 1828348217 | 1828348217 | 0.00 | 843.305 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 35018 | 35018 | 35018 | 0.00 | 0.016 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 40991 | 40991 | 40991 | 0.00 | 0.019 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 135001358 | 135001358 | 135001358 | 0.00 | 62.268 |
| br_misp (`ex_ret_brn_misp`) | 551258 | 551258 | 551258 | 0.00 | 0.254 |

## emoji

| counter | median | min | max | stddev | per-1k-insn |
| --- | ---: | ---: | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 2741 | 2741 | 2741 | 0.00 | 0.001 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 3617 | 3617 | 3617 | 0.00 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2492692 | 2492692 | 2492692 | 0.00 | 1.058 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1927744645 | 1927744645 | 1927744645 | 0.00 | 818.570 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 37488 | 37488 | 37488 | 0.00 | 0.016 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 50966 | 50966 | 50966 | 0.00 | 0.022 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 811242316 | 811242316 | 811242316 | 0.00 | 344.474 |
| br_misp (`ex_ret_brn_misp`) | 221181 | 221181 | 221181 | 0.00 | 0.094 |

## mixed

| counter | median | min | max | stddev | per-1k-insn |
| --- | ---: | ---: | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5261 | 5261 | 5261 | 0.00 | 0.002 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 3790 | 3790 | 3790 | 0.00 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2567879 | 2567879 | 2567879 | 0.00 | 1.130 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1882804616 | 1882804616 | 1882804616 | 0.00 | 828.652 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 38602 | 38602 | 38602 | 0.00 | 0.017 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 43971 | 43971 | 43971 | 0.00 | 0.019 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 470636046 | 470636046 | 470636046 | 0.00 | 207.134 |
| br_misp (`ex_ret_brn_misp`) | 167697 | 167697 | 167697 | 0.00 | 0.074 |

## Hypothesis verdict (Phase 2 kill-criteria)

> Thresholds: C huge-pages dtlb/cyc > 2%; A2 MPHF (l2+l3)/insn > 0.5% (simplified from the umbrella's latency-weighted form); B fused backend_stalls/cyc > 15%; E1 pre-scan br_misp/branches > 3%; D3 vpternlogd dsb_ratio < 75%.

| hypothesis (child) | cjk | arabic | hangul | emoji | mixed |
| --- | :---: | :---: | :---: | :---: | :---: |
| C huge-pages | ✗ | ✗ | ✗ | ✗ | ✗ |
| A2 MPHF | ✗ | ✗ | ✗ | ✗ | ✗ |
| B fused decode | ✓ | ✓ | ✓ | ✓ | ✓ |
| E1 pre-scan | ✗ | ✗ | ✗ | ✗ | ✗ |
| D3 vpternlogd | ✗ | ✗ | ✗ | ✗ | ✗ |

