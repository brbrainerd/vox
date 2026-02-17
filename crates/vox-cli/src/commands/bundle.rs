use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};
use crate::commands::build;
use crate::templates;

/// Bundle a Vox source file into a complete, runnable web application.
///
/// 1. Runs the build pipeline (lex → parse → typecheck → codegen)
/// 2. Scaffolds a Vite + React project around the generated TS components
/// 3. Runs `npm install && npm run build` to produce static assets
/// 4. Copies built assets into the Rust backend's public/ directory
/// 5. Runs `cargo build --release` to produce a single binary
pub async fn run(file: &Path, out_dir: &Path, target: Option<&str>, release: bool) -> Result<()> {
    // Step 1: Run the standard build pipeline
    println!("=== Step 1/5: Compiling Vox source ===");
    build::run(file, out_dir).await?;

    // Check if we have any frontend components
    let has_frontend = out_dir.join("Chat.tsx").exists()
        || fs::read_dir(out_dir)
            .ok()
            .map(|entries| entries.filter_map(|e| e.ok()).any(|e| {
                e.path().extension().map_or(false, |ext| ext == "tsx")
            }))
            .unwrap_or(false);

    if !has_frontend {
        println!("No frontend components found. Backend-only build complete.");
        return Ok(());
    }

    // Step 2: Scaffold the React/Vite project
    println!("=== Step 2/5: Scaffolding React application ===");
    let app_dir = out_dir.join("app");
    scaffold_react_app(&app_dir, out_dir)?;

    // Step 3: Install deps and build
    println!("=== Step 3/5: Installing dependencies & building ===");
    npm_install_and_build(&app_dir)?;

    // Step 4: Copy built assets to backend public dir
    println!("=== Step 4/5: Packaging static assets ===");
    let generated_dir = PathBuf::from("target").join("generated");
    let public_dir = generated_dir.join("public");
    copy_built_assets(&app_dir.join("dist"), &public_dir)?;

    // Step 5: Build the single binary
    println!("=== Step 5/5: Building single binary ===");
    let binary_path = build_single_binary(&generated_dir, target, release)?;

    // Copy binary to dist/
    let dist_dir = PathBuf::from("dist");
    fs::create_dir_all(&dist_dir)?;
    let app_name = file.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());
    let ext = if cfg!(windows) && target.is_none() { ".exe" } else if target.map_or(false, |t| t.contains("windows")) { ".exe" } else { "" };
    let dest = dist_dir.join(format!("{}{}", app_name, ext));
    fs::copy(&binary_path, &dest)
        .with_context(|| format!("Failed to copy binary to {}", dest.display()))?;

    println!("\n✓ Bundle complete!");
    println!("  Single binary: {}", dest.display());
    if let Some(t) = target {
        println!("  Target: {}", t);
    }
    println!("  Size: {:.1} MB", fs::metadata(&dest)?.len() as f64 / 1_048_576.0);
    println!("\n  Run with: ./{}", dest.display());
    println!("  Then open: http://localhost:3000");

    Ok(())
}

/// Scaffold a complete Vite + React project around the generated TS components.
fn scaffold_react_app(app_dir: &Path, generated_ts_dir: &Path) -> Result<()> {
    let src_dir = app_dir.join("src");
    let generated_dir = src_dir.join("generated");

    fs::create_dir_all(&generated_dir)
        .context("Failed to create app/src/generated directory")?;

    // Write template files
    fs::write(app_dir.join("index.html"), templates::index_html())
        .context("Failed to write index.html")?;

    fs::write(app_dir.join("package.json"), templates::package_json())
        .context("Failed to write package.json")?;

    fs::write(app_dir.join("vite.config.ts"), templates::vite_config(3000))
        .context("Failed to write vite.config.ts")?;

    fs::write(app_dir.join("tsconfig.json"), templates::tsconfig_json())
        .context("Failed to write tsconfig.json")?;

    fs::write(src_dir.join("index.css"), templates::index_css())
        .context("Failed to write index.css")?;

    // Find the main component name from generated TSX files
    let component_name = find_component_name(generated_ts_dir)?;

    fs::write(src_dir.join("main.tsx"), templates::main_tsx(&component_name))
        .context("Failed to write main.tsx")?;

    // Copy all generated TS/TSX files into app/src/generated/
    for entry in fs::read_dir(generated_ts_dir)
        .context("Failed to read generated TS directory")?
    {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "tsx" || ext == "ts" {
                let dest = generated_dir.join(path.file_name().unwrap());
                fs::copy(&path, &dest)
                    .with_context(|| format!("Failed to copy {} to generated/", path.display()))?;
            }
        }
    }

    Ok(())
}

