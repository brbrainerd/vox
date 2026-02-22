use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub async fn run(file: &Path, out_dir: &Path) -> Result<()> {
    let source = fs::read_to_string(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    // 1. Lex
    let tokens = vox_lexer::lex(&source);
    tracing::info!("Lexed {} tokens", tokens.len());

    // 2. Parse
    let module = vox_parser::parser::parse(tokens).map_err(|errors| {
        for e in &errors {
            eprintln!("Parse error: {} at {:?}", e.message, e.span);
        }
        anyhow::anyhow!("Parsing failed with {} error(s)", errors.len())
    })?;
    tracing::info!("Parsed {} declarations", module.declarations.len());

    // 3. Type check
    let diagnostics = vox_typeck::typecheck_module(&module);
    let has_errors = diagnostics
        .iter()
        .any(|d| d.severity == vox_typeck::diagnostics::Severity::Error);
    for d in &diagnostics {
        match d.severity {
            vox_typeck::diagnostics::Severity::Error => {
                eprintln!("error: {} at {:?}", d.message, d.span)
            }
            vox_typeck::diagnostics::Severity::Warning => {
                eprintln!("warning: {} at {:?}", d.message, d.span)
            }
        }
    }
    if has_errors {
        anyhow::bail!("Type checking failed");
    }
    tracing::info!("Type checking passed");

    // 4. Lower to HIR
    let hir = vox_hir::lower::lower_module(&module);

    // 5. Generate TypeScript (Frontend)
    let ts_output = vox_codegen_ts::generate(&module)
        .map_err(|e| anyhow::anyhow!("TypeScript code generation failed: {e}"))?;

    // 6. Generate Rust (Backend)
    let rust_output = vox_codegen_rust::generate(&hir, "vox_generated_app")
        .map_err(|e| anyhow::anyhow!("Rust code generation failed: {e}"))?;

    // 7. Write output files
    fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;

    // Write generated TS files
    for (filename, content) in &ts_output.files {
        let path = out_dir.join(filename);
        fs::write(&path, content)
            .with_context(|| format!("Failed to write output file: {}", path.display()))?;
        println!("  wrote {}", path.display());
    }

    // 8. Handle @v0 components
    // We iterate over the parsed declarations to find V0Components
    for decl in &module.declarations {
        if let vox_ast::decl::Decl::V0Component(comp) = decl {
            let component_name = &comp.name;
            let filename = format!("{}.tsx", component_name);
            let target_path = out_dir.join(&filename);

            // Only generate if file doesn't exist to avoid overwriting edits
            if !target_path.exists() {
                println!("Generating v0 component '{}'...", component_name);

                // Determine prompt and optional image path
                let (prompt, image_path) = if !comp.prompt.is_empty() {
                    (comp.prompt.clone(), None)
                } else if let Some(img_str) = &comp.image_path {
                    let parent = file.parent().unwrap_or(Path::new("."));
                    let path = parent.join(img_str);
                    (
                        "Create a component based on the provided image.".to_string(),
                        Some(path),
                    )
                } else {
                    ("Create a React component".to_string(), None)
                };

                match crate::v0::generate_component(
                    &prompt,
                    component_name,
                    out_dir,
                    image_path.as_deref(),
                )
                .await
                {
                    Ok(path) => println!("  generated v0 component: {}", path.display()),
                    Err(e) => eprintln!(
                        "  failed to generate v0 component '{}': {}",
                        component_name, e
                    ),
                }
            } else {
                println!("  skipping v0 component '{}' (file exists)", component_name);
            }
        }
    }

    // Write API client for server functions (if any)
    if !rust_output.api_client_ts.is_empty() {
        let api_path = out_dir.join("api.ts");
        fs::write(&api_path, &rust_output.api_client_ts)
            .with_context(|| format!("Failed to write API client: {}", api_path.display()))?;
        println!("  wrote {}", api_path.display());
    }

    // Rust goes to target/generated
    let generated_dir = std::path::Path::new("target").join("generated");
    fs::create_dir_all(generated_dir.join("src"))
        .context("Failed to create generated src directory")?;

    for (filename, content) in &rust_output.files {
        let path = generated_dir.join(filename);
        // Ensure parent dir exists (e.g. src/)
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)
            .with_context(|| format!("Failed to write output file: {}", path.display()))?;
        println!("  wrote {}", path.display());
    }

    println!(
        "Build complete: {} TS file(s), {} Rust file(s) generated",
        ts_output.files.len(),
        rust_output.files.len()
    );
    Ok(())
}
