"""Recover extracted Behavior claims from workflow journals and synthesize per-crate
behavior maps DETERMINISTICALLY (no LLM).

The LLM synthesis stage in the Phase-2 workflow fails on large crates (huge claim JSON
+ huge generated report) and, because raw claims were never persisted, the extraction
was lost. This tool reads the workflow `journal.jsonl` `result` records (which DO hold
the extract agents' `behaviors` arrays), groups them by crate, and writes the map
without any further model calls. Synthesis here is purely mechanical: group by symbol,
flag symbols proven only on the happy path.

Usage:
  python recover_and_synth.py --journal <journal.jsonl> [--journal <more>] --out-dir graphify-out [--skip-existing]
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path


def crate_of(file_path: str) -> str:
    p = (file_path or "").replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"


def load_claims(journals):
    by_crate = defaultdict(list)
    for jp in journals:
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
            res = o.get("result") or {}
            for b in res.get("behaviors", []) or []:
                cr = crate_of(b.get("file", ""))
                if cr != "?":
                    by_crate[cr].append(b)
    return by_crate


def synth_markdown(crate: str, claims):
    # dedup identical (about, claim)
    seen = set()
    deduped = []
    for b in claims:
        k = (b.get("about", ""), b.get("claim", "").strip().lower())
        if k in seen:
            continue
        seen.add(k)
        deduped.append(b)
    by_sym = defaultdict(list)
    for b in deduped:
        by_sym[b.get("about", "?")].append(b)

    def has(kinds, items):
        return any(b.get("kind") in kinds for b in items)

    gaps = []
    proven_error = 0
    for sym, items in by_sym.items():
        err = has({"error"}, items)
        edge = has({"edge", "invariant"}, items)
        if err:
            proven_error += 1
        if not err and not edge:
            gaps.append(sym)

    lines = [f"# Semantic Behavior Map — `{crate}`\n",
             f"Deterministically synthesized from {len(deduped)} distinct proven-behavior claims "
             f"(of {len(claims)} extracted) across {len(by_sym)} symbols. "
             f"{proven_error} symbols have an explicit error-path proof; "
             f"**{len(gaps)} are proven only on the happy path** (no error/edge/invariant claim) — "
             f"the semantic holes line coverage hides.\n"]
    lines.append("## Per-symbol proven behaviors\n")
    for sym in sorted(by_sym, key=lambda s: (-len(by_sym[s]), s)):
        items = by_sym[sym]
        kinds = sorted({b.get("kind", "?") for b in items})
        conf = sorted({b.get("confidence", "?") for b in items})
        lines.append(f"\n### `{sym}`  ({', '.join(kinds)}; {', '.join(conf)})")
        for b in items[:12]:
            f = b.get("file", "")
            lines.append(f"- [{b.get('kind','?')}] {b.get('claim','').strip()}  ({f})")
        if len(items) > 12:
            lines.append(f"- … +{len(items) - 12} more claims")

    lines.append("\n## Semantic gaps (proven happy-path only)\n")
    if gaps:
        lines.append("These symbols have proven behavior but **no error, edge, or invariant proof** — "
                     "failure/empty/boundary modes are unverified:\n")
        for sym in sorted(gaps):
            ex = by_sym[sym][0].get("claim", "").strip()
            lines.append(f"- **`{sym}`** — only: _{ex}_")
    else:
        lines.append("_None — every proven symbol has at least one error/edge/invariant claim._")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--journal", action="append", required=True)
    ap.add_argument("--out-dir", default="graphify-out")
    ap.add_argument("--skip-existing", action="store_true")
    ap.add_argument("--only", default="", help="comma-separated crate allowlist")
    args = ap.parse_args()

    only = {c for c in args.only.split(",") if c}
    by_crate = load_claims(args.journal)
    out = Path(args.out_dir)
    wrote, skipped = [], []
    for crate, claims in sorted(by_crate.items()):
        if only and crate not in only:
            continue
        dest = out / f"COVERAGE_BEHAVIORS_{crate}.md"
        if args.skip_existing and dest.exists():
            skipped.append(crate)
            continue
        dest.write_text(synth_markdown(crate, claims), encoding="utf-8", newline="\n")
        wrote.append((crate, len(claims)))
    for c, n in wrote:
        print(f"  wrote {c}: {n} claims")
    print(f"recovered {len(by_crate)} crates; wrote {len(wrote)}; skipped {len(skipped)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
