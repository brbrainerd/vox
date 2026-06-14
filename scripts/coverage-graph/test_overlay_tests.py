"""
Tests for overlay_tests.py — Phase 1 semantic coverage overlay.
"""
import json
import sys
import os
from pathlib import Path
import pytest

# Ensure the script directory is importable
sys.path.insert(0, str(Path(__file__).parent))

from overlay_tests import (
    detect_tests,
    build_name_index,
    extract_body,
    find_assertion_spans,
    in_assertion_context,
    crate_from_source_file,
    is_production_symbol,
    run_overlay,
    _strip_label,
    RUST_KEYWORDS,
    STD_PRELUDE_STOPLIST,
    MAX_NAME_CRATE_SPREAD,
)

FIXTURES_DIR = Path(__file__).parent / "fixtures"
FIXTURE_GRAPH = FIXTURES_DIR / "fixture_graph.json"
SRC_LIB = FIXTURES_DIR / "crates" / "fixture-lib" / "src" / "lib.rs"
INTEGRATION_TEST = FIXTURES_DIR / "crates" / "fixture-lib" / "tests" / "integration_test.rs"
TARGET_FILE = FIXTURES_DIR / "target" / "skip_me.rs"


# ---------------------------------------------------------------------------
# Unit tests: helpers
# ---------------------------------------------------------------------------

class TestStripLabel:
    def test_strips_parens(self):
        assert _strip_label("add()") == "add"

    def test_strips_dot_and_parens(self):
        assert _strip_label(".set_security()") == "set_security"

    def test_strips_generics(self):
        assert _strip_label("MyStruct<T>") == "MyStruct"

    def test_plain_name(self):
        assert _strip_label("multiply") == "multiply"


class TestCrateFromSourceFile:
    def test_normal_path(self):
        assert crate_from_source_file("crates/vox-compiler/src/lib.rs") == "vox-compiler"

    def test_unknown(self):
        assert crate_from_source_file("some/random/path.rs") == "unknown"


class TestExtractBody:
    def test_simple_body(self):
        code = "fn foo() { let x = 1; }"
        m_start = code.index("fn foo")
        s, e = extract_body(code, m_start)
        assert s >= 0
        assert code[s:e+1] == "{ let x = 1; }"

    def test_nested_braces(self):
        code = "fn bar() { if true { let x = { 1 + 2 }; } }"
        s, e = extract_body(code, 0)
        assert s >= 0
        body = code[s:e+1]
        assert body.count("{") == body.count("}")
        assert "1 + 2" in body

    def test_no_brace_returns_minus1(self):
        code = "fn foo()"
        s, e = extract_body(code, 0)
        assert s == -1


class TestFindAssertionSpans:
    def test_assert_eq(self):
        body = "{ assert_eq!(add(2, 3), 5); }"
        spans = find_assertion_spans(body)
        assert len(spans) > 0
        # "add" should be in an assertion span
        pos = body.index("add")
        assert in_assertion_context(pos, spans)

    def test_non_assert_not_in_span(self):
        body = "{ let x = multiply(2, 3); }"
        spans = find_assertion_spans(body)
        pos = body.index("multiply")
        assert not in_assertion_context(pos, spans)

    def test_expect_span(self):
        body = '{ let v: Option<i32> = Some(1); let _ = v.expect("msg"); }'
        spans = find_assertion_spans(body)
        # "v" before .expect should be in the span
        # find the position of 'v' in the .expect line
        pos = body.index("v.expect")
        assert in_assertion_context(pos, spans)

    def test_unwrap_span(self):
        body = "{ let result = some_fn().unwrap(); }"
        spans = find_assertion_spans(body)
        pos = body.index("some_fn")
        assert in_assertion_context(pos, spans)


# ---------------------------------------------------------------------------
# Test detection on fixtures
# ---------------------------------------------------------------------------

