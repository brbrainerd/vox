#!/usr/bin/env python
"""Generate COVERAGE_BEHAVIORS_INDEX.md from Phase-2 extraction journals.

Mirrors recover_and_synth.py's gap logic: per crate, count distinct symbols,
symbols with an explicit error-path proof, and "happy-only" gaps (symbols with
no error/edge/invariant claim). Deterministic; no LLM.

Usage: python build_index.py --journal <j1> [--journal <j2> ...] --out-dir graphify-out
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path


def crate_of(file_path: str) -> str:
    p = (file_path or "").replace("\\", "/")
    if "crates/" not in p:
        return "?"
    parts = p.split("crates/", 1)[1].split("/")
    return parts[0] if parts and parts[0] else "?"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--journal", action="append", required=True)
    ap.add_argument("--out-dir", default="graphify-out")
    args = ap.parse_args()

    by_crate = defaultdict(list)
    for jp in args.journal:
        for line in Path(jp).read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                o = json.loads(line)
            except Exception:
                continue
            if o.get("type") != "result":
                continue
            for b in (o.get("result") or {}).get("behaviors", []) or []:
                cr = crate_of(b.get("file", ""))
                if cr != "?":
                    by_crate[cr].append(b)

    rows = []
    total_gaps = 0
    for crate, claims in by_crate.items():
        by_sym = defaultdict(list)
        for b in claims:
            by_sym[b.get("about", "?")].append(b)
        err = sum(1 for items in by_sym.values() if any(b.get("kind") == "error" for b in items))
        gaps = sum(
            1
            for items in by_sym.values()
            if not any(b.get("kind") in {"error", "edge", "invariant"} for b in items)
        )
        total_gaps += gaps
        rows.append((crate, len(by_sym), err, gaps))

    rows.sort(key=lambda r: (-r[3], -r[1], r[0]))
    lines = [
        "# Semantic Test-Coverage — Master Index\n",
        f"{len(rows)} crates mapped (fresh extraction). Each links to its per-crate behavior map. "
        '"Happy-only gaps" = symbols with proven behavior but NO error/edge/invariant proof '
        "(the holes line coverage hides).\n",
        "| Crate | Symbols | Error-proven | Happy-only gaps |",
        "|---|---|---|---|",
    ]
    for crate, syms, err, gaps in rows:
        lines.append(f"| [{crate}](COVERAGE_BEHAVIORS_{crate}.md) | {syms} | {err} | {gaps} |")
    lines.append(f"\n**Total happy-only gaps across all crates: {total_gaps}**")

    out = Path(args.out_dir) / "COVERAGE_BEHAVIORS_INDEX.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out}: {len(rows)} crates, {total_gaps} total happy-only gaps")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
