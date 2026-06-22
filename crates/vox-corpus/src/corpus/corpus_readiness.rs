//! B2.1 — Corpus readiness gate.
//!
//! Checks whether a corpus has enough rows AND sufficient semantic diversity
//! before any GPU training spend is authorized.

/// Minimum thresholds a corpus must meet before training may proceed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MinReadiness {
    /// Minimum number of training rows required.
    pub min_rows: usize,
    /// Minimum AST/semantic diversity score (0.0–1.0).
    pub min_ast_diversity: f64,
}

/// Report produced by [`assess_corpus_readiness`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReadinessReport {
    /// Number of rows found in the corpus.
    pub rows: usize,
    /// Computed AST diversity score (0.0–1.0).
    pub ast_diversity: f64,
    /// Whether the row count meets the minimum.
    pub rows_ok: bool,
    /// Whether the diversity score meets the minimum.
    pub diversity_ok: bool,
    /// True only when both `rows_ok` and `diversity_ok` are true.
    pub ready: bool,
}

/// Assess whether a corpus (as a slice of JSONL rows) meets the minimum
/// readiness thresholds before GPU training spend is authorized.
///
/// Diversity is measured via [`vox_eval::eval_semantic_entropy`] over all
/// `"task"`, `"output"`, `"response"`, and `"vox_code"` strings found in the
/// rows.  When none are present the score is 0.0.
pub fn assess_corpus_readiness(rows: &[serde_json::Value], min: &MinReadiness) -> ReadinessReport {
    let rows_ok = rows.len() >= min.min_rows;

    // Collect candidate strings for diversity analysis.
    let mut strings: Vec<String> = Vec::with_capacity(rows.len());
    for row in rows {
        for field in &["task", "output", "response", "vox_code"] {
            if let Some(s) = row.get(field).and_then(|v| v.as_str()) {
                strings.push(s.to_owned());
                break; // one string per row is enough for diversity
            }
        }
    }

    let entropy = vox_eval::eval_semantic_entropy(&strings, min.min_ast_diversity);
    let ast_diversity = entropy.ast_diversity;
    let diversity_ok = ast_diversity >= min.min_ast_diversity;
    let ready = rows_ok && diversity_ok;

    ReadinessReport {
        rows: rows.len(),
        ast_diversity,
        rows_ok,
        diversity_ok,
        ready,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_when_above_thresholds() {
        // 200 rows where enough rows have structurally distinct task strings.
        //
        // eval_semantic_entropy hashes a "pseudo-AST" formed by stripping string
        // literals, normalising digits to "0", and collapsing whitespace.  To
        // guarantee diversity >= 0.1 with min_rows=100 we supply 20 unique
        // structural templates (keywords-only, no digit suffixes) each repeated
        // 10 times → 20/200 = 0.10 ast_diversity, which equals the threshold.
        // We use >=  operator so 0.10 passes.
        //
        // The templates differ in keyword structure so the pseudo-AST hash varies.
        let templates: Vec<&str> = vec![
            "fn alpha() -> Result { ok() }",
            "if ready { process() } else { skip() }",
            "match mode { A => op_a() _ => op_b() }",
            "for item in batch { handle(item) }",
            "let val = fetch().await?; val",
            "while running { tick() sleep() }",
            "pub struct Config { field: Type }",
            "impl Trait for Foo { fn method(&self) -> R { body() } }",
            "use crate::module::Thing;",
            "pub enum Kind { Alpha Beta Gamma }",
            "async fn serve(ctx: Ctx) -> Response { ctx.run().await }",
            "return Err(anyhow::anyhow!(msg))",
            "let _ = spawn(async move { worker().await });",
            "tracing::info!(target: msg)",
            "assert_eq!(left right msg)",
            "vec![alpha beta gamma delta]",
            "Box::new(handler(config))",
            "Arc::clone(&shared_state)",
            "serde_json::from_str::<T>(raw)?",
            "tokio::select! { a = fut_a => handle_a(a) b = fut_b => handle_b(b) }",
        ];
        let rows: Vec<_> = (0..200)
            .map(|i| {
                serde_json::json!({
                    "task": templates[i % templates.len()],
                    "lane": "test"
                })
            })
            .collect();
        let r = assess_corpus_readiness(
            &rows,
            &MinReadiness {
                min_rows: 100,
                min_ast_diversity: 0.1,
            },
        );
        assert!(r.rows_ok, "200 rows should meet min_rows=100");
        assert!(
            r.ready,
            "should be ready with sufficient rows and diversity (got ast_diversity={})",
            r.ast_diversity
        );
    }

    #[test]
    fn not_ready_when_too_few_rows() {
        let rows: Vec<_> = (0..5)
            .map(|i| serde_json::json!({"task": format!("t{i}")}))
            .collect();
        let r = assess_corpus_readiness(
            &rows,
            &MinReadiness {
                min_rows: 100,
                min_ast_diversity: 0.0,
            },
        );
        assert!(!r.rows_ok, "5 rows should not meet min_rows=100");
        assert!(!r.ready, "not ready with too few rows");
    }

    #[test]
    fn empty_corpus_is_not_ready() {
        let r = assess_corpus_readiness(
            &[],
            &MinReadiness {
                min_rows: 1,
                min_ast_diversity: 0.0,
            },
        );
        assert!(!r.ready);
        assert_eq!(r.rows, 0);
        assert_eq!(r.ast_diversity, 0.0);
    }

    #[test]
    fn diversity_gate_can_block_even_with_enough_rows() {
        // All rows identical → near-zero diversity.
        let rows: Vec<_> = (0..200)
            .map(|_| serde_json::json!({"task": "same task every time"}))
            .collect();
        let r = assess_corpus_readiness(
            &rows,
            &MinReadiness {
                min_rows: 100,
                min_ast_diversity: 0.9,
            },
        );
        assert!(r.rows_ok, "200 rows meet min_rows");
        // Identical strings → diversity should be very low.
        assert!(
            !r.diversity_ok,
            "identical rows should fail high diversity threshold"
        );
        assert!(!r.ready);
    }
}
