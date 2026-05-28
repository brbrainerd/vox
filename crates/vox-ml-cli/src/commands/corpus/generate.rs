use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use vox_bounded_fs::{read_utf8_path_capped, read_utf8_path_capped_async};

pub(super) async fn run_generate(
    output: std::path::PathBuf,
    force_regen: bool,
    dry_run: bool,
) -> Result<()> {
    let workspace_root = vox_corpus::training::contract::find_workspace_root();
    let mut current_fp = String::new();
    if let Some(ref root) = workspace_root {
        current_fp = vox_corpus::corpus::preflight::compute_corpus_fingerprint(root);
    }

    let is_fresh = if !force_regen && !current_fp.is_empty() {
        if let Ok(db) = vox_db::VoxDb::connect_default().await {
            db.is_corpus_fresh(&current_fp).await.unwrap_or(false)
        } else if let Some(ref root) = workspace_root {
            let fp_file = vox_corpus::corpus::preflight::fingerprint_cache_path(root);
            vox_corpus::corpus::preflight::corpus_is_fresh(root, &fp_file)
        } else {
            false
        }
    } else {
        false
    };

    if is_fresh && !force_regen {
        eprintln!(
            "  {} Corpus is fresh (fingerprint: {}). Use --force-regen to rebuild.",
            "✓".green(),
            current_fp
        );
        return Ok(());
    }

    if dry_run {
        eprintln!("  {} Dry-run mode: would regenerate corpus", "ℹ".blue());
        return Ok(());
    }

    // Cleanup stale targets before regeneration
    if let Some(ref root) = workspace_root {
        vox_corpus::corpus::preflight::clean_corpus_targets(root)?;
    }

    let cfg = vox_corpus::synthetic_gen::SyntheticGenConfig::default();
    let count = vox_corpus::synthetic_gen::generate_all(&cfg, &output)?;
    eprintln!("  {} Synthesized {} pairs", "✓".green(), count);

    // Record snapshot in Arca and local file
    if !current_fp.is_empty() {
        if let Ok(db) = vox_db::VoxDb::connect_default().await {
            let _ = db
                .record_corpus_snapshot(&current_fp, env!("CARGO_PKG_VERSION"), count as i64, None)
                .await;
        }
        if let Some(ref root) = workspace_root {
            let fp_file = vox_corpus::corpus::preflight::fingerprint_cache_path(root);
            let _ = vox_corpus::corpus::preflight::write_fingerprint_snapshot(root, &fp_file);
        }
    }

    Ok(())
}

