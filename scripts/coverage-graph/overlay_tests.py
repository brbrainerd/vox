#!/usr/bin/env python3
"""
overlay_tests.py — Phase 1: Deterministic static test-coverage overlay for the Vox code graph.

Usage:
    python overlay_tests.py --graph <in.json> --repo-root <repo> --out <out.json> [--report <report.md>]
"""
import argparse
import json
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Regex patterns
# ---------------------------------------------------------------------------
TEST_ATTR_RE = re.compile(
    r"#\s*\[\s*(test|tokio\s*::\s*test|rstest)[^\]]*\]"
)
CFG_TEST_MOD_RE = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]"
)
FN_DEF_RE = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")

# Assertion macros we care about
ASSERT_MACRO_RE = re.compile(
    r"\b(assert!|assert_eq!|assert_ne!|assert_matches!)\s*\("
)
# .expect( and .unwrap() as assertion proxies
EXPECT_UNWRAP_RE = re.compile(r"\.(expect|unwrap)\s*[\(\!]")

# Identifier token (not a keyword)
IDENT_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\b")

# ---------------------------------------------------------------------------
# Precision constants
# ---------------------------------------------------------------------------

# Stoplist: normalized symbol names that are Rust std/prelude and must NEVER be
# edge targets.  Extend freely — never shrink.
STD_PRELUDE_STOPLIST: frozenset[str] = frozenset([
    "Result", "Option", "Vec", "String", "str", "Path", "PathBuf",
    "Box", "Arc", "Rc", "Cell", "RefCell", "Mutex", "RwLock", "Cow",
    "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque",
    "Error", "Self", "Some", "None", "Ok", "Err", "Default", "Value",
    "Into", "From", "TryFrom", "TryInto", "Iterator",
    "Clone", "Copy", "Debug", "Display", "Ord", "Eq", "Hash",
    "Send", "Sync", "Sized", "Drop",
    "Fn", "FnMut", "FnOnce",
    "ToString", "AsRef", "Deref",
])

# If a normalized symbol name appears in strictly MORE than this many distinct
# crates, it is too generic to be a useful coverage signal.
MAX_NAME_CRATE_SPREAD: int = 3

RUST_KEYWORDS = frozenset([
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
    "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "typeof", "unsized", "virtual",
    "yield", "union",
    # common primitives / stdlib names too common to be useful as symbol refs
    "String", "Vec", "Option", "Result", "Ok", "Err", "Some", "None",
    "bool", "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
    "str", "char", "println", "eprintln", "format", "write", "writeln",
    "panic", "todo", "unimplemented", "unreachable",
    "Box", "Rc", "Arc", "Cell", "RefCell", "HashMap", "HashSet",
    "BTreeMap", "BTreeSet", "Default", "Clone", "Copy", "Debug",
    "Display", "From", "Into", "Iterator", "Send", "Sync",
    "new", "fmt", "len", "is_empty", "push", "pop", "get", "set",
    "iter", "map", "filter", "collect", "unwrap", "expect", "ok",
    "err", "and_then", "or_else", "into", "from", "clone", "drop",
])


# ---------------------------------------------------------------------------
# Graph index helpers
# ---------------------------------------------------------------------------

def _strip_label(label: str) -> str:
    """Normalize a label: remove trailing (), leading dot, generic args."""
    s = label.strip()
    # remove generic args like <T, U>
    s = re.sub(r"<[^>]*>", "", s)
    # remove trailing ()
    s = s.rstrip("()")
    # remove leading dot
    s = s.lstrip(".")
    return s.strip()


