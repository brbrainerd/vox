"""Tests for manifest_writer.py — graphify corpus manifest (P0.5)."""
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

from manifest_writer import (
    MANIFEST_BASENAME,
    graph_stats,
    maybe_write_graphify_manifest,
    write_graphify_manifest,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY = REPO_ROOT / "contracts/retrieval/graphify-corpora.v1.yaml"


def _write_minimal_registry(tmp: Path) -> None:
  dest = tmp / "contracts/retrieval"
  dest.mkdir(parents=True)
  dest.joinpath("graphify-corpora.v1.yaml").write_text(
      REGISTRY.read_text(encoding="utf-8"),
      encoding="utf-8",
  )


def _init_git(tmp: Path, sha: str = "abc123deadbeef") -> None:
  subprocess.run(["git", "init"], cwd=tmp, check=True, capture_output=True)
  subprocess.run(
      ["git", "config", "user.email", "test@test.local"],
      cwd=tmp,
      check=True,
      capture_output=True,
  )
  subprocess.run(
      ["git", "config", "user.name", "test"],
      cwd=tmp,
      check=True,
      capture_output=True,
  )
  (tmp / "marker.txt").write_text("x\n", encoding="utf-8")
  subprocess.run(["git", "add", "marker.txt"], cwd=tmp, check=True, capture_output=True)
  subprocess.run(
      ["git", "commit", "-m", "init"],
      cwd=tmp,
      check=True,
      capture_output=True,
  )
  subprocess.run(
      ["git", "checkout", "-B", "test-branch"],
      cwd=tmp,
      check=True,
      capture_output=True,
  )
  if sha != "HEAD":
    # Leave real HEAD sha; tests compare against git rev-parse output.
    pass


class TestGraphStats:
  def test_counts_nodes_and_links(self):
    graph = {"nodes": [{"id": "a"}, {"id": "b"}], "links": [{"source": "a", "target": "b"}]}
    assert graph_stats(graph) == (2, 1)

  def test_accepts_edges_key(self):
    graph = {"nodes": [{}], "edges": [{}, {}]}
    assert graph_stats(graph) == (1, 2)


class TestWriteGraphifyManifest:
  def test_writes_manifest_beside_registered_graph(self, tmp_path):
    _write_minimal_registry(tmp_path)
    _init_git(tmp_path)

    graph_dir = tmp_path / "graphify-out"
    graph_dir.mkdir(parents=True)
    graph_path = graph_dir / "graph.json"
    graph_body = {"nodes": [{"id": "n1"}], "links": []}
    graph_bytes = json.dumps(graph_body, separators=(",", ":")).encode("utf-8")
    graph_path.write_bytes(graph_bytes)

    before = datetime.now(timezone.utc)
    manifest_path = write_graphify_manifest(tmp_path, graph_path)
    after = datetime.now(timezone.utc)

    assert manifest_path == graph_dir / MANIFEST_BASENAME
    assert manifest_path.is_file()

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert manifest["corpus_id"] == "repo-code-graph"
    assert manifest["scope_path"] == "."
    assert manifest["extraction_mode"] == "structural"
    assert manifest["node_count"] == 1
    assert manifest["edge_count"] == 0
    assert manifest["graph_json_sha256"] == hashlib.sha256(graph_bytes).hexdigest()

    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    assert manifest["git_sha"] == head

    built_at = datetime.fromisoformat(manifest["built_at"].replace("Z", "+00:00"))
    assert before.replace(microsecond=0) <= built_at <= after.replace(microsecond=0) + timedelta(
        seconds=1
    )

  def test_maybe_skips_unregistered_graph_path(self, tmp_path):
    _write_minimal_registry(tmp_path)
    graph_path = tmp_path / "graphify-out" / "graph.full.json"
    graph_path.parent.mkdir(parents=True)
    graph_path.write_text('{"nodes":[],"links":[]}', encoding="utf-8")
    assert maybe_write_graphify_manifest(tmp_path, graph_path) is None
