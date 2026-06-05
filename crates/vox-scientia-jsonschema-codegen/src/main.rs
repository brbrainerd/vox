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

    // typify can *panic* (not just return Err) on schemas it cannot model — e.g. a
    // root-level `oneOf` discriminated union (a typify 0.6 limitation). Such schemas
    // are validation-only: they're consumed via the jsonschema validator at runtime,
    // not as generated Rust types. So we catch the panic, skip the schema with a
    // marker, and keep generating the rest — a single un-modelable schema must not
    // abort the whole regeneration. A genuine `Err` (bad schema) still propagates.
    let mut skipped: Vec<String> = Vec::new();
    let mut hard_error: Option<anyhow::Error> = None;
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // silence default panic output; we report below

    for path in &paths {
        eprintln!("[codegen] processing {}", path.display());
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let root_schema: RootSchema =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
        let rel = path
            .strip_prefix(&repo)
            .unwrap_or(path)
            .display()
            .to_string();

        let rendered =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<String> {
                let mut type_space = TypeSpace::default();
                type_space
                    .add_root_schema(root_schema)
                    .with_context(|| format!("typify ingest {rel}"))?;
                let stream = type_space.to_stream();
                let syntax_tree =
                    syn::parse2(stream).with_context(|| format!("typify parse {rel}"))?;
                Ok(prettyplease::unparse(&syntax_tree))
            }));

        let formatted = match rendered {
            Ok(Ok(code)) => code,
            Ok(Err(e)) => {
                hard_error = Some(e);
                break;
            }
            Err(_panic) => {
                eprintln!(
                    "[codegen] WARNING: typify could not model {rel} — skipping \
                     (validation-only schema; no Rust types emitted)"
                );
                out.push_str(&format!(
                    "// --- {rel} : SKIPPED — typify could not model this schema (validation-only) ---\n\n"
                ));
                skipped.push(rel);
                continue;
            }
        };

        let mod_name = module_name(path);
        out.push_str(&format!("// --- {rel} ---\npub mod {mod_name} {{\n"));
        for line in formatted.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("}\n\n");
    }

    std::panic::set_hook(prev_hook);
    if let Some(e) = hard_error {
        return Err(e);
    }

    fs::write(&out_path, out).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!(
        "wrote {} ({} schema(s) skipped: {})",
        out_path.display(),
        skipped.len(),
        if skipped.is_empty() {
            "none".to_string()
        } else {
            skipped.join(", ")
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_normalises_dots_and_hyphens() {
        let p = PathBuf::from("foo.bar-baz.schema.json");
        // Strip last extension only: file_stem -> "foo.bar-baz.schema" -> normalised.
        assert_eq!(module_name(&p), "foo_bar_baz_schema");
    }
}