/// Find the name of the main React component in the generated output.
fn find_component_name(generated_dir: &Path) -> Result<String> {
    for entry in fs::read_dir(generated_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "tsx") {
            if let Some(stem) = path.file_stem() {
                let name = stem.to_string_lossy().to_string();
                // Skip types.ts files, look for component-like names (PascalCase)
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    return Ok(name);
                }
            }
        }
    }
    // Default fallback
    Ok("App".to_string())
}

/// Run npm install and build in the scaffolded project.
fn npm_install_and_build(app_dir: &Path) -> Result<()> {
    // npm install
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    println!("  Running npm install...");
    let install_status = std::process::Command::new(npm)
        .arg("install")
        .arg("--prefer-offline")
        .current_dir(app_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run npm install. Is Node.js/npm installed?")?;

    if !install_status.success() {
        anyhow::bail!("npm install failed");
    }

    // npm run build
    println!("  Running npm run build...");
    let build_status = std::process::Command::new(npm)
        .arg("run")
        .arg("build")
        .current_dir(app_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run npm run build")?;

    if !build_status.success() {
        anyhow::bail!("npm run build failed");
    }

    Ok(())
}

/// Copy built static assets from Vite output to the backend's public directory.
fn copy_built_assets(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        anyhow::bail!("Built assets not found at {}", from.display());
    }

    // Clean and recreate public dir
    if to.exists() {
        fs::remove_dir_all(to).ok();
    }
    fs::create_dir_all(to)?;

    copy_dir_recursive(from, to)
        .with_context(|| format!("Failed to copy assets from {} to {}", from.display(), to.display()))?;

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());

        if from_path.is_dir() {
            fs::create_dir_all(&to_path)?;
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path)?;
        }
    }
    Ok(())
}

/// Build the generated Rust backend into a single binary.
/// Optionally cross-compiles for a specific target triple.
fn build_single_binary(generated_dir: &Path, target: Option<&str>, release: bool) -> Result<PathBuf> {
    // If cross-compiling, ensure the target is installed
    if let Some(target_triple) = target {
        println!("  Installing target: {}", target_triple);
        let rustup = if cfg!(windows) { "rustup.exe" } else { "rustup" };
        let _ = std::process::Command::new(rustup)
            .args(["target", "add", target_triple])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }

    let cargo = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let mut cmd = std::process::Command::new(cargo);
    cmd.arg("build");

    if release {
        cmd.arg("--release");
    }

    if let Some(target_triple) = target {
        cmd.args(["--target", target_triple]);
    }

    cmd.current_dir(generated_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());

    println!("  Running: {:?}", cmd);
    let status = cmd.status()
        .context("Failed to run cargo build on generated backend")?;

    if !status.success() {
        anyhow::bail!("cargo build failed");
    }

    // Determine binary path
    let profile = if release { "release" } else { "debug" };
    let binary_name = if cfg!(windows) && target.is_none() {
        "vox_generated_app.exe"
    } else if target.map_or(false, |t| t.contains("windows")) {
        "vox_generated_app.exe"
    } else {
        "vox_generated_app"
    };

    let binary_path = if let Some(target_triple) = target {
        generated_dir.join("target").join(target_triple).join(profile).join(binary_name)
    } else {
        generated_dir.join("target").join(profile).join(binary_name)
    };

    if !binary_path.exists() {
        anyhow::bail!("Binary not found at {}", binary_path.display());
    }

    Ok(binary_path)
}