def build_name_index(
    nodes: list[dict],
) -> tuple[dict[str, list[dict]], dict[str, list[dict]]]:
    """Build two name indices from graph nodes, applying precision filters.

    Returns (pruned_idx, full_same_crate_idx):
      - pruned_idx: names with spread <= MAX_NAME_CRATE_SPREAD (used for cross-crate
        globally-unique fallback).
      - full_same_crate_idx: ALL names (spread-unlimited) subject only to the
        stoplist and method-only-label filters (used for same-crate lookup).

    Filters applied to BOTH indices:
      1. Skip test-origin nodes.
      2. Skip method-only labels (raw label starts with '.').
      3. Skip std/prelude stoplist names and Rust keywords.

    Filter applied ONLY to pruned_idx:
      4. Skip names that appear in more than MAX_NAME_CRATE_SPREAD distinct crates.
    """
    # First pass: collect candidate mappings and per-name crate sets
    raw_idx: dict[str, list[dict]] = defaultdict(list)
    name_crates: dict[str, set[str]] = defaultdict(set)

    for node in nodes:
        if node.get("_origin") == "test":
            continue

        # Filter 1: skip method-only labels (raw label starts with '.')
        raw_label = node.get("label", "")
        if raw_label.lstrip().startswith("."):
            continue

        node_crate = crate_from_source_file(node.get("source_file", ""))

        for raw in (raw_label, node.get("norm_label", "")):
            key = _strip_label(raw)
            # Filter 2: skip std/prelude stoplist and Rust keywords.
            # Check both exact case and title-case so that norm_label lowercasing
            # (e.g. "path" from label "Path") doesn't slip past the stoplist.
            if not key or key in RUST_KEYWORDS or key in STD_PRELUDE_STOPLIST:
                continue
            if key.capitalize() in STD_PRELUDE_STOPLIST or key.title() in STD_PRELUDE_STOPLIST:
                continue
            raw_idx[key].append(node)
            name_crates[key].add(node_crate)

    def _dedup(node_list: list[dict]) -> list[dict]:
        seen_ids: set[str] = set()
        deduped = []
        for n in node_list:
            if n["id"] not in seen_ids:
                seen_ids.add(n["id"])
                deduped.append(n)
        return deduped

    # pruned_idx: drop names spread across too many crates (cross-crate fallback)
    pruned_idx: dict[str, list[dict]] = {}
    for key, node_list in raw_idx.items():
        if len(name_crates[key]) > MAX_NAME_CRATE_SPREAD:
            continue
        pruned_idx[key] = _dedup(node_list)

    # full_same_crate_idx: keep ALL names (no spread cap); used only for same-crate lookup
    full_same_crate_idx: dict[str, list[dict]] = {
        key: _dedup(node_list) for key, node_list in raw_idx.items()
    }

    return pruned_idx, full_same_crate_idx


def crate_from_source_file(source_file: str) -> str:
    """Derive crate name from a source_file path like crates/vox-foo/src/bar.rs"""
    parts = source_file.replace("\\", "/").split("/")
    try:
        ci = parts.index("crates")
        return parts[ci + 1]
    except (ValueError, IndexError):
        return "unknown"


# ---------------------------------------------------------------------------
# Brace-matching body extraction
# ---------------------------------------------------------------------------

def extract_body(text: str, fn_start: int) -> tuple[int, int]:
    """
    Given text and the position of 'fn NAME(...', find the opening brace and
    extract the full body by brace matching.
    Returns (body_start, body_end) indices into text (inclusive of braces).
    Returns (-1, -1) if not found.
    """
    # find opening brace after fn_start
    i = fn_start
    while i < len(text) and text[i] != "{":
        i += 1
    if i >= len(text):
        return -1, -1
    depth = 0
    body_start = i
    while i < len(text):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return body_start, i
        i += 1
    return -1, -1


# ---------------------------------------------------------------------------
# Assertion context detection
# ---------------------------------------------------------------------------

def find_assertion_spans(body: str) -> list[tuple[int, int]]:
    """
    Return list of (start, end) character spans that are within assertion
    macro arguments or .expect(/.unwrap() call sites.
    """
    spans = []

    # assert!, assert_eq!, etc. — capture from macro open-paren to matching close-paren
    for m in ASSERT_MACRO_RE.finditer(body):
        s, e = _find_paren_end(body, m.end() - 1)  # m.end()-1 is the '('
        if s >= 0:
            spans.append((s, e))

    # .expect( ... ) — include a window before the dot to capture the receiver
    for m in re.finditer(r"\.(expect)\s*\(", body):
        s, e = _find_paren_end(body, m.end() - 1)
        if s >= 0:
            # extend span leftward to include the receiver identifier
            window_start = max(0, m.start() - 80)
            spans.append((window_start, e))

    # .unwrap() — the call itself implies assertion, so include the surrounding token
    for m in re.finditer(r"\.(unwrap)\s*\(\s*\)", body):
        # the identifier being unwrapped is just before the dot
        # grab a window before the dot as assertion context
        dot_pos = m.start()
        window_start = max(0, dot_pos - 80)
        spans.append((window_start, m.end()))

    return spans


def _find_paren_end(text: str, open_pos: int) -> tuple[int, int]:
    """Find matching close paren starting from open_pos (which must be '(')."""
    if open_pos >= len(text) or text[open_pos] != "(":
        return -1, -1
    depth = 0
    i = open_pos
    while i < len(text):
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return open_pos, i
        i += 1
    return -1, -1


