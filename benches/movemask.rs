//! Microbenchmark for the NEON movemask reduction.
//!
//! Compares three implementations on the dev host:
//!
//! - `current_vandq_vpaddq` — the previous reduction (`vandq` + `vpaddq×3`
//!   + `vget_lane×2`).
//! - `new_vaddv_halves` — the production implementation (lives in
//!   `src/simd/aarch64/neon.rs::simd_cmpge_mask`).
//! - `nibble_vshrn` — candidate 2 from the design spec, a `vshrn_n_u16(_, 4)`
//!   nibble-mask. Kept here only for measurement; not wired into production.
//!
//! Three input shapes:
//!
//! - `all_below_bound` — ASCII (every byte < bound).
//! - `all_above_bound` — every byte >= bound.
//! - `mixed` — half above, half below.
//!
//! Run with `cargo bench --bench movemask -- --quick`.

use criterion::{Criterion, criterion_group, criterion_main};

#[cfg(target_arch = "aarch64")]
mod neon_bench {
    use core::arch::aarch64::{
        uint8x16_t, vaddv_u8, vandq_u8, vcgeq_u8, vdupq_n_u8, vget_high_u8, vget_lane_u64,
        vget_low_u8, vgetq_lane_u8, vld1q_u8, vpaddq_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8,
        vshrn_n_u16,
    };
    use criterion::Criterion;
    use std::hint::black_box;

    const BIT_MASK: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];

    /// Old movemask: vandq + 3× vpaddq + 2× vgetq_lane.
    #[target_feature(enable = "neon")]
    #[inline]
    unsafe fn current_vandq_vpaddq(a: uint8x16_t, b: uint8x16_t) -> u32 {
        unsafe {
            let cmp = vcgeq_u8(a, b);
            let bit_mask = vld1q_u8(BIT_MASK.as_ptr());
            let masked = vandq_u8(cmp, bit_mask);
            let p1 = vpaddq_u8(masked, masked);
            let p2 = vpaddq_u8(p1, p1);
            let p3 = vpaddq_u8(p2, p2);
            let lo = vgetq_lane_u8(p3, 0) as u32;
            let hi = vgetq_lane_u8(p3, 1) as u32;
            lo | (hi << 8)
        }
    }

    /// New movemask: vandq + vget_low/high + 2× vaddv_u8.
    #[target_feature(enable = "neon")]
    #[inline]
    unsafe fn new_vaddv_halves(a: uint8x16_t, b: uint8x16_t) -> u32 {
        unsafe {
            let cmp = vcgeq_u8(a, b);
            let bit_mask = vld1q_u8(BIT_MASK.as_ptr());
            let masked = vandq_u8(cmp, bit_mask);
            let lo = vaddv_u8(vget_low_u8(masked)) as u32;
            let hi = vaddv_u8(vget_high_u8(masked)) as u32;
            lo | (hi << 8)
        }
    }

    /// Candidate 2: nibble mask via `vshrn_n_u16(_, 4)`. Returns a 64-bit
    /// nibble mask (each nibble is 0xF / 0x0). Kept here only for the
    /// microbench — not used in production because the consumer encoding
    /// would need to change.
    #[target_feature(enable = "neon")]
    #[inline]
    #[allow(unused_unsafe)]
    unsafe fn nibble_vshrn(a: uint8x16_t, b: uint8x16_t) -> u64 {
        unsafe {
            let cmp = vcgeq_u8(a, b);
            let narrow = vshrn_n_u16::<4>(vreinterpretq_u16_u8(cmp));
            vget_lane_u64::<0>(vreinterpret_u64_u8(narrow))
        }
    }

    fn make_input(kind: &str) -> [u8; 16] {
        match kind {
            "all_below_bound" => [0x41; 16],
            "all_above_bound" => [0xFF; 16],
            "mixed" => {
                let mut v = [0x41u8; 16];
                for (i, b) in v.iter_mut().enumerate() {
                    if i & 1 == 1 {
                        *b = 0xE0;
                    }
                }
                v
            },
            _ => unreachable!(),
        }
    }

    pub fn bench(c: &mut Criterion) {
        for kind in ["all_below_bound", "all_above_bound", "mixed"] {
            let mut group = c.benchmark_group(kind);
            let input = make_input(kind);

            group.bench_function("current_vandq_vpaddq", |bencher| {
                bencher.iter(|| unsafe {
                    let a = vld1q_u8(input.as_ptr());
                    let bnd = vdupq_n_u8(0xC0);
                    // Run a small inner loop to amortize Criterion overhead.
                    let mut acc = 0u32;
                    for _ in 0..16 {
                        acc ^= current_vandq_vpaddq(black_box(a), black_box(bnd));
                    }
                    black_box(acc)
                });
            });

            group.bench_function("new_vaddv_halves", |bencher| {
                bencher.iter(|| unsafe {
                    let a = vld1q_u8(input.as_ptr());
                    let bnd = vdupq_n_u8(0xC0);
                    let mut acc = 0u32;
                    for _ in 0..16 {
                        acc ^= new_vaddv_halves(black_box(a), black_box(bnd));
                    }
                    black_box(acc)
                });
            });

            group.bench_function("nibble_vshrn", |bencher| {
                bencher.iter(|| unsafe {
                    let a = vld1q_u8(input.as_ptr());
                    let bnd = vdupq_n_u8(0xC0);
                    let mut acc = 0u64;
                    for _ in 0..16 {
                        acc ^= nibble_vshrn(black_box(a), black_box(bnd));
                    }
                    black_box(acc)
                });
            });

            group.finish();
        }
    }
}

#[cfg(target_arch = "aarch64")]
fn movemask_bench(c: &mut Criterion) {
    neon_bench::bench(c);
}

#[cfg(not(target_arch = "aarch64"))]
fn movemask_bench(_c: &mut Criterion) {
    // Microbench is aarch64-only; on other targets it is a no-op so the
    // bench file still compiles in CI.
}

criterion_group!(benches, movemask_bench);
criterion_main!(benches);