class TestDetectTests:
    def test_detects_unit_tests_in_lib(self):
        tests = detect_tests(SRC_LIB, FIXTURES_DIR)
        names = [t["fn_name"] for t in tests]
        assert "test_add_basic" in names
        assert "test_multiply_called_not_asserted" in names
        assert "test_add_with_expect" in names
        assert "test_nested_braces" in names

    def test_add_not_in_test_names(self):
        """'add' itself is not a test function."""
        tests = detect_tests(SRC_LIB, FIXTURES_DIR)
        names = [t["fn_name"] for t in tests]
        assert "add" not in names

    def test_integration_flag(self):
        tests = detect_tests(INTEGRATION_TEST, FIXTURES_DIR)
        assert len(tests) > 0
        for t in tests:
            assert t["is_integration"] is True

    def test_target_skipped(self):
        """Files under /target/ should be skipped by the walk (not by detect_tests itself)."""
        # detect_tests CAN parse target files if called directly,
        # but the walker excludes them. We verify the file parses fine at least.
        tests = detect_tests(TARGET_FILE, FIXTURES_DIR)
        # It may or may not find the fn (it's not inside a test mod/attr-annotated)
        # the key is it doesn't crash
        assert isinstance(tests, list)

    def test_unit_kind_label(self):
        tests = detect_tests(SRC_LIB, FIXTURES_DIR)
        for t in tests:
            assert t["test_kind"] == "unit" if not t["is_integration"] else t["test_kind"] == "integration"


# ---------------------------------------------------------------------------
# Full overlay run on fixtures
# ---------------------------------------------------------------------------

@pytest.fixture(scope="module")
def overlay_result(tmp_path_factory):
    """Run overlay_tests on the fixture graph + fixtures dir, return parsed output."""
    tmp = tmp_path_factory.mktemp("overlay")
    out_json = str(tmp / "out.json")
    report_md = str(tmp / "report.md")

    run_overlay(
        graph_path=str(FIXTURE_GRAPH),
        repo_root=str(FIXTURES_DIR),
        out_path=out_json,
        report_path=report_md,
    )
    with open(out_json, encoding="utf-8") as f:
        graph = json.load(f)
    with open(report_md, encoding="utf-8") as f:
        report = f.read()
    return graph, report


class TestFullOverlay:
    def test_output_has_required_keys(self, overlay_result):
        graph, _ = overlay_result
        for key in ("directed", "multigraph", "graph", "nodes", "links", "hyperedges", "built_at_commit"):
            assert key in graph

    def test_test_nodes_added(self, overlay_result):
        graph, _ = overlay_result
        test_nodes = [n for n in graph["nodes"] if n.get("_origin") == "test"]
        assert len(test_nodes) >= 4, f"Expected >=4 test nodes, got {len(test_nodes)}"

    def test_test_node_schema(self, overlay_result):
        graph, _ = overlay_result
        test_nodes = [n for n in graph["nodes"] if n.get("_origin") == "test"]
        for n in test_nodes:
            assert n["id"].startswith("test::")
            assert n["file_type"] == "test"
            assert "source_file" in n
            assert "source_location" in n
            assert "test_kind" in n
            assert "crate" in n
            assert "norm_label" in n

    def test_proves_edge_for_asserted_symbol(self, overlay_result):
        """'add' is asserted in test_add_basic → should have a proves edge."""
        graph, _ = overlay_result
        proves_edges = [l for l in graph["links"] if l["relation"] == "proves"]
        targets = {l["target"] for l in proves_edges}
        # fixture_lib_add should appear as a proves target
        assert "fixture_lib_add" in targets, (
            f"Expected fixture_lib_add in proves targets, got: {sorted(targets)}"
        )

    def test_targets_edge_for_called_symbol(self, overlay_result):
        """'multiply' is called but not asserted → should have targets but NOT proves."""
        graph, _ = overlay_result
        proves_edges = [l for l in graph["links"] if l["relation"] == "proves"]
        targets_edges = [l for l in graph["links"] if l["relation"] == "targets"]

        proven_targets = {l["target"] for l in proves_edges}
        targeted_targets = {l["target"] for l in targets_edges}

        assert "fixture_lib_multiply" in targeted_targets, "multiply should be targeted"
        assert "fixture_lib_multiply" not in proven_targets, "multiply should NOT be proven"

    def test_nested_brace_test_detected(self, overlay_result):
        """test_nested_braces (which has nested braces in its body) should appear."""
        graph, _ = overlay_result
        test_names = [n["label"] for n in graph["nodes"] if n.get("_origin") == "test"]
        assert "test_nested_braces" in test_names

    def test_no_duplicate_edges(self, overlay_result):
        """No (source, target, relation) triple should appear twice."""
        graph, _ = overlay_result
        seen = set()
        for l in graph["links"]:
            key = (l.get("source"), l.get("target"), l.get("relation"))
            assert key not in seen, f"Duplicate edge: {key}"
            seen.add(key)

    def test_edge_schema(self, overlay_result):
        """New edges should have all required fields."""
        graph, _ = overlay_result
        new_edges = [l for l in graph["links"] if l.get("relation") in ("targets", "proves")]
        for e in new_edges:
            assert "relation" in e
            assert "confidence" in e
            assert "source_file" in e
            assert "source_location" in e
            assert "weight" in e
            assert "source" in e
            assert "target" in e
            assert "confidence_score" in e

    def test_report_has_table(self, overlay_result):
        _, report = overlay_result
        assert "Per-Crate Coverage" in report
        assert "| Crate" in report
        assert "Proven" in report