def in_assertion_context(pos: int, assertion_spans: list[tuple[int, int]]) -> bool:
    return any(s <= pos <= e for s, e in assertion_spans)


# ---------------------------------------------------------------------------
# Test detection
# ---------------------------------------------------------------------------

def detect_tests(file_path: Path, repo_root: Path) -> list[dict]:
    """
    Parse a Rust source file and return a list of test-function descriptors.
    Each has: fn_name, start_line, body, is_integration, crate, rel_path.
    """
    try:
        text = file_path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []

    rel = file_path.relative_to(repo_root).as_posix()
    crate = crate_from_source_file(rel)

    # Determine if this is an integration test file (under a crate's tests/ dir).
    # Pattern: either crates/<crate>/tests/ or any path where 'tests' appears
    # at the same level as 'src' (i.e. not inside src/).
    rel_parts = rel.split("/")
    is_integration = False
    if "tests" in rel_parts:
        tests_idx = rel_parts.index("tests")
        # If crates/ is present: integration = tests/ is a direct child of the crate dir
        if "crates" in rel_parts:
            crates_idx = rel_parts.index("crates")
            is_integration = tests_idx == crates_idx + 2
        else:
            # No crates/ prefix (e.g. fixture paths): treat top-level tests/ as integration
            is_integration = tests_idx == 0

    lines = text.splitlines(keepends=True)
    # Build line-start offset map
    line_offsets = []
    offset = 0
    for line in lines:
        line_offsets.append(offset)
        offset += len(line)

    def offset_to_lineno(off: int) -> int:
        lo, hi = 0, len(line_offsets) - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if line_offsets[mid] <= off:
                lo = mid
            else:
                hi = mid - 1
        return lo + 1  # 1-based

    results = []

    # Scan for #[cfg(test)] module boundaries to know which fns are inside
    # We use a simpler approach: track whether we're in a cfg(test) block
    # by scanning tokens linearly.

    # Strategy: collect all candidate test-fn positions:
    # Either preceded by a test attribute, or inside a cfg(test) mod.
    # We'll do two passes:
    # Pass 1: find all #[test]/#[tokio::test]/#[rstest] fn positions
    # Pass 2: find all cfg(test) mod blocks and harvest fn positions inside

    test_fn_positions: set[int] = set()

    # Pass 1: attribute-preceded
    for m in TEST_ATTR_RE.finditer(text):
        attr_end = m.end()
        # look for 'fn NAME(' after the attribute (possibly with whitespace/other attrs)
        window = text[attr_end:attr_end + 300]
        fn_m = FN_DEF_RE.search(window)
        if fn_m:
            test_fn_positions.add(attr_end + fn_m.start())

    # Pass 2: inside cfg(test) mods — find the mod body, then harvest only
    # #[test]/#[tokio::test]/#[rstest]-annotated fns inside it.  Plain helper
    # fns inside a test mod are NOT test functions and must not be counted.
    for cfg_m in CFG_TEST_MOD_RE.finditer(text):
        # find 'mod NAME {' after the cfg attr
        mod_start = cfg_m.end()
        mod_window = text[mod_start:mod_start + 200]
        mod_m = re.search(r"\bmod\s+\w+\s*\{", mod_window)
        if not mod_m:
            continue
        mod_body_start, mod_body_end = extract_body(text, mod_start + mod_m.start())
        if mod_body_start < 0:
            continue
        mod_body = text[mod_body_start:mod_body_end + 1]
        # only fns preceded by a test attribute count as tests
        for attr_m in TEST_ATTR_RE.finditer(mod_body):
            window = mod_body[attr_m.end():attr_m.end() + 300]
            fn_m = FN_DEF_RE.search(window)
            if fn_m:
                test_fn_positions.add(mod_body_start + attr_m.end() + fn_m.start())

    # Now extract body for each test fn position
    for pos in sorted(test_fn_positions):
        fn_m = FN_DEF_RE.match(text, pos)
        if not fn_m:
            # try searching from pos
            fn_m = FN_DEF_RE.search(text, pos, pos + 100)
        if not fn_m:
            continue
        fn_name = fn_m.group(1)
        start_line = offset_to_lineno(fn_m.start())
        body_start, body_end = extract_body(text, fn_m.start())
        if body_start < 0:
            body = ""
        else:
            body = text[body_start:body_end + 1]

        results.append({
            "fn_name": fn_name,
            "start_line": start_line,
            "body": body,
            "is_integration": is_integration,
            "test_kind": "integration" if is_integration else "unit",
            "crate": crate,
            "rel_path": rel,
        })

    return results


