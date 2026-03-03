use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use vox_ast::decl::Decl;

/// `vox doc` — generate API documentation from source comments.
pub async fn run(file: &Path, out_dir: &Path) -> Result<()> {
    println!("Generating documentation for {}...", file.display());

    if !file.exists() {
        anyhow::bail!("Source file {} does not exist", file.display());
    }

    // Use the shared pipeline for parsing — avoids duplicating lex+parse here.
    let result = crate::pipeline::run_frontend(file, false).await.map_err(|e| {
        anyhow::anyhow!("Failed to parse source for documentation: {}", e)
    })?;

    let mut markdown = format!(
        "# Project Documentation: {}\n\n",
        file.file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );

    for decl in &result.module.declarations {
        match decl {
            Decl::Function(f) => {
                markdown.push_str(&format!("## Function: `{}`\n\n", f.name));
                markdown.push_str("```vox\n");
                markdown.push_str(&result.source[f.span.start..f.span.end]);
                markdown.push_str("\n```\n\n");
            }
            Decl::Actor(a) => {
                markdown.push_str(&format!("## Actor: `{}`\n\n", a.name));
                markdown.push_str("```vox\n");
                markdown.push_str(&result.source[a.span.start..a.span.end]);
                markdown.push_str("\n```\n\n");
            }
            Decl::TypeDef(t) => {
                markdown.push_str(&format!("## Type: `{}`\n\n", t.name));
                markdown.push_str("```vox\n");
                markdown.push_str(&result.source[t.span.start..t.span.end]);
                markdown.push_str("\n```\n\n");
            }
            Decl::Table(t) => {
                markdown.push_str(&format!("## Table: `{}`\n\n", t.name));
                markdown.push_str("```vox\n");
                markdown.push_str(&result.source[t.span.start..t.span.end]);
                markdown.push_str("\n```\n\n");
            }
            Decl::Workflow(w) => {
                markdown.push_str(&format!("## Workflow: `{}`\n\n", w.name));
                markdown.push_str("```vox\n");
                markdown.push_str(&result.source[w.span.start..w.span.end]);
                markdown.push_str("\n```\n\n");
            }
            _ => {}
        }
    }

    fs::create_dir_all(out_dir).context("Failed to create documentation directory")?;
    let out_file = out_dir.join("API.md");
    fs::write(&out_file, markdown).context("Failed to write documentation file")?;

    println!("✓ Documentation generated at {}", out_file.display());
    Ok(())
}
