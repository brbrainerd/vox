//! Compute Rust spoke eval metrics from a batch of model outputs in the corpus.
use std::path::Path;

/// Fraction of `outputs` for which `verifier` returns true. Empty → 0.0.
pub fn pass_rate(outputs: &[String], verifier: impl Fn(&[String]) -> Vec<bool>) -> f64 {
    if outputs.is_empty() {
        return 0.0;
    }
    let flags = verifier(outputs);
    let ok = flags.iter().filter(|&&f| f).count();
    ok as f64 / outputs.len() as f64
}

fn extract_rust_from_markdown(md: &str) -> String {
    if let Some(start) = md.find("```rust\n") {
        let content = &md[start + 8..];
        if let Some(end) = content.find("\n```") {
            return content[..end].to_string();
        }
    }
    md.to_string()
}

/// Compute compile/clippy clean rates for rust_authoring samples in `input_jsonl`.
///
/// Returns `Ok(None)` when the corpus contains **no** rust_authoring rows (the
/// metric is not applicable — the caller must omit it rather than report 0.0,
/// which a blocking rust gate would misread as "model writes no compiling Rust").
/// Returns `Ok(Some((compile_rate, clippy_rate)))` otherwise. Cargo is only
/// spawned when rust rows are present.
pub fn compute_rust_spoke_metrics(
    workspace_root: &Path,
    input_jsonl: &Path,
) -> anyhow::Result<Option<(f64, f64)>> {
    let content = std::fs::read_to_string(input_jsonl)?;
    let mut snippets = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            let category = val.get("category").and_then(|c| c.as_str()).unwrap_or("");
            let lane = val.get("lane").and_then(|l| l.as_str()).unwrap_or("");
            if (category == "rust_authoring" || lane == "vox_rust_authoring")
                && let Some(response) = val
                    .get("response")
                    .or_else(|| val.get("output"))
                    .and_then(|r| r.as_str())
            {
                let cleaned = extract_rust_from_markdown(response);
                if !cleaned.trim().is_empty() {
                    snippets.push(cleaned);
                }
            }
        }
    }

    if snippets.is_empty() {
        // Not applicable — no rust rows. Distinct from "0% compiled".
        return Ok(None);
    }

    // Limit the verification set size to prevent excessively long eval runs
    snippets.truncate(100);

    let compile_verifier =
        |s: &[String]| crate::corpus::rust_authoring::compile_batch_in_workspace(workspace_root, s);
    let clippy_verifier =
        |s: &[String]| crate::corpus::rust_authoring::clippy_batch_in_workspace(workspace_root, s);

    let compile_rate = pass_rate(&snippets, compile_verifier);
    let clippy_rate = pass_rate(&snippets, clippy_verifier);

    Ok(Some((compile_rate, clippy_rate)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_is_fraction_passing() {
        let outs = vec!["a".to_string(), "b".to_string()];
        let r = pass_rate(&outs, |s| s.iter().map(|x| x == "a").collect());
        assert!((r - 0.5).abs() < 1e-9);
    }

    #[test]
    fn empty_is_zero() {
        let r = pass_rate(&[], |_| vec![]);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn test_extract_markdown() {
        let md = "```rust\nfn main() {}\n```";
        assert_eq!(extract_rust_from_markdown(md), "fn main() {}");
    }
}
