//! `vox corpus` subcommand — native training data pipeline.
//!
//! Replaces the Python scripts with in-process Rust implementations:
//! - `extract` — walk .vox files, emit JSONL corpus
//! - `validate` — re-check entries, dedup, print coverage
//! - `pairs` — generate instruction→response training pairs
//! - `prompt` — auto-generate system prompt from construct reference

use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

use clap::Parser;

#[derive(Parser)]
pub enum CorpusAction {
    /// Extract training corpus from .vox files
    Extract {
        /// Directory containing .vox files (recursive)
        #[arg(required = true)]
        dir: std::path::PathBuf,
        /// Output JSONL file
        #[arg(short, long, default_value = "populi/data/validated.jsonl")]
        output: std::path::PathBuf,
    },
    /// Validate and deduplicate a corpus JSONL file
    Validate {
        /// Input JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output validated JSONL file (defaults to overwriting input)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Skip re-checking code through the compiler
        #[arg(long)]
        no_recheck: bool,
    },
    /// Generate instruction→response training pairs from validated corpus
    Pairs {
        /// Input validated JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output training JSONL file
        #[arg(short, long, default_value = "target/dogfood/train.jsonl")]
        output: std::path::PathBuf,
        /// Documentation directory to extract additional pairs from
        #[arg(long)]
        docs: Option<std::path::PathBuf>,
    },
    /// Auto-generate vox_system_prompt.txt from construct reference
    Prompt {
        /// Output file path
        #[arg(short, long, default_value = "scripts/vox_system_prompt.txt")]
        output: std::path::PathBuf,
    },
    /// Run eval metrics on training data (format, safety, parse, coverage)
    Eval {
        /// Input training JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output eval results JSON
        #[arg(short, long, default_value = "target/dogfood/eval_results.json")]
        output: std::path::PathBuf,
    },
}

pub async fn run(action: CorpusAction) -> Result<()> {
    match action {
        CorpusAction::Extract { dir, output } => run_extract(&dir, &output).await,
        CorpusAction::Validate { input, output, no_recheck } => {
            let out = output.as_deref().unwrap_or(&input);
            run_validate(&input, out, !no_recheck).await
        }
        CorpusAction::Pairs { input, output, docs } => {
            run_pairs(&input, &output, docs.as_deref()).await
        }
        CorpusAction::Prompt { output } => run_prompt(&output),
        CorpusAction::Eval { input, output } => run_eval(&input, &output).await,
    }
}

// ── Extract ──────────────────────────────────────────────────────────────

async fn run_extract(dir: &Path, output: &Path) -> Result<()> {
    let entries = crate::training::walk_vox_files(dir);

    if entries.is_empty() {
        eprintln!("{}", format!("No .vox files found in {}", dir.display()).yellow());
        return Ok(());
    }

    // ── 8.2: Incremental — load existing hashes to skip unchanged files ──
    let existing_hashes: std::collections::HashSet<String> = if output.exists() {
        let content = std::fs::read_to_string(output).unwrap_or_default();
        content.lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| v.get("ast_hash").and_then(|h| h.as_str()).map(String::from))
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let incremental_skipped = existing_hashes.len();
    if incremental_skipped > 0 {
        println!("{}", format!("  ↻ Incremental mode: {} known entries, skipping unchanged files", incremental_skipped).cyan());
    }

    // Ensure output dir exists; open in append mode for incremental
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // For a clean run (no file yet) create it; otherwise append
    if !output.exists() {
        std::fs::File::create(output)?;
    }

    let total = entries.len();

    // ── 8.1: Parallel extraction using tokio::spawn ──────────────────────
    use std::sync::{Arc, Mutex};
    let output_arc = Arc::new(Mutex::new(output.to_path_buf()));
    let existing_arc = Arc::new(existing_hashes);

    let mut handles = Vec::with_capacity(entries.len());
    for path in entries {
        let output_path = Arc::clone(&output_arc);
        let known = Arc::clone(&existing_arc);
        let handle = tokio::spawn(async move {
            match crate::pipeline::run_frontend(&path, false).await {
                Ok(result) if !result.has_errors() => {
                    // Build record first to check the hash
                    match crate::training::build_training_record(&path, &result) {
                        Ok(record) => {
                            let hash = record.get("ast_hash")
                                .and_then(|h| h.as_str())
                                .unwrap_or("")
                                .to_string();
                            // Skip if already in corpus
                            if !hash.is_empty() && known.contains(&hash) {
                                return (path, true, true); // (path, ok, skipped)
                            }
                            // Append to file under lock
                            let out = output_path.lock().unwrap();
                            if let Ok(line) = serde_json::to_string(&record) {
                                use std::io::Write;
                                if let Ok(mut f) = std::fs::OpenOptions::new()
                                    .create(true).append(true).open(&*out)
                                {
                                    let _ = writeln!(f, "{}", line);
                                    return (path, true, false);
                                }
                            }
                            (path, false, false)
                        }
                        Err(_) => (path, false, false),
                    }
                }
                _ => (path, false, false),
            }
        });
        handles.push(handle);
    }

    let mut success = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    for handle in handles {
        match handle.await {
            Ok((_, true, true)) => skipped += 1,
            Ok((_, true, false)) => success += 1,
            _ => failed += 1,
        }
    }

    println!(
        "{}",
        format!(
            "✓ Corpus extraction: {}/{} new ({} skipped, {} failed) → {}",
            success, total, skipped + incremental_skipped as u32, failed, output.display()
        )
        .green()
    );
    Ok(())
}

