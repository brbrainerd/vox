#!/usr/bin/env python
"""Write `.graphify_manifest.v1.json` beside a registered graphify corpus graph.

DEPRECATED: Use the new `scripts/graphify-refresh.vox` script instead.
This Python script is kept for legacy compatibility but is deprecated.

SSOT schema: `crates/vox-config/src/graphify.rs` (MANIFEST_BASENAME).
Registry: `contracts/retrieval/graphify-corpora.v1.yaml`.
"""
from __future__ import annotations

import warnings
warnings.warn(
    "manifest_writer.py is deprecated. Use `vox run scripts/graphify-refresh.vox` instead. "
    "See AGENTS.md §VoxScript-First Glue Code.",
    DeprecationWarning,
    stacklevel=2,
)

import hashlib
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

MANIFEST_BASENAME = ".graphify_manifest.v1.json"
REGISTRY_REL = "contracts/retrieval/graphify-corpora.v1.yaml"

_CORPUS_FIELD = re.compile(
    r"^\s+(scope_path|graph_path|manifest_path|extraction_mode):\s+(.+)$"
)


def graph_stats(graph: dict[str, Any]) -> tuple[int, int]:
    """Count nodes and edges/links in a graphify export JSON object."""
    nodes = graph.get("nodes") or []
    links = graph.get("links")
    if links is None:
        links = graph.get("edges") or []
    return len(nodes), len(links)


def _parse_registry_corpora(text: str) -> list[dict[str, str]]:
    corpora: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    for line in text.splitlines():
        id_match = re.match(r"^\s*-\s+id:\s+(\S+)", line)
        if id_match:
            if current:
                corpora.append(current)
            current = {"id": id_match.group(1)}
            continue
        if current is None:
            continue
        field_match = _CORPUS_FIELD.match(line)
        if field_match:
            key, raw = field_match.group(1), field_match.group(2).strip()
            current[key] = raw.strip('"').strip("'")
    if current:
        corpora.append(current)
    return corpora


def load_corpora(repo_root: Path) -> list[dict[str, str]]:
    registry_path = repo_root / REGISTRY_REL
    return _parse_registry_corpora(registry_path.read_text(encoding="utf-8"))


def find_corpus_for_graph_path(
    repo_root: Path, graph_rel_posix: str
) -> dict[str, str] | None:
    graph_rel_posix = graph_rel_posix.replace("\\", "/")
    for corpus in load_corpora(repo_root):
        if corpus.get("graph_path") == graph_rel_posix:
            return corpus
    return None


def git_head_sha(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def write_graphify_manifest(repo_root: Path, graph_path: Path) -> Path:
    """Write manifest for a registered corpus graph path; return manifest path."""
    repo_root = repo_root.resolve()
    graph_path = graph_path.resolve()
    rel = graph_path.relative_to(repo_root).as_posix()
    corpus = find_corpus_for_graph_path(repo_root, rel)
    if corpus is None:
        raise ValueError(f"no graphify corpus registered for graph path `{rel}`")

    graph_bytes = graph_path.read_bytes()
    graph_obj = json.loads(graph_bytes.decode("utf-8"))
    node_count, edge_count = graph_stats(graph_obj)

    manifest = {
        "corpus_id": corpus["id"],
        "built_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "git_sha": git_head_sha(repo_root),
        "scope_path": corpus.get("scope_path", "."),
        "node_count": node_count,
        "edge_count": edge_count,
        "graph_json_sha256": hashlib.sha256(graph_bytes).hexdigest(),
        "extraction_mode": corpus.get("extraction_mode"),
    }

    manifest_path = repo_root / corpus["manifest_path"]
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(
        json.dumps(manifest, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return manifest_path


def find_repo_root(start: Path | None = None) -> Path:
    """Walk parents until the graphify corpora registry is found."""
    cur = (start or Path.cwd()).resolve()
    if cur.is_file():
        cur = cur.parent
    for _ in range(24):
        if (cur / REGISTRY_REL).is_file():
            return cur
        parent = cur.parent
        if parent == cur:
            break
        cur = parent
    raise FileNotFoundError(
        f"could not locate repo root (missing {REGISTRY_REL}) from {start}"
    )


def maybe_write_graphify_manifest(repo_root: Path, graph_path: Path) -> Path | None:
    """Write manifest when graph_path matches a registry entry; else no-op."""
    repo_root = repo_root.resolve()
    graph_path = graph_path.resolve()
    try:
        rel = graph_path.relative_to(repo_root).as_posix()
    except ValueError:
        return None
    if find_corpus_for_graph_path(repo_root, rel) is None:
        return None
    return write_graphify_manifest(repo_root, graph_path)
