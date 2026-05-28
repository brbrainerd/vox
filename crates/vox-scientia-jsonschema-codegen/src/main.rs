//! Regenerate `crates/vox-research-events/src/schema_types.generated.rs` from SCIENTIA JSON Schemas.
//!
//! Run from repo root after schema edits:
//! `cargo run -p vox-scientia-jsonschema-codegen`

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use schemars08::schema::RootSchema;
use typify::TypeSpace;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn module_name(schema_path: &Path) -> String {
    schema_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .chars()
        .map(|c| match c {
            '.' | '-' => '_',
            _ => c,
        })
        .collect()
}

fn main() -> Result<()> {
    let repo = repo_root();
    let scientia = repo.join("contracts/scientia");
    let out_path = repo.join("crates/vox-research-events/src/schema_types.generated.rs");

    let mut out = String::new();
    out.push_str("// @generated — source: contracts/scientia/*.schema.json\n");
    out.push_str("// Regenerate: cargo run -p vox-scientia-jsonschema-codegen\n\n");

    let mut paths: Vec<PathBuf> = WalkDir::new(&scientia)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".schema.json"))
        })
        .collect();
    paths.sort();

    for path in &paths {
        let raw =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let root_schema: RootSchema =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;

        let mut type_space = TypeSpace::default();
        type_space
            .add_root_schema(root_schema)
            .with_context(|| format!("typify ingest {}", path.display()))?;

        let stream = type_space.to_stream();
        let syntax_tree =
            syn::parse2(stream).with_context(|| format!("typify parse {}", path.display()))?;
        let formatted = prettyplease::unparse(&syntax_tree);

        let mod_name = module_name(path);
        out.push_str(&format!(
            "// --- {} ---\npub mod {mod_name} {{\n",
            path.strip_prefix(&repo).unwrap_or(path).display()
        ));
        for line in formatted.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }

    fs::write(&out_path, out).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!("wrote {}", out_path.display());
    Ok(())
}
