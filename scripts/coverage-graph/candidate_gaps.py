#!/usr/bin/env python
"""Emit the CLEANED per-crate candidate-gap worklist from the Phase-1 overlay graph.

Filters the raw graph node set down to genuine production-symbol DEFINITIONS that
carry NO `proves` edge, removing the false-positive classes the fidelity audit
found (see docs/src/architecture/semantic-coverage-remediation-plan-2026-06-13.md
§A): file nodes, std/external type references, `impl Type {` marker nodes, and
in-`src/` `#[cfg(test)]` test functions.

This is a CANDIDATE list (pre-verification): every entry must still pass the
§C per-symbol verification protocol before a test is written, because the static
overlay structurally under-credits methods and cross-crate/integration tests.

Usage:
  python candidate_gaps.py --graph graphify-out/graph.coverage.json --out graphify-out/CANDIDATE_GAPS.md
"""
import argparse, json, collections, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from overlay_tests import is_production_symbol, crate_from_source_file as crate


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--graph", default="graphify-out/graph.coverage.json")
    ap.add_argument("--out", default="graphify-out/CANDIDATE_GAPS.md")
    args = ap.parse_args()

    G = json.loads(Path(args.graph).read_text(encoding="utf-8"))
    proven = set()
    test_fn_by_file = collections.defaultdict(set)
    for n in G["nodes"]:
        if n.get("_origin") == "test":
            test_fn_by_file[(n.get("source_file") or "").replace("\\", "/")].add(n.get("label", ""))
    for l in G["links"]:
        if l.get("relation") == "proves":
            proven.add(l["target"])

    by_crate = collections.defaultdict(list)
    for n in G["nodes"]:
        # Shared production-symbol filter (see overlay_tests.is_production_symbol):
        # drops file nodes, type/std refs, non-/src/ defs, and in-src test fns.
        if not is_production_symbol(n, test_fn_by_file):
            continue
        if n["id"] in proven:
            continue
        sf = (n.get("source_file") or "").replace("\\", "/")
        by_crate[crate(sf)].append((n.get("label", ""), sf, n.get("source_location", "")))

    rows = sorted(by_crate.items(), key=lambda kv: -len(kv[1]))
    total = sum(len(v) for v in by_crate.values())
    lines = [
        "# Candidate Semantic-Coverage Gaps (cleaned, pre-verification)",
        "",
        f"**{total} candidate production symbols with no `proves` edge across {len(rows)} crates.**",
        "",
        "> CANDIDATES ONLY. The static overlay under-credits methods and "
        "cross-crate/integration tests, so ~2/3 of these are NOT genuine gaps "
        "(measured: ~47% already-tested elsewhere, ~20% trivial). Run the §C "
        "verification protocol on each symbol BEFORE writing a test. "
        "Regenerate: `python scripts/coverage-graph/candidate_gaps.py`.",
        "",
    ]
    for c, syms in rows:
        lines.append(f"## {c} ({len(syms)})")
        for lbl, sf, loc in sorted(syms, key=lambda s: (s[1], s[2])):
            lines.append(f"- [ ] `{lbl}` @ {sf}:{loc}")
        lines.append("")

    Path(args.out).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {args.out}: {total} candidates across {len(rows)} crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
