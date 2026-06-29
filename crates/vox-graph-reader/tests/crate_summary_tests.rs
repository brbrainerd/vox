use serde_json::json;
use std::collections::HashMap;
use vox_graph_reader::crate_model::build_crate_summary;

fn compile_times(entries: &[(&str, f64)]) -> HashMap<String, f64> {
    entries.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn summary_is_sorted_and_carries_compile_times() {
    // c -> b -> a  (a is the leaf everything depends on)
    let graph = json!({ "crates": { "c": ["b"], "b": ["a"], "a": [] } });
    let times = compile_times(&[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
    let s = build_crate_summary(&graph, &times);

    assert_eq!(s["schema_version"], 1);
    assert_eq!(s["has_compile_times"], true);
    assert_eq!(s["crates_without_compile_times"], 0);

    let crates = s["crates"].as_array().unwrap();
    assert_eq!(crates[0]["crate"], "a"); // sorted alphabetically
    assert_eq!(crates[1]["crate"], "b");
    assert_eq!(crates[2]["crate"], "c");

    // a's blast_s = compile_s(a)+dependents(b,c) = 1+2+3 = 6; dependents=2; fan_in=1 (b depends on a)
    assert_eq!(crates[0]["blast_s"], 6.0);
    assert_eq!(crates[0]["dependents"], 2);
    assert_eq!(crates[0]["fan_in"], 1);
    // c is a root: no dependents, blast_s = own compile_s
    assert_eq!(crates[2]["blast_s"], 3.0);
    assert_eq!(crates[2]["dependents"], 0);
}

#[test]
fn summary_flags_missing_compile_times() {
    let graph = json!({ "crates": { "a": [], "b": ["a"] } });
    let times = compile_times(&[("a", 1.0)]); // b missing
    let s = build_crate_summary(&graph, &times);
    assert_eq!(s["crates_without_compile_times"], 1);
    assert_eq!(s["has_compile_times"], true); // any time present → usable
}

#[test]
fn summary_empty_times_sets_has_compile_times_false() {
    let graph = json!({ "crates": { "a": [], "b": ["a"] } });
    let s = build_crate_summary(&graph, &HashMap::new());
    assert_eq!(s["has_compile_times"], false);
    assert_eq!(s["crates_without_compile_times"], 2);
}

#[test]
fn summary_round_trips_exactly() {
    // INVARIANT for the parity gate: rebuilding from the summary's own compile_s
    // must reproduce identical derived fields (compile_s stored at input precision).
    let graph = json!({ "crates": { "x": ["y", "z"], "y": ["z"], "z": [] } });
    let times = compile_times(&[("x", 1.0), ("y", 2.0), ("z", 3.0)]);
    let first = build_crate_summary(&graph, &times);

    // Extract compile_s back out (as the parity gate will) and rebuild.
    let mut reextracted = HashMap::new();
    for c in first["crates"].as_array().unwrap() {
        reextracted.insert(
            c["crate"].as_str().unwrap().to_string(),
            c["compile_s"].as_f64().unwrap(),
        );
    }
    let second = build_crate_summary(&graph, &reextracted);
    assert_eq!(first, second);
}
