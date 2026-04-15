#!/usr/bin/env python3
"""
Generate Rust source files for simd-normalizer from the Unicode Character Database.

Usage:
    python3 scripts/generate_tables.py

Downloads UCD files for Unicode 17.0 and generates:
    src/tables/decomposition.rs
    src/tables/composition.rs
    src/tables/ccc.rs
    src/tables/qc.rs
"""

import os
import sys
import urllib.request
import hashlib
from pathlib import Path
from collections import defaultdict

UNICODE_VERSION = "17.0.0"
UCD_BASE_URL = f"https://www.unicode.org/Public/{UNICODE_VERSION}/ucd"

UCD_FILES = {
    "UnicodeData.txt": f"{UCD_BASE_URL}/UnicodeData.txt",
    "CompositionExclusions.txt": f"{UCD_BASE_URL}/CompositionExclusions.txt",
    "DerivedNormalizationProps.txt": f"{UCD_BASE_URL}/DerivedNormalizationProps.txt",
    "NormalizationTest.txt": f"{UCD_BASE_URL}/NormalizationTest.txt",
    "CaseFolding.txt": f"{UCD_BASE_URL}/CaseFolding.txt",
}

SECURITY_BASE_URL = "https://www.unicode.org/Public/security/latest"
SECURITY_FILES = {
    "confusables.txt": f"{SECURITY_BASE_URL}/confusables.txt",
}

SCRIPT_DIR = Path(__file__).resolve().parent
CACHE_DIR = SCRIPT_DIR / "ucd_cache"
PROJECT_ROOT = SCRIPT_DIR.parent
OUTPUT_DIR = PROJECT_ROOT / "src" / "tables"
TEST_DATA_DIR = PROJECT_ROOT / "tests" / "data"

# Hangul constants
S_BASE = 0xAC00
L_BASE = 0x1100
V_BASE = 0x1161
T_BASE = 0x11A7
L_COUNT = 19
V_COUNT = 21
T_COUNT = 28
N_COUNT = V_COUNT * T_COUNT
S_COUNT = L_COUNT * N_COUNT


# ---------------------------------------------------------------------------
# Step 1: Download UCD files
# ---------------------------------------------------------------------------

def download_ucd_files():
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    for filename, url in UCD_FILES.items():
        dest = CACHE_DIR / filename
        if dest.exists():
            print(f"  [cached] {filename}")
            continue
        print(f"  [download] {filename} from {url}")
        urllib.request.urlretrieve(url, dest)
    for filename, url in SECURITY_FILES.items():
        dest = CACHE_DIR / filename
        if dest.exists():
            print(f"  [cached] {filename}")
            continue
        print(f"  [download] {filename} from {url}")
        urllib.request.urlretrieve(url, dest)
    print()


# ---------------------------------------------------------------------------
# Step 2: Parse UnicodeData.txt
# ---------------------------------------------------------------------------

def parse_unicode_data():
    path = CACHE_DIR / "UnicodeData.txt"
    ccc_map = {}
    canon_decomp = {}
    compat_decomp = {}
    char_names = {}
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            fields = line.split(";")
            cp = int(fields[0], 16)
            name = fields[1]
            ccc = int(fields[3])
            decomp_field = fields[5].strip()
            char_names[cp] = name
            if ccc != 0:
                ccc_map[cp] = ccc
            if decomp_field:
                if decomp_field.startswith("<"):
                    tag_end = decomp_field.index(">")
                    mapping_str = decomp_field[tag_end + 1:].strip()
                    if mapping_str:
                        cps = [int(x, 16) for x in mapping_str.split()]
                        compat_decomp[cp] = cps
                else:
                    cps = [int(x, 16) for x in decomp_field.split()]
                    canon_decomp[cp] = cps
    return ccc_map, canon_decomp, compat_decomp, char_names


# ---------------------------------------------------------------------------
# Step 3: Parse CompositionExclusions.txt
# ---------------------------------------------------------------------------

def parse_composition_exclusions():
    path = CACHE_DIR / "CompositionExclusions.txt"
    exclusions = set()
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "#" in line:
                line = line[:line.index("#")].strip()
            if not line:
                continue
            cp = int(line.split()[0], 16)
            exclusions.add(cp)
    return exclusions


# ---------------------------------------------------------------------------
# Step 4: Parse DerivedNormalizationProps.txt
# ---------------------------------------------------------------------------

