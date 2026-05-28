//! Rust source code corpus extractor for Mens training data.
//!
//! Walks `crates/**/*.rs` and extracts function-level doc comment + signature + body
//! triples as `prompt`/`response` JSONL rows compatible with
//! `vox_tensor::data::TrainingPair`.
//!
//! ## Extraction strategy
//! - **Prompt**: concatenation of `///` doc lines immediately preceding the `pub fn` / `fn`
//!   signature. If no doc is found, a generic imperative template is generated from the
//!   function name and crate category tag.
//! - **Response**: the full function signature + body block (collected by brace-depth tracking).
//! - **Category**: inferred from the file path (`crates/vox-compiler` → `compiler`;
//!   the compiler monolith under `vox-compiler`).
//! - **Minimum body lines**: functions with fewer than `min_body_lines` non-empty body lines
//!   are skipped (avoids training on trivial stubs).
//! - **Test modules**: `#[cfg(test)]` blocks are excluded by default.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::json;

/// Configuration for Rust source extraction.
#[derive(Debug, Clone)]
pub struct ExtractRsConfig {
    /// Root directory to walk (usually the repo root or `crates/`).
    pub root: PathBuf,
    /// Minimum number of non-empty lines inside a function body to include it.
    pub min_body_lines: usize,
    /// When true, skip `#[cfg(test)]` modules and `#[test]` functions.
    pub skip_tests: bool,
    /// Maximum number of pairs to emit (0 = unlimited).
    pub limit: usize,
    /// Minimum quality rating to assign. Rust source pairs always get this value.
    pub default_rating: u8,
}

impl Default for ExtractRsConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::from("crates"),
            min_body_lines: 3,
            skip_tests: true,
            limit: 0,
            default_rating: 4,
        }
    }
}

/// One extracted training pair from Rust source.
#[derive(Debug, Clone)]
pub struct RsTrainingPair {
    /// Source file path (relative to root).
    pub source_path: PathBuf,
    /// Inferred category from crate name.
    pub category: String,
    /// The prompt (doc comment text or generated imperative).
    pub prompt: String,
    /// The full function block (signature + body).
    pub response: String,
    /// Quality rating.
    pub rating: u8,
    /// Estimated difficulty (3-10).
    pub difficulty: u8,
}

impl RsTrainingPair {
    /// Serialize to a JSONL row compatible with `vox_tensor::data::TrainingPair`.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let lane = "rust_context".to_string();
        let task_family = format!("rust_{}", self.category);
        let v = json!({
            "prompt": self.prompt,
            "response": self.response,
            "category": self.category,
            "rating": self.rating,
            "difficulty": self.difficulty,
            "source": self.source_path.display().to_string(),
            "lane": lane,
            "task_family": task_family,
        });
        v.to_string()
    }
}

/// Infer a corpus category tag from a crate path component.
fn infer_category(path: &Path) -> String {
    // Walk path components to find crate name like `vox-compiler`
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.starts_with("vox-") {
            return s.trim_start_matches("vox-").replace('-', "_").to_string();
        }
    }
    "rust_source".to_string()
}

/// Return true if the line (trimmed) marks the beginning of a `#[cfg(test)]` block.
fn is_cfg_test_attr(line: &str) -> bool {
    let t = line.trim();
    t == "#[cfg(test)]" || t.starts_with("#[cfg(test,") || t.starts_with("#[cfg(any(test")
}

/// Return true if the line (trimmed) is a `#[test]` attribute.
fn is_test_attr(line: &str) -> bool {
    let t = line.trim();
    t == "#[test]" || t.starts_with("#[test]")
}

/// Extract function-level pairs from a single Rust source file.
pub fn extract_from_file(
    path: &Path,
    config: &ExtractRsConfig,
    category: &str,
) -> anyhow::Result<Vec<RsTrainingPair>> {
    let source = vox_bounded_fs::read_utf8_path_capped(path)
        .with_context(|| format!("read {}", path.display()))?;
    extract_from_source(&source, path, config, category)
}

