#!/usr/bin/env python
"""Rebuild the base AST code graph over crates/ (deterministic, cached).

Recreated to match the pipeline documented in scripts/coverage-graph/README.md
step 1: `python rebuild_full_graph.py . graphify-out/graph.full.json`.

Part of the deferred-Python coverage-graph toolchain (see README language-policy
note); do not extend — add new functionality in Vox.

Usage:
    python rebuild_full_graph.py <repo_root> <out_graph_json>
"""
import sys
from pathlib import Path

from graphify.extract import collect_files, extract
from graphify.build import build_from_json
from graphify.cluster import cluster
from graphify.export import to_json


def main() -> int:
    repo_root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    out_path = sys.argv[2] if len(sys.argv) > 2 else "graphify-out/graph.full.json"

    crates_dir = repo_root / "crates"
    files = collect_files(crates_dir, root=repo_root)
    rust = [f for f in files if f.suffix == ".rs"]
    print(f"collected {len(rust)} rust files under crates/", flush=True)

    extraction = extract(rust, cache_root=repo_root)
    print(
        f"extracted {len(extraction['nodes'])} nodes, "
        f"{len(extraction['edges'])} edges",
        flush=True,
    )

    G = build_from_json(extraction, root=repo_root)
    communities = cluster(G)
    print(
        f"graph {G.number_of_nodes()} nodes, {G.number_of_edges()} edges, "
        f"{len(communities)} communities",
        flush=True,
    )

    to_json(G, communities, out_path, force=True)
    print(f"wrote {out_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