def parse_derived_normalization_props():
    path = CACHE_DIR / "DerivedNormalizationProps.txt"
    qc_props = defaultdict(dict)
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "#" in line:
                line = line[:line.index("#")].strip()
            if not line:
                continue
            parts = line.split(";")
            if len(parts) < 2:
                continue
            range_str = parts[0].strip()
            prop_name = parts[1].strip()
            if prop_name not in ("NFC_QC", "NFD_QC", "NFKC_QC", "NFKD_QC"):
                continue
            if len(parts) < 3:
                continue
            value = parts[2].strip()
            if ".." in range_str:
                start_str, end_str = range_str.split("..")
                start = int(start_str, 16)
                end = int(end_str, 16)
            else:
                start = int(range_str, 16)
                end = start
            for cp in range(start, end + 1):
                qc_props[prop_name][cp] = value
    return dict(qc_props)


# ---------------------------------------------------------------------------
# Step 5: Full recursive decomposition builder
# ---------------------------------------------------------------------------

def build_full_decompositions(canon_decomp, compat_decomp, ccc_map):
    def _recursive_canonical(cp, memo):
        if cp in memo:
            return memo[cp]
        if cp not in canon_decomp:
            memo[cp] = [cp]
            return [cp]
        result = []
        for sub_cp in canon_decomp[cp]:
            result.extend(_recursive_canonical(sub_cp, memo))
        memo[cp] = result
        return result

    def _recursive_compat(cp, memo, canon_memo):
        if cp in memo:
            return memo[cp]
        if cp in compat_decomp:
            raw = compat_decomp[cp]
        elif cp in canon_decomp:
            raw = canon_decomp[cp]
        else:
            memo[cp] = [cp]
            return [cp]
        result = []
        for sub_cp in raw:
            result.extend(_recursive_compat(sub_cp, memo, canon_memo))
        memo[cp] = result
        return result

    canon_memo = {}
    full_canon = {}
    for cp in canon_decomp:
        decomp = _recursive_canonical(cp, canon_memo)
        if decomp != [cp]:
            full_canon[cp] = decomp

    compat_memo = {}
    full_compat = {}
    all_compat_cps = set(canon_decomp.keys()) | set(compat_decomp.keys())
    for cp in all_compat_cps:
        decomp = _recursive_compat(cp, compat_memo, canon_memo)
        if decomp != [cp]:
            full_compat[cp] = decomp

    return full_canon, full_compat


# ---------------------------------------------------------------------------
# Step 6: Composition pair extractor
# ---------------------------------------------------------------------------

def build_composition_pairs(canon_decomp, exclusions, ccc_map):
    pairs = []
    for cp, decomp in canon_decomp.items():
        if len(decomp) != 2:
            continue
        a, b = decomp
        if S_BASE <= cp < S_BASE + S_COUNT:
            continue
        if cp in exclusions:
            continue
        if ccc_map.get(a, 0) != 0:
            continue
        pairs.append((a, b, cp))
    pairs.sort(key=lambda t: (t[0], t[1]))
    return pairs


# ---------------------------------------------------------------------------
# Step 7: Trie builder with block deduplication
# ---------------------------------------------------------------------------

BLOCK_SIZE = 32


class TrieBuilder:
    def __init__(self, default_value=0):
        self.default_value = default_value
        self.cp_values = {}

    def set(self, cp, value):
        if value != self.default_value:
            self.cp_values[cp] = value

    def build(self):
        bmp_blocks = []
        for block_start in range(0, 0x10000, BLOCK_SIZE):
            block = []
            for offset in range(BLOCK_SIZE):
                cp = block_start + offset
                block.append(self.cp_values.get(cp, self.default_value))
            bmp_blocks.append(tuple(block))

        supp_data_blocks = []
        for block_start in range(0x10000, 0x110000, BLOCK_SIZE):
            block = []
            for offset in range(BLOCK_SIZE):
                cp = block_start + offset
                block.append(self.cp_values.get(cp, self.default_value))
            supp_data_blocks.append(tuple(block))

        block_to_offset = {}
        data = []

        def intern_block(block):
            if block in block_to_offset:
                return block_to_offset[block]
            offset = len(data)
            block_to_offset[block] = offset
            data.extend(block)
            return offset

        default_block = tuple([self.default_value] * BLOCK_SIZE)
        intern_block(default_block)

        bmp_index = []
        for block in bmp_blocks:
            bmp_index.append(intern_block(block))

        supp_data_offsets = []
        for block in supp_data_blocks:
            supp_data_offsets.append(intern_block(block))

        l2_block_size = 64
        num_l1_entries = (0x110000 - 0x10000) // 2048

        supp_l2_blocks = []
        for l1_idx in range(num_l1_entries):
            start = l1_idx * l2_block_size
            end = start + l2_block_size
            l2_block = tuple(supp_data_offsets[start:end])
            supp_l2_blocks.append(l2_block)

        l2_block_to_offset = {}
        supp_index2 = []

        def intern_l2_block(block):
            if block in l2_block_to_offset:
                return l2_block_to_offset[block]
            offset = len(supp_index2)
            l2_block_to_offset[block] = offset
            supp_index2.extend(block)
            return offset

        default_l2 = tuple([intern_block(default_block)] * l2_block_size)
        intern_l2_block(default_l2)

        supp_index1 = []
        for l2_block in supp_l2_blocks:
            supp_index1.append(intern_l2_block(l2_block))

        return bmp_index, data, supp_index1, supp_index2