# ---------------------------------------------------------------------------
# Main overlay logic
# ---------------------------------------------------------------------------

def build_node_id(crate: str, rel_path: str, fn_name: str) -> str:
    return f"test::{crate}::{rel_path}::{fn_name}"


def run_overlay(
    graph_path: str,
    repo_root: str,
    out_path: str,
    report_path: str | None = None,
):
    print(f"Loading graph from {graph_path} ...", flush=True)
    with open(graph_path, encoding="utf-8") as f:
        graph = json.load(f)

    original_nodes = graph["nodes"]
    original_links = graph["links"]

    print(f"  {len(original_nodes)} nodes, {len(original_links)} links loaded.", flush=True)

    pruned_index, full_index = build_name_index(original_nodes)
    print(f"  Name index: {len(pruned_index)} pruned, {len(full_index)} full symbol names.", flush=True)

    repo = Path(repo_root)
    crates_root = repo / "crates"

    # Collect all .rs files, skip /target/
    rs_files = []
    if crates_root.exists():
        for f in crates_root.rglob("*.rs"):
            if "/target/" in f.as_posix().replace("\\", "/") or "\\target\\" in str(f):
                continue
            rs_files.append(f)
    print(f"  Found {len(rs_files)} Rust source files.", flush=True)

    # Parse tests
    all_tests = []
    skipped = 0
    for f in rs_files:
        try:
            tests = detect_tests(f, repo)
            all_tests.extend(tests)
        except (OSError, ValueError):
            skipped += 1

    print(f"  Extracted {len(all_tests)} test functions ({skipped} files skipped).", flush=True)

    # Build new nodes and edges
    new_nodes = []
    new_links = []
    edge_set: set[tuple[str, str, str]] = set()

    # Track coverage per code symbol
    proven_ids: set[str] = set()
    targeted_ids: set[str] = set()

    for t in all_tests:
        node_id = build_node_id(t["crate"], t["rel_path"], t["fn_name"])
        test_kind = "integration" if t["is_integration"] else "unit"

        node = {
            "id": node_id,
            "label": t["fn_name"],
            "file_type": "test",
            "source_file": t["rel_path"],
            "source_location": f"L{t['start_line']}",
            "_origin": "test",
            "norm_label": t["fn_name"],
            "test_kind": test_kind,
            "crate": t["crate"],
        }
        new_nodes.append(node)

        body = t["body"]
        if not body:
            continue

        # Find assertion spans
        assertion_spans = find_assertion_spans(body)

        # Tokenize identifiers in body
        body_lines = body.splitlines()

        # We'll use character offsets within body for assertion context
        for ident_m in IDENT_RE.finditer(body):
            ident = ident_m.group(1)
            if ident in RUST_KEYWORDS or len(ident) <= 2:
                continue

            # Resolution: two-path same-crate-first logic.
            #
            # Path 1 (same-crate, no spread cap): look in the full index for a
            # symbol whose crate matches the test crate.  The genericness cap does
            # NOT apply here — a test asserting on a symbol in its own crate is a
            # valid proof regardless of how common the name is elsewhere.
            # Stoplist / method-only filters are already applied inside full_index.
            #
            # Path 2 (cross-crate fallback, spread cap applies): use pruned_index
            # and require global uniqueness (exactly one candidate).
            all_candidates = full_index.get(ident, [])
            same_crate = [
                c for c in all_candidates
                if crate_from_source_file(c.get("source_file", "")) == t["crate"]
            ]

            if same_crate:
                # Path 1: same-crate match — always allowed
                selected = same_crate
                conf = "EXTRACTED" if len(same_crate) == 1 else "AMBIGUOUS"
            else:
                # Path 2: cross-crate fallback — use pruned index, require uniqueness
                pruned_candidates = pruned_index.get(ident, [])
                if len(pruned_candidates) == 1:
                    selected = pruned_candidates
                    conf = "EXTRACTED"
                else:
                    # Cross-crate non-unique or pruned-as-generic: drop
                    continue

            conf_score = 1.0 if conf == "EXTRACTED" else 0.5

            # Find line number within body for this match
            char_offset = ident_m.start()
            # compute line number relative to body start line
            body_line_num = body[:char_offset].count("\n")
            abs_line = t["start_line"] + body_line_num

            in_assert = in_assertion_context(char_offset, assertion_spans)

            for sym_node in selected:
                sym_id = sym_node["id"]

                # targets edge
                edge_key = (node_id, sym_id, "targets")
                if edge_key not in edge_set:
                    edge_set.add(edge_key)
                    new_links.append({
                        "relation": "targets",
                        "confidence": conf,
                        "source_file": t["rel_path"],
                        "source_location": f"L{abs_line}",
                        "weight": 1.0,
                        "source": node_id,
                        "target": sym_id,
                        "confidence_score": conf_score,
                    })
                    targeted_ids.add(sym_id)

                # proves edge (if in assertion context)
                if in_assert:
                    edge_key_p = (node_id, sym_id, "proves")
                    if edge_key_p not in edge_set:
                        edge_set.add(edge_key_p)
                        new_links.append({
                            "relation": "proves",
                            "confidence": "INFERRED",
                            "source_file": t["rel_path"],
                            "source_location": f"L{abs_line}",
                            "weight": 1.0,
                            "source": node_id,
                            "target": sym_id,
                            "confidence_score": 0.5,
                        })
                        proven_ids.add(sym_id)

    print(f"  New test nodes: {len(new_nodes)}", flush=True)
    targets_count = sum(1 for link in new_links if link["relation"] == "targets")
    proves_count = sum(1 for link in new_links if link["relation"] == "proves")
    print(f"  New edges: {targets_count} targets, {proves_count} proves", flush=True)

    # Write augmented graph
    augmented = dict(graph)
    augmented["nodes"] = original_nodes + new_nodes
    augmented["links"] = original_links + new_links

    print(f"Writing augmented graph to {out_path} ...", flush=True)
    with open(out_path, encoding="utf-8", mode="w") as f:
        json.dump(augmented, f, ensure_ascii=False)
    print(f"  Done. Total nodes: {len(augmented['nodes'])}, total links: {len(augmented['links'])}", flush=True)

    # Optional report
    if report_path:
        _write_report(
            report_path=report_path,
            original_nodes=original_nodes,
            new_nodes=new_nodes,
            new_links=new_links,
            proven_ids=proven_ids,
            targeted_ids=targeted_ids,
        )
        print(f"Report written to {report_path}", flush=True)