/// Extract function-level pairs from a Rust source string.
pub fn extract_from_source(
    source: &str,
    path: &Path,
    config: &ExtractRsConfig,
    category: &str,
) -> anyhow::Result<Vec<RsTrainingPair>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut pairs = Vec::new();
    let mut in_cfg_test = false;
    let mut cfg_test_depth: i32 = 0;
    let mut pending_doc: Vec<String> = Vec::new();
    let mut pending_is_test = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Track #[cfg(test)] block entry
        if is_cfg_test_attr(trimmed) && config.skip_tests {
            in_cfg_test = true;
            cfg_test_depth = 0;
        }

        if in_cfg_test {
            for c in trimmed.chars() {
                if c == '{' {
                    cfg_test_depth += 1;
                } else if c == '}' {
                    cfg_test_depth -= 1;
                }
            }
            if cfg_test_depth <= 0 && trimmed.contains('}') {
                in_cfg_test = false;
            }
            i += 1;
            pending_doc.clear();
            continue;
        }

        // Accumulate doc comments
        if trimmed.starts_with("///") {
            let doc_text = trimmed.trim_start_matches("///").trim().to_string();
            if !doc_text.is_empty() {
                pending_doc.push(doc_text);
            }
            i += 1;
            continue;
        }

        // Track #[test] attribute
        if is_test_attr(trimmed) && config.skip_tests {
            pending_is_test = true;
            i += 1;
            continue;
        }

        // Detect function signature lines
        let is_fn_line = trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub(crate) fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("async fn ");

        let is_type_def = trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub(crate) struct ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("pub(crate) enum ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("pub(crate) type ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("pub(crate) trait ")
            || trimmed.starts_with("trait ");

        if is_fn_line || is_type_def {
            if pending_is_test && config.skip_tests {
                pending_doc.clear();
                pending_is_test = false;
                i += 1;
                continue;
            }

            // Collect the full block by tracking brace depth
            let block_start = i;
            let mut depth: i32 = 0;
            let mut block_lines: Vec<&str> = Vec::new();
            let mut found_open = false;

            while i < lines.len() {
                let fl = lines[i];
                block_lines.push(fl);

                // Handle type alias or simple struct with semicolon
                if !found_open && fl.trim().ends_with(';') {
                    i += 1;
                    break;
                }

                for c in fl.chars() {
                    if c == '{' {
                        depth += 1;
                        found_open = true;
                    } else if c == '}' {
                        depth -= 1;
                    }
                }
                i += 1;
                if found_open && depth <= 0 {
                    break;
                }
            }

            // For functions, enforce min_body_lines. For types, take them all.
            let mut eligible = true;
            if is_fn_line {
                let body_lines: Vec<&&str> = block_lines
                    .iter()
                    .skip(1)
                    .filter(|l| !l.trim().is_empty() && l.trim() != "{" && l.trim() != "}")
                    .collect();
                eligible = body_lines.len() >= config.min_body_lines;
            }

            if eligible {
                let response = block_lines.join("\n");
                let kind = if is_fn_line {
                    "function"
                } else if trimmed.contains("struct ") {
                    "struct"
                } else if trimmed.contains("enum ") {
                    "enum"
                } else if trimmed.contains("trait ") {
                    "trait"
                } else {
                    "type"
                };

                // Build prompt from doc or generate imperative
                let prompt = if pending_doc.is_empty() {
                    // Extract name for a generic template
                    let name = extract_block_name(lines[block_start], kind);
                    format!(
                        "Implement the `{name}` {kind} in Rust (Vox crate: `{category}`)",
                        name = name,
                        kind = kind,
                        category = category
                    )
                } else {
                    pending_doc.join(" ")
                };

                pairs.push(RsTrainingPair {
                    source_path: path.to_path_buf(),
                    category: format!("rust_{category}_{kind}"),
                    prompt,
                    response,
                    rating: config.default_rating,
                    difficulty: match block_lines.len() {
                        0..=10 => 3,
                        11..=30 => 5,
                        _ => 8,
                    },
                });

                if config.limit > 0 && pairs.len() >= config.limit {
                    return Ok(pairs);
                }
            }

            pending_doc.clear();
            pending_is_test = false;
            continue;
        }

        // Non-doc, non-fn line: flush pending doc accumulator
        if !trimmed.is_empty()
            && !trimmed.starts_with("//!")
            && !trimmed.starts_with("#[")
            && !trimmed.starts_with("//")
        {
            pending_doc.clear();
        }
        pending_is_test = false;
        i += 1;
    }

    Ok(pairs)
}

