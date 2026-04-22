# Perf counters — slow-path workloads (2026-04-22)

**Host:** AMD Ryzen AI 9 HX PRO 370 w/ Radeon 890M  |  **Kernel:** 6.8.0-51-generic  |  **perf:** 6.12.74
**Vendor counter set:** amd

## cjk

| counter | value | cv% | per-1k-insn |
| --- | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5696 | 6.80 | 0.006 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 29076 | 22.43 | 0.033 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 3336494 | 5.23 | 3.743 |
| opcache_uops (`de_src_op_disp.op_cache`) | 800434088 | 9.12 | 897.885 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 44826 | 8.44 | 0.050 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 69504 | 11.40 | 0.078 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 219746906 | 12.21 | 246.501 |
| br_misp (`ex_ret_brn_misp`) | 918118 | 7.86 | 1.030 |

## arabic

| counter | value | cv% | per-1k-insn |
| --- | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 8066 | 15.29 | 0.005 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 14741 | 36.07 | 0.009 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2566369 | 7.52 | 1.627 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1354523944 | 6.27 | 858.828 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 33501 | 5.10 | 0.021 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 40706 | 9.31 | 0.026 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 487486918 | 7.17 | 309.088 |
| br_misp (`ex_ret_brn_misp`) | 863629 | 10.06 | 0.548 |

## hangul

| counter | value | cv% | per-1k-insn |
| --- | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 6439 | 19.35 | 0.004 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 3980 | 5.47 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2251774 | 2.68 | 1.315 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1453874869 | 7.85 | 848.793 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 33739 | 4.21 | 0.020 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 43182 | 7.60 | 0.025 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 109275299 | 8.18 | 63.796 |
| br_misp (`ex_ret_brn_misp`) | 437525 | 7.23 | 0.255 |

## emoji

| counter | value | cv% | per-1k-insn |
| --- | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5360 | 14.56 | 0.003 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 4413 | 5.14 | 0.002 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2407407 | 2.64 | 1.170 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1700341466 | 4.25 | 826.174 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 37090 | 3.37 | 0.018 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 43787 | 5.43 | 0.021 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 706054027 | 3.74 | 343.062 |
| br_misp (`ex_ret_brn_misp`) | 192139 | 6.28 | 0.093 |

## mixed

| counter | value | cv% | per-1k-insn |
| --- | ---: | ---: | ---: |
| l3_miss (`ls_dmnd_fills_from_sys.dram_io_near`) | 5527 | 18.85 | 0.003 |
| dtlb_walks (`ls_l1_d_tlb_miss.all_l2_miss`) | 5778 | 15.32 | 0.003 |
| decoder_uops (`de_src_op_disp.x86_decoder`) | 2476051 | 2.62 | 1.187 |
| opcache_uops (`de_src_op_disp.op_cache`) | 1738291605 | 2.48 | 833.240 |
| l2_miss (`l2_cache_req_stat.ls_rd_blk_c`) | 35344 | 1.91 | 0.017 |
| l1d_miss (`ls_dmnd_fills_from_sys.all`) | 43377 | 7.50 | 0.021 |
| backend_stalls (`de_no_dispatch_per_slot.backend_stalls`) | 405943370 | 2.88 | 194.586 |
| br_misp (`ex_ret_brn_misp`) | 242884 | 24.33 | 0.116 |

## Hypothesis verdict (Phase 2 kill-criteria)

> Thresholds: C huge-pages dtlb/cyc > 2%; A2 MPHF (l2+l3)/insn > 0.5% (simplified from the umbrella's latency-weighted form); B fused backend_stalls/cyc > 15%; E1 pre-scan br_misp/branches > 3%; D3 vpternlogd dsb_ratio < 75%.

| hypothesis (child) | cjk | arabic | hangul | emoji | mixed |
| --- | :---: | :---: | :---: | :---: | :---: |
| C huge-pages | ✗ | ✗ | ✗ | ✗ | ✗ |
| A2 MPHF | ✗ | ✗ | ✗ | ✗ | ✗ |
| B fused decode | ✓ | ✓ | ✓ | ✓ | ✓ |
| E1 pre-scan | ✗ | ✗ | ✗ | ✗ | ✗ |
| D3 vpternlogd | ✗ | ✗ | ✗ | ✗ | ✗ |