class TestTargetSkipping:
    """Verify that the walker (not just detect_tests) skips /target/ paths."""

    def test_target_file_not_in_results(self, tmp_path):
        """
        Run overlay with a repo_root that has a /target/ file.
        The target file contains a test fn. Verify no test node for it.
        """
        # Create a minimal fixture structure
        crates_dir = tmp_path / "crates" / "my-crate"
        src_dir = crates_dir / "src"
        target_dir = crates_dir / "target"
        src_dir.mkdir(parents=True)
        target_dir.mkdir(parents=True)

        (src_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n    fn real_test() { assert_eq!(1, 1); }\n}\n',
            encoding="utf-8"
        )
        (target_dir / "fake.rs").write_text(
            '#[test]\nfn should_be_skipped() { assert_eq!(2, 2); }\n',
            encoding="utf-8"
        )

        # minimal graph
        graph = {"directed": True, "multigraph": False, "graph": {}, "nodes": [], "links": [], "hyperedges": [], "built_at_commit": "x"}
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        test_names = [n["label"] for n in result["nodes"] if n.get("_origin") == "test"]
        assert "should_be_skipped" not in test_names
        assert "real_test" in test_names


# ---------------------------------------------------------------------------
# Precision defect fixes — TDD for the new stoplist / method-drop / spread-cap
# ---------------------------------------------------------------------------

def _make_minimal_graph(nodes):
    """Build a minimal graph dict from a list of node dicts."""
    return {
        "directed": True, "multigraph": False, "graph": {},
        "nodes": nodes, "links": [], "hyperedges": [], "built_at_commit": "x",
    }


