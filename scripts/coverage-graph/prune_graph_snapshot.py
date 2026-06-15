# scripts/coverage-graph/prune_graph_snapshot.py
"""Strip graphify-out/graph.json to ONLY the fields ingest_reaches.py consumes, and
gzip it, so a small frozen snapshot can be committed for the CI ratchet. The full
graph (~109 MB, gitignored, LLM-derived) is not reproducible in CI; this snapshot is.

Usage:
  python prune_graph_snapshot.py --graph graphify-out/graph.json \
      --out contracts/reports/semantic-coverage-graph.snapshot.json.gz
"""
import argparse, gzip, json
from pathlib import Path

NODE_FIELDS = ("id", "label", "source_file", "source_location", "file_type", "_origin")

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--graph", default="graphify-out/graph.json")
    ap.add_argument("--out", default="contracts/reports/semantic-coverage-graph.snapshot.json.gz")
    args = ap.parse_args()
    g = json.loads(Path(args.graph).read_text(encoding="utf-8"))
    pruned = {
        "nodes": [{k: n.get(k) for k in NODE_FIELDS} for n in g["nodes"]],
        "links": [l for l in g["links"] if l.get("relation") == "proves"],
    }
    blob = json.dumps(pruned, separators=(",", ":")).encode("utf-8")
    Path(args.out).write_bytes(gzip.compress(blob, compresslevel=9))
    print(f"snapshot: {len(pruned['nodes'])} nodes, "
          f"{len(pruned['links'])} proves-links, "
          f"{Path(args.out).stat().st_size/1e6:.1f} MB gz")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
