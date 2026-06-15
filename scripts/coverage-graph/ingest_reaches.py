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


def _load_graph(path: str):
    import gzip
    p = Path(path)
    raw = gzip.decompress(p.read_bytes()) if p.suffix == ".gz" else p.read_bytes()
    return json.loads(raw)


def parse_lcov(path: str):
    """Return ({file -> {fn_name -> reached}}, {file -> set(hit_lines)}).

    FN/FNDA give per-function reach (needs demangled names); DA gives per-LINE hit
    counts (no demangling needed). We use both: line-based matching by a symbol's
    definition line is robust to Rust name mangling. `reached` ORs across records, so
    concatenating chunked lcov exports yields the correct union.
    """
    by_fn = defaultdict(dict)
    hit_lines = defaultdict(set)
    cur = None
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("SF:"):
            cur = line[3:].strip().replace("\\", "/")
            if "crates/" in cur:
                cur = "crates/" + cur.split("crates/", 1)[1]
        elif not cur:
            continue
        elif line.startswith("DA:"):
            try:
                ln, cnt = line[3:].split(",")[:2]
                if int(cnt) > 0:
                    hit_lines[cur].add(int(ln))
            except ValueError:
                continue
        elif line.startswith("FNDA:"):
            try:
                count_s, name = line[5:].split(",", 1)
            except ValueError:
                continue
            short = name.strip().split("::")[-1]
            by_fn[cur][norm(short)] = by_fn[cur].get(norm(short), False) or (int(count_s) > 0)
    return by_fn, hit_lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lcov", required=True)
    ap.add_argument("--graph", default="graphify-out/graph.json")
    ap.add_argument("--out", default="graphify-out/graph.json")
    ap.add_argument("--report", default="graphify-out/REACHED_VS_PROVEN.md")
    args = ap.parse_args()

    by_fn, hit_lines = parse_lcov(args.lcov)
    g = _load_graph(args.graph)

    # symbols with an inbound `proves` edge = proven
    proven_ids = {l["target"] for l in g["links"] if l.get("relation") == "proves"}

    # Exclude TEST functions from the "code to be proven" universe: a code node is a
    # test if it matches a Test overlay node or lives under a tests/ dir. (Counting test
    # fns as reached-but-unproven is meaningless — we don't write tests for tests.)
    test_keys = {((n.get("source_file") or ""), norm(n.get("label", "")))
                 for n in g["nodes"] if n.get("_origin") == "test"}
    STD_TYPES = {"Result", "Error", "Option", "Self", "Vec", "String", "Box", "Arc",
                 "HashMap", "Ok", "Err", "Some", "None", "Default", "PathBuf", "Path"}

    def is_testfn(n):
        sf = n.get("source_file") or ""
        return "/tests/" in sf or (sf, norm(n.get("label", ""))) in test_keys

    per_crate = defaultdict(lambda: {"code": 0, "reached": 0, "proven": 0, "reached_not_proven": 0})
    rnp_examples = defaultdict(list)
    annotated = 0
    for n in g["nodes"]:
        if n.get("file_type") != "code":
            continue
        # skip test functions and bare std-type reference nodes from the count
        if is_testfn(n) or norm(n.get("label", "")) in STD_TYPES:
            continue
        sf = (n.get("source_file") or "").replace("\\", "/")
        nm = norm(n.get("label", ""))
        # line-based reach: symbol's definition line was executed
        line_no = None
        loc = (n.get("source_location") or "").lstrip("L")
        if loc.isdigit():
            line_no = int(loc)
        is_reached = None
        if sf in hit_lines or sf in by_fn:
            fn_hit = by_fn.get(sf, {}).get(nm)
            line_hit = (line_no in hit_lines.get(sf, set())) if line_no is not None else None
            if fn_hit is not None or line_hit is not None:
                is_reached = bool(fn_hit) or bool(line_hit)
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