class TestStdPreludeStoplist:
    """Std/prelude names must NEVER appear as edge targets."""

    def test_stoplist_is_frozenset(self):
        assert isinstance(STD_PRELUDE_STOPLIST, frozenset)

    def test_stoplist_contains_key_names(self):
        for name in ("Result", "Vec", "Path", "String", "Option", "Error",
                     "HashMap", "Default", "Self", "Clone", "Iterator"):
            assert name in STD_PRELUDE_STOPLIST, f"{name!r} missing from STD_PRELUDE_STOPLIST"

    def test_no_edge_to_std_name(self, tmp_path):
        """
        A test that returns Result / uses Vec / Path should produce NO edge to those names,
        even if the graph contains nodes with those labels.
        """
        # Graph has nodes named Result, Vec, Path in an unrelated crate
        nodes = [
            {"id": "other_Result", "label": "Result", "file_type": "code",
             "source_file": "crates/other-crate/src/lib.rs", "_origin": "ast", "norm_label": "Result"},
            {"id": "other_Vec", "label": "Vec", "file_type": "code",
             "source_file": "crates/other-crate/src/lib.rs", "_origin": "ast", "norm_label": "Vec"},
            {"id": "other_Path", "label": "Path", "file_type": "code",
             "source_file": "crates/other-crate/src/lib.rs", "_origin": "ast", "norm_label": "Path"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        # Write a test file that references Result, Vec, Path
        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    use super::*;\n'
            '    #[test]\n    fn test_uses_std_types() {\n'
            '        let v: Vec<String> = Vec::new();\n'
            '        let r: Result<(), ()> = Ok(());\n'
            '        assert!(r.is_ok());\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        for node_id in ("other_Result", "other_Vec", "other_Path"):
            assert node_id not in edge_targets, (
                f"Std name {node_id!r} should not be an edge target"
            )


class TestMethodOnlyLabels:
    """Nodes whose label starts with '.' must never be edge targets."""

    def test_method_node_not_indexed(self):
        nodes = [
            {"id": "n1", "label": ".default()", "file_type": "code",
             "source_file": "crates/foo/src/lib.rs", "_origin": "ast", "norm_label": ".default()"},
            {"id": "n2", "label": ".parse()", "file_type": "code",
             "source_file": "crates/foo/src/lib.rs", "_origin": "ast", "norm_label": ".parse()"},
            {"id": "n3", "label": "real_fn()", "file_type": "code",
             "source_file": "crates/foo/src/lib.rs", "_origin": "ast", "norm_label": "real_fn()"},
        ]
        pruned_idx, full_idx = build_name_index(nodes)
        # .default() and .parse() must not be indexed under their stripped names in EITHER index
        assert "default" not in pruned_idx, "method-only node .default() should not be in pruned index"
        assert "parse" not in pruned_idx, "method-only node .parse() should not be in pruned index"
        assert "default" not in full_idx, "method-only node .default() should not be in full index"
        assert "parse" not in full_idx, "method-only node .parse() should not be in full index"
        # real_fn should still be indexed in both
        assert "real_fn" in pruned_idx
        assert "real_fn" in full_idx

    def test_no_edge_to_method_only_node(self, tmp_path):
        """
        Even if a test body references 'default', no edge should point at a
        method-only node whose label is '.default()'.
        """
        nodes = [
            {"id": "n_default", "label": ".default()", "file_type": "code",
             "source_file": "crates/my-crate/src/lib.rs", "_origin": "ast", "norm_label": ".default()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_calls_default() {\n'
            '        let x = SomeType::default();\n'
            '        assert_eq!(x.value, 0);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        assert "n_default" not in edge_targets, ".default() node should not be an edge target"


class TestNameCrateSpread:
    """Names appearing in >MAX_NAME_CRATE_SPREAD crates must be skipped."""

    def test_constant_is_defined(self):
        assert isinstance(MAX_NAME_CRATE_SPREAD, int)
        assert MAX_NAME_CRATE_SPREAD == 3

    def test_spread_name_not_indexed_when_over_threshold(self):
        """A symbol name in 4 distinct crates should be excluded from the PRUNED index."""
        nodes = [
            {"id": f"crate{i}_generic_helper", "label": "generic_helper", "file_type": "code",
             "source_file": f"crates/crate-{i}/src/lib.rs", "_origin": "ast", "norm_label": "generic_helper"}
            for i in range(MAX_NAME_CRATE_SPREAD + 1)  # 4 crates
        ]
        pruned_idx, full_idx = build_name_index(nodes)
        assert "generic_helper" not in pruned_idx, (
            "symbol in >3 crates should be excluded from pruned name index"
        )
        # But it IS present in the full index (for same-crate lookup)
        assert "generic_helper" in full_idx, (
            "symbol in >3 crates should still appear in full index (same-crate lookup)"
        )

    def test_spread_name_indexed_when_at_threshold(self):
        """A symbol name in exactly MAX_NAME_CRATE_SPREAD crates should be kept in both indices."""
        nodes = [
            {"id": f"crate{i}_rare_fn", "label": "rare_fn", "file_type": "code",
             "source_file": f"crates/crate-{i}/src/lib.rs", "_origin": "ast", "norm_label": "rare_fn"}
            for i in range(MAX_NAME_CRATE_SPREAD)  # exactly 3 crates
        ]
        pruned_idx, full_idx = build_name_index(nodes)
        assert "rare_fn" in pruned_idx, "symbol in <=3 crates should remain in pruned index"
        assert "rare_fn" in full_idx, "symbol in <=3 crates should remain in full index"

    def test_no_edge_to_spread_symbol(self, tmp_path):
        """
        A symbol present in 4+ crates should produce no edge from a test
        that references it.
        """
        nodes = [
            {"id": f"crate{i}_common_fn", "label": "common_fn", "file_type": "code",
             "source_file": f"crates/crate-{i}/src/lib.rs", "_origin": "ast", "norm_label": "common_fn"}
            for i in range(MAX_NAME_CRATE_SPREAD + 1)
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_uses_common() {\n'
            '        assert_eq!(common_fn(), 0);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        for i in range(MAX_NAME_CRATE_SPREAD + 1):
            assert f"crate{i}_common_fn" not in edge_targets, (
                f"crate{i}_common_fn should not be an edge target (symbol too generic)"
            )


class TestSameCrateOrUniqueResolution:
    """Same-crate or globally-unique symbols get edges; cross-crate non-unique do not."""

    def test_same_crate_unique_gets_proves_edge(self, tmp_path):
        """A same-crate symbol that is asserted on STILL gets a proves edge (recall preserved)."""
        nodes = [
            {"id": "my_crate_real_fn", "label": "real_fn()", "file_type": "code",
             "source_file": "crates/my-crate/src/lib.rs", "_origin": "ast", "norm_label": "real_fn()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n'
            '    fn test_real_fn() {\n'
            '        assert_eq!(real_fn(), 42);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        proves_targets = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "my_crate_real_fn" in proves_targets, (
            "same-crate asserted symbol should have a proves edge"
        )

    def test_cross_crate_non_unique_dropped(self, tmp_path):
        """Cross-crate match on a non-unique name is dropped."""
        # Two crates have a symbol named 'shared_fn' — neither is same-crate as the test
        nodes = [
            {"id": "crate_a_shared_fn", "label": "shared_fn()", "file_type": "code",
             "source_file": "crates/crate-a/src/lib.rs", "_origin": "ast", "norm_label": "shared_fn()"},
            {"id": "crate_b_shared_fn", "label": "shared_fn()", "file_type": "code",
             "source_file": "crates/crate-b/src/lib.rs", "_origin": "ast", "norm_label": "shared_fn()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        # Test is in 'my-crate', which has no node named shared_fn
        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_uses_shared() {\n'
            '        assert_eq!(shared_fn(), 0);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        assert "crate_a_shared_fn" not in edge_targets, "cross-crate non-unique should be dropped"
        assert "crate_b_shared_fn" not in edge_targets, "cross-crate non-unique should be dropped"

    def test_cross_crate_globally_unique_kept(self, tmp_path):
        """Cross-crate match on a globally-unique name IS kept."""
        nodes = [
            {"id": "other_crate_unique_symbol", "label": "unique_symbol_xyz()", "file_type": "code",
             "source_file": "crates/other-crate/src/lib.rs", "_origin": "ast",
             "norm_label": "unique_symbol_xyz()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_uses_unique() {\n'
            '        assert_eq!(unique_symbol_xyz(), 1);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        proves_targets = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "other_crate_unique_symbol" in proves_targets, (
            "globally-unique cross-crate symbol should be kept"
        )


# ---------------------------------------------------------------------------
# Regression tests for same-crate bypass of genericness cap (the over-correction fix)
# ---------------------------------------------------------------------------

class TestSameCrateBypassesGenericnessCap:
    """
    A generic name (spread across >MAX_NAME_CRATE_SPREAD crates) must still
    produce a proves edge when the test is in the SAME crate as the symbol node.
    The genericness cap should only gate the cross-crate fallback path.
    """

    def test_same_crate_generic_name_gets_proves_edge(self, tmp_path):
        """
        'Config' lives in 5 crates. A test in 'vox-db' asserts on Config which is
        ALSO defined in 'vox-db'. Must produce a proves edge (regression guard).
        """
        # Build 5 nodes for 'Config' across 5 crates; one of them IS vox-db
        nodes = [
            {"id": "vox_db_Config", "label": "Config", "file_type": "code",
             "source_file": "crates/vox-db/src/lib.rs", "_origin": "ast", "norm_label": "Config"},
            {"id": "crate_a_Config", "label": "Config", "file_type": "code",
             "source_file": "crates/crate-a/src/lib.rs", "_origin": "ast", "norm_label": "Config"},
            {"id": "crate_b_Config", "label": "Config", "file_type": "code",
             "source_file": "crates/crate-b/src/lib.rs", "_origin": "ast", "norm_label": "Config"},
            {"id": "crate_c_Config", "label": "Config", "file_type": "code",
             "source_file": "crates/crate-c/src/lib.rs", "_origin": "ast", "norm_label": "Config"},
            {"id": "crate_d_Config", "label": "Config", "file_type": "code",
             "source_file": "crates/crate-d/src/lib.rs", "_origin": "ast", "norm_label": "Config"},
        ]
        # Sanity: 5 crates > MAX_NAME_CRATE_SPREAD (3)
        assert len(nodes) > MAX_NAME_CRATE_SPREAD

        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        # Test is in vox-db — same crate as vox_db_Config
        crate_dir = tmp_path / "crates" / "vox-db" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n'
            '    fn test_config_in_vox_db() {\n'
            '        assert_eq!(Config::default().timeout, 30);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        proves_targets = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "vox_db_Config" in proves_targets, (
            "same-crate Config must get a proves edge even when Config spread > MAX_NAME_CRATE_SPREAD"
        )

    def test_generic_name_no_cross_crate_edge_when_no_same_crate_node(self, tmp_path):
        """
        'Config' spread across 5 crates, but the test is in 'my-crate' which has NO
        Config node. Must produce NO cross-crate edge (no leakage).
        """
        nodes = [
            {"id": f"crate_{c}_Config", "label": "Config", "file_type": "code",
             "source_file": f"crates/crate-{c}/src/lib.rs", "_origin": "ast", "norm_label": "Config"}
            for c in ["a", "b", "c", "d", "e"]
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        # Test is in 'my-crate' — no Config node there
        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_no_local_config() {\n'
            '        assert_eq!(Config::new().value, 0);\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        for c in ["a", "b", "c", "d", "e"]:
            assert f"crate_{c}_Config" not in edge_targets, (
                f"crate_{c}_Config must not get an edge: generic name, no same-crate node"
            )

    def test_std_prelude_name_blocked_even_in_same_crate(self, tmp_path):
        """
        Even when a std/prelude name has a same-crate symbol node, the stoplist
        MUST still prevent edge creation.
        """
        # 'Result' is in the stoplist — no edge regardless
        nodes = [
            {"id": "my_crate_Result", "label": "Result", "file_type": "code",
             "source_file": "crates/my-crate/src/lib.rs", "_origin": "ast", "norm_label": "Result"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")

        crate_dir = tmp_path / "crates" / "my-crate" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            '#[cfg(test)]\nmod tests {\n    #[test]\n'
            '    fn test_std_blocked() {\n'
            '        let r: Result<i32, ()> = Ok(42);\n'
            '        assert!(r.is_ok());\n'
            '    }\n}\n',
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)

        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)

        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)

        edge_targets = {e["target"] for e in result["links"]
                        if e.get("relation") in ("targets", "proves")}
        assert "my_crate_Result" not in edge_targets, (
            "std/prelude name 'Result' must be blocked even on same-crate path"
        )


# ---------------------------------------------------------------------------
# Production-symbol filtering for the per-crate report denominator (Task 0.1)
# ---------------------------------------------------------------------------

class TestProductionSymbolFilter:
    """The report's per-crate 'Symbols' count must include only real production
    definitions: src_-prefixed defs under /src/, excluding file nodes, type/std
    REFERENCE nodes (crates_-prefixed ids), and in-src #[cfg(test)] test fns."""

    TEST_FNS = {"crates/c/src/lib.rs": {"my_test"}}

    def test_real_src_definition_is_production(self):
        assert is_production_symbol(
            {"id": "src_lib_do_thing", "label": "do_thing()", "_origin": "ast",
             "source_file": "crates/c/src/lib.rs"}, self.TEST_FNS) is True

    def test_subdirectory_definition_is_production(self):
        # Regression for C1: a symbol in a src/ SUBDIRECTORY has a non-`src_` id
        # prefix (the first path component is the subdir, not `src`). It MUST still
        # count as production — the old `startswith("src_")` gate wrongly dropped it.
        assert is_production_symbol(
            {"id": "commands_mod_run", "label": "run()", "_origin": "ast",
             "source_file": "crates/c/src/commands/mod.rs"}, self.TEST_FNS) is True
        assert is_production_symbol(
            {"id": "store_pool_acquire", "label": "acquire()", "_origin": "ast",
             "source_file": "crates/c/src/store/pool.rs"}, self.TEST_FNS) is True

    def test_file_node_is_excluded(self):
        assert is_production_symbol(
            {"id": "crates_c_src_lib_rs", "label": "lib.rs", "_origin": "ast",
             "source_file": "crates/c/src/lib.rs"}, self.TEST_FNS) is False

    def test_reference_node_is_excluded(self):
        # type/std-use nodes carry full-path 'crates_' ids, not 'src_'
        assert is_production_symbol(
            {"id": "crates_c_src_lib_rs_option", "label": "Option", "_origin": "ast",
             "source_file": "crates/c/src/lib.rs"}, self.TEST_FNS) is False

    def test_in_src_test_fn_is_excluded(self):
        assert is_production_symbol(
            {"id": "src_lib_my_test", "label": "my_test()", "_origin": "ast",
             "source_file": "crates/c/src/lib.rs"}, self.TEST_FNS) is False

    def test_non_src_definition_is_excluded(self):
        # benches/examples/build.rs defs are not production library symbols
        assert is_production_symbol(
            {"id": "benches_b_bench_it", "label": "bench_it()", "_origin": "ast",
             "source_file": "crates/c/benches/b.rs"}, self.TEST_FNS) is False

    def test_test_origin_node_is_excluded(self):
        assert is_production_symbol(
            {"id": "test::c::crates/c/src/lib.rs::my_test", "label": "my_test",
             "_origin": "test", "source_file": "crates/c/src/lib.rs"}, self.TEST_FNS) is False

    def test_report_denominator_counts_only_production(self, tmp_path):
        # A graph with one real def + one file node + one ref node + one in-src
        # test-fn def. The report's Symbols column must be 1, not 4.
        nodes = [
            {"id": "src_lib_do_thing", "label": "do_thing()", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": "do_thing()"},
            {"id": "crates_c_src_lib_rs", "label": "lib.rs", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": "lib.rs"},
            {"id": "crates_c_src_lib_rs_option", "label": "Option", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": "option"},
            {"id": "src_lib_my_test", "label": "my_test()", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": "my_test()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")
        report = str(tmp_path / "report.md")
        crate_dir = tmp_path / "crates" / "c" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            "pub fn do_thing() -> i32 { 1 }\n"
            "#[cfg(test)]\nmod tests {\n    #[test]\n"
            "    fn my_test() { assert_eq!(do_thing(), 1); }\n}\n",
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)
        run_overlay(graph_path=in_json, repo_root=str(tmp_path),
                    out_path=out_json, report_path=report)
        text = Path(report).read_text(encoding="utf-8")
        # find the row for crate 'c' and assert Symbols == 1
        row = [ln for ln in text.splitlines() if ln.startswith("| c |")]
        assert row, f"no crate row for 'c' in report:\n{text}"
        symbols = int(row[0].split("|")[2].strip())
        assert symbols == 1, f"expected 1 production symbol, report counted {symbols}"


# ---------------------------------------------------------------------------
# Crediting method (.foo()) assertions (Task 0.2)
# ---------------------------------------------------------------------------

class TestMethodAssertionCredit:
    """A test asserting on a same-crate METHOD call (`x.redact(...)`) must create
    a `proves` edge to that method's definition node (label `.redact()`). Before
    Task 0.2 the analyzer dropped every leading-dot label, so methods could never
    be proven — the dominant false-negative class in the fidelity audit."""

    def _run(self, tmp_path, src_body):
        nodes = [
            {"id": "src_lib_piifilter", "label": "PiiFilter", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": "piifilter"},
            {"id": "src_lib_redact", "label": ".redact()", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": ".redact()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")
        crate_dir = tmp_path / "crates" / "c" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(src_body, encoding="utf-8")
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)
        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)
        with open(out_json, encoding="utf-8") as f:
            return json.load(f)

    def test_method_assertion_creates_proves_edge(self, tmp_path):
        src = (
            "pub struct PiiFilter;\n"
            "impl PiiFilter { pub fn redact(&self, _s: &str) -> String { \"***\".into() } }\n"
            "#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n"
            "    fn redacts_email() {\n"
            "        let f = PiiFilter;\n"
            "        assert_eq!(f.redact(\"a@b.com\"), \"***\");\n"
            "    }\n}\n"
        )
        result = self._run(tmp_path, src)
        proves = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "src_lib_redact" in proves, (
            "method `.redact()` asserted in a test must get a proves edge"
        )

    def test_method_outside_assertion_is_targets_not_proves(self, tmp_path):
        # method called but NOT inside an assertion → targets edge, no proves
        src = (
            "pub struct PiiFilter;\n"
            "impl PiiFilter { pub fn redact(&self, _s: &str) -> String { \"***\".into() } }\n"
            "#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n"
            "    fn calls_without_asserting() {\n"
            "        let f = PiiFilter;\n"
            "        let _out = f.redact(\"x\");\n"
            "        assert!(true);\n"
            "    }\n}\n"
        )
        result = self._run(tmp_path, src)
        rels = {(e["target"], e["relation"]) for e in result["links"]}
        assert ("src_lib_redact", "targets") in rels, "method call should be a targets edge"
        assert ("src_lib_redact", "proves") not in rels, (
            "method not inside an assertion must NOT be proven"
        )

    def test_chained_method_calls_each_credited(self, tmp_path):
        # both methods in `a.redact(..).trim()` inside an assertion must be proven
        nodes = [
            {"id": "src_lib_redact", "label": ".redact()", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": ".redact()"},
            {"id": "src_lib_normalize", "label": ".normalize()", "file_type": "code",
             "source_file": "crates/c/src/lib.rs", "_origin": "ast", "norm_label": ".normalize()"},
        ]
        graph = _make_minimal_graph(nodes)
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")
        crate_dir = tmp_path / "crates" / "c" / "src"
        crate_dir.mkdir(parents=True)
        (crate_dir / "lib.rs").write_text(
            "pub struct F;\n"
            "impl F {\n"
            "    pub fn redact(&self, _s: &str) -> Self { F }\n"
            "    pub fn normalize(&self) -> String { String::new() }\n"
            "}\n"
            "#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n"
            "    fn chained() {\n"
            "        assert_eq!(F.redact(\"x\").normalize(), \"\");\n"
            "    }\n}\n",
            encoding="utf-8",
        )
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(graph, f)
        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)
        with open(out_json, encoding="utf-8") as f:
            result = json.load(f)
        proves = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert {"src_lib_redact", "src_lib_normalize"} <= proves, (
            "both chained methods asserted on must be proven"
        )


# ---------------------------------------------------------------------------
# Crediting cross-crate integration-test assertions via `use` imports (Task 0.3)
# ---------------------------------------------------------------------------

class TestCrossCrateImportResolution:
    """An integration test in crate A that `use`s crate B and asserts on a B
    symbol whose name is NOT globally unique must still produce a `proves` edge
    to B's definition — resolved via the test file's `use` imports. Before Task
    0.3 only globally-unique cross-crate names were credited, dropping most
    integration-test proofs."""

    def _graph(self):
        # `Widget` is DEFINED in two crates (b and c) -> not globally unique.
        return _make_minimal_graph([
            {"id": "src_lib_widget", "label": "Widget", "file_type": "code",
             "source_file": "crates/vox-b/src/lib.rs", "_origin": "ast", "norm_label": "widget"},
            {"id": "src_libc_widget", "label": "Widget", "file_type": "code",
             "source_file": "crates/vox-c/src/lib.rs", "_origin": "ast", "norm_label": "widget"},
        ])

    def _write_it(self, tmp_path, use_line):
        it = tmp_path / "crates" / "vox-a" / "tests"
        it.mkdir(parents=True)
        (it / "it.rs").write_text(
            f"{use_line}\n"
            "#[test]\n"
            "fn uses_widget() {\n"
            "    assert_eq!(Widget::tag(), \"b\");\n"
            "}\n",
            encoding="utf-8",
        )

    def _run(self, tmp_path):
        in_json = str(tmp_path / "in.json")
        out_json = str(tmp_path / "out.json")
        with open(in_json, "w", encoding="utf-8") as f:
            json.dump(self._graph(), f)
        run_overlay(graph_path=in_json, repo_root=str(tmp_path), out_path=out_json)
        with open(out_json, encoding="utf-8") as f:
            return json.load(f)

    def test_imported_crate_disambiguates_cross_crate_proof(self, tmp_path):
        self._write_it(tmp_path, "use vox_b::Widget;")
        result = self._run(tmp_path)
        proves = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "src_lib_widget" in proves, "use vox_b should credit B's Widget"
        assert "src_libc_widget" not in proves, "must NOT credit C's Widget (not imported)"

    def test_no_import_leaves_ambiguous_name_uncredited(self, tmp_path):
        # No `use` of vox_b/vox_c -> name stays ambiguous -> no proof (no false positive)
        self._write_it(tmp_path, "// no relevant import")
        result = self._run(tmp_path)
        proves = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "src_lib_widget" not in proves and "src_libc_widget" not in proves, (
            "ambiguous cross-crate name with no import must not be proven"
        )

    def test_symbol_imported_from_two_crates_stays_ambiguous(self, tmp_path):
        # If the SAME name is imported from both defining crates, it is genuinely
        # ambiguous -> credit neither (no false positive).
        self._write_it(tmp_path, "use vox_b::Widget;\nuse vox_c::Widget;")
        result = self._run(tmp_path)
        proves = {e["target"] for e in result["links"] if e.get("relation") == "proves"}
        assert "src_lib_widget" not in proves and "src_libc_widget" not in proves, (
            "a name imported from two crates resolves to >1 crate -> must not be proven"
        )