# ---------------------------------------------------------------------------
# Step 8: U32 trie value packer
# ---------------------------------------------------------------------------

BACKWARD_COMBINING = 1 << 31
NON_ROUND_TRIP     = 1 << 30
HAS_DECOMPOSITION  = 1 << 29
IS_EXPANSION       = 1 << 24
CCC_SHIFT          = 16
CCC_MASK           = 0xFF << CCC_SHIFT
DECOMP_INFO_MASK   = 0xFFFF


def pack_trie_value(ccc=0, has_decomp=False, decomp_info=0,
                    backward_combining=False, non_round_trip=False,
                    is_expansion=False):
    value = 0
    if backward_combining:
        value |= BACKWARD_COMBINING
    if non_round_trip:
        value |= NON_ROUND_TRIP
    if has_decomp:
        value |= HAS_DECOMPOSITION
    if is_expansion:
        value |= IS_EXPANSION
    value |= (ccc & 0xFF) << CCC_SHIFT
    value |= decomp_info & DECOMP_INFO_MASK
    return value


def pack_singleton_decomp(target_cp, ccc=0, non_round_trip=False,
                          backward_combining=False):
    assert target_cp <= 0xFFFD, f"Singleton target too large: U+{target_cp:04X}"
    return pack_trie_value(
        ccc=ccc, has_decomp=True, decomp_info=target_cp,
        backward_combining=backward_combining, non_round_trip=non_round_trip,
    )


def pack_expansion_decomp(offset, ccc=0, non_round_trip=False,
                           backward_combining=False):
    assert offset >= 0 and offset <= 0xFFFF, f"Expansion offset too large: {offset}"
    return pack_trie_value(
        ccc=ccc, has_decomp=True, decomp_info=offset,
        backward_combining=backward_combining, non_round_trip=non_round_trip,
        is_expansion=True,
    )


def build_decomp_trie(full_decomp, ccc_map, qc_props, qc_name_nrt):
    trie = TrieBuilder()
    expansions = []
    expansion_index = {}
    nrt_set = set()
    if qc_name_nrt in qc_props:
        nrt_set = set(qc_props[qc_name_nrt].keys())

    for cp in range(0x110000):
        ccc = ccc_map.get(cp, 0)
        is_nrt = cp in nrt_set
        if cp in full_decomp:
            decomp = full_decomp[cp]
            backward = ccc_map.get(decomp[0], 0) != 0 if decomp else False
            if len(decomp) == 1 and decomp[0] <= 0xFFFD:
                value = pack_singleton_decomp(
                    decomp[0], ccc=ccc, non_round_trip=is_nrt,
                    backward_combining=backward,
                )
            else:
                decomp_tuple = tuple(decomp)
                if decomp_tuple in expansion_index:
                    offset = expansion_index[decomp_tuple]
                else:
                    offset = len(expansions)
                    expansion_index[decomp_tuple] = offset
                    # Store length as prefix, then packed (ccc << 21 | code_point) entries
                    exp_len = len(decomp)
                    expansions.append(exp_len)
                    for dcp in decomp:
                        dcp_ccc = ccc_map.get(dcp, 0)
                        expansions.append((dcp_ccc << 21) | dcp)
                if offset > 0xFFFF:
                    print(f"  WARNING: skipping U+{cp:04X}: expansion offset={offset} exceeds 16 bits")
                    continue
                value = pack_expansion_decomp(
                    offset, ccc=ccc, non_round_trip=is_nrt,
                    backward_combining=backward,
                )
        elif ccc != 0:
            value = pack_trie_value(ccc=ccc, non_round_trip=is_nrt)
        elif is_nrt:
            value = pack_trie_value(non_round_trip=True)
        else:
            continue
        trie.set(cp, value)
    return trie, expansions