pub(super) async fn run_extract(dir: &Path, output: &Path) -> Result<()> {
    let entries = crate::training::walk_vox_files(dir);

    if entries.is_empty() {
        eprintln!(
            "{}",
            format!("No .vox files found in {}", dir.display()).yellow()
        );
        return Ok(());
    }

    // ── 8.2: Incremental — load existing hashes to skip unchanged files ──
    let output_owned = output.to_path_buf();
    let existing_hashes: std::collections::HashSet<String> = if output_owned.exists() {
        let p = output_owned.clone();
        let content = match tokio::task::spawn_blocking(move || read_utf8_path_capped(&p)).await {
            Ok(Ok(s)) => s,
            Ok(Err(_)) | Err(_) => String::new(),
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| v.get("ast_hash").and_then(|h| h.as_str()).map(String::from))
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let incremental_skipped = existing_hashes.len();
    if incremental_skipped > 0 {
        println!(
            "{}",
            format!(
                "  ↻ Incremental mode: {} known entries, skipping unchanged files",
                incremental_skipped
            )
            .cyan()
        );
    }

    // Ensure output dir exists; open in append mode for incremental (blocking I/O off async runtime)
    if let Some(parent) = output_owned.parent() {
        let p = parent.to_path_buf();
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(&p))
            .await
            .map_err(|e| anyhow::anyhow!("join create_dir_all: {e}"))?
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    // For a clean run (no file yet) create it; otherwise append
    if !output_owned.exists() {
        let p = output_owned.clone();
        tokio::task::spawn_blocking(move || std::fs::File::create(&p))
            .await
            .map_err(|e| anyhow::anyhow!("join File::create: {e}"))?
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    let total = entries.len();

    // ── 8.1: Parallel extraction using tokio::spawn ──────────────────────
    use std::sync::Arc;
    let existing_arc = Arc::new(existing_hashes);

    let mut handles = Vec::with_capacity(entries.len());
    for path in entries {
        let known = Arc::clone(&existing_arc);
        let handle = tokio::spawn(async move {
            match crate::training::core::run_frontend(&path).await {
                Ok(result) if !crate::training::core::has_errors(&result) => {
                    match crate::training::build_training_record(&path, &result) {
                        Ok(record) => {
                            let hash = record
                                .get("ast_hash")
                                .and_then(|h| h.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !hash.is_empty() && known.contains(&hash) {
                                return (path, Some(record), true, true);
                            }
                            (path, Some(record), true, false)
                        }
                        Err(_) => (path, None, false, false),
                    }
                }
                _ => (path, None, false, false),
            }
        });
        handles.push(handle);
    }

    let mut success_count = 0u32;
    let mut failed_count = 0u32;
    let mut skipped_count = 0u32;
    let mut new_records = Vec::new();

    for handle in handles {
        match handle.await {
            Ok((_, Some(_), true, true)) => skipped_count += 1,
            Ok((_, Some(record), true, false)) => {
                success_count += 1;
                new_records.push(record);
            }
            _ => failed_count += 1,
        }
    }

    if !new_records.is_empty() {
        let output_for_write = output.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<()> {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&output_for_write)?;
            for record in new_records {
                let line = serde_json::to_string(&record)?;
                writeln!(f, "{}", line)?;
            }
            Ok(())
        })
        .await??;
    }

    println!(
        "{}",
        format!(
            "✓ Corpus extraction: {}/{} new ({} skipped, {} failed) → {}",
            success_count,
            total,
            skipped_count + incremental_skipped as u32,
            failed_count,
            output.display()
        )
        .green()
    );
    Ok(())
}

pub(super) async fn run_pairs(
    input: &Path,
    output: &Path,
    docs_dirs: &[std::path::PathBuf],
) -> Result<()> {
    if tokio::fs::metadata(input).await.is_err() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let content = read_utf8_path_capped_async(input).await?;
    let mut all_pairs: Vec<serde_json::Value> = Vec::new();
    let mut pair_hashes: HashSet<String> = HashSet::new();

    for line in content.lines().filter(|l| !l.is_empty()) {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let code = record.get("code").and_then(|v| v.as_str()).unwrap_or("");
        let source = record
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let constructs: Vec<String> = record
            .get("constructs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if code.is_empty() {
            continue;
        }

        let name = crate::training::extract_name_from_source(code);

        for construct in &constructs {
            let templates = crate::training::instruction_templates(construct);
            for template in templates {
                let instruction = template.replace("{name}", &name);

                // Dedup by content hash (XXH3)
                let combined = format!("{}|||{}", instruction, code);
                let h = vox_actor_runtime::builtins::vox_hash_fast(&combined);
                if pair_hashes.contains(&h) {
                    continue;
                }
                pair_hashes.insert(h);

                let pair = serde_json::json!({
                    "prompt": instruction,
                    "response": code,
                    "messages": [
                        { "role": "user", "content": instruction },
                        { "role": "assistant", "content": code }
                    ],
                    "instruction": instruction,
                    "output": code,
                    "category": construct,
                    "difficulty": crate::training::construct_difficulty(construct),
                    "source": source,
                    "rating": 5,
                    "schema_version": crate::training::SCHEMA_VERSION,
                    "lane": "vox_codegen",
                    "response_mode": "code_only",
                    "task_family": "vox_codegen",
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
            let fix_instruction = format!("Fix this broken Vox code. Error: {}", error_desc);
            let fix_pair = serde_json::json!({
                "prompt": format!("{}\n\n```vox\n{}\n```", fix_instruction, broken_code),
                "response": code,
                "messages": [
                    { "role": "user", "content": format!("{}\n\n```vox\n{}\n```", fix_instruction, broken_code) },
                    { "role": "assistant", "content": code }
                ],
                "instruction": fix_instruction,
                "output": code,
                "category": "error_correction",
                "difficulty": crate::training::construct_difficulty("error_correction"),
                "source": source,
                "rating": 4,
                "schema_version": crate::training::SCHEMA_VERSION,
                "lane": "vox_codegen",
                "response_mode": "code_only",
                "task_family": "error_correction",
            });
            all_pairs.push(fix_pair);
        }
    }

    for docs in docs_dirs {
        let docs_for_task = docs.clone();
        let docs_label = docs_for_task.display().to_string();
        let doc_pairs = tokio::task::spawn_blocking(move || extract_doc_pairs(&docs_for_task))
            .await
            .map_err(|e| anyhow::anyhow!("extract_doc_pairs join: {e}"))?;
        println!(
            "  Extracted {} pairs from documentation ({})",
            doc_pairs.len(),
            docs_label
        );
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

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut body = String::new();
    for pair in &all_pairs {
        body.push_str(&serde_json::to_string(pair)?);
        body.push('\n');
    }
    tokio::fs::write(output, body).await?;

    // Stats
    let mut cats: HashMap<String, u32> = HashMap::new();
    for p in &all_pairs {
        let cat = p
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        *cats.entry(cat.to_string()).or_insert(0) += 1;
    }
    let neg_count = cats.get("error_correction").copied().unwrap_or(0);
    println!(
        "\n{}",
        format!(
            "Generated {} training pairs ({} negative examples):",
            all_pairs.len(),
            neg_count
        )
        .green()
    );
    let mut sorted_cats: Vec<_> = cats.into_iter().collect();
    sorted_cats.sort_by_key(|t| std::cmp::Reverse(t.1));
    for (cat, count) in &sorted_cats {
        let diff = crate::training::construct_difficulty(cat);
        println!("  {:<20} {:>4} pairs  (difficulty: {})", cat, count, diff);
    }

    // Metadata
    let meta_path = output
        .parent()
        .unwrap_or(Path::new("."))
        .join("metadata.json");
    let meta = serde_json::json!({
        "schema_version": crate::training::SCHEMA_VERSION,
        "total_pairs": all_pairs.len(),
        "negative_pairs": neg_count,
        "curriculum_ordered": true,
        "generated_by": "vox corpus pairs",
        "compiler_version": env!("CARGO_PKG_VERSION"),
    });
    tokio::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?).await?;

    println!(
        "\n✓ Wrote {} pairs to {} (curriculum-ordered)",
        all_pairs.len(),
        output.display()
    );
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
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Extract instruction-response pairs from Markdown documentation files.
fn extract_doc_pairs(docs_dir: &Path) -> Vec<serde_json::Value> {
    let mut pairs = Vec::new();
    let entries = walk_md_files(docs_dir);

    for md_file in &entries {
        let content = match read_utf8_path_capped(md_file) {
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
                        format!(
                            "Write Vox code as shown in {} documentation",
                            md_file.file_stem().unwrap_or_default().to_string_lossy()
                        )
                    } else {
                        context_line.trim_end_matches(':').to_string()
                    };
                    pairs.push(serde_json::json!({
                        "prompt": instruction,
                        "response": code,
                        "messages": [
                            { "role": "user", "content": instruction.clone() },
                            { "role": "assistant", "content": code.clone() }
                        ],
                        "instruction": instruction,
                        "output": code,
                        "category": "documentation",
                        "difficulty": crate::training::construct_difficulty("documentation"),
                        "source": format!("docs/{}", md_file.file_name().unwrap_or_default().to_string_lossy()),
                        "rating": 4,
                        "schema_version": crate::training::SCHEMA_VERSION,
                        "lane": "vox_codegen",
                        "response_mode": "code_only",
                        "task_family": "docs_code",
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

pub(super) async fn run_prompt(output: &Path) -> Result<()> {
    let prompt = crate::training::generate_system_prompt();

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(output, &prompt).await?;

    println!("✓ System prompt written to {}", output.display());
    println!(
        "  {} characters, {} lines",
        prompt.len(),
        prompt.lines().count()
    );

    Ok(())
}

pub async fn run_heal_to_dpo(input: Option<std::path::PathBuf>, output: &Path) -> Result<()> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let input_path =
        input.unwrap_or_else(|| home.join(".vox").join("corpus").join("heal_pairs.jsonl"));

    if !input_path.exists() {
        eprintln!("No heal_pairs.jsonl found at {}", input_path.display());
        return Ok(());
    }

    let content = vox_bounded_fs::read_utf8_path_capped_async(&input_path).await?;
    let mut pairs = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            let attempts = value.get("attempts").and_then(|v| v.as_u64()).unwrap_or(0);
            if attempts == 1 {
                let prompt = format!(
                    "{}\n\nCompiler Diagnostics:\n{}",
                    value
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    value
                        .get("diagnostics")
                        .and_then(|v| v.as_array())
                        .map(|a| a
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"))
                        .unwrap_or_default()
                );

                let pair = serde_json::json!({
                    "prompt": prompt,
                    "chosen": value.get("repaired_source"),
                    "rejected": value.get("failed_source"),
                    "category": "vox_heal_dpo",
                    "attempts": attempts,
                });
                pairs.push(pair);
            }
        }
    }

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut body = String::new();
    for pair in &pairs {
        body.push_str(&serde_json::to_string(&pair)?);
        body.push('\n');
    }
    tokio::fs::write(output, body).await?;

    println!(
        "✓ Extracted {} DPO pairs from heal logs -> {}",
        pairs.len(),
        output.display()
    );
    Ok(())
}

pub(super) async fn run_mutate(source_dir: &Path, count: usize, output: &Path) -> Result<()> {
    let entries = crate::training::walk_vox_files(source_dir);
    if entries.is_empty() {
        eprintln!("No .vox files found in {}", source_dir.display());
        return Ok(());
    }

    let mut generated = 0;
    let mut out_buffer = String::new();

    while generated < count {
        let mut progress = false;
        for path in &entries {
            if generated >= count {
                break;
            }

            if let Ok(result) = crate::training::core::run_frontend(path).await
                && !crate::training::core::has_errors(&result)
            {
                let mutations =
                    vox_corpus::ast_mutator::generate_mutations(&result.source, &result.module);
                if mutations.is_empty() {
                    continue;
                }

                let mutated_source =
                    vox_corpus::ast_mutator::apply_mutations(&result.source, mutations);

                // Round-trip verification
                let mutated_source_clone = mutated_source.clone();
                let verification_res = tokio::task::spawn_blocking(move || {
                    vox_compiler::pipeline::run_frontend_str(&mutated_source_clone, "<mutated>")
                })
                .await?;

                if let Ok(verification) = verification_res
                    && !verification.has_errors()
                {
                    let pair = serde_json::json!({
                        "prompt": format!(
                            "Rewrite the following code to adhere to style guidelines focusing on snake_case:\n\n```vox\n{}\n```",
                            result.source
                        ),
                        "response": format!("```vox\n{mutated_source}\n```"),
                        "messages": [
                            { "role": "user", "content": format!(
                                "Rewrite the following code to adhere to style guidelines focusing on snake_case:\n\n```vox\n{}\n```",
                                result.source
                            ) },
                            { "role": "assistant", "content": format!("```vox\n{mutated_source}\n```") }
                        ],
                        "category": "ast_mutate",
                        "lane": "vox_lang_tier_b",
                        "origin": "synthetic",
                        "schema_version": "vox_dogfood_v1",
                        "difficulty": 0.5,
                    });

                    out_buffer.push_str(&serde_json::to_string(&pair)?);
                    out_buffer.push('\n');
                    generated += 1;
                    progress = true;
                }
            }
        }

        if !progress {
            break; // Avoid infinite loop if no more mutations possible
        }
    }

    if let Some(p) = output.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    tokio::fs::write(output, out_buffer).await?;
    println!(
        "✓ Extracted {} mutated source pairs -> {}",
        generated,
        output.display()
    );

    Ok(())
}

pub(super) async fn run_rust_mine(source_dir: &Path, output: &Path) -> Result<()> {
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(source_dir) {
        if let Ok(e) = entry
            && e.path().extension().and_then(|e| e.to_str()) == Some("rs")
        {
            entries.push(e.path().to_path_buf());
        }
    }

    if entries.is_empty() {
        eprintln!("No .rs files found in {}", source_dir.display());
        return Ok(());
    }

    let mut generated = 0;
    let mut out_buffer = String::new();

    for path in entries {
        if let Ok(source) = std::fs::read_to_string(&path) {
            let translations = vox_corpus::rust_to_vox::extract_translations(&source);

            for tr in translations {
                // Verify the generated code with vox parser
                if let Ok(result) =
                    vox_compiler::pipeline::run_frontend_str(&tr.output_vox, "<synthetic>")
                    && !result.has_errors()
                {
                    let pair = serde_json::json!({
                        "prompt": format!("{}\n\n```rust\n{}\n```", tr.instruction, tr.input_rust),
                        "response": format!("```vox\n{}\n```", result.source),
                        "messages": [
                            { "role": "user", "content": format!("{}\n\n```rust\n{}\n```", tr.instruction, tr.input_rust) },
                            { "role": "assistant", "content": format!("```vox\n{}\n```", result.source) }
                        ],
                        "category": "rust_to_vox_translation",
                        "lane": "vox_rust_expert_cross",
                        "origin": "human",
                        "confidence": tr.confidence,
                    });

                    out_buffer.push_str(&serde_json::to_string(&pair)?);
                    out_buffer.push('\n');
                    generated += 1;
                }
            }
        }
    }

    if let Some(p) = output.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    tokio::fs::write(output, out_buffer).await?;
    println!(
        "✓ Extracted {} rust->vox translation pairs -> {}",
        generated,
        output.display()
    );

    Ok(())
}

pub(super) async fn run_diversity_check(
    input: &Path,
    min_diversity: f64,
    domain: Option<&str>,
) -> Result<()> {
    let content = read_utf8_path_capped_async(input).await?;
    let mut codes = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            // Check for common code fields: "vox_code", "output", "response"
            if let Some(code) = value
                .get("vox_code")
                .or_else(|| value.get("output"))
                .or_else(|| value.get("response"))
                .and_then(|v| v.as_str())
            {
                codes.push(code.to_string());
            }
        }
    }

    if codes.is_empty() {
        // Fallback: try looking for 'code' field often used in intermediate steps
        for line in content.lines() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                && let Some(code) = value.get("code").and_then(|v| v.as_str())
            {
                codes.push(code.to_string());
            }
        }
    }

    if codes.is_empty() {
        anyhow::bail!(
            "No Vox code found in {}. Ensure records have 'vox_code', 'output', or 'response' fields.",
            input.display()
        );
    }

    let report = vox_eval::eval_semantic_entropy(&codes, min_diversity);

    println!("--- Corpus Diversity Report ---");
    println!("  Input:      {}", input.display());
    println!("  Domain:     {}", domain.unwrap_or("(default)"));
    println!("  Records:    {}", codes.len());
    println!("  Diversity:  {:.2}%", report.ast_diversity * 100.0);
    println!("  Variance:   {:.2}", report.construct_variance);

    // Record to telemetry if DB is available (ALWAYS record for Flywheel tracking)
    if let Ok(db) = crate::workspace_db::connect_cli_workspace_voxdb_with_overrides(true).await {
        let session_id = if let Some(d) = domain {
            format!("corpus_diversity_check:{}", d)
        } else {
            "corpus_diversity_check".to_string()
        };
        let _ = db
            .append_research_metric(
                &session_id,
                "ast_diversity",
                Some(report.ast_diversity),
                Some(&serde_json::to_string(&report)?),
            )
            .await;
        let _ = db
            .append_research_metric(
                &session_id,
                "corpus_sample_count",
                Some(codes.len() as f64),
                None,
            )
            .await;
    }

    if report.collapse_warning {
        eprintln!(
            "  {} ALARM: Diversity below threshold ({:.2})!",
            "⚠".red(),
            min_diversity
        );
        anyhow::bail!("Corpus failed diversity check (potential mode collapse/monoculture).");
    } else {
        println!("  {} Diversity check PASSED.", "✓".green());
    }

    Ok(())
}

/// Curate prose-heavy lanes using a frontier LLM to filter logic hazards and structural monoculture.
pub(super) async fn run_curate_prose(
    input: &Path,
    output: &Path,
    min_score: f64,
    quarantine: Option<&Path>,
) -> Result<()> {
    use std::sync::Arc;
    use tokio::fs::{File, OpenOptions};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    println!(
        "{} Curating prose from {}...",
        "◆".cyan().bold(),
        input.display()
    );

    let input_file = File::open(input).await?;
    let mut reader = BufReader::new(input_file);
    let mut line = String::new();

    let mut output_file = File::create(output).await?;
    let mut quarantine_file = if let Some(q) = quarantine {
        Some(OpenOptions::new().create(true).append(true).open(q).await?)
    } else {
        None
    };

    let mut total = 0;
    let mut accepted = 0;
    let mut rejected = 0;

    // Concurrency throttle: keep it at 10 to avoid rate limits
    let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
    let mut set = tokio::task::JoinSet::new();

    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        let record: serde_json::Value = serde_json::from_str(trimmed)?;
        let content = record
            .get("response")
            .or_else(|| record.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if content.is_empty() {
            line.clear();
            continue;
        }

        total += 1;
        let sem = Arc::clone(&semaphore);
        let record_cl = record.clone();

        set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            curate_record_via_ai(record_cl, content).await
        });

        line.clear();
    }

    while let Some(res) = set.join_next().await {
        let (record, score, reason) = res??;
        if score >= min_score {
            let mut out_rec = record;
            if let Some(obj) = out_rec.as_object_mut() {
                obj.insert("curation_score".to_string(), serde_json::json!(score));
                obj.insert("curation_reason".to_string(), serde_json::json!(reason));
            }
            let row = serde_json::to_string(&out_rec)? + "\n";
            output_file.write_all(row.as_bytes()).await?;
            accepted += 1;
        } else {
            if let Some(ref mut q) = quarantine_file {
                let mut out_rec = record;
                if let Some(obj) = out_rec.as_object_mut() {
                    obj.insert("rejection_score".to_string(), serde_json::json!(score));
                    obj.insert("rejection_reason".to_string(), serde_json::json!(reason));
                }
                let row = serde_json::to_string(&out_rec)? + "\n";
                q.write_all(row.as_bytes()).await?;
            }
            rejected += 1;
        }
    }

    println!(
        "{} Curation complete: {}/{} accepted ({} rejected) -> {}",
        "✓".green().bold(),
        accepted,
        total,
        rejected,
        output.display()
    );

    Ok(())
}

async fn curate_record_via_ai(
    record: serde_json::Value,
    content: String,
) -> Result<(serde_json::Value, f64, String)> {
    let prompt = format!(
        "You are a high-fidelity data curator for an AI training pipeline.\n\
         Assess the following research/prose record for semantic integrity, logical consistency, and structural quality.\n\n\
         # Quality Hazards to Detect:\n\
         1. Logical inconsistencies or factual hallucinations.\n\
         2. Structural repetition (e.g., overuse of em-dashes, repetitive 'not just X, but Y' frames).\n\
         3. Low-entropy or unfalsifiable claims.\n\
         4. Hallucinated APIs or incorrect syntax relative to Vox/Rust patterns.\n\n\
         # Content to Curate:\n\
         {content}\n\n\
         # Output Format:\n\
         Return ONLY a single JSON object with 'score' (0.0 to 1.0) and 'reason' (string).\n\
         Example: {{ \"score\": 0.85, \"reason\": \"Solid technical explanation but uses one repetitive metaphor.\" }}"
    );

    // Call the daemon to generate
    let result = crate::dei_daemon::call(
        crate::dei_daemon::method::AI_GENERATE,
        serde_json::json!({ "prompt": prompt }),
        false,
    )
    .await?;

    let text = result
        .get("text")
        .or_else(|| result.get("output"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Find JSON block in text (best effort)
    let json_str = if let (Some(s), Some(e)) = (text.find('{'), text.rfind('}')) {
        &text[s..=e]
    } else {
        text
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("AI returned invalid JSON: {}\nError: {}", text, e))?;

    let score = parsed.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reason = parsed
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("no reason provided")
        .to_string();

    Ok((record, score, reason))
}