// ── Validate ─────────────────────────────────────────────────────────────

async fn run_validate(input: &Path, output: &Path, recheck: bool) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let content = std::fs::read_to_string(input)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let total = lines.len();
    let mut valid: Vec<serde_json::Value> = Vec::new();
    let mut rejected = 0u32;
    let mut construct_counts: HashMap<String, u32> = HashMap::new();

    for line in &lines {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => { rejected += 1; continue; }
        };

        let code = record.get("code").and_then(|v| v.as_str()).unwrap_or("");

        // Re-validate through compiler if requested
        if recheck && !code.is_empty() {
            let dummy_path = Path::new("__validate__.vox");
            match crate::pipeline::run_frontend_str(code, dummy_path, false) {
                Ok(result) if !result.has_errors() => {}
                _ => { rejected += 1; continue; }
            }
        }

        // Count constructs
        if let Some(constructs) = record.get("constructs").and_then(|v| v.as_array()) {
            for c in constructs {
                if let Some(s) = c.as_str() {
                    *construct_counts.entry(s.to_string()).or_insert(0) += 1;
                }
            }
        }

        valid.push(record);
    }

    // Dedup by ast_hash
    let mut seen: HashSet<String> = HashSet::new();
    let mut deduped: Vec<serde_json::Value> = Vec::new();
    for record in valid {
        let hash = record.get("ast_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !hash.is_empty() && seen.contains(&hash) {
            continue;
        }
        if !hash.is_empty() {
            seen.insert(hash);
        }
        deduped.push(record);
    }

    // Write output
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(output)?;
    for record in &deduped {
        writeln!(f, "{}", serde_json::to_string(record)?)?;
    }

    // Coverage report
    let taxonomy: HashSet<&str> = crate::training::TAXONOMY.iter().copied().collect();
    let covered: HashSet<&str> = construct_counts.keys()
        .map(|s| s.as_str())
        .filter(|s| taxonomy.contains(s))
        .collect();
    let uncovered: Vec<&&str> = taxonomy.iter()
        .filter(|s| !covered.contains(**s))
        .collect();
    let coverage_pct = if taxonomy.is_empty() { 0.0 }
        else { 100.0 * covered.len() as f64 / taxonomy.len() as f64 };

    println!("╔══════════════════════════════════════════════════╗");
    println!("║       Vox Training Data Validation Report       ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  Input records:     {:<28}║", total);
    println!("║  Valid (post-check):{:<28}║", deduped.len() + rejected as usize);
    println!("║  After dedup:       {:<28}║", deduped.len());
    println!("║  Rejected:          {:<28}║", rejected);
    let cov_text = format!("{:.1}% ({}/{})", coverage_pct, covered.len(), taxonomy.len());
    println!("║  Construct coverage:{:<28}║", cov_text);
    println!("╠══════════════════════════════════════════════════╣");
    if uncovered.is_empty() {
        println!("║  ✅ All constructs covered!                      ║");
    } else {
        println!("║  Missing constructs:                             ║");
        for c in uncovered.iter().take(10) {
            println!("║    - {:<43}║", c);
        }
        if uncovered.len() > 10 {
            println!("║    ... and {} more                               ║", uncovered.len() - 10);
        }
    }
    println!("╚══════════════════════════════════════════════════╝");

    Ok(())
}

// ── Pairs ────────────────────────────────────────────────────────────────

