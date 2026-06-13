"""Deterministic duplicate-code / split-brain / wiring analysis over the Vox crates.

Three independent passes, no LLM:

1. DUPLICATE CODE — hash normalized function bodies across all crates.
   - exact:      identical body after stripping comments + collapsing whitespace.
   - structural: identical after also replacing every identifier with `X`
                 (catches copy-paste with renamed variables/types).

2. SPLIT-BRAIN — same symbol name DEFINED in multiple crates (from the graph),
   ranked by crate spread. Candidate divergent implementations of one concept.

3. WIRING (advisory only) — graph nodes with zero inbound edges. NOTE: the AST
   extractor does not resolve every call, so this is a NOISY candidate list, not
   proof of dead code. Cross-check with vox-arch-check before acting.

Usage: python dup_and_wiring.py --repo-root <repo> --graph <graph.full.json> --out <report.md>
"""
import argparse
import hashlib
import re
from collections import defaultdict
from pathlib import Path

IDENT = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
LINE_COMMENT = re.compile(r"//[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
FN = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*[(<]")
WS = re.compile(r"\s+")
# tokens that survive structural normalization (keywords/punctuation carry shape)
KEYWORDS = {
    "let", "mut", "fn", "if", "else", "match", "for", "while", "loop", "return",
    "self", "Self", "pub", "ref", "move", "as", "in", "where", "impl", "async",
    "await", "use", "struct", "enum", "trait", "const", "static", "true", "false",
}


def crate_of(path: str) -> str:
    p = path.replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"


def find_fn_bodies(text: str):
    """Yield (fn_name, start_line, body_text) for each top-ish fn with a brace body."""
    for m in FN.finditer(text):
        name = m.group(1)
        brace = text.find("{", m.end() - 1)
        if brace == -1:
            continue
        depth = 0
        i = brace
        while i < len(text):
            c = text[i]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        if depth != 0:
            continue
        body = text[brace + 1 : i]
        start_line = text.count("\n", 0, m.start()) + 1
        yield name, start_line, body


def strip(body: str) -> str:
    body = BLOCK_COMMENT.sub(" ", body)
    body = LINE_COMMENT.sub(" ", body)
    return WS.sub(" ", body).strip()


def structural(norm: str) -> str:
    return IDENT.sub(lambda mm: mm.group(0) if mm.group(0) in KEYWORDS else "X", norm)


def h(s: str) -> str:
    return hashlib.sha1(s.encode("utf-8", "replace")).hexdigest()[:12]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo-root", default=".")
    ap.add_argument("--graph", default="graphify-out/graph.full.json")
    ap.add_argument("--out", default="graphify-out/DUPLICATION_AND_WIRING.md")
    ap.add_argument("--min-exact", type=int, default=120, help="min normalized body chars for exact dup")
    ap.add_argument("--min-struct", type=int, default=240, help="min chars for structural near-dup")
    args = ap.parse_args()

    repo = Path(args.repo_root)
    exact = defaultdict(list)
    struct = defaultdict(list)
    skipped = 0
    nfiles = 0
    for rs in (repo / "crates").rglob("*.rs"):
        if "/target/" in str(rs).replace("\\", "/") or "\\target\\" in str(rs):
            continue
        try:
            text = rs.read_text(encoding="utf-8", errors="replace")
        except Exception:
            skipped += 1
            continue
        nfiles += 1
        rel = str(rs.relative_to(repo)).replace("\\", "/")
        for name, line, body in find_fn_bodies(text):
            norm = strip(body)
            if len(norm) >= args.min_exact:
                exact[h(norm)].append((name, rel, line, len(norm)))
            if len(norm) >= args.min_struct:
                struct[h(structural(norm))].append((name, rel, line))

    exact_groups = [(k, v) for k, v in exact.items() if len({(n, f, l) for n, f, l, _ in v}) >= 2]
    # structural groups that are NOT already exact-identical, >=3 sites
    exact_keys_for_sites = {}
    for k, v in exact.items():
        for n, f, l, _ in v:
            exact_keys_for_sites[(f, l)] = k
    struct_groups = []
    for k, v in struct.items():
        sites = {(n, f, l) for n, f, l in v}
        if len(sites) >= 3:
            ekeys = {exact_keys_for_sites.get((f, l)) for _, f, l in sites}
            if len(ekeys) > 1:  # not a single exact-dup cluster
                struct_groups.append((k, sorted(sites)))

    exact_groups.sort(key=lambda kv: -max(c for *_, c in kv[1]))
    struct_groups.sort(key=lambda kv: -len(kv[1]))

    # ---- split-brain from graph ----
    import json

    g = json.loads(Path(args.graph).read_text(encoding="utf-8"))
    name_crates = defaultdict(set)
    name_sites = defaultdict(list)
    indeg = defaultdict(int)
    nodes = {n["id"]: n for n in g["nodes"]}
    for l in g["links"]:
        indeg[l["target"]] += 1
    for n in g["nodes"]:
        lab = (n.get("label") or "").rstrip("()")
        if not lab or lab.startswith("."):
            continue
        cr = crate_of(n.get("source_file", ""))
        name_crates[lab].add(cr)
        name_sites[lab].append((cr, n.get("source_file", ""), n.get("source_location", "")))
    splitbrain = [(name, sorted(cs)) for name, cs in name_crates.items() if len(cs) >= 4]
    splitbrain.sort(key=lambda x: -len(x[1]))

    orphans = [
        nodes[i] for i in nodes
        if indeg.get(i, 0) == 0 and nodes[i].get("_origin") != "test"
    ]

    # ---- report ----
    out = []
    out.append("# Duplicate Code / Split-Brain / Wiring — deterministic scan\n")
    out.append(f"Scanned {nfiles} Rust files ({skipped} unreadable).\n")
    out.append(
        f"- **Exact body duplicates:** {len(exact_groups)} clusters "
        f"(identical normalized body, >= {args.min_exact} chars)\n"
        f"- **Structural near-duplicates:** {len(struct_groups)} clusters "
        f"(same shape, renamed identifiers, >=3 sites, >= {args.min_struct} chars)\n"
        f"- **Split-brain name candidates:** {len(splitbrain)} names defined in >=4 crates\n"
        f"- **Zero-inbound-edge nodes (ADVISORY/noisy):** {len(orphans)} — cross-check with vox-arch-check\n"
    )

    out.append("\n## Exact body duplicates (top 40)\n")
    for k, v in exact_groups[:40]:
        sites = sorted({(n, f, l) for n, f, l, _ in v})
        size = max(c for *_, c in v)
        out.append(f"\n**{len(sites)}× `{sites[0][0]}` (~{size} chars)**")
        for n, f, l in sites[:8]:
            out.append(f"  - `{n}` — {f}:{l}")

    out.append("\n## Structural near-duplicates (top 30)\n")
    for k, sites in struct_groups[:30]:
        names = sorted({n for n, _, _ in sites})
        out.append(f"\n**{len(sites)} sites, names: {', '.join(list(names)[:6])}**")
        for n, f, l in sites[:6]:
            out.append(f"  - `{n}` — {f}:{l}")

    out.append("\n## Split-brain candidates (names defined in >=4 crates, top 40)\n")
    out.append("_Same symbol name across many crates — candidate divergent implementations of one concept. Verify by reading bodies._\n")
    for name, crates in splitbrain[:40]:
        out.append(f"- **`{name}`** — {len(crates)} crates: {', '.join(crates[:10])}")

    Path(args.out).write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"exact={len(exact_groups)} struct={len(struct_groups)} splitbrain={len(splitbrain)} orphans={len(orphans)}")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
