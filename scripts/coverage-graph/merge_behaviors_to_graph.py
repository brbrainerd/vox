"""Inject Behavior nodes + edges into the coverage graph so the semantic layer is
queryable via `/graphify query` and the HTML viz.

Reads behavior claims from the Phase-2 workflow journals, adds:
  - a Behavior node per distinct claim (file_type=behavior),
  - Test --proves--> Behavior  (when the Test node exists from the Phase-1 overlay),
  - Behavior --about--> Symbol  (same-crate label resolution).

Input graph: graph.coverage.json (already has Test nodes + targets/proves edges).
Output:      graph.semantic.json

Usage: python merge_behaviors_to_graph.py --journals-list <file> --graph <in> --out <out>
"""
import argparse
import hashlib
import json
import re
from collections import defaultdict
from pathlib import Path

GEN = re.compile(r"<.*?>")


def crate_of(fp: str) -> str:
    p = (fp or "").replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"


def norm(label: str) -> str:
    return GEN.sub("", (label or "")).rstrip("()").strip().lstrip(".")


def load_claims(list_file: str):
    claims = []
    seen = set()
    for jp in Path(list_file).read_text(encoding="utf-8").splitlines():
        jp = jp.strip()
        if not jp:
            continue
        for line in Path(jp).read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                o = json.loads(line)
            except Exception:
                continue
            if not isinstance(o, dict) or o.get("type") != "result":
                continue
            for b in (o.get("result") or {}).get("behaviors", []) or []:
                cr = crate_of(b.get("file", ""))
                if cr == "?":
                    continue
                k = (cr, b.get("test", ""), b.get("about", ""), (b.get("claim", "") or "").strip().lower())
                if k in seen:
                    continue
                seen.add(k)
                b["_crate"] = cr
                claims.append(b)
    return claims


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--journals-list", required=True)
    ap.add_argument("--graph", default="graphify-out/graph.coverage.json")
    ap.add_argument("--out", default="graphify-out/graph.semantic.json")
    args = ap.parse_args()

    g = json.loads(Path(args.graph).read_text(encoding="utf-8"))
    nodes = g["nodes"]
    links = g["links"]
    node_ids = {n["id"] for n in nodes}

    # same-crate label -> code node id
    by_crate_label = defaultdict(dict)
    for n in nodes:
        if n.get("_origin") == "test" or n.get("file_type") == "behavior":
            continue
        cr = crate_of(n.get("source_file", ""))
        by_crate_label[cr].setdefault(norm(n.get("label", "")), n["id"])

    claims = load_claims(args.journals_list)
    added_nodes = added_about = added_proves = 0
    new_nodes, new_links = [], []
    for b in claims:
        cr = b["_crate"]
        bid = "behavior::" + hashlib.sha1(
            (cr + "|" + b.get("about", "") + "|" + b.get("claim", "")).encode("utf-8", "replace")
        ).hexdigest()[:14]
        if bid not in node_ids:
            node_ids.add(bid)
            new_nodes.append({
                "id": bid,
                "label": (b.get("claim", "") or "")[:90],
                "file_type": "behavior",
                "_origin": "behavior",
                "source_file": b.get("file", ""),
                "source_location": "",
                "kind": b.get("kind", ""),
                "confidence": b.get("confidence", "INFERRED"),
                "crate": cr,
                "norm_label": (b.get("about", "") or "")[:60],
            })
            added_nodes += 1
        # Behavior --about--> symbol
        sym = by_crate_label.get(cr, {}).get(norm(b.get("about", "")))
        if sym:
            new_links.append({
                "relation": "about", "confidence": b.get("confidence", "INFERRED"),
                "source_file": b.get("file", ""), "source_location": "", "weight": 1.0,
                "source": bid, "target": sym, "confidence_score": 1.0,
            })
            added_about += 1
        # Test --proves--> Behavior
        tid = f"test::{cr}::{b.get('file','')}::{b.get('test','')}"
        if tid in node_ids:
            new_links.append({
                "relation": "proves", "confidence": "EXTRACTED",
                "source_file": b.get("file", ""), "source_location": "", "weight": 1.0,
                "source": tid, "target": bid, "confidence_score": 1.0,
            })
            added_proves += 1

    g["nodes"] = nodes + new_nodes
    g["links"] = links + new_links
    Path(args.out).write_text(json.dumps(g), encoding="utf-8")
    print(f"claims={len(claims)} behavior_nodes+={added_nodes} about_edges+={added_about} proves(test->behavior)+={added_proves}")
    print(f"graph now {len(g['nodes'])} nodes, {len(g['links'])} links -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