# ---------------------------------------------------------------------------
# Step 9: Rust code generation helpers
# ---------------------------------------------------------------------------

def format_u16_array(name, data, doc=None):
    lines = []
    if doc:
        lines.append(f"/// {doc}")
    lines.append(f"pub(crate) static {name}: &[u16] = &[")
    for i in range(0, len(data), 16):
        chunk = data[i:i + 16]
        vals = ", ".join(f"0x{v:04X}" for v in chunk)
        lines.append(f"    {vals},")
    lines.append("];")
    return "\n".join(lines)


def format_u32_array(name, data, doc=None):
    lines = []
    if doc:
        lines.append(f"/// {doc}")
    lines.append(f"pub(crate) static {name}: &[u32] = &[")
    for i in range(0, len(data), 8):
        chunk = data[i:i + 8]
        vals = ", ".join(f"0x{v:08X}" for v in chunk)
        lines.append(f"    {vals},")
    lines.append("];")
    return "\n".join(lines)


def format_pair_array(name, pairs, doc=None):
    lines = []
    if doc:
        lines.append(f"/// {doc}")
    # Use u64 for packed pair since starter or combining can be supplementary
    lines.append(f"pub(crate) static {name}: &[(u64, u32)] = &[")
    for a, b, composed in pairs:
        packed = (a << 21) | b
        lines.append(f"    (0x{packed:012X}, 0x{composed:08X}),")
    lines.append("];")
    return "\n".join(lines)


HEADER = """\
// AUTO-GENERATED by scripts/generate_tables.py -- DO NOT EDIT
//
// Unicode version: {version}
"""


def write_decomposition_rs(canon_bmp_idx, canon_data, canon_s1, canon_s2,
                           canon_expansions,
                           compat_bmp_idx, compat_data, compat_s1, compat_s2,
                           compat_expansions):
    path = OUTPUT_DIR / "decomposition.rs"
    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append(format_u16_array("CANONICAL_BMP_INDEX", canon_bmp_idx,
                                  "BMP block index for canonical decomposition trie."))
    parts.append("")
    parts.append(format_u32_array("CANONICAL_TRIE_DATA", canon_data,
                                  "Trie data for canonical decomposition."))
    parts.append("")
    parts.append(format_u16_array("CANONICAL_SUPP_INDEX1", canon_s1,
                                  "Supplementary level-1 index for canonical decomposition."))
    parts.append("")
    parts.append(format_u16_array("CANONICAL_SUPP_INDEX2", canon_s2,
                                  "Supplementary level-2 index for canonical decomposition."))
    parts.append("")
    parts.append(format_u32_array("CANONICAL_EXPANSIONS", canon_expansions,
                                  "Canonical expansion table: each entry = (ccc << 21) | code_point."))
    parts.append("")
    parts.append(format_u16_array("COMPAT_BMP_INDEX", compat_bmp_idx,
                                  "BMP block index for compatibility decomposition trie."))
    parts.append("")
    parts.append(format_u32_array("COMPAT_TRIE_DATA", compat_data,
                                  "Trie data for compatibility decomposition."))
    parts.append("")
    parts.append(format_u16_array("COMPAT_SUPP_INDEX1", compat_s1,
                                  "Supplementary level-1 index for compatibility decomposition."))
    parts.append("")
    parts.append(format_u16_array("COMPAT_SUPP_INDEX2", compat_s2,
                                  "Supplementary level-2 index for compatibility decomposition."))
    parts.append("")
    parts.append(format_u32_array("COMPAT_EXPANSIONS", compat_expansions,
                                  "Compatibility expansion table: each entry = (ccc << 21) | code_point."))
    parts.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")


def write_composition_rs(composition_pairs):
    path = OUTPUT_DIR / "composition.rs"
    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append(format_pair_array(
        "COMPOSITION_PAIRS", composition_pairs,
        "Canonical composition pairs: (packed_pair, composed_char).\n"
        "/// packed_pair = (starter << 21) | combining.\n"
        "/// Sorted by packed_pair for binary search."
    ))
    parts.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")


# ---------------------------------------------------------------------------
# Step 10: CCC trie generation
# ---------------------------------------------------------------------------

