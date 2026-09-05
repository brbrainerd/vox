//! Guard: the `merge_group` self-hosted fan-out must fit the runner ceiling.
//!
//! The merge gate runs on a single box capped at [`DEFAULT_MAX_RUNNERS`] ephemeral
//! runners. If more self-hosted jobs than that fire on a `merge_group` event, the
//! gate serializes into multiple waves — the exact regression this guards. When a
//! new self-hosted job is added without tiering it off `merge_group`, the
//! `#[test]` below fails, forcing the author to add an exclusion (or raise the
//! ceiling deliberately).
//!
//! Runs as a unit test (under `cargo nextest`, which the gate runs), so it gates
//! pre-merge with no extra CLI wiring.

use std::collections::BTreeMap;

/// Heuristic: does a job's `if:` evaluate truthy on a `merge_group` event?
/// Covers the expression shapes actually used in `ci.yml` (one rule per shape);
/// the conservative default (`true`) means a NEW, unrecognized `if:` on a
/// self-hosted job counts toward the fan-out until it is classified.
fn fires_on_merge_group(if_expr: Option<&str>) -> bool {
    let Some(e) = if_expr else {
        return true; // no `if:` → always runs
    };
    if e.contains("== 'merge_group'") {
        return true; // positive opt-in (possibly compound, e.g. push || merge_group)
    }
    if e.contains("!= 'merge_group'") {
        return false; // explicit exclusion added when tiering a job off the gate
    }
    if e.contains("== 'pull_request'") {
        return false; // PR-only job — never on merge_group
    }
    if e.contains("== 'push'") {
        return false; // push/label opt-in (post-merge), not merge_group
    }
    // `full == 'true'` / `rust_changed` / no-if are all truthy on merge_group
    // (setup forces `full=true`); anything unrecognized is counted conservatively.
    true
}

/// Count self-hosted jobs that fire on `merge_group`, bucketed by their exact
/// (sorted) `runs-on` label set. Non-self-hosted jobs are ignored. A matrix job
/// counts once (this measures distinct queued jobs, not expanded legs).
fn merge_group_self_hosted_fanout(workflow_yaml: &str) -> BTreeMap<String, usize> {
    let mut buckets = BTreeMap::new();
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(workflow_yaml) else {
        return buckets;
    };
    let Some(jobs) = doc.get("jobs").and_then(|j| j.as_mapping()) else {
        return buckets;
    };
    for (_name, job) in jobs {
        let Some(runs_on) = job.get("runs-on") else {
            continue;
        };
        let labels: Vec<String> = match runs_on {
            serde_yaml::Value::Sequence(seq) => seq
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            serde_yaml::Value::String(s) => vec![s.clone()],
            _ => continue,
        };
        if !labels.iter().any(|l| l == "self-hosted") {
            continue;
        }
        if !fires_on_merge_group(job.get("if").and_then(|v| v.as_str())) {
            continue;
        }
        let mut sorted = labels;
        sorted.sort();
        *buckets.entry(sorted.join(",")).or_insert(0) += 1;
    }
    buckets
}

#[cfg(test)]
mod tests {
    use super::super::runner_scale::DEFAULT_MAX_RUNNERS;
    use super::*;

    #[test]
    fn fires_on_merge_group_classifies_current_patterns() {
        assert!(fires_on_merge_group(None)); // setup/tests/audits: always run
        assert!(fires_on_merge_group(Some(
            "${{ needs.setup.outputs.full == 'true' || needs.setup.outputs.affects_compiler == 'true' }}"
        ))); // compiler-gates: full forced on merge_group
        assert!(!fires_on_merge_group(Some(
            "github.event_name != 'merge_group' && (needs.setup.outputs.full == 'true')"
        ))); // tiered off (Group A)
        assert!(!fires_on_merge_group(Some(
            "github.event_name == 'pull_request'"
        ))); // PR-only
        assert!(!fires_on_merge_group(Some(
            "(github.event_name == 'push' && github.ref == 'refs/heads/main') || contains(github.event.pull_request.labels.*.name, 'full-ci')"
        ))); // post-merge (Group B)
        assert!(fires_on_merge_group(Some(
            "(github.event_name == 'merge_group' && github.event.merge_group.base_ref == 'refs/heads/main') || contains(github.event.pull_request.labels.*.name, 'full-ci')"
        ))); // merge-queue opt-in (positive mention wins over the label clause)
        assert!(fires_on_merge_group(Some(
            "github.event_name == 'push' || github.event_name == 'merge_group'"
        ))); // compound push||merge_group still fires on merge_group
    }

    #[test]
    fn fanout_buckets_by_label_set() {
        let yaml = "\
jobs:\n  a:\n    runs-on: [self-hosted, linux, x64]\n  b:\n    runs-on: [self-hosted, linux, x64]\n  c:\n    runs-on: [self-hosted, linux, x64, browser]\n  d:\n    runs-on: ubuntu-latest\n  e:\n    runs-on: [self-hosted, linux, x64]\n    if: github.event_name != 'merge_group' && (x)\n";
        let b = merge_group_self_hosted_fanout(yaml);
        assert_eq!(b.get("linux,self-hosted,x64").copied(), Some(2)); // a,b (e excluded, d hosted)
        assert_eq!(b.get("browser,linux,self-hosted,x64").copied(), Some(1)); // c
    }

    /// The required-needs lane must fit the runner ceiling in one wave.
    #[test]
    fn ci_yml_merge_group_required_lane_fits_runner_ceiling() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.github/workflows/ci.yml"
        );
        let yaml = std::fs::read_to_string(path).expect("read ci.yml");
        assert!(
            serde_yaml::from_str::<serde_yaml::Value>(&yaml).is_ok(),
            "ci.yml failed to parse as YAML — the guard would pass vacuously"
        );
        let buckets = merge_group_self_hosted_fanout(&yaml);
        let general = buckets
            .get("linux,self-hosted")
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "expected `linux,self-hosted` bucket missing — label set renamed? \
                     Buckets: {buckets:?}"
                )
            });
        assert!(
            general > 0,
            "merge_group `linux,self-hosted` fan-out is zero — the guard is measuring nothing. \
             Buckets: {buckets:?}"
        );
        assert!(
            general <= DEFAULT_MAX_RUNNERS as usize,
            "merge_group `linux,self-hosted` fan-out {general} exceeds runner ceiling {} — \
             tier a job off merge_group (add `github.event_name != 'merge_group'`) or raise the ceiling. \
             Buckets: {buckets:?}",
            DEFAULT_MAX_RUNNERS
        );
    }
}
