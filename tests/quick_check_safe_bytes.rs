//! Layer 1 (design spec §3): exhaustive soundness audit for the
//! `quick_check` safe-lead byte set. For every byte in the proposed
//! safe set, enumerate all 3-byte UTF-8 sequences that start with it
//! (4 096 code points per lead) and assert the CCC+QC packed lookup
//! returns `(0, 0)` under every form-shift that claims the byte.

use simd_normalizer::tables_ext::{
    CCC_QC_NFC_SHIFT, CCC_QC_NFD_SHIFT, CCC_QC_NFKC_SHIFT, CCC_QC_NFKD_SHIFT, lookup_ccc_qc,
};

// (lead_byte, [shifts that claim it])
const SAFE: &[(u8, &[u32])] = &[
    (
        0xE4,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (
        0xE5,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (
        0xE6,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (
        0xE7,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (
        0xE8,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (
        0xE9,
        &[
            CCC_QC_NFC_SHIFT,
            CCC_QC_NFD_SHIFT,
            CCC_QC_NFKC_SHIFT,
            CCC_QC_NFKD_SHIFT,
        ],
    ),
    (0xEB, &[CCC_QC_NFC_SHIFT, CCC_QC_NFKC_SHIFT]),
    (0xEC, &[CCC_QC_NFC_SHIFT, CCC_QC_NFKC_SHIFT]),
];

#[test]
fn safe_bytes_are_truly_safe() {
    for &(lead, shifts) in SAFE {
        // 3-byte UTF-8: 1110xxxx 10yyyyyy 10zzzzzz -> cp in 0x0000..=0xFFFF
        let base = ((lead & 0x0F) as u32) << 12;
        for low12 in 0u32..4096 {
            let cp = base | low12;
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            for &shift in shifts {
                assert_eq!(
                    lookup_ccc_qc(c, shift),
                    (0, 0),
                    "lead=0x{:02X} cp=U+{:04X} shift={} not safe",
                    lead,
                    cp,
                    shift
                );
            }
        }
    }
}
