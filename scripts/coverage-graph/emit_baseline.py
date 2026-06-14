#!/usr/bin/env python
"""Emit the per-crate semantic-coverage baseline snapshot from the corrected
COVERAGE_MAP.md (post Wave-0 analyzer fixes).

This is the reference floor for the (deferred) `vox ci semantic-coverage` ratchet
and for tracking progress: it records, per crate, the production-symbol count and
how many are proven. Regenerate after each remediation wave to ratchet floors up.

Usage: python emit_baseline.py [--map graphify-out/COVERAGE_MAP.md] [--out contracts/reports/semantic-coverage.v1.json]
"""
import argparse, json, re
from pathlib import Path

ROW = re.compile(r"\| (vox[\w-]+|workspace-hack) \| (\d+) \| (\d+) \|")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--map", default="graphify-out/COVERAGE_MAP.md")
    ap.add_argument("--out", default="contracts/reports/semantic-coverage.v1.json")
    ap.add_argument("--commit", default="", help="optional source commit sha")
    args = ap.parse_args()

    crates = {}
    tot_defs = tot_proven = 0
    for line in Path(args.map).read_text(encoding="utf-8").splitlines():
        m = ROW.match(line)
        if not m:
            continue
        crate, defs, proven = m.group(1), int(m.group(2)), int(m.group(3))
        crates[crate] = {"defs": defs, "proven": proven}
        tot_defs += defs
        tot_proven += proven

    out = {
        "schema": "semantic-coverage.v1",
        "note": "Production-symbol proven-coverage floors (Wave-0 corrected denominator). "
                "Enforcing gate deferred — see semantic-coverage-remediation-plan §E Task 0.4.",
        "source_commit": args.commit or None,
        "totals": {
            "defs": tot_defs,
            "proven": tot_proven,
            "proven_pct": round(100 * tot_proven / max(1, tot_defs), 1),
        },
        "crates": dict(sorted(crates.items())),
    }
    dest = Path(args.out)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {dest}: {tot_proven}/{tot_defs} proven ({out['totals']['proven_pct']}%) "
          f"across {len(crates)} crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