def write_ccc_rs(ccc_map):
    path = OUTPUT_DIR / "ccc.rs"
    trie = TrieBuilder()
    for cp, ccc in ccc_map.items():
        trie.set(cp, ccc)
    bmp_idx, data, s1, s2 = trie.build()
    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append(format_u16_array("CCC_BMP_INDEX", bmp_idx,
                                  "BMP block index for CCC trie."))
    parts.append("")
    parts.append(format_u32_array("CCC_TRIE_DATA", data,
                                  "Trie data for canonical combining class lookup."))
    parts.append("")
    parts.append(format_u16_array("CCC_SUPP_INDEX1", s1,
                                  "Supplementary level-1 index for CCC trie."))
    parts.append("")
    parts.append(format_u16_array("CCC_SUPP_INDEX2", s2,
                                  "Supplementary level-2 index for CCC trie."))
    parts.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")


# ---------------------------------------------------------------------------
# Step 11: Quick-check property generation
# ---------------------------------------------------------------------------

def write_qc_rs(qc_props):
    path = OUTPUT_DIR / "qc.rs"
    qc_value_map = {"Y": 0, "M": 1, "N": 2}
    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append("// Quick-check values: Y=0, M=1, N=2.")
    parts.append("")
    for prop_name in ["NFC_QC", "NFD_QC", "NFKC_QC", "NFKD_QC"]:
        entries = qc_props.get(prop_name, {})
        trie = TrieBuilder()
        for cp, value_str in entries.items():
            value = qc_value_map.get(value_str, 0)
            if value != 0:
                trie.set(cp, value)
        bmp_idx, data, s1, s2 = trie.build()
        prefix = prop_name.upper().replace("_", "_")
        parts.append(format_u16_array(f"{prefix}_BMP_INDEX", bmp_idx,
                                      f"BMP block index for {prop_name} quick-check trie."))
        parts.append("")
        parts.append(format_u32_array(f"{prefix}_TRIE_DATA", data,
                                      f"Trie data for {prop_name} quick-check."))
        parts.append("")
        parts.append(format_u16_array(f"{prefix}_SUPP_INDEX1", s1,
                                      f"Supplementary level-1 index for {prop_name} quick-check."))
        parts.append("")
        parts.append(format_u16_array(f"{prefix}_SUPP_INDEX2", s2,
                                      f"Supplementary level-2 index for {prop_name} quick-check."))
        parts.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")


# ---------------------------------------------------------------------------
# Step 11b: Fused CCC + QC trie generation
# ---------------------------------------------------------------------------

def write_ccc_qc_rs(ccc_map, qc_props):
    """Generate a fused CCC+QC trie.

    Packed u32 value per code point:
        Bits [15..8]: CCC value (0-255)
        Bits [7..6]:  NFKD_QC (0=Y, 1=M, 2=N)
        Bits [5..4]:  NFKC_QC
        Bits [3..2]:  NFD_QC
        Bits [1..0]:  NFC_QC
    """
    path = OUTPUT_DIR / "ccc_qc.rs"
    qc_value_map = {"Y": 0, "M": 1, "N": 2}

    # Build the fused value for every code point that has non-zero CCC or non-Yes QC.
    trie = TrieBuilder()
    # Collect all code points that need entries.
    all_cps = set(ccc_map.keys())
    for prop_entries in qc_props.values():
        all_cps.update(prop_entries.keys())

    for cp in all_cps:
        ccc = ccc_map.get(cp, 0)
        nfc_qc = qc_value_map.get(qc_props.get("NFC_QC", {}).get(cp, "Y"), 0)
        nfd_qc = qc_value_map.get(qc_props.get("NFD_QC", {}).get(cp, "Y"), 0)
        nfkc_qc = qc_value_map.get(qc_props.get("NFKC_QC", {}).get(cp, "Y"), 0)
        nfkd_qc = qc_value_map.get(qc_props.get("NFKD_QC", {}).get(cp, "Y"), 0)
        packed = (ccc << 8) | (nfkd_qc << 6) | (nfkc_qc << 4) | (nfd_qc << 2) | nfc_qc
        if packed != 0:
            trie.set(cp, packed)

    bmp_idx, data, s1, s2 = trie.build()
    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append("// Fused CCC + QC trie: (ccc << 8) | (nfkd_qc << 6) | (nfkc_qc << 4) | (nfd_qc << 2) | nfc_qc")
    parts.append("// QC values: Y=0, M=1, N=2.")
    parts.append("")
    parts.append(format_u16_array("CCC_QC_BMP_INDEX", bmp_idx,
                                  "BMP block index for fused CCC+QC trie."))
    parts.append("")
    parts.append(format_u32_array("CCC_QC_TRIE_DATA", data,
                                  "Trie data for fused CCC + quick-check lookup."))
    parts.append("")
    parts.append(format_u16_array("CCC_QC_SUPP_INDEX1", s1,
                                  "Supplementary level-1 index for fused CCC+QC trie."))
    parts.append("")
    parts.append(format_u16_array("CCC_QC_SUPP_INDEX2", s2,
                                  "Supplementary level-2 index for fused CCC+QC trie."))
    parts.append("")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")