async fn run_pairs(input: &Path, output: &Path, docs_dir: Option<&Path>) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let content = std::fs::read_to_string(input)?;
    let mut all_pairs: Vec<serde_json::Value> = Vec::new();
    let mut pair_hashes: HashSet<String> = HashSet::new();

    for line in content.lines().filter(|l| !l.is_empty()) {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let code = record.get("code").and_then(|v| v.as_str()).unwrap_or("");
        let source = record.get("source").and_then(|v| v.as_str()).unwrap_or("unknown");
        let constructs: Vec<String> = record.get("constructs")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        if code.is_empty() { continue; }

        let name = crate::training::extract_name_from_source(code);

        for construct in &constructs {
            let templates = crate::training::instruction_templates(construct);
            for template in templates {
                let instruction = template.replace("{name}", &name);

                // Dedup by content hash (XXH3)
                let combined = format!("{}|||{}", instruction, code);
                let h = vox_runtime::builtins::vox_hash_fast(&combined);
                if pair_hashes.contains(&h) { continue; }
                pair_hashes.insert(h);

                let pair = serde_json::json!({
                    "prompt": instruction,
                    "response": code,
                    "instruction": instruction,
                    "output": code,
                    "category": construct,
                    "source": source,
                    "rating": 5,
                    "schema_version": crate::training::SCHEMA_VERSION,
                });
                all_pairs.push(pair);

                // Multi-turn: generate follow-up refinement pairs
                let multi = crate::training::generate_multiturn_pairs(
                    construct,
                    &name,
                    &instruction,
                    code,
                    crate::training::SCHEMA_VERSION,
                    source,
                );
                all_pairs.extend(multi);
            }
        }

        // Generate negative (broken code) examples for this record
        let neg_examples = crate::training::generate_negative_examples(code);
        for (broken_code, error_desc) in neg_examples {
            let fix_instruction = format!(
                "Fix this broken Vox code. Error: {}",
                error_desc
            );
            let fix_pair = serde_json::json!({
                "prompt": format!("{}\n\n```vox\n{}\n```", fix_instruction, broken_code),
                "response": code,
                "instruction": fix_instruction,
                "output": code,
                "category": "error_correction",
                "source": source,
                "rating": 4,
                "schema_version": crate::training::SCHEMA_VERSION,
            });
            all_pairs.push(fix_pair);
        }
    }

    // Extract documentation pairs
    if let Some(docs) = docs_dir {
        let doc_pairs = extract_doc_pairs(docs);
        println!("  Extracted {} pairs from documentation", doc_pairs.len());
        all_pairs.extend(doc_pairs);
    }

    // ── Curriculum ordering: sort by construct difficulty ──────────
    all_pairs.sort_by(|a, b| {
        let cat_a = a.get("category").and_then(|v| v.as_str()).unwrap_or("");
        let cat_b = b.get("category").and_then(|v| v.as_str()).unwrap_or("");
        let diff_a = crate::training::construct_difficulty(cat_a);
        let diff_b = crate::training::construct_difficulty(cat_b);
        diff_a.cmp(&diff_b)
    });

    // Write output
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(output)?;
    for pair in &all_pairs {
        writeln!(f, "{}", serde_json::to_string(pair)?)?;
    }

    // Stats
    let mut cats: HashMap<String, u32> = HashMap::new();
    for p in &all_pairs {
        let cat = p.get("category").and_then(|v| v.as_str()).unwrap_or("unknown");
        *cats.entry(cat.to_string()).or_insert(0) += 1;
    }
    let neg_count = cats.get("error_correction").copied().unwrap_or(0);
    println!("\n{}", format!("Generated {} training pairs ({} negative examples):", all_pairs.len(), neg_count).green());
    let mut sorted_cats: Vec<_> = cats.into_iter().collect();
    sorted_cats.sort_by(|a, b| b.1.cmp(&a.1));
    for (cat, count) in &sorted_cats {
        let diff = crate::training::construct_difficulty(cat);
        println!("  {:<20} {:>4} pairs  (difficulty: {})", cat, count, diff);
    }

    // Metadata
    let meta_path = output.parent().unwrap_or(Path::new(".")).join("metadata.json");
    let meta = serde_json::json!({
        "schema_version": crate::training::SCHEMA_VERSION,
        "total_pairs": all_pairs.len(),
        "negative_pairs": neg_count,
        "curriculum_ordered": true,
        "generated_by": "vox corpus pairs",
        "compiler_version": env!("CARGO_PKG_VERSION"),
    });
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    println!("\n✓ Wrote {} pairs to {} (curriculum-ordered)", all_pairs.len(), output.display());
    println!("✓ Metadata written to {}", meta_path.display());

    Ok(())
}

/// Recursively walk a directory for markdown files.
fn walk_md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    walk_md_recursive(dir, &mut result);
    result.sort();
    result
}

fn walk_md_recursive(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_md_recursive(&path, out);
        } else if path.extension().map_or(false, |e| e == "md") {
            out.push(path);
        }
    }
}

