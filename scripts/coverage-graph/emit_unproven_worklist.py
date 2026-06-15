# scripts/coverage-graph/emit_unproven_worklist.py
"""Emit per-crate, leverage-ranked worklists of FRONTIER symbols: reached-but-unproven
AND non-trivial (skip getters/derives/Display/new/Default/builders). These are the
symbols worth a behavioral assertion — the input to Phase 3 waves.

Usage:
  python emit_unproven_worklist.py --graph contracts/reports/semantic-coverage-graph.snapshot.json.gz \
      --lcov target/llvm-cov-lcov.info --out-dir graphify-out/worklists
Requires the same lcov used for ingest (so `reached` matches).
"""
import argparse, gzip, json, re
from collections import defaultdict
from pathlib import Path
import importlib.util

# reuse ingest's lcov parser + norm
spec = importlib.util.spec_from_file_location("ingest", Path(__file__).with_name("ingest_reaches.py"))
ingest = importlib.util.module_from_spec(spec); spec.loader.exec_module(ingest)

TRIVIAL = re.compile(r"^(new|default|from|from_str|fmt|clone|eq|hash|builder|with_|get_|is_|as_|to_|into_|len|size)\b", re.I)

def crate_of(fp: str) -> str:
    p = (fp or "").replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"

def load_graph(path):
    raw = gzip.decompress(Path(path).read_bytes()) if path.endswith(".gz") else Path(path).read_bytes()
    return json.loads(raw)

def is_trivial(label: str) -> bool:
    base = ingest.norm(label or "")
    return bool(TRIVIAL.match(base)) or len(base) <= 2

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--graph", required=True)
    ap.add_argument("--lcov", required=True)
    ap.add_argument("--out-dir", default="graphify-out/worklists")
    args = ap.parse_args()
    by_fn, hit_lines = ingest.parse_lcov(args.lcov)
    g = load_graph(args.graph)
    proven = {l["target"] for l in g["links"] if l.get("relation") == "proves"}
    test_keys = {((n.get("source_file") or ""), ingest.norm(n.get("label", "")))
                 for n in g["nodes"] if n.get("_origin") == "test"}
    rows = defaultdict(list)
    for n in g["nodes"]:
        if n.get("file_type") != "code":
            continue
        sf = (n.get("source_file") or "").replace("\\", "/")
        nm = ingest.norm(n.get("label", ""))
        if "/tests/" in sf or (sf, nm) in test_keys:
            continue
        loc = (n.get("source_location") or "").lstrip("L")
        line_no = int(loc) if loc.isdigit() else None
        reached = (line_no in hit_lines.get(sf, set())) if line_no is not None else False
        reached = reached or by_fn.get(sf, {}).get(nm, False)
        if reached and n["id"] not in proven and not is_trivial(n.get("label", "")):
            rows[crate_of(sf)].append((n.get("label", ""), sf, n.get("source_location", "")))
    Path(args.out_dir).mkdir(parents=True, exist_ok=True)
    summary = sorted(((c, len(v)) for c, v in rows.items()), key=lambda kv: -kv[1])
    for c, items in rows.items():
        lines = ["label\tsource_file\tline"] + [f"{l}\t{f}\t{loc}" for (l, f, loc) in sorted(items)]
        (Path(args.out_dir) / f"{c}.tsv").write_text("\n".join(lines), encoding="utf-8")
    (Path(args.out_dir) / "_summary.tsv").write_text(
        "\n".join(f"{c}\t{n}" for c, n in summary), encoding="utf-8")
    print(f"frontier total = {sum(n for _, n in summary)} across {len(summary)} crates")
    for c, n in summary[:8]:
        print(f"  {c}\t{n}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