# ---------------------------------------------------------------------------
# Step 12: Parse CaseFolding.txt and generate casefold trie
# ---------------------------------------------------------------------------

def parse_case_folding():
    """Parse CaseFolding.txt.

    Returns:
        simple_folds: dict of cp -> target_cp for status C and S
        turkish_folds: dict of cp -> target_cp for status T
    """
    path = CACHE_DIR / "CaseFolding.txt"
    simple_folds = {}
    turkish_folds = {}
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "#" in line:
                line = line[:line.index("#")].strip()
            if not line:
                continue
            parts = line.split(";")
            if len(parts) < 3:
                continue
            cp = int(parts[0].strip(), 16)
            status = parts[1].strip()
            mapping_str = parts[2].strip()
            if status in ("C", "S"):
                # Simple case folding: single code point mapping
                target_cps = mapping_str.split()
                if len(target_cps) == 1:
                    simple_folds[cp] = int(target_cps[0], 16)
            elif status == "T":
                # Turkish-specific folding
                target_cps = mapping_str.split()
                if len(target_cps) == 1:
                    turkish_folds[cp] = int(target_cps[0], 16)
    return simple_folds, turkish_folds


def write_casefold_rs(simple_folds, turkish_folds):
    """Generate src/tables/casefold.rs with a case folding trie."""
    path = OUTPUT_DIR / "casefold.rs"

    # Build the trie: value = target code point, 0 = no folding
    trie = TrieBuilder()
    for cp, target in simple_folds.items():
        trie.set(cp, target)
    bmp_idx, data, s1, s2 = trie.build()

    parts = [HEADER.format(version=UNICODE_VERSION)]
    parts.append(format_u16_array("CASEFOLD_BMP_INDEX", bmp_idx,
                                  "BMP block index for case folding trie."))
    parts.append("")
    parts.append(format_u32_array("CASEFOLD_TRIE_DATA", data,
                                  "Trie data for simple case folding (value = target code point, 0 = identity)."))
    parts.append("")
    parts.append(format_u16_array("CASEFOLD_SUPP_INDEX1", s1,
                                  "Supplementary level-1 index for case folding trie."))
    parts.append("")
    parts.append(format_u16_array("CASEFOLD_SUPP_INDEX2", s2,
                                  "Supplementary level-2 index for case folding trie."))
    parts.append("")

    # Turkish exception table: sorted array of (source, target) pairs
    turkish_sorted = sorted(turkish_folds.items())
    parts.append(f"/// Turkish case folding exceptions: (source, target) pairs.")
    parts.append(f"/// {len(turkish_sorted)} entries, sorted by source code point.")
    parts.append(f"pub(crate) static TURKISH_FOLDS: &[(u32, u32)] = &[")
    for src, tgt in turkish_sorted:
        parts.append(f"    (0x{src:04X}, 0x{tgt:04X}),")
    parts.append("];")
    parts.append("")

    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")


# ---------------------------------------------------------------------------
# Step 13: Parse confusables.txt and generate confusable tables
# ---------------------------------------------------------------------------

# Encoding for confusable mapping entries:
# - Single-char: value = target code point (< 0x80000000)
# - Multi-char: value = CONFUSABLE_EXPANSION_FLAG | (length << 16) | offset
CONFUSABLE_EXPANSION_FLAG = 0x80000000


def parse_confusables():
    """Parse confusables.txt.

    Returns dict of source_cp -> [target_cps].
    """
    path = CACHE_DIR / "confusables.txt"
    mappings = {}
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "#" in line:
                line = line[:line.index("#")].strip()
            if not line:
                continue
            parts = line.split(";")
            if len(parts) < 3:
                continue
            source_str = parts[0].strip()
            target_str = parts[1].strip()
            # Parse source (single code point)
            source_cp = int(source_str, 16)
            # Parse target (one or more code points)
            target_cps = [int(x, 16) for x in target_str.split()]
            mappings[source_cp] = target_cps
    return mappings


