//! Seeded train/eval split by tool identity (B1.4).
//!
//! Splits a harness corpus by unique tool-identity keys so no tool appears
//! in both train and eval sets (prevents data leakage). Split is fully
//! deterministic given the same seed.
//!
//! Tool identity is extracted from the row payload:
//! - `vox_tool_selection` rows: use `chosen_tool`
//! - `vox_argument_generation` rows: use `tool_name`
//! - Unknown lanes: use lane name itself as identity key

use crate::corpus::harness_union::HarnessRow;

pub struct SplitManifest {
    pub train_tools: Vec<String>,
    pub eval_tools: Vec<String>,
    pub seed: u64,
    pub eval_frac: f64,
}

/// Extract the tool identity key from a HarnessRow payload.
fn tool_key(row: &HarnessRow) -> String {
    match row.lane.as_str() {
        "vox_tool_selection" => row
            .payload
            .get("chosen_tool")
            .and_then(|v| v.as_str())
            .unwrap_or(&row.lane)
            .to_string(),
        "vox_argument_generation" => row
            .payload
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&row.lane)
            .to_string(),
        _ => row.lane.clone(),
    }
}

/// XorShift64 step — no external deps, matches the rng pattern in synthetic_gen.
fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Deterministic Fisher-Yates shuffle of `indices` using an xorshift RNG.
fn shuffle(indices: &mut Vec<String>, seed: u64) {
    let mut state = if seed == 0 {
        0xdeadbeef_cafebabe
    } else {
        seed
    };
    for i in (1..indices.len()).rev() {
        let j = xorshift(&mut state) as usize % (i + 1);
        indices.swap(i, j);
    }
}

/// Split `rows` into (train, eval) sets by tool identity.
///
/// Algorithm:
/// 1. Collect unique tool identity keys from all rows.
/// 2. Shuffle the keys deterministically using `seed`.
/// 3. Assign the first `ceil(eval_frac * n_keys)` keys to eval, rest to train.
/// 4. Partition rows by which key set their tool identity belongs to.
/// 5. Emit a SplitManifest recording the partition.
///
/// HarnessRow is not Clone/Copy, so we re-construct rows from slices.
pub fn split_surface(
    seed: u64,
    eval_frac: f64,
    rows: &[HarnessRow],
) -> (Vec<HarnessRow>, Vec<HarnessRow>, SplitManifest) {
    use std::collections::BTreeSet;

    // 1. Collect unique keys in stable order (BTreeSet for reproducibility)
    let unique_keys: Vec<String> = rows
        .iter()
        .map(|r| tool_key(r))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // 2. Shuffle keys deterministically
    let mut shuffled = unique_keys.clone();
    shuffle(&mut shuffled, seed);

    // 3. Assign eval_frac of unique keys to eval
    let n_eval = ((shuffled.len() as f64 * eval_frac).ceil() as usize).min(shuffled.len());
    let eval_keys: BTreeSet<String> = shuffled[..n_eval].iter().cloned().collect();
    let train_keys: BTreeSet<String> = shuffled[n_eval..].iter().cloned().collect();

    // 4. Partition rows
    let mut train = Vec::new();
    let mut eval_rows = Vec::new();

    for row in rows {
        let key = tool_key(row);
        let target = if eval_keys.contains(&key) {
            &mut eval_rows
        } else {
            &mut train
        };
        target.push(HarnessRow {
            task: row.task.clone(),
            lane: row.lane.clone(),
            payload: row.payload.clone(),
        });
    }

    // 5. Emit manifest
    let manifest = SplitManifest {
        train_tools: train_keys.into_iter().collect(),
        eval_tools: eval_keys.into_iter().collect(),
        seed,
        eval_frac,
    };

    (train, eval_rows, manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::harness_union::generate_harness_rows;

    #[test]
    fn split_is_by_tool_identity() {
        let rows = generate_harness_rows(100);
        let (train, eval, _) = split_surface(42, 0.2, &rows);
        let train_lanes: std::collections::HashSet<_> =
            train.iter().map(|r| &r.lane).collect();
        let eval_lanes: std::collections::HashSet<_> =
            eval.iter().map(|r| &r.lane).collect();
        // Both sets must exist (not trivially empty)
        assert!(!train.is_empty() && !eval.is_empty());
        let _ = (train_lanes, eval_lanes); // structural check; identity guard below
    }

    #[test]
    fn split_is_deterministic() {
        let rows = generate_harness_rows(50);
        let (a, _, _) = split_surface(42, 0.2, &rows);
        let (b, _, _) = split_surface(42, 0.2, &rows);
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn manifest_is_non_empty() {
        let rows = generate_harness_rows(50);
        let (_, _, m) = split_surface(42, 0.2, &rows);
        assert!(!m.train_tools.is_empty() || !m.eval_tools.is_empty());
    }

    #[test]
    fn no_tool_appears_in_both_sets() {
        let rows = generate_harness_rows(100);
        let (train, eval, _) = split_surface(42, 0.2, &rows);
        let train_keys: std::collections::HashSet<String> =
            train.iter().map(|r| tool_key(r)).collect();
        let eval_keys: std::collections::HashSet<String> =
            eval.iter().map(|r| tool_key(r)).collect();
        let overlap: Vec<_> = train_keys.intersection(&eval_keys).collect();
        assert!(
            overlap.is_empty(),
            "tool leakage: tools in both sets: {:?}",
            overlap
        );
    }
}
