# scripts/coverage-graph/ratchet_check.py
"""Fail (exit 1) if current reached_not_proven exceeds the committed baseline.
Run AFTER ingest_reaches.py has produced its report.

Usage:
  python ratchet_check.py --report graphify-out/REACHED_VS_PROVEN.md \
      --baseline contracts/reports/semantic-coverage.v1.json
"""
import argparse, json, re, sys
from pathlib import Path

def current_rnp(report: str) -> int:
    m = re.search(r"Total reached-but-unproven symbols:\s*(\d+)", Path(report).read_text(encoding="utf-8"))
    if not m:
        print("ratchet: could not parse report", file=sys.stderr); sys.exit(2)
    return int(m.group(1))

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", required=True)
    ap.add_argument("--baseline", required=True)
    args = ap.parse_args()
    cur = current_rnp(args.report)
    base = json.loads(Path(args.baseline).read_text(encoding="utf-8"))["totals"]["ratchet"]["reached_not_proven_baseline"]
    if cur > base:
        print(f"::error::reached-but-unproven ROSE {base} -> {cur} (+{cur-base}). Add behavioral assertions or justify.")
        return 1
    if cur < base:
        print(f"::notice::reached-but-unproven improved {base} -> {cur} (-{base-cur}). Lower the baseline in this PR.")
    print(f"ratchet OK: {cur} <= {base}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