def write_confusable_rs(mappings):
    """Generate src/tables/confusable.rs with confusable mapping tables."""
    path = OUTPUT_DIR / "confusable.rs"

    # Separate single-char and multi-char mappings
    single_mappings = []
    multi_mappings = []
    for source, targets in sorted(mappings.items()):
        if len(targets) == 1:
            single_mappings.append((source, targets[0]))
        else:
            multi_mappings.append((source, targets))

    # Build expansion table for multi-char mappings
    expansions = []
    expansion_entries = []  # (source, packed_value)
    for source, targets in multi_mappings:
        offset = len(expansions)
        length = len(targets)
        for t in targets:
            expansions.append(t)
        packed = CONFUSABLE_EXPANSION_FLAG | (length << 16) | offset
        expansion_entries.append((source, packed))

    # Combine all entries into a single sorted array
    all_entries = []
    for source, target in single_mappings:
        all_entries.append((source, target))
    for source, packed in expansion_entries:
        all_entries.append((source, packed))
    all_entries.sort(key=lambda x: x[0])

    parts = [HEADER.format(version=UNICODE_VERSION)]

    # Main mapping table: sorted array of (source, value) pairs
    parts.append(f"/// Confusable mapping table: (source_cp, mapping_value) pairs.")
    parts.append(f"/// {len(all_entries)} entries, sorted by source code point.")
    parts.append(f"/// If high bit of mapping_value is set, it is an expansion:")
    parts.append(f"///   bits 16-23 = length, bits 0-15 = offset into CONFUSABLE_EXPANSIONS.")
    parts.append(f"/// Otherwise, mapping_value is the target code point directly.")
    parts.append(f"pub(crate) static CONFUSABLE_MAPPINGS: &[(u32, u32)] = &[")
    for source, value in all_entries:
        parts.append(f"    (0x{source:06X}, 0x{value:08X}),")
    parts.append("];")
    parts.append("")

    # Expansion table
    parts.append(f"/// Expansion data for multi-char confusable mappings.")
    parts.append(f"/// {len(expansions)} code points total.")
    parts.append(f"pub(crate) static CONFUSABLE_EXPANSIONS: &[u32] = &[")
    for i in range(0, len(expansions), 8):
        chunk = expansions[i:i + 8]
        vals = ", ".join(f"0x{v:06X}" for v in chunk)
        parts.append(f"    {vals},")
    parts.append("];")
    parts.append("")

    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(parts))
    print(f"  Wrote {path} ({path.stat().st_size} bytes)")
    print(f"    Single-char mappings: {len(single_mappings)}")
    print(f"    Multi-char mappings: {len(multi_mappings)}")
    print(f"    Expansion table entries: {len(expansions)}")


# ---------------------------------------------------------------------------
# Step 14: Parse and generate conformance test data
# ---------------------------------------------------------------------------

def codepoints_to_rust_str(hex_cps):
    """Convert a space-separated string of hex code points to a Rust string literal."""
    cps = hex_cps.strip().split()
    chars = []
    for cp_hex in cps:
        cp = int(cp_hex, 16)
        chars.append(f"\\u{{{cp:04X}}}")
    return '"' + "".join(chars) + '"'


def parse_normalization_test():
    """Parse NormalizationTest.txt and return a list of (source, nfc, nfd, nfkc, nfkd) tuples."""
    path = CACHE_DIR / "NormalizationTest.txt"
    tests = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("@"):
                continue
            # Strip trailing comment
            if "#" in line:
                line = line[:line.index("#")].strip()
            parts = line.split(";")
            if len(parts) < 5:
                continue
            source = parts[0].strip()
            nfc = parts[1].strip()
            nfd = parts[2].strip()
            nfkc = parts[3].strip()
            nfkd = parts[4].strip()
            tests.append((source, nfc, nfd, nfkc, nfkd))
    return tests


def write_normalization_tests_rs(tests):
    """Generate tests/data/normalization_tests.rs from parsed test data."""
    path = TEST_DATA_DIR / "normalization_tests.rs"
    TEST_DATA_DIR.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write("// AUTO-GENERATED by scripts/generate_tables.py -- DO NOT EDIT\n")
        f.write(f"// Unicode version: {UNICODE_VERSION}\n")
        f.write(f"// Test cases: {len(tests)}\n\n")
        f.write("#[derive(Debug)]\n")
        f.write("pub struct NormalizationTest {\n")
        f.write("    pub source: &'static str,\n")
        f.write("    pub nfc: &'static str,\n")
        f.write("    pub nfd: &'static str,\n")
        f.write("    pub nfkc: &'static str,\n")
        f.write("    pub nfkd: &'static str,\n")
        f.write("}\n\n")
        f.write("pub const NORMALIZATION_TESTS: &[NormalizationTest] = &[\n")
        for source, nfc, nfd, nfkc, nfkd in tests:
            src_lit = codepoints_to_rust_str(source)
            nfc_lit = codepoints_to_rust_str(nfc)
            nfd_lit = codepoints_to_rust_str(nfd)
            nfkc_lit = codepoints_to_rust_str(nfkc)
            nfkd_lit = codepoints_to_rust_str(nfkd)
            f.write(f"    NormalizationTest {{ source: {src_lit}, nfc: {nfc_lit}, nfd: {nfd_lit}, nfkc: {nfkc_lit}, nfkd: {nfkd_lit} }},\n")
        f.write("];\n")
    print(f"  Wrote {path} ({path.stat().st_size} bytes, {len(tests)} test cases)")