/// Extract the bare identifier name from a definition line.
fn extract_block_name<'a>(line: &'a str, kind: &'a str) -> &'a str {
    let marker = match kind {
        "function" => "fn ",
        "struct" => "struct ",
        "enum" => "enum ",
        "trait" => "trait ",
        "type" => "type ",
        _ => "fn ",
    };

    // Find the marker and take the identifier after it
    let after = if let Some(p) = line.find(marker) {
        &line[p + marker.len()..]
    } else {
        return kind;
    };
    let end = after
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(after.len());
    &after[..end]
}

/// Walk `config.root` recursively and extract pairs from all `.rs` files.
pub fn walk_and_extract(config: &ExtractRsConfig) -> anyhow::Result<Vec<RsTrainingPair>> {
    let mut all = Vec::new();
    walk_dir_rs(&config.root, config, &mut all)?;
    Ok(all)
}

fn walk_dir_rs(
    dir: &Path,
    config: &ExtractRsConfig,
    out: &mut Vec<RsTrainingPair>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip target/, .git/, and similar noise
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(name, "target" | ".git" | "node_modules" | ".vox") {
                continue;
            }
            walk_dir_rs(&path, config, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let category = infer_category(&path);
            match extract_from_file(&path, config, &category) {
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
                    tracing::debug!(path = %path.display(), error = %e, "rs extract skip");
                }
            }
        }
    }
    Ok(())
}

/// Write extracted pairs to a JSONL file (appending).
pub fn write_to_jsonl(pairs: &[RsTrainingPair], output: &Path) -> anyhow::Result<usize> {
    use std::io::Write;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
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

    const SAMPLE_RS: &str = r#"
/// Compute the sum of two integers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// A no-doc function with multiple body lines.
fn complex(x: u32) -> u32 {
    let y = x * 2;
    let z = y + 1;
    z
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
"#;

    #[test]
    fn extracts_doc_functions() {
        let cfg = ExtractRsConfig {
            min_body_lines: 1,
            skip_tests: true,
            ..ExtractRsConfig::default()
        };
        let pairs = extract_from_source(SAMPLE_RS, Path::new("vox-test/src/lib.rs"), &cfg, "test")
            .expect("extract");
        assert!(!pairs.is_empty(), "should extract at least one pair");
        let add_pair = pairs.iter().find(|p| p.response.contains("fn add"));
        assert!(add_pair.is_some(), "should have extracted add function");
        assert!(
            add_pair.unwrap().prompt.contains("sum"),
            "prompt should come from doc: {}",
            add_pair.unwrap().prompt
        );
    }

    #[test]
    fn skips_test_module() {
        let cfg = ExtractRsConfig {
            min_body_lines: 1,
            skip_tests: true,
            ..ExtractRsConfig::default()
        };
        let pairs =
            extract_from_source(SAMPLE_RS, Path::new("lib.rs"), &cfg, "test").expect("extract");
        for p in &pairs {
            assert!(
                !p.response.contains("it_works"),
                "should not include test function"
            );
        }
    }

    #[test]
    fn respects_min_body_lines() {
        let cfg = ExtractRsConfig {
            min_body_lines: 4,
            skip_tests: false,
            ..ExtractRsConfig::default()
        };
        let pairs =
            extract_from_source(SAMPLE_RS, Path::new("lib.rs"), &cfg, "test").expect("extract");
        // add() has 1 body line, complex() has 3 — neither meets 4
        assert!(
            pairs.is_empty(),
            "should skip all with min_body_lines=4, got {} pairs",
            pairs.len()
        );
    }

    #[test]
    fn infers_category_from_crate_path() {
        let p = Path::new("crates/vox-compiler/src/parser/grammar.rs");
        assert_eq!(infer_category(p), "compiler");

        let p2 = Path::new("crates/vox-actor-runtime/src/lib.rs");
        assert_eq!(infer_category(p2), "actor_runtime");
    }

    #[test]
    fn generic_prompt_for_no_doc_fn() {
        let src =
            "fn my_helper(x: u32) -> u32 {\n    let a = x + 1;\n    let b = a * 2;\n    b\n}\n";
        let cfg = ExtractRsConfig {
            min_body_lines: 2,
            skip_tests: false,
            ..ExtractRsConfig::default()
        };
        let pairs = extract_from_source(src, Path::new("lib.rs"), &cfg, "core").expect("extract");
        assert!(!pairs.is_empty());
        assert!(
            pairs[0].prompt.contains("my_helper"),
            "generic prompt should contain fn name: {}",
            pairs[0].prompt
        );
    }
}
