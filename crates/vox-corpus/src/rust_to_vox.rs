use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct TranslationPair {
    pub instruction: String,
    pub input_rust: String,
    pub output_vox: String,
    pub confidence: f32,
}

/// Recursively find all .rs files in a directory, skipping common ignored dirs.
fn crawl_rust_files(dir: &Path, results: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" || name == ".git" || name == ".gemini" || name == "node_modules" {
                continue;
            }
            crawl_rust_files(&path, results)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            results.push(path);
        }
    }
    Ok(())
}

pub fn extract_translations(source: &str) -> Vec<TranslationPair> {
    let mut results = Vec::new();

    // Pattern 1: Simple Struct to @table
    // (Improved regex to capture public fields and map them)
    let struct_re = regex::Regex::new(r"(?s)pub struct (\w+) \{\s*([^}]+)\}").unwrap();
    for cap in struct_re.captures_iter(source) {
        let name = &cap[1];
        let fields_raw = &cap[2];
        let mut vox_fields = String::new();

        for line in fields_raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if let Some(id_pos) = line.find(':') {
                let fname = line[..id_pos].trim().trim_start_matches("pub").trim();
                let ftype_part = line[id_pos + 1..].trim();
                let ftype = ftype_part.split([',', ' ', '}']).next().unwrap_or("");

                let vtype = match ftype {
                    "u64" | "i64" | "usize" | "i32" | "u32" | "u16" | "i16" => "int",
                    "String" | "&str" | "Box<str>" | "Arc<str>" => "str",
                    "bool" => "bool",
                    "f32" | "f64" => "dec",
                    _ => ftype,
                };
                vox_fields.push_str(&format!("    {fname}: {vtype}\n"));
            }
        }

        if !vox_fields.is_empty() {
            results.push(TranslationPair {
                instruction: format!("Translate the Rust struct `{name}` to a Vox `@table` type."),
                input_rust: cap[0].to_string(),
                output_vox: format!("table {name} {{\n{vox_fields}}}"),
                confidence: 0.9,
            });
        }
    }

    // Pattern 2: Enum to Type
    let enum_re = regex::Regex::new(r"(?s)pub enum (\w+) \{\s*([^}]+)\}").unwrap();
    for cap in enum_re.captures_iter(source) {
        let name = &cap[1];
        let variants_raw = &cap[2];
        let mut vox_variants = String::new();

        for line in variants_raw.lines() {
            let line = line.trim().trim_end_matches(',');
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            vox_variants.push_str(&format!("    {line}\n"));
        }

        if !vox_variants.is_empty() {
            results.push(TranslationPair {
                instruction: format!(
                    "Translate the Rust enum `{name}` to a Vox tagged union `type`."
                ),
                input_rust: cap[0].to_string(),
                output_vox: format!("type {name} {{\n{vox_variants}}}"),
                confidence: 0.85,
            });
        }
    }

    results
}

pub fn generate_rust_to_vox_pairs(
    out: &mut impl Write,
    _target_count: usize,
) -> anyhow::Result<usize> {
    let mut actual_count = 0;
    let mut rust_files = Vec::new();

    // Search in the crates directory relative to workspace root
    let workspace_root = Path::new(".");
    let crates_dir = workspace_root.join("crates");

    if let Err(e) = crawl_rust_files(&crates_dir, &mut rust_files) {
        eprintln!("  [rust_to_vox] warning: failed to crawl crates: {e}");
    }

    // Also check current crate if we are in one
    if rust_files.is_empty() {
        let _ = crawl_rust_files(Path::new("src"), &mut rust_files);
    }

    for path in rust_files {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let pairs = extract_translations(&content);
            for pair in pairs {
                let record = json!({
                    "instruction": pair.instruction,
                    "input": format!("```rust\n{}\n```", pair.input_rust),
                    "output": format!("```vox\n{}\n```", pair.output_vox),
                    "category": "rust_to_vox_translation",
                    "lane": "vox_rust_expert_cross",
                    "source": format!("{}", path.display()),
                    "rating": 5,
                    "origin": "human"
                });

                writeln!(out, "{}", serde_json::to_string(&record)?)?;
                actual_count += 1;
            }
        }
    }

    Ok(actual_count)
}
