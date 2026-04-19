#!/usr/bin/env python3
"""Render media/throughput.png from criterion benchmark output.

Walks target/criterion/<group>/<impl>/<input>/new/ producing a 2x2 grid of
grouped bar charts (one per NF form: NFC/NFD/NFKC/NFKD) showing MB/s for
simd_normalizer vs unicode_normalization vs icu4x across the input
categories exercised by benches/bench.rs.

Regenerate with:
    cargo bench
    python3 scripts/plot_throughput.py
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402


REPO_ROOT = Path(__file__).resolve().parent.parent
CRITERION_ROOT = REPO_ROOT / "target" / "criterion"
OUT_PATH = REPO_ROOT / "media" / "throughput.png"

FORMS = ["nfc", "nfd", "nfkc", "nfkd"]
IMPLS = ["simd_normalizer", "unicode_normalization", "icu4x"]
IMPL_LABELS = {
    "simd_normalizer": "simd-normalizer",
    "unicode_normalization": "unicode-normalization",
    "icu4x": "icu4x",
}
IMPL_COLORS = {
    "simd_normalizer": "#2E7D32",
    "unicode_normalization": "#1565C0",
    "icu4x": "#C62828",
}
INPUT_ORDER = [
    "ascii_only",
    "latin1",
    "cjk",
    "arabic",
    "hangul",
    "emoji",
    "mixed",
    "already_nfc",
    "worst_case",
]
INPUT_LABELS = {
    "ascii_only": "ASCII",
    "latin1": "Latin-1",
    "cjk": "CJK",
    "arabic": "Arabic",
    "hangul": "Hangul",
    "emoji": "Emoji",
    "mixed": "Mixed",
    "already_nfc": "Already NFC",
    "worst_case": "Worst case",
}


def read_mb_per_sec(form: str, impl: str, input_name: str) -> float | None:
    """Return MB/s for a single data point, or None if missing."""
    bench_dir = CRITERION_ROOT / form / impl / input_name / "new"
    bench_json = bench_dir / "benchmark.json"
    estimates_json = bench_dir / "estimates.json"
    if not bench_json.is_file() or not estimates_json.is_file():
        return None
    with bench_json.open() as fh:
        bench = json.load(fh)
    with estimates_json.open() as fh:
        estimates = json.load(fh)
    throughput = bench.get("throughput") or {}
    bytes_ = throughput.get("Bytes")
    if bytes_ is None:
        return None
    ns_per_iter = estimates["mean"]["point_estimate"]
    if ns_per_iter <= 0:
        return None
    return (bytes_ / (ns_per_iter * 1e-9)) / 1e6


def collect(form: str) -> dict[str, dict[str, float]]:
    """Return {input_name: {impl: mb_per_sec}} for a form."""
    data: dict[str, dict[str, float]] = {}
    for input_name in INPUT_ORDER:
        row: dict[str, float] = {}
        for impl in IMPLS:
            mb = read_mb_per_sec(form, impl, input_name)
            if mb is not None:
                row[impl] = mb
        if row:
            data[input_name] = row
    return data


def plot_form(ax, form: str, form_data: dict[str, dict[str, float]]) -> None:
    if not form_data:
        ax.set_title(f"{form.upper()} — no data")
        ax.axis("off")
        return
    inputs = [i for i in INPUT_ORDER if i in form_data]
    n_impls = len(IMPLS)
    width = 0.8 / n_impls
    x_positions = list(range(len(inputs)))
    for idx, impl in enumerate(IMPLS):
        values = [form_data[i].get(impl, 0.0) for i in inputs]
        offsets = [x + (idx - (n_impls - 1) / 2) * width for x in x_positions]
        ax.bar(
            offsets,
            values,
            width=width,
            label=IMPL_LABELS[impl],
            color=IMPL_COLORS[impl],
            edgecolor="black",
            linewidth=0.4,
        )
    ax.set_yscale("log")
    ax.set_title(form.upper())
    ax.set_xticks(x_positions)
    ax.set_xticklabels(
        [INPUT_LABELS.get(i, i) for i in inputs],
        rotation=30,
        ha="right",
        fontsize=8,
    )
    ax.set_ylabel("MB/s (log scale)")
    ax.grid(axis="y", which="both", linestyle=":", alpha=0.5)
    ax.set_axisbelow(True)


def main() -> int:
    if not CRITERION_ROOT.is_dir():
        print(
            f"error: {CRITERION_ROOT} not found. Run `cargo bench` first.",
            file=sys.stderr,
        )
        return 1

    per_form = {form: collect(form) for form in FORMS}
    total_points = sum(len(d) for d in per_form.values())
    if total_points == 0:
        print(
            "error: no benchmark estimates found under "
            f"{CRITERION_ROOT}. Run `cargo bench` first.",
            file=sys.stderr,
        )
        return 1

    fig, axes = plt.subplots(2, 2, figsize=(14, 9))
    for ax, form in zip(axes.flat, FORMS):
        plot_form(ax, form, per_form[form])

    handles, labels = axes[0, 0].get_legend_handles_labels()
    if handles:
        fig.legend(
            handles,
            labels,
            loc="upper center",
            ncol=len(IMPLS),
            bbox_to_anchor=(0.5, 0.97),
            frameon=False,
        )
    fig.suptitle(
        "simd-normalizer throughput vs unicode-normalization and icu4x",
        fontsize=13,
        y=0.995,
    )
    fig.tight_layout(rect=(0, 0, 1, 0.93))

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT_PATH, dpi=140, bbox_inches="tight")
    plt.close(fig)
    print(f"wrote {OUT_PATH.relative_to(REPO_ROOT)} ({total_points} data points)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
