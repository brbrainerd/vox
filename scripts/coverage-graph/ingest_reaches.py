"""Phase 0 — ingest llvm-cov line/function coverage into the semantic graph as the
`reached` layer, and compute the keystone set: symbols REACHED-BUT-NOT-PROVEN
(executed during tests but with no asserted behavior — the gap line coverage hides).

Input: an LCOV file (`target/llvm-cov-lcov.info`) — already published by CI as the
`llvm-cov` artifact (.github/workflows/ci.yml). LCOV `FNDA:<count>,<name>` records give
per-function execution counts; count>0 == reached. No new CI step required: download the
`llvm-cov` artifact, then run this.

Usage:
  python ingest_reaches.py --lcov target/llvm-cov-lcov.info --graph graphify-out/graph.json \
      --out graphify-out/graph.json --report graphify-out/REACHED_VS_PROVEN.md
"""
import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

GEN = re.compile(r"<.*?>")


def norm(label: str) -> str:
    return GEN.sub("", (label or "")).rstrip("()").strip().lstrip(".")


def crate_of(fp: str) -> str:
    p = (fp or "").replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"


def parse_lcov(path: str):
    """Return {source_file -> {fn_name -> reached_bool}} from FN/FNDA records."""
    reached = defaultdict(dict)
    cur = None
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("SF:"):
            cur = line[3:].strip().replace("\\", "/")
            # normalize to repo-relative crates/... path
            if "crates/" in cur:
                cur = "crates/" + cur.split("crates/", 1)[1]
        elif line.startswith("FNDA:") and cur:
            try:
                count_s, name = line[5:].split(",", 1)
            except ValueError:
                continue
            # demangle-ish: keep last path segment of a rust fn name
            short = name.strip().split("::")[-1]
            reached[cur][norm(short)] = reached[cur].get(norm(short), False) or (int(count_s) > 0)
        elif line.startswith("FN:") and cur:
            # FN:<line>,<name> — ensure key exists even if no FNDA (treated as not reached)
            try:
                _ln, name = line[3:].split(",", 1)
            except ValueError:
                continue
            short = name.strip().split("::")[-1]
            reached[cur].setdefault(norm(short), False)
    return reached


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lcov", required=True)
    ap.add_argument("--graph", default="graphify-out/graph.json")
    ap.add_argument("--out", default="graphify-out/graph.json")
    ap.add_argument("--report", default="graphify-out/REACHED_VS_PROVEN.md")
    args = ap.parse_args()

    reached = parse_lcov(args.lcov)
    g = json.loads(Path(args.graph).read_text(encoding="utf-8"))

    # symbols with an inbound `proves` edge = proven
    proven_ids = {l["target"] for l in g["links"] if l.get("relation") == "proves"}

    per_crate = defaultdict(lambda: {"code": 0, "reached": 0, "proven": 0, "reached_not_proven": 0})
    rnp_examples = defaultdict(list)
    annotated = 0
    for n in g["nodes"]:
        if n.get("file_type") != "code":
            continue
        sf = (n.get("source_file") or "").replace("\\", "/")
        nm = norm(n.get("label", ""))
        is_reached = reached.get(sf, {}).get(nm, None)
        if is_reached is not None:
            n["reached"] = bool(is_reached)
            annotated += 1
        cr = crate_of(sf)
        st = per_crate[cr]
        st["code"] += 1
        r = bool(is_reached)
        p = n["id"] in proven_ids
        if r:
            st["reached"] += 1
        if p:
            st["proven"] += 1
        if r and not p:
            st["reached_not_proven"] += 1
            if len(rnp_examples[cr]) < 8:
                rnp_examples[cr].append((n.get("label", ""), sf, n.get("source_location", "")))

    Path(args.out).write_text(json.dumps(g), encoding="utf-8")

    rows = sorted(per_crate.items(), key=lambda kv: -kv[1]["reached_not_proven"])
    tot_rnp = sum(v["reached_not_proven"] for v in per_crate.values())
    out = ["# Reached-but-NOT-Proven — Phase 0 (llvm-cov × proven map)\n",
           f"Annotated {annotated} code symbols with llvm-cov `reached` status.\n",
           "**reached-not-proven** = a symbol whose code EXECUTED during tests but has NO asserted "
           "behavior (`proves` edge). This is the precise set line coverage counts as 'covered' but "
           "that proves nothing — the keystone signal of this whole initiative.\n",
           f"\n**Total reached-but-unproven symbols: {tot_rnp}**\n",
           "\n| Crate | Code | Reached | Proven | Reached-not-proven |",
           "|---|---|---|---|---|"]
    for cr, v in rows[:60]:
        out.append(f"| {cr} | {v['code']} | {v['reached']} | {v['proven']} | **{v['reached_not_proven']}** |")
    out.append("\n## Top reached-but-unproven symbols (per worst crate)\n")
    for cr, _v in rows[:15]:
        if not rnp_examples[cr]:
            continue
        out.append(f"\n### {cr}")
        for lab, sf, loc in rnp_examples[cr]:
            out.append(f"- `{lab}` — {sf}:{loc}")
    Path(args.report).write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"annotated={annotated} reached_not_proven={tot_rnp} -> {args.report}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
