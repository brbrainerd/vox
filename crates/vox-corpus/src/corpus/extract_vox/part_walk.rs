/// Extract all training pairs from a single `.vox` file.
pub fn extract_from_vox_file(
    path: &Path,
    config: &ExtractVoxConfig,
) -> anyhow::Result<Vec<VoxTrainingPair>> {
    let source =
        vox_bounded_fs::read_utf8_path_capped(path).with_context(|| format!("read {}", path.display()))?;

    if !is_eligible_for_training(&source) {
        return Ok(Vec::new());
    }

    let content_lines = source
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#') && !t.starts_with("//")
        })
        .count();

    if content_lines < config.min_content_lines {
        return Ok(Vec::new());
    }

    let category = infer_vox_category(path, &source);
    let mut pairs = Vec::new();

    // 1. Whole-file pair
    let file_prompt = extract_file_doc(&source).unwrap_or_else(|| {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("program");
        format!(
            "Write a complete Vox program that implements {}",
            stem.replace('_', " ")
        )
    });

    pairs.push(VoxTrainingPair {
        source_path: path.to_path_buf(),
        category: format!("vox_{category}"),
        prompt: file_prompt,
        response: source.clone(),
        rating: config.default_rating,
    });

    // 2. Per-construct pairs
    let blocks = extract_construct_blocks(&source);
    for (i, (construct_type, name, block)) in blocks.iter().enumerate() {
        if block.lines().count() < 2 {
            continue;
        }
        let prompt = construct_prompt(construct_type.as_str(), name, i);
        pairs.push(VoxTrainingPair {
            source_path: path.to_path_buf(),
            category: format!("vox_{construct_type}"),
            prompt,
            response: block.clone(),
            rating: config.default_rating,
        });
    }

    if config.limit > 0 {
        pairs.truncate(config.limit);
    }

    Ok(pairs)
}

/// Walk a directory tree and extract pairs from all `.vox` files.
pub fn walk_and_extract_vox(config: &ExtractVoxConfig) -> anyhow::Result<Vec<VoxTrainingPair>> {
    let mut all = Vec::new();
    
    if let Ok(mut golden) = extract_golden_examples(&config.root) {
        all.append(&mut golden);
    }
    
    walk_vox_dir(&config.root, config, &mut all)?;
    Ok(all)
}

fn walk_vox_dir(
    dir: &Path,
    config: &ExtractVoxConfig,
    out: &mut Vec<VoxTrainingPair>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "target" | ".git" | "node_modules" | ".vox" | "vox-vscode"
            ) {
                continue;
            }
            walk_vox_dir(&path, config, out)?;
        } else if path.extension().is_some_and(|e| e == "vox") {
            match extract_from_vox_file(&path, config) {
                Ok(mut pairs) => {
                    if config.limit > 0 {
                        let remaining = config.limit.saturating_sub(out.len());
                        pairs.truncate(remaining);
                    }
                    out.extend(pairs);
                    if config.limit > 0 && out.len() >= config.limit {
                        return Ok(());
                    }
                }
                Err(e) => {
                    eprintln!("  [vox extract] skip {}: {e}", path.display());
                }
            }
        }
    }
    Ok(())
}

pub fn extract_golden_examples(dir: &Path) -> anyhow::Result<Vec<VoxTrainingPair>> {
    let mut pairs = Vec::new();
    let golden_dir = dir.join("examples/golden");
    if !golden_dir.exists() {
        return Ok(pairs);
    }
    walk_golden_dir(&golden_dir, &mut pairs)?;
    Ok(pairs)
}

fn walk_golden_dir(dir: &Path, out: &mut Vec<VoxTrainingPair>) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_golden_dir(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "vox")
            && let Ok(source) = std::fs::read_to_string(&path) {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("example");
                let summary = extract_golden_prompt_summary(&source).unwrap_or_else(|| {
                    source
                        .lines()
                        .find(|l| l.trim().starts_with("//"))
                        .map(|l| l.trim().trim_start_matches("//").trim().to_string())
                        .unwrap_or_else(|| "the given specification".into())
                });
                let prompt = format!(
                    "Write a complete Vox program for {stem} that implements {summary}"
                );

                out.push(VoxTrainingPair {
                    source_path: path.to_path_buf(),
                    category: "golden".into(),
                    prompt,
                    response: source.clone(),
                    rating: 5,
                });
            }
    }
    Ok(())
}

/// Write extracted Vox pairs to a JSONL file (truncating).
pub fn write_vox_to_jsonl(pairs: &[VoxTrainingPair], output: &Path) -> anyhow::Result<usize> {
    use std::io::Write;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output)
        .with_context(|| format!("open output {}", output.display()))?;
    for pair in pairs {
        writeln!(f, "{}", pair.to_jsonl())?;
    }
    Ok(pairs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VOX: &str = r#"# agent.vox
# Example agent definition with tools and memory

table AgentMemory {
    session_id: str
    context: str
}

fn SupportBot(query: str, session: str) to str {
    let past = db.agent_memory.find(session)
    let response = "Based on " + past.context + " -> " + query
    db.agent_memory.insert(AgentMemory(session, query))
    return response
}
"#;

    #[test]
    fn extracts_whole_file_pair() {
        let cfg = ExtractVoxConfig {
            min_content_lines: 1,
            ..Default::default()
        };
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("agent.vox");
        std::fs::write(&p, SAMPLE_VOX).unwrap();
        let pairs = extract_from_vox_file(&p, &cfg).unwrap();
        assert!(!pairs.is_empty(), "should extract at least one pair");
        assert!(pairs[0].response.contains("SupportBot"));
    }

    #[test]
    fn extracts_construct_blocks() {
        let blocks = extract_construct_blocks(SAMPLE_VOX);
        assert!(
            blocks.iter().any(|(_, name, _)| name == "AgentMemory"),
            "should find table AgentMemory"
        );
        assert!(
            blocks.iter().any(|(_, name, _)| name == "SupportBot"),
            "should find fn SupportBot"
        );
    }

    #[test]
    fn infers_category_from_content() {
        let category = infer_vox_category(Path::new("test.vox"), SAMPLE_VOX);
        assert_eq!(category, "table"); // @table is the first construct keyword found
    }

    #[test]
    fn extract_file_doc_works() {
        let doc = extract_file_doc(SAMPLE_VOX);
        assert!(doc.is_some());
        assert!(doc.unwrap().contains("agent"));
    }
}