/// Extract instruction-response pairs from Markdown documentation files.
fn extract_doc_pairs(docs_dir: &Path) -> Vec<serde_json::Value> {
    let mut pairs = Vec::new();
    let entries = walk_md_files(docs_dir);

    for md_file in &entries {
        let content = match std::fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Find ```vox ... ``` blocks
        let mut in_vox_block = false;
        let mut code_lines: Vec<String> = Vec::new();
        let mut context_line = String::new();

        for line in content.lines() {
            if line.trim_start().starts_with("```vox") {
                in_vox_block = true;
                code_lines.clear();
                continue;
            }
            if in_vox_block && line.trim_start().starts_with("```") {
                in_vox_block = false;
                let code = code_lines.join("\n");
                if code.len() >= 20 {
                    let instruction = if context_line.is_empty() || context_line.starts_with('#') {
                        format!("Write Vox code as shown in {} documentation",
                            md_file.file_stem().unwrap_or_default().to_string_lossy())
                    } else {
                        context_line.trim_end_matches(':').to_string()
                    };
                    pairs.push(serde_json::json!({
                        "prompt": instruction,
                        "response": code,
                        "instruction": instruction,
                        "output": code,
                        "category": "documentation",
                        "source": format!("docs/{}", md_file.file_name().unwrap_or_default().to_string_lossy()),
                        "rating": 4,
                        "schema_version": crate::training::SCHEMA_VERSION,
                    }));
                }
                continue;
            }
            if in_vox_block {
                code_lines.push(line.to_string());
            } else if !line.trim().is_empty() {
                context_line = line.to_string();
            }
        }
    }

    pairs
}

// ── Prompt ───────────────────────────────────────────────────────────────

fn run_prompt(output: &Path) -> Result<()> {
    let prompt = crate::training::generate_system_prompt();

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &prompt)?;

    println!("✓ System prompt written to {}", output.display());
    println!("  {} characters, {} lines", prompt.len(), prompt.lines().count());

    Ok(())
}

// ── Eval ─────────────────────────────────────────────────────────────────

async fn run_eval(input: &Path, output: &Path) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let content = std::fs::read_to_string(input)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let total = lines.len();

    let mut format_valid = 0u32;
    let mut safety_rejected = 0u32;
    let mut parse_passed = 0u32;
    let mut construct_hits: HashSet<String> = HashSet::new();

    let safety_patterns = [
        "ignore previous instructions",
        "ignore all above",
        "disregard your instructions",
        "you are now",
        "new instructions:",
    ];

    for line in &lines {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let response = record.get("response")
            .or_else(|| record.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Format validity: has content
        if !response.trim().is_empty() {
            format_valid += 1;
        }

        // Safety: check for injection patterns
        let lower = response.to_lowercase();
        if safety_patterns.iter().any(|p| lower.contains(p)) {
            safety_rejected += 1;
        }

        // Vox parse score: try running through the compiler
        let dummy_path = Path::new("__eval__.vox");
        if let Ok(result) = crate::pipeline::run_frontend_str(response, dummy_path, false) {
            if !result.has_errors() {
                parse_passed += 1;
                // Collect construct hits
                let constructs = crate::training::extract_constructs(&result.module);
                for c in constructs {
                    construct_hits.insert(c);
                }
            }
        }
    }

    let taxonomy: HashSet<&str> = crate::training::TAXONOMY.iter().copied().collect();
    let coverage = construct_hits.iter()
        .filter(|s| taxonomy.contains(s.as_str()))
        .count();

    let results = serde_json::json!({
        "run_id": format!("eval_{}", crate::training::timestamp_string()),
        "total_samples": total,
        "format_validity": if total > 0 { format_valid as f64 / total as f64 } else { 0.0 },
        "safety_rejection_rate": if total > 0 { safety_rejected as f64 / total as f64 } else { 0.0 },
        "vox_parse_rate": if total > 0 { parse_passed as f64 / total as f64 } else { 0.0 },
        "construct_coverage": coverage,
        "construct_total": taxonomy.len(),
        "construct_coverage_pct": if taxonomy.is_empty() { 0.0 } else { 100.0 * coverage as f64 / taxonomy.len() as f64 },
        "quality_proxy": if total > 0 { parse_passed as f64 / total as f64 } else { 0.0 },
    });

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, serde_json::to_string_pretty(&results)?)?;

    println!("\n{}", "Vox Training Data Eval Report".bold().cyan());
    println!("  Total samples:      {}", total);
    println!("  Format validity:    {:.1}%", 100.0 * format_valid as f64 / total.max(1) as f64);
    println!("  Safety rejection:   {:.1}%", 100.0 * safety_rejected as f64 / total.max(1) as f64);
    println!("  Vox parse rate:     {:.1}%", 100.0 * parse_passed as f64 / total.max(1) as f64);
    println!("  Construct coverage: {}/{} ({:.1}%)",
        coverage, taxonomy.len(),
        100.0 * coverage as f64 / taxonomy.len().max(1) as f64);
    println!("\n✓ Results written to {}", output.display());

    Ok(())
}