# ---------------------------------------------------------------------------
# Main orchestrator
# ---------------------------------------------------------------------------

def main():
    print("=== simd-normalizer table generator ===")
    print(f"Unicode version: {UNICODE_VERSION}\n")

    print("Step 1: Downloading UCD files...")
    download_ucd_files()

    print("Step 2: Parsing UnicodeData.txt...")
    ccc_map, canon_decomp, compat_decomp, char_names = parse_unicode_data()
    print(f"  CCC entries: {len(ccc_map)}")
    print(f"  Canonical decompositions: {len(canon_decomp)}")
    print(f"  Compatibility decompositions: {len(compat_decomp)}")
    print()

    print("Step 3: Parsing CompositionExclusions.txt...")
    exclusions = parse_composition_exclusions()
    print(f"  Exclusions: {len(exclusions)}")

    print("Step 4: Parsing DerivedNormalizationProps.txt...")
    qc_props = parse_derived_normalization_props()
    for prop_name, entries in sorted(qc_props.items()):
        print(f"  {prop_name}: {len(entries)} entries")
    print()

    print("Step 5: Building full recursive decompositions...")
    full_canon, full_compat = build_full_decompositions(
        canon_decomp, compat_decomp, ccc_map
    )
    print(f"  Full canonical decompositions: {len(full_canon)}")
    print(f"  Full compatibility decompositions: {len(full_compat)}")
    print()

    print("Step 6: Building composition pairs...")
    composition_pairs = build_composition_pairs(canon_decomp, exclusions, ccc_map)
    print(f"  Composition pairs: {len(composition_pairs)}")
    print()

    print("Step 7: Building canonical decomposition trie...")
    canon_trie_builder, canon_expansions = build_decomp_trie(
        full_canon, ccc_map, qc_props, "NFC_QC"
    )
    canon_bmp_idx, canon_data, canon_s1, canon_s2 = canon_trie_builder.build()
    print(f"  Canonical trie: data={len(canon_data)}, expansions={len(canon_expansions)}")

    print("Step 8: Building compatibility decomposition trie...")
    compat_trie_builder, compat_expansions = build_decomp_trie(
        full_compat, ccc_map, qc_props, "NFKC_QC"
    )
    compat_bmp_idx, compat_data, compat_s1, compat_s2 = compat_trie_builder.build()
    print(f"  Compat trie: data={len(compat_data)}, expansions={len(compat_expansions)}")
    print()

    print("Step 9: Generating Rust source files...")
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    write_decomposition_rs(
        canon_bmp_idx, canon_data, canon_s1, canon_s2, canon_expansions,
        compat_bmp_idx, compat_data, compat_s1, compat_s2, compat_expansions,
    )
    write_composition_rs(composition_pairs)
    write_ccc_rs(ccc_map)
    write_qc_rs(qc_props)
    write_ccc_qc_rs(ccc_map, qc_props)

    print()
    print("Step 10: Generating conformance test data...")
    norm_tests = parse_normalization_test()
    write_normalization_tests_rs(norm_tests)

    print()
    print("Step 11: Parsing CaseFolding.txt...")
    simple_folds, turkish_folds = parse_case_folding()
    print(f"  Simple case folds (C+S): {len(simple_folds)}")
    print(f"  Turkish exceptions (T): {len(turkish_folds)}")

    print("Step 12: Generating case folding tables...")
    write_casefold_rs(simple_folds, turkish_folds)

    print()
    print("Step 13: Parsing confusables.txt...")
    confusable_mappings = parse_confusables()
    print(f"  Confusable mappings: {len(confusable_mappings)}")

    print("Step 14: Generating confusable tables...")
    write_confusable_rs(confusable_mappings)

    print()
    print("=== Table generation complete ===")


if __name__ == "__main__":
    main()
