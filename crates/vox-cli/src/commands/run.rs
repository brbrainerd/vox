use crate::commands::build;
use crate::templates;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub async fn run(file: &Path, args: &[String]) -> Result<()> {
    // 1. Build using existing build command logic
    let out_dir = PathBuf::from("dist");

    println!("Building {}...", file.display());
    build::run(file, &out_dir).await?;

    // 2. Check if we have frontend components to bundle
    let has_frontend = fs::read_dir(&out_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|ext| ext == "tsx"))
        })
        .unwrap_or(false);

    if has_frontend {
        println!("\nBundling frontend...");
        build_frontend(&out_dir)?;
    }

    // 3. Run backend (Rust)
    let generated_dir = Path::new("target").join("generated");

    println!("\nStarting server...");
    if has_frontend {
        println!("  Frontend + Backend at http://localhost:3000");
    } else {
        println!("  Backend at http://localhost:3000");
    }
    println!("  Press Ctrl+C to stop\n");

    let status = Command::new("cargo")
        .arg("run")
        .arg("--")
        .args(args)
        .current_dir(&generated_dir)
        .status()
        .context("Failed to execute cargo run in generated directory")?;

    if !status.success() {
        anyhow::bail!("Application exited with error code: {:?}", status.code());
    }

    Ok(())
}

/// Build the frontend React application and copy assets to backend public dir.
fn build_frontend(generated_ts_dir: &Path) -> Result<()> {
    let app_dir = generated_ts_dir.join("app");
    let src_dir = app_dir.join("src");
    let gen_dir = src_dir.join("generated");

    fs::create_dir_all(&gen_dir).context("Failed to create frontend app directory")?;

    // Write template files
    fs::write(app_dir.join("index.html"), templates::index_html())?;
    fs::write(app_dir.join("package.json"), templates::package_json())?;
    fs::write(app_dir.join("vite.config.ts"), templates::vite_config(3000))?;
    fs::write(app_dir.join("tsconfig.json"), templates::tsconfig_json())?;
    fs::write(src_dir.join("index.css"), templates::index_css())?;

    // Find component name and write main.tsx
    let component_name = find_component_name(generated_ts_dir)?;
    fs::write(
        src_dir.join("main.tsx"),
        templates::main_tsx(&component_name),
    )?;

    // Copy generated TS/TSX files
    for entry in fs::read_dir(generated_ts_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "tsx" || ext == "ts" {
                    let dest = gen_dir.join(path.file_name().unwrap());
                    fs::copy(&path, &dest)?;
                }
            }
        }
    }

    // npm install (skip if node_modules exists and is fresh)
    let node_modules = app_dir.join("node_modules");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    if !node_modules.exists() {
        println!("  Installing npm dependencies...");
        let status = Command::new(npm)
            .args(["install", "--prefer-offline"])
            .current_dir(&app_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to run npm install. Is Node.js installed?")?;

        if !status.success() {
            anyhow::bail!("npm install failed");
        }
    }

    // npm run build
    println!("  Building frontend assets...");
    let status = Command::new(npm)
        .args(["run", "build"])
        .current_dir(&app_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to build frontend")?;

    if !status.success() {
        anyhow::bail!("Frontend build failed");
    }

    // Copy built assets to target/generated/public/
    let public_dir = Path::new("target").join("generated").join("public");
    let built_dir = app_dir.join("dist");

    if built_dir.exists() {
        if public_dir.exists() {
            fs::remove_dir_all(&public_dir).ok();
        }
        fs::create_dir_all(&public_dir)?;
        copy_dir_recursive(&built_dir, &public_dir)?;
        println!("  Frontend assets copied to {}", public_dir.display());
    }

    Ok(())
}

fn find_component_name(dir: &Path) -> Result<String> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "tsx") {
            if let Some(stem) = path.file_stem() {
                let name = stem.to_string_lossy().to_string();
                if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    return Ok(name);
                }
            }
        }
    }
    Ok("App".to_string())
}

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