def _write_report(
    report_path: str,
    original_nodes: list[dict],
    new_nodes: list[dict],
    new_links: list[dict],
    proven_ids: set[str],
    targeted_ids: set[str],
):
    targets_count = sum(1 for link in new_links if link["relation"] == "targets")
    proves_count = sum(1 for link in new_links if link["relation"] == "proves")

    # Group code symbols by crate
    crate_symbols: dict[str, list[dict]] = defaultdict(list)
    for node in original_nodes:
        if node.get("_origin") == "test":
            continue
        crate = crate_from_source_file(node.get("source_file", ""))
        crate_symbols[crate].append(node)

    # Per-crate counts
    rows = []
    for crate, syms in sorted(crate_symbols.items()):
        sym_ids = {s["id"] for s in syms}
        proven = len(sym_ids & proven_ids)
        targeted_only = len((sym_ids & targeted_ids) - proven_ids)
        unproven = len(sym_ids - targeted_ids - proven_ids)
        rows.append((crate, len(syms), proven, targeted_only, unproven))

    total_tests = len(new_nodes)

    lines = [
        "# Semantic Test-Coverage Report",
        "",
        f"**Total test functions found:** {total_tests}",
        f"**Edges by relation:** `targets` = {targets_count}, `proves` = {proves_count}",
        "",
        "## Per-Crate Coverage",
        "",
        "| Crate | Symbols | Proven (≥1 proves) | Targeted-only | Unproven |",
        "|-------|---------|-------------------|---------------|----------|",
    ]
    for crate, total, proven, targeted_only, unproven in rows:
        lines.append(f"| {crate} | {total} | {proven} | {targeted_only} | {unproven} |")

    lines += [
        "",
        "---",
        "_Generated by overlay_tests.py — Phase 1 static analysis. No LLM used._",
    ]

    with open(report_path, encoding="utf-8", mode="w") as f:
        f.write("\n".join(lines) + "\n")


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Overlay test coverage onto a code knowledge graph.")
    parser.add_argument("--graph", required=True, help="Input graph JSON path")
    parser.add_argument("--repo-root", required=True, help="Repository root directory")
    parser.add_argument("--out", required=True, help="Output graph JSON path")
    parser.add_argument("--report", default=None, help="Optional Markdown report path")
    args = parser.parse_args()

    run_overlay(
        graph_path=args.graph,
        repo_root=args.repo_root,
        out_path=args.out,
        report_path=args.report,
    )


if __name__ == "__main__":
    main()
